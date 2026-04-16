//! Feed parsing and normalization for RSS, Atom, and JSON Feed.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::io::Cursor;

use atom_syndication::Feed as AtomFeed;
use chrono::{DateTime, Utc};
use models::{DedupReason, FeedType, NormalizedItem};
use rss::Channel;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;
use url::Url;

/// Parsed feed payload with normalized metadata and items.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParsedFeed {
    /// Inferred/parsed feed type.
    pub feed_type: FeedType,
    /// Feed title.
    pub title: Option<String>,
    /// Source site URL if available.
    pub site_url: Option<Url>,
    /// Feed description/subtitle.
    pub description: Option<String>,
    /// Normalized parsed items.
    pub items: Vec<NormalizedItem>,
}

/// Parser errors.
#[derive(Debug, Error)]
pub enum ParseError {
    /// Unsupported or undetected feed format.
    #[error("unsupported feed format")]
    UnsupportedFormat,
    /// RSS parse failure.
    #[error("rss parse error: {0}")]
    Rss(String),
    /// Atom parse failure.
    #[error("atom parse error: {0}")]
    Atom(String),
    /// JSON Feed parse failure.
    #[error("json feed parse error: {0}")]
    JsonFeed(String),
}

/// Detect feed type from content-type hint and body root structure.
pub fn detect_feed_type(body: &[u8], content_type: Option<&str>) -> FeedType {
    if let Some(ctype) = content_type.map(|v| v.to_ascii_lowercase()) {
        if ctype.contains("application/rss+xml") {
            return FeedType::Rss;
        }
        if ctype.contains("application/atom+xml") {
            return FeedType::Atom;
        }
        if ctype.contains("application/feed+json") || ctype.contains("application/json") {
            return FeedType::JsonFeed;
        }
    }

    let text = String::from_utf8_lossy(body);
    let trimmed = text.trim_start();
    if trimmed.starts_with("<rss") || trimmed.contains("<rss ") {
        return FeedType::Rss;
    }
    if trimmed.starts_with("<feed") || trimmed.contains("<feed ") {
        return FeedType::Atom;
    }
    if trimmed.starts_with('{') {
        if let Ok(value) = serde_json::from_str::<serde_json::Value>(trimmed) {
            if value.get("version").is_some() && value.get("items").is_some() {
                return FeedType::JsonFeed;
            }
        }
    }

    FeedType::Unknown
}

/// Parse feed bytes into normalized output.
pub fn parse_feed(
    source_feed_id: &str,
    body: &[u8],
    content_type: Option<&str>,
) -> Result<ParsedFeed, ParseError> {
    match detect_feed_type(body, content_type) {
        FeedType::Rss => parse_rss(source_feed_id, body),
        FeedType::Atom => parse_atom(source_feed_id, body),
        FeedType::JsonFeed => parse_json_feed(source_feed_id, body),
        FeedType::Unknown => parse_rss(source_feed_id, body)
            .or_else(|_| parse_atom(source_feed_id, body))
            .or_else(|_| parse_json_feed(source_feed_id, body))
            .map_err(|_| ParseError::UnsupportedFormat),
    }
}

/// Build dedup key using the configured precedence order.
pub fn dedup_key(item: &NormalizedItem) -> (DedupReason, String) {
    if let Some(external) = item.external_item_id.as_ref() {
        return (DedupReason::ExternalId, external.clone());
    }
    if let (Some(url), Some(published_at)) = (item.canonical_url.as_ref(), item.published_at) {
        return (
            DedupReason::UrlAndPublishedAt,
            format!("{}|{}", normalize_url_for_dedup(url), published_at.to_rfc3339()),
        );
    }
    if let Some(url) = item.canonical_url.as_ref() {
        return (DedupReason::CanonicalUrl, normalize_url_for_dedup(url));
    }

    let title_hash = stable_hash(&item.title);
    let date_part =
        item.published_at.map(|v| v.to_rfc3339()).unwrap_or_else(|| "no-date".to_owned());
    (DedupReason::TitleDateFallback, format!("{}|{}", title_hash, date_part))
}

fn parse_rss(source_feed_id: &str, body: &[u8]) -> Result<ParsedFeed, ParseError> {
    let channel =
        Channel::read_from(Cursor::new(body)).map_err(|e| ParseError::Rss(e.to_string()))?;
    let site_url = non_empty(Some(channel.link().to_owned())).and_then(|v| Url::parse(&v).ok());
    let title = non_empty(Some(channel.title().to_owned()));
    let description = non_empty(Some(channel.description().to_owned()));

    let items = channel
        .items()
        .iter()
        .map(|item| {
            let external_item_id = item.guid().map(|g| g.value().to_owned());
            let canonical_url = item.link().and_then(|v| Url::parse(v).ok());
            let title_value =
                item.title().map(ToOwned::to_owned).unwrap_or_else(|| "(untitled)".to_owned());
            let summary = non_empty(item.description().map(ToOwned::to_owned));
            let mut content_html = non_empty(item.content().map(ToOwned::to_owned))
                .or_else(|| rss_content_encoded(item))
                .or_else(|| summary.clone().filter(|value| value.contains('<')));
            if content_html.is_none() {
                content_html = rss_image_enclosure_html(item);
            }
            let content_text = content_html
                .as_ref()
                .map(|v| strip_html_tags(v))
                .or_else(|| summary.as_ref().map(|v| strip_html_tags(v)))
                .filter(|value| !value.trim().is_empty());
            let published_at = item.pub_date().and_then(parse_datetime);
            let updated_at = published_at;
            let raw_hash = hash_item_payload(
                &title_value,
                canonical_url.as_ref(),
                summary.as_deref(),
                content_html.as_deref(),
            );
            let dedup_input = external_item_id
                .clone()
                .or_else(|| canonical_url.as_ref().map(ToString::to_string))
                .unwrap_or_else(|| format!("{}:{}", title_value, raw_hash));

            NormalizedItem {
                id: format!("itm_{}", stable_hash(&dedup_input)),
                source_feed_id: source_feed_id.to_owned(),
                external_item_id,
                canonical_url,
                title: title_value,
                author: non_empty(item.author().map(ToOwned::to_owned)),
                summary,
                content_html,
                content_text,
                published_at,
                updated_at,
                raw_hash,
                dedup_reason: None,
                duplicate_of_item_id: None,
            }
        })
        .collect();

    Ok(ParsedFeed { feed_type: FeedType::Rss, title, site_url, description, items })
}

fn parse_atom(source_feed_id: &str, body: &[u8]) -> Result<ParsedFeed, ParseError> {
    let feed =
        AtomFeed::read_from(Cursor::new(body)).map_err(|e| ParseError::Atom(e.to_string()))?;
    let site_url = feed.links().iter().find_map(|l| Url::parse(l.href()).ok());
    let title = non_empty(Some(feed.title().to_string()));
    let description = non_empty(feed.subtitle().map(|text| text.value.clone()));

    let items = feed
        .entries()
        .iter()
        .map(|entry| {
            let external_item_id = non_empty(Some(entry.id().to_owned()));
            let canonical_url = entry.links().iter().find_map(|l| Url::parse(l.href()).ok());
            let summary = non_empty(entry.summary().map(|text| text.value.clone()));
            let content_html =
                non_empty(entry.content().and_then(|c| c.value().map(ToOwned::to_owned)));
            let content_text = content_html.as_ref().map(|v| strip_html_tags(v));
            let title_value = non_empty(Some(entry.title().to_string()))
                .unwrap_or_else(|| "(untitled)".to_owned());
            let author = entry
                .authors()
                .first()
                .map(|author| author.name().to_owned())
                .filter(|v| !v.trim().is_empty());
            let published_at = entry.published().map(|dt| dt.with_timezone(&Utc));
            let updated_at = Some(entry.updated().with_timezone(&Utc));
            let raw_hash = hash_item_payload(
                &title_value,
                canonical_url.as_ref(),
                summary.as_deref(),
                content_html.as_deref(),
            );
            let dedup_input = external_item_id
                .clone()
                .or_else(|| canonical_url.as_ref().map(ToString::to_string))
                .unwrap_or_else(|| format!("{}:{}", title_value, raw_hash));

            NormalizedItem {
                id: format!("itm_{}", stable_hash(&dedup_input)),
                source_feed_id: source_feed_id.to_owned(),
                external_item_id,
                canonical_url,
                title: title_value,
                author,
                summary,
                content_html,
                content_text,
                published_at,
                updated_at,
                raw_hash,
                dedup_reason: None,
                duplicate_of_item_id: None,
            }
        })
        .collect();

    Ok(ParsedFeed { feed_type: FeedType::Atom, title, site_url, description, items })
}

fn parse_json_feed(source_feed_id: &str, body: &[u8]) -> Result<ParsedFeed, ParseError> {
    let feed: JsonFeedRoot =
        serde_json::from_slice(body).map_err(|e| ParseError::JsonFeed(e.to_string()))?;

    let items = feed
        .items
        .iter()
        .map(|item| {
            let canonical_url =
                item.url.as_deref().and_then(|value| Url::parse(value).ok()).or_else(|| {
                    item.external_url.as_deref().and_then(|value| Url::parse(value).ok())
                });
            let external_item_id = non_empty(Some(item.id.clone()));
            let title_value = item
                .title
                .clone()
                .filter(|v| !v.trim().is_empty())
                .unwrap_or_else(|| "(untitled)".to_owned());
            let summary = item.summary.clone().filter(|v| !v.trim().is_empty());
            let content_html = item.content_html.clone().filter(|v| !v.trim().is_empty());
            let content_text = item
                .content_text
                .clone()
                .filter(|v| !v.trim().is_empty())
                .or_else(|| content_html.as_ref().map(|value| strip_html_tags(value)));
            let author = item
                .authors
                .as_ref()
                .and_then(|list| list.first())
                .and_then(|author| author.name.clone())
                .filter(|v| !v.trim().is_empty());
            let published_at = item.date_published.as_deref().and_then(parse_datetime);
            let updated_at = item.date_modified.as_deref().and_then(parse_datetime);
            let raw_hash = hash_item_payload(
                &title_value,
                canonical_url.as_ref(),
                summary.as_deref(),
                content_html.as_deref(),
            );
            let dedup_input = external_item_id
                .clone()
                .or_else(|| canonical_url.as_ref().map(ToString::to_string))
                .unwrap_or_else(|| format!("{}:{}", title_value, raw_hash));

            NormalizedItem {
                id: format!("itm_{}", stable_hash(&dedup_input)),
                source_feed_id: source_feed_id.to_owned(),
                external_item_id,
                canonical_url,
                title: title_value,
                author,
                summary,
                content_html,
                content_text,
                published_at,
                updated_at,
                raw_hash,
                dedup_reason: None,
                duplicate_of_item_id: None,
            }
        })
        .collect();

    Ok(ParsedFeed {
        feed_type: FeedType::JsonFeed,
        title: feed.title,
        site_url: feed.home_page_url.and_then(|v| Url::parse(&v).ok()),
        description: feed.description,
        items,
    })
}

fn parse_datetime(value: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(value)
        .map(|v| v.with_timezone(&Utc))
        .ok()
        .or_else(|| DateTime::parse_from_rfc2822(value).map(|v| v.with_timezone(&Utc)).ok())
}

fn normalize_url_for_dedup(url: &Url) -> String {
    let mut cloned = url.clone();
    cloned.set_fragment(None);
    cloned.to_string()
}

fn hash_item_payload(
    title: &str,
    canonical_url: Option<&Url>,
    summary: Option<&str>,
    content_html: Option<&str>,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(title.as_bytes());
    if let Some(url) = canonical_url {
        hasher.update(url.as_str().as_bytes());
    }
    if let Some(summary) = summary {
        hasher.update(summary.as_bytes());
    }
    if let Some(content_html) = content_html {
        hasher.update(content_html.as_bytes());
    }
    to_hex(&hasher.finalize())
}

fn stable_hash(value: &str) -> String {
    let mut hasher = DefaultHasher::new();
    value.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

fn non_empty(value: Option<String>) -> Option<String> {
    value.filter(|v| !v.trim().is_empty())
}

fn rss_content_encoded(item: &rss::Item) -> Option<String> {
    let extension = item.extensions().get("content")?.get("encoded")?.first()?;
    extension.value().map(str::trim).filter(|value| !value.is_empty()).map(ToOwned::to_owned)
}

fn rss_image_enclosure_html(item: &rss::Item) -> Option<String> {
    let enclosure = item.enclosure()?;
    let mime = enclosure.mime_type().to_ascii_lowercase();
    if !mime.starts_with("image/") {
        return None;
    }
    let url = enclosure.url();
    if url.trim().is_empty() {
        return None;
    }
    Some(format!(r#"<p><img src="{url}" alt="" /></p>"#))
}

fn strip_html_tags(value: &str) -> String {
    let mut output = String::with_capacity(value.len());
    let mut inside_tag = false;
    for c in value.chars() {
        match c {
            '<' => inside_tag = true,
            '>' => inside_tag = false,
            _ if !inside_tag => output.push(c),
            _ => {}
        }
    }
    output.trim().to_owned()
}

fn to_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0x0f) as usize] as char);
    }
    out
}

#[derive(Debug, Deserialize)]
struct JsonFeedRoot {
    title: Option<String>,
    home_page_url: Option<String>,
    description: Option<String>,
    items: Vec<JsonFeedItem>,
}

#[derive(Debug, Deserialize)]
struct JsonFeedItem {
    id: String,
    url: Option<String>,
    external_url: Option<String>,
    title: Option<String>,
    summary: Option<String>,
    content_html: Option<String>,
    content_text: Option<String>,
    date_published: Option<String>,
    date_modified: Option<String>,
    authors: Option<Vec<JsonFeedAuthor>>,
}

#[derive(Debug, Deserialize)]
struct JsonFeedAuthor {
    name: Option<String>,
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use models::{DedupReason, FeedType, NormalizedItem};
    use url::Url;

    use super::{dedup_key, detect_feed_type, parse_feed};

    fn fixture_path(name: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../tests/feeds").join(name)
    }

    #[test]
    fn parses_rss_fixture() {
        let bytes = std::fs::read(fixture_path("valid_rss.xml")).expect("fixture must exist");
        let parsed = parse_feed("feed_1", &bytes, Some("application/rss+xml")).expect("rss parse");
        assert_eq!(parsed.feed_type, FeedType::Rss);
        assert_eq!(parsed.items.len(), 2);
        assert_eq!(parsed.items[0].title, "RSS Item One");
    }

    #[test]
    fn parses_atom_fixture() {
        let bytes = std::fs::read(fixture_path("valid_atom.xml")).expect("fixture must exist");
        let parsed =
            parse_feed("feed_1", &bytes, Some("application/atom+xml")).expect("atom parse");
        assert_eq!(parsed.feed_type, FeedType::Atom);
        assert_eq!(parsed.items.len(), 1);
        assert_eq!(parsed.items[0].title, "Atom Entry One");
    }

    #[test]
    fn parses_json_feed_fixture() {
        let bytes =
            std::fs::read(fixture_path("valid_json_feed.json")).expect("fixture must exist");
        let parsed =
            parse_feed("feed_1", &bytes, Some("application/feed+json")).expect("json parse");
        assert_eq!(parsed.feed_type, FeedType::JsonFeed);
        assert_eq!(parsed.items.len(), 1);
        assert_eq!(parsed.items[0].title, "JSON Item One");
    }

    #[test]
    fn dedup_key_prefers_external_id() {
        let bytes =
            std::fs::read(fixture_path("valid_json_feed.json")).expect("fixture must exist");
        let parsed =
            parse_feed("feed_1", &bytes, Some("application/feed+json")).expect("json parse");
        let (reason, key) = dedup_key(&parsed.items[0]);
        assert_eq!(reason, DedupReason::ExternalId);
        assert_eq!(key, "json-item-1");
    }

    #[test]
    fn dedup_key_uses_url_plus_published_before_url_only() {
        use chrono::{TimeZone, Utc};

        let mut item = parsed_item_template();
        item.external_item_id = None;
        item.canonical_url =
            Some(Url::parse("https://example.com/post?utm_source=newsletter").expect("url"));
        item.published_at =
            Some(Utc.with_ymd_and_hms(2026, 3, 20, 10, 0, 0).single().expect("datetime"));

        let (reason, key) = dedup_key(&item);
        assert_eq!(reason, DedupReason::UrlAndPublishedAt);
        assert!(key.contains("|2026-03-20T10:00:00+00:00"));
    }

    fn parsed_item_template() -> NormalizedItem {
        NormalizedItem {
            id: "itm_test".to_owned(),
            source_feed_id: "feed_1".to_owned(),
            external_item_id: Some("external".to_owned()),
            canonical_url: None,
            title: "Title".to_owned(),
            author: None,
            summary: None,
            content_html: None,
            content_text: None,
            published_at: None,
            updated_at: None,
            raw_hash: "hash".to_owned(),
            dedup_reason: None,
            duplicate_of_item_id: None,
        }
    }

    #[test]
    fn parses_rss_without_guid_using_fallback_id() {
        let bytes =
            std::fs::read(fixture_path("missing_guid_rss.xml")).expect("fixture must exist");
        let parsed = parse_feed("feed_1", &bytes, Some("application/rss+xml")).expect("rss parse");
        assert_eq!(parsed.items.len(), 1);
        assert!(parsed.items[0].external_item_id.is_none());
        assert!(parsed.items[0].id.starts_with("itm_"));
    }

    #[test]
    fn rejects_malformed_feed_fixture() {
        let bytes = std::fs::read(fixture_path("malformed_feed.xml")).expect("fixture must exist");
        let result = parse_feed("feed_1", &bytes, Some("application/rss+xml"));
        assert!(result.is_err());
    }

    #[test]
    fn infers_atom_from_text_xml_body_root() {
        let bytes = std::fs::read(fixture_path("valid_atom.xml")).expect("fixture must exist");
        let detected = detect_feed_type(&bytes, Some("text/xml; charset=utf-8"));
        assert_eq!(detected, FeedType::Atom);
    }
}
