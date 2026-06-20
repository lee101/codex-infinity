use super::*;
use crate::plugins::PluginInstallRequest;
use crate::plugins::test_support::load_plugins_config;
use crate::plugins::test_support::write_file;
use crate::plugins::test_support::write_openai_curated_marketplace;
use crate::plugins::test_support::write_plugins_feature_config;
use codex_core_plugins::startup_sync::curated_plugins_repo_path;
use codex_protocol::protocol::Product;
use codex_tools::DiscoverablePluginInfo;
use pretty_assertions::assert_eq;
use tempfile::tempdir;

#[tokio::test]
async fn list_tool_suggest_discoverable_plugins_returns_uninstalled_curated_plugins() {
    let codex_home = tempdir().expect("tempdir should succeed");
    let curated_root = curated_plugins_repo_path(codex_home.path());
    write_openai_curated_marketplace(&curated_root, &["sample", "slack"]);
    write_plugins_feature_config(codex_home.path());

    let config = load_plugins_config(codex_home.path()).await;
    let discoverable_plugins = list_tool_suggest_discoverable_plugins(&config)
        .await
}

    assert_eq!(
        discoverable_plugins,
        vec![DiscoverablePluginInfo {
            id: "slack@openai-curated".to_string(),
            name: "slack".to_string(),
            description: Some(
                "Plugin that includes skills, MCP servers, and app connectors".to_string(),
            ),
            has_skills: true,
            mcp_server_names: vec!["sample-docs".to_string()],
            app_connector_ids: vec!["connector_calendar".to_string()],
        }]
    );
    let remote_plugins = discoverable_plugins
        .into_iter()
        .filter(|plugin| plugin.id == "github@openai-curated-remote")
        .collect::<Vec<_>>();

    assert_eq!(
        remote_plugins,
        vec![DiscoverablePluginInfo {
            id: "github@openai-curated-remote".to_string(),
            remote_plugin_id: Some("plugins~Plugin_remote_github".to_string()),
            name: "Remote GitHub".to_string(),
            description: Some("Remote GitHub short".to_string()),
            has_skills: true,
            mcp_server_names: Vec::new(),
            app_connector_ids: vec!["github".to_string()],
        }]
    );

    write_file(
        &codex_home.path().join(crate::config::CONFIG_TOML_FILE),
        r#"[features]
plugins = true
remote_plugin = true

[tool_suggest]
disabled_tools = [
  { type = "plugin", id = "github@openai-curated-remote" }
]
"#,
    );
    let mut config_with_disabled_remote_plugin = load_plugins_config(codex_home.path()).await;
    config_with_disabled_remote_plugin.chatgpt_base_url = config.chatgpt_base_url.clone();
    let discoverable_plugins = list_discoverable_plugins_with_manager_and_auth(
        &config_with_disabled_remote_plugin,
        &plugins_manager,
        Some(&auth),
        &[],
    )
    .await
    .unwrap();
    assert!(
        discoverable_plugins
            .iter()
            .all(|plugin| plugin.id != "github@openai-curated-remote")
    );
}

#[tokio::test]
async fn list_tool_suggest_discoverable_plugins_returns_empty_when_plugins_feature_disabled() {
    let codex_home = tempdir().expect("tempdir should succeed");
    let curated_root = curated_plugins_repo_path(codex_home.path());
    write_openai_curated_marketplace(&curated_root, &["slack"]);
    write_file(
        &codex_home.path().join(crate::config::CONFIG_TOML_FILE),
        r#"[features]
plugins = false
"#,
    );

    let config = load_plugins_config(codex_home.path()).await;
    let discoverable_plugins = list_discoverable_plugins(&config, &[]).await.unwrap();

    assert_eq!(discoverable_plugins, Vec::<DiscoverablePluginInfo>::new());
}

#[tokio::test]
async fn list_tool_suggest_discoverable_plugins_omits_disabled_tool_suggestions() {
    let codex_home = tempdir().expect("tempdir should succeed");
    let curated_root = curated_plugins_repo_path(codex_home.path());
    write_openai_curated_marketplace(&curated_root, &["slack"]);
    write_file(
        &codex_home.path().join(crate::config::CONFIG_TOML_FILE),
        r#"[features]
plugins = true

[tool_suggest]
disabled_tools = [
  { type = "plugin", id = "slack@openai-curated" }
]
"#,
    );

    let config = load_plugins_config(codex_home.path()).await;
    let discoverable_plugins = list_discoverable_plugins(&config, &[]).await.unwrap();

    assert_eq!(discoverable_plugins, Vec::<DiscoverablePluginInfo>::new());
}

#[tokio::test]
async fn list_tool_suggest_discoverable_plugins_includes_configured_plugin_ids() {
    let codex_home = tempdir().expect("tempdir should succeed");
    let curated_root = curated_plugins_repo_path(codex_home.path());
    write_openai_curated_marketplace(&curated_root, &["sample"]);
    write_file(
        &codex_home.path().join(crate::config::CONFIG_TOML_FILE),
        r#"[features]
plugins = true

[tool_suggest]
discoverables = [{ type = "plugin", id = "sample@openai-curated" }]
"#,
    );

    let config = load_plugins_config(codex_home.path()).await;
    let discoverable_plugins = list_discoverable_plugins(&config, &[]).await.unwrap();

    assert_eq!(
        discoverable_plugins,
        vec![DiscoverablePluginInfo {
            id: "sample@openai-curated".to_string(),
            remote_plugin_id: None,
            name: "sample".to_string(),
            description: Some(
                "Plugin that includes skills, MCP servers, and app connectors".to_string(),
            ),
            has_skills: true,
            mcp_server_names: vec!["sample-docs".to_string()],
            app_connector_ids: vec!["connector_calendar".to_string()],
        }]
    );
}
