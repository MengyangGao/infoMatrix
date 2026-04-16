//! OPML import/export helpers.

use quick_xml::{de::from_str, se::to_string};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use url::Url;

/// OPML feed row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpmlFeed {
    /// Feed title.
    pub title: Option<String>,
    /// Feed XML URL.
    pub xml_url: Url,
    /// Optional site URL.
    pub html_url: Option<Url>,
    /// Optional group/folder label.
    pub group: Option<String>,
}

/// OPML parse/serialize errors.
#[derive(Debug, Error)]
pub enum OpmlError {
    /// XML decode error.
    #[error("opml decode error: {0}")]
    Decode(String),
    /// XML encode error.
    #[error("opml encode error: {0}")]
    Encode(String),
    /// Invalid URL in OPML.
    #[error("invalid feed url: {0}")]
    InvalidUrl(String),
}

/// Parse OPML into feed rows.
pub fn import_opml(xml: &str) -> Result<Vec<OpmlFeed>, OpmlError> {
    let parsed: OpmlDocument = from_str(xml).map_err(|err| OpmlError::Decode(err.to_string()))?;

    let mut feeds = Vec::new();
    for outline in parsed.body.outlines {
        collect_outline(None, &outline, &mut feeds)?;
    }
    Ok(feeds)
}

/// Serialize feed rows into OPML string.
pub fn export_opml(feeds: &[OpmlFeed], title: &str) -> Result<String, OpmlError> {
    let mut groups: std::collections::BTreeMap<Option<String>, Vec<&OpmlFeed>> =
        std::collections::BTreeMap::new();
    for feed in feeds {
        groups.entry(feed.group.clone()).or_default().push(feed);
    }

    let mut outlines = Vec::new();

    if let Some(root_feeds) = groups.remove(&None) {
        outlines.extend(root_feeds.into_iter().map(|feed| feed_to_outline(feed, None)));
    }

    for (group, group_feeds) in groups {
        let group_name = group.unwrap_or_else(|| "Ungrouped".to_owned());
        let children = group_feeds
            .into_iter()
            .map(|feed| feed_to_outline(feed, Some(group_name.clone())))
            .collect();
        outlines.push(Outline {
            text: Some(group_name.clone()),
            title: Some(group_name),
            outline_type: None,
            xml_url: None,
            html_url: None,
            outlines: children,
        });
    }

    let doc = OpmlDocument {
        version: "2.0".to_owned(),
        head: OpmlHead { title: Some(title.to_owned()) },
        body: OpmlBody { outlines },
    };

    let xml = to_string(&doc).map_err(|err| OpmlError::Encode(err.to_string()))?;
    Ok(format!("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n{xml}"))
}

fn feed_to_outline(feed: &OpmlFeed, _group: Option<String>) -> Outline {
    Outline {
        text: feed.title.clone(),
        title: feed.title.clone(),
        outline_type: Some("rss".to_owned()),
        xml_url: Some(feed.xml_url.to_string()),
        html_url: feed.html_url.as_ref().map(ToString::to_string),
        outlines: Vec::new(),
    }
}

fn collect_outline(
    inherited_group: Option<String>,
    outline: &Outline,
    out: &mut Vec<OpmlFeed>,
) -> Result<(), OpmlError> {
    if let Some(xml_url) = outline.xml_url.as_ref().filter(|value| !value.trim().is_empty()) {
        let xml_url = Url::parse(xml_url).map_err(|err| OpmlError::InvalidUrl(err.to_string()))?;
        let html_url = outline.html_url.as_ref().and_then(|value| Url::parse(value).ok());
        out.push(OpmlFeed {
            title: outline.title.clone().or_else(|| outline.text.clone()),
            xml_url,
            html_url,
            group: inherited_group,
        });
        return Ok(());
    }

    let group_name = outline.title.clone().or_else(|| outline.text.clone());
    for child in &outline.outlines {
        collect_outline(group_name.clone(), child, out)?;
    }

    Ok(())
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename = "opml")]
struct OpmlDocument {
    #[serde(rename = "@version")]
    version: String,
    head: OpmlHead,
    body: OpmlBody,
}

#[derive(Debug, Serialize, Deserialize)]
struct OpmlHead {
    title: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct OpmlBody {
    #[serde(default, rename = "outline")]
    outlines: Vec<Outline>,
}

#[derive(Debug, Serialize, Deserialize)]
struct Outline {
    #[serde(rename = "@text")]
    text: Option<String>,
    #[serde(rename = "@title")]
    title: Option<String>,
    #[serde(rename = "@type")]
    outline_type: Option<String>,
    #[serde(rename = "@xmlUrl")]
    xml_url: Option<String>,
    #[serde(rename = "@htmlUrl")]
    html_url: Option<String>,
    #[serde(default, rename = "outline")]
    outlines: Vec<Outline>,
}

#[cfg(test)]
mod tests {
    use super::{OpmlFeed, export_opml, import_opml};

    #[test]
    fn round_trip_opml() {
        let feeds = vec![OpmlFeed {
            title: Some("Example".to_owned()),
            xml_url: url::Url::parse("https://example.com/feed.xml").expect("url parse"),
            html_url: Some(url::Url::parse("https://example.com").expect("url parse")),
            group: Some("Tech".to_owned()),
        }];

        let xml = export_opml(&feeds, "Subscriptions").expect("export");
        let imported = import_opml(&xml).expect("import");

        assert_eq!(imported.len(), 1);
        assert_eq!(imported[0].group.as_deref(), Some("Tech"));
        assert_eq!(imported[0].title.as_deref(), Some("Example"));
    }
}
