//! App-core orchestration layer joining discovery, fetching, parsing, and storage.

use discovery::{DiscoveryHttpClient, DiscoveryService};
use fetcher::{FeedHttpClient, FetchOutcome, FetchRequest, FetchService};
use models::{
    DiscoveryResult, EntryKind, FeedType, ItemScopeCounts, ItemState, ItemStatePatch, NewEntry,
    NewFeed,
};
use storage::{
    FeedGroupRow, FeedRow, ItemDetailRow, ItemListFilter, ItemSummaryRow, Storage, StorageError,
};
use thiserror::Error;
use url::Url;
use uuid::Uuid;

/// App-core errors.
#[derive(Debug, Error)]
pub enum AppCoreError {
    /// Discovery layer error.
    #[error("discovery error: {0}")]
    Discovery(String),
    /// Fetch layer error.
    #[error("fetch error: {0}")]
    Fetch(String),
    /// Storage layer error.
    #[error("storage error: {0}")]
    Storage(#[from] StorageError),
    /// URL parse failure.
    #[error("invalid url: {0}")]
    InvalidUrl(String),
}

/// Main app-core service.
pub struct InfoMatrixAppCore {
    /// Shared storage handle.
    pub storage: Storage,
}

impl InfoMatrixAppCore {
    /// Create app core from initialized storage.
    pub fn new(storage: Storage) -> Self {
        Self { storage }
    }

    /// Add direct feed subscription by URL.
    pub async fn add_subscription_by_feed_url<C: FeedHttpClient>(
        &mut self,
        service: &FetchService<C>,
        feed_url: &str,
    ) -> Result<String, AppCoreError> {
        let url = Url::parse(feed_url).map_err(|err| AppCoreError::InvalidUrl(err.to_string()))?;
        let request = FetchRequest { url: url.clone(), etag: None, last_modified: None };

        if let Ok(FetchOutcome::Updated { response, parsed }) =
            service.fetch_and_parse("direct_subscription", &request).await
        {
            if parsed.feed_type != FeedType::Unknown {
                let feed_id = self.storage.upsert_feed(&NewFeed {
                    feed_url: response.final_url.clone(),
                    site_url: parsed.site_url.clone(),
                    title: parsed.title.clone(),
                    feed_type: parsed.feed_type,
                })?;
                let rebased_items: Vec<models::NormalizedItem> = parsed
                    .items
                    .into_iter()
                    .map(|mut item| {
                        item.source_feed_id = feed_id.clone();
                        item
                    })
                    .collect();
                self.storage.upsert_items(&rebased_items)?;
                self.storage.record_fetch_result(
                    &feed_id,
                    response.status,
                    response.etag.as_deref(),
                    response.last_modified.as_deref(),
                    response.duration_ms,
                    None,
                )?;
                return Ok(feed_id);
            }
        }

        let id = self.storage.upsert_feed(&NewFeed {
            feed_url: url,
            site_url: None,
            title: None,
            feed_type: FeedType::Unknown,
        })?;
        Ok(id)
    }

    /// Discover feeds from site URL.
    pub async fn discover_site<C: DiscoveryHttpClient>(
        &self,
        service: &DiscoveryService<C>,
        site_url: &str,
    ) -> Result<DiscoveryResult, AppCoreError> {
        service.discover(site_url).await.map_err(|err| AppCoreError::Discovery(err.to_string()))
    }

    /// Subscribe to selected discovered feed candidate.
    pub fn subscribe_discovered_feed(
        &self,
        discovered: &models::DiscoveredFeed,
        site_url: Option<Url>,
    ) -> Result<String, AppCoreError> {
        self.storage
            .upsert_feed(&NewFeed {
                feed_url: discovered.url.clone(),
                site_url,
                title: discovered.title.clone(),
                feed_type: discovered.feed_type,
            })
            .map_err(Into::into)
    }

    /// List subscriptions in creation order.
    pub fn list_feeds(&self) -> Result<Vec<FeedRow>, AppCoreError> {
        self.storage.list_feeds().map_err(Into::into)
    }

    /// List feed groups.
    pub fn list_groups(&self) -> Result<Vec<FeedGroupRow>, AppCoreError> {
        self.storage.list_groups().map_err(Into::into)
    }

    /// Create or reuse a feed group.
    pub fn create_group(&self, name: &str) -> Result<FeedGroupRow, AppCoreError> {
        self.storage.create_group(name).map_err(Into::into)
    }

    /// List items for one feed.
    pub fn list_items_for_feed(
        &self,
        feed_id: &str,
        limit: usize,
        search_query: Option<&str>,
    ) -> Result<Vec<ItemSummaryRow>, AppCoreError> {
        self.storage.list_items_for_feed(feed_id, limit, search_query).map_err(Into::into)
    }

    /// Search items within one feed.
    pub fn search_items_for_feed(
        &self,
        feed_id: &str,
        limit: usize,
        search_query: Option<&str>,
    ) -> Result<Vec<ItemSummaryRow>, AppCoreError> {
        self.list_items_for_feed(feed_id, limit, search_query)
    }

    /// List items across all feeds.
    pub fn list_all_items(
        &self,
        limit: usize,
        search_query: Option<&str>,
        filter: ItemListFilter,
        kind: Option<EntryKind>,
    ) -> Result<Vec<ItemSummaryRow>, AppCoreError> {
        match kind {
            Some(kind) => self
                .storage
                .list_items_global_with_kind(limit, search_query, filter, Some(kind))
                .map_err(Into::into),
            None => self.storage.list_items_global(limit, search_query, filter).map_err(Into::into),
        }
    }

    /// Search items across all feeds.
    pub fn search_all_items(
        &self,
        limit: usize,
        search_query: Option<&str>,
        filter: ItemListFilter,
        kind: Option<EntryKind>,
    ) -> Result<Vec<ItemSummaryRow>, AppCoreError> {
        self.list_all_items(limit, search_query, filter, kind)
    }

    /// Count items in the core inbox scopes.
    pub fn item_counts(&self) -> Result<ItemScopeCounts, AppCoreError> {
        self.storage.count_item_scopes().map_err(Into::into)
    }

    /// Read one item detail payload.
    pub fn item_detail(&self, item_id: &str) -> Result<ItemDetailRow, AppCoreError> {
        self.storage.get_item_detail(item_id).map_err(Into::into)
    }

    /// Patch one item state row.
    pub fn patch_item_state(
        &self,
        item_id: &str,
        patch: &ItemStatePatch,
    ) -> Result<ItemState, AppCoreError> {
        self.storage.patch_item_state(item_id, patch).map_err(Into::into)
    }

    /// Persist a unified entry and return its stable id.
    pub fn create_entry(&mut self, mut entry: NewEntry) -> Result<String, AppCoreError> {
        let entry_id = entry.id.clone().unwrap_or_else(|| Uuid::now_v7().to_string());
        entry.id = Some(entry_id.clone());
        self.storage.upsert_entries(&[entry])?;
        Ok(entry_id)
    }

    /// Fetch feed and persist parsed items when updated.
    pub async fn refresh_feed<C: FeedHttpClient>(
        &mut self,
        service: &FetchService<C>,
        source_feed_id: &str,
        request: &FetchRequest,
    ) -> Result<FetchOutcome, AppCoreError> {
        let current_feed = self.storage.get_feed(source_feed_id)?;
        let outcome = service
            .fetch_and_parse(source_feed_id, request)
            .await
            .map_err(|err| AppCoreError::Fetch(err.to_string()))?;

        match &outcome {
            FetchOutcome::NotModified(response) => {
                self.storage.record_fetch_result(
                    source_feed_id,
                    response.status,
                    response.etag.as_deref(),
                    response.last_modified.as_deref(),
                    response.duration_ms,
                    None,
                )?;
            }
            FetchOutcome::Updated { response, parsed } => {
                self.storage.upsert_items(&parsed.items)?;
                self.storage.upsert_feed(&NewFeed {
                    feed_url: current_feed.feed_url,
                    site_url: parsed.site_url.clone(),
                    title: parsed.title.clone(),
                    feed_type: parsed.feed_type,
                })?;
                self.storage.record_fetch_result(
                    source_feed_id,
                    response.status,
                    response.etag.as_deref(),
                    response.last_modified.as_deref(),
                    response.duration_ms,
                    None,
                )?;
            }
        }

        Ok(outcome)
    }

    /// Mark item archived/unarchived.
    pub fn set_item_archived(
        &self,
        item_id: &str,
        is_archived: bool,
    ) -> Result<ItemState, AppCoreError> {
        self.storage
            .patch_item_state(
                item_id,
                &ItemStatePatch { is_archived: Some(is_archived), ..ItemStatePatch::default() },
            )
            .map_err(Into::into)
    }

    /// Mark item read/unread.
    pub fn set_item_read(&self, item_id: &str, is_read: bool) -> Result<ItemState, AppCoreError> {
        self.storage
            .patch_item_state(
                item_id,
                &ItemStatePatch { is_read: Some(is_read), ..ItemStatePatch::default() },
            )
            .map_err(Into::into)
    }

    /// Mark item starred/unstarred.
    pub fn set_item_starred(
        &self,
        item_id: &str,
        is_starred: bool,
    ) -> Result<ItemState, AppCoreError> {
        self.storage
            .patch_item_state(
                item_id,
                &ItemStatePatch { is_starred: Some(is_starred), ..ItemStatePatch::default() },
            )
            .map_err(Into::into)
    }

    /// Mark item save-for-later on/off.
    pub fn set_item_saved_for_later(
        &self,
        item_id: &str,
        is_saved_for_later: bool,
    ) -> Result<ItemState, AppCoreError> {
        self.storage
            .patch_item_state(
                item_id,
                &ItemStatePatch {
                    is_saved_for_later: Some(is_saved_for_later),
                    ..ItemStatePatch::default()
                },
            )
            .map_err(Into::into)
    }

    /// Rename a feed if it exists.
    pub fn rename_feed(&self, feed_id: &str, title: Option<&str>) -> Result<(), AppCoreError> {
        self.storage.update_feed_title(feed_id, title).map_err(Into::into)
    }

    /// Toggle automatic full-text fetching for a feed.
    pub fn set_feed_auto_full_text(
        &self,
        feed_id: &str,
        enabled: bool,
    ) -> Result<(), AppCoreError> {
        self.storage.update_feed_auto_full_text(feed_id, enabled).map_err(Into::into)
    }

    /// Assign a feed to one group or clear the assignment.
    pub fn set_feed_group(
        &mut self,
        feed_id: &str,
        group_id: Option<&str>,
    ) -> Result<(), AppCoreError> {
        self.storage.set_feed_group(feed_id, group_id).map_err(Into::into)
    }

    /// Delete a feed subscription.
    pub fn delete_feed(&self, feed_id: &str) -> Result<(), AppCoreError> {
        self.storage.delete_feed(feed_id).map_err(Into::into)
    }
}

#[cfg(test)]
mod tests {
    use async_trait::async_trait;
    use fetcher::{
        FeedHttpClient, FetchError, FetchOutcome, FetchRequest, FetchResponse, FetchService,
    };
    use models::FeedHealthState;
    use models::{EntryKind, EntrySource, EntrySourceKind, FeedType, ItemStatePatch, NewEntry};
    use storage::ItemListFilter;
    use storage::Storage;

    use super::InfoMatrixAppCore;

    struct MockFetchClient {
        response: FetchResponse,
    }

    #[async_trait]
    impl FeedHttpClient for MockFetchClient {
        async fn fetch(&self, _request: &FetchRequest) -> Result<FetchResponse, FetchError> {
            Ok(self.response.clone())
        }
    }

    #[tokio::test]
    async fn subscribes_direct_feed() {
        let storage = Storage::open_in_memory().expect("open");
        storage.migrate().expect("migrate");

        let mut app = InfoMatrixAppCore::new(storage);
        let response = FetchResponse {
            final_url: url::Url::parse("https://example.com/feed.xml").expect("url parse"),
            status: 200,
            body: std::fs::read(
                std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("../../../tests/feeds/valid_rss.xml"),
            )
            .expect("fixture rss"),
            content_type: Some("application/rss+xml".to_owned()),
            etag: Some("etag-1".to_owned()),
            last_modified: Some("Tue, 04 Mar 2025 12:00:00 GMT".to_owned()),
            duration_ms: 12,
        };
        let id = app
            .add_subscription_by_feed_url(
                &FetchService::new(MockFetchClient { response }),
                "https://example.com/feed.xml",
            )
            .await
            .expect("subscribe");
        assert!(!id.is_empty());

        let feed = app.storage.get_feed(&id).expect("feed");
        assert_eq!(feed.feed_type, FeedType::Rss);
        assert_eq!(feed.title.as_deref(), Some("Fixture RSS Feed"));
        assert!(feed.auto_full_text);
        let items = app.storage.list_items_for_feed(&id, 10, None).expect("items");
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].title, "RSS Item Two");
        assert_eq!(items[1].title, "RSS Item One");
    }

    #[test]
    fn creates_entries_and_lists_counts() {
        let storage = Storage::open_in_memory().expect("open");
        storage.migrate().expect("migrate");

        let mut app = InfoMatrixAppCore::new(storage);
        let entry_id = app
            .create_entry(NewEntry {
                id: None,
                kind: EntryKind::Note,
                source: EntrySource {
                    source_kind: EntrySourceKind::Manual,
                    source_id: None,
                    source_url: None,
                    source_title: Some("manual".to_owned()),
                },
                external_item_id: None,
                canonical_url: None,
                title: "Notebook".to_owned(),
                author: None,
                summary: Some("Quick memo".to_owned()),
                content_html: None,
                content_text: Some("Quick memo".to_owned()),
                published_at: None,
                updated_at: None,
                raw_hash: "memo-1".to_owned(),
                dedup_reason: None,
                duplicate_of_entry_id: None,
            })
            .expect("create entry");

        assert!(!entry_id.is_empty());

        let counts = app.item_counts().expect("counts");
        assert_eq!(counts.notes, 1);
        assert_eq!(counts.all, 1);

        let items = app
            .list_all_items(10, None, ItemListFilter::All, Some(EntryKind::Note))
            .expect("list items");
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].title, "Notebook");
    }

    #[tokio::test]
    async fn refresh_feed_records_fetch_metadata() {
        let storage = Storage::open_in_memory().expect("open");
        storage.migrate().expect("migrate");

        let mut app = InfoMatrixAppCore::new(storage);
        let response = FetchResponse {
            final_url: url::Url::parse("https://example.com/feed.xml").expect("url parse"),
            status: 200,
            body: std::fs::read(
                std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("../../../tests/feeds/valid_rss.xml"),
            )
            .expect("fixture rss"),
            content_type: Some("application/rss+xml".to_owned()),
            etag: Some("etag-1".to_owned()),
            last_modified: Some("Tue, 04 Mar 2025 12:00:00 GMT".to_owned()),
            duration_ms: 17,
        };
        let feed_id = app
            .add_subscription_by_feed_url(
                &FetchService::new(MockFetchClient { response }),
                "https://example.com/feed.xml",
            )
            .await
            .expect("subscribe");

        let request = FetchRequest {
            url: url::Url::parse("https://example.com/feed.xml").expect("url parse"),
            etag: None,
            last_modified: None,
        };
        let response = FetchResponse {
            final_url: request.url.clone(),
            status: 200,
            body: std::fs::read(
                std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("../../../tests/feeds/valid_rss.xml"),
            )
            .expect("fixture rss"),
            content_type: Some("application/rss+xml".to_owned()),
            etag: Some("etag-1".to_owned()),
            last_modified: Some("Tue, 04 Mar 2025 12:00:00 GMT".to_owned()),
            duration_ms: 17,
        };

        let outcome = app
            .refresh_feed(&FetchService::new(MockFetchClient { response }), &feed_id, &request)
            .await
            .expect("refresh");
        assert!(matches!(outcome, FetchOutcome::Updated { .. }));

        let feed = app.storage.get_feed(&feed_id).expect("feed");
        assert_eq!(feed.last_http_status, Some(200));
        assert_eq!(feed.failure_count, 0);
        assert_eq!(feed.health_state, FeedHealthState::Healthy);
        assert!(feed.last_fetch_at.is_some());
        assert!(feed.last_success_at.is_some());
        assert!(feed.next_scheduled_fetch_at.is_some());
        assert_eq!(feed.title.as_deref(), Some("Fixture RSS Feed"));
        assert_eq!(feed.feed_type, FeedType::Rss);
    }

    #[test]
    fn patches_item_state_through_core() {
        let storage = Storage::open_in_memory().expect("open");
        storage.migrate().expect("migrate");

        let mut app = InfoMatrixAppCore::new(storage);
        let entry_id = app
            .create_entry(NewEntry {
                id: None,
                kind: EntryKind::Note,
                source: EntrySource {
                    source_kind: EntrySourceKind::Manual,
                    source_id: None,
                    source_url: None,
                    source_title: Some("manual".to_owned()),
                },
                external_item_id: None,
                canonical_url: None,
                title: "Patch Target".to_owned(),
                author: None,
                summary: Some("Patch me".to_owned()),
                content_html: None,
                content_text: Some("Patch me".to_owned()),
                published_at: None,
                updated_at: None,
                raw_hash: "patch-1".to_owned(),
                dedup_reason: None,
                duplicate_of_entry_id: None,
            })
            .expect("create entry");

        let state = app
            .patch_item_state(
                &entry_id,
                &ItemStatePatch {
                    is_read: Some(true),
                    is_starred: Some(true),
                    is_saved_for_later: Some(true),
                    is_archived: Some(true),
                },
            )
            .expect("patch state");

        assert!(state.is_read);
        assert!(state.is_starred);
        assert!(state.is_saved_for_later);
        assert!(state.is_archived);
    }

    #[tokio::test]
    async fn searches_items_through_core() {
        let storage = Storage::open_in_memory().expect("open");
        storage.migrate().expect("migrate");

        let mut app = InfoMatrixAppCore::new(storage);
        let response = FetchResponse {
            final_url: url::Url::parse("https://example.com/feed.xml").expect("url parse"),
            status: 200,
            body: std::fs::read(
                std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("../../../tests/feeds/valid_rss.xml"),
            )
            .expect("fixture rss"),
            content_type: Some("application/rss+xml".to_owned()),
            etag: Some("etag-1".to_owned()),
            last_modified: Some("Tue, 04 Mar 2025 12:00:00 GMT".to_owned()),
            duration_ms: 17,
        };
        let feed_id = app
            .add_subscription_by_feed_url(
                &FetchService::new(MockFetchClient { response }),
                "https://example.com/feed.xml",
            )
            .await
            .expect("subscribe");
        app.storage
            .upsert_items(&[
                models::NormalizedItem {
                    id: "match-1".to_owned(),
                    source_feed_id: feed_id.clone(),
                    external_item_id: Some("ext-1".to_owned()),
                    canonical_url: Some(url::Url::parse("https://example.com/match").expect("url")),
                    title: "Matching item".to_owned(),
                    author: None,
                    summary: Some("contains special phrase".to_owned()),
                    content_html: None,
                    content_text: Some("special phrase appears here".to_owned()),
                    published_at: None,
                    updated_at: None,
                    raw_hash: "match-1".to_owned(),
                    dedup_reason: None,
                    duplicate_of_item_id: None,
                },
                models::NormalizedItem {
                    id: "other-1".to_owned(),
                    source_feed_id: feed_id.clone(),
                    external_item_id: Some("ext-2".to_owned()),
                    canonical_url: Some(url::Url::parse("https://example.com/other").expect("url")),
                    title: "Other item".to_owned(),
                    author: None,
                    summary: Some("irrelevant".to_owned()),
                    content_html: None,
                    content_text: Some("completely different".to_owned()),
                    published_at: None,
                    updated_at: None,
                    raw_hash: "other-1".to_owned(),
                    dedup_reason: None,
                    duplicate_of_item_id: None,
                },
            ])
            .expect("upsert items");

        let feed_results =
            app.search_items_for_feed(&feed_id, 10, Some("special phrase")).expect("search feed");
        assert_eq!(feed_results.len(), 1);
        assert_eq!(feed_results[0].title, "Matching item");

        let global_results = app
            .search_all_items(10, Some("special phrase"), ItemListFilter::All, None)
            .expect("search all");
        assert_eq!(global_results.len(), 1);
        assert_eq!(global_results[0].title, "Matching item");
    }

    #[tokio::test]
    async fn manages_groups_and_feed_metadata_through_core() {
        let storage = Storage::open_in_memory().expect("open");
        storage.migrate().expect("migrate");

        let mut app = InfoMatrixAppCore::new(storage);
        let response = FetchResponse {
            final_url: url::Url::parse("https://example.com/feed.xml").expect("url parse"),
            status: 200,
            body: std::fs::read(
                std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("../../../tests/feeds/valid_rss.xml"),
            )
            .expect("fixture rss"),
            content_type: Some("application/rss+xml".to_owned()),
            etag: Some("etag-1".to_owned()),
            last_modified: Some("Tue, 04 Mar 2025 12:00:00 GMT".to_owned()),
            duration_ms: 17,
        };
        let feed_id = app
            .add_subscription_by_feed_url(
                &FetchService::new(MockFetchClient { response }),
                "https://example.com/feed.xml",
            )
            .await
            .expect("subscribe");
        let group = app.create_group(" Tech ").expect("create group");

        let groups = app.list_groups().expect("list groups");
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].id, group.id);
        assert_eq!(groups[0].name, "Tech");

        app.set_feed_group(&feed_id, Some(&group.id)).expect("set group");
        let feed_groups = app.storage.list_groups_for_feed(&feed_id).expect("feed groups");
        assert_eq!(feed_groups.len(), 1);
        assert_eq!(feed_groups[0].id, group.id);

        app.rename_feed(&feed_id, Some("Renamed feed")).expect("rename");
        let feed = app.list_feeds().expect("list feeds");
        assert_eq!(feed.len(), 1);
        assert_eq!(feed[0].id, feed_id);
        assert_eq!(feed[0].title.as_deref(), Some("Renamed feed"));

        app.delete_feed(&feed_id).expect("delete");
        assert!(app.list_feeds().expect("list feeds").is_empty());
    }
}
