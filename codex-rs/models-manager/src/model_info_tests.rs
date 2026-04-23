use super::*;
use crate::ModelsManagerConfig;
use pretty_assertions::assert_eq;

#[test]
fn reasoning_summaries_override_true_enables_support() {
    let model = model_info_from_slug("unknown-model");
    let config = ModelsManagerConfig {
        model_supports_reasoning_summaries: Some(true),
        ..Default::default()
    };

    let updated = with_config_overrides(model.clone(), &config);
    let mut expected = model;
    expected.supports_reasoning_summaries = true;

    assert_eq!(updated, expected);
}

#[test]
fn reasoning_summaries_override_false_does_not_disable_support() {
    let mut model = model_info_from_slug("unknown-model");
    model.supports_reasoning_summaries = true;
    let config = ModelsManagerConfig {
        model_supports_reasoning_summaries: Some(false),
        ..Default::default()
    };

    let updated = with_config_overrides(model.clone(), &config);

    assert_eq!(updated, model);
}

#[test]
fn reasoning_summaries_override_false_is_noop_when_model_is_false() {
    let model = model_info_from_slug("unknown-model");
    let config = ModelsManagerConfig {
        model_supports_reasoning_summaries: Some(false),
        ..Default::default()
    };

    let updated = with_config_overrides(model.clone(), &config);

    assert_eq!(updated, model);
}

#[test]
fn model_context_window_override_clamps_to_max_context_window() {
    let mut model = model_info_from_slug("unknown-model");
    model.context_window = Some(273_000);
    model.max_context_window = Some(400_000);
    let config = ModelsManagerConfig {
        model_context_window: Some(500_000),
        ..Default::default()
    };

    let updated = with_config_overrides(model.clone(), &config);
    let mut expected = model;
    expected.context_window = Some(400_000);

    assert_eq!(updated, expected);
}

#[test]
fn model_context_window_uses_model_value_without_override() {
    let mut model = model_info_from_slug("unknown-model");
    model.context_window = Some(273_000);
    model.max_context_window = Some(400_000);
    let config = ModelsManagerConfig::default();

    let updated = with_config_overrides(model.clone(), &config);

    assert_eq!(updated, model);
}

#[test]
fn remote_metadata_uses_compact_local_prompt() {
    let mut model = model_info_from_slug("gpt-5.4");
    model.base_instructions = "remote verbose instructions".repeat(100);
    model.model_messages = Some(ModelMessages {
        instructions_template: Some("remote verbose template {{ personality }}".repeat(100)),
        instructions_variables: Some(ModelInstructionsVariables {
            personality_default: Some(String::new()),
            personality_friendly: Some("remote friendly".to_string()),
            personality_pragmatic: Some("remote pragmatic".to_string()),
        }),
    });

    let updated = with_config_overrides(model, &ModelsManagerConfig::default());

    assert_eq!(updated.base_instructions, BASE_INSTRUCTIONS);
    assert_eq!(updated.model_messages, None);
}

#[test]
fn gpt_5_5_remote_metadata_uses_compact_local_personality_template() {
    let mut model = model_info_from_slug("gpt-5.5");
    model.base_instructions = "remote verbose instructions".repeat(100);
    model.model_messages = Some(ModelMessages {
        instructions_template: Some("remote verbose template {{ personality }}".repeat(100)),
        instructions_variables: Some(ModelInstructionsVariables {
            personality_default: Some(String::new()),
            personality_friendly: Some("remote friendly".to_string()),
            personality_pragmatic: Some("remote pragmatic".to_string()),
        }),
    });
    let config = ModelsManagerConfig {
        personality_enabled: true,
        ..Default::default()
    };

    let updated = with_config_overrides(model, &config);

    assert_eq!(updated.base_instructions, BASE_INSTRUCTIONS);
    assert_eq!(
        updated.get_model_instructions(/*personality*/ None),
        format!("{DEFAULT_PERSONALITY_HEADER}\n\n\n\n{BASE_INSTRUCTIONS}")
    );
    assert_eq!(
        updated.get_model_instructions(Some(codex_protocol::config_types::Personality::Pragmatic)),
        format!(
            "{DEFAULT_PERSONALITY_HEADER}\n\n{LOCAL_PRAGMATIC_TEMPLATE}\n\n{BASE_INSTRUCTIONS}"
        )
    );
}

#[test]
fn gpt_5_5_compact_prompt_does_not_override_user_base_instructions() {
    let model = model_info_from_slug("gpt-5.5");
    let config = ModelsManagerConfig {
        base_instructions: Some("user override".to_string()),
        ..Default::default()
    };

    let updated = with_config_overrides(model, &config);

    assert_eq!(updated.base_instructions, "user override");
    assert_eq!(updated.model_messages, None);
}
