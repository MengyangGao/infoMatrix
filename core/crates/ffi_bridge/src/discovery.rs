use discovery::{DiscoveryService, ReqwestDiscoveryClient};
use serde::{Deserialize, Serialize};
use shared_api::constants::{DEFAULT_TIMEOUT_SECS, USER_AGENT};
use shared_api::labels::{feed_type_label, score_from_confidence};

use crate::TOKIO_RUNTIME;

#[derive(Debug, Deserialize)]
pub struct DiscoverInput {
    pub db_path: Option<String>,
    pub site_url: String,
}

#[derive(Debug, Serialize)]
pub struct DiscoverOutput {
    pub normalized_site_url: String,
    pub discovered_feeds: Vec<DiscoveredFeedOutput>,
    pub site_title: Option<String>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct DiscoveredFeedOutput {
    pub url: String,
    pub title: Option<String>,
    pub feed_type: String,
    pub confidence: f32,
    pub source: String,
    pub score: i32,
}

pub fn discover_site(site_url: &str, _db_path: &Option<String>) -> Result<DiscoverOutput, String> {
    let _ = _db_path;
    let client = ReqwestDiscoveryClient::new(DEFAULT_TIMEOUT_SECS, USER_AGENT)
        .map_err(|err| format!("failed to initialize discovery client: {err}"))?;
    let service = DiscoveryService::new(client);

    let runtime = &*TOKIO_RUNTIME;

    let discovered = runtime.block_on(service.discover(site_url)).map_err(|err| err.to_string())?;

    let mut discovered_feeds: Vec<DiscoveredFeedOutput> = discovered
        .discovered_feeds
        .into_iter()
        .map(|feed| {
            let score = score_from_confidence(feed.confidence);
            DiscoveredFeedOutput {
                url: feed.url.to_string(),
                title: feed.title,
                feed_type: feed_type_label(feed.feed_type).to_owned(),
                confidence: feed.confidence,
                source: format!("{:?}", feed.source).to_ascii_lowercase(),
                score,
            }
        })
        .collect();
    discovered_feeds.sort_by(|a, b| {
        b.score
            .cmp(&a.score)
            .then_with(|| b.confidence.total_cmp(&a.confidence))
            .then_with(|| a.url.cmp(&b.url))
    });

    Ok(DiscoverOutput {
        normalized_site_url: discovered.normalized_site_url.to_string(),
        discovered_feeds,
        site_title: discovered.site_title,
        warnings: discovered.warnings,
    })
}
