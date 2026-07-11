use anyhow::Result;
use std::time::Instant;
use std::sync::Arc;

use crate::inference::capabilities::CapabilitySet;
use crate::inference::metrics::InferenceMetricsSnapshot;
use crate::inference::supervisor::SupervisedLaunch;
use crate::inference::llama_cpp::LlamaCppAdapter;

/// The BackendAdapter is an enum to ensure exhaustive and zero-overhead dispatch
/// to the specific implementation for each inference engine.
#[allow(dead_code)]
#[derive(Clone)]
pub enum BackendAdapter {
    LlamaCpp(Arc<LlamaCppAdapter>),
    RapidMlx(Arc<RapidMlxAdapter>),
}

#[allow(dead_code)]
impl BackendAdapter {
    /// Validate platform, runtime, model source, and flag conflicts before launch.
    pub async fn validate(&self) -> Result<()> {
        match self {
            Self::LlamaCpp(adapter) => adapter.validate().await,
            Self::RapidMlx(adapter) => adapter.validate().await,
        }
    }

    /// Build the launch bundle. Called after validate() succeeds.
    pub async fn build_launch(&self) -> Result<SupervisedLaunch> {
        match self {
            Self::LlamaCpp(adapter) => adapter.build_launch().await,
            Self::RapidMlx(adapter) => adapter.build_launch().await,
        }
    }

    /// Poll until the server is ready to serve requests or the deadline elapses.
    pub async fn await_ready(&self, port: u16, deadline: Instant) -> Result<()> {
        match self {
            Self::LlamaCpp(adapter) => adapter.await_ready(port, deadline).await,
            Self::RapidMlx(adapter) => adapter.await_ready(port, deadline).await,
        }
    }

    /// Fetch a normalized metrics snapshot. Called by the shared poller loop.
    pub async fn poll_metrics(&self, port: u16, session_id: &str) -> Result<InferenceMetricsSnapshot> {
        match self {
            Self::LlamaCpp(adapter) => adapter.poll_metrics(port, session_id).await,
            Self::RapidMlx(adapter) => adapter.poll_metrics(port, session_id).await,
        }
    }

    /// Native request cancellation.
    pub async fn cancel_request(&self, port: u16, request_id: &str) -> Result<()> {
        match self {
            Self::LlamaCpp(adapter) => adapter.cancel_request(port, request_id).await,
            Self::RapidMlx(adapter) => adapter.cancel_request(port, request_id).await,
        }
    }

    /// Return the static capability set for the active runtime profile.
    pub fn capabilities(&self) -> &CapabilitySet {
        match self {
            Self::LlamaCpp(adapter) => adapter.capabilities(),
            Self::RapidMlx(adapter) => adapter.capabilities(),
        }
    }
}

// Placeholder structs for the adapters to satisfy the compiler.
// LlamaCppAdapter is now fully implemented in llama_cpp.rs.
pub struct RapidMlxAdapter {
    // Fields to be added in Phase 3
}

impl RapidMlxAdapter {
    pub async fn validate(&self) -> Result<()> { Ok(()) }
    pub async fn build_launch(&self) -> Result<SupervisedLaunch> { 
        Err(anyhow::anyhow!("RapidMlxAdapter::build_launch not implemented")) 
    }
    pub async fn await_ready(&self, _port: u16, _deadline: Instant) -> Result<()> { Ok(()) }
    pub async fn poll_metrics(&self, _port: u16, _session_id: &str) -> Result<InferenceMetricsSnapshot> { 
        Err(anyhow::anyhow!("RapidMlxAdapter::poll_metrics not implemented")) 
    }
    pub async fn cancel_request(&self, _port: u16, _request_id: &str) -> Result<()> { Ok(()) }
    pub fn capabilities(&self) -> &CapabilitySet { 
        unimplemented!("RapidMlxAdapter::capabilities not implemented") 
    }
}
