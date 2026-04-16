use chrono::{DateTime, Duration, Utc};
use models::FeedHealthState;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// Why a refresh is happening.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RefreshReason {
    /// Periodic background refresh.
    Periodic,
    /// Catch-up refresh on app launch.
    CatchUp,
    /// Manual user-triggered refresh.
    Manual,
}

/// Current durable state for a refresh job.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RefreshJobState {
    /// Feed identifier.
    pub feed_id: String,
    /// Last refresh attempt.
    pub last_attempt_at: Option<DateTime<Utc>>,
    /// Last successful refresh.
    pub last_success_at: Option<DateTime<Utc>>,
    /// Consecutive failure count.
    pub failure_count: u32,
    /// Next scheduled attempt time.
    pub next_attempt_at: Option<DateTime<Utc>>,
    /// Last HTTP status seen.
    pub last_http_status: Option<u16>,
    /// Last error message, if any.
    pub last_error_message: Option<String>,
    /// Last refresh reason.
    pub last_reason: Option<RefreshReason>,
}

/// Refresh attempt record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RefreshAttempt {
    /// Feed identifier.
    pub feed_id: String,
    /// Refresh reason.
    pub reason: RefreshReason,
    /// Started at.
    pub started_at: DateTime<Utc>,
    /// Finished at.
    pub finished_at: DateTime<Utc>,
    /// HTTP status observed if any.
    pub http_status: Option<u16>,
    /// Success flag.
    pub success: bool,
    /// Item count added or updated.
    pub item_count: usize,
    /// Optional error message.
    pub error_message: Option<String>,
    /// Next scheduled refresh.
    pub next_attempt_at: Option<DateTime<Utc>>,
}

/// Compute the next refresh time using bounded exponential backoff and deterministic jitter.
pub fn compute_next_refresh_at(
    now: DateTime<Utc>,
    reason: RefreshReason,
    health_state: FeedHealthState,
    failure_count: u32,
    jitter_seed: &str,
) -> DateTime<Utc> {
    let base = match reason {
        RefreshReason::Manual => Duration::minutes(30),
        RefreshReason::CatchUp => Duration::minutes(15),
        RefreshReason::Periodic => match health_state {
            FeedHealthState::Healthy => Duration::hours(1),
            FeedHealthState::Stale => Duration::hours(3),
            FeedHealthState::Failing => Duration::hours(6),
        },
    };
    let multiplier = 2u32.saturating_pow(failure_count.min(6));
    let base_seconds = (base.num_seconds() as u64).saturating_mul(multiplier as u64);
    let capped_seconds = base_seconds.min(24 * 60 * 60);
    let jitter_seconds = deterministic_jitter_seconds(jitter_seed, failure_count, capped_seconds);
    now + Duration::seconds((capped_seconds + jitter_seconds) as i64)
}

fn deterministic_jitter_seconds(seed: &str, failure_count: u32, ceiling: u64) -> u64 {
    let mut hasher = Sha256::new();
    hasher.update(seed.as_bytes());
    hasher.update(failure_count.to_be_bytes());
    let digest = hasher.finalize();
    let raw = u64::from_be_bytes([
        digest[0], digest[1], digest[2], digest[3], digest[4], digest[5], digest[6], digest[7],
    ]);
    let jitter_cap = (ceiling / 10).clamp(0, 15 * 60);
    if jitter_cap == 0 { 0 } else { raw % jitter_cap }
}

#[cfg(test)]
mod tests {
    use chrono::TimeZone;
    use models::FeedHealthState;

    use super::{RefreshReason, compute_next_refresh_at};

    #[test]
    fn schedules_with_deterministic_jitter() {
        let now = chrono::Utc.with_ymd_and_hms(2026, 3, 23, 12, 0, 0).single().expect("utc time");
        let first = compute_next_refresh_at(
            now,
            RefreshReason::Periodic,
            FeedHealthState::Healthy,
            2,
            "feed-1",
        );
        let second = compute_next_refresh_at(
            now,
            RefreshReason::Periodic,
            FeedHealthState::Healthy,
            2,
            "feed-1",
        );
        assert_eq!(first, second);
    }
}
