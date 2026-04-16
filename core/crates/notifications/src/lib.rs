//! Notification policy, scheduling, and delivery boundaries.

pub mod audit;
pub mod bridge;
pub mod coordinator;
pub mod delivery;
pub mod policy;
pub mod scheduler;

use chrono::{DateTime, Utc};
use models::NotificationMode;
use serde::{Deserialize, Serialize};
use url::Url;

pub use audit::{canonical_identity_key, content_fingerprint};
pub use coordinator::{build_digest_batch, summarize_candidates};
pub use policy::{NotificationDecision, NotificationSuppressionReason};
pub use scheduler::{RefreshAttempt, RefreshJobState, RefreshReason, compute_next_refresh_at};

/// Candidate item fed into notification policy evaluation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NotificationCandidate {
    /// Feed identifier.
    pub feed_id: String,
    /// Entry identifier.
    pub entry_id: String,
    /// Human-readable title.
    pub title: String,
    /// Canonical URL if available.
    pub canonical_url: Option<Url>,
    /// Optional summary or excerpt.
    pub summary: Option<String>,
    /// Optional text content.
    pub content_text: Option<String>,
    /// Optional HTML content.
    pub content_html: Option<String>,
    /// Published timestamp.
    pub published_at: Option<DateTime<Utc>>,
    /// Updated timestamp.
    pub updated_at: Option<DateTime<Utc>>,
    /// Raw payload hash.
    pub raw_hash: String,
}

impl NotificationCandidate {
    /// Short notification body to show in local notifications.
    pub fn body_text(&self) -> String {
        self.summary
            .as_ref()
            .map(|value| value.trim().to_owned())
            .filter(|value| !value.is_empty())
            .or_else(|| {
                self.content_text
                    .as_ref()
                    .map(|value| value.trim().to_owned())
                    .filter(|value| !value.is_empty())
            })
            .unwrap_or_else(|| self.title.clone())
    }
}

/// Draft payload for a pending or queued notification.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NotificationEventDraft {
    /// Feed identifier.
    pub feed_id: String,
    /// Entry identifier.
    pub entry_id: String,
    /// Canonical identity key.
    pub canonical_key: String,
    /// Dedup fingerprint.
    pub content_fingerprint: String,
    /// Local title.
    pub title: String,
    /// Local body preview.
    pub body: String,
    /// Delivery mode chosen by policy.
    pub mode: NotificationMode,
    /// Human-readable decision reason.
    pub reason: String,
    /// Creation time.
    pub created_at: DateTime<Utc>,
    /// Time the notification becomes deliverable.
    pub ready_at: Option<DateTime<Utc>>,
}

/// Digest batch payload for grouped notifications.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NotificationDigestDraft {
    /// Feed identifier.
    pub feed_id: String,
    /// Entry identifiers in this batch.
    pub entry_ids: Vec<String>,
    /// Digest title.
    pub title: String,
    /// Digest body summary.
    pub body: String,
    /// Creation time.
    pub created_at: DateTime<Utc>,
    /// Delivery-ready time.
    pub ready_at: Option<DateTime<Utc>>,
}
