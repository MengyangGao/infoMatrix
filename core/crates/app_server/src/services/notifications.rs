use chrono::{Local, Utc};
use models::{
    NormalizedItem, NotificationDeliveryState, NotificationDigest, NotificationEvent,
    NotificationMode,
};
use notifications::{
    NotificationDecision, build_digest_batch, canonical_identity_key, content_fingerprint,
};
use serde_json;
use shared_api::notifications::notification_candidate_from_item;
use storage::Storage;
use uuid::Uuid;

use crate::error::ApiError;

pub(crate) struct NotificationProcessingResult {
    pub(crate) sent_count: usize,
    pub(crate) suppressed_count: usize,
}

pub(crate) fn process_refresh_notifications(
    storage: &mut Storage,
    feed_id: &str,
    items: &[NormalizedItem],
) -> Result<NotificationProcessingResult, ApiError> {
    let globals = storage.get_global_notification_settings()?;
    let settings = storage
        .get_notification_settings(feed_id)?
        .map(|row| row.settings)
        .unwrap_or_else(|| globals.default_feed_settings.clone());

    let now = Utc::now();
    let local_offset_minutes = Local::now().offset().local_minus_utc() / 60;
    let local_minute_of_day = notifications::policy::local_minute_of_day(now, local_offset_minutes);
    let mut last_notification_at = storage.get_latest_notification_at_for_feed(feed_id)?;

    let mut sent_count = 0usize;
    let mut suppressed_count = 0usize;
    let mut digest_drafts = Vec::new();

    for item in items {
        let candidate = notification_candidate_from_item(feed_id, item);
        let canonical_key = canonical_identity_key(item);
        let fingerprint = content_fingerprint(item);
        let duplicate = storage.get_notification_signature(&canonical_key, &fingerprint)?.is_some();
        let seen_at = now.to_rfc3339();
        let _ = storage.record_notification_signature(
            feed_id,
            &candidate.entry_id,
            &canonical_key,
            &fingerprint,
            &seen_at,
            None,
        )?;
        let decision = notifications::policy::evaluate_candidate(
            &settings,
            &candidate,
            canonical_key.clone(),
            fingerprint.clone(),
            now,
            local_minute_of_day,
            duplicate,
            last_notification_at,
        );

        match decision {
            NotificationDecision::Send(draft) => {
                let event_id = Uuid::now_v7().to_string();
                let event = NotificationEvent {
                    id: event_id,
                    feed_id: Some(feed_id.to_owned()),
                    entry_id: Some(draft.entry_id.clone()),
                    canonical_key: draft.canonical_key.clone(),
                    content_fingerprint: draft.content_fingerprint.clone(),
                    title: draft.title.clone(),
                    body: draft.body.clone(),
                    mode: draft.mode,
                    delivery_state: NotificationDeliveryState::Pending,
                    reason: draft.reason.clone(),
                    digest_id: None,
                    created_at: draft.created_at,
                    ready_at: draft.ready_at,
                    delivered_at: None,
                    suppressed_at: None,
                };
                let metadata_json = serde_json::json!({
                    "source": "refresh",
                    "decision": "send",
                    "feed_id": feed_id,
                    "entry_id": draft.entry_id,
                })
                .to_string();
                storage.insert_notification_event(&event, Some(&metadata_json))?;
                storage.touch_notification_signature(&canonical_key, &fingerprint, &seen_at)?;
                last_notification_at = Some(now);
                sent_count = sent_count.saturating_add(1);
            }
            NotificationDecision::QueueDigest(draft) => {
                digest_drafts.push(draft);
            }
            NotificationDecision::Suppress(reason) => {
                let event = NotificationEvent {
                    id: Uuid::now_v7().to_string(),
                    feed_id: Some(feed_id.to_owned()),
                    entry_id: Some(candidate.entry_id.clone()),
                    canonical_key: canonical_key.clone(),
                    content_fingerprint: fingerprint.clone(),
                    title: candidate.title.clone(),
                    body: candidate.body_text(),
                    mode: settings.mode,
                    delivery_state: NotificationDeliveryState::Suppressed,
                    reason: notifications::coordinator::suppression_reason_label(&reason)
                        .to_owned(),
                    digest_id: None,
                    created_at: now,
                    ready_at: None,
                    delivered_at: None,
                    suppressed_at: Some(now),
                };
                let metadata_json = serde_json::json!({
                    "source": "refresh",
                    "decision": "suppressed",
                    "reason": notifications::coordinator::suppression_reason_label(&reason),
                    "feed_id": feed_id,
                    "entry_id": candidate.entry_id,
                })
                .to_string();
                storage.insert_notification_event(&event, Some(&metadata_json))?;
                suppressed_count = suppressed_count.saturating_add(1);
            }
        }
    }

    if !digest_drafts.is_empty() {
        let digest_batch = build_digest_batch(feed_id.to_owned(), &digest_drafts, now)
            .ok_or_else(|| ApiError::Internal("unable to build notification digest".to_owned()))?;
        let digest = NotificationDigest {
            id: Uuid::now_v7().to_string(),
            feed_id: Some(feed_id.to_owned()),
            entry_count: digest_batch.entry_ids.len(),
            title: digest_batch.title.clone(),
            body: digest_batch.body.clone(),
            created_at: digest_batch.created_at,
            ready_at: digest_batch.ready_at,
            delivered_at: None,
        };
        let entry_ids_json = serde_json::to_string(&digest_batch.entry_ids)
            .map_err(|err| ApiError::Internal(err.to_string()))?;
        storage.insert_notification_digest(&digest, &entry_ids_json)?;
        let digest_event = NotificationEvent {
            id: Uuid::now_v7().to_string(),
            feed_id: Some(feed_id.to_owned()),
            entry_id: digest_batch.entry_ids.first().cloned(),
            canonical_key: format!("digest:{feed_id}:{}", digest.id),
            content_fingerprint: digest.id.clone(),
            title: digest.title.clone(),
            body: digest.body.clone(),
            mode: NotificationMode::Digest,
            delivery_state: NotificationDeliveryState::Pending,
            reason: "digest_batch".to_owned(),
            digest_id: Some(digest.id.clone()),
            created_at: digest.created_at,
            ready_at: digest.ready_at,
            delivered_at: None,
            suppressed_at: None,
        };
        let metadata_json = serde_json::json!({
            "source": "refresh",
            "decision": "digest",
            "feed_id": feed_id,
            "entry_ids": digest_batch.entry_ids,
        })
        .to_string();
        storage.insert_notification_event(&digest_event, Some(&metadata_json))?;
        for draft in digest_drafts {
            storage.touch_notification_signature(
                &draft.canonical_key,
                &draft.content_fingerprint,
                &now.to_rfc3339(),
            )?;
        }
        sent_count = sent_count.saturating_add(1);
    }

    Ok(NotificationProcessingResult { sent_count, suppressed_count })
}
