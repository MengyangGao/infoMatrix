use serde::{Deserialize, Serialize};
use storage::SyncEventRecord;

use crate::storage::open_storage;

#[derive(Debug, Deserialize)]
pub struct ListSyncEventsInput {
    pub db_path: Option<String>,
    pub limit: Option<usize>,
}

#[derive(Debug, Deserialize)]
pub struct AckSyncEventsInput {
    pub db_path: Option<String>,
    pub event_ids: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct SyncEventInput {
    pub id: String,
    pub entity_type: String,
    pub entity_id: String,
    pub event_type: String,
    pub payload_json: String,
    pub created_at: String,
}

#[derive(Debug, Deserialize)]
pub struct ApplySyncEventsInput {
    pub db_path: Option<String>,
    pub events: Vec<SyncEventInput>,
}

#[derive(Debug, Serialize)]
pub struct SyncEventOutput {
    pub id: String,
    pub entity_type: String,
    pub entity_id: String,
    pub event_type: String,
    pub payload_json: String,
    pub created_at: String,
}

#[derive(Debug, Serialize)]
pub struct ApplySyncEventsOutput {
    pub applied: usize,
}

pub fn list_sync_events(
    db_path: &Option<String>,
    limit: Option<usize>,
) -> Result<Vec<SyncEventOutput>, String> {
    let storage = open_storage(db_path)?;
    let limit = limit.unwrap_or(100).clamp(1, 500);
    let events: Vec<SyncEventOutput> = storage
        .list_pending_sync_events(limit)
        .map_err(|err| err.to_string())?
        .into_iter()
        .map(|event| SyncEventOutput {
            id: event.id,
            entity_type: event.entity_type,
            entity_id: event.entity_id,
            event_type: event.event_type,
            payload_json: event.payload_json,
            created_at: event.created_at,
        })
        .collect();
    Ok(events)
}

pub fn ack_sync_events(db_path: &Option<String>, event_ids: &[String]) -> Result<usize, String> {
    let storage = open_storage(db_path)?;
    storage.acknowledge_sync_events(event_ids).map_err(|err| err.to_string())
}

pub fn apply_sync_events(
    db_path: &Option<String>,
    events: Vec<SyncEventInput>,
) -> Result<ApplySyncEventsOutput, String> {
    let mut storage = open_storage(db_path)?;
    let events = events
        .into_iter()
        .map(|event| SyncEventRecord {
            id: event.id,
            entity_type: event.entity_type,
            entity_id: event.entity_id,
            event_type: event.event_type,
            payload_json: event.payload_json,
            created_at: event.created_at,
        })
        .collect::<Vec<_>>();
    let applied = storage.apply_sync_events(&events).map_err(|err| err.to_string())?;
    Ok(ApplySyncEventsOutput { applied })
}
