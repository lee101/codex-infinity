use super::*;

use codex_protocol::openai_models::ModelVisibility;
use pretty_assertions::assert_eq;

#[test]
fn gateway_models_are_picker_visible_and_resolve_to_provider_model_ids() {
    assert_eq!(
        gateway_models()
            .iter()
            .map(|model| {
                (
                    model.slug.as_str(),
                    model.display_name.as_str(),
                    model.description.as_deref().unwrap_or_default(),
                    model.priority,
                    model.context_window,
                    model.max_context_window,
                    model.visibility,
                    model_info::wire_model(&model.slug),
                )
            })
            .collect::<Vec<_>>(),
        vec![
            (
                "openrouter/mimo-v2.6-pro",
                "MiMo 2.6 Pro",
                "Xiaomi MiMo 2.6 Pro via OpenRouter.",
                44,
                Some(1_050_000),
                Some(1_050_000),
                ModelVisibility::List,
                "xiaomi/mimo-v2.6-pro",
            ),
            (
                "openrouter/space-bunny-alpha",
                "Space Bunny Alpha",
                "Stealth Space Bunny Alpha via OpenRouter.",
                45,
                Some(1_000_000),
                Some(1_000_000),
                ModelVisibility::List,
                "stealth/space-bunny-alpha",
            ),
            (
                "openpaths/openpaths-free",
                "OpenPaths Free",
                "OpenPaths free router via openpaths.io.",
                46,
                Some(1_000_000),
                Some(1_000_000),
                ModelVisibility::List,
                "openpaths-free",
            ),
        ]
    );
}

#[test]
fn gateway_models_carry_the_shared_base_instructions() {
    for model in gateway_models() {
        assert_eq!(
            model
                .model_messages
                .as_ref()
                .and_then(|messages| messages.instructions_template.as_deref()),
            Some(model_info::BASE_INSTRUCTIONS)
        );
    }
}

#[test]
fn merge_appends_missing_gateway_models_and_keeps_catalog_entries() {
    let mut catalog = gateway_models();
    catalog.truncate(/*len*/ 1);
    catalog[0].description = Some("Backend served this slug.".into());

    let merged = merge_gateway_models(catalog);

    assert_eq!(merged.len(), gateway_models().len());
    assert_eq!(
        merged[0].description.as_deref(),
        Some("Backend served this slug.")
    );
    assert_eq!(
        merged[1..],
        gateway_models()[1..],
        "gateway models missing from the catalog are appended in order"
    );
}

#[test]
fn merge_of_an_empty_catalog_yields_the_gateway_models() {
    assert_eq!(merge_gateway_models(Vec::new()), gateway_models());
}
