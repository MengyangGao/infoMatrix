use axum::Router;
use axum::middleware;
use axum::routing::{get, patch, post};

use crate::config::AppContext;
use crate::handlers::core::{health, meta, require_api_token};
use crate::handlers::feeds::{
    delete_feed, list_due_feeds, list_feeds, refresh_due_feeds, refresh_feed, update_feed,
    update_feed_group,
};
use crate::handlers::groups::{create_group, list_groups};
use crate::handlers::items::{
    create_entry, fetch_item_fulltext, get_item_detail, item_counts, list_all_items, list_items,
    patch_item_state,
};
use crate::handlers::notifications::{
    ack_notification_events, delete_feed_refresh_settings, delete_group_refresh_settings,
    get_feed_notification_settings, get_feed_refresh_settings, get_global_notification_settings,
    get_group_refresh_settings, list_pending_notification_events,
    update_feed_notification_settings, update_feed_refresh_settings,
    update_global_notification_settings, update_group_refresh_settings,
};
use crate::handlers::opml::{export_opml_subscriptions, import_opml_subscriptions};
use crate::handlers::subscriptions::{add_subscription, discover_site, subscribe_input};
use crate::handlers::sync::{ack_sync_events, apply_sync_events, list_sync_events};

pub(crate) fn app_router(context: AppContext) -> Router {
    let protected_routes = Router::new()
        .route("/api/v1/discover", post(discover_site))
        .route("/api/v1/subscriptions", post(add_subscription))
        .route("/api/v1/subscribe", post(subscribe_input))
        .route("/api/v1/feeds", get(list_feeds))
        .route("/api/v1/groups", get(list_groups).post(create_group))
        .route("/api/v1/feeds/{feed_id}", patch(update_feed).delete(delete_feed))
        .route("/api/v1/feeds/{feed_id}/group", patch(update_feed_group))
        .route("/api/v1/feeds/{feed_id}/items", get(list_items))
        .route("/api/v1/items", get(list_all_items))
        .route("/api/v1/items/counts", get(item_counts))
        .route("/api/v1/entries", get(list_all_items).post(create_entry))
        .route("/api/v1/entries/counts", get(item_counts))
        .route("/api/v1/items/{item_id}", get(get_item_detail))
        .route("/api/v1/entries/{item_id}", get(get_item_detail))
        .route("/api/v1/items/{item_id}/fulltext", post(fetch_item_fulltext))
        .route("/api/v1/entries/{item_id}/fulltext", post(fetch_item_fulltext))
        .route("/api/v1/refresh/{feed_id}", post(refresh_feed))
        .route("/api/v1/refresh/due", post(refresh_due_feeds))
        .route("/api/v1/feeds/due", get(list_due_feeds))
        .route(
            "/api/v1/feeds/{feed_id}/notifications",
            get(get_feed_notification_settings).put(update_feed_notification_settings),
        )
        .route(
            "/api/v1/feeds/{feed_id}/refresh-settings",
            get(get_feed_refresh_settings)
                .put(update_feed_refresh_settings)
                .delete(delete_feed_refresh_settings),
        )
        .route(
            "/api/v1/groups/{group_id}/refresh-settings",
            get(get_group_refresh_settings)
                .put(update_group_refresh_settings)
                .delete(delete_group_refresh_settings),
        )
        .route(
            "/api/v1/notifications/settings",
            get(get_global_notification_settings).put(update_global_notification_settings),
        )
        .route("/api/v1/notifications/pending", get(list_pending_notification_events))
        .route("/api/v1/notifications/pending/ack", post(ack_notification_events))
        .route("/api/v1/items/{item_id}/state", patch(patch_item_state))
        .route("/api/v1/entries/{item_id}/state", patch(patch_item_state))
        .route("/api/v1/sync/events", get(list_sync_events))
        .route("/api/v1/sync/events/ack", post(ack_sync_events))
        .route("/api/v1/sync/events/apply", post(apply_sync_events))
        .route("/api/v1/opml/export", get(export_opml_subscriptions))
        .route("/api/v1/opml/import", post(import_opml_subscriptions))
        .route_layer(middleware::from_fn_with_state(context.clone(), require_api_token));

    Router::new()
        .route("/api/v1/health", get(health))
        .route("/api/v1/meta", get(meta))
        .merge(protected_routes)
        .with_state(context)
}
