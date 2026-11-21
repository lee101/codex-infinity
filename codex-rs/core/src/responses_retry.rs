//! Shared retry and transport fallback decisions for Responses requests.
//! Content-filter guidance is recorded for sampling requests before retry decisions.
//! Server advice controls timing without extending configured retry limits. Model capacity
//! pressure is the exception: callers that can wait out an outage call
//! [`wait_for_server_overload_retry`] directly instead of spending the retry budget.

use std::time::Duration;

use crate::client::ModelClientSession;
use crate::context::ContentFilterGuidance;
use crate::context::ContextualUserFragment;
use crate::session::session::Session;
use crate::session::step_context::StepContext;
use crate::session::turn_context::TurnContext;
use chrono::DateTime;
use chrono::Utc;
use codex_async_utils::CancelErr;
use codex_async_utils::OrCancelExt;
use codex_client::RetryOperation;
use codex_features::Feature;
use codex_http_client::RetryAfter;
use codex_protocol::error::CodexErr;
use codex_protocol::error::CodexErrorDetails;
use codex_protocol::error::UsageLimitReachedError;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::RateLimitReachedType;
use codex_protocol::protocol::WarningEvent;
use tokio::time::Instant;
use tokio_util::sync::CancellationToken;
use tracing::warn;

const INITIAL_CONNECTION_RETRY_DELAY: Duration = Duration::from_secs(5);
const MAX_CONNECTION_RETRY_DELAY: Duration = Duration::from_secs(240);
const INITIAL_SERVER_OVERLOAD_RETRY_DELAY: Duration = Duration::from_secs(5);
const MAX_SERVER_OVERLOAD_RETRY_DELAY: Duration = Duration::from_secs(240);

/// Small cushion after the advertised reset time so retries do not race the window boundary.
const USAGE_LIMIT_RESET_BUFFER: Duration = Duration::from_secs(2);

#[derive(Debug, Clone, Copy)]
pub(crate) enum ResponsesStreamRequest {
    Sampling,
    RemoteCompactionV2,
}

/// A new generation cannot repair a terminal provider response or settle the
/// previous attempt's bill. Preserve these outcomes across transport retries,
/// same-turn steering, and compaction's fallback to another model.
pub(crate) fn is_terminal_sampling_outcome(error: &CodexErr) -> bool {
    matches!(
        error.details(),
        CodexErrorDetails::ResponseIncomplete(_)
            | CodexErrorDetails::UsageAccountingFailed(_)
            | CodexErrorDetails::SessionBudgetExceeded
            | CodexErrorDetails::APISpendLimitExceeded(_)
            | CodexErrorDetails::InferenceHookDenied { .. }
    )
}

pub(crate) struct ResponsesStreamRetryState {
    retries: u64,
    connection_retries: u64,
    connection_retry_delay: Duration,
    server_overload_attempts: u64,
    server_overload_retry_delay: Duration,
}

impl Default for ResponsesStreamRetryState {
    fn default() -> Self {
        Self {
            retries: 0,
            connection_retries: 0,
            connection_retry_delay: INITIAL_CONNECTION_RETRY_DELAY,
            server_overload_attempts: 0,
            server_overload_retry_delay: INITIAL_SERVER_OVERLOAD_RETRY_DELAY,
        }
    }
}

/// Server retry advice retained after stream retries are exhausted. The turn ID
/// prevents a reused Guardian session from applying advice from an earlier review.
pub(crate) struct ExhaustedResponseRetry {
    pub(crate) turn_id: String,
    pub(crate) retry_at: Option<tokio::time::Instant>,
}

/// Returns `Ok(())` when the caller should retry the request loop, or the original error when
/// it is terminal or the retry budget is exhausted.
#[tracing::instrument(level = "trace", skip_all, fields(request = ?request))]
pub(crate) async fn handle_response_stream_error(
    retry_state: &mut ResponsesStreamRetryState,
    max_retries: u64,
    err: CodexErr,
    client_session: &mut ModelClientSession,
    sess: &Session,
    step_context: &StepContext,
    request: ResponsesStreamRequest,
) -> Result<(), CodexErr> {
    let turn_context = &step_context.turn;
    if matches!(request, ResponsesStreamRequest::Sampling)
        && matches!(err.details(), CodexErrorDetails::ContentFilter)
    {
        let model_info = &step_context.settings.model_info;
        let guidance = ContentFilterGuidance {
            text: codex_prompts::ResolvedModelMessages::from_model(model_info)
                .content_filter_guidance()
                .to_string(),
        };
        sess.record_conversation_items(
            turn_context,
            model_info,
            &[ContextualUserFragment::into(guidance)],
        )
        .await;
    }
    let operation = match request {
        ResponsesStreamRequest::Sampling => RetryOperation::Sampling,
        ResponsesStreamRequest::RemoteCompactionV2 => RetryOperation::RemoteCompactionV2,
    };
    let retry_count = retry_state.retries.saturating_add(1);
    let Some(delay) = err.retry_delay(retry_count) else {
        return Err(err);
    };
    let retry_after = err.retry_after();

    if turn_context
        .config
        .features
        .enabled(Feature::UnboundedConnectionRetries)
        && matches!(request, ResponsesStreamRequest::Sampling)
        && matches!(err.details(), CodexErrorDetails::ConnectionFailed(_))
        && !turn_context.session_source.is_internal()
        && !turn_context.provider.info().is_amazon_bedrock()
    {
        let retry_delay = retry_state.connection_retry_delay;
        warn!(
            turn_id = %turn_context.sub_id,
            error = %err,
            ?retry_delay,
            "stream connection failed; waiting to retry"
        );
        sess.notify_stream_error(turn_context, "Reconnecting... waiting for network", err)
            .await;
        retry_state.connection_retries = retry_state.connection_retries.saturating_add(1);
        codex_client::record_retry!(retry_state.connection_retries, retry_delay, operation);
        tokio::time::sleep(retry_delay).await;
        retry_state.connection_retry_delay = retry_delay
            .saturating_mul(2)
            .min(MAX_CONNECTION_RETRY_DELAY);
        return Ok(());
    }

    let requires_http_fallback = err.requires_http_fallback();
    let switched = (requires_http_fallback || retry_state.retries >= max_retries)
        && client_session.try_switch_fallback_transport(
            &turn_context.session_telemetry,
            turn_context.model_info(),
        );
    if switched {
        // Changing transport must not bypass the server's retry deadline.
        if let Some(retry_after) = retry_after {
            tokio::time::sleep_until(retry_after.deadline()).await;
        }
        sess.send_event(
            turn_context,
            EventMsg::Warning(WarningEvent {
                message: format!("Falling back from WebSockets to HTTPS transport. {err:#}"),
            }),
        )
        .await;
    }
    // Another request can already have switched this session to HTTP.
    if switched || requires_http_fallback {
        retry_state.retries = 0;
        return Ok(());
    }

    if retry_state.retries < max_retries {
        retry_state.retries = retry_count;
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
        // Use one clock sample so local backoff telemetry retains the selected delay.
        let now = Instant::now();
        let retry_at = retry_after.map(RetryAfter::deadline).unwrap_or(now + delay);
        let delay = retry_at.saturating_duration_since(now);
        codex_client::record_retry!(retry_count, delay, operation);
        tokio::time::sleep_until(retry_at).await;
        return Ok(());
    }

    sess.services
        .thread_extension_data
        .insert(ExhaustedResponseRetry {
            turn_id: turn_context.sub_id.clone(),
            retry_at: retry_after.map(RetryAfter::deadline),
        });
    Err(err)
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
                turn_id = %turn_context.sub_id,
                retries,
                max_retries,
                sampling_error = %err,
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

/// Waits when the selected model reports capacity pressure, returning `Ok(())` when the caller
/// should retry the request loop. Unlike bounded stream retries this waits indefinitely so long
/// capacity outages do not end a turn; the caller owns cancellation by dropping this future.
/// Server advice sets the deadline when present, otherwise the wait grows exponentially.
pub(crate) async fn wait_for_server_overload_retry(
    retry_state: &mut ResponsesStreamRetryState,
    sess: &Session,
    turn_context: &TurnContext,
    err: CodexErr,
) -> Result<(), CodexErr> {
    if !matches!(err.details(), CodexErrorDetails::ServerOverloaded) {
        return Err(err);
    }
    retry_state.server_overload_attempts = retry_state.server_overload_attempts.saturating_add(1);
    let attempt = retry_state.server_overload_attempts;
    let backoff_delay = retry_state.server_overload_retry_delay;
    // Use one clock sample so the notification and the sleep agree on the remaining delay.
    let now = Instant::now();
    let retry_at = err
        .retry_after()
        .map(RetryAfter::deadline)
        .unwrap_or(now + backoff_delay);
    let delay = retry_at.saturating_duration_since(now);
    warn!(
        turn_id = %turn_context.sub_id,
        attempt,
        ?delay,
        error = %err,
        "model at capacity; waiting to retry"
    );
    sess.notify_stream_error(
        turn_context,
        format!(
            "Model at capacity, retrying in {}s (attempt {attempt})",
            delay.as_secs()
        ),
        err,
    )
    .await;
    retry_state.server_overload_retry_delay = backoff_delay
        .saturating_mul(2)
        .min(MAX_SERVER_OVERLOAD_RETRY_DELAY);
    tokio::time::sleep_until(retry_at).await;
    Ok(())
}

/// Waits until a usage-limit window resets and returns `Ok(())` when the caller should retry the
/// sampling request. Returns the original error when auto-wait does not apply.
pub(crate) async fn wait_for_usage_limit_reset_if_applicable(
    sess: &Session,
    turn_context: &TurnContext,
    err: CodexErr,
    cancellation_token: &CancellationToken,
) -> Result<(), CodexErr> {
    let CodexErrorDetails::UsageLimitReached(limit) = err.details() else {
        return Err(err);
    };

    if !is_auto_waitable_usage_limit(limit) {
        return Err(err);
    }

    let Some(resets_at) = limit.resets_at else {
        return Err(err);
    };

    let Some(delay) = delay_until_usage_limit_reset(resets_at) else {
        return Err(err);
    };

    warn!(
        turn_id = %turn_context.sub_id,
        ?delay,
        "usage limit reached; waiting for reset before retrying"
    );
    sess.notify_stream_error(
        turn_context,
        "Waiting for usage limit to reset...".to_string(),
        err,
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

#[cfg(test)]
#[path = "responses_retry_tests.rs"]
mod tests;
