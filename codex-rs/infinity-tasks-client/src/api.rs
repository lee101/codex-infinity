use chrono::DateTime;
use chrono::Utc;
use serde::Deserialize;
use serde::Serialize;

pub type Result<T> = std::result::Result<T, InfinityError>;

#[derive(Debug, thiserror::Error)]
pub enum InfinityError {
    #[error("missing API key: set CODEX_INFINITY_API_KEY")]
    MissingApiKey,
    #[error("http error: {0}")]
    Http(String),
    #[error("io error: {0}")]
    Io(String),
    #[error("{0}")]
    Msg(String),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct AgentId(pub i64);

impl std::fmt::Display for AgentId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::str::FromStr for AgentId {
    type Err = std::num::ParseIntError;
    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        Ok(AgentId(s.parse()?))
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AgentStatus {
    Initializing,
    Running,
    Stopped,
    Error,
}

impl std::fmt::Display for AgentStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AgentStatus::Initializing => write!(f, "initializing"),
            AgentStatus::Running => write!(f, "running"),
            AgentStatus::Stopped => write!(f, "stopped"),
            AgentStatus::Error => write!(f, "error"),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Agent {
    pub id: AgentId,
    pub name: String,
    pub status: AgentStatus,
    pub ip: Option<String>,
    pub server_type: String,
    pub location: String,
    pub ssh_command: Option<String>,
    #[serde(default)]
    pub created_at: Option<DateTime<Utc>>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct LaunchRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub repo_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub server_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub location: Option<String>,
    #[serde(default)]
    pub with_gpu: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub setup_script: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub github_token: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub openai_key: Option<String>,
    #[serde(default)]
    pub auto_next_steps: bool,
    #[serde(default)]
    pub auto_next_idea: bool,
    #[serde(default = "default_pack_size")]
    pub pack_size: usize,
}

fn default_pack_size() -> usize {
    1
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LaunchResponse {
    pub agent: Agent,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub root_password: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct AgentLogs {
    pub stdout: String,
    pub stderr: String,
}

#[async_trait::async_trait]
pub trait InfinityBackend: Send + Sync {
    async fn launch(&self, req: LaunchRequest) -> Result<LaunchResponse>;
    async fn list(&self) -> Result<Vec<Agent>>;
    async fn get(&self, id: AgentId) -> Result<Agent>;
    async fn delete(&self, id: AgentId) -> Result<()>;
    async fn logs(&self, id: AgentId) -> Result<AgentLogs>;
}
