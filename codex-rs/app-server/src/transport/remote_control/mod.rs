mod auth;
mod client_tracker;
mod clients;
mod desired_state;
mod enroll;
mod protocol;
mod websocket;

use self::auth::load_remote_control_auth;
use self::auth::recover_remote_control_auth;
use self::desired_state::RemoteControlDesiredState;
use self::desired_state::acquire_persistence_lock;
use self::enroll::RemoteControlEnrollment;
use self::enroll::enroll_remote_control_server;
use self::enroll::load_persisted_remote_control_enrollment;
use self::enroll::refresh_remote_control_server;
use self::enroll::update_persisted_remote_control_enrollment;
use crate::transport::remote_control::websocket::RemoteControlChannels;
use crate::transport::remote_control::websocket::RemoteControlStatusPublisher;
use crate::transport::remote_control::websocket::RemoteControlWebsocket;

pub use self::protocol::ClientId;
use self::protocol::RemoteControlPairingStatusCode;
use self::protocol::ServerEvent;
use self::protocol::StreamId;
use self::protocol::normalize_remote_control_url;
use super::CHANNEL_CAPACITY;
use super::TransportEvent;
use super::next_connection_id;
use codex_app_server_protocol::RemoteControlClientsListParams;
use codex_app_server_protocol::RemoteControlClientsListResponse;
use codex_app_server_protocol::RemoteControlClientsRevokeParams;
use codex_app_server_protocol::RemoteControlClientsRevokeResponse;
use codex_app_server_protocol::RemoteControlConnectionStatus;
use codex_app_server_protocol::RemoteControlPairingStartParams;
use codex_app_server_protocol::RemoteControlPairingStartResponse;
use codex_app_server_protocol::RemoteControlPairingStatusParams;
use codex_app_server_protocol::RemoteControlPairingStatusResponse;
use codex_app_server_protocol::RemoteControlStatusChangedNotification;
use codex_login::AuthManager;
use codex_state::StateRuntime;
use std::io;
use std::ops::Deref;
use std::ops::DerefMut;
use std::panic::AssertUnwindSafe;
use std::sync::Arc;
use std::sync::Mutex as StdMutex;
use tokio::sync::Semaphore;
use tokio::sync::SemaphorePermit;
use tokio::sync::mpsc;
use tokio::sync::oneshot;
use tokio::sync::watch;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use tracing::error;
use tracing::info;
use tracing::warn;

pub(super) struct QueuedServerEnvelope {
    pub(super) event: ServerEvent,
    pub(super) client_id: ClientId,
    pub(super) stream_id: StreamId,
    pub(super) write_complete_tx: Option<oneshot::Sender<()>>,
}

#[derive(Clone)]
pub(crate) struct RemoteControlHandle {
    enabled_tx: Arc<watch::Sender<bool>>,
    status_tx: Arc<watch::Sender<RemoteControlStatusChangedNotification>>,
    state_db: Option<Arc<StateRuntime>>,
    remote_control_url: String,
    current_enrollment: CurrentRemoteControlEnrollment,
    pairing_persistence_key: RemoteControlPairingPersistenceKey,
    pairing_persistence_key_required: bool,
    auth_manager: Arc<AuthManager>,
}

// Pairing and websocket connect share one selected server so they cannot enroll or replace
// different persisted rows while either path is awaiting backend I/O.
type CurrentRemoteControlEnrollment = Arc<RemoteControlEnrollmentState>;
type RemoteControlPairingPersistenceKey = watch::Sender<Option<String>>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RemoteControlEnrollmentSelection {
    ReuseOrCreate,
    ReplaceExisting,
}

struct RemoteControlEnrollmentState {
    enrollment: StdMutex<Option<RemoteControlEnrollment>>,
    lock: Semaphore,
}

impl RemoteControlEnrollmentState {
    fn new(enrollment: Option<RemoteControlEnrollment>) -> Self {
        Self {
            enrollment: StdMutex::new(enrollment),
            lock: Semaphore::new(1),
        }
    }

    async fn lock(&self) -> RemoteControlEnrollmentLease<'_> {
        let permit = match self.lock.acquire().await {
            Ok(permit) => permit,
            Err(_) => unreachable!("remote control enrollment lock should stay open"),
        };
        RemoteControlEnrollmentLease {
            state: self,
            enrollment: self.snapshot(),
            _permit: permit,
        }
    }

    fn snapshot(&self) -> Option<RemoteControlEnrollment> {
        self.enrollment
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }
}

struct RemoteControlEnrollmentLease<'a> {
    state: &'a RemoteControlEnrollmentState,
    enrollment: Option<RemoteControlEnrollment>,
    _permit: SemaphorePermit<'a>,
}

impl Deref for RemoteControlEnrollmentLease<'_> {
    type Target = Option<RemoteControlEnrollment>;

    fn deref(&self) -> &Self::Target {
        &self.enrollment
    }
}

impl DerefMut for RemoteControlEnrollmentLease<'_> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.enrollment
    }
}

impl Drop for RemoteControlEnrollmentLease<'_> {
    fn drop(&mut self) {
        *self
            .state
            .enrollment
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = self.enrollment.take();
    }
}

impl RemoteControlHandle {
    pub(crate) fn set_enabled(&self, enabled: bool) {
        let requested_enabled = enabled;
        let enabled = enabled && self.state_db_available;
        if requested_enabled && !self.state_db_available {
            warn!("remote control cannot be enabled because sqlite state db is unavailable");
        }
        self.enabled_tx.send_if_modified(|state| {
            let changed = *state != enabled;
            *state = enabled;
            changed
        });
    }

    pub(crate) fn status_receiver(
        &self,
    ) -> watch::Receiver<RemoteControlStatusChangedNotification> {
        self.status_tx.subscribe()
    }
}

pub(crate) async fn start_remote_control(
    remote_control_url: String,
    state_db: Option<Arc<StateRuntime>>,
    auth_manager: Arc<AuthManager>,
    transport_event_tx: mpsc::Sender<TransportEvent>,
    shutdown_token: CancellationToken,
    app_server_client_name_rx: Option<oneshot::Receiver<String>>,
    startup_mode: RemoteControlStartupMode,
) -> io::Result<(JoinHandle<()>, RemoteControlHandle)> {
    let policy = config.policy;
    let state_db_available = state_db.is_some();
    let requested_initial_enabled = startup_mode == RemoteControlStartupMode::EnabledEphemeral;
    let desired_state =
        if policy == RemoteControlPolicy::DisabledByRequirements || !state_db_available {
            RemoteControlDesiredState::Disabled
        } else {
            match startup_mode {
                RemoteControlStartupMode::ResolvePersisted => RemoteControlDesiredState::Unknown,
                RemoteControlStartupMode::DisabledEphemeral => RemoteControlDesiredState::Disabled,
                RemoteControlStartupMode::EnabledEphemeral => RemoteControlDesiredState::Enabled {
                    persistence_preference: None,
                },
            }
        };
    let initial_enabled = desired_state.is_enabled();
    if requested_initial_enabled && !state_db_available {
        warn!("remote control disabled because sqlite state db is unavailable");
    }
    let remote_control_target = if initial_enabled {
        Some(normalize_remote_control_url(&remote_control_url)?)
    } else {
        None
    };

    let (enabled_tx, enabled_rx) = watch::channel(initial_enabled);
    let initial_status = RemoteControlStatusChangedNotification {
        status: if initial_enabled {
            RemoteControlConnectionStatus::Connecting
        } else {
            RemoteControlConnectionStatus::Disabled
        },
        environment_id: None,
    };
    let (status_tx, _status_rx) = watch::channel(initial_status);
    let status_publisher = RemoteControlStatusPublisher::new(status_tx.clone());
    info!(
        remote_control_url = %remote_control_url,
        installation_id = %installation_id,
        server_name = %server_name,
        state_db_available,
        ?desired_state,
        "starting app-server remote control websocket task"
    );
    let remote_control_url_for_log = remote_control_url.clone();
    let handle_remote_control_url = remote_control_url.clone();
    let installation_id_for_log = installation_id.clone();
    let server_name_for_log = server_name.clone();
    let shutdown_token_for_log = shutdown_token.clone();
    let join_handle = tokio::spawn(async move {
        RemoteControlWebsocket::new(
            remote_control_url,
            remote_control_target,
            state_db,
            auth_manager,
            RemoteControlChannels {
                transport_event_tx,
                status_publisher,
                current_enrollment: websocket_current_enrollment,
                pairing_persistence_key: websocket_pairing_persistence_key,
                desired_state_persistence_lock: websocket_desired_state_persistence_lock,
            },
            shutdown_token,
            websocket_desired_state_tx,
        )
        .run(app_server_client_name_rx);
        match AssertUnwindSafe(websocket_task).catch_unwind().await {
            Ok(()) => {
                let shutdown_requested = shutdown_token_for_log.is_cancelled();
                if shutdown_requested {
                    info!(
                        remote_control_url = %remote_control_url_for_log,
                        installation_id = %installation_id_for_log,
                        server_name = %server_name_for_log,
                        shutdown_requested,
                        "app-server remote control websocket task exited"
                    );
                } else {
                    warn!(
                        remote_control_url = %remote_control_url_for_log,
                        installation_id = %installation_id_for_log,
                        server_name = %server_name_for_log,
                        shutdown_requested,
                        "app-server remote control websocket task exited without shutdown"
                    );
                }
            }
            Err(panic) => {
                error!(
                    remote_control_url = %remote_control_url_for_log,
                    installation_id = %installation_id_for_log,
                    server_name = %server_name_for_log,
                    "app-server remote control websocket task panicked"
                );
                std::panic::resume_unwind(panic);
            }
        }
    });

    Ok((
        join_handle,
        RemoteControlHandle {
            policy,
            desired_state_tx,
            desired_state_rpc_lock,
            desired_state_persistence_lock,
            status_tx: Arc::new(status_tx),
            state_db: handle_state_db,
            remote_control_url: handle_remote_control_url,
            current_enrollment,
            pairing_persistence_key,
            pairing_persistence_key_required,
            auth_manager: handle_auth_manager,
        },
    ))
}

#[cfg(test)]
mod tests;
