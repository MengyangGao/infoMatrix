use discovery::{DiscoveryService, ReqwestDiscoveryClient};
use models::NewFeed;
use serde::{Deserialize, Serialize};
use shared_api::constants::{DEFAULT_TIMEOUT_SECS, USER_AGENT};
use shared_api::feed_snapshot::{FeedSnapshotFetchMetadata, persist_parsed_feed_snapshot};
use shared_api::labels::feed_type_label;
use shared_api::url::{
    derive_site_url_from_feed, enforce_web_url, feed_icon_url, normalize_input_url,
};
use url::Url;

use crate::TOKIO_RUNTIME;
use crate::refresh::probe_direct_feed;
use crate::storage::open_storage;

#[derive(Debug, Deserialize)]
pub struct DbInput {
    pub db_path: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct AddSubscriptionInput {
    pub db_path: Option<String>,
    pub feed_url: String,
    pub title: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct SubscribeInput {
    pub db_path: Option<String>,
    pub input_url: String,
}

#[derive(Debug, Deserialize)]
pub struct CreateGroupInput {
    pub db_path: Option<String>,
    pub name: String,
}

#[derive(Debug, Deserialize)]
pub struct UpdateFeedInput {
    pub db_path: Option<String>,
    pub feed_id: String,
    pub title: Option<String>,
    pub auto_full_text: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateFeedGroupInput {
    pub db_path: Option<String>,
    pub feed_id: String,
    pub group_id: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct DeleteFeedInput {
    pub db_path: Option<String>,
    pub feed_id: String,
}

#[derive(Debug, Serialize)]
pub struct FeedOutput {
    pub id: String,
    pub title: String,
    pub feed_url: String,
    pub site_url: Option<String>,
    pub feed_type: String,
    pub auto_full_text: bool,
    pub icon_url: Option<String>,
    pub groups: Vec<FeedGroupOutput>,
}

#[derive(Debug, Serialize)]
pub struct FeedGroupOutput {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Serialize)]
pub struct SubscribeOutput {
    pub feed_id: String,
    pub resolved_feed_url: String,
    pub subscription_source: String,
}

#[derive(Debug, Serialize)]
pub struct AckOutput {
    pub ok: bool,
}

#[derive(Debug, Serialize)]
pub struct DefaultDbPathOutput {
    pub db_path: String,
}

pub fn list_feeds(db_path: &Option<String>) -> Result<Vec<FeedOutput>, String> {
    let storage = open_storage(db_path)?;
    let rows = storage.list_feeds().map_err(|err| err.to_string())?;
    let feeds: Vec<FeedOutput> = rows
        .into_iter()
        .map(|feed| {
            let groups = storage
                .list_groups_for_feed(&feed.id)
                .map_err(|err| err.to_string())?
                .into_iter()
                .map(|group| FeedGroupOutput { id: group.id, name: group.name })
                .collect();
            let icon_url = storage
                .get_feed_icon(&feed.id)
                .map_err(|err| err.to_string())?
                .map(|icon| icon.source_url)
                .or_else(|| feed_icon_url(feed.site_url.as_ref(), &feed.feed_url));
            Ok(FeedOutput {
                id: feed.id,
                title: feed
                    .title
                    .unwrap_or_else(|| feed.feed_url.host_str().unwrap_or("Untitled").to_owned()),
                feed_url: feed.feed_url.to_string(),
                site_url: feed.site_url.map(|value| value.to_string()),
                feed_type: feed_type_label(feed.feed_type).to_owned(),
                auto_full_text: feed.auto_full_text,
                icon_url,
                groups,
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    Ok(feeds)
}

pub fn list_groups(db_path: &Option<String>) -> Result<Vec<FeedGroupOutput>, String> {
    let storage = open_storage(db_path)?;
    let groups = storage
        .list_groups()
        .map_err(|err| err.to_string())?
        .into_iter()
        .map(|group| FeedGroupOutput { id: group.id, name: group.name })
        .collect::<Vec<_>>();
    Ok(groups)
}

pub fn create_group(db_path: &Option<String>, name: &str) -> Result<FeedGroupOutput, String> {
    let storage = open_storage(db_path)?;
    let group = storage.create_group(name).map_err(|err| err.to_string())?;
    Ok(FeedGroupOutput { id: group.id, name: group.name })
}

pub fn update_feed(
    db_path: &Option<String>,
    feed_id: &str,
    title: Option<&str>,
    auto_full_text: Option<bool>,
) -> Result<AckOutput, String> {
    let storage = open_storage(db_path)?;
    if let Some(title) = title {
        storage.update_feed_title(feed_id, Some(title)).map_err(|err| err.to_string())?;
    }
    if let Some(auto_full_text) = auto_full_text {
        storage
            .update_feed_auto_full_text(feed_id, auto_full_text)
            .map_err(|err| err.to_string())?;
    }
    Ok(AckOutput { ok: true })
}

pub fn update_feed_group(
    db_path: &Option<String>,
    feed_id: &str,
    group_id: Option<&str>,
) -> Result<AckOutput, String> {
    let mut storage = open_storage(db_path)?;
    storage.set_feed_group(feed_id, group_id).map_err(|err| err.to_string())?;
    Ok(AckOutput { ok: true })
}

pub fn delete_feed(db_path: &Option<String>, feed_id: &str) -> Result<AckOutput, String> {
    let storage = open_storage(db_path)?;
    storage.delete_feed(feed_id).map_err(|err| err.to_string())?;
    Ok(AckOutput { ok: true })
}

pub fn add_subscription(
    db_path: &Option<String>,
    feed_url: &str,
    title: Option<String>,
) -> Result<String, String> {
    let feed_url = Url::parse(feed_url).map_err(|err| format!("invalid feed url: {err}"))?;
    enforce_web_url(&feed_url, "feed url").map_err(|err| err.to_string())?;
    let title_fallback = title;
    let runtime = &*TOKIO_RUNTIME;

    if let Some(direct_probe) = probe_direct_feed(runtime, &feed_url)? {
        let mut storage = open_storage(db_path)?;
        let site_url_for_icon = direct_probe
            .parsed
            .site_url
            .clone()
            .or_else(|| derive_site_url_from_feed(&direct_probe.response.final_url));
        let feed_id = persist_parsed_feed_snapshot(
            &mut storage,
            &direct_probe.response.final_url,
            title_fallback.clone(),
            site_url_for_icon.clone(),
            &direct_probe.parsed,
            FeedSnapshotFetchMetadata {
                http_status: direct_probe.response.status,
                etag: direct_probe.response.etag,
                last_modified: direct_probe.response.last_modified,
                duration_ms: direct_probe.response.duration_ms,
            },
        )
        .map_err(|err| err.to_string())?;
        if let Some(site_url) = site_url_for_icon.as_ref() {
            if let Some(icon_url) = feed_icon_url(Some(site_url), &direct_probe.response.final_url)
            {
                let _ = storage.set_feed_icon(&feed_id, &icon_url);
            }
        }
        return Ok(feed_id);
    }

    Err("direct feed URL did not resolve to a valid feed".to_owned())
}

pub fn subscribe_input(
    db_path: &Option<String>,
    input_url: &str,
) -> Result<SubscribeOutput, String> {
    let normalized_input = normalize_input_url(input_url).map_err(|err| err.to_string())?;
    let parsed_input =
        Url::parse(&normalized_input).map_err(|err| format!("invalid input url: {err}"))?;
    enforce_web_url(&parsed_input, "input url").map_err(|err| err.to_string())?;

    let runtime = &*TOKIO_RUNTIME;

    if let Some(direct_probe) = probe_direct_feed(runtime, &parsed_input)? {
        let mut storage = open_storage(db_path)?;
        let site_url_fallback = derive_site_url_from_feed(&direct_probe.response.final_url);
        let feed_id = persist_parsed_feed_snapshot(
            &mut storage,
            &direct_probe.response.final_url,
            direct_probe.parsed.title.clone(),
            site_url_fallback,
            &direct_probe.parsed,
            FeedSnapshotFetchMetadata {
                http_status: direct_probe.response.status,
                etag: direct_probe.response.etag,
                last_modified: direct_probe.response.last_modified,
                duration_ms: direct_probe.response.duration_ms,
            },
        )
        .map_err(|err| err.to_string())?;

        return Ok(SubscribeOutput {
            feed_id,
            resolved_feed_url: direct_probe.response.final_url.to_string(),
            subscription_source: "direct_feed".to_owned(),
        });
    }

    let discovery_client = ReqwestDiscoveryClient::new(DEFAULT_TIMEOUT_SECS, USER_AGENT)
        .map_err(|err| format!("failed to initialize discovery client: {err}"))?;
    let discovery_service = DiscoveryService::new(discovery_client);
    let discovered = runtime
        .block_on(discovery_service.discover(&normalized_input))
        .map_err(|err| err.to_string())?;
    let candidate = discovered
        .discovered_feeds
        .first()
        .ok_or_else(|| "could not discover a valid feed from input url".to_owned())?;
    enforce_web_url(&candidate.url, "discovered feed url").map_err(|err| err.to_string())?;

    let storage = open_storage(db_path)?;
    let feed_id = storage
        .upsert_feed(&NewFeed {
            feed_url: candidate.url.clone(),
            site_url: Some(discovered.normalized_site_url.clone()),
            title: candidate.title.clone(),
            feed_type: candidate.feed_type,
        })
        .map_err(|err| err.to_string())?;

    Ok(SubscribeOutput {
        feed_id,
        resolved_feed_url: candidate.url.to_string(),
        subscription_source: "discovery".to_owned(),
    })
}
