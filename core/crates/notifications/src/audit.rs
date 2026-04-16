use chrono::{DateTime, Utc};
use models::NormalizedItem;
use sha2::{Digest, Sha256};
use url::Url;

/// Canonical deduplication identity for a candidate item.
pub fn canonical_identity_key(item: &NormalizedItem) -> String {
    if let Some(url) = item.canonical_url.as_ref() {
        return normalize_url(url);
    }
    if let Some(external_id) = item.external_item_id.as_ref() {
        return format!("external:{external_id}");
    }
    format!("raw:{}", item.raw_hash)
}

/// Stable fingerprint for content comparison.
pub fn content_fingerprint(item: &NormalizedItem) -> String {
    let mut hasher = Sha256::new();
    hasher.update(item.title.as_bytes());
    hasher.update([0]);
    if let Some(author) = item.author.as_ref() {
        hasher.update(author.as_bytes());
    }
    hasher.update([0]);
    if let Some(summary) = item.summary.as_ref() {
        hasher.update(summary.as_bytes());
    }
    hasher.update([0]);
    if let Some(content_text) = item.content_text.as_ref() {
        hasher.update(content_text.as_bytes());
    }
    hasher.update([0]);
    if let Some(content_html) = item.content_html.as_ref() {
        hasher.update(content_html.as_bytes());
    }
    hasher.update([0]);
    if let Some(published_at) = item.published_at.as_ref() {
        hasher.update(published_at.to_rfc3339().as_bytes());
    }
    hasher.update([0]);
    hasher.update(item.raw_hash.as_bytes());
    format!("{:x}", hasher.finalize())
}

fn normalize_url(url: &Url) -> String {
    let mut normalized = url.clone();
    normalized.set_fragment(None);
    normalized.to_string()
}

/// Build a notification audit reason with timestamp suffix.
pub fn audit_reason(base: &str, now: DateTime<Utc>) -> String {
    format!("{base}@{}", now.to_rfc3339())
}
