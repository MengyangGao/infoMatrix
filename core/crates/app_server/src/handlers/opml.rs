use std::collections::HashSet;

use axum::Json;
use axum::extract::State;
use models::{FeedType, NewFeed};
use opml::{OpmlFeed, export_opml, import_opml};

use crate::config::AppContext;
use crate::error::ApiError;
use crate::state::open_storage;
use crate::views::{OpmlExportResponse, OpmlImportRequest, OpmlImportResponse};

pub(crate) async fn export_opml_subscriptions(
    State(context): State<AppContext>,
) -> Result<Json<OpmlExportResponse>, ApiError> {
    let storage = open_storage(&context)?;
    let feeds = storage.list_feeds()?;
    let mut opml_feeds = Vec::with_capacity(feeds.len());

    for feed in feeds {
        let group = storage.list_groups_for_feed(&feed.id)?.into_iter().next().map(|row| row.name);
        opml_feeds.push(OpmlFeed {
            title: feed.title,
            xml_url: feed.feed_url,
            html_url: feed.site_url,
            group,
        });
    }

    let feed_count = opml_feeds.len();
    let opml_xml = export_opml(&opml_feeds, "InfoMatrix Subscriptions")
        .map_err(|err| ApiError::Internal(err.to_string()))?;
    Ok(Json(OpmlExportResponse { opml_xml, feed_count }))
}

pub(crate) async fn import_opml_subscriptions(
    State(context): State<AppContext>,
    Json(payload): Json<OpmlImportRequest>,
) -> Result<Json<OpmlImportResponse>, ApiError> {
    let opml_xml = payload.opml_xml.trim();
    if opml_xml.is_empty() {
        return Err(ApiError::BadRequest("opml_xml is empty".to_owned()));
    }

    let feeds = import_opml(opml_xml).map_err(|err| ApiError::BadRequest(err.to_string()))?;
    let parsed_feed_count = feeds.len();
    let mut unique_urls = HashSet::new();
    let mut grouped_feed_count = 0usize;

    let mut storage = open_storage(&context)?;
    for feed in feeds {
        unique_urls.insert(feed.xml_url.to_string());
        let feed_id = storage.upsert_feed(&NewFeed {
            feed_url: feed.xml_url,
            site_url: feed.html_url,
            title: feed.title,
            feed_type: FeedType::Unknown,
        })?;

        if let Some(group_name) =
            feed.group.map(|name| name.trim().to_owned()).filter(|name| !name.is_empty())
        {
            let group = storage.create_group(&group_name)?;
            storage.set_feed_group(&feed_id, Some(&group.id))?;
            grouped_feed_count = grouped_feed_count.saturating_add(1);
        }
    }

    Ok(Json(OpmlImportResponse {
        parsed_feed_count,
        unique_feed_count: unique_urls.len(),
        grouped_feed_count,
    }))
}
