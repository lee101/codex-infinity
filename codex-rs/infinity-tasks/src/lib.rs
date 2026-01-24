mod cli;
pub use cli::Cli;

use anyhow::anyhow;
use codex_infinity_tasks_client::AgentId;
use codex_infinity_tasks_client::AgentStatus;
use codex_infinity_tasks_client::InfinityBackend;
use codex_infinity_tasks_client::LaunchRequest;
use owo_colors::OwoColorize;
use owo_colors::Stream;
use std::path::PathBuf;
use std::sync::Arc;

struct BackendContext {
    backend: Arc<dyn InfinityBackend>,
}

fn init_backend() -> anyhow::Result<BackendContext> {
    let use_mock = matches!(
        std::env::var("CODEX_INFINITY_MODE").ok().as_deref(),
        Some("mock") | Some("MOCK")
    );

    if use_mock {
        return Ok(BackendContext {
            backend: Arc::new(codex_infinity_tasks_client::MockClient),
        });
    }

    let http = codex_infinity_tasks_client::HttpClient::new()?;
    Ok(BackendContext {
        backend: Arc::new(http),
    })
}

pub async fn run_main(cli: Cli, _sandbox_exe: Option<PathBuf>) -> anyhow::Result<()> {
    match cli.command {
        None => {
            run_list_command(cli::ListCommand { json: false }).await
        }
        Some(cli::Command::Launch(cmd)) => run_launch_command(cmd).await,
        Some(cli::Command::List(cmd)) => run_list_command(cmd).await,
        Some(cli::Command::Status(cmd)) => run_status_command(cmd).await,
        Some(cli::Command::Logs(cmd)) => run_logs_command(cmd).await,
        Some(cli::Command::Attach(cmd)) => run_attach_command(cmd).await,
        Some(cli::Command::Cancel(cmd)) => run_cancel_command(cmd).await,
    }
}

async fn run_launch_command(cmd: cli::LaunchCommand) -> anyhow::Result<()> {
    let ctx = init_backend()?;

    let openai_key = std::env::var("CODEX_INFINITY_OPENAI_KEY")
        .or_else(|_| std::env::var("OPENAI_API_KEY"))
        .ok();

    let github_token = std::env::var("CODEX_INFINITY_GITHUB_TOKEN")
        .or_else(|_| std::env::var("GITHUB_TOKEN"))
        .ok();

    let req = LaunchRequest {
        name: cmd.name,
        repo_url: cmd.repo,
        server_type: Some(cmd.server_type),
        location: Some(cmd.location),
        with_gpu: cmd.with_gpu,
        setup_script: cmd.setup_script,
        github_token,
        openai_key,
        auto_next_steps: cmd.auto_next_steps,
        auto_next_idea: cmd.auto_next_idea,
        pack_size: cmd.pack_size,
    };

    let resp = ctx.backend.launch(req).await?;

    if cmd.json {
        println!("{}", serde_json::to_string_pretty(&resp)?);
    } else {
        println!(
            "Agent {} launched",
            resp.agent.id.to_string().if_supports_color(Stream::Stdout, |t| t.cyan())
        );
        println!("  Name:   {}", resp.agent.name);
        println!("  Status: {}", format_status(&resp.agent.status));
        if let Some(ip) = &resp.agent.ip {
            println!("  IP:     {ip}");
        }
        if let Some(ssh) = &resp.agent.ssh_command {
            println!("  SSH:    {ssh}");
        }
        if let Some(pw) = &resp.root_password {
            println!(
                "  Password: {}",
                pw.if_supports_color(Stream::Stdout, |t| t.yellow())
            );
        }
    }

    Ok(())
}

async fn run_list_command(cmd: cli::ListCommand) -> anyhow::Result<()> {
    let ctx = init_backend()?;
    let agents = ctx.backend.list().await?;

    if cmd.json {
        println!("{}", serde_json::to_string_pretty(&agents)?);
        return Ok(());
    }

    if agents.is_empty() {
        println!("No agents running");
        return Ok(());
    }

    println!(
        "{:<8} {:<20} {:<12} {:<16} {:<8} {:<8}",
        "ID", "NAME", "STATUS", "IP", "TYPE", "LOCATION"
    );
    println!("{}", "-".repeat(80));

    for agent in agents {
        println!(
            "{:<8} {:<20} {:<12} {:<16} {:<8} {:<8}",
            agent.id.0,
            truncate(&agent.name, 20),
            format_status(&agent.status),
            agent.ip.as_deref().unwrap_or("-"),
            agent.server_type,
            agent.location,
        );
    }

    Ok(())
}

async fn run_status_command(cmd: cli::StatusCommand) -> anyhow::Result<()> {
    let ctx = init_backend()?;
    let agent = ctx.backend.get(AgentId(cmd.agent_id)).await?;

    if cmd.json {
        println!("{}", serde_json::to_string_pretty(&agent)?);
        return Ok(());
    }

    println!("Agent {}", agent.id.0);
    println!("  Name:     {}", agent.name);
    println!("  Status:   {}", format_status(&agent.status));
    println!("  IP:       {}", agent.ip.as_deref().unwrap_or("-"));
    println!("  Type:     {}", agent.server_type);
    println!("  Location: {}", agent.location);
    if let Some(ssh) = &agent.ssh_command {
        println!("  SSH:      {ssh}");
    }

    Ok(())
}

async fn run_logs_command(cmd: cli::LogsCommand) -> anyhow::Result<()> {
    let ctx = init_backend()?;
    let logs = ctx.backend.logs(AgentId(cmd.agent_id)).await?;

    if !logs.stdout.is_empty() {
        print!("{}", logs.stdout);
    }
    if !logs.stderr.is_empty() {
        eprint!("{}", logs.stderr);
    }

    Ok(())
}

async fn run_attach_command(cmd: cli::AttachCommand) -> anyhow::Result<()> {
    let ctx = init_backend()?;
    let agent = ctx.backend.get(AgentId(cmd.agent_id)).await?;

    let ssh_cmd = agent
        .ssh_command
        .ok_or_else(|| anyhow!("Agent {} has no SSH command available", cmd.agent_id))?;

    let parts: Vec<&str> = ssh_cmd.split_whitespace().collect();
    if parts.is_empty() {
        return Err(anyhow!("Invalid SSH command"));
    }

    let mut child = tokio::process::Command::new(parts[0])
        .args(&parts[1..])
        .spawn()?;

    child.wait().await?;
    Ok(())
}

async fn run_cancel_command(cmd: cli::CancelCommand) -> anyhow::Result<()> {
    if !cmd.force {
        eprint!("Cancel agent {}? [y/N] ", cmd.agent_id);
        let mut input = String::new();
        std::io::stdin().read_line(&mut input)?;
        if !input.trim().eq_ignore_ascii_case("y") {
            println!("Cancelled");
            return Ok(());
        }
    }

    let ctx = init_backend()?;
    ctx.backend.delete(AgentId(cmd.agent_id)).await?;
    println!("Agent {} cancelled", cmd.agent_id);

    Ok(())
}

fn format_status(status: &AgentStatus) -> String {
    let s = status.to_string();
    match status {
        AgentStatus::Running => s.if_supports_color(Stream::Stdout, |t| t.green()).to_string(),
        AgentStatus::Initializing => s.if_supports_color(Stream::Stdout, |t| t.yellow()).to_string(),
        AgentStatus::Stopped => s.if_supports_color(Stream::Stdout, |t| t.dimmed()).to_string(),
        AgentStatus::Error => s.if_supports_color(Stream::Stdout, |t| t.red()).to_string(),
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("{}...", &s[..max - 3])
    }
}
