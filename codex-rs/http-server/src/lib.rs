//! HTTP Server for Codex - enables remote control of Codex agent
//!
//! This server exposes the Codex agent functionality over HTTP, allowing:
//! - Creating conversations
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
use codex_core::ConversationManager;
use codex_core::config::Config;
use codex_core::config::ConfigOverrides;
use codex_core::protocol::EventMsg;
use codex_core::protocol::Op;
use codex_protocol::ConversationId;
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
    conversation_manager: Arc<ConversationManager>,
    config: Arc<Config>,
    codex_linux_sandbox_exe: Option<PathBuf>,
    cli_config_overrides: CliConfigOverrides,
    /// Active event streams per conversation
    event_streams: Arc<RwLock<HashMap<ConversationId, Vec<tokio::sync::mpsc::Sender<String>>>>>,
}

/// Request to create a new conversation
#[derive(Debug, Deserialize)]
pub struct NewConversationRequest {
    pub cwd: Option<String>,
    #[allow(dead_code)]
    pub model: Option<String>,
    #[allow(dead_code)]
    pub approval_policy: Option<String>,
    #[allow(dead_code)]
    pub sandbox_mode: Option<String>,
}

/// Response after creating a conversation
#[derive(Debug, Serialize)]
pub struct NewConversationResponse {
    pub conversation_id: String,
    pub model: Option<String>,
}

/// Request to send a message to a conversation
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

/// Response after interrupting a conversation
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
    let config = Config::load_with_cli_overrides(cli_kv_overrides, overrides)
        .await
        .map_err(|e| IoError::new(ErrorKind::InvalidData, format!("error loading config: {e}")))?;

    let config = Arc::new(config);

    // Create auth manager
    let auth_manager = AuthManager::shared(
        config.codex_home.clone(),
        false,
        config.cli_auth_credentials_store_mode,
    );

    // Create conversation manager
    let conversation_manager = Arc::new(ConversationManager::new(
        auth_manager.clone(),
        SessionSource::Exec, // Use Exec source for remote/automated connections
    ));

    // Create app state
    let state = AppState {
        conversation_manager,
        config,
        codex_linux_sandbox_exe,
        cli_config_overrides,
        event_streams: Arc::new(RwLock::new(HashMap::new())),
    };

    // Build router
    let app = Router::new()
        .route("/health", get(health_handler))
        .route("/conversations", post(create_conversation))
        .route("/conversations/:id/messages", post(send_message))
        .route("/conversations/:id/interrupt", post(interrupt_conversation))
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

/// Create a new conversation
async fn create_conversation(
    State(state): State<AppState>,
    Json(req): Json<NewConversationRequest>,
) -> Result<Json<NewConversationResponse>, (StatusCode, Json<ErrorResponse>)> {
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
        Config::load_with_cli_overrides(cli_kv_overrides, overrides)
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

    match state.conversation_manager.new_conversation(config).await {
        Ok(new_conv) => {
            let conversation_id = new_conv.conversation_id;
            info!("Created conversation: {}", conversation_id);

            // Start event listener for this conversation
            let state_clone = state.clone();
            let conv_id_clone = conversation_id;
            tokio::spawn(async move {
                if let Ok(conversation) = state_clone
                    .conversation_manager
                    .get_conversation(conv_id_clone)
                    .await
                {
                    loop {
                        match conversation.next_event().await {
                            Ok(event) => {
                                let event_json = match serde_json::to_string(&event) {
                                    Ok(j) => j,
                                    Err(_) => continue,
                                };

                                // Broadcast to all listeners
                                let mut prune_after_send = false;
                                let senders = {
                                    let mut streams = state_clone.event_streams.write().await;
                                    match streams.get_mut(&conv_id_clone) {
                                        Some(senders) => {
                                            senders.retain(|sender| !sender.is_closed());
                                            if senders.is_empty() {
                                                streams.remove(&conv_id_clone);
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
                                    if let Some(senders) = streams.get_mut(&conv_id_clone) {
                                        senders.retain(|sender| !sender.is_closed());
                                        if senders.is_empty() {
                                            streams.remove(&conv_id_clone);
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
                    streams.remove(&conv_id_clone);
                }
            });

            Ok(Json(NewConversationResponse {
                conversation_id: conversation_id.to_string(),
                model: Some(new_conv.session_configured.model),
            }))
        }
        Err(e) => {
            error!("Failed to create conversation: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: format!("Failed to create conversation: {}", e),
                }),
            ))
        }
    }
}

/// Send a message to an existing conversation
async fn send_message(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(req): Json<SendMessageRequest>,
) -> Result<Json<SendMessageResponse>, (StatusCode, Json<ErrorResponse>)> {
    let conversation_id = ConversationId::from_string(&id).map_err(|e| {
        (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: format!("Invalid conversation ID: {}", e),
            }),
        )
    })?;

    let conversation = state
        .conversation_manager
        .get_conversation(conversation_id)
        .await
        .map_err(|_| {
            (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: "Conversation not found".to_string(),
                }),
            )
        })?;

    // Build input items
    let mut items = vec![CoreInputItem::Text {
        text: req.text.clone(),
    }];

    for image_url in req.images {
        items.push(CoreInputItem::Image { image_url });
    }

    // Submit user input
    conversation
        .submit(Op::UserInput { items })
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: format!("Failed to send message: {}", e),
                }),
            )
        })?;

    info!("Sent message to conversation {}: {}", id, req.text);

    Ok(Json(SendMessageResponse { queued: true }))
}

/// Interrupt an in-progress conversation
async fn interrupt_conversation(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<InterruptResponse>, (StatusCode, Json<ErrorResponse>)> {
    let conversation_id = ConversationId::from_string(&id).map_err(|e| {
        (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: format!("Invalid conversation ID: {}", e),
            }),
        )
    })?;

    let conversation = state
        .conversation_manager
        .get_conversation(conversation_id)
        .await
        .map_err(|_| {
            (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: "Conversation not found".to_string(),
                }),
            )
        })?;

    conversation.submit(Op::Interrupt).await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: format!("Failed to interrupt: {}", e),
            }),
        )
    })?;

    info!("Interrupted conversation {}", id);

    Ok(Json(InterruptResponse { interrupted: true }))
}

/// Stream events from a conversation via SSE
async fn stream_events(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<
    Sse<impl futures::Stream<Item = Result<Event, Infallible>>>,
    (StatusCode, Json<ErrorResponse>),
> {
    let conversation_id = ConversationId::from_string(&id).map_err(|e| {
        (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: format!("Invalid conversation ID: {}", e),
            }),
        )
    })?;

    // Verify conversation exists
    state
        .conversation_manager
        .get_conversation(conversation_id)
        .await
        .map_err(|_| {
            (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: "Conversation not found".to_string(),
                }),
            )
        })?;

    // Create channel for this subscriber
    let (tx, rx) = tokio::sync::mpsc::channel::<String>(100);

    // Register subscriber
    {
        let mut streams = state.event_streams.write().await;
        streams.entry(conversation_id).or_default().push(tx);
    }

    info!("Started event stream for conversation {}", id);

    // Convert to SSE stream
    let stream = ReceiverStream::new(rx).map(|msg| Ok(Event::default().data(msg)));

    Ok(Sse::new(stream))
}
