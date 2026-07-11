//! Shared retry and transport fallback decisions for Responses requests.

use std::time::Duration;

use crate::client::ModelClientSession;
use crate::session::session::Session;
use crate::session::turn_context::TurnContext;
use crate::util::backoff;
use chrono::DateTime;
use chrono::Utc;
use codex_async_utils::CancelErr;
use codex_async_utils::OrCancelExt;
use codex_protocol::error::CodexErr;
use codex_protocol::error::UsageLimitReachedError;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::RateLimitReachedType;
use codex_protocol::protocol::WarningEvent;
use tokio_util::sync::CancellationToken;
use tracing::warn;

/// Small cushion after the advertised reset time so retries do not race the window boundary.
const USAGE_LIMIT_RESET_BUFFER: Duration = Duration::from_secs(2);

#[derive(Debug, Clone, Copy)]
pub(crate) enum ResponsesStreamRequest {
    Sampling,
    RemoteCompactionV2,
}

/// Handles a retryable stream error and returns `Ok(())` when the caller should
/// retry the request loop.
pub(crate) async fn handle_retryable_response_stream_error(
    retries: &mut u64,
    max_retries: u64,
    err: CodexErr,
    client_session: &mut ModelClientSession,
    sess: &Session,
    turn_context: &TurnContext,
    request: ResponsesStreamRequest,
) -> Result<(), CodexErr> {
    if *retries >= max_retries
        && client_session.try_switch_fallback_transport(
            &turn_context.session_telemetry,
            &turn_context.model_info,
        )
    {
        sess.send_event(
            turn_context,
            EventMsg::Warning(WarningEvent {
                message: format!("Falling back from WebSockets to HTTPS transport. {err:#}"),
            }),
        )
        .await;
        *retries = 0;
        return Ok(());
    }

    if *retries < max_retries {
        *retries += 1;
        let retry_count = *retries;
        let delay = match &err {
            CodexErr::Stream(_, requested_delay) => {
                requested_delay.unwrap_or_else(|| backoff(retry_count))
            }
            _ => backoff(retry_count),
        };
        log_retry(request, turn_context, &err, retry_count, max_retries, delay);

        // In release builds, hide the first websocket retry notification to reduce noisy
        // transient reconnect messages. In debug builds, keep full visibility for diagnosis.
        let report_error = retry_count > 1
            || cfg!(debug_assertions)
            || !sess.services.model_client.responses_websocket_enabled();
        if report_error {
            // Surface retry information to any UI/front-end so the user understands what is
            // happening instead of staring at a seemingly frozen screen.
            sess.notify_stream_error(
                turn_context,
                format!("Reconnecting... {retry_count}/{max_retries}"),
                err,
            )
            .await;
        }
        tokio::time::sleep(delay).await;
        return Ok(());
    }

    Err(err)
}

/// Waits until a usage-limit window resets and returns `Ok(())` when the caller should retry the
/// sampling request. Returns the original error when auto-wait does not apply.
pub(crate) async fn wait_for_usage_limit_reset_if_applicable(
    sess: &Session,
    turn_context: &TurnContext,
    err: UsageLimitReachedError,
    cancellation_token: &CancellationToken,
) -> Result<(), CodexErr> {
    if !is_auto_waitable_usage_limit(&err) {
        return Err(CodexErr::UsageLimitReached(err));
    }

    let Some(resets_at) = err.resets_at else {
        return Err(CodexErr::UsageLimitReached(err));
    };

    let Some(delay) = delay_until_usage_limit_reset(resets_at) else {
        return Err(CodexErr::UsageLimitReached(err));
    };

    let codex_err = CodexErr::UsageLimitReached(err);
    warn!(
        turn_id = %turn_context.sub_id,
        ?delay,
        "usage limit reached; waiting for reset before retrying"
    );
    sess.notify_stream_error(
        turn_context,
        "Waiting for usage limit to reset...".to_string(),
        codex_err,
    )
    .await;

    match tokio::time::sleep(delay)
        .or_cancel(cancellation_token)
        .await
    {
        Ok(()) => Ok(()),
        Err(CancelErr::Cancelled) => Err(CodexErr::TurnAborted),
    }
}

fn is_auto_waitable_usage_limit(err: &UsageLimitReachedError) -> bool {
    match err.rate_limit_reached_type {
        Some(
            RateLimitReachedType::WorkspaceOwnerCreditsDepleted
            | RateLimitReachedType::WorkspaceMemberCreditsDepleted
            | RateLimitReachedType::WorkspaceOwnerUsageLimitReached
            | RateLimitReachedType::WorkspaceMemberUsageLimitReached,
        ) => false,
        Some(RateLimitReachedType::RateLimitReached) | None => err.resets_at.is_some(),
    }
}

fn delay_until_usage_limit_reset(resets_at: DateTime<Utc>) -> Option<Duration> {
    let target = resets_at + chrono::Duration::from_std(USAGE_LIMIT_RESET_BUFFER).ok()?;
    let now = Utc::now();
    if target <= now {
        return Some(Duration::from_secs(0));
    }
    (target - now).to_std().ok()
}

fn log_retry(
    request: ResponsesStreamRequest,
    turn_context: &TurnContext,
    err: &CodexErr,
    retries: u64,
    max_retries: u64,
    delay: Duration,
) {
    match request {
        ResponsesStreamRequest::Sampling => {
            warn!(
                "stream disconnected - retrying sampling request ({retries}/{max_retries} in {delay:?})...",
            );
        }
        ResponsesStreamRequest::RemoteCompactionV2 => {
            warn!(
                turn_id = %turn_context.sub_id,
                retries,
                max_retries,
                compact_error = %err,
                "remote compaction v2 stream failed; retrying request after delay"
            );
        }
    }
}

#[cfg(test)]
#[path = "responses_retry_tests.rs"]
mod tests;
