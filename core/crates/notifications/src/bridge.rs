use async_trait::async_trait;
use thiserror::Error;

use crate::{NotificationEventDraft, scheduler::RefreshReason};

/// Endpoint metadata for future hosted push integrations.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct PushEndpointRegistration {
    /// Registration identifier.
    pub id: String,
    /// Transport/platform label.
    pub platform: String,
    /// Device token or endpoint payload.
    pub endpoint: String,
    /// Whether the endpoint is active.
    pub enabled: bool,
}

/// Future hosted push bridge boundary.
#[async_trait]
pub trait RemotePushBridge: Send + Sync {
    /// Register or update a device endpoint.
    async fn register_endpoint(
        &self,
        endpoint: &PushEndpointRegistration,
    ) -> Result<(), BridgeError>;

    /// Submit feed update events for remote evaluation.
    async fn push_events(
        &self,
        reason: RefreshReason,
        events: &[NotificationEventDraft],
    ) -> Result<(), BridgeError>;
}

/// No-op bridge for local-only mode.
pub struct NoopRemotePushBridge;

#[async_trait]
impl RemotePushBridge for NoopRemotePushBridge {
    async fn register_endpoint(
        &self,
        _endpoint: &PushEndpointRegistration,
    ) -> Result<(), BridgeError> {
        Ok(())
    }

    async fn push_events(
        &self,
        _reason: RefreshReason,
        _events: &[NotificationEventDraft],
    ) -> Result<(), BridgeError> {
        Ok(())
    }
}

/// Bridge errors.
#[derive(Debug, Error)]
pub enum BridgeError {
    /// Transport or backend failure.
    #[error("bridge error: {0}")]
    Transport(String),
}
