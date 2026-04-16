//! Search indexing boundary for future full-text search.

use thiserror::Error;

/// Search index errors.
#[derive(Debug, Error)]
pub enum SearchError {
    /// Generic indexing error.
    #[error("index error: {0}")]
    Index(String),
}

/// Search indexer interface.
pub trait SearchIndexer {
    /// Index one item payload.
    fn index_item(&self, item_id: &str, title: &str, content_text: &str)
    -> Result<(), SearchError>;
    /// Remove indexed entry by item id.
    fn remove_item(&self, item_id: &str) -> Result<(), SearchError>;
}

/// No-op search indexer for builds that rely on SQLite FTS in storage.
pub struct NoopSearchIndexer;

impl SearchIndexer for NoopSearchIndexer {
    fn index_item(
        &self,
        _item_id: &str,
        _title: &str,
        _content_text: &str,
    ) -> Result<(), SearchError> {
        Ok(())
    }

    fn remove_item(&self, _item_id: &str) -> Result<(), SearchError> {
        Ok(())
    }
}
