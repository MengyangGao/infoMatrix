use models::FeedType;
use opml::{OpmlFeed, export_opml, import_opml};
use serde::{Deserialize, Serialize};

use crate::storage::open_app_core;

#[derive(Debug, Deserialize)]
pub struct OpmlImportInput {
    pub db_path: Option<String>,
    pub opml_xml: String,
}

#[derive(Debug, Serialize)]
pub struct OpmlImportOutput {
    pub parsed_feed_count: usize,
    pub unique_feed_count: usize,
    pub grouped_feed_count: usize,
}

#[derive(Debug, Serialize)]
pub struct OpmlExportOutput {
    pub opml_xml: String,
    pub feed_count: usize,
}

pub fn export_opml_subscriptions(db_path: &Option<String>) -> Result<OpmlExportOutput, String> {
    let core = open_app_core(db_path)?;
    let feeds = core.list_feeds().map_err(|err| err.to_string())?;
    let storage = core.storage;
    let mut opml_feeds = Vec::with_capacity(feeds.len());

    for feed in feeds {
        let group = storage
            .list_groups_for_feed(&feed.id)
            .map_err(|err| err.to_string())?
            .into_iter()
            .next()
            .map(|row| row.name);
        opml_feeds.push(OpmlFeed {
            title: feed.title,
            xml_url: feed.feed_url,
            html_url: feed.site_url,
            group,
        });
    }

    let feed_count = opml_feeds.len();
    let opml_xml = export_opml(&opml_feeds, "InfoMatrix Subscriptions")
        .map_err(|err| format!("opml export failed: {err}"))?;
    Ok(OpmlExportOutput { opml_xml, feed_count })
}

pub fn import_opml_subscriptions(
    db_path: &Option<String>,
    opml_xml: &str,
) -> Result<OpmlImportOutput, String> {
    let opml_xml = opml_xml.trim();
    if opml_xml.is_empty() {
        return Err("opml_xml is empty".to_owned());
    }

    let feeds = import_opml(opml_xml).map_err(|err| format!("opml import failed: {err}"))?;
    let parsed_feed_count = feeds.len();
    let mut unique_urls = std::collections::HashSet::new();
    let mut grouped_feed_count = 0usize;

    let mut storage = crate::storage::open_storage(db_path)?;
    for feed in feeds {
        unique_urls.insert(feed.xml_url.to_string());
        let feed_id = storage
            .upsert_feed(&models::NewFeed {
                feed_url: feed.xml_url,
                site_url: feed.html_url,
                title: feed.title,
                feed_type: FeedType::Unknown,
            })
            .map_err(|err| err.to_string())?;

        if let Some(group_name) =
            feed.group.map(|name| name.trim().to_owned()).filter(|name| !name.is_empty())
        {
            let group = storage.create_group(&group_name).map_err(|err| err.to_string())?;
            storage.set_feed_group(&feed_id, Some(&group.id)).map_err(|err| err.to_string())?;
            grouped_feed_count = grouped_feed_count.saturating_add(1);
        }
    }

    Ok(OpmlImportOutput {
        parsed_feed_count,
        unique_feed_count: unique_urls.len(),
        grouped_feed_count,
    })
}
