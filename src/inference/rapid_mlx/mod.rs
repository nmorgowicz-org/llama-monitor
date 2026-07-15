pub mod command;
pub mod discovery;
pub mod poller;
pub mod runtime;

use self::command::RapidMlxCommandBuilder;
use self::runtime::RuntimeMetadata;
use crate::inference::capabilities::CapabilitySet;
use crate::inference::metrics::InferenceMetricsSnapshot;
use crate::inference::supervisor::SupervisedLaunch;
use anyhow::{Result, anyhow};
use std::path::PathBuf;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RapidMlxConfig {
    #[serde(default)]
    pub model_path: String,
    #[serde(default)]
    pub served_model_name: Option<String>,
    #[serde(default)]
    pub executable_path: Option<PathBuf>,
    #[serde(default)]
    pub managed_runtime_path: Option<PathBuf>,
    #[serde(default = "default_host")]
    pub host: String,
    #[serde(default = "default_port")]
    pub port: u16,
    #[serde(default = "default_log_level")]
    pub log_level: String,
}

fn default_host() -> String {
    "0.0.0.0".into()
}

fn default_port() -> u16 {
    8000
}

fn default_log_level() -> String {
    "INFO".into()
}

impl Default for RapidMlxConfig {
    fn default() -> Self {
        Self {
            model_path: String::new(),
            served_model_name: None,
            executable_path: None,
            managed_runtime_path: None,
            host: default_host(),
            port: default_port(),
            log_level: default_log_level(),
        }
    }
}

pub struct RapidMlxAdapter {
    pub runtime: RuntimeMetadata,
    pub model_path: PathBuf,
    pub served_model_name: Option<String>,
    pub host: String,
    pub port: u16,
    pub log_level: String,
}

impl RapidMlxAdapter {
    #[allow(dead_code)]
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

        if !self.runtime.executable_path.is_file() {
            return Err(anyhow!(
                "Rapid-MLX executable does not exist: {}",
                self.runtime.executable_path.display()
            ));
        }
        if self.model_path.as_os_str().is_empty() {
            return Err(anyhow!("Rapid-MLX requires a model path"));
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

        let readiness_host = match self.host.as_str() {
            "0.0.0.0" | "::" | "[::]" => "127.0.0.1",
            host => host,
        };
        let url = format!("http://{readiness_host}:{port}/health/ready");

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

    pub async fn poll_metrics(
        &self,
        port: u16,
        _session_id: &str,
    ) -> Result<InferenceMetricsSnapshot> {
        let poller = self::poller::RapidMlxPoller::new(&self.host, port);
        poller.poll().await
    }

    pub async fn cancel_request(&self, _port: u16, _request_id: &str) -> Result<()> {
        Err(anyhow!(
            "RapidMlxAdapter::cancel_request not implemented (Phase 4)"
        ))
    }

    pub fn capabilities(&self) -> &CapabilitySet {
        static CAPS: CapabilitySet = CapabilitySet {
            vision: false,
            mtp: false,
            cancellation: false,
            embeddings: false,
            guided_generation: false,
            audio: false,
            tool_parsing: false,
            automatic_tool_choice: false,
            reasoning_parser: false,
            thinking_controls: false,
            mcp: false,
            cache_telemetry: false,
            status_memory_telemetry: true,
            self_diagnostic: false,
            interpretability: false,
            one_shot_launch: true,
        };
        &CAPS
    }
}
