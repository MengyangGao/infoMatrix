use std::collections::HashMap;

use app_core::InfoMatrixAppCore;
use async_trait::async_trait;
use discovery::{DiscoveryError, DiscoveryHttpClient, DiscoveryService, HttpResponse};
use fetcher::{FeedHttpClient, FetchRequest, FetchResponse, FetchService};
use models::ItemStatePatch;

struct MockDiscoveryClient {
    responses: HashMap<String, HttpResponse>,
}

#[async_trait]
impl DiscoveryHttpClient for MockDiscoveryClient {
    async fn get(&self, url: &url::Url) -> Result<HttpResponse, DiscoveryError> {
        self.responses
            .get(url.as_str())
            .cloned()
            .ok_or_else(|| DiscoveryError::Network(format!("missing {}", url)))
    }
}

struct MockFetchClient {
    response: FetchResponse,
}

#[async_trait]
impl FeedHttpClient for MockFetchClient {
    async fn fetch(&self, _request: &FetchRequest) -> Result<FetchResponse, fetcher::FetchError> {
        Ok(self.response.clone())
    }
}

#[tokio::test]
async fn discovery_pipeline_finds_feed_and_persists_items() {
    let storage = storage::Storage::open_in_memory().expect("open");
    storage.migrate().expect("migrate");
    let mut app = InfoMatrixAppCore::new(storage);

    let site_url = url::Url::parse("https://example.com/").expect("url parse");
    let feed_url = url::Url::parse("https://example.com/feed.xml").expect("url parse");

    let html = std::fs::read_to_string(
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../tests/discovery/site_multiple_feeds.html"),
    )
    .expect("fixture html");

    let rss = std::fs::read(
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../tests/feeds/valid_rss.xml"),
    )
    .expect("fixture rss");

    let discovery_service = DiscoveryService::new(MockDiscoveryClient {
        responses: HashMap::from([
            (
                site_url.to_string(),
                HttpResponse {
                    final_url: site_url.clone(),
                    status: 200,
                    content_type: Some("text/html".to_owned()),
                    body: html.as_bytes().to_vec(),
                },
            ),
            (
                feed_url.to_string(),
                HttpResponse {
                    final_url: feed_url.clone(),
                    status: 200,
                    content_type: Some("application/rss+xml".to_owned()),
                    body: rss.clone(),
                },
            ),
            (
                "https://example.com/atom.xml".to_owned(),
                HttpResponse {
                    final_url: url::Url::parse("https://example.com/atom.xml").expect("url parse"),
                    status: 404,
                    content_type: Some("text/plain".to_owned()),
                    body: b"not found".to_vec(),
                },
            ),
        ]),
    });

    let discovered =
        app.discover_site(&discovery_service, "https://example.com").await.expect("discover");
    assert!(!discovered.discovered_feeds.is_empty());

    let feed_id = app
        .subscribe_discovered_feed(&discovered.discovered_feeds[0], Some(site_url))
        .expect("subscribe");

    let fetch_service = FetchService::new(MockFetchClient {
        response: FetchResponse {
            final_url: feed_url.clone(),
            status: 200,
            body: rss,
            content_type: Some("application/rss+xml".to_owned()),
            etag: Some("etag-2".to_owned()),
            last_modified: None,
            duration_ms: 30,
        },
    });

    let request = FetchRequest { url: feed_url, etag: None, last_modified: None };

    app.refresh_feed(&fetch_service, &feed_id, &request).await.expect("refresh");

    let item_ids = app.storage.list_item_ids_for_feed(&feed_id, 10).expect("list ids");
    assert_eq!(item_ids.len(), 2);

    let updated = app
        .storage
        .patch_item_state(
            &item_ids[0],
            &ItemStatePatch {
                is_read: Some(true),
                is_starred: Some(true),
                is_saved_for_later: Some(true),
                is_archived: Some(false),
            },
        )
        .expect("patch state");

    assert!(updated.is_read);
    assert!(updated.is_starred);
    assert!(updated.is_saved_for_later);
}
