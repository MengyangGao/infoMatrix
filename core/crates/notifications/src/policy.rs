use chrono::{DateTime, Timelike, Utc};
use models::{NotificationMode, NotificationSettings};

use crate::{NotificationCandidate, NotificationEventDraft};

/// Why a notification was suppressed or queued.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NotificationSuppressionReason {
    /// Notifications are disabled for the feed.
    Disabled,
    /// Duplicate canonical item / fingerprint pair.
    Duplicate,
    /// Quiet hours are active.
    QuietHours,
    /// Minimum interval has not elapsed.
    MinimumInterval,
    /// Include keywords did not match.
    KeywordIncludeMiss,
    /// Exclude keywords matched.
    KeywordExcludeHit,
}

/// Result of policy evaluation.
#[derive(Debug, Clone, PartialEq)]
pub enum NotificationDecision {
    /// Deliver now as a local notification.
    Send(NotificationEventDraft),
    /// Queue into a digest batch.
    QueueDigest(NotificationEventDraft),
    /// Suppress the candidate.
    Suppress(NotificationSuppressionReason),
}

/// Evaluate whether a candidate should notify.
#[allow(clippy::too_many_arguments)]
pub fn evaluate_candidate(
    settings: &NotificationSettings,
    candidate: &NotificationCandidate,
    canonical_key: String,
    content_fingerprint: String,
    now: DateTime<Utc>,
    local_minute_of_day: u16,
    is_duplicate: bool,
    last_notification_at: Option<DateTime<Utc>>,
) -> NotificationDecision {
    if !settings.enabled {
        return NotificationDecision::Suppress(NotificationSuppressionReason::Disabled);
    }
    if is_duplicate {
        return NotificationDecision::Suppress(NotificationSuppressionReason::Duplicate);
    }
    if !matches_keywords(settings, candidate) {
        return NotificationDecision::Suppress(NotificationSuppressionReason::KeywordIncludeMiss);
    }
    if is_excluded(settings, candidate) {
        return NotificationDecision::Suppress(NotificationSuppressionReason::KeywordExcludeHit);
    }
    if let Some(previous) = last_notification_at {
        let elapsed = now.signed_duration_since(previous).num_minutes().max(0) as u32;
        if elapsed < settings.minimum_interval_minutes && !settings.high_priority {
            return NotificationDecision::Suppress(NotificationSuppressionReason::MinimumInterval);
        }
    }
    if quiet_hours_active(settings, local_minute_of_day) && !settings.high_priority {
        return NotificationDecision::Suppress(NotificationSuppressionReason::QuietHours);
    }

    let body = candidate.body_text();
    let draft = NotificationEventDraft {
        feed_id: candidate.feed_id.clone(),
        entry_id: candidate.entry_id.clone(),
        canonical_key,
        content_fingerprint,
        title: candidate.title.clone(),
        body,
        mode: settings.mode,
        reason: "new_item".to_owned(),
        created_at: now,
        ready_at: Some(now),
    };

    match settings.mode {
        NotificationMode::Immediate => NotificationDecision::Send(draft),
        NotificationMode::Digest => NotificationDecision::QueueDigest(draft),
    }
}

fn quiet_hours_active(settings: &NotificationSettings, local_minute_of_day: u16) -> bool {
    let quiet = &settings.quiet_hours;
    if !quiet.enabled {
        return false;
    }
    let minute = local_minute_of_day;
    let start = quiet.start_minute;
    let end = quiet.end_minute;
    if start == end {
        return true;
    }
    if start < end { minute >= start && minute < end } else { minute >= start || minute < end }
}

fn matches_keywords(settings: &NotificationSettings, candidate: &NotificationCandidate) -> bool {
    if settings.keyword_include.is_empty() {
        return true;
    }
    let haystack = searchable_text(candidate);
    settings.keyword_include.iter().any(|keyword| haystack.contains(&keyword.to_lowercase()))
}

fn is_excluded(settings: &NotificationSettings, candidate: &NotificationCandidate) -> bool {
    if settings.keyword_exclude.is_empty() {
        return false;
    }
    let haystack = searchable_text(candidate);
    settings.keyword_exclude.iter().any(|keyword| haystack.contains(&keyword.to_lowercase()))
}

fn searchable_text(candidate: &NotificationCandidate) -> String {
    let mut text = vec![candidate.title.to_lowercase()];
    if let Some(summary) = candidate.summary.as_ref() {
        text.push(summary.to_lowercase());
    }
    if let Some(content_text) = candidate.content_text.as_ref() {
        text.push(content_text.to_lowercase());
    }
    if let Some(content_html) = candidate.content_html.as_ref() {
        text.push(content_html.to_lowercase());
    }
    text.join(" ")
}

/// Convert a notification mode to a human-readable label.
pub fn mode_label(mode: NotificationMode) -> &'static str {
    match mode {
        NotificationMode::Immediate => "immediate",
        NotificationMode::Digest => "digest",
    }
}

/// Human-readable quiet-hour status.
pub fn quiet_hours_label(settings: &NotificationSettings) -> String {
    if !settings.quiet_hours.enabled {
        return "off".to_owned();
    }
    format!(
        "{:02}:{:02}-{:02}:{:02}",
        settings.quiet_hours.start_minute / 60,
        settings.quiet_hours.start_minute % 60,
        settings.quiet_hours.end_minute / 60,
        settings.quiet_hours.end_minute % 60
    )
}

/// Current local minute-of-day helper.
pub fn local_minute_of_day(now: DateTime<Utc>, offset_minutes: i32) -> u16 {
    let adjusted = now + chrono::Duration::minutes(offset_minutes as i64);
    (adjusted.hour() as u16) * 60 + adjusted.minute() as u16
}

#[cfg(test)]
mod tests {
    use chrono::TimeZone;
    use models::{DigestPolicy, NotificationMode, NotificationSettings, QuietHours};

    use crate::NotificationCandidate;

    use super::{
        NotificationDecision, NotificationSuppressionReason, evaluate_candidate,
        local_minute_of_day,
    };

    fn sample_candidate() -> NotificationCandidate {
        NotificationCandidate {
            feed_id: "feed-1".to_owned(),
            entry_id: "entry-1".to_owned(),
            title: "Rust 1.0 released".to_owned(),
            canonical_url: None,
            summary: Some("Rust".to_owned()),
            content_text: Some("Rust content".to_owned()),
            content_html: None,
            published_at: None,
            updated_at: None,
            raw_hash: "hash".to_owned(),
        }
    }

    #[test]
    fn disabled_feeds_are_suppressed() {
        let settings = NotificationSettings::default();
        let now = chrono::Utc.with_ymd_and_hms(2026, 3, 23, 12, 0, 0).single().unwrap();
        let decision = evaluate_candidate(
            &settings,
            &sample_candidate(),
            "canonical".to_owned(),
            "fingerprint".to_owned(),
            now,
            local_minute_of_day(now, 0),
            false,
            None,
        );
        assert!(matches!(
            decision,
            NotificationDecision::Suppress(NotificationSuppressionReason::Disabled)
        ));
    }

    #[test]
    fn keyword_include_and_quiet_hours_apply() {
        let mut settings = NotificationSettings {
            enabled: true,
            mode: NotificationMode::Immediate,
            digest_policy: DigestPolicy::default(),
            quiet_hours: QuietHours { enabled: true, start_minute: 21 * 60, end_minute: 7 * 60 },
            minimum_interval_minutes: 30,
            high_priority: false,
            keyword_include: vec!["rust".to_owned()],
            keyword_exclude: vec!["spoiler".to_owned()],
        };
        let now = chrono::Utc.with_ymd_and_hms(2026, 3, 23, 22, 0, 0).single().unwrap();
        let decision = evaluate_candidate(
            &settings,
            &sample_candidate(),
            "canonical".to_owned(),
            "fingerprint".to_owned(),
            now,
            local_minute_of_day(now, 0),
            false,
            None,
        );
        assert!(matches!(
            decision,
            NotificationDecision::Suppress(NotificationSuppressionReason::QuietHours)
        ));

        settings.quiet_hours.enabled = false;
        let decision = evaluate_candidate(
            &settings,
            &sample_candidate(),
            "canonical".to_owned(),
            "fingerprint".to_owned(),
            now,
            local_minute_of_day(now, 0),
            false,
            None,
        );
        assert!(matches!(decision, NotificationDecision::Send(_)));
    }

    #[test]
    fn digest_mode_queues_digest_batches() {
        let settings = NotificationSettings {
            enabled: true,
            mode: NotificationMode::Digest,
            digest_policy: DigestPolicy { enabled: true, interval_minutes: 60, max_items: 20 },
            quiet_hours: QuietHours::default(),
            minimum_interval_minutes: 10,
            high_priority: false,
            keyword_include: vec![],
            keyword_exclude: vec![],
        };
        let now = chrono::Utc.with_ymd_and_hms(2026, 3, 23, 12, 0, 0).single().unwrap();
        let decision = evaluate_candidate(
            &settings,
            &sample_candidate(),
            "canonical".to_owned(),
            "fingerprint".to_owned(),
            now,
            local_minute_of_day(now, 0),
            false,
            None,
        );
        assert!(matches!(decision, NotificationDecision::QueueDigest(_)));
    }
}
