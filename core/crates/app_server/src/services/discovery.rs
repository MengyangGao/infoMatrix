use std::collections::{HashMap, HashSet};

use models::FeedType;
use parser::{ParsedFeed, parse_feed};
use reqwest::Client;
use scraper::{Html, Selector};
use serde::{Deserialize, Serialize};
use shared_api::labels::confidence_from_score;
use shared_api::url::{enforce_web_url, normalize_input_url};
use tokio::task::JoinSet;
use url::Url;

use crate::config::AppContext;
use crate::error::ApiError;
use crate::state::build_http_client;
use crate::views::{DiscoverResponse, DiscoveredFeedView};

pub(crate) const DISCOVERY_CACHE_TTL_HOURS: i64 = 24;

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct DiscoveryCachePayload {
    pub(crate) normalized_site_url: String,
    pub(crate) site_title: Option<String>,
    pub(crate) warnings: Vec<String>,
    pub(crate) candidates: Vec<DiscoveryCacheCandidate>,
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct DiscoveryCacheCandidate {
    pub(crate) url: String,
    pub(crate) title: Option<String>,
    pub(crate) feed_type: FeedType,
    pub(crate) confidence: f32,
    pub(crate) source: String,
    pub(crate) score: i32,
    pub(crate) http_status: u16,
    pub(crate) etag: Option<String>,
    pub(crate) last_modified: Option<String>,
    pub(crate) duration_ms: u128,
    pub(crate) parsed_feed: ParsedFeed,
}

#[derive(Debug, Clone)]
pub(crate) struct CandidateHint {
    pub(crate) url: Url,
    pub(crate) fallback_type: FeedType,
    pub(crate) fallback_title: Option<String>,
    pub(crate) source: String,
    pub(crate) order_found: usize,
}

#[derive(Debug)]
pub(crate) struct DiscoveryRun {
    pub(crate) response: DiscoverResponse,
    pub(crate) cache: DiscoveryCachePayload,
}

pub(crate) async fn discover_site_with_reqwest(
    context: &AppContext,
    site_url: &str,
) -> Result<DiscoveryRun, ApiError> {
    let client = build_http_client(context)?;
    discover_site_with_client(&client, site_url).await
}

pub(crate) async fn discover_site_with_client(
    client: &Client,
    site_url: &str,
) -> Result<DiscoveryRun, ApiError> {
    let normalized_input = normalize_input_url(site_url)?;
    let normalized_site_url = Url::parse(&normalized_input)
        .map_err(|err| ApiError::BadRequest(format!("invalid site url: {err}")))?;
    enforce_web_url(&normalized_site_url, "site url")?;
    let page_candidates = candidate_site_urls(&normalized_site_url);
    let mut warnings = Vec::new();
    let (final_site_url, site_title, mut candidates) =
        match fetch_first_successful_response(client, &page_candidates).await {
            Ok(page_response) => {
                let final_site_url = page_response.url().clone();
                let html = page_response
                    .text()
                    .await
                    .map_err(|err| ApiError::Internal(err.to_string()))?;
                let (site_title, candidates) =
                    collect_html_feed_candidates(&html, &final_site_url)?;
                (final_site_url, site_title, candidates)
            }
            Err(err) => {
                warnings.push(format!("entry page fetch failed: {err}"));
                (normalized_site_url.clone(), None, Vec::new())
            }
        };

    for candidate_url in fallback_candidate_urls(&normalized_site_url) {
        if !candidates.iter().any(|candidate| candidate.url == candidate_url) {
            candidates.push(CandidateHint {
                url: candidate_url,
                fallback_type: FeedType::Unknown,
                fallback_title: None,
                source: "commonpath".to_owned(),
                order_found: candidates.len().saturating_add(1),
            });
        }
    }

    for candidate_url in fallback_candidate_urls(&final_site_url) {
        if !candidates.iter().any(|candidate| candidate.url == candidate_url) {
            candidates.push(CandidateHint {
                url: candidate_url,
                fallback_type: FeedType::Unknown,
                fallback_title: None,
                source: "commonpath".to_owned(),
                order_found: candidates.len().saturating_add(1),
            });
        }
    }

    let mut discovered_map: HashMap<String, DiscoveryCacheCandidate> = HashMap::new();
    let mut join_set = JoinSet::new();

    for candidate in candidates {
        let candidate_client = client.clone();
        join_set.spawn(async move {
            probe_discovered_feed_candidate(candidate_client, candidate).await
        });
    }

    while let Some(join_result) = join_set.join_next().await {
        match join_result {
            Ok(Ok(Some(discovered))) => {
                let key = discovered.url.clone();
                match discovered_map.get_mut(&key) {
                    Some(existing) => {
                        if discovered.score > existing.score {
                            *existing = discovered;
                        } else if existing.title.is_none() && discovered.title.is_some() {
                            existing.title = discovered.title.clone();
                        }
                    }
                    None => {
                        discovered_map.insert(key, discovered);
                    }
                }
            }
            Ok(Ok(None)) => {}
            Ok(Err(warning)) => warnings.push(warning),
            Err(err) => warnings.push(format!("candidate task failed: {err}")),
        }
    }

    let mut discovered_candidates: Vec<DiscoveryCacheCandidate> =
        discovered_map.into_values().collect();
    discovered_candidates.sort_by(|a, b| {
        b.score
            .cmp(&a.score)
            .then_with(|| b.confidence.total_cmp(&a.confidence))
            .then_with(|| a.url.cmp(&b.url))
    });

    let discovered_feeds = discovered_candidates
        .iter()
        .map(|candidate| DiscoveredFeedView {
            url: candidate.url.clone(),
            title: candidate.title.clone(),
            feed_type: candidate.feed_type,
            confidence: candidate.confidence,
            source: candidate.source.clone(),
            score: candidate.score,
        })
        .collect();

    let response = DiscoverResponse {
        normalized_site_url: final_site_url.to_string(),
        discovered_feeds,
        site_title,
        warnings,
    };
    let cache = DiscoveryCachePayload {
        normalized_site_url: response.normalized_site_url.clone(),
        site_title: response.site_title.clone(),
        warnings: response.warnings.clone(),
        candidates: discovered_candidates,
    };

    Ok(DiscoveryRun { response, cache })
}

pub(crate) async fn probe_discovered_feed_candidate(
    client: Client,
    candidate: CandidateHint,
) -> Result<Option<DiscoveryCacheCandidate>, String> {
    let start = std::time::Instant::now();
    let response = client
        .get(candidate.url.clone())
        .send()
        .await
        .map_err(|_| format!("candidate fetch failed: {}", candidate.url))?;
    let duration_ms = start.elapsed().as_millis();
    let response_status = response.status().as_u16();
    if response_status >= 400 {
        return Ok(None);
    }

    let final_url = response.url().clone();
    let content_type = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .map(ToOwned::to_owned);
    let etag = response
        .headers()
        .get(reqwest::header::ETAG)
        .and_then(|value| value.to_str().ok())
        .map(ToOwned::to_owned);
    let last_modified = response
        .headers()
        .get(reqwest::header::LAST_MODIFIED)
        .and_then(|value| value.to_str().ok())
        .map(ToOwned::to_owned);
    let bytes =
        response.bytes().await.map_err(|err| format!("candidate body read failed: {err}"))?;
    let parsed_feed = parse_feed("discovery_probe", &bytes, content_type.as_deref())
        .map_err(|err| err.to_string())?;
    let feed_type = if parsed_feed.feed_type == FeedType::Unknown {
        candidate.fallback_type
    } else {
        parsed_feed.feed_type
    };
    if feed_type == FeedType::Unknown {
        return Ok(None);
    }

    let parsed_title = parsed_feed.title.clone().or(candidate.fallback_title.clone());
    let score = feed_candidate_score(
        final_url.as_str(),
        parsed_title.as_deref(),
        &candidate.source,
        candidate.order_found,
    );
    Ok(Some(DiscoveryCacheCandidate {
        url: final_url.to_string(),
        title: parsed_title,
        feed_type,
        confidence: confidence_from_score(score),
        source: candidate.source,
        score,
        http_status: response_status,
        etag,
        last_modified,
        duration_ms,
        parsed_feed,
    }))
}

pub(crate) fn cached_discovery_candidate_for_feed_url<'a>(
    cache: &'a DiscoveryCachePayload,
    feed_url: &Url,
) -> Option<&'a DiscoveryCacheCandidate> {
    cache.candidates.iter().find(|candidate| candidate.url == feed_url.as_str())
}

pub(crate) fn collect_html_feed_candidates(
    html: &str,
    base_url: &Url,
) -> Result<(Option<String>, Vec<CandidateHint>), ApiError> {
    let document = Html::parse_document(html);
    let site_title = Selector::parse("title")
        .ok()
        .and_then(|selector| {
            document
                .select(&selector)
                .next()
                .map(|node| node.text().collect::<String>().trim().to_owned())
        })
        .filter(|value| !value.is_empty());

    let link_selector = Selector::parse("link[rel][href]")
        .map_err(|_| ApiError::Internal("selector parse failed".to_owned()))?;
    let mut candidates: Vec<CandidateHint> = Vec::new();
    let mut order_found = 0usize;

    for node in document.select(&link_selector) {
        let rel = node.value().attr("rel").unwrap_or_default().to_ascii_lowercase();
        if !rel.split_whitespace().any(|value| value == "alternate") {
            continue;
        }
        let feed_type =
            match node.value().attr("type").unwrap_or_default().to_ascii_lowercase().as_str() {
                "application/rss+xml" => FeedType::Rss,
                "application/atom+xml" => FeedType::Atom,
                "application/feed+json" | "application/json" => FeedType::JsonFeed,
                _ => continue,
            };

        let Some(href) = node.value().attr("href") else {
            continue;
        };
        let Ok(url) = base_url.join(href) else {
            continue;
        };
        let title = node.value().attr("title").map(ToOwned::to_owned);
        order_found = order_found.saturating_add(1);
        candidates.push(CandidateHint {
            url,
            fallback_type: feed_type,
            fallback_title: title,
            source: "autodiscovery".to_owned(),
            order_found,
        });
    }

    if let Ok(anchor_selector) = Selector::parse("a[href]") {
        for node in document.select(&anchor_selector) {
            let Some(href) = node.value().attr("href") else {
                continue;
            };
            let href_lower = href.to_ascii_lowercase();
            let looks_like_feed = href_lower.contains("atom")
                || href_lower.contains("rss")
                || href_lower.contains("feed");
            if !looks_like_feed {
                continue;
            }
            let Ok(url) = base_url.join(href) else {
                continue;
            };
            order_found = order_found.saturating_add(1);
            candidates.push(CandidateHint {
                url,
                fallback_type: FeedType::Unknown,
                fallback_title: None,
                source: "heuristic_link".to_owned(),
                order_found,
            });
        }
    }

    Ok((site_title, candidates))
}

pub(crate) fn source_base_score(source: &str) -> i32 {
    match source {
        "manual" | "direct_feed" => 1000,
        "autodiscovery" => 50,
        "heuristic_link" => 15,
        "commonpath" => 10,
        _ => 0,
    }
}

pub(crate) fn feed_candidate_score(
    url: &str,
    title: Option<&str>,
    source: &str,
    order_found: usize,
) -> i32 {
    let mut score = source_base_score(source);
    score -= (order_found.saturating_sub(1) as i32) * 5;

    let url_lower = url.to_ascii_lowercase();
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

pub(crate) fn candidate_site_urls(base: &Url) -> Vec<Url> {
    let mut output = Vec::new();
    let mut seen = HashSet::new();

    let mut push_unique = |url: Url| {
        let key = url.to_string();
        if seen.insert(key) {
            output.push(url);
        }
    };

    push_unique(base.clone());

    if base.scheme() == "https" {
        let mut alt = base.clone();
        if alt.set_scheme("http").is_ok() {
            push_unique(alt);
        }
    } else if base.scheme() == "http" {
        let mut alt = base.clone();
        if alt.set_scheme("https").is_ok() {
            push_unique(alt);
        }
    }

    if let Some(domain) = base.domain() {
        if let Some(stripped) = domain.strip_prefix("www.") {
            let mut alt = base.clone();
            if alt.set_host(Some(stripped)).is_ok() {
                push_unique(alt);
            }
        } else {
            let mut alt = base.clone();
            if alt.set_host(Some(&format!("www.{domain}"))).is_ok() {
                push_unique(alt);
            }
        }
    }

    output
}

pub(crate) async fn fetch_first_successful_response(
    client: &Client,
    candidates: &[Url],
) -> Result<reqwest::Response, ApiError> {
    let mut errors = Vec::new();

    for candidate in candidates {
        match client.get(candidate.clone()).send().await {
            Ok(response) if response.status().is_success() => return Ok(response),
            Ok(response) => {
                errors.push(format!("{} (HTTP {})", candidate, response.status().as_u16()));
            }
            Err(err) => {
                errors.push(format!("{} ({})", candidate, summarize_reqwest_error(&err)));
            }
        }
    }

    let detail = if errors.is_empty() {
        "no candidate url attempted".to_owned()
    } else {
        errors.into_iter().take(3).collect::<Vec<_>>().join("; ")
    };
    Err(ApiError::BadRequest(format!(
        "could not reach website url. tried alternate urls and all failed: {detail}"
    )))
}

pub(crate) fn summarize_reqwest_error(err: &reqwest::Error) -> String {
    if err.is_timeout() {
        return "request timeout".to_owned();
    }
    if err.is_connect() {
        return "connection failure".to_owned();
    }
    if err.is_builder() {
        return "request build failure".to_owned();
    }
    if err.is_request() {
        return "request failure".to_owned();
    }
    err.to_string()
}

pub(crate) fn fallback_candidate_urls(site_url: &Url) -> Vec<Url> {
    const ROOT_PATHS: &[&str] = &[
        "/feed",
        "/rss",
        "/rss.xml",
        "/feed.xml",
        "/atom.xml",
        "/index.xml",
        "/posts.atom",
        "/blog/feed",
        "/blog/rss",
        "/blog/rss.xml",
        "/blog/feed.xml",
        "/blog/atom.xml",
        "/blog/index.xml",
        "/blog/posts.atom",
    ];
    const RELATIVE_PATHS: &[&str] =
        &["feed", "rss", "rss.xml", "feed.xml", "atom.xml", "index.xml", "posts.atom"];

    let mut output = Vec::new();
    let mut relative_bases = vec![site_url.clone()];
    if !site_url.path().ends_with('/') {
        let mut with_slash = site_url.clone();
        with_slash.set_path(&format!("{}/", site_url.path()));
        relative_bases.push(with_slash);
    }

    for path in ROOT_PATHS {
        if let Ok(url) = site_url.join(path) {
            output.push(url);
        }
    }
    for base in relative_bases {
        for path in RELATIVE_PATHS {
            if let Ok(url) = base.join(path) {
                output.push(url);
            }
        }
    }
    output
}
