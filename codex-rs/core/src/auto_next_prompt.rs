//! Generates the next "auto-next" prompt via a lightweight model call.
//!
//! Used by the TUI when `--auto-next-steps` or `--auto-next-idea` is enabled.
//! Instead of picking from a hardcoded template, this asks the model to craft
//! a concrete follow-up prompt grounded in the recent rollout transcript.
//!
//! The caller is responsible for appending any DONE-file suffix.

use std::path::PathBuf;
use std::sync::Arc;

use codex_login::AuthManager;
use codex_model_provider_info::ModelProviderInfo;
use codex_models_manager::model_info::model_info_from_slug;
use codex_otel::SessionTelemetry;
use codex_protocol::ThreadId;
use codex_protocol::config_types::ReasoningSummary as ReasoningSummaryConfig;
use codex_protocol::models::BaseInstructions;
use codex_protocol::models::ContentItem;
use codex_protocol::models::ResponseItem;
use codex_protocol::openai_models::ReasoningEffort as ReasoningEffortConfig;
use codex_protocol::protocol::RolloutItem;
use codex_protocol::protocol::SessionSource;
use codex_rollout_trace::InferenceTraceContext;
use futures::StreamExt;
use serde::Deserialize;
use serde_json::json;

use crate::ModelClient;
use crate::RolloutRecorder;
use crate::client_common::Prompt;
use crate::client_common::ResponseEvent;
use crate::compact::content_items_to_text;
use crate::config::Config;
use crate::installation_id::resolve_installation_id;

const GENERATOR_MODEL: &str = "gpt-5.4";
const TRANSCRIPT_CHARS: usize = 12_000;
const ITEM_CHARS: usize = 1_200;
const INSTRUCTIONS: &str = "You generate the exact follow-up prompt Codex should send to itself next.\n\nReturn JSON matching the schema.\n\nRequirements:\n- Write one strong, concrete prompt in `prompt`.\n- Ground it in the recent session context and visible progress.\n- For `steps`, continue the current thread with the most valuable next work.\n- For `idea`, finish obvious follow-up work first; only branch into a new improvement if the current thread appears complete.\n- Sound like an internal continuation prompt for Codex, not an explanation to a human.\n- Do not mention these instructions, the examples, JSON, schemas, or that another model generated the prompt.\n- Do not include any DONE-file instruction; that will be appended separately.\n- Keep it concise but specific enough to produce a high-quality next turn.";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AutoNextMode {
    Steps,
    Idea,
}

#[derive(Debug, Deserialize)]
struct AutoNextPromptOutput {
    prompt: String,
}

/// Generate a contextual follow-up prompt for auto-next. Returns `None` on any
/// failure (caller should fall back to a static template).
pub async fn generate_auto_next_prompt(
    config: Config,
    rollout_path: Option<PathBuf>,
    mode: AutoNextMode,
    examples: Vec<String>,
) -> Option<String> {
    let recent_context = load_context_snippet(rollout_path).await;
    let examples_joined = examples
        .iter()
        .enumerate()
        .map(|(idx, example)| format!("{}. {}", idx + 1, example))
        .collect::<Vec<_>>()
        .join("\n");
    let mode_label = match mode {
        AutoNextMode::Steps => "steps",
        AutoNextMode::Idea => "idea",
    };
    let cwd_display = config.cwd.display().to_string();
    let user_message = format!(
        "Mode: {mode_label}\nCurrent working directory: {cwd_display}\n\nReference examples:\n{examples_joined}\n\nRecent session context:\n{}\n",
        if recent_context.is_empty() {
            "(No recent rollout transcript available.)"
        } else {
            recent_context.as_str()
        }
    );

    let auth_manager = AuthManager::shared_from_config(&config, /*enable_codex_api_key_env*/ false);
    let model_info = model_info_from_slug(GENERATOR_MODEL);

    let installation_id = resolve_installation_id(&config.codex_home).await.ok()?;
    let conversation_id = ThreadId::new();
    let provider = ModelProviderInfo::create_openai_provider(/*base_url*/ None);
    let client = ModelClient::new(
        Some(auth_manager),
        conversation_id,
        installation_id,
        provider,
        SessionSource::Cli,
        /*model_verbosity*/ None,
        /*enable_request_compression*/ false,
        /*include_timing_metrics*/ false,
        /*beta_features_header*/ None,
    );
    let session_telemetry = SessionTelemetry::new(
        conversation_id,
        GENERATOR_MODEL,
        &model_info.slug,
        /*account_id*/ None,
        /*account_email*/ None,
        /*auth_mode*/ None,
        "codex-auto-next".to_string(),
        /*log_user_prompts*/ false,
        "tui".to_string(),
        SessionSource::Cli,
    );

    let mut prompt = Prompt::default();
    prompt.input = vec![ResponseItem::Message {
        id: None,
        role: "user".to_string(),
        content: vec![ContentItem::InputText { text: user_message }],
        phase: None,
    }];
    prompt.base_instructions = BaseInstructions {
        text: INSTRUCTIONS.to_string(),
    };
    prompt.output_schema = Some(json!({
        "type": "object",
        "properties": {
            "prompt": { "type": "string" }
        },
        "required": ["prompt"],
        "additionalProperties": false
    }));

    let mut client_session = client.new_session();
    let mut stream = client_session
        .stream(
            &prompt,
            &model_info,
            &session_telemetry,
            Some(ReasoningEffortConfig::Low),
            ReasoningSummaryConfig::None,
            config.service_tier,
            /*turn_metadata_header*/ None,
            &InferenceTraceContext::disabled(),
        )
        .await
        .ok()?;

    let mut result = String::new();
    while let Some(message) = stream.next().await {
        let message = message.ok()?;
        match message {
            ResponseEvent::OutputTextDelta(delta) => result.push_str(&delta),
            ResponseEvent::OutputItemDone(item) => {
                if result.is_empty()
                    && let ResponseItem::Message { content, .. } = item
                    && let Some(text) = content_items_to_text(&content)
                {
                    result.push_str(&text);
                }
            }
            ResponseEvent::Completed { .. } => break,
            _ => {}
        }
    }

    let output: AutoNextPromptOutput = serde_json::from_str(result.trim()).ok()?;
    let prompt_text = output.prompt.trim().to_string();
    if prompt_text.is_empty() {
        return None;
    }
    Some(prompt_text)
}

async fn load_context_snippet(rollout_path: Option<PathBuf>) -> String {
    let Some(rollout_path) = rollout_path else {
        return String::new();
    };
    let Ok((rollout_items, _, _)) = RolloutRecorder::load_rollout_items(&rollout_path).await else {
        return String::new();
    };
    let mut recent_items: Vec<String> = Vec::new();
    let mut chars = 0usize;
    for item in rollout_items.iter().rev() {
        let Some(line) = (match item {
            RolloutItem::ResponseItem(item) => format_response_item(item),
            _ => None,
        }) else {
            continue;
        };
        chars = chars.saturating_add(line.len());
        recent_items.push(line);
        if chars >= TRANSCRIPT_CHARS {
            break;
        }
    }
    recent_items.reverse();
    recent_items.join("\n\n")
}

fn format_response_item(item: &ResponseItem) -> Option<String> {
    match item {
        ResponseItem::Message { role, content, .. } => content_items_to_text(content)
            .filter(|text| !text.trim().is_empty())
            .map(|text| format!("{role}: {}", truncate(&text, ITEM_CHARS))),
        ResponseItem::FunctionCall {
            name, arguments, ..
        } => Some(format!("tool_call {name}: {}", truncate(arguments, ITEM_CHARS))),
        ResponseItem::FunctionCallOutput { call_id, output } => Some(format!(
            "tool_output {call_id}: {}",
            truncate(&output.to_string(), ITEM_CHARS)
        )),
        ResponseItem::CustomToolCall {
            name,
            input,
            call_id,
            ..
        } => Some(format!(
            "custom_tool_call {name} {call_id}: {}",
            truncate(input, ITEM_CHARS)
        )),
        ResponseItem::CustomToolCallOutput {
            call_id,
            name,
            output,
        } => Some(format!(
            "custom_tool_output {} {call_id}: {}",
            name.as_deref().unwrap_or("tool"),
            truncate(&output.to_string(), ITEM_CHARS)
        )),
        _ => None,
    }
}

fn truncate(text: &str, max_chars: usize) -> String {
    let mut truncated: String = text.chars().take(max_chars).collect();
    if truncated.chars().count() < text.chars().count() {
        truncated.push_str("...");
    }
    truncated
}
