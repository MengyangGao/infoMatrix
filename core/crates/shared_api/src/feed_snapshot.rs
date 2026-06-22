use models::{NewFeed, NormalizedItem};
use parser::ParsedFeed;
use storage::Storage;
use url::Url;

use crate::SharedApiError;

#[derive(Debug, Clone)]
pub struct FeedSnapshotFetchMetadata {
    pub http_status: u16,
    pub etag: Option<String>,
    pub last_modified: Option<String>,
    pub duration_ms: u128,
}

pub fn rebind_items_to_feed(feed_id: &str, items: &[NormalizedItem]) -> Vec<NormalizedItem> {
    items
        .iter()
        .cloned()
        .map(|mut item| {
            item.source_feed_id = feed_id.to_owned();
            item
        })
        .collect()
}

pub fn persist_parsed_feed_snapshot(
    storage: &mut Storage,
    feed_url: &Url,
    title_fallback: Option<String>,
    site_url_fallback: Option<Url>,
    parsed_feed: &ParsedFeed,
    fetch_metadata: FeedSnapshotFetchMetadata,
) -> Result<String, SharedApiError> {
    let feed_id = storage.upsert_feed(&NewFeed {
        feed_url: feed_url.clone(),
        site_url: parsed_feed.site_url.clone().or(site_url_fallback),
        title: parsed_feed.title.clone().or(title_fallback),
        feed_type: parsed_feed.feed_type,
    })?;

    let rebased_items = rebind_items_to_feed(&feed_id, &parsed_feed.items);
    storage.upsert_items(&rebased_items)?;
    storage.record_fetch_result(
        &feed_id,
        fetch_metadata.http_status,
        fetch_metadata.etag.as_deref(),
        fetch_metadata.last_modified.as_deref(),
        fetch_metadata.duration_ms,
        None,
    )?;
    Ok(feed_id)
}
