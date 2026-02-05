use assert_cmd::Command;
use pretty_assertions::assert_eq;
use serde_json::json;
use tokio::runtime::Runtime;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::ResponseTemplate;
use wiremock::matchers::method;
use wiremock::matchers::path;
use wiremock::matchers::query_param;

fn run_with_runtime<F>(task: F) -> Result<(), Box<dyn std::error::Error>>
where
    F: std::future::Future<Output = Result<(), Box<dyn std::error::Error>>>,
{
    let runtime = Runtime::new()?;
    runtime.block_on(task)
}

#[test]
fn addons_backups_selects_addon_type_and_outputs_json() -> Result<(), Box<dyn std::error::Error>> {
    run_with_runtime(async {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/api/projects/owner/repo/addons"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "addons": [
                    {
                        "id": "addon-mongo",
                        "type": "mongo",
                        "status": "active",
                        "plan": "starter",
                        "region": "nbg1"
                    },
                    {
                        "id": "addon-pg",
                        "type": "postgres",
                        "status": "active",
                        "plan": "pro",
                        "region": "hel1"
                    }
                ],
                "total": 2
            })))
            .mount(&server)
            .await;

        Mock::given(method("GET"))
            .and(path("/api/projects/owner/repo/addons/addon-pg/backups"))
            .and(query_param("limit", "50"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "backups": [
                    {
                        "object_key": "addon-backups/addon-pg/2026/02/05/backup.dump",
                        "url": "https://cdn.example.com/addon-backups/addon-pg/2026/02/05/backup.dump",
                        "size_bytes": 2048,
                        "last_modified": "2026-02-05T00:00:00Z"
                    }
                ],
                "total": 1,
                "retention_days": 7,
                "backup_enabled": true,
                "billing_enabled": true
            })))
            .mount(&server)
            .await;

        let output = Command::new(codex_utils_cargo_bin::cargo_bin("codex")?)
            .env("CODEX_INFINITY_BASE_URL", server.uri())
            .env("CODEX_INFINITY_API_KEY", "test-key")
            .args([
                "infinity",
                "addons",
                "backups",
                "owner/repo",
                "--type",
                "postgres",
                "--json",
            ])
            .output()?;

        assert!(output.status.success());
        let parsed: serde_json::Value = serde_json::from_slice(&output.stdout)?;
        assert_eq!(
            parsed["backups"][0]["object_key"],
            json!("addon-backups/addon-pg/2026/02/05/backup.dump")
        );

        let requests = server.received_requests().await.unwrap_or_default();
        let mut has_list = false;
        let mut has_backups = false;
        for request in requests {
            if request.url.path() == "/api/projects/owner/repo/addons" {
                has_list = true;
            }
            if request.url.path() == "/api/projects/owner/repo/addons/addon-pg/backups" {
                has_backups = true;
            }
        }
        assert!(has_list);
        assert!(has_backups);

        Ok(())
    })
}

#[test]
fn addons_backups_outputs_table() -> Result<(), Box<dyn std::error::Error>> {
    run_with_runtime(async {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/api/projects/owner/repo/addons"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "addons": [
                    {
                        "id": "addon-mongo",
                        "type": "mongo",
                        "status": "active",
                        "plan": "starter",
                        "region": "nbg1"
                    }
                ],
                "total": 1
            })))
            .mount(&server)
            .await;

        Mock::given(method("GET"))
            .and(path("/api/projects/owner/repo/addons/addon-mongo/backups"))
            .and(query_param("limit", "50"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "backups": [
                    {
                        "object_key": "addon-backups/addon-mongo/2026/02/05/backup.archive",
                        "url": "https://cdn.example.com/addon-backups/addon-mongo/2026/02/05/backup.archive",
                        "size_bytes": 2048,
                        "last_modified": "2026-02-05T00:00:00Z"
                    }
                ],
                "total": 1,
                "retention_days": 14,
                "backup_enabled": true,
                "billing_enabled": false
            })))
            .mount(&server)
            .await;

        let output = Command::new(codex_utils_cargo_bin::cargo_bin("codex")?)
            .env("CODEX_INFINITY_BASE_URL", server.uri())
            .env("CODEX_INFINITY_API_KEY", "test-key")
            .args([
                "infinity",
                "addons",
                "backups",
                "owner/repo",
                "--type",
                "mongodb",
            ])
            .output()?;

        assert!(output.status.success());
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(
            stdout.contains("Retention: 14 days | Backups enabled: true | Billing enabled: false")
        );
        assert!(stdout.contains("last_modified\tsize\tobject_key"));
        assert!(stdout.contains(
            "2026-02-05T00:00:00Z\t2.0 KB\taddon-backups/addon-mongo/2026/02/05/backup.archive"
        ));

        Ok(())
    })
}

#[test]
fn addons_restore_requires_yes() -> Result<(), Box<dyn std::error::Error>> {
    let output = Command::new(codex_utils_cargo_bin::cargo_bin("codex")?)
        .env("CODEX_INFINITY_BASE_URL", "http://127.0.0.1:9")
        .env("CODEX_INFINITY_API_KEY", "test-key")
        .args([
            "infinity",
            "addons",
            "restore",
            "owner/repo",
            "--type",
            "postgres",
            "--object-key",
            "addon-backups/addon-pg/2026/02/05/backup.dump",
        ])
        .output()?;

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Re-run with --yes"));

    Ok(())
}

#[test]
fn addons_events_outputs_json_with_filters() -> Result<(), Box<dyn std::error::Error>> {
    run_with_runtime(async {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/api/projects/owner/repo/addons"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "addons": [
                    {
                        "id": "addon-pg",
                        "type": "postgres",
                        "status": "active",
                        "plan": "starter",
                        "region": "nbg1"
                    }
                ],
                "total": 1
            })))
            .mount(&server)
            .await;

        Mock::given(method("GET"))
            .and(path("/api/projects/owner/repo/addons/addon-pg/events"))
            .and(query_param("limit", "50"))
            .and(query_param("event_type", "restore"))
            .and(query_param("cursor", "cursor-token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "events": [
                    {
                        "id": "evt-1",
                        "addon_id": "addon-pg",
                        "event_type": "restore",
                        "payload": {
                            "object_key": "addon-backups/addon-pg/backup.dump"
                        },
                        "created_at": "2026-02-05T00:00:00Z"
                    }
                ],
                "total": 1,
                "next_cursor": "next-token"
            })))
            .mount(&server)
            .await;

        let output = Command::new(codex_utils_cargo_bin::cargo_bin("codex")?)
            .env("CODEX_INFINITY_BASE_URL", server.uri())
            .env("CODEX_INFINITY_API_KEY", "test-key")
            .args([
                "infinity",
                "addons",
                "events",
                "owner/repo",
                "--type",
                "postgres",
                "--event-type",
                "restore",
                "--cursor",
                "cursor-token",
                "--json",
            ])
            .output()?;

        assert!(output.status.success());
        let parsed: serde_json::Value = serde_json::from_slice(&output.stdout)?;
        assert_eq!(parsed["next_cursor"], json!("next-token"));
        assert_eq!(parsed["events"][0]["event_type"], json!("restore"));

        Ok(())
    })
}

#[test]
fn addons_events_outputs_table() -> Result<(), Box<dyn std::error::Error>> {
    run_with_runtime(async {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/api/projects/owner/repo/addons"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "addons": [
                    {
                        "id": "addon-pg",
                        "type": "postgres",
                        "status": "active",
                        "plan": "starter",
                        "region": "nbg1"
                    }
                ],
                "total": 1
            })))
            .mount(&server)
            .await;

        Mock::given(method("GET"))
            .and(path("/api/projects/owner/repo/addons/addon-pg/events"))
            .and(query_param("limit", "50"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "events": [
                    {
                        "id": "evt-1",
                        "addon_id": "addon-pg",
                        "event_type": "backup",
                        "payload": {
                            "size_bytes": 1024
                        },
                        "created_at": "2026-02-05T00:00:00Z"
                    }
                ],
                "total": 1,
                "next_cursor": "next-token"
            })))
            .mount(&server)
            .await;

        let output = Command::new(codex_utils_cargo_bin::cargo_bin("codex")?)
            .env("CODEX_INFINITY_BASE_URL", server.uri())
            .env("CODEX_INFINITY_API_KEY", "test-key")
            .args([
                "infinity",
                "addons",
                "events",
                "owner/repo",
                "--type",
                "postgres",
            ])
            .output()?;

        assert!(output.status.success());
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("created_at\tevent_type\tpayload"));
        assert!(stdout.contains("2026-02-05T00:00:00Z\tbackup"));
        assert!(stdout.contains("next_cursor\tnext-token"));

        Ok(())
    })
}
