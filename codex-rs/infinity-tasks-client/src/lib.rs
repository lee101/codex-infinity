mod api;

pub use api::Agent;
pub use api::AgentId;
pub use api::AgentLogs;
pub use api::AgentStatus;
pub use api::InfinityBackend;
pub use api::InfinityError;
pub use api::LaunchRequest;
pub use api::LaunchResponse;
pub use api::Result;

#[cfg(feature = "mock")]
mod mock;

#[cfg(feature = "online")]
mod http;

#[cfg(feature = "mock")]
pub use mock::MockClient;

#[cfg(feature = "online")]
pub use http::HttpClient;
