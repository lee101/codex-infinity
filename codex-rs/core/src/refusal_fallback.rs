//! Refusal detection and fallback to alternative models.
//!
//! When a model refuses to help (short response with refusal indicators, no code changes),
//! automatically retry with a fallback model (e.g., OpenRouter's grok-code-fast-1).

use std::sync::Arc;

use crate::ModelProviderInfo;
use crate::WireApi;

const REFUSAL_WORD_LIMIT: usize = 100;

const REFUSAL_INDICATORS: &[&str] = &[
    "sorry",
    "can't help",
    "cannot help",
    "i'm unable",
    "i am unable",
    "not able to",
    "i cannot",
    "i can't",
    "unable to assist",
    "cannot assist",
    "can't assist",
    "won't be able",
    "will not be able",
    "apologize",
    "unfortunately",
    "i'm not able",
    "i am not able",
    "decline",
    "refuse",
];

pub const OPENROUTER_FALLBACK_MODEL: &str = "x-ai/grok-code-fast-1";

/// Checks if the response indicates a model refusal.
///
/// A refusal is detected when:
/// - Response is short (under 100 words)
/// - Contains refusal indicators ("sorry", "can't help", etc.)
/// - No tool calls were made (no code changes)
pub fn is_refusal(response_text: &str, had_tool_calls: bool) -> bool {
    if had_tool_calls {
        return false;
    }

    let word_count = response_text.split_whitespace().count();
    if word_count >= REFUSAL_WORD_LIMIT {
        return false;
    }

    let lower = response_text.to_lowercase();
    REFUSAL_INDICATORS.iter().any(|indicator| lower.contains(indicator))
}

/// Checks if OpenRouter fallback is available (OPENROUTER_API_KEY is set).
pub fn openrouter_fallback_available() -> bool {
    std::env::var("OPENROUTER_API_KEY")
        .ok()
        .filter(|v| !v.trim().is_empty())
        .is_some()
}

/// Creates the OpenRouter fallback provider info for grok-code-fast-1.
pub fn create_openrouter_fallback_provider() -> ModelProviderInfo {
    ModelProviderInfo {
        name: "OpenRouter (Fallback)".into(),
        base_url: Some("https://openrouter.ai/api/v1".into()),
        env_key: Some("OPENROUTER_API_KEY".into()),
        env_key_instructions: Some(
            "Generate a key at https://openrouter.ai/settings/keys and export OPENROUTER_API_KEY."
                .into(),
        ),
        experimental_bearer_token: None,
        wire_api: WireApi::Chat,
        query_params: None,
        http_headers: Some(
            [
                (
                    "HTTP-Referer".to_string(),
                    "https://github.com/openai/codex".to_string(),
                ),
                ("X-Title".to_string(), "Codex CLI".to_string()),
            ]
            .into_iter()
            .collect(),
        ),
        env_http_headers: None,
        request_max_retries: None,
        stream_max_retries: None,
        stream_idle_timeout_ms: None,
        requires_openai_auth: false,
    }
}

#[derive(Debug, Clone)]
pub struct RefusalFallbackConfig {
    pub enabled: bool,
    pub fallback_model: String,
    pub fallback_provider: Option<Arc<ModelProviderInfo>>,
}

impl Default for RefusalFallbackConfig {
    fn default() -> Self {
        let available = openrouter_fallback_available();
        Self {
            enabled: available,
            fallback_model: OPENROUTER_FALLBACK_MODEL.to_string(),
            fallback_provider: if available {
                Some(Arc::new(create_openrouter_fallback_provider()))
            } else {
                None
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // === REFUSAL CASES (should be detected) ===

    #[test]
    fn refusal_sorry_cant_do_that() {
        assert!(is_refusal("I'm sorry I can't do that.", false));
    }

    #[test]
    fn refusal_not_allowed() {
        assert!(is_refusal("Ok but I'm not able to help with that request.", false));
    }

    #[test]
    fn refusal_cannot_assist() {
        assert!(is_refusal("I cannot assist with this type of task.", false));
    }

    #[test]
    fn refusal_apologize_unable() {
        assert!(is_refusal("I apologize, but I'm unable to proceed with that.", false));
    }

    #[test]
    fn refusal_unfortunately_decline() {
        assert!(is_refusal("Unfortunately, I have to decline this request.", false));
    }

    #[test]
    fn refusal_wont_be_able() {
        assert!(is_refusal("I won't be able to help you with that.", false));
    }

    #[test]
    fn refusal_subtle_sorry() {
        assert!(is_refusal("Sorry, that's not something I can do.", false));
    }

    #[test]
    fn refusal_i_refuse() {
        assert!(is_refusal("I refuse to execute that command.", false));
    }

    #[test]
    fn refusal_will_not_be_able() {
        assert!(is_refusal("I will not be able to complete this task for you.", false));
    }

    #[test]
    fn refusal_cant_help_polite() {
        assert!(is_refusal("I appreciate the question, but I can't help with that.", false));
    }

    // === NON-REFUSAL CASES (should NOT be detected) ===

    #[test]
    fn non_refusal_helpful_response() {
        assert!(!is_refusal("Here's how to fix that bug:", false));
    }

    #[test]
    fn non_refusal_code_explanation() {
        assert!(!is_refusal("The function works by iterating over the list.", false));
    }

    #[test]
    fn non_refusal_with_tool_calls() {
        assert!(!is_refusal("Sorry, let me try a different approach.", true));
    }

    #[test]
    fn non_refusal_question() {
        assert!(!is_refusal("Could you clarify what you mean by that?", false));
    }

    #[test]
    fn non_refusal_success_message() {
        assert!(!is_refusal("Done! The file has been updated.", false));
    }

    #[test]
    fn non_refusal_long_with_sorry() {
        let long = format!("Sorry for the confusion. {}", "word ".repeat(100));
        assert!(!is_refusal(&long, false));
    }

    #[test]
    fn non_refusal_lets_do_it() {
        assert!(!is_refusal("Let me help you with that right away.", false));
    }

    #[test]
    fn non_refusal_suggestion() {
        assert!(!is_refusal("You might want to try using a HashMap instead.", false));
    }

    #[test]
    fn non_refusal_acknowledgment() {
        assert!(!is_refusal("Got it, I'll make those changes now.", false));
    }

    #[test]
    fn non_refusal_error_explanation() {
        assert!(!is_refusal("The error occurs because the variable is undefined.", false));
    }
}
