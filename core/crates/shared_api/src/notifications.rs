use chrono::{DateTime, Utc};
use models::{NormalizedItem, NotificationEvent};
use notifications::NotificationCandidate;

pub fn notification_candidate_from_item(
    feed_id: &str,
    item: &NormalizedItem,
) -> NotificationCandidate {
    NotificationCandidate {
        feed_id: feed_id.to_owned(),
        entry_id: item.id.clone(),
        title: item.title.clone(),
        canonical_url: item.canonical_url.clone(),
        summary: item.summary.clone(),
        content_text: item.content_text.clone(),
        content_html: item.content_html.clone(),
        published_at: item.published_at,
        updated_at: item.updated_at,
        raw_hash: item.raw_hash.clone(),
    }
}

pub fn notification_event_from_row(row: storage::NotificationEventRow) -> NotificationEvent {
    NotificationEvent {
        id: row.id,
        feed_id: row.feed_id,
        entry_id: row.entry_id,
        canonical_key: row.canonical_key,
        content_fingerprint: row.content_fingerprint,
        title: row.title,
        body: row.body,
        mode: row.mode,
        delivery_state: row.delivery_state,
        reason: row.reason,
        digest_id: row.digest_id,
        created_at: parse_datetime(row.created_at).unwrap_or_else(Utc::now),
        ready_at: row.ready_at.and_then(parse_datetime),
        delivered_at: row.delivered_at.and_then(parse_datetime),
        suppressed_at: row.suppressed_at.and_then(parse_datetime),
    }
}

pub fn parse_datetime(value: String) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(&value).map(|value| value.with_timezone(&Utc)).ok()
}
