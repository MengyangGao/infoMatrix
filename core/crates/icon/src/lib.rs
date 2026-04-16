//! Icon discovery, ranking, and fetch helpers.

use std::collections::HashSet;

use async_trait::async_trait;
use reqwest::redirect::Policy;
use scraper::{Html, Selector};
use thiserror::Error;
use url::Url;

/// Icon source type used for ranking.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IconSource {
    /// Feed-level image metadata.
    FeedImage,
    /// HTML icon relation.
    HtmlIcon,
    /// Apple touch icon relation.
    AppleTouchIcon,
    /// Open Graph fallback image.
    OpenGraphImage,
}

/// Candidate icon URL with ranking metadata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IconCandidate {
    /// Candidate URL.
    pub url: Url,
    /// Source class.
    pub source: IconSource,
    /// Optional declared width/height hint.
    pub size_hint: Option<(u32, u32)>,
}

/// Fetch result for icon bytes.
#[derive(Debug, Clone)]
pub struct IconAsset {
    /// Source URL.
    pub url: Url,
    /// Content type.
    pub content_type: Option<String>,
    /// Byte payload.
    pub bytes: Vec<u8>,
}

/// Icon fetch errors.
#[derive(Debug, Error)]
pub enum IconError {
    /// HTTP/network failure.
    #[error("network error: {0}")]
    Network(String),
    /// Response status not acceptable.
    #[error("unexpected status: {0}")]
    UnexpectedStatus(u16),
    /// Payload too large.
    #[error("icon payload exceeds limit")]
    TooLarge,
}

/// HTTP abstraction for icon fetch tests.
#[async_trait]
pub trait IconHttpClient: Send + Sync {
    /// GET bytes for icon URL.
    async fn get(&self, url: &Url) -> Result<IconAsset, IconError>;
}

/// Reqwest-based icon fetch client.
pub struct ReqwestIconClient {
    client: reqwest::Client,
}

impl ReqwestIconClient {
    /// Build a new client with timeout and redirect policy.
    pub fn new(timeout_secs: u64, user_agent: &str) -> Result<Self, IconError> {
        let client = reqwest::Client::builder()
            .user_agent(user_agent)
            .timeout(std::time::Duration::from_secs(timeout_secs))
            .redirect(Policy::limited(8))
            .build()
            .map_err(|err| IconError::Network(err.to_string()))?;
        Ok(Self { client })
    }
}

#[async_trait]
impl IconHttpClient for ReqwestIconClient {
    async fn get(&self, url: &Url) -> Result<IconAsset, IconError> {
        let response = self
            .client
            .get(url.clone())
            .send()
            .await
            .map_err(|err| IconError::Network(err.to_string()))?;

        if !response.status().is_success() {
            return Err(IconError::UnexpectedStatus(response.status().as_u16()));
        }

        let final_url = response.url().clone();
        let content_type = response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .map(ToOwned::to_owned);
        let bytes =
            response.bytes().await.map_err(|err| IconError::Network(err.to_string()))?.to_vec();

        Ok(IconAsset { url: final_url, content_type, bytes })
    }
}

/// Extract and rank icon candidates from feed and HTML metadata.
pub fn extract_icon_candidates(
    base_url: &Url,
    html: &str,
    feed_image: Option<&Url>,
) -> Vec<IconCandidate> {
    let document = Html::parse_document(html);
    let mut seen = HashSet::new();
    let mut candidates = Vec::new();

    if let Some(feed_image) = feed_image {
        seen.insert(feed_image.to_string());
        candidates.push(IconCandidate {
            url: feed_image.clone(),
            source: IconSource::FeedImage,
            size_hint: None,
        });
    }

    let link_selector = match Selector::parse("link[rel][href]") {
        Ok(value) => value,
        Err(_) => return candidates,
    };

    for node in document.select(&link_selector) {
        let rel = node.value().attr("rel").unwrap_or_default().to_ascii_lowercase();
        let href = match node.value().attr("href") {
            Some(value) => value,
            None => continue,
        };

        let source = if rel.contains("apple-touch-icon") {
            Some(IconSource::AppleTouchIcon)
        } else if rel.contains("icon") {
            Some(IconSource::HtmlIcon)
        } else {
            None
        };
        let Some(source) = source else { continue };

        let Ok(url) = base_url.join(href) else {
            continue;
        };
        if !seen.insert(url.to_string()) {
            continue;
        }

        let size_hint = node.value().attr("sizes").and_then(parse_sizes_attribute);

        candidates.push(IconCandidate { url, source, size_hint });
    }

    let meta_selector = match Selector::parse("meta[property='og:image'][content]") {
        Ok(value) => value,
        Err(_) => return sort_candidates(candidates),
    };

    for node in document.select(&meta_selector) {
        let Some(content) = node.value().attr("content") else {
            continue;
        };
        let Ok(url) = base_url.join(content) else {
            continue;
        };
        if !seen.insert(url.to_string()) {
            continue;
        }
        candidates.push(IconCandidate { url, source: IconSource::OpenGraphImage, size_hint: None });
    }

    sort_candidates(candidates)
}

/// Download and validate icon payload.
pub async fn fetch_icon<C: IconHttpClient>(
    client: &C,
    candidate: &IconCandidate,
    max_bytes: usize,
) -> Result<IconAsset, IconError> {
    let asset = client.get(&candidate.url).await?;

    if asset.bytes.len() > max_bytes {
        return Err(IconError::TooLarge);
    }

    if let Some(content_type) = asset.content_type.as_deref() {
        if !content_type.to_ascii_lowercase().starts_with("image/") {
            return Err(IconError::Network(format!("unexpected content-type {content_type}")));
        }
    }

    Ok(asset)
}

fn parse_sizes_attribute(value: &str) -> Option<(u32, u32)> {
    let first = value.split_whitespace().next()?;
    let (w, h) = first.split_once('x')?;
    let width = w.parse::<u32>().ok()?;
    let height = h.parse::<u32>().ok()?;
    Some((width, height))
}

fn sort_candidates(mut candidates: Vec<IconCandidate>) -> Vec<IconCandidate> {
    candidates.sort_by(|a, b| {
        let left_priority = source_priority(a.source);
        let right_priority = source_priority(b.source);
        right_priority
            .cmp(&left_priority)
            .then_with(|| candidate_size_score(b).cmp(&candidate_size_score(a)))
    });
    candidates
}

fn candidate_size_score(candidate: &IconCandidate) -> u32 {
    candidate
        .size_hint
        .map(|(w, h)| {
            let ratio = if w > h { w as f32 / h as f32 } else { h as f32 / w as f32 };
            if ratio <= 1.25 { w.saturating_mul(h) } else { 0 }
        })
        .unwrap_or(0)
}

fn source_priority(source: IconSource) -> i32 {
    match source {
        IconSource::FeedImage => 4,
        IconSource::HtmlIcon => 3,
        IconSource::AppleTouchIcon => 2,
        IconSource::OpenGraphImage => 1,
    }
}

#[cfg(test)]
mod tests {
    use super::{IconSource, extract_icon_candidates};

    #[test]
    fn ranks_feed_image_above_html_candidates() {
        let base = url::Url::parse("https://example.com/").expect("url parse");
        let html = r#"
<html>
  <head>
    <link rel="icon" href="/favicon-16x16.png" sizes="16x16" />
    <link rel="apple-touch-icon" href="/apple-touch-icon.png" sizes="180x180" />
    <meta property="og:image" content="/og-image.jpg" />
  </head>
</html>
"#;
        let feed_image =
            url::Url::parse("https://cdn.example.com/feed-icon.png").expect("url parse");

        let candidates = extract_icon_candidates(&base, html, Some(&feed_image));
        assert!(!candidates.is_empty());
        assert_eq!(candidates[0].source, IconSource::FeedImage);
    }
}
