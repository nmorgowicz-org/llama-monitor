use std::time::Instant;
use anyhow::Result;
use crate::inference::metrics::InferenceMetricsSnapshot;
use crate::inference::capabilities::CapabilitySet;
use crate::inference::supervisor::SupervisedLaunch;

pub enum BackendAdapter {
    LlamaCpp(crate::inference::llama_cpp::LlamaCppAdapter),
    RapidMlx(crate::inference::rapid_mlx::RapidMlxAdapter),
}

impl BackendAdapter {
    /// Validate platform, runtime, model source, and flag conflicts before launch.
    pub async fn validate(&self) -> Result<()> {
        match self {
            BackendAdapter::LlamaCpp(adapter) => adapter.validate().await,
            BackendAdapter::RapidMlx(adapter) => adapter.validate().await,
        }
    }

    /// Build the launch bundle. Called after validate() succeeds.
    pub async fn build_launch(&self) -> Result<SupervisedLaunch> {
        match self {
            BackendAdapter::LlamaCpp(adapter) => adapter.build_launch().await,
            BackendAdapter::RapidMlx(adapter) => adapter.build_launch().await,
        }
    }

    /// Poll until the server is ready to serve requests or the deadline elapses.
    pub async fn await_ready(&self, port: u16, deadline: Instant) -> Result<()> {
        match self {
            BackendAdapter::LlamaCpp(adapter) => adapter.await_ready(port, deadline).await,
            BackendAdapter::RapidMlx(adapter) => adapter.await_ready(port, deadline).await,
        }
    }

    /// Fetch a normalized metrics snapshot. Called by the shared poller loop.
    pub async fn poll_metrics(&self, port: u16) -> Result<InferenceMetricsSnapshot> {
        match self {
            BackendAdapter::LlamaCpp(adapter) => adapter.poll_metrics(port).await,
            BackendAdapter::RapidMlx(adapter) => adapter.poll_metrics(port).await,
        }
    }

    /// Native request cancellation.
    pub async fn cancel_request(&self, port: u16, request_id: &str) -> Result<()> {
        match self {
            BackendAdapter::LlamaCpp(adapter) => adapter.cancel_request(port, request_id).await,
            BackendAdapter::RapidMlx(adapter) => adapter.cancel_request(port, request_id).await,
        }
    }

    /// Return the static capability set for the active runtime profile.
    pub fn capabilities(&self) -> &CapabilitySet {
        match self {
            BackendAdapter::LlamaCpp(adapter) => adapter.capabilities(),
            BackendAdapter::RapidMlx(adapter) => adapter.capabilities(),
        }
    }
}
