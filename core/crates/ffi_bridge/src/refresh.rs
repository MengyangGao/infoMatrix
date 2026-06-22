use chrono::{Local, Utc};
use fetcher::{FetchError, FetchOutcome, FetchRequest, FetchService, ReqwestFeedClient};
use models::{FeedType, NotificationDeliveryState, NotificationEvent, NotificationMode};
use notifications::{
    NotificationDecision, build_digest_batch, canonical_identity_key, content_fingerprint,
};
use serde::{Deserialize, Serialize};
use shared_api::constants::{DEFAULT_TIMEOUT_SECS, USER_AGENT};
use shared_api::feed_snapshot::{FeedSnapshotFetchMetadata, persist_parsed_feed_snapshot};
use shared_api::notifications::notification_candidate_from_item;
use shared_api::url::derive_site_url_from_feed;
use storage::Storage;

#[derive(Debug, Deserialize)]
pub struct RefreshInput {
    pub db_path: Option<String>,
    pub feed_id: String,
}

#[derive(Debug, Deserialize)]
pub struct RefreshDueInput {
    pub db_path: Option<String>,
    pub limit: Option<usize>,
}

#[derive(Debug, Serialize)]
pub struct RefreshOutput {
    pub status: String,
    pub fetched_http_status: u16,
    pub item_count: usize,
    pub notification_count: usize,
    pub suppressed_notification_count: usize,
}

#[derive(Debug, Serialize)]
pub struct RefreshDueOutput {
    pub refreshed_count: usize,
    pub total_item_count: usize,
}

#[derive(Debug)]
pub struct DirectFeedProbe {
    pub response: fetcher::FetchResponse,
    pub parsed: parser::ParsedFeed,
}

pub fn refresh_feed_with_runtime(
    runtime: &tokio::runtime::Runtime,
    storage: &mut Storage,
    feed_id: &str,
) -> Result<RefreshOutput, String> {
    let feed = storage.get_feed(feed_id).map_err(|err| err.to_string())?;

    let client = ReqwestFeedClient::new(DEFAULT_TIMEOUT_SECS, USER_AGENT)
        .map_err(|err| format!("failed to initialize fetch client: {err}"))?;
    let service = FetchService::new(client);
    let request = FetchRequest {
        url: feed.feed_url.clone(),
        etag: feed.etag,
        last_modified: feed.last_modified,
    };

    let outcome = runtime
        .block_on(service.fetch_and_parse(feed_id, &request))
        .map_err(|err| err.to_string())?;

    match outcome {
        FetchOutcome::NotModified(response) => {
            storage
                .record_fetch_result(
                    feed_id,
                    response.status,
                    response.etag.as_deref(),
                    response.last_modified.as_deref(),
                    response.duration_ms,
                    None,
                )
                .map_err(|err| err.to_string())?;

            Ok(RefreshOutput {
                status: "not_modified".to_owned(),
                fetched_http_status: response.status,
                item_count: 0,
                notification_count: 0,
                suppressed_notification_count: 0,
            })
        }
        FetchOutcome::Updated { response, parsed } => {
            let item_count = parsed.items.len();
            let persisted_feed_id = persist_parsed_feed_snapshot(
                storage,
                &feed.feed_url,
                parsed.title.clone(),
                derive_site_url_from_feed(&feed.feed_url),
                &parsed,
                FeedSnapshotFetchMetadata {
                    http_status: response.status,
                    etag: response.etag,
                    last_modified: response.last_modified,
                    duration_ms: response.duration_ms,
                },
            )
            .map_err(|err| err.to_string())?;

            let notification_result =
                process_refresh_notifications(storage, &persisted_feed_id, &parsed.items)?;

            Ok(RefreshOutput {
                status: "updated".to_owned(),
                fetched_http_status: response.status,
                item_count,
                notification_count: notification_result.sent_count,
                suppressed_notification_count: notification_result.suppressed_count,
            })
        }
    }
}

pub fn probe_direct_feed(
    runtime: &tokio::runtime::Runtime,
    candidate_url: &url::Url,
) -> Result<Option<DirectFeedProbe>, String> {
    let client = ReqwestFeedClient::new(DEFAULT_TIMEOUT_SECS, USER_AGENT)
        .map_err(|err| format!("failed to initialize fetch client: {err}"))?;
    let service = FetchService::new(client);
    let request = FetchRequest { url: candidate_url.clone(), etag: None, last_modified: None };

    let outcome = match runtime.block_on(service.fetch_and_parse("ffi_direct_probe", &request)) {
        Ok(outcome) => outcome,
        Err(FetchError::Parse(_)) => return Ok(None),
        Err(FetchError::UnexpectedStatus(status)) if status >= 400 => return Ok(None),
        Err(err) => return Err(err.to_string()),
    };

    match outcome {
        FetchOutcome::NotModified(_) => Ok(None),
        FetchOutcome::Updated { response, parsed } => {
            if parsed.feed_type == FeedType::Unknown {
                return Ok(None);
            }
            Ok(Some(DirectFeedProbe { response, parsed }))
        }
    }
}

struct NotificationProcessingResult {
    sent_count: usize,
    suppressed_count: usize,
}

fn process_refresh_notifications(
    storage: &mut Storage,
    feed_id: &str,
    items: &[models::NormalizedItem],
) -> Result<NotificationProcessingResult, String> {
    let globals = storage.get_global_notification_settings().map_err(|err| err.to_string())?;
    let settings = storage
        .get_notification_settings(feed_id)
        .map_err(|err| err.to_string())?
        .map(|row| row.settings)
        .unwrap_or_else(|| globals.default_feed_settings.clone());

    let now = Utc::now();
    let local_offset_minutes = Local::now().offset().local_minus_utc() / 60;
    let local_minute_of_day = notifications::policy::local_minute_of_day(now, local_offset_minutes);
    let mut last_notification_at =
        storage.get_latest_notification_at_for_feed(feed_id).map_err(|err| err.to_string())?;

    let mut sent_count = 0usize;
    let mut suppressed_count = 0usize;
    let mut digest_drafts = Vec::new();

    for item in items {
        let candidate = notification_candidate_from_item(feed_id, item);
        let canonical_key = canonical_identity_key(item);
        let fingerprint = content_fingerprint(item);
        let duplicate = storage
            .get_notification_signature(&canonical_key, &fingerprint)
            .map_err(|err| err.to_string())?
            .is_some();
        let seen_at = now.to_rfc3339();
        let _ = storage
            .record_notification_signature(
                feed_id,
                &candidate.entry_id,
                &canonical_key,
                &fingerprint,
                &seen_at,
                None,
            )
            .map_err(|err| err.to_string())?;
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
                let event = NotificationEvent {
                    id: uuid::Uuid::now_v7().to_string(),
                    feed_id: Some(feed_id.to_owned()),
                    entry_id: Some(draft.entry_id.clone()),
                    canonical_key: draft.canonical_key.clone(),
                    content_fingerprint: draft.content_fingerprint.clone(),
                    mode: draft.mode,
                    delivery_state: NotificationDeliveryState::Pending,
                    title: draft.title.clone(),
                    body: draft.body.clone(),
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
                storage
                    .insert_notification_event(&event, Some(&metadata_json))
                    .map_err(|err| err.to_string())?;
                storage
                    .touch_notification_signature(&canonical_key, &fingerprint, &seen_at)
                    .map_err(|err| err.to_string())?;
                last_notification_at = Some(now);
                sent_count = sent_count.saturating_add(1);
            }
            NotificationDecision::QueueDigest(draft) => {
                digest_drafts.push(draft);
            }
            NotificationDecision::Suppress(reason) => {
                let event = NotificationEvent {
                    id: uuid::Uuid::now_v7().to_string(),
                    feed_id: Some(feed_id.to_owned()),
                    entry_id: Some(candidate.entry_id.clone()),
                    canonical_key: canonical_key.clone(),
                    content_fingerprint: fingerprint.clone(),
                    mode: settings.mode,
                    delivery_state: NotificationDeliveryState::Suppressed,
                    title: candidate.title.clone(),
                    body: candidate.body_text(),
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
                storage
                    .insert_notification_event(&event, Some(&metadata_json))
                    .map_err(|err| err.to_string())?;
                suppressed_count = suppressed_count.saturating_add(1);
            }
        }
    }

    if !digest_drafts.is_empty() {
        let digest_batch = build_digest_batch(feed_id.to_owned(), &digest_drafts, now)
            .ok_or_else(|| "unable to build notification digest".to_owned())?;
        let digest = models::NotificationDigest {
            id: uuid::Uuid::now_v7().to_string(),
            feed_id: Some(feed_id.to_owned()),
            entry_count: digest_batch.entry_ids.len(),
            title: digest_batch.title.clone(),
            body: digest_batch.body.clone(),
            created_at: digest_batch.created_at,
            ready_at: digest_batch.ready_at,
            delivered_at: None,
        };
        let entry_ids_json =
            serde_json::to_string(&digest_batch.entry_ids).map_err(|err| err.to_string())?;
        storage
            .insert_notification_digest(&digest, &entry_ids_json)
            .map_err(|err| err.to_string())?;
        let digest_event = NotificationEvent {
            id: uuid::Uuid::now_v7().to_string(),
            feed_id: Some(feed_id.to_owned()),
            entry_id: digest_batch.entry_ids.first().cloned(),
            canonical_key: format!("digest:{feed_id}:{}", digest.id),
            content_fingerprint: digest.id.clone(),
            mode: NotificationMode::Digest,
            delivery_state: NotificationDeliveryState::Pending,
            title: digest.title.clone(),
            body: digest.body.clone(),
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
        storage
            .insert_notification_event(&digest_event, Some(&metadata_json))
            .map_err(|err| err.to_string())?;
        for draft in digest_drafts {
            storage
                .touch_notification_signature(
                    &draft.canonical_key,
                    &draft.content_fingerprint,
                    &now.to_rfc3339(),
                )
                .map_err(|err| err.to_string())?;
        }
        sent_count = sent_count.saturating_add(1);
    }

    Ok(NotificationProcessingResult { sent_count, suppressed_count })
}
