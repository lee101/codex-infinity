use anyhow::Context;
use clap::Parser;
use serde::Deserialize;
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
                    attempt, MAX_RETRIES - 1, url, delay_ms
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

        Err(last_error.unwrap_or_else(|| anyhow::anyhow!("Request failed after {MAX_RETRIES} retries")))
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
