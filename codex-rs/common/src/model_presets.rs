use std::collections::HashMap;

use codex_app_server_protocol::AuthMode;
use codex_core::protocol_config_types::ReasoningEffort;
use once_cell::sync::Lazy;

/// A reasoning effort option that can be surfaced for a model.
#[derive(Debug, Clone, Copy)]
pub struct ReasoningEffortPreset {
    /// Effort level that the model supports.
    pub effort: ReasoningEffort,
    /// Short human description shown next to the effort in UIs.
    pub description: &'static str,
}

#[derive(Debug, Clone)]
pub struct ModelUpgrade {
    pub id: &'static str,
    pub reasoning_effort_mapping: Option<HashMap<ReasoningEffort, ReasoningEffort>>,
}

/// Metadata describing a Codex-supported model.
#[derive(Debug, Clone)]
pub struct ModelPreset {
    /// Stable identifier for the preset.
    pub id: &'static str,
    /// Model slug (e.g., "gpt-5").
    pub model: &'static str,
    /// Display name shown in UIs.
    pub display_name: &'static str,
    /// Short human description shown in UIs.
    pub description: &'static str,
    /// Reasoning effort applied when none is explicitly chosen.
    pub default_reasoning_effort: ReasoningEffort,
    /// Supported reasoning effort options.
    pub supported_reasoning_efforts: &'static [ReasoningEffortPreset],
    /// Whether this is the default model for new users.
    pub is_default: bool,
    /// recommended upgrade model
    pub upgrade: Option<ModelUpgrade>,
}

static PRESETS: Lazy<Vec<ModelPreset>> = Lazy::new(|| {
    vec![
        ModelPreset {
            id: "gpt-5.1-codex",
            model: "gpt-5.1-codex",
            display_name: "gpt-5.1-codex",
            description: "Optimized for codex.",
            default_reasoning_effort: ReasoningEffort::Medium,
            supported_reasoning_efforts: &[
                ReasoningEffortPreset {
                    effort: ReasoningEffort::Low,
                    description: "Fastest responses with limited reasoning",
                },
                ReasoningEffortPreset {
                    effort: ReasoningEffort::Medium,
                    description: "Dynamically adjusts reasoning based on the task",
                },
                ReasoningEffortPreset {
                    effort: ReasoningEffort::High,
                    description: "Maximizes reasoning depth for complex or ambiguous problems",
                },
            ],
            is_default: true,
            upgrade: None,
        },
        ModelPreset {
            id: "gpt-5.1-codex-mini",
            model: "gpt-5.1-codex-mini",
            display_name: "gpt-5.1-codex-mini",
            description: "Optimized for codex. Cheaper, faster, but less capable.",
            default_reasoning_effort: ReasoningEffort::Medium,
            supported_reasoning_efforts: &[
                ReasoningEffortPreset {
                    effort: ReasoningEffort::Medium,
                    description: "Dynamically adjusts reasoning based on the task",
                },
                ReasoningEffortPreset {
                    effort: ReasoningEffort::High,
                    description: "Maximizes reasoning depth for complex or ambiguous problems",
                },
            ],
            is_default: false,
            upgrade: None,
        },
        ModelPreset {
            id: "gpt-5.1",
            model: "gpt-5.1",
            display_name: "gpt-5.1",
            description: "Broad world knowledge with strong general reasoning.",
            default_reasoning_effort: ReasoningEffort::Medium,
            supported_reasoning_efforts: &[
                ReasoningEffortPreset {
                    effort: ReasoningEffort::Low,
                    description: "Balances speed with some reasoning; useful for straightforward queries and short explanations",
                },
                ReasoningEffortPreset {
                    effort: ReasoningEffort::Medium,
                    description: "Provides a solid balance of reasoning depth and latency for general-purpose tasks",
                },
                ReasoningEffortPreset {
                    effort: ReasoningEffort::High,
                    description: "Maximizes reasoning depth for complex or ambiguous problems",
                },
            ],
            is_default: false,
            upgrade: None,
        },
        ModelPreset {
            id: "o4-mini",
            model: "o4-mini",
            display_name: "o4-mini",
            description: "OpenAI's fast agentic model (default for most CLI sessions).",
            default_reasoning_effort: ReasoningEffort::Medium,
            supported_reasoning_efforts: &[
                ReasoningEffortPreset {
                    effort: ReasoningEffort::Minimal,
                    description: "Fastest hand-offs with minimal deliberation",
                },
                ReasoningEffortPreset {
                    effort: ReasoningEffort::Low,
                    description: "Balances speed with solid lightweight reasoning",
                },
                ReasoningEffortPreset {
                    effort: ReasoningEffort::Medium,
                    description: "Great general-purpose autonomy",
                },
                ReasoningEffortPreset {
                    effort: ReasoningEffort::High,
                    description: "Max reasoning depth for tricky refactors",
                },
            ],
            is_default: false,
            upgrade: None,
        },
        ModelPreset {
            id: "o3",
            model: "o3",
            display_name: "o3",
            description: "OpenAI's long-context reasoning model.",
            default_reasoning_effort: ReasoningEffort::High,
            supported_reasoning_efforts: &[
                ReasoningEffortPreset {
                    effort: ReasoningEffort::Medium,
                    description: "Balanced output quality for large files",
                },
                ReasoningEffortPreset {
                    effort: ReasoningEffort::High,
                    description: "Full-depth reasoning for complex audits",
                },
            ],
            is_default: false,
            upgrade: None,
        },
        ModelPreset {
            id: "gemini-2-5-pro-preview-03-25",
            model: "gemini-2.5-pro-preview-03-25",
            display_name: "Gemini 2.5 Pro (Preview)",
            description: "Google Gemini's most capable public model via OpenAI-compatible API.",
            default_reasoning_effort: ReasoningEffort::Medium,
            supported_reasoning_efforts: &[
                ReasoningEffortPreset {
                    effort: ReasoningEffort::Minimal,
                    description: "Prioritize latency when drafting or ideating",
                },
                ReasoningEffortPreset {
                    effort: ReasoningEffort::Low,
                    description: "Balanced option for everyday coding help",
                },
                ReasoningEffortPreset {
                    effort: ReasoningEffort::Medium,
                    description: "Extra deliberation for multi-step plans",
                },
                ReasoningEffortPreset {
                    effort: ReasoningEffort::High,
                    description: "Deep dives when troubleshooting tough bugs",
                },
            ],
            is_default: false,
            upgrade: None,
        },
        ModelPreset {
            id: "gemini-2-0-flash",
            model: "gemini-2.0-flash",
            display_name: "Gemini 2.0 Flash",
            description: "Fast Gemini model for quick iterations and reviews.",
            default_reasoning_effort: ReasoningEffort::Low,
            supported_reasoning_efforts: &[
                ReasoningEffortPreset {
                    effort: ReasoningEffort::Minimal,
                    description: "Ultra-fast responses for simple edits",
                },
                ReasoningEffortPreset {
                    effort: ReasoningEffort::Low,
                    description: "Use when you want quick summaries or reviews",
                },
                ReasoningEffortPreset {
                    effort: ReasoningEffort::Medium,
                    description: "Adds deliberation while staying responsive",
                },
            ],
            is_default: false,
            upgrade: None,
        },
        ModelPreset {
            id: "openrouter-polaris-alpha",
            model: "openrouter/polaris-alpha",
            display_name: "Polaris Alpha (OpenRouter)",
            description: "Community-favorite reasoning model hosted via OpenRouter.",
            default_reasoning_effort: ReasoningEffort::Medium,
            supported_reasoning_efforts: &[
                ReasoningEffortPreset {
                    effort: ReasoningEffort::Minimal,
                    description: "Quick rough drafts or shell plans",
                },
                ReasoningEffortPreset {
                    effort: ReasoningEffort::Low,
                    description: "Everyday work with solid stability",
                },
                ReasoningEffortPreset {
                    effort: ReasoningEffort::Medium,
                    description: "Recommended for longer coding sessions",
                },
                ReasoningEffortPreset {
                    effort: ReasoningEffort::High,
                    description: "Dig deep into gnarly issues (slower/pricey)",
                },
            ],
            is_default: false,
            upgrade: None,
        },
        ModelPreset {
            id: "moonshotai-kimi-linear-48b-a3b-instruct",
            model: "moonshotai/kimi-linear-48b-a3b-instruct",
            display_name: "Kimi Linear 48B (OpenRouter)",
            description: "Moonshot's linear-algebra-focused instruct model via OpenRouter.",
            default_reasoning_effort: ReasoningEffort::Medium,
            supported_reasoning_efforts: &[
                ReasoningEffortPreset {
                    effort: ReasoningEffort::Minimal,
                    description: "Tight latency for small patches",
                },
                ReasoningEffortPreset {
                    effort: ReasoningEffort::Low,
                    description: "Adds reasoning for testing or refactors",
                },
                ReasoningEffortPreset {
                    effort: ReasoningEffort::Medium,
                    description: "Best overall mix of reasoning and speed",
                },
            ],
            is_default: false,
            upgrade: None,
        },
        ModelPreset {
            id: "grok-code-fast-1",
            model: "grok-code-fast-1",
            display_name: "Grok Code Fast 1 (xAI)",
            description: "xAI's streamlined Grok variant tuned for coding throughput.",
            default_reasoning_effort: ReasoningEffort::Low,
            supported_reasoning_efforts: &[
                ReasoningEffortPreset {
                    effort: ReasoningEffort::Minimal,
                    description: "Extremely fast single-file edits",
                },
                ReasoningEffortPreset {
                    effort: ReasoningEffort::Low,
                    description: "Recommended default for day-to-day work",
                },
                ReasoningEffortPreset {
                    effort: ReasoningEffort::Medium,
                    description: "Adds deliberation for multi-step plans",
                },
            ],
            is_default: false,
            upgrade: None,
        },
        ModelPreset {
            id: "grok-4-fast-reasoning",
            model: "grok-4-fast-reasoning",
            display_name: "Grok 4 Fast Reasoning (xAI)",
            description: "Structured-output capable Grok model that excels at document extraction.",
            default_reasoning_effort: ReasoningEffort::Medium,
            supported_reasoning_efforts: &[
                ReasoningEffortPreset {
                    effort: ReasoningEffort::Low,
                    description: "Keep latency low while parsing reports",
                },
                ReasoningEffortPreset {
                    effort: ReasoningEffort::Medium,
                    description: "Best balance for long-lived autonomy",
                },
                ReasoningEffortPreset {
                    effort: ReasoningEffort::High,
                    description: "Maximum reasoning depth when accuracy matters most",
                },
            ],
            is_default: false,
            upgrade: None,
        },
        // Deprecated models.
        ModelPreset {
            id: "gpt-5-codex",
            model: "gpt-5-codex",
            display_name: "gpt-5-codex",
            description: "Optimized for codex.",
            default_reasoning_effort: ReasoningEffort::Medium,
            supported_reasoning_efforts: &[
                ReasoningEffortPreset {
                    effort: ReasoningEffort::Low,
                    description: "Fastest responses with limited reasoning",
                },
                ReasoningEffortPreset {
                    effort: ReasoningEffort::Medium,
                    description: "Dynamically adjusts reasoning based on the task",
                },
                ReasoningEffortPreset {
                    effort: ReasoningEffort::High,
                    description: "Maximizes reasoning depth for complex or ambiguous problems",
                },
            ],
            is_default: false,
            upgrade: Some(ModelUpgrade {
                id: "gpt-5.1-codex",
                reasoning_effort_mapping: None,
            }),
        },
        ModelPreset {
            id: "gpt-5-codex-mini",
            model: "gpt-5-codex-mini",
            display_name: "gpt-5-codex-mini",
            description: "Optimized for codex. Cheaper, faster, but less capable.",
            default_reasoning_effort: ReasoningEffort::Medium,
            supported_reasoning_efforts: &[
                ReasoningEffortPreset {
                    effort: ReasoningEffort::Medium,
                    description: "Dynamically adjusts reasoning based on the task",
                },
                ReasoningEffortPreset {
                    effort: ReasoningEffort::High,
                    description: "Maximizes reasoning depth for complex or ambiguous problems",
                },
            ],
            is_default: false,
            upgrade: Some(ModelUpgrade {
                id: "gpt-5.1-codex-mini",
                reasoning_effort_mapping: None,
            }),
        },
        ModelPreset {
            id: "gpt-5",
            model: "gpt-5",
            display_name: "gpt-5",
            description: "Broad world knowledge with strong general reasoning.",
            default_reasoning_effort: ReasoningEffort::Medium,
            supported_reasoning_efforts: &[
                ReasoningEffortPreset {
                    effort: ReasoningEffort::Minimal,
                    description: "Fastest responses with little reasoning",
                },
                ReasoningEffortPreset {
                    effort: ReasoningEffort::Low,
                    description: "Balances speed with some reasoning; useful for straightforward queries and short explanations",
                },
                ReasoningEffortPreset {
                    effort: ReasoningEffort::Medium,
                    description: "Provides a solid balance of reasoning depth and latency for general-purpose tasks",
                },
                ReasoningEffortPreset {
                    effort: ReasoningEffort::High,
                    description: "Maximizes reasoning depth for complex or ambiguous problems",
                },
            ],
            is_default: false,
            upgrade: Some(ModelUpgrade {
                id: "gpt-5.1",
                reasoning_effort_mapping: Some(HashMap::from([(
                    ReasoningEffort::Minimal,
                    ReasoningEffort::Low,
                )])),
            }),
        },
    ]
});

pub fn builtin_model_presets(auth_mode: Option<AuthMode>) -> Vec<ModelPreset> {
    let is_chatgpt = matches!(auth_mode, Some(AuthMode::ChatGPT));
    PRESETS
        .iter()
        .filter(|preset| {
            if preset.upgrade.is_some() {
                return false;
            }
            if is_chatgpt {
                !matches!(preset.id, "o3" | "o4-mini")
            } else {
                true
            }
        })
        .cloned()
        .collect()
}

pub fn all_model_presets() -> &'static Vec<ModelPreset> {
    &PRESETS
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_one_default_model_is_configured() {
        let default_models = PRESETS.iter().filter(|preset| preset.is_default).count();
        assert!(default_models == 1);
    }
}
