use async_trait::async_trait;
use thiserror::Error;

use crate::{NotificationDigestDraft, NotificationEventDraft};

/// Delivery abstraction for local or future remote notification sinks.
#[async_trait]
pub trait NotificationDelivery: Send + Sync {
    /// Deliver a single notification event.
    async fn deliver_event(&self, event: &NotificationEventDraft) -> Result<(), DeliveryError>;

    /// Deliver a digest batch.
    async fn deliver_digest(&self, digest: &NotificationDigestDraft) -> Result<(), DeliveryError>;
}

/// No-op sink used for local-only or test environments.
pub struct NoopNotificationDelivery;

#[async_trait]
impl NotificationDelivery for NoopNotificationDelivery {
    async fn deliver_event(&self, _event: &NotificationEventDraft) -> Result<(), DeliveryError> {
        Ok(())
    }

    async fn deliver_digest(&self, _digest: &NotificationDigestDraft) -> Result<(), DeliveryError> {
        Ok(())
    }
}

/// Delivery errors.
#[derive(Debug, Error)]
pub enum DeliveryError {
    /// Transport or backend failure.
    #[error("delivery error: {0}")]
    Transport(String),
}
