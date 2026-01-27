//! HTTP Server for Codex - enables remote control of Codex agent
//!
//! This server exposes the Codex agent functionality over HTTP, allowing:
//! - Creating threads (conversations)
//! - Sending messages (follow-ups)
//! - Interrupting in-progress work
//! - Streaming events via SSE

use axum::Router;
use axum::extract::Path;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::Json;
use axum::response::sse::Event;
use axum::response::sse::Sse;
use axum::routing::get;
use axum::routing::post;
use codex_common::CliConfigOverrides;
use codex_core::AuthManager;
use codex_core::ThreadManager;
use codex_core::config::Config;
use codex_core::config::ConfigOverrides;
use codex_core::protocol::EventMsg;
use codex_core::protocol::Op;
use codex_protocol::ThreadId;
use codex_protocol::protocol::SessionSource;
use codex_protocol::user_input::UserInput as CoreInputItem;
use serde::Deserialize;
use serde::Serialize;
use std::collections::HashMap;
use std::convert::Infallible;
use std::io::Error as IoError;
use std::io::ErrorKind;
use std::io::Result as IoResult;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio_stream::StreamExt;
use tokio_stream::wrappers::ReceiverStream;
use tower_http::cors::Any;
use tower_http::cors::CorsLayer;
use tracing::error;
use tracing::info;

/// Shared application state
#[derive(Clone)]
pub struct AppState {
    thread_manager: Arc<ThreadManager>,
    config: Arc<Config>,
    codex_linux_sandbox_exe: Option<PathBuf>,
    cli_config_overrides: CliConfigOverrides,
    /// Active event streams per thread
    event_streams: Arc<RwLock<HashMap<ThreadId, Vec<tokio::sync::mpsc::Sender<String>>>>>,
}

/// Request to create a new thread
#[derive(Debug, Deserialize)]
pub struct NewThreadRequest {
    pub cwd: Option<String>,
    #[allow(dead_code)]
    pub model: Option<String>,
    #[allow(dead_code)]
    pub approval_policy: Option<String>,
    #[allow(dead_code)]
    pub sandbox_mode: Option<String>,
}

/// Response after creating a thread
#[derive(Debug, Serialize)]
pub struct NewThreadResponse {
    pub thread_id: String,
    pub model: Option<String>,
}

/// Request to send a message to a thread
#[derive(Debug, Deserialize)]
pub struct SendMessageRequest {
    pub text: String,
    #[serde(default)]
    pub images: Vec<String>,
}

/// Response after sending a message
#[derive(Debug, Serialize)]
pub struct SendMessageResponse {
    pub queued: bool,
}

/// Response after interrupting a thread
#[derive(Debug, Serialize)]
pub struct InterruptResponse {
    pub interrupted: bool,
}

/// Error response
#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    pub error: String,
}

/// Health check response
#[derive(Debug, Serialize)]
pub struct HealthResponse {
    pub status: String,
    pub version: String,
}

pub async fn run_main(
    port: u16,
    codex_linux_sandbox_exe: Option<PathBuf>,
    cli_config_overrides: CliConfigOverrides,
) -> IoResult<()> {
    // Initialize tracing
    let _ = tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive(tracing::Level::INFO.into()),
        )
        .try_init();

    info!("Starting Codex HTTP server on port {}", port);

    // Parse CLI overrides
    let cli_kv_overrides = cli_config_overrides.parse_overrides().map_err(|e| {
        IoError::new(
            ErrorKind::InvalidInput,
            format!("error parsing -c overrides: {e}"),
        )
    })?;

    // Load config
    let overrides = ConfigOverrides {
        codex_linux_sandbox_exe: codex_linux_sandbox_exe.clone(),
        ..ConfigOverrides::default()
    };
    let config = Config::load_with_cli_overrides_and_harness_overrides(cli_kv_overrides, overrides)
        .await
        .map_err(|e| IoError::new(ErrorKind::InvalidData, format!("error loading config: {e}")))?;

    let config = Arc::new(config);

    // Create auth manager
    let auth_manager = AuthManager::shared(
        config.codex_home.clone(),
        false,
        config.cli_auth_credentials_store_mode,
    );

    // Create thread manager
    let thread_manager = Arc::new(ThreadManager::new(
        config.codex_home.clone(),
        auth_manager.clone(),
        SessionSource::Exec, // Use Exec source for remote/automated connections
    ));

    // Create app state
    let state = AppState {
        thread_manager,
        config,
        codex_linux_sandbox_exe,
        cli_config_overrides,
        event_streams: Arc::new(RwLock::new(HashMap::new())),
    };

    // Build router
    let app = Router::new()
        .route("/health", get(health_handler))
        .route("/threads", post(create_thread))
        .route("/threads/:id/messages", post(send_message))
        .route("/threads/:id/interrupt", post(interrupt_thread))
        .route("/threads/:id/events", get(stream_events))
        // Legacy conversation routes for backwards compatibility
        .route("/conversations", post(create_thread))
        .route("/conversations/:id/messages", post(send_message))
        .route("/conversations/:id/interrupt", post(interrupt_thread))
        .route("/conversations/:id/events", get(stream_events))
        .layer(
            CorsLayer::new()
                .allow_origin(Any)
                .allow_methods(Any)
                .allow_headers(Any),
        )
        .with_state(state);

    // Start server
    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{}", port))
        .await
        .map_err(|e| IoError::new(ErrorKind::AddrInUse, format!("failed to bind: {e}")))?;

    info!("Codex HTTP server listening on http://0.0.0.0:{}", port);

    axum::serve(listener, app)
        .await
        .map_err(|e| IoError::new(ErrorKind::Other, format!("server error: {e}")))
}

/// Health check endpoint
async fn health_handler() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
    })
}

/// Create a new thread
async fn create_thread(
    State(state): State<AppState>,
    Json(req): Json<NewThreadRequest>,
) -> Result<Json<NewThreadResponse>, (StatusCode, Json<ErrorResponse>)> {
    let config = if let Some(cwd) = req.cwd {
        let cli_kv_overrides = state.cli_config_overrides.parse_overrides().map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: format!("Failed to parse config overrides: {e}"),
                }),
            )
        })?;
        let overrides = ConfigOverrides {
            cwd: Some(PathBuf::from(cwd)),
            codex_linux_sandbox_exe: state.codex_linux_sandbox_exe.clone(),
            ..ConfigOverrides::default()
        };
        Config::load_with_cli_overrides_and_harness_overrides(cli_kv_overrides, overrides)
            .await
            .map_err(|e| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ErrorResponse {
                        error: format!("Failed to load config: {e}"),
                    }),
                )
            })?
    } else {
        (*state.config).clone()
    };

    match state.thread_manager.start_thread(config).await {
        Ok(new_thread) => {
            let thread_id = new_thread.thread_id;
            info!("Created thread: {}", thread_id);

            // Start event listener for this thread
            let state_clone = state.clone();
            let thread_id_clone = thread_id;
            let thread = new_thread.thread.clone();
            tokio::spawn(async move {
                loop {
                    match thread.next_event().await {
                        Ok(event) => {
                            let event_json = match serde_json::to_string(&event) {
                                Ok(j) => j,
                                Err(_) => continue,
                            };

                            // Broadcast to all listeners
                            let mut prune_after_send = false;
                            let senders = {
                                let mut streams = state_clone.event_streams.write().await;
                                match streams.get_mut(&thread_id_clone) {
                                    Some(senders) => {
                                        senders.retain(|sender| !sender.is_closed());
                                        if senders.is_empty() {
                                            streams.remove(&thread_id_clone);
                                            Vec::new()
                                        } else {
                                            senders.clone()
                                        }
                                    }
                                    None => Vec::new(),
                                }
                            };
                            for sender in senders {
                                if sender.send(event_json.clone()).await.is_err() {
                                    prune_after_send = true;
                                }
                            }
                            if prune_after_send {
                                let mut streams = state_clone.event_streams.write().await;
                                if let Some(senders) = streams.get_mut(&thread_id_clone) {
                                    senders.retain(|sender| !sender.is_closed());
                                    if senders.is_empty() {
                                        streams.remove(&thread_id_clone);
                                    }
                                }
                            }

                            // Check for shutdown
                            if matches!(event.msg, EventMsg::ShutdownComplete) {
                                break;
                            }
                        }
                        Err(_) => break,
                    }
                }
                let mut streams = state_clone.event_streams.write().await;
                streams.remove(&thread_id_clone);
            });

            Ok(Json(NewThreadResponse {
                thread_id: thread_id.to_string(),
                model: Some(new_thread.session_configured.model),
            }))
        }
        Err(e) => {
            error!("Failed to create thread: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: format!("Failed to create thread: {}", e),
                }),
            ))
        }
    }
}

/// Send a message to an existing thread
async fn send_message(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(req): Json<SendMessageRequest>,
) -> Result<Json<SendMessageResponse>, (StatusCode, Json<ErrorResponse>)> {
    let thread_id = ThreadId::from_string(&id).map_err(|e| {
        (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: format!("Invalid thread ID: {}", e),
            }),
        )
    })?;

    let thread = state
        .thread_manager
        .get_thread(thread_id)
        .await
        .map_err(|_| {
            (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: "Thread not found".to_string(),
                }),
            )
        })?;

    // Build input items
    let mut items = vec![CoreInputItem::Text {
        text: req.text.clone(),
        text_elements: vec![],
    }];

    for image_url in req.images {
        items.push(CoreInputItem::Image { image_url });
    }

    // Submit user input
    thread
        .submit(Op::UserInput {
            items,
            final_output_json_schema: None,
        })
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: format!("Failed to send message: {}", e),
                }),
            )
        })?;

    info!("Sent message to thread {}: {}", id, req.text);

    Ok(Json(SendMessageResponse { queued: true }))
}

/// Interrupt an in-progress thread
async fn interrupt_thread(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<InterruptResponse>, (StatusCode, Json<ErrorResponse>)> {
    let thread_id = ThreadId::from_string(&id).map_err(|e| {
        (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: format!("Invalid thread ID: {}", e),
            }),
        )
    })?;

    let thread = state
        .thread_manager
        .get_thread(thread_id)
        .await
        .map_err(|_| {
            (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: "Thread not found".to_string(),
                }),
            )
        })?;

    thread.submit(Op::Interrupt).await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: format!("Failed to interrupt: {}", e),
            }),
        )
    })?;

    info!("Interrupted thread {}", id);

    Ok(Json(InterruptResponse { interrupted: true }))
}

/// Stream events from a thread via SSE
async fn stream_events(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<
    Sse<impl futures::Stream<Item = Result<Event, Infallible>>>,
    (StatusCode, Json<ErrorResponse>),
> {
    let thread_id = ThreadId::from_string(&id).map_err(|e| {
        (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: format!("Invalid thread ID: {}", e),
            }),
        )
    })?;

    // Verify thread exists
    state
        .thread_manager
        .get_thread(thread_id)
        .await
        .map_err(|_| {
            (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: "Thread not found".to_string(),
                }),
            )
        })?;

    // Create channel for this subscriber
    let (tx, rx) = tokio::sync::mpsc::channel::<String>(100);

    // Register subscriber
    {
        let mut streams = state.event_streams.write().await;
        streams.entry(thread_id).or_default().push(tx);
    }

    info!("Started event stream for thread {}", id);

    // Convert to SSE stream
    let stream = ReceiverStream::new(rx).map(|msg| Ok(Event::default().data(msg)));

    Ok(Sse::new(stream))
}
