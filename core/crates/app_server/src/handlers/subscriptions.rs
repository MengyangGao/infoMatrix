use axum::Json;
use axum::extract::State;
use chrono::{Duration, Utc};
use models::{FeedType, NewFeed};
use parser::parse_feed;
use shared_api::feed_snapshot::{FeedSnapshotFetchMetadata, persist_parsed_feed_snapshot};
use shared_api::url::{
    derive_site_url_from_feed, enforce_web_url, feed_icon_url, normalize_input_url,
};
use url::Url;

use crate::config::AppContext;
use crate::error::ApiError;
use crate::services::discovery::{
    DISCOVERY_CACHE_TTL_HOURS, cached_discovery_candidate_for_feed_url, candidate_site_urls,
    discover_site_with_client, discover_site_with_reqwest, fetch_first_successful_response,
};
use crate::services::icon::{discover_and_cache_site_icon, spawn_background_icon_refresh};
use crate::state::{build_http_client, open_storage};
use crate::views::{
    AddSubscriptionRequest, AddSubscriptionResponse, DiscoverRequest, DiscoverResponse,
    UnifiedSubscribeRequest, UnifiedSubscribeResponse,
};

#[axum::debug_handler]
pub(crate) async fn discover_site(
    State(context): State<AppContext>,
    Json(payload): Json<DiscoverRequest>,
) -> Result<Json<DiscoverResponse>, ApiError> {
    let normalized_input_site_url = normalize_input_url(&payload.site_url)?;
    let run = discover_site_with_reqwest(&context, &payload.site_url).await?;
    let storage = open_storage(&context)?;
    let cache_json =
        serde_json::to_string(&run.cache).map_err(|err| ApiError::Internal(err.to_string()))?;
    let expires_at = (Utc::now() + Duration::hours(DISCOVERY_CACHE_TTL_HOURS)).to_rfc3339();
    storage.upsert_discovery_cache(
        normalized_input_site_url.as_str(),
        &run.cache.normalized_site_url,
        &cache_json,
        Some(expires_at.as_str()),
    )?;
    let response = run.response;
    Ok(Json(response))
}

pub(crate) async fn add_subscription(
    State(context): State<AppContext>,
    Json(payload): Json<AddSubscriptionRequest>,
) -> Result<Json<AddSubscriptionResponse>, ApiError> {
    let feed_url = Url::parse(&payload.feed_url)
        .map_err(|err| ApiError::BadRequest(format!("invalid feed url: {err}")))?;
    enforce_web_url(&feed_url, "feed url")?;
    let title_fallback = payload.title.clone();
    let probe_start = std::time::Instant::now();
    let direct_probe = {
        let client = build_http_client(&context)?;
        match client.get(feed_url.clone()).send().await {
            Ok(response) if response.status().is_success() => {
                let probe_url = response.url().clone();
                let probe_status = response.status().as_u16();
                let probe_content_type = response
                    .headers()
                    .get(reqwest::header::CONTENT_TYPE)
                    .and_then(|value| value.to_str().ok())
                    .map(ToOwned::to_owned);
                let probe_etag = response
                    .headers()
                    .get(reqwest::header::ETAG)
                    .and_then(|value| value.to_str().ok())
                    .map(ToOwned::to_owned);
                let probe_last_modified = response
                    .headers()
                    .get(reqwest::header::LAST_MODIFIED)
                    .and_then(|value| value.to_str().ok())
                    .map(ToOwned::to_owned);
                let probe_bytes =
                    response.bytes().await.map_err(|err| ApiError::Internal(err.to_string()))?;
                parse_feed("add_subscription", &probe_bytes, probe_content_type.as_deref())
                    .ok()
                    .and_then(|parsed| {
                        if parsed.feed_type == FeedType::Unknown {
                            None
                        } else {
                            Some((probe_url, probe_status, probe_etag, probe_last_modified, parsed))
                        }
                    })
            }
            _ => None,
        }
    };

    if let Some((probe_url, probe_status, probe_etag, probe_last_modified, parsed)) = direct_probe {
        let site_url_for_icon =
            parsed.site_url.clone().or_else(|| derive_site_url_from_feed(&probe_url));
        let mut storage = open_storage(&context)?;
        let feed_id = persist_parsed_feed_snapshot(
            &mut storage,
            &probe_url,
            title_fallback.clone(),
            site_url_for_icon.clone(),
            &parsed,
            FeedSnapshotFetchMetadata {
                http_status: probe_status,
                etag: probe_etag,
                last_modified: probe_last_modified,
                duration_ms: probe_start.elapsed().as_millis(),
            },
        )?;
        if let Some(site_url) = site_url_for_icon {
            if let Some(icon_url) = feed_icon_url(Some(&site_url), &probe_url) {
                let _ = storage.set_feed_icon(&feed_id, &icon_url);
            }
            spawn_background_icon_refresh(context.clone(), feed_id.clone(), site_url);
        }
        return Ok(Json(AddSubscriptionResponse { feed_id }));
    }

    Err(ApiError::BadRequest("direct feed URL did not resolve to a valid feed".to_owned()))
}

pub(crate) async fn subscribe_input(
    State(context): State<AppContext>,
    Json(payload): Json<UnifiedSubscribeRequest>,
) -> Result<Json<UnifiedSubscribeResponse>, ApiError> {
    let normalized_input = normalize_input_url(&payload.input_url)?;
    let normalized = Url::parse(&normalized_input)
        .map_err(|err| ApiError::BadRequest(format!("invalid input url: {err}")))?;
    enforce_web_url(&normalized, "input url")?;
    let client = build_http_client(&context)?;
    let probe_candidates = candidate_site_urls(&normalized);
    let probe_start = std::time::Instant::now();

    let discovered_seed_url =
        match fetch_first_successful_response(&client, &probe_candidates).await {
            Ok(probe) => {
                let probe_url = probe.url().clone();
                let probe_status = probe.status().as_u16();
                let probe_content_type = probe
                    .headers()
                    .get(reqwest::header::CONTENT_TYPE)
                    .and_then(|value| value.to_str().ok())
                    .map(ToOwned::to_owned);
                let probe_etag = probe
                    .headers()
                    .get(reqwest::header::ETAG)
                    .and_then(|value| value.to_str().ok())
                    .map(ToOwned::to_owned);
                let probe_last_modified = probe
                    .headers()
                    .get(reqwest::header::LAST_MODIFIED)
                    .and_then(|value| value.to_str().ok())
                    .map(ToOwned::to_owned);
                let probe_bytes =
                    probe.bytes().await.map_err(|err| ApiError::Internal(err.to_string()))?;
                let parsed_direct =
                    parse_feed("direct_probe", &probe_bytes, probe_content_type.as_deref()).ok();
                if let Some(parsed) = parsed_direct {
                    let site_url_for_icon =
                        parsed.site_url.clone().or_else(|| derive_site_url_from_feed(&probe_url));
                    let mut storage = open_storage(&context)?;
                    let duration_ms = probe_start.elapsed().as_millis();
                    let feed_id = persist_parsed_feed_snapshot(
                        &mut storage,
                        &probe_url,
                        parsed.title.clone(),
                        site_url_for_icon.clone(),
                        &parsed,
                        FeedSnapshotFetchMetadata {
                            http_status: probe_status,
                            etag: probe_etag,
                            last_modified: probe_last_modified,
                            duration_ms,
                        },
                    )?;
                    if let Some(site_url) = site_url_for_icon {
                        if let Some(icon_url) = feed_icon_url(Some(&site_url), &probe_url) {
                            let _ = storage.set_feed_icon(&feed_id, &icon_url);
                        }
                        spawn_background_icon_refresh(context.clone(), feed_id.clone(), site_url);
                    }
                    return Ok(Json(UnifiedSubscribeResponse {
                        feed_id,
                        resolved_feed_url: probe_url.to_string(),
                        subscription_source: "direct_feed".to_owned(),
                    }));
                }
                probe_url.to_string()
            }
            Err(_) => normalized.to_string(),
        };

    let discovered = discover_site_with_client(&client, discovered_seed_url.as_ref()).await?;
    let candidate = discovered
        .response
        .discovered_feeds
        .iter()
        .max_by(|a, b| a.score.cmp(&b.score).then_with(|| a.confidence.total_cmp(&b.confidence)))
        .ok_or_else(|| {
            ApiError::BadRequest("could not discover a valid feed from input url".to_owned())
        })?;
    let feed_url = Url::parse(&candidate.url)
        .map_err(|err| ApiError::Internal(format!("discovered invalid feed url: {err}")))?;
    enforce_web_url(&feed_url, "discovered feed url")?;

    let mut storage = open_storage(&context)?;
    let site_url = Url::parse(&discovered.response.normalized_site_url).ok();
    if let Some(cache_candidate) =
        cached_discovery_candidate_for_feed_url(&discovered.cache, &feed_url)
    {
        let feed_id = persist_parsed_feed_snapshot(
            &mut storage,
            &feed_url,
            candidate.title.clone(),
            site_url.clone(),
            &cache_candidate.parsed_feed,
            FeedSnapshotFetchMetadata {
                http_status: cache_candidate.http_status,
                etag: cache_candidate.etag.clone(),
                last_modified: cache_candidate.last_modified.clone(),
                duration_ms: cache_candidate.duration_ms,
            },
        )?;
        if let Some(site_url) = site_url.clone() {
            if let Some(icon_url) = feed_icon_url(Some(&site_url), &feed_url) {
                let _ = storage.set_feed_icon(&feed_id, &icon_url);
            }
            spawn_background_icon_refresh(context.clone(), feed_id.clone(), site_url);
        }
        return Ok(Json(UnifiedSubscribeResponse {
            feed_id,
            resolved_feed_url: feed_url.to_string(),
            subscription_source: "discovery".to_owned(),
        }));
    }

    let feed_id = storage.upsert_feed(&NewFeed {
        feed_url: feed_url.clone(),
        site_url: site_url.clone(),
        title: candidate.title.clone(),
        feed_type: candidate.feed_type,
    })?;
    if let Some(site_url) = site_url {
        if discover_and_cache_site_icon(&context, &mut storage, &feed_id, &site_url).await.is_none()
        {
            if let Some(icon_url) = feed_icon_url(Some(&site_url), &feed_url) {
                let _ = storage.set_feed_icon(&feed_id, &icon_url);
            }
        }
    }

    Ok(Json(UnifiedSubscribeResponse {
        feed_id,
        resolved_feed_url: feed_url.to_string(),
        subscription_source: "discovery".to_owned(),
    }))
}
