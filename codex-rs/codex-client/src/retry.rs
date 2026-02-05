use crate::error::TransportError;
use crate::request::Request;
use http::HeaderMap;
use http::header::RETRY_AFTER;
use rand::Rng;
use serde_json::Value;
use std::future::Future;
use std::time::Duration;
use tokio::time::sleep;

#[derive(Debug, Clone)]
pub struct RetryPolicy {
    pub max_attempts: u64,
    pub base_delay: Duration,
    pub retry_on: RetryOn,
}

#[derive(Debug, Clone)]
pub struct RetryOn {
    pub retry_429: bool,
    pub retry_5xx: bool,
    pub retry_transport: bool,
}

impl RetryOn {
    pub fn should_retry(&self, err: &TransportError, attempt: u64, max_attempts: u64) -> bool {
        if attempt >= max_attempts {
            return false;
        }
        match err {
            TransportError::Http { status, body, .. } => {
                if status.as_u16() == 429 {
                    return self.retry_429 && !is_non_retryable_429(body.as_deref());
                }
                self.retry_5xx && status.is_server_error()
            }
            TransportError::Timeout | TransportError::Network(_) => self.retry_transport,
            _ => false,
        }
    }
}

const MAX_BACKOFF_MS: u64 = 120_000; // 120 seconds

fn is_non_retryable_429(body: Option<&str>) -> bool {
    let Some(body) = body else {
        return false;
    };
    let Ok(value) = serde_json::from_str::<Value>(body) else {
        return false;
    };
    let Some(error) = value.get("error") else {
        return false;
    };

    let error_type = error.get("type").and_then(Value::as_str);
    let error_code = error.get("code").and_then(Value::as_str);

    matches!(
        error_type,
        Some("usage_limit_reached" | "usage_not_included")
    ) || matches!(error_code, Some("insufficient_quota" | "quota_exceeded"))
}

pub fn backoff(base: Duration, attempt: u64) -> Duration {
    if attempt == 0 {
        return base;
    }
    let exp = 2u64.saturating_pow(attempt as u32 - 1);
    let millis = base.as_millis() as u64;
    let raw = millis.saturating_mul(exp).min(MAX_BACKOFF_MS);
    let jitter: f64 = rand::rng().random_range(0.9..1.1);
    Duration::from_millis((raw as f64 * jitter) as u64)
}

fn retry_after_from_headers(headers: &HeaderMap) -> Option<Duration> {
    let value = headers.get(RETRY_AFTER)?;
    let value = value.to_str().ok()?;
    let seconds = value.parse::<f64>().ok()?;
    if seconds.is_sign_negative() {
        return None;
    }
    Some(Duration::from_secs_f64(seconds))
}

fn retry_delay(policy: &RetryPolicy, err: &TransportError, attempt: u64) -> Duration {
    if let TransportError::Http {
        status, headers, ..
    } = err
        && status.as_u16() == 429
        && let Some(headers) = headers
        && let Some(delay) = retry_after_from_headers(headers)
    {
        return delay.min(Duration::from_millis(MAX_BACKOFF_MS));
    }
    backoff(policy.base_delay, attempt)
}

pub async fn run_with_retry<T, F, Fut>(
    policy: RetryPolicy,
    mut make_req: impl FnMut() -> Request,
    op: F,
) -> Result<T, TransportError>
where
    F: Fn(Request, u64) -> Fut,
    Fut: Future<Output = Result<T, TransportError>>,
{
    for attempt in 0..=policy.max_attempts {
        let req = make_req();
        match op(req, attempt).await {
            Ok(resp) => return Ok(resp),
            Err(err)
                if policy
                    .retry_on
                    .should_retry(&err, attempt, policy.max_attempts) =>
            {
                sleep(retry_delay(&policy, &err, attempt + 1)).await;
            }
            Err(err) => return Err(err),
        }
    }
    Err(TransportError::RetryLimit)
}
