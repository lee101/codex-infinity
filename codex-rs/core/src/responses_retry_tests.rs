use std::time::Duration;

use chrono::TimeZone;
use chrono::Utc;
use codex_protocol::auth::KnownPlan;
use codex_protocol::auth::PlanType;
use codex_protocol::error::UsageLimitReachedError;
use codex_protocol::protocol::RateLimitReachedType;
use pretty_assertions::assert_eq;

use super::delay_until_usage_limit_reset;
use super::is_auto_waitable_usage_limit;

#[test]
fn auto_wait_skips_workspace_credit_and_spend_cap_errors() {
    let resets_at = Utc.with_ymd_and_hms(2026, 7, 10, 13, 0, 0).unwrap();
    for rate_limit_reached_type in [
        RateLimitReachedType::WorkspaceOwnerCreditsDepleted,
        RateLimitReachedType::WorkspaceMemberCreditsDepleted,
        RateLimitReachedType::WorkspaceOwnerUsageLimitReached,
        RateLimitReachedType::WorkspaceMemberUsageLimitReached,
    ] {
        let err = UsageLimitReachedError {
            plan_type: Some(PlanType::Known(KnownPlan::Pro)),
            resets_at: Some(resets_at),
            rate_limits: None,
            promo_message: None,
            rate_limit_reached_type: Some(rate_limit_reached_type),
        };
        assert!(!is_auto_waitable_usage_limit(&err));
    }
}

#[test]
fn auto_wait_allows_rate_limit_with_reset_time() {
    let resets_at = Utc.with_ymd_and_hms(2026, 7, 10, 13, 0, 0).unwrap();
    for rate_limit_reached_type in [Some(RateLimitReachedType::RateLimitReached), None] {
        let err = UsageLimitReachedError {
            plan_type: Some(PlanType::Known(KnownPlan::Pro)),
            resets_at: Some(resets_at),
            rate_limits: None,
            promo_message: None,
            rate_limit_reached_type,
        };
        assert!(is_auto_waitable_usage_limit(&err));
    }
}

#[test]
fn auto_wait_requires_reset_time() {
    let err = UsageLimitReachedError {
        plan_type: Some(PlanType::Known(KnownPlan::Pro)),
        resets_at: None,
        rate_limits: None,
        promo_message: None,
        rate_limit_reached_type: None,
    };
    assert!(!is_auto_waitable_usage_limit(&err));
}

#[test]
fn delay_until_reset_is_zero_when_reset_time_has_passed() {
    let resets_at = Utc::now() - chrono::Duration::minutes(5);
    assert_eq!(
        delay_until_usage_limit_reset(resets_at),
        Some(Duration::from_secs(0))
    );
}
