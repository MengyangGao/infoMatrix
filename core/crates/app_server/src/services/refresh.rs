use chrono::Utc;
use fetcher::{FetchError, FetchOutcome, FetchRequest, FetchService, ReqwestFeedClient};
use models::NewFeed;
use notifications::{RefreshAttempt, RefreshReason, compute_next_refresh_at};
use shared_api::url::feed_icon_url;
use storage::Storage;
use tracing::{info, warn};

use crate::config::AppContext;
use crate::error::ApiError;
use crate::services::icon::spawn_background_icon_refresh;
use crate::services::notifications::process_refresh_notifications;
use crate::state::open_storage;
use crate::views::{RefreshDueResponse, RefreshResponse};

pub(crate) async fn refresh_due_feeds_once(
    context: &AppContext,
    limit: usize,
    reason: RefreshReason,
) -> Result<RefreshDueResponse, ApiError> {
    let mut storage = open_storage(context)?;
    let due_feeds = storage.list_due_feeds(limit)?;
    let mut results = Vec::with_capacity(due_feeds.len());
    let mut total_item_count = 0usize;

    for feed in due_feeds {
        let result = refresh_feed_by_id(context, &mut storage, &feed.id, reason).await?;
        total_item_count = total_item_count.saturating_add(result.item_count);
        results.push(result);
    }

    Ok(RefreshDueResponse { refreshed_count: results.len(), total_item_count, results })
}

pub(crate) async fn refresh_feed_by_id(
    context: &AppContext,
    storage: &mut Storage,
    feed_id: &str,
    reason: RefreshReason,
) -> Result<RefreshResponse, ApiError> {
    let feed = storage.get_feed(feed_id)?;
    let feed_url = feed.feed_url.clone();
    let site_url = feed.site_url.clone();
    let started_at = Utc::now();

    let client = ReqwestFeedClient::new(context.timeout_secs, &context.user_agent)
        .map_err(|err| ApiError::Internal(err.to_string()))?;
    let service = FetchService::new(client);
    let request =
        FetchRequest { url: feed_url.clone(), etag: feed.etag, last_modified: feed.last_modified };

    let outcome = match service.fetch_and_parse(feed_id, &request).await {
        Ok(outcome) => outcome,
        Err(err) => {
            let (http_status, error_message, api_error) = match err {
                FetchError::UnexpectedStatus(status) => (
                    status,
                    format!("feed fetch returned HTTP {status}"),
                    ApiError::BadRequest(format!("feed refresh failed: HTTP {status}")),
                ),
                FetchError::Parse(message) => (
                    422,
                    format!("feed parse failed: {message}"),
                    ApiError::BadRequest(format!("feed refresh parse failed: {message}")),
                ),
                FetchError::Network(message) => (
                    599,
                    format!("feed network error: {message}"),
                    ApiError::Internal(format!("feed refresh network error: {message}")),
                ),
            };
            let _ = storage.record_fetch_result(
                feed_id,
                http_status,
                None,
                None,
                0,
                Some(&error_message),
            );
            if let Ok(updated_feed) = storage.get_feed(feed_id) {
                let finished_at = Utc::now();
                let next_attempt_at = compute_next_refresh_at(
                    finished_at,
                    reason,
                    updated_feed.health_state,
                    updated_feed.failure_count as u32,
                    feed_id,
                );
                let _ = storage.record_refresh_attempt(&RefreshAttempt {
                    feed_id: feed_id.to_owned(),
                    reason,
                    started_at,
                    finished_at,
                    http_status: Some(http_status),
                    success: false,
                    item_count: 0,
                    error_message: Some(error_message.clone()),
                    next_attempt_at: Some(next_attempt_at),
                });
            }
            return Err(api_error);
        }
    };

    match outcome {
        FetchOutcome::NotModified(response) => {
            storage.record_fetch_result(
                feed_id,
                response.status,
                response.etag.as_deref(),
                response.last_modified.as_deref(),
                response.duration_ms,
                None,
            )?;
            let updated_feed = storage.get_feed(feed_id)?;
            let finished_at = Utc::now();
            let next_attempt_at = compute_next_refresh_at(
                finished_at,
                reason,
                updated_feed.health_state,
                updated_feed.failure_count as u32,
                feed_id,
            );
            storage.record_refresh_attempt(&RefreshAttempt {
                feed_id: feed_id.to_owned(),
                reason,
                started_at,
                finished_at,
                http_status: Some(response.status),
                success: true,
                item_count: 0,
                error_message: None,
                next_attempt_at: Some(next_attempt_at),
            })?;

            Ok(RefreshResponse {
                feed_id: feed_id.to_owned(),
                status: "not_modified".to_owned(),
                fetched_http_status: response.status,
                item_count: 0,
                notification_count: 0,
                suppressed_notification_count: 0,
            })
        }
        FetchOutcome::Updated { response, parsed } => {
            storage.upsert_items(&parsed.items)?;
            let discovered_site_url = parsed.site_url.clone();
            let refreshed_feed_id = storage.upsert_feed(&NewFeed {
                feed_url: feed_url.clone(),
                site_url: parsed.site_url,
                title: parsed.title,
                feed_type: parsed.feed_type,
            })?;

            if let Some(site_url) = discovered_site_url {
                if storage.get_feed_icon(&refreshed_feed_id)?.is_none() {
                    if let Some(icon_url) = feed_icon_url(Some(&site_url), &feed_url) {
                        let _ = storage.set_feed_icon(&refreshed_feed_id, &icon_url);
                    }
                }
                spawn_background_icon_refresh(context.clone(), refreshed_feed_id, site_url);
            } else if storage.get_feed_icon(&refreshed_feed_id)?.is_none() {
                if let Some(icon_url) = feed_icon_url(site_url.as_ref(), &feed_url) {
                    let _ = storage.set_feed_icon(&refreshed_feed_id, &icon_url);
                }
            }

            storage.record_fetch_result(
                feed_id,
                response.status,
                response.etag.as_deref(),
                response.last_modified.as_deref(),
                response.duration_ms,
                None,
            )?;
            let notification_result =
                process_refresh_notifications(storage, feed_id, &parsed.items)?;
            let updated_feed = storage.get_feed(feed_id)?;
            let finished_at = Utc::now();
            let next_attempt_at = compute_next_refresh_at(
                finished_at,
                reason,
                updated_feed.health_state,
                updated_feed.failure_count as u32,
                feed_id,
            );
            storage.record_refresh_attempt(&RefreshAttempt {
                feed_id: feed_id.to_owned(),
                reason,
                started_at,
                finished_at,
                http_status: Some(response.status),
                success: true,
                item_count: parsed.items.len(),
                error_message: None,
                next_attempt_at: Some(next_attempt_at),
            })?;

            Ok(RefreshResponse {
                feed_id: feed_id.to_owned(),
                status: "updated".to_owned(),
                fetched_http_status: response.status,
                item_count: parsed.items.len(),
                notification_count: notification_result.sent_count,
                suppressed_notification_count: notification_result.suppressed_count,
            })
        }
    }
}

pub(crate) fn background_refresh_limit() -> usize {
    std::env::var("INFOMATRIX_BACKGROUND_REFRESH_LIMIT")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(20)
        .clamp(1, 100)
}

pub(crate) fn spawn_background_refresh_task(context: AppContext) {
    tokio::spawn(async move {
        let limit = background_refresh_limit();
        let globals = open_storage(&context)
            .ok()
            .and_then(|storage| storage.get_global_notification_settings().ok());
        let Some(settings) = globals else {
            warn!("background refresh settings unavailable");
            return;
        };
        if !settings.background_refresh_enabled {
            info!("background refresh disabled");
            return;
        }
        let interval = std::time::Duration::from_secs(
            settings.background_refresh_interval_minutes.max(1) as u64 * 60,
        );

        if let Err(err) = refresh_due_feeds_once(&context, limit, RefreshReason::CatchUp).await {
            warn!(error = %err, "background catch-up refresh failed");
        }

        let mut ticker = tokio::time::interval(interval);
        ticker.tick().await;

        loop {
            ticker.tick().await;
            match refresh_due_feeds_once(&context, limit, RefreshReason::Periodic).await {
                Ok(result) => {
                    if result.refreshed_count > 0 {
                        info!(
                            refreshed_count = result.refreshed_count,
                            total_item_count = result.total_item_count,
                            "background refresh completed"
                        );
                    }
                }
                Err(err) => {
                    warn!(error = %err, "background refresh failed");
                }
            }
        }
    });
}
