//! Feed discovery pipeline for site URLs.

use std::collections::{HashMap, HashSet};

use async_trait::async_trait;
use models::{DiscoveredFeed, DiscoveryResult, DiscoverySource, FeedType};
use parser::{detect_feed_type, parse_feed};
use reqwest::redirect::Policy;
use scraper::{Html, Selector};
use thiserror::Error;
use url::Url;

/// HTTP response shape used by discovery clients.
#[derive(Debug, Clone)]
pub struct HttpResponse {
    /// Final URL after redirects.
    pub final_url: Url,
    /// HTTP status code.
    pub status: u16,
    /// Response content type, if present.
    pub content_type: Option<String>,
    /// Response body bytes.
    pub body: Vec<u8>,
}

/// HTTP abstraction for deterministic discovery tests.
#[async_trait]
pub trait DiscoveryHttpClient: Send + Sync {
    /// Fetch URL and return bytes/content type/final URL.
    async fn get(&self, url: &Url) -> Result<HttpResponse, DiscoveryError>;
}

/// `reqwest` implementation of discovery HTTP client.
pub struct ReqwestDiscoveryClient {
    client: reqwest::Client,
}

impl ReqwestDiscoveryClient {
    /// Build a discovery client with deterministic timeout and redirect behavior.
    pub fn new(timeout_secs: u64, user_agent: &str) -> Result<Self, DiscoveryError> {
        let client = reqwest::Client::builder()
            .user_agent(user_agent)
            .timeout(std::time::Duration::from_secs(timeout_secs))
            .redirect(Policy::limited(8))
            .build()
            .map_err(|err| DiscoveryError::Network(err.to_string()))?;
        Ok(Self { client })
    }
}

#[async_trait]
impl DiscoveryHttpClient for ReqwestDiscoveryClient {
    async fn get(&self, url: &Url) -> Result<HttpResponse, DiscoveryError> {
        let response = self
            .client
            .get(url.clone())
            .send()
            .await
            .map_err(|err| DiscoveryError::Network(err.to_string()))?;

        let status = response.status().as_u16();
        let final_url = response.url().clone();
        let content_type = response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .map(ToOwned::to_owned);
        let body = response
            .bytes()
            .await
            .map_err(|err| DiscoveryError::Network(err.to_string()))?
            .to_vec();

        Ok(HttpResponse { final_url, status, content_type, body })
    }
}

/// Discovery pipeline failures.
#[derive(Debug, Error)]
pub enum DiscoveryError {
    /// URL normalization failed.
    #[error("invalid url: {0}")]
    InvalidUrl(String),
    /// Network or HTTP-layer failure.
    #[error("network error: {0}")]
    Network(String),
    /// HTML parse error.
    #[error("html parse error")]
    HtmlParse,
}

/// Main discovery service.
pub struct DiscoveryService<C: DiscoveryHttpClient> {
    client: C,
}

#[derive(Debug)]
struct ScoredCandidate {
    feed: DiscoveredFeed,
    score: i32,
}

impl<C: DiscoveryHttpClient> DiscoveryService<C> {
    /// Build service from an HTTP client implementation.
    pub fn new(client: C) -> Self {
        Self { client }
    }

    /// Discover feed candidates for a site URL.
    pub async fn discover(&self, input: &str) -> Result<DiscoveryResult, DiscoveryError> {
        let normalized = normalize_site_url(input)?;
        let mut warnings = Vec::new();

        let html_response = self.client.get(&normalized).await?;
        if html_response.status >= 400 {
            warnings.push(format!(
                "site fetch returned HTTP {} for {}",
                html_response.status, html_response.final_url
            ));
        }

        let html = String::from_utf8_lossy(&html_response.body).to_string();
        let document = Html::parse_document(&html);
        let site_title = extract_site_title(&document);
        let site_icon_candidates = extract_icon_links(&document, &html_response.final_url);
        let autodiscovered = extract_autodiscovery_links(&document, &html_response.final_url);
        drop(document);

        let mut candidate_map: HashMap<String, ScoredCandidate> = HashMap::new();

        for (index, candidate) in autodiscovered.into_iter().enumerate() {
            let validated = self.validate_candidate(candidate).await;
            match validated {
                Some(mut feed) => {
                    let score = score_feed_candidate(
                        &feed.url,
                        feed.title.as_deref(),
                        feed.source,
                        index.saturating_add(1),
                    );
                    feed.confidence = confidence_from_score(score);
                    upsert_scored_candidate(&mut candidate_map, feed, score);
                }
                None => warnings.push("autodiscovery candidate validation failed".to_owned()),
            }
        }

        if candidate_map.is_empty() {
            for (index, candidate_url) in
                fallback_candidates(&html_response.final_url).into_iter().enumerate()
            {
                let candidate = DiscoveredFeed {
                    url: candidate_url,
                    title: None,
                    feed_type: FeedType::Unknown,
                    confidence: 0.0,
                    source: DiscoverySource::CommonPath,
                };
                if let Some(mut feed) = self.validate_candidate(candidate).await {
                    let score = score_feed_candidate(
                        &feed.url,
                        feed.title.as_deref(),
                        feed.source,
                        index.saturating_add(1),
                    );
                    feed.confidence = confidence_from_score(score);
                    upsert_scored_candidate(&mut candidate_map, feed, score);
                }
            }
        }

        let mut discovered_feeds: Vec<DiscoveredFeed> =
            candidate_map.into_values().map(|candidate| candidate.feed).collect();
        discovered_feeds.sort_by(|a, b| {
            b.confidence.total_cmp(&a.confidence).then_with(|| a.url.as_str().cmp(b.url.as_str()))
        });

        if discovered_feeds.is_empty() {
            warnings.push("no valid feed candidates discovered".to_owned());
        }

        Ok(DiscoveryResult {
            normalized_site_url: html_response.final_url,
            discovered_feeds,
            site_title,
            site_icon_candidates,
            warnings,
        })
    }

    async fn validate_candidate(&self, candidate: DiscoveredFeed) -> Option<DiscoveredFeed> {
        let response = self.client.get(&candidate.url).await.ok()?;
        if response.status >= 400 {
            return None;
        }

        let feed_type = detect_feed_type(&response.body, response.content_type.as_deref());
        if feed_type == FeedType::Unknown {
            return None;
        }

        let title = parse_feed("discovery_probe", &response.body, response.content_type.as_deref())
            .ok()
            .and_then(|parsed| parsed.title)
            .or(candidate.title.clone());

        Some(DiscoveredFeed {
            url: response.final_url,
            title,
            feed_type,
            confidence: candidate.confidence,
            source: candidate.source,
        })
    }
}

fn upsert_scored_candidate(
    candidate_map: &mut HashMap<String, ScoredCandidate>,
    feed: DiscoveredFeed,
    score: i32,
) {
    let key = feed.url.to_string();
    match candidate_map.get_mut(&key) {
        Some(existing) => {
            if score > existing.score {
                existing.feed = feed;
                existing.score = score;
            } else if existing.feed.title.is_none() && feed.title.is_some() {
                existing.feed.title = feed.title;
            }
        }
        None => {
            candidate_map.insert(key, ScoredCandidate { feed, score });
        }
    }
}

/// Normalize user-entered site URL.
pub fn normalize_site_url(input: &str) -> Result<Url, DiscoveryError> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err(DiscoveryError::InvalidUrl("empty input".to_owned()));
    }

    let with_scheme = if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
        trimmed.to_owned()
    } else {
        format!("https://{trimmed}")
    };

    let mut url =
        Url::parse(&with_scheme).map_err(|err| DiscoveryError::InvalidUrl(err.to_string()))?;
    url.set_fragment(None);

    if let Some(host) = url.host_str().map(|h| h.to_ascii_lowercase()) {
        let _ = url.set_host(Some(&host));
    }

    Ok(url)
}

fn extract_site_title(document: &Html) -> Option<String> {
    let selector = Selector::parse("title").ok()?;
    document
        .select(&selector)
        .next()
        .map(|node| node.text().collect::<String>().trim().to_owned())
        .filter(|v| !v.is_empty())
}

fn extract_autodiscovery_links(document: &Html, base_url: &Url) -> Vec<DiscoveredFeed> {
    let mut output = Vec::new();
    let selector = match Selector::parse("link[rel]") {
        Ok(selector) => selector,
        Err(_) => return output,
    };

    for node in document.select(&selector) {
        let rel = node.value().attr("rel").unwrap_or_default().to_ascii_lowercase();
        if !rel.split_whitespace().any(|part| part == "alternate") {
            continue;
        }

        let feed_type =
            match node.value().attr("type").unwrap_or_default().to_ascii_lowercase().as_str() {
                "application/rss+xml" => FeedType::Rss,
                "application/atom+xml" => FeedType::Atom,
                "application/feed+json" | "application/json" => FeedType::JsonFeed,
                _ => continue,
            };

        let href = match node.value().attr("href") {
            Some(value) => value,
            None => continue,
        };
        let url = match base_url.join(href) {
            Ok(url) => url,
            Err(_) => continue,
        };
        let title =
            node.value().attr("title").map(ToOwned::to_owned).filter(|v| !v.trim().is_empty());

        output.push(DiscoveredFeed {
            url,
            title,
            feed_type,
            confidence: 0.9,
            source: DiscoverySource::Autodiscovery,
        });
    }

    output
}

fn extract_icon_links(document: &Html, base_url: &Url) -> Vec<Url> {
    let selector = match Selector::parse("link[rel][href]") {
        Ok(selector) => selector,
        Err(_) => return Vec::new(),
    };
    let mut seen = HashSet::new();
    let mut icons = Vec::new();

    for node in document.select(&selector) {
        let rel = node.value().attr("rel").unwrap_or_default().to_ascii_lowercase();
        let is_icon = rel.contains("icon") || rel.contains("apple-touch-icon");
        if !is_icon {
            continue;
        }
        let href = match node.value().attr("href") {
            Some(v) => v,
            None => continue,
        };
        let Ok(url) = base_url.join(href) else {
            continue;
        };
        if seen.insert(url.to_string()) {
            icons.push(url);
        }
    }

    icons
}

fn fallback_candidates(site_url: &Url) -> Vec<Url> {
    const PATHS: &[&str] = &["/feed", "/rss", "/rss.xml", "/feed.xml", "/atom.xml", "/index.xml"];
    PATHS.iter().filter_map(|path| site_url.join(path).ok()).collect()
}

fn score_feed_candidate(
    url: &Url,
    title: Option<&str>,
    source: DiscoverySource,
    order_found: usize,
) -> i32 {
    let mut score = match source {
        DiscoverySource::Manual => 1000,
        DiscoverySource::Autodiscovery => 50,
        DiscoverySource::CommonPath => 10,
    };

    score -= (order_found.saturating_sub(1) as i32) * 5;

    let url_lower = url.as_str().to_ascii_lowercase();
    if url_lower.contains("comments") {
        score -= 10;
    }
    if url_lower.contains("podcast") {
        score -= 10;
    }
    if url_lower.contains("rss") {
        score += 5;
    }
    if url_lower.ends_with("/index.xml") {
        score += 5;
    }
    if url_lower.ends_with("/feed/") {
        score += 5;
    }
    if url_lower.ends_with("/feed") {
        score += 4;
    }
    if url_lower.contains("json") {
        score += 3;
    }

    if let Some(title) = title.map(|value| value.to_ascii_lowercase()) {
        if title.contains("comments") {
            score -= 10;
        }
    }

    score
}

fn confidence_from_score(score: i32) -> f32 {
    let normalized = (score + 30) as f32 / 120.0;
    normalized.clamp(0.15, 0.99)
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use async_trait::async_trait;
    use models::FeedType;

    use super::{
        DiscoveryError, DiscoveryHttpClient, DiscoveryService, DiscoverySource, HttpResponse,
        confidence_from_score, normalize_site_url, score_feed_candidate,
    };

    struct MockClient {
        responses: HashMap<String, HttpResponse>,
    }

    #[async_trait]
    impl DiscoveryHttpClient for MockClient {
        async fn get(&self, url: &url::Url) -> Result<HttpResponse, DiscoveryError> {
            self.responses
                .get(url.as_str())
                .cloned()
                .ok_or_else(|| DiscoveryError::Network(format!("missing mock for {}", url)))
        }
    }

    #[tokio::test]
    async fn discovers_autodiscovery_feed() {
        let site = url::Url::parse("https://example.com/").expect("url parse");
        let feed = url::Url::parse("https://example.com/feed.xml").expect("url parse");

        let html = r#"
<!doctype html>
<html>
  <head>
    <title>Example Blog</title>
    <link rel="alternate" type="application/rss+xml" href="/feed.xml" title="Main Feed" />
    <link rel="icon" href="/favicon.ico" />
  </head>
</html>
"#;

        let rss = r#"
<?xml version="1.0" encoding="utf-8"?>
<rss version="2.0">
  <channel>
    <title>Example Feed</title>
    <link>https://example.com/</link>
    <description>Sample</description>
    <item>
      <title>One</title>
      <link>https://example.com/posts/one</link>
      <guid>item-1</guid>
      <pubDate>Tue, 03 Jan 2023 10:00:00 +0000</pubDate>
    </item>
  </channel>
</rss>
"#;

        let responses = HashMap::from([
            (
                site.to_string(),
                HttpResponse {
                    final_url: site.clone(),
                    status: 200,
                    content_type: Some("text/html".to_owned()),
                    body: html.as_bytes().to_vec(),
                },
            ),
            (
                feed.to_string(),
                HttpResponse {
                    final_url: feed.clone(),
                    status: 200,
                    content_type: Some("application/rss+xml".to_owned()),
                    body: rss.as_bytes().to_vec(),
                },
            ),
        ]);

        let service = DiscoveryService::new(MockClient { responses });
        let result = service.discover("example.com").await.expect("discover");

        assert_eq!(result.normalized_site_url.as_str(), "https://example.com/");
        assert_eq!(result.discovered_feeds.len(), 1);
        assert_eq!(result.discovered_feeds[0].feed_type, FeedType::Rss);
        assert_eq!(result.site_title.as_deref(), Some("Example Blog"));
        assert_eq!(result.site_icon_candidates.len(), 1);
    }

    #[test]
    fn normalizes_url_and_drops_fragment() {
        let normalized = normalize_site_url("example.com/path#tracking").expect("normalize");
        assert_eq!(normalized.as_str(), "https://example.com/path");
    }

    #[tokio::test]
    async fn resolves_relative_autodiscovery_links() {
        let site = url::Url::parse("https://example.com/blog/").expect("url parse");
        let feed = url::Url::parse("https://example.com/blog/feeds/site.rss").expect("url parse");

        let html = std::fs::read_to_string(
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../../tests/discovery/site_relative_links.html"),
        )
        .expect("fixture html");
        let rss = std::fs::read(
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../../tests/feeds/valid_rss.xml"),
        )
        .expect("fixture rss");

        let responses = HashMap::from([
            (
                site.to_string(),
                HttpResponse {
                    final_url: site.clone(),
                    status: 200,
                    content_type: Some("text/html".to_owned()),
                    body: html.into_bytes(),
                },
            ),
            (
                feed.to_string(),
                HttpResponse {
                    final_url: feed.clone(),
                    status: 200,
                    content_type: Some("application/rss+xml".to_owned()),
                    body: rss,
                },
            ),
        ]);

        let service = DiscoveryService::new(MockClient { responses });
        let result = service.discover("https://example.com/blog/").await.expect("discover");
        assert_eq!(result.discovered_feeds.len(), 1);
        assert_eq!(result.discovered_feeds[0].url, feed);
    }

    #[test]
    fn feed_candidate_scoring_penalizes_comment_feed_urls() {
        let primary = url::Url::parse("https://example.com/feed").expect("url");
        let comments = url::Url::parse("https://example.com/comments/feed").expect("url");

        let primary_score =
            score_feed_candidate(&primary, Some("Main Feed"), DiscoverySource::Autodiscovery, 1);
        let comments_score = score_feed_candidate(
            &comments,
            Some("Comments Feed"),
            DiscoverySource::Autodiscovery,
            1,
        );

        assert!(primary_score > comments_score);
    }

    #[test]
    fn confidence_mapping_is_bounded() {
        assert!((0.15..=0.99).contains(&confidence_from_score(-200)));
        assert!((0.15..=0.99).contains(&confidence_from_score(500)));
    }
}
