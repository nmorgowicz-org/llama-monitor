pub mod command;
pub mod compatibility;
pub mod discovery;
pub mod poller;
pub mod runtime;
pub mod updater;

use anyhow::Result;
use std::time::Instant;
use crate::inference::supervisor::SupervisedLaunch;
use crate::inference::metrics::{InferenceMetricsSnapshot};
use crate::inference::capabilities::CapabilitySet;
use crate::config::AppConfig;

#[allow(dead_code)]
pub struct RapidMlxAdapter {
    pub app_config: std::sync::Arc<AppConfig>,
    // Other config fields to be added
}

#[allow(dead_code)]
impl RapidMlxAdapter {
    pub async fn validate(&self) -> Result<()> {
        Ok(())
    }

    pub async fn build_launch(&self) -> Result<SupervisedLaunch> {
        unimplemented!()
    }

    pub async fn await_ready(&self, _port: u16, _deadline: Instant) -> Result<()> {
        unimplemented!()
    }

    pub async fn poll_metrics(&self, _port: u16) -> Result<InferenceMetricsSnapshot> {
        unimplemented!()
    }

    pub async fn cancel_request(&self, _port: u16, _request_id: &str) -> Result<()> {
        unimplemented!()
    }

    pub fn capabilities(&self) -> &CapabilitySet {
        unimplemented!()
    }
}
