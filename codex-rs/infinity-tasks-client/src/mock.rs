use crate::Agent;
use crate::AgentId;
use crate::AgentLogs;
use crate::AgentStatus;
use crate::InfinityBackend;
use crate::LaunchRequest;
use crate::LaunchResponse;
use crate::Result;
use chrono::Utc;

#[derive(Clone, Default)]
pub struct MockClient;

#[async_trait::async_trait]
impl InfinityBackend for MockClient {
    async fn launch(&self, req: LaunchRequest) -> Result<LaunchResponse> {
        let agent = Agent {
            id: AgentId(1001),
            name: req.name.unwrap_or_else(|| "mock-agent".to_string()),
            status: AgentStatus::Initializing,
            ip: Some("192.168.1.100".to_string()),
            server_type: req.server_type.unwrap_or_else(|| "cx22".to_string()),
            location: req.location.unwrap_or_else(|| "nbg1".to_string()),
            ssh_command: Some("ssh root@192.168.1.100".to_string()),
            created_at: Some(Utc::now()),
        };
        Ok(LaunchResponse {
            agent,
            root_password: Some("mock-password-123".to_string()),
        })
    }

    async fn list(&self) -> Result<Vec<Agent>> {
        Ok(vec![
            Agent {
                id: AgentId(1001),
                name: "codex-alpha".to_string(),
                status: AgentStatus::Running,
                ip: Some("192.168.1.100".to_string()),
                server_type: "cx22".to_string(),
                location: "nbg1".to_string(),
                ssh_command: Some("ssh root@192.168.1.100".to_string()),
                created_at: Some(Utc::now()),
            },
            Agent {
                id: AgentId(1002),
                name: "codex-beta".to_string(),
                status: AgentStatus::Stopped,
                ip: Some("192.168.1.101".to_string()),
                server_type: "cx32".to_string(),
                location: "fsn1".to_string(),
                ssh_command: Some("ssh root@192.168.1.101".to_string()),
                created_at: Some(Utc::now()),
            },
        ])
    }

    async fn get(&self, id: AgentId) -> Result<Agent> {
        Ok(Agent {
            id,
            name: "codex-alpha".to_string(),
            status: AgentStatus::Running,
            ip: Some("192.168.1.100".to_string()),
            server_type: "cx22".to_string(),
            location: "nbg1".to_string(),
            ssh_command: Some("ssh root@192.168.1.100".to_string()),
            created_at: Some(Utc::now()),
        })
    }

    async fn delete(&self, _id: AgentId) -> Result<()> {
        Ok(())
    }

    async fn logs(&self, _id: AgentId) -> Result<AgentLogs> {
        Ok(AgentLogs {
            stdout: "[mock] Agent started successfully\n[mock] Running codex...".to_string(),
            stderr: String::new(),
        })
    }
}
