use super::*;

use crate::sandbox_tags::sandbox_tag;
use codex_protocol::models::PermissionProfile;
use codex_protocol::protocol::SandboxPolicy;
use codex_protocol::protocol::SessionSource;
use codex_protocol::protocol::SubAgentSource;
use core_test_support::PathBufExt;
use core_test_support::PathExt;
use serde_json::Value;
use std::collections::HashMap;
use std::time::Duration;
use tempfile::TempDir;
use tokio::process::Command;

#[tokio::test]
async fn build_turn_metadata_header_includes_has_changes_for_clean_repo() {
    let temp_dir = TempDir::new().expect("temp dir");
    let repo_path = temp_dir.path().join("repo").abs();
    std::fs::create_dir_all(&repo_path).expect("create repo");

    Command::new("git")
        .args(["init"])
        .current_dir(&repo_path)
        .output()
        .await
        .expect("git init");
    Command::new("git")
        .args(["config", "user.name", "Test User"])
        .current_dir(&repo_path)
        .output()
        .await
        .expect("git config user.name");
    Command::new("git")
        .args(["config", "user.email", "test@example.com"])
        .current_dir(&repo_path)
        .output()
        .await
        .expect("git config user.email");
    std::fs::write(repo_path.join("README.md"), "hello").expect("write file");
    Command::new("git")
        .args(["add", "."])
        .current_dir(&repo_path)
        .output()
        .await
        .expect("git add");
    Command::new("git")
        .args(["commit", "-m", "initial"])
        .current_dir(&repo_path)
        .output()
        .await
        .expect("git commit");

    let header = build_turn_metadata_header(&repo_path, Some("none"))
        .await
        .expect("header");
    let parsed: Value = serde_json::from_str(&header).expect("valid json");
    let workspace = parsed
        .get("workspaces")
        .and_then(Value::as_object)
        .and_then(|workspaces| workspaces.values().next())
        .cloned()
        .expect("workspace");
    assert_eq!(
        workspace.get("has_changes").and_then(Value::as_bool),
        Some(false)
    );
}

#[tokio::test]
async fn detached_memory_responses_metadata_omits_empty_workspace_metadata() {
    let temp_dir = TempDir::new().expect("temp dir");
    let cwd = temp_dir.path().abs();

    let header = detached_memory_responses_metadata(
        String::new(),
        String::new(),
        String::new(),
        String::new(),
        &SessionSource::Unknown,
        &cwd,
        /*sandbox*/ None,
    )
    .await
    .turn_metadata_json()
    .expect("detached memory should emit its request kind");
    let parsed: Value = serde_json::from_str(&header).expect("valid json");

    assert_eq!(parsed, serde_json::json!({"request_kind": "memory"}));
}

#[test]
fn turn_metadata_state_uses_platform_sandbox_tag() {
    let temp_dir = TempDir::new().expect("temp dir");
    let cwd = temp_dir.path().abs();
    let sandbox_policy = SandboxPolicy::new_read_only_policy();
    let permission_profile = PermissionProfile::read_only();

    let state = TurnMetadataState::new(
        "session-a".to_string(),
        &SessionSource::Exec,
        "turn-a".to_string(),
        cwd,
        &permission_profile,
        WindowsSandboxLevel::Disabled,
        /*enforce_managed_network*/ false,
    );

    let header = test_turn_metadata_header(&state);
    let json: Value = serde_json::from_str(&header).expect("json");
    let sandbox_name = json.get("sandbox").and_then(Value::as_str);
    let session_id = json.get("session_id").and_then(Value::as_str);
    let thread_source = json.get("thread_source").and_then(Value::as_str);

    let expected_sandbox = sandbox_tag(&sandbox_policy, WindowsSandboxLevel::Disabled);
    assert_eq!(sandbox_name, Some(expected_sandbox));
    assert_eq!(session_id, Some("session-a"));
    assert_eq!(thread_source, Some("user"));
    assert!(json.get("session_source").is_none());
}

#[test]
fn turn_metadata_state_classifies_subagent_thread_source() {
    let temp_dir = TempDir::new().expect("temp dir");
    let cwd = temp_dir.path().abs();
    let permission_profile = PermissionProfile::read_only();
    let session_source = SessionSource::SubAgent(SubAgentSource::Review);

    let state = TurnMetadataState::new(
        "session-a".to_string(),
        &session_source,
        "turn-a".to_string(),
        cwd,
        &permission_profile,
        WindowsSandboxLevel::Disabled,
        /*enforce_managed_network*/ false,
    );

    let header = test_turn_metadata_header(&state);
    let json: Value = serde_json::from_str(&header).expect("json");

    assert_eq!(
        json["forked_from_thread_id"].as_str(),
        Some("11111111-1111-4111-8111-111111111111")
    );
    assert!(json.get("parent_thread_id").is_none());
    assert!(json.get("subagent_kind").is_none());
}

#[test]
fn turn_metadata_state_includes_thread_spawn_subagent_parent_without_fork() {
    let temp_dir = TempDir::new().expect("temp dir");
    let cwd = temp_dir.path().abs();
    let permission_profile = PermissionProfile::read_only();
    let parent_thread_id =
        ThreadId::from_string("22222222-2222-4222-8222-222222222222").expect("thread id");

    let state = TurnMetadataState::new(
        "session-a".to_string(),
        "thread-a".to_string(),
        /*forked_from_thread_id*/ None,
        Some(parent_thread_id),
        &SessionSource::SubAgent(SubAgentSource::ThreadSpawn {
            parent_thread_id,
            depth: 1,
            agent_path: None,
            agent_nickname: None,
            agent_role: None,
        }),
        "turn-a".to_string(),
        cwd,
        &permission_profile,
        WindowsSandboxLevel::Disabled,
        /*enforce_managed_network*/ false,
    );

    let header = test_turn_metadata_header(&state);
    let json: Value = serde_json::from_str(&header).expect("json");

    assert!(json.get("forked_from_thread_id").is_none());
    assert_eq!(
        json["parent_thread_id"].as_str(),
        Some("22222222-2222-4222-8222-222222222222")
    );
    assert_eq!(json["subagent_kind"].as_str(), Some("thread_spawn"));
}

#[test]
fn turn_metadata_state_includes_forked_thread_spawn_subagent_lineage() {
    let temp_dir = TempDir::new().expect("temp dir");
    let cwd = temp_dir.path().abs();
    let permission_profile = PermissionProfile::read_only();
    let parent_thread_id =
        ThreadId::from_string("33333333-3333-4333-8333-333333333333").expect("thread id");

    let state = TurnMetadataState::new(
        "session-a".to_string(),
        "thread-a".to_string(),
        Some(parent_thread_id),
        Some(parent_thread_id),
        &SessionSource::SubAgent(SubAgentSource::ThreadSpawn {
            parent_thread_id,
            depth: 1,
            agent_path: None,
            agent_nickname: None,
            agent_role: None,
        }),
        "turn-a".to_string(),
        cwd,
        &permission_profile,
        WindowsSandboxLevel::Disabled,
        /*enforce_managed_network*/ false,
    );

    let header = test_turn_metadata_header(&state);
    let json: Value = serde_json::from_str(&header).expect("json");

    assert_eq!(
        json["forked_from_thread_id"].as_str(),
        Some("33333333-3333-4333-8333-333333333333")
    );
    assert_eq!(
        json["parent_thread_id"].as_str(),
        Some("33333333-3333-4333-8333-333333333333")
    );
    assert_eq!(json["subagent_kind"].as_str(), Some("thread_spawn"));
}

#[test]
fn turn_metadata_state_includes_known_parent_for_non_thread_spawn_subagents_without_fork() {
    let temp_dir = TempDir::new().expect("temp dir");
    let cwd = temp_dir.path().abs();
    let permission_profile = PermissionProfile::read_only();
    let parent_thread_id =
        ThreadId::from_string("44444444-4444-4444-8444-444444444444").expect("thread id");
    let sources = [
        (SubAgentSource::Review, "review"),
        (SubAgentSource::Other("guardian".to_string()), "guardian"),
        (
            SubAgentSource::Other("agent_job:job-1".to_string()),
            "agent_job:job-1",
        ),
    ];

    for (subagent_source, subagent_kind) in sources {
        let state = TurnMetadataState::new(
            "session-a".to_string(),
            "thread-a".to_string(),
            /*forked_from_thread_id*/ None,
            Some(parent_thread_id),
            &SessionSource::SubAgent(subagent_source),
            "turn-a".to_string(),
            cwd.clone(),
            &permission_profile,
            WindowsSandboxLevel::Disabled,
            /*enforce_managed_network*/ false,
        );

        let header = test_turn_metadata_header(&state);
        let json: Value = serde_json::from_str(&header).expect("json");

        assert!(json.get("forked_from_thread_id").is_none());
        assert_eq!(
            json["parent_thread_id"].as_str(),
            Some("44444444-4444-4444-8444-444444444444")
        );
        assert_eq!(json["subagent_kind"].as_str(), Some(subagent_kind));
    }
}

#[test]
fn turn_metadata_state_includes_turn_started_at_unix_ms_after_start() {
    let temp_dir = TempDir::new().expect("temp dir");
    let cwd = temp_dir.path().abs();
    let permission_profile = PermissionProfile::read_only();

    let state = TurnMetadataState::new(
        "session-a".to_string(),
        &SessionSource::Exec,
        "turn-a".to_string(),
        cwd,
        &permission_profile,
        WindowsSandboxLevel::Disabled,
        /*enforce_managed_network*/ false,
    );
    state.set_turn_started_at_unix_ms(/*turn_started_at_unix_ms*/ 1_700_000_000_123);

    let header = test_turn_metadata_header(&state);
    let json: Value = serde_json::from_str(&header).expect("json");

    assert_eq!(
        json["turn_started_at_unix_ms"].as_i64(),
        Some(1_700_000_000_123)
    );
}

#[test]
fn turn_metadata_state_ignores_client_turn_started_at_unix_ms_before_start() {
    let temp_dir = TempDir::new().expect("temp dir");
    let cwd = temp_dir.path().abs();
    let permission_profile = PermissionProfile::read_only();

    let state = TurnMetadataState::new(
        "session-a".to_string(),
        &SessionSource::Exec,
        "turn-a".to_string(),
        cwd,
        &permission_profile,
        WindowsSandboxLevel::Disabled,
        /*enforce_managed_network*/ false,
    );
    state.set_responsesapi_client_metadata(HashMap::from([
        (
            "turn_started_at_unix_ms".to_string(),
            "client-supplied".to_string(),
        ),
        (
            "forked_from_thread_id".to_string(),
            "client-supplied".to_string(),
        ),
        (
            "parent_thread_id".to_string(),
            "client-supplied".to_string(),
        ),
        ("subagent_kind".to_string(), "client-supplied".to_string()),
    ]));

    let header = test_turn_metadata_header(&state);
    let json: Value = serde_json::from_str(&header).expect("json");

    assert!(json.get("turn_started_at_unix_ms").is_none());
    assert!(json.get("forked_from_thread_id").is_none());
    assert!(json.get("parent_thread_id").is_none());
    assert!(json.get("subagent_kind").is_none());
}

#[test]
fn turn_metadata_state_merges_client_metadata_without_replacing_reserved_fields() {
    let temp_dir = TempDir::new().expect("temp dir");
    let cwd = temp_dir.path().abs();
    let permission_profile = PermissionProfile::read_only();
    let source_thread_id =
        ThreadId::from_string("44444444-4444-4444-8444-444444444444").expect("thread id");
    let parent_thread_id =
        ThreadId::from_string("55555555-5555-4555-8555-555555555555").expect("thread id");

    let state = TurnMetadataState::new(
        "session-a".to_string(),
        &SessionSource::Exec,
        "turn-a".to_string(),
        cwd,
        &permission_profile,
        WindowsSandboxLevel::Disabled,
        /*enforce_managed_network*/ false,
    );
    state.set_responsesapi_client_metadata(HashMap::from([
        ("fiber_run_id".to_string(), "fiber-123".to_string()),
        ("session_id".to_string(), "client-supplied".to_string()),
        ("thread_source".to_string(), "client-supplied".to_string()),
        ("request_kind".to_string(), "client-supplied".to_string()),
        (
            "turn_started_at_unix_ms".to_string(),
            "client-supplied".to_string(),
        ),
    ]));
    state.set_turn_started_at_unix_ms(/*turn_started_at_unix_ms*/ 1_700_000_000_123);

    let header = state.current_header_value().expect("header");
    let json: Value = serde_json::from_str(&header).expect("json");

    assert_eq!(json["fiber_run_id"].as_str(), Some("fiber-123"));
    assert_eq!(json["session_id"].as_str(), Some("session-a"));
    assert_eq!(json["thread_source"].as_str(), Some("user"));
    assert_eq!(json["turn_id"].as_str(), Some("turn-a"));
    assert!(json.get("request_kind").is_none());
    assert!(json.get(WINDOW_ID_KEY).is_none());
    assert_eq!(
        json["turn_started_at_unix_ms"].as_i64(),
        Some(1_700_000_000_123)
    );
}
