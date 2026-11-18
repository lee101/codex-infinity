use codex_protocol::config_types::Personality;
use codex_protocol::config_types::ReasoningSummary;
use codex_protocol::openai_models::ConfigShellToolType;
use codex_protocol::openai_models::ModelInfo;
use codex_protocol::openai_models::ModelMessages;
use codex_protocol::openai_models::ModelVisibility;
use codex_protocol::openai_models::ReasoningEffort;
use codex_protocol::openai_models::TruncationMode;
use codex_protocol::openai_models::TruncationPolicyConfig;
use codex_protocol::openai_models::WebSearchToolType;
use codex_protocol::openai_models::default_input_modalities;

use crate::config::ModelsManagerConfig;
use codex_utils_output_truncation::approx_bytes_for_tokens;
use tracing::warn;

pub const BASE_INSTRUCTIONS: &str = include_str!("../prompt.md");
const PERSONALITY_SECTION_HEADER: &str = "# Personality";

pub fn with_config_overrides(mut model: ModelInfo, config: &ModelsManagerConfig) -> ModelInfo {
    // Preserve Infinity's concise base prompt after a backend catalog refresh.
    // Tool, approval, and collaboration messages still come from that catalog.
    if matches!(
        model.slug.as_str(),
        "gpt-6.2-sol"
            | "gpt-6.2-terra"
            | "gpt-6.1-sol"
            | "gpt-6.1-terra"
            | "gpt-6-sol"
            | "gpt-5.6-sol"
            | "gpt-5.6-terra"
            | "gpt-5.6-luna"
    ) && let Ok(catalog) = crate::bundled_models_response()
        && let Some(bundled) = catalog
            .models
            .into_iter()
            .find(|item| item.slug == model.slug)
        && let Some(messages) = bundled.model_messages
    {
        let active = model.model_messages.get_or_insert_default();
        active.instructions_template = messages.instructions_template;
    }
    if let Some(context_window) = config.model_context_window {
        model.context_window = Some(
            model
                .max_context_window
                .map_or(context_window, |max_context_window| {
                    context_window.min(max_context_window)
                }),
        );
    }
    if let Some(auto_compact_token_limit) = config.model_auto_compact_token_limit {
        model.auto_compact_token_limit = Some(auto_compact_token_limit);
    }
    if let Some(token_limit) = config.tool_output_token_limit {
        model.truncation_policy = match model.truncation_policy.mode {
            TruncationMode::Bytes => {
                let byte_limit =
                    i64::try_from(approx_bytes_for_tokens(token_limit)).unwrap_or(i64::MAX);
                TruncationPolicyConfig::bytes(byte_limit)
            }
            TruncationMode::Tokens => {
                let limit = i64::try_from(token_limit).unwrap_or(i64::MAX);
                TruncationPolicyConfig::tokens(limit)
            }
        };
    }

    if let Some(base_instructions) = &config.base_instructions {
        let model_messages = model.model_messages.get_or_insert_default();
        model_messages.instructions_template = Some(base_instructions.clone());
    } else if config.personality == Some(Personality::None)
        && let Some(instructions_template) = model
            .model_messages
            .as_mut()
            .and_then(|messages| messages.instructions_template.as_mut())
    {
        *instructions_template = strip_personality_section(std::mem::take(instructions_template));
    }

    model
}

fn strip_personality_section(mut instructions: String) -> String {
    let mut section_start = None;
    let mut section_end = None;
    let mut offset = 0;

    for line_with_ending in instructions.split_inclusive('\n') {
        let line = match line_with_ending.strip_suffix('\n') {
            Some(line) => line.strip_suffix('\r').unwrap_or(line),
            None => line_with_ending,
        };
        if section_start.is_some() {
            if is_h1_heading(line) {
                section_end = Some(offset);
                break;
            }
        } else if line == PERSONALITY_SECTION_HEADER {
            section_start = Some(offset);
        }
        offset += line_with_ending.len();
    }

    if let Some(section_start) = section_start {
        let section_end = section_end.unwrap_or(instructions.len());
        instructions.replace_range(section_start..section_end, "");
    }

    instructions
}

fn is_h1_heading(line: &str) -> bool {
    let Some(rest) = line.strip_prefix('#') else {
        return false;
    };
    rest.is_empty() || rest.starts_with(' ') || rest.starts_with('\t')
}

/// Model slugs routed to the DeepSeek provider when no provider is configured.
pub fn is_deepseek_slug(slug: &str) -> bool {
    matches!(
        slug,
        "deepseek-v4-flash" | "deepseek-v4-pro" | "deepseek-v4.1-flash"
    )
}

/// Slugs the fork serves locally with wide-context metadata rather than a generic fallback.
///
/// Distinct from [`is_deepseek_slug`], which only drives provider routing: OpenRouter-hosted
/// slugs are namespaced and must keep ordinary fallback metadata.
fn is_local_wide_context_slug(slug: &str) -> bool {
    is_deepseek_slug(slug) || slug.starts_with("openpaths/") || slug.ends_with("-exp")
}

/// Provider prefix that selects the built-in OpenRouter provider. It is a Codex-side
/// namespace: OpenRouter ids start at the vendor segment, so the prefix never reaches it.
const OPENROUTER_SLUG_PREFIX: &str = "openrouter/";

/// Model id to send to the provider for `slug`.
///
/// The provider prefix on a slug selects the provider and never reaches it: OpenRouter ids
/// start at the vendor segment, and OpenPaths serves the free router as `openpaths-free`.
/// Gateway models also get a short alias for their vendor-namespaced id, and
/// `deepseek-v4.1-flash` is served as `deepseek-flash` and rejects the 4.1 slug ("The
/// supported API model names are deepseek-flash, deepseek-v4-pro").
pub fn wire_model(slug: &str) -> &str {
    match slug {
        "deepseek-v4.1-flash" => "deepseek-flash",
        "openrouter/mimo-v2.6-pro" => "xiaomi/mimo-v2.6-pro",
        "openrouter/space-bunny-alpha" => "stealth/space-bunny-alpha",
        "openpaths/openpaths-free" => "openpaths-free",
        other => other.strip_prefix(OPENROUTER_SLUG_PREFIX).unwrap_or(other),
    }
}

/// Build a minimal fallback model descriptor for missing/unknown slugs.
pub fn model_info_from_slug(slug: &str) -> ModelInfo {
    let is_deepseek = is_deepseek_slug(slug);
    if !is_deepseek {
        warn!("Unknown model {slug} is used. This will use fallback model metadata.");
    }
    ModelInfo {
        used_fallback_model_metadata: !is_deepseek,
        ..fallback_model_info(slug)
    }
}

/// Fallback metadata for `slug` without the unknown-model warning or marker.
///
/// Catalog entries that the fork owns (see `crate::gateway_models`) start from the same
/// descriptor but are known models, so neither the warning nor the fallback marker applies.
pub(crate) fn fallback_model_info(slug: &str) -> ModelInfo {
    let is_deepseek = is_local_wide_context_slug(slug);
    // Callers that serve the slug decide whether it counts as unknown metadata.
    let mut model = ModelInfo {
        used_fallback_model_metadata: false,
        slug: slug.to_string(),
        display_name: slug.to_string(),
        description: None,
        default_reasoning_level: is_deepseek.then_some(ReasoningEffort::High),
        supported_reasoning_levels: Vec::new(),
        shell_type: ConfigShellToolType::UnifiedExec,
        visibility: ModelVisibility::None,
        supported_in_api: true,
        priority: 99,
        additional_speed_tiers: Vec::new(),
        service_tiers: Vec::new(),
        default_service_tier: None,
        available_access_programs: None,
        availability_nux: None,
        upgrade: None,
        model_messages: Some(local_model_messages()),
        include_skills_usage_instructions: false,
        include_plugin_usage_instructions: false,
        include_apps_usage_instructions: false,
        supports_reasoning_summary_parameter: true,
        default_reasoning_summary: ReasoningSummary::Auto,
        support_verbosity: false,
        default_verbosity: None,
        apply_patch_tool_type: None,
        web_search_tool_type: WebSearchToolType::Text,
        truncation_policy: TruncationPolicyConfig::bytes(/*limit*/ 10_000),
        supports_image_detail_original: false,
        context_window: Some(if is_deepseek { 1_000_000 } else { 272_000 }),
        max_context_window: Some(if is_deepseek { 1_000_000 } else { 272_000 }),
        auto_compact_token_limit: None,
        comp_hash: None,
        effective_context_window_percent: 95,
        experimental_supported_tools: Vec::new(),
        input_modalities: default_input_modalities(),
        supports_search_tool: false,
        supports_experimental_context: false,
        use_responses_lite: false,
        supports_reasoning_effort_updates: false,
        guardian: None,
        node_repl_auto_review_required: false,
        node_repl_disabled: false,
        auto_review_model_override: None,
        model_specialty: None,
        tool_mode: None,
        multi_agent_version: None,
        multi_agent_reasoning_effort: None,
    };
    if is_deepseek {
        model.description = Some("DeepSeek V4 native API model".into());
    }
    model
}

fn local_model_messages() -> ModelMessages {
    ModelMessages {
        instructions_template: Some(BASE_INSTRUCTIONS.to_string()),
        ..Default::default()
    }
}

#[cfg(test)]
#[path = "model_info_tests.rs"]
mod tests;
