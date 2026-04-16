use chrono::{DateTime, Utc};
use models::NotificationSettings;

use crate::{
    NotificationCandidate, NotificationDigestDraft, NotificationEventDraft,
    audit::{canonical_identity_key, content_fingerprint},
    policy::{NotificationDecision, NotificationSuppressionReason, evaluate_candidate},
};

/// Convert a feed refresh result into notification-ready drafts.
pub fn summarize_candidates(
    settings: &NotificationSettings,
    candidates: &[NotificationCandidate],
    now: DateTime<Utc>,
    local_minute_of_day: u16,
    duplicate_checker: impl Fn(&str, &str) -> bool,
    last_notification_at: Option<DateTime<Utc>>,
) -> Vec<NotificationDecision> {
    let mut decisions = Vec::with_capacity(candidates.len());
    for candidate in candidates {
        let canonical_key = canonical_identity_key_from_candidate(candidate);
        let fingerprint = fingerprint_from_candidate(candidate);
        let duplicate = duplicate_checker(&canonical_key, &fingerprint);
        decisions.push(evaluate_candidate(
            settings,
            candidate,
            canonical_key,
            fingerprint,
            now,
            local_minute_of_day,
            duplicate,
            last_notification_at,
        ));
    }
    decisions
}

/// Build a digest batch for a set of queued notification drafts.
pub fn build_digest_batch(
    feed_id: String,
    drafts: &[NotificationEventDraft],
    now: DateTime<Utc>,
) -> Option<NotificationDigestDraft> {
    if drafts.is_empty() {
        return None;
    }
    let mut ordered = drafts.to_vec();
    ordered.sort_by(|a, b| a.created_at.cmp(&b.created_at).then_with(|| a.title.cmp(&b.title)));

    let entry_ids = ordered.iter().map(|draft| draft.entry_id.clone()).collect::<Vec<_>>();
    let preview_titles =
        ordered.iter().take(3).map(|draft| draft.title.clone()).collect::<Vec<_>>();
    let title = if ordered.len() == 1 {
        format!("1 条新内容: {}", ordered[0].title)
    } else {
        format!("{} 条新内容", ordered.len())
    };
    let body = if preview_titles.is_empty() {
        "新的更新已整理进摘要".to_owned()
    } else {
        preview_titles.join(" · ")
    };

    Some(NotificationDigestDraft {
        feed_id,
        entry_ids,
        title,
        body,
        created_at: now,
        ready_at: Some(now),
    })
}

fn canonical_identity_key_from_candidate(candidate: &NotificationCandidate) -> String {
    let normalized = models::NormalizedItem {
        id: candidate.entry_id.clone(),
        source_feed_id: candidate.feed_id.clone(),
        external_item_id: None,
        canonical_url: candidate.canonical_url.clone(),
        title: candidate.title.clone(),
        author: None,
        summary: candidate.summary.clone(),
        content_html: candidate.content_html.clone(),
        content_text: candidate.content_text.clone(),
        published_at: candidate.published_at,
        updated_at: candidate.updated_at,
        raw_hash: candidate.raw_hash.clone(),
        dedup_reason: None,
        duplicate_of_item_id: None,
    };
    canonical_identity_key(&normalized)
}

fn fingerprint_from_candidate(candidate: &NotificationCandidate) -> String {
    let normalized = models::NormalizedItem {
        id: candidate.entry_id.clone(),
        source_feed_id: candidate.feed_id.clone(),
        external_item_id: None,
        canonical_url: candidate.canonical_url.clone(),
        title: candidate.title.clone(),
        author: None,
        summary: candidate.summary.clone(),
        content_html: candidate.content_html.clone(),
        content_text: candidate.content_text.clone(),
        published_at: candidate.published_at,
        updated_at: candidate.updated_at,
        raw_hash: candidate.raw_hash.clone(),
        dedup_reason: None,
        duplicate_of_item_id: None,
    };
    content_fingerprint(&normalized)
}

/// Build a text summary for a notification draft.
pub fn event_body_from_candidates(
    candidates: &[NotificationCandidate],
    max_items: usize,
) -> String {
    let titles = candidates
        .iter()
        .take(max_items)
        .map(|candidate| candidate.title.as_str())
        .collect::<Vec<_>>();
    if titles.is_empty() { "有新的更新".to_owned() } else { titles.join(" · ") }
}

/// Extract queued drafts from policy decisions.
pub fn queued_drafts(decisions: &[NotificationDecision]) -> Vec<NotificationEventDraft> {
    decisions
        .iter()
        .filter_map(|decision| match decision {
            NotificationDecision::QueueDigest(draft) | NotificationDecision::Send(draft) => {
                Some(draft.clone())
            }
            NotificationDecision::Suppress(_) => None,
        })
        .collect()
}

/// Count suppressions for audit.
pub fn suppression_count(decisions: &[NotificationDecision]) -> usize {
    decisions
        .iter()
        .filter(|decision| matches!(decision, NotificationDecision::Suppress(_)))
        .count()
}

/// Convert a suppression reason to a stable string.
pub fn suppression_reason_label(reason: &NotificationSuppressionReason) -> &'static str {
    match reason {
        NotificationSuppressionReason::Disabled => "disabled",
        NotificationSuppressionReason::Duplicate => "duplicate",
        NotificationSuppressionReason::QuietHours => "quiet_hours",
        NotificationSuppressionReason::MinimumInterval => "minimum_interval",
        NotificationSuppressionReason::KeywordIncludeMiss => "keyword_include_miss",
        NotificationSuppressionReason::KeywordExcludeHit => "keyword_exclude_hit",
    }
}

#[cfg(test)]
mod tests {
    use chrono::TimeZone;
    use models::{NotificationMode, NotificationSettings};

    use crate::{NotificationCandidate, NotificationEventDraft};

    use super::{build_digest_batch, summarize_candidates};

    fn sample_candidate(entry_id: &str) -> NotificationCandidate {
        NotificationCandidate {
            feed_id: "feed-1".to_owned(),
            entry_id: entry_id.to_owned(),
            title: format!("Item {entry_id}"),
            canonical_url: None,
            summary: Some("summary".to_owned()),
            content_text: Some("content".to_owned()),
            content_html: None,
            published_at: None,
            updated_at: None,
            raw_hash: format!("hash-{entry_id}"),
        }
    }

    #[test]
    fn digest_batch_groups_titles() {
        let now = chrono::Utc.with_ymd_and_hms(2026, 3, 23, 12, 0, 0).single().unwrap();
        let drafts = vec![
            NotificationEventDraft {
                feed_id: "feed-1".to_owned(),
                entry_id: "1".to_owned(),
                canonical_key: "canonical-1".to_owned(),
                content_fingerprint: "fingerprint-1".to_owned(),
                title: "First".to_owned(),
                body: "First body".to_owned(),
                mode: NotificationMode::Digest,
                reason: "new_item".to_owned(),
                created_at: now,
                ready_at: Some(now),
            },
            NotificationEventDraft {
                feed_id: "feed-1".to_owned(),
                entry_id: "2".to_owned(),
                canonical_key: "canonical-2".to_owned(),
                content_fingerprint: "fingerprint-2".to_owned(),
                title: "Second".to_owned(),
                body: "Second body".to_owned(),
                mode: NotificationMode::Digest,
                reason: "new_item".to_owned(),
                created_at: now,
                ready_at: Some(now),
            },
        ];

        let digest = build_digest_batch("feed-1".to_owned(), &drafts, now).expect("digest");
        assert_eq!(digest.entry_ids, vec!["1".to_owned(), "2".to_owned()]);
        assert!(digest.title.contains("2 条新内容"));
    }

    #[test]
    fn summarize_candidates_uses_duplicate_checker() {
        let settings = NotificationSettings {
            enabled: true,
            mode: NotificationMode::Immediate,
            digest_policy: Default::default(),
            quiet_hours: Default::default(),
            minimum_interval_minutes: 30,
            high_priority: false,
            keyword_include: vec![],
            keyword_exclude: vec![],
        };
        let now = chrono::Utc.with_ymd_and_hms(2026, 3, 23, 12, 0, 0).single().unwrap();
        let decisions = summarize_candidates(
            &settings,
            &[sample_candidate("1"), sample_candidate("2")],
            now,
            12 * 60,
            |_, _| true,
            None,
        );
        assert_eq!(decisions.len(), 2);
        assert!(
            decisions.iter().any(|decision| matches!(
                decision,
                crate::policy::NotificationDecision::Suppress(_)
            ))
        );
    }
}
