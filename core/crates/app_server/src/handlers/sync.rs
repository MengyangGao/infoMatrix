use axum::Json;
use axum::extract::{Query, State};
use storage::SyncEventRecord;

use crate::config::AppContext;
use crate::error::ApiError;
use crate::state::open_storage;
use crate::views::{
    AckSyncEventsRequest, AckSyncEventsResponse, ApplySyncEventsRequest, ApplySyncEventsResponse,
    ListSyncEventsQuery, SyncEventView,
};

pub(crate) async fn list_sync_events(
    State(context): State<AppContext>,
    Query(query): Query<ListSyncEventsQuery>,
) -> Result<Json<Vec<SyncEventView>>, ApiError> {
    let storage = open_storage(&context)?;
    let limit = query.limit.unwrap_or(100).clamp(1, 500);
    let events = storage
        .list_pending_sync_events(limit)?
        .into_iter()
        .map(|event| SyncEventView {
            id: event.id,
            entity_type: event.entity_type,
            entity_id: event.entity_id,
            event_type: event.event_type,
            payload_json: event.payload_json,
            created_at: event.created_at,
        })
        .collect();
    Ok(Json(events))
}

pub(crate) async fn ack_sync_events(
    State(context): State<AppContext>,
    Json(payload): Json<AckSyncEventsRequest>,
) -> Result<Json<AckSyncEventsResponse>, ApiError> {
    let storage = open_storage(&context)?;
    let acknowledged = storage.acknowledge_sync_events(&payload.event_ids)?;
    Ok(Json(AckSyncEventsResponse { acknowledged }))
}

pub(crate) async fn apply_sync_events(
    State(context): State<AppContext>,
    Json(payload): Json<ApplySyncEventsRequest>,
) -> Result<Json<ApplySyncEventsResponse>, ApiError> {
    let mut storage = open_storage(&context)?;
    let events = payload
        .events
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
    let applied = storage.apply_sync_events(&events)?;
    Ok(Json(ApplySyncEventsResponse { applied }))
}
