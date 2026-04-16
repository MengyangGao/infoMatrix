//! Feed fetch pipeline with conditional HTTP support and scheduling policy.

use std::time::Duration;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use models::FeedHealthState;
use parser::ParsedFeed;
use reqwest::header::{ETAG, IF_MODIFIED_SINCE, IF_NONE_MATCH, LAST_MODIFIED};
use reqwest::redirect::Policy;
use thiserror::Error;
use url::Url;

/// Request metadata for feed fetching.
#[derive(Debug, Clone)]
pub struct FetchRequest {
    /// Feed URL.
    pub url: Url,
    /// Existing ETag for conditional requests.
    pub etag: Option<String>,
    /// Existing Last-Modified value for conditional requests.
    pub last_modified: Option<String>,
}

/// Response metadata for feed fetch requests.
#[derive(Debug, Clone)]
pub struct FetchResponse {
    /// Final URL after redirects.
    pub final_url: Url,
    /// HTTP status.
    pub status: u16,
    /// Response bytes when status has body.
    pub body: Vec<u8>,
    /// Content-Type header.
    pub content_type: Option<String>,
    /// New ETag header value.
    pub etag: Option<String>,
    /// New Last-Modified header value.
    pub last_modified: Option<String>,
    /// End-to-end request duration in ms.
    pub duration_ms: u128,
}

/// Fetch results after parse integration.
#[derive(Debug, Clone)]
pub enum FetchOutcome {
    /// Resource not modified (HTTP 304).
    NotModified(FetchResponse),
    /// Feed updated and successfully parsed.
    Updated { response: FetchResponse, parsed: ParsedFeed },
}

/// HTTP abstraction for fetch logic.
#[async_trait]
pub trait FeedHttpClient: Send + Sync {
    /// Execute feed fetch request.
    async fn fetch(&self, request: &FetchRequest) -> Result<FetchResponse, FetchError>;
}

/// Reqwest HTTP client for fetching feeds.
pub struct ReqwestFeedClient {
    client: reqwest::Client,
}

impl ReqwestFeedClient {
    /// Build client with timeout and redirect constraints.
    pub fn new(timeout_secs: u64, user_agent: &str) -> Result<Self, FetchError> {
        let client = reqwest::Client::builder()
            .user_agent(user_agent)
            .timeout(Duration::from_secs(timeout_secs))
            .no_proxy()
            .redirect(Policy::limited(8))
            .build()
            .map_err(|err| FetchError::Network(err.to_string()))?;
        Ok(Self { client })
    }
}

#[async_trait]
impl FeedHttpClient for ReqwestFeedClient {
    async fn fetch(&self, request: &FetchRequest) -> Result<FetchResponse, FetchError> {
        let mut req = self.client.get(request.url.clone());
        if let Some(etag) = request.etag.as_ref() {
            req = req.header(IF_NONE_MATCH, etag);
        }
        if let Some(last_modified) = request.last_modified.as_ref() {
            req = req.header(IF_MODIFIED_SINCE, last_modified);
        }

        let start = std::time::Instant::now();
        let response = req.send().await.map_err(|err| FetchError::Network(err.to_string()))?;
        let duration_ms = start.elapsed().as_millis();

        let status = response.status().as_u16();
        let final_url = response.url().clone();
        let content_type = response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .map(ToOwned::to_owned);
        let etag = response
            .headers()
            .get(ETAG)
            .and_then(|value| value.to_str().ok())
            .map(ToOwned::to_owned);
        let last_modified = response
            .headers()
            .get(LAST_MODIFIED)
            .and_then(|value| value.to_str().ok())
            .map(ToOwned::to_owned);
        let body =
            response.bytes().await.map_err(|err| FetchError::Network(err.to_string()))?.to_vec();

        Ok(FetchResponse {
            final_url,
            status,
            body,
            content_type,
            etag,
            last_modified,
            duration_ms,
        })
    }
}

/// Fetch errors.
#[derive(Debug, Error)]
pub enum FetchError {
    /// HTTP/network failure.
    #[error("network error: {0}")]
    Network(String),
    /// Parse failure.
    #[error("parse error: {0}")]
    Parse(String),
    /// Unexpected HTTP status.
    #[error("unexpected status: {0}")]
    UnexpectedStatus(u16),
}

/// Feed fetch orchestration service.
pub struct FetchService<C: FeedHttpClient> {
    client: C,
}

impl<C: FeedHttpClient> FetchService<C> {
    /// Build service from HTTP client.
    pub fn new(client: C) -> Self {
        Self { client }
    }

    /// Execute conditional fetch and parse feed if updated.
    pub async fn fetch_and_parse(
        &self,
        source_feed_id: &str,
        request: &FetchRequest,
    ) -> Result<FetchOutcome, FetchError> {
        let response = self.client.fetch(request).await?;

        match response.status {
            304 => Ok(FetchOutcome::NotModified(response)),
            200..=299 => {
                let parsed = parser::parse_feed(
                    source_feed_id,
                    &response.body,
                    response.content_type.as_deref(),
                )
                .map_err(|err| FetchError::Parse(err.to_string()))?;
                Ok(FetchOutcome::Updated { response, parsed })
            }
            status => Err(FetchError::UnexpectedStatus(status)),
        }
    }
}

/// Compute next fetch time using feed health and failure backoff.
pub fn compute_next_scheduled_fetch(
    now: DateTime<Utc>,
    is_new_feed: bool,
    health_state: FeedHealthState,
    failure_count: u32,
) -> DateTime<Utc> {
    let base = if is_new_feed {
        Duration::from_secs(15 * 60)
    } else {
        match health_state {
            FeedHealthState::Healthy => Duration::from_secs(60 * 60),
            FeedHealthState::Stale => Duration::from_secs(3 * 60 * 60),
            FeedHealthState::Failing => Duration::from_secs(6 * 60 * 60),
        }
    };

    let multiplier = 2u32.saturating_pow(failure_count.min(6));
    let capped = (base.as_secs() * multiplier as u64).min(24 * 60 * 60);
    now + chrono::TimeDelta::seconds(capped as i64)
}

#[cfg(test)]
mod tests {
    use async_trait::async_trait;
    use chrono::{TimeDelta, Utc};
    use models::FeedHealthState;

    use super::{
        FeedHttpClient, FetchError, FetchOutcome, FetchRequest, FetchResponse, FetchService,
        compute_next_scheduled_fetch,
    };

    struct MockClient {
        response: FetchResponse,
    }

    #[async_trait]
    impl FeedHttpClient for MockClient {
        async fn fetch(&self, _request: &FetchRequest) -> Result<FetchResponse, FetchError> {
            Ok(self.response.clone())
        }
    }

    #[tokio::test]
    async fn handles_not_modified() {
        let request = FetchRequest {
            url: url::Url::parse("https://example.com/feed.xml").expect("url parse"),
            etag: Some("etag-1".to_owned()),
            last_modified: None,
        };
        let response = FetchResponse {
            final_url: request.url.clone(),
            status: 304,
            body: Vec::new(),
            content_type: Some("application/rss+xml".to_owned()),
            etag: Some("etag-2".to_owned()),
            last_modified: None,
            duration_ms: 21,
        };

        let service = FetchService::new(MockClient { response });
        let result = service.fetch_and_parse("feed-1", &request).await.expect("fetch result");

        assert!(matches!(result, FetchOutcome::NotModified(_)));
    }

    #[test]
    fn schedules_with_backoff() {
        let now = Utc::now();
        let next = compute_next_scheduled_fetch(now, false, FeedHealthState::Failing, 3);
        assert!(next - now >= TimeDelta::hours(24));
    }
}
