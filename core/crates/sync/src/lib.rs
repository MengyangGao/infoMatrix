//! Sync event model and adapter interface.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

/// Sync event kinds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SyncEventType {
    /// Entity create mutation.
    Created,
    /// Entity update mutation.
    Updated,
    /// Entity delete mutation.
    Deleted,
}

/// Local event log entry for future sync adapters.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SyncEvent {
    /// Event ID.
    pub id: Uuid,
    /// Entity type, e.g. `feed`, `item_state`.
    pub entity_type: String,
    /// Entity identifier.
    pub entity_id: String,
    /// Event kind.
    pub event_type: SyncEventType,
    /// JSON payload.
    pub payload_json: String,
    /// Event creation time.
    pub created_at: DateTime<Utc>,
}

/// Sync adapter errors.
#[derive(Debug, Error)]
pub enum SyncError {
    /// Adapter operation failure.
    #[error("adapter error: {0}")]
    Adapter(String),
}

/// Adapter boundary for future sync backends.
#[async_trait]
pub trait SyncAdapter: Send + Sync {
    /// Push pending local events to remote.
    async fn push_events(&self, events: &[SyncEvent]) -> Result<(), SyncError>;
    /// Pull remote events into local representation.
    async fn pull_events(&self) -> Result<Vec<SyncEvent>, SyncError>;
}

/// Local-only no-op sync adapter.
pub struct LocalOnlySyncAdapter;

#[async_trait]
impl SyncAdapter for LocalOnlySyncAdapter {
    async fn push_events(&self, _events: &[SyncEvent]) -> Result<(), SyncError> {
        Ok(())
    }

    async fn pull_events(&self) -> Result<Vec<SyncEvent>, SyncError> {
        Ok(Vec::new())
    }
}
