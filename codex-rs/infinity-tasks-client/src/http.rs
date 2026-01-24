use crate::Agent;
use crate::AgentId;
use crate::AgentLogs;
use crate::InfinityBackend;
use crate::InfinityError;
use crate::LaunchRequest;
use crate::LaunchResponse;
use crate::Result;

#[derive(Clone)]
pub struct HttpClient {
    base_url: String,
    api_key: String,
    client: reqwest::Client,
}

impl HttpClient {
    pub fn new() -> Result<Self> {
        let api_key =
            std::env::var("CODEX_INFINITY_API_KEY").map_err(|_| InfinityError::MissingApiKey)?;
        let base_url = std::env::var("CODEX_INFINITY_BASE_URL")
            .unwrap_or_else(|_| "https://netwrck.com".to_string());
        let client = reqwest::Client::new();
        Ok(Self {
            base_url,
            api_key,
            client,
        })
    }

    pub fn with_base_url(mut self, url: impl Into<String>) -> Self {
        self.base_url = url.into();
        self
    }

    pub fn with_api_key(mut self, key: impl Into<String>) -> Self {
        self.api_key = key.into();
        self
    }
}

#[async_trait::async_trait]
impl InfinityBackend for HttpClient {
    async fn launch(&self, req: LaunchRequest) -> Result<LaunchResponse> {
        let resp = self
            .client
            .post(format!("{}/api/agents", self.base_url))
            .bearer_auth(&self.api_key)
            .json(&req)
            .send()
            .await
            .map_err(|e| InfinityError::Http(e.to_string()))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(InfinityError::Http(format!(
                "launch failed: {} - {}",
                status, body
            )));
        }

        resp.json()
            .await
            .map_err(|e| InfinityError::Http(format!("parse error: {e}")))
    }

    async fn list(&self) -> Result<Vec<Agent>> {
        let resp = self
            .client
            .get(format!("{}/api/agents", self.base_url))
            .bearer_auth(&self.api_key)
            .send()
            .await
            .map_err(|e| InfinityError::Http(e.to_string()))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(InfinityError::Http(format!(
                "list failed: {} - {}",
                status, body
            )));
        }

        resp.json()
            .await
            .map_err(|e| InfinityError::Http(format!("parse error: {e}")))
    }

    async fn get(&self, id: AgentId) -> Result<Agent> {
        let resp = self
            .client
            .get(format!("{}/api/agents/{}", self.base_url, id.0))
            .bearer_auth(&self.api_key)
            .send()
            .await
            .map_err(|e| InfinityError::Http(e.to_string()))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(InfinityError::Http(format!(
                "get failed: {} - {}",
                status, body
            )));
        }

        resp.json()
            .await
            .map_err(|e| InfinityError::Http(format!("parse error: {e}")))
    }

    async fn delete(&self, id: AgentId) -> Result<()> {
        let resp = self
            .client
            .delete(format!("{}/api/agents/{}", self.base_url, id.0))
            .bearer_auth(&self.api_key)
            .send()
            .await
            .map_err(|e| InfinityError::Http(e.to_string()))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(InfinityError::Http(format!(
                "delete failed: {} - {}",
                status, body
            )));
        }

        Ok(())
    }

    async fn logs(&self, id: AgentId) -> Result<AgentLogs> {
        let resp = self
            .client
            .get(format!("{}/api/agents/{}/logs", self.base_url, id.0))
            .bearer_auth(&self.api_key)
            .send()
            .await
            .map_err(|e| InfinityError::Http(e.to_string()))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(InfinityError::Http(format!(
                "logs failed: {} - {}",
                status, body
            )));
        }

        resp.json()
            .await
            .map_err(|e| InfinityError::Http(format!("parse error: {e}")))
    }
}
