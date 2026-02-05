use anyhow::Context;
use clap::Parser;
use serde::Deserialize;
use serde::Serialize;
use std::env;
use std::process::Command;

#[derive(Debug, Parser)]
pub struct InfinityCli {
    /// Base URL for the Codex Infinity API (e.g. https://codex-infinity.com).
    #[arg(long = "base-url")]
    base_url: Option<String>,

    /// API key for Codex Infinity (from the account page).
    #[arg(long = "api-key")]
    api_key: Option<String>,

    #[command(subcommand)]
    cmd: InfinityCommand,
}

#[derive(Debug, Parser)]
enum InfinityCommand {
    /// List available Codex Infinity machines.
    List {
        /// List agent machines instead of cloud servers.
        #[arg(long = "agents", default_value_t = false)]
        agents: bool,
    },

    /// Attach to a Codex Infinity machine via SSH.
    Attach {
        /// Server or agent id (see `list`).
        id: i64,

        /// Attach to an agent machine instead of a cloud server.
        #[arg(long = "agents", default_value_t = false)]
        agents: bool,

        /// SSH user.
        #[arg(long = "user", default_value = "root")]
        user: String,

        /// Print the SSH command instead of running it.
        #[arg(long = "dry-run", default_value_t = false)]
        dry_run: bool,
    },

    /// Manage add-on backups (list) and restore workflows.
    Addons(AddonsCli),
}

#[derive(Debug, Parser)]
#[command(
    after_help = "Examples:\n  codex infinity addons backups owner/repo --type postgres\n  codex infinity addons backups owner/repo --type postgres --json\n  codex infinity addons restore owner/repo --type postgres --object-key addon-backups/addon-id/2026/02/05/backup.dump --yes\n  codex infinity addons restore owner/repo --type postgres --object-key addon-backups/addon-id/2026/02/05/backup.dump --yes --json\n"
)]
struct AddonsCli {
    #[command(subcommand)]
    cmd: AddonsCommand,
}

#[derive(Debug, Parser)]
enum AddonsCommand {
    /// List recent backups for an add-on.
    Backups(AddonBackupsCommand),
    /// Restore an add-on from a backup object key or URL.
    Restore(AddonRestoreCommand),
}

#[derive(Debug, Parser)]
struct AddonBackupsCommand {
    /// Repository in owner/repo format.
    #[arg(value_name = "OWNER/REPO")]
    repo: String,

    /// Add-on type (postgres or mongo).
    #[arg(long = "type", value_name = "TYPE", value_parser = ["postgres", "mongo", "mongodb"])]
    addon_type: String,

    /// Maximum number of backups to return (1-500).
    #[arg(long = "limit", default_value_t = 50, value_parser = parse_backup_limit, value_name = "N")]
    limit: usize,

    /// Emit JSON instead of a table.
    #[arg(long = "json", default_value_t = false)]
    json: bool,
}

#[derive(Debug, Parser)]
struct AddonRestoreCommand {
    /// Repository in owner/repo format.
    #[arg(value_name = "OWNER/REPO")]
    repo: String,

    /// Add-on type (postgres or mongo).
    #[arg(long = "type", value_name = "TYPE", value_parser = ["postgres", "mongo", "mongodb"])]
    addon_type: String,

    /// Backup object key to restore.
    #[arg(
        long = "object-key",
        value_name = "KEY",
        required_unless_present = "url",
        conflicts_with = "url"
    )]
    object_key: Option<String>,

    /// Backup URL to restore (R2 public URL).
    #[arg(
        long = "url",
        value_name = "URL",
        required_unless_present = "object_key",
        conflicts_with = "object_key"
    )]
    url: Option<String>,

    /// Confirm destructive restore.
    #[arg(long = "yes", default_value_t = false)]
    yes: bool,

    /// Emit JSON instead of a confirmation message.
    #[arg(long = "json", default_value_t = false)]
    json: bool,
}

#[derive(Debug, Deserialize)]
struct CloudServer {
    id: i64,
    name: String,
    status: String,
    ipv4: Option<String>,
    server_type: Option<String>,
    datacenter: Option<String>,
}

#[derive(Debug, Deserialize)]
struct AgentServer {
    id: i64,
    name: String,
    status: String,
    ip: Option<String>,
    server_type: String,
    location: String,
    ssh_command: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
struct AddonSummary {
    id: String,
    #[serde(rename = "type")]
    addon_type: String,
    status: String,
    plan: Option<String>,
    region: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
struct AddonListResponse {
    addons: Vec<AddonSummary>,
    total: usize,
}

#[derive(Debug, Deserialize, Serialize)]
struct AddonBackupInfo {
    object_key: String,
    url: String,
    size_bytes: i64,
    last_modified: String,
}

#[derive(Debug, Deserialize, Serialize)]
struct AddonBackupsResponse {
    backups: Vec<AddonBackupInfo>,
    total: usize,
    retention_days: i64,
    backup_enabled: bool,
    billing_enabled: bool,
}

#[derive(Debug, Serialize)]
struct AddonRestoreRequest {
    object_key: Option<String>,
    url: Option<String>,
    confirm: bool,
}

#[derive(Debug, Deserialize, Serialize)]
struct AddonRestoreResponse {
    object_key: String,
    restored_at: String,
}

impl InfinityCli {
    pub async fn run(self) -> anyhow::Result<()> {
        let base_url = normalize_base_url(
            self.base_url
                .or_else(|| env::var("CODEX_INFINITY_BASE_URL").ok())
                .unwrap_or_else(default_base_url),
        );
        let api_key = self
            .api_key
            .or_else(|| env::var("CODEX_INFINITY_API_KEY").ok())
            .context("Missing API key: set CODEX_INFINITY_API_KEY or pass --api-key")?;
        let client = InfinityClient::new(base_url, api_key)?;

        match self.cmd {
            InfinityCommand::List { agents } => {
                if agents {
                    list_agents(&client).await?;
                } else {
                    list_servers(&client).await?;
                }
            }
            InfinityCommand::Attach {
                id,
                agents,
                user,
                dry_run,
            } => {
                if agents {
                    attach_agent(&client, id, &user, dry_run).await?;
                } else {
                    attach_server(&client, id, &user, dry_run).await?;
                }
            }
            InfinityCommand::Addons(addons_cli) => {
                run_addons_command(&client, addons_cli).await?;
            }
        }

        Ok(())
    }
}

fn default_base_url() -> String {
    "https://codex-infinity.com".to_string()
}

fn normalize_base_url(mut base_url: String) -> String {
    while base_url.ends_with('/') {
        base_url.pop();
    }
    base_url
}

fn parse_backup_limit(input: &str) -> Result<usize, String> {
    let value: usize = input
        .parse()
        .map_err(|_| "limit must be an integer between 1 and 500".to_string())?;
    if (1..=500).contains(&value) {
        Ok(value)
    } else {
        Err("limit must be between 1 and 500".to_string())
    }
}

fn normalize_addon_type(value: &str) -> String {
    let normalized = value.trim().to_lowercase();
    if normalized == "mongodb" {
        "mongo".to_string()
    } else {
        normalized
    }
}

struct InfinityClient {
    base_url: String,
    api_key: String,
    http: reqwest::Client,
}

const MAX_RETRIES: u32 = 3;
const INITIAL_RETRY_DELAY_MS: u64 = 1000;
const REQUEST_TIMEOUT_SECS: u64 = 30;

impl InfinityClient {
    fn new(base_url: String, api_key: String) -> anyhow::Result<Self> {
        let http = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(REQUEST_TIMEOUT_SECS))
            .connect_timeout(std::time::Duration::from_secs(REQUEST_TIMEOUT_SECS))
            .build()?;
        Ok(Self {
            base_url,
            api_key,
            http,
        })
    }

    async fn get_json<T: for<'de> Deserialize<'de>>(&self, path: &str) -> anyhow::Result<T> {
        let trimmed = path.trim_start_matches('/');
        let base_url = &self.base_url;
        let url = format!("{base_url}/{trimmed}");

        let mut last_error = None;
        for attempt in 0..MAX_RETRIES {
            if attempt > 0 {
                let delay_ms = INITIAL_RETRY_DELAY_MS * (1 << (attempt - 1)); // Exponential backoff
                eprintln!(
                    "Retry {}/{} for {} (waiting {}ms)...",
                    attempt,
                    MAX_RETRIES - 1,
                    url,
                    delay_ms
                );
                tokio::time::sleep(std::time::Duration::from_millis(delay_ms)).await;
            }

            match self.try_get_json::<T>(&url).await {
                Ok(result) => return Ok(result),
                Err(e) => {
                    // Only retry on timeout/connection errors
                    let is_retryable = e.to_string().contains("timeout")
                        || e.to_string().contains("connect")
                        || e.to_string().contains("connection");
                    if !is_retryable {
                        return Err(e);
                    }
                    last_error = Some(e);
                }
            }
        }

        Err(last_error
            .unwrap_or_else(|| anyhow::anyhow!("Request failed after {MAX_RETRIES} retries")))
    }

    async fn try_get_json<T: for<'de> Deserialize<'de>>(&self, url: &str) -> anyhow::Result<T> {
        let res = self
            .http
            .get(url)
            .header("X-API-Key", &self.api_key)
            .send()
            .await
            .with_context(|| format!("request failed: GET {url}"))?;
        let status = res.status();
        let body = res.text().await.unwrap_or_default();
        if !status.is_success() {
            anyhow::bail!("GET {url} failed: {status}; body={body}");
        }
        let parsed = serde_json::from_str::<T>(&body)
            .with_context(|| format!("Failed to decode response from {url}"))?;
        Ok(parsed)
    }

    async fn post_json<T: for<'de> Deserialize<'de>, B: Serialize>(
        &self,
        path: &str,
        body: &B,
    ) -> anyhow::Result<T> {
        let trimmed = path.trim_start_matches('/');
        let base_url = &self.base_url;
        let url = format!("{base_url}/{trimmed}");

        let mut last_error = None;
        for attempt in 0..MAX_RETRIES {
            if attempt > 0 {
                let delay_ms = INITIAL_RETRY_DELAY_MS * (1 << (attempt - 1));
                eprintln!(
                    "Retry {}/{} for {} (waiting {}ms)...",
                    attempt,
                    MAX_RETRIES - 1,
                    url,
                    delay_ms
                );
                tokio::time::sleep(std::time::Duration::from_millis(delay_ms)).await;
            }

            match self.try_post_json::<T, B>(&url, body).await {
                Ok(result) => return Ok(result),
                Err(e) => {
                    let is_retryable = e.to_string().contains("timeout")
                        || e.to_string().contains("connect")
                        || e.to_string().contains("connection");
                    if !is_retryable {
                        return Err(e);
                    }
                    last_error = Some(e);
                }
            }
        }

        Err(last_error
            .unwrap_or_else(|| anyhow::anyhow!("Request failed after {MAX_RETRIES} retries")))
    }

    async fn try_post_json<T: for<'de> Deserialize<'de>, B: Serialize>(
        &self,
        url: &str,
        body: &B,
    ) -> anyhow::Result<T> {
        let res = self
            .http
            .post(url)
            .header("X-API-Key", &self.api_key)
            .json(body)
            .send()
            .await
            .with_context(|| format!("request failed: POST {url}"))?;
        let status = res.status();
        let body = res.text().await.unwrap_or_default();
        if !status.is_success() {
            anyhow::bail!("POST {url} failed: {status}; body={body}");
        }
        let parsed = serde_json::from_str::<T>(&body)
            .with_context(|| format!("Failed to decode response from {url}"))?;
        Ok(parsed)
    }
}

async fn list_servers(client: &InfinityClient) -> anyhow::Result<()> {
    let servers: Vec<CloudServer> = client.get_json("/api/cloud/servers").await?;
    println!("id\tname\tstatus\ttype\tlocation\tip");
    for server in servers {
        let CloudServer {
            id,
            name,
            status,
            ipv4,
            server_type,
            datacenter,
        } = server;
        let ip = ipv4.as_deref().unwrap_or("-");
        let server_type = server_type.as_deref().unwrap_or("-");
        let location = datacenter.as_deref().unwrap_or("-");
        println!("{id}\t{name}\t{status}\t{server_type}\t{location}\t{ip}");
    }
    Ok(())
}

async fn list_agents(client: &InfinityClient) -> anyhow::Result<()> {
    let agents: Vec<AgentServer> = client.get_json("/api/agents").await?;
    println!("id\tname\tstatus\ttype\tlocation\tip");
    for agent in agents {
        let AgentServer {
            id,
            name,
            status,
            ip,
            server_type,
            location,
            ssh_command: _,
        } = agent;
        let ip = ip.as_deref().unwrap_or("-");
        println!("{id}\t{name}\t{status}\t{server_type}\t{location}\t{ip}");
    }
    Ok(())
}

async fn run_addons_command(client: &InfinityClient, addons_cli: AddonsCli) -> anyhow::Result<()> {
    match addons_cli.cmd {
        AddonsCommand::Backups(cmd) => run_addon_backups(client, cmd).await,
        AddonsCommand::Restore(cmd) => run_addon_restore(client, cmd).await,
    }
}

async fn run_addon_backups(
    client: &InfinityClient,
    cmd: AddonBackupsCommand,
) -> anyhow::Result<()> {
    let AddonBackupsCommand {
        repo,
        addon_type,
        limit,
        json,
    } = cmd;
    let addon = find_addon_by_type(client, &repo, &addon_type).await?;
    let id = addon.id;
    let path = format!("/api/projects/{repo}/addons/{id}/backups?limit={limit}");
    let response: AddonBackupsResponse = client.get_json(&path).await?;

    if json {
        println!("{}", serde_json::to_string_pretty(&response)?);
        return Ok(());
    }

    println!(
        "Retention: {} days | Backups enabled: {} | Billing enabled: {}",
        response.retention_days, response.backup_enabled, response.billing_enabled
    );
    if response.backups.is_empty() {
        println!("No backups found");
        return Ok(());
    }

    println!("last_modified\tsize\tobject_key");
    for backup in response.backups {
        let last_modified = if backup.last_modified.is_empty() {
            "-"
        } else {
            backup.last_modified.as_str()
        };
        println!(
            "{last_modified}\t{}\t{}",
            format_bytes(backup.size_bytes),
            backup.object_key
        );
    }

    Ok(())
}

async fn run_addon_restore(
    client: &InfinityClient,
    cmd: AddonRestoreCommand,
) -> anyhow::Result<()> {
    let AddonRestoreCommand {
        repo,
        addon_type,
        object_key,
        url,
        yes,
        json,
    } = cmd;
    if !yes {
        anyhow::bail!("Restore is destructive. Re-run with --yes to confirm.");
    }
    let addon = find_addon_by_type(client, &repo, &addon_type).await?;
    let request = AddonRestoreRequest {
        object_key,
        url,
        confirm: true,
    };
    let id = addon.id;
    let path = format!("/api/projects/{repo}/addons/{id}/restore");
    let response: AddonRestoreResponse = client.post_json(&path, &request).await?;

    if json {
        println!("{}", serde_json::to_string_pretty(&response)?);
        return Ok(());
    }

    println!("Restore completed");
    println!("  Object key: {}", response.object_key);
    println!("  Restored at: {}", response.restored_at);
    Ok(())
}

async fn find_addon_by_type(
    client: &InfinityClient,
    repo: &str,
    addon_type: &str,
) -> anyhow::Result<AddonSummary> {
    let normalized = normalize_addon_type(addon_type);
    let path = format!("/api/projects/{repo}/addons");
    let response: AddonListResponse = client.get_json(&path).await?;
    let mut matches = response
        .addons
        .into_iter()
        .filter(|addon| normalize_addon_type(&addon.addon_type) == normalized)
        .collect::<Vec<_>>();

    if matches.is_empty() {
        anyhow::bail!("No {normalized} add-on found for {repo}");
    }
    if matches.len() > 1 {
        let ids = matches
            .iter()
            .map(|addon| addon.id.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        anyhow::bail!("Multiple {normalized} add-ons found for {repo}: {ids}");
    }
    Ok(matches.remove(0))
}

async fn attach_server(
    client: &InfinityClient,
    id: i64,
    user: &str,
    dry_run: bool,
) -> anyhow::Result<()> {
    let server: CloudServer = client.get_json(&format!("/api/cloud/servers/{id}")).await?;
    let ip = server
        .ipv4
        .as_deref()
        .context("Server has no IPv4 address")?;
    run_ssh(user, ip, dry_run)
}

async fn attach_agent(
    client: &InfinityClient,
    id: i64,
    user: &str,
    dry_run: bool,
) -> anyhow::Result<()> {
    let agent: AgentServer = client.get_json(&format!("/api/agents/{id}")).await?;
    let ip = agent.ip.as_deref().context("Agent has no IPv4 address")?;
    run_ssh(user, ip, dry_run)
}

fn format_bytes(bytes: i64) -> String {
    if bytes < 0 {
        return bytes.to_string();
    }
    let mut size = bytes as f64;
    let mut unit = "B";
    for next_unit in ["KB", "MB", "GB", "TB"] {
        if size < 1024.0 {
            break;
        }
        size /= 1024.0;
        unit = next_unit;
    }
    if unit == "B" {
        format!("{bytes} B")
    } else {
        format!("{size:.1} {unit}")
    }
}

fn run_ssh(user: &str, ip: &str, dry_run: bool) -> anyhow::Result<()> {
    let target = format!("{user}@{ip}");
    if dry_run {
        println!("ssh {target}");
        return Ok(());
    }

    let status = Command::new("ssh")
        .arg(&target)
        .status()
        .with_context(|| format!("Failed to launch ssh to {target}"))?;
    if !status.success() {
        anyhow::bail!("ssh exited with status {status}");
    }
    Ok(())
}
