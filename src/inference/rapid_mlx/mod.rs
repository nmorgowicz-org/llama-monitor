pub mod command;
pub mod discovery;
pub mod runtime;

use anyhow::{Result, anyhow};
use std::path::PathBuf;
use std::time::{Duration, Instant};
use crate::inference::supervisor::SupervisedLaunch;
use self::command::RapidMlxCommandBuilder;
use self::runtime::RuntimeMetadata;
use crate::inference::metrics::InferenceMetricsSnapshot;
use crate::inference::capabilities::CapabilitySet;

pub struct RapidMlxAdapter {
    pub runtime: RuntimeMetadata,
    pub model_path: PathBuf,
    pub served_model_name: Option<String>,
    pub host: String,
    pub port: u16,
    pub log_level: String,
}

impl RapidMlxAdapter {
    pub fn new(runtime: RuntimeMetadata, model_path: PathBuf) -> Self {
        Self {
            runtime,
            model_path,
            served_model_name: None,
            host: "0.0.0.0".to_string(),
            port: 8000,
            log_level: "INFO".to_string(),
        }
    }

    pub async fn validate(&self) -> Result<()> {
        if std::env::consts::OS != "macos" {
            return Err(anyhow!(
                "Rapid-MLX is only supported on macOS. Detected: {}",
                std::env::consts::OS
            ));
        }

        if std::env::consts::ARCH != "aarch64" {
            return Err(anyhow!(
                "Rapid-MLX requires Apple Silicon (aarch64). Detected: {}",
                std::env::consts::ARCH
            ));
        }

        Ok(())
    }

    pub async fn build_launch(&self) -> Result<SupervisedLaunch> {
        let builder = RapidMlxCommandBuilder::new(self.model_path.clone())
            .host(self.host.clone())
            .port(self.port)
            .log_level(self.log_level.clone());

        let builder = if let Some(name) = &self.served_model_name {
            builder.served_model_name(name.clone())
        } else {
            builder
        };

        Ok(builder.build(self.runtime.executable_path.clone()))
    }

    pub async fn await_ready(&self, port: u16, deadline: Instant) -> Result<()> {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(5))
            .build()
            .map_err(|e| anyhow!(e))?;

        let url = format!("http://{}:{}/health/ready", self.host, port);

        loop {
            if Instant::now() > deadline {
                return Err(anyhow!("Timed out waiting for Rapid-MLX to become ready"));
            }

            match client.get(&url).send().await {
                Ok(resp) if resp.status() == reqwest::StatusCode::OK => {
                    return Ok(());
                }
                Ok(_resp) => {}
                Err(_) => {}
            }

            tokio::time::sleep(Duration::from_millis(500)).await;
        }
    }

    pub async fn poll_metrics(&self, _port: u16, _session_id: &str) -> Result<InferenceMetricsSnapshot> {
        Err(anyhow!("RapidMlxAdapter::poll_metrics not implemented (Phase 4)"))
    }

    pub async fn cancel_request(&self, _port: u16, _request_id: &str) -> Result<()> {
        Err(anyhow!("RapidMlxAdapter::cancel_request not implemented (Phase 4)"))
    }

    pub fn capabilities(&self) -> &CapabilitySet {
        unimplemented!("RapidMlxAdapter::capabilities not implemented (Phase 4)")
    }
}
