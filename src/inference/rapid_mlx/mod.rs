pub mod command;
pub mod compatibility;
pub mod discovery;
pub mod poller;
pub mod runtime;

use self::command::RapidMlxCommandBuilder;
use self::compatibility::CompatibilityProfile;
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
    #[serde(default)]
    pub timeout: Option<u32>,
    #[serde(default)]
    pub max_cache_blocks: Option<u32>,
    /// Accepted only on launch input. Secrets are never serialized into presets,
    /// sessions, or diagnostics.
    #[serde(default, skip_serializing)]
    pub api_key: Option<String>,
}

fn default_host() -> String {
    "127.0.0.1".into()
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
            timeout: None,
            max_cache_blocks: None,
            api_key: None,
        }
    }
}

impl RapidMlxConfig {
    pub fn validate_access(&self, fallback_api_key: Option<&str>) -> Result<()> {
        let loopback = matches!(
            self.host.as_str(),
            "127.0.0.1" | "localhost" | "::1" | "[::1]"
        );
        let has_key = self
            .api_key
            .as_deref()
            .or(fallback_api_key)
            .is_some_and(|key| !key.is_empty());
        if !loopback && !has_key {
            return Err(anyhow!(
                "Rapid-MLX LAN exposure requires an API key; use 127.0.0.1 or configure authenticated access"
            ));
        }
        Ok(())
    }
}

pub struct RapidMlxAdapter {
    pub runtime: RuntimeMetadata,
    pub model_path: PathBuf,
    pub served_model_name: Option<String>,
    pub host: String,
    pub port: u16,
    pub log_level: String,
    pub timeout: Option<u32>,
    pub max_cache_blocks: Option<u32>,
    api_key: Option<String>,
    compatibility: CompatibilityProfile,
}

impl RapidMlxAdapter {
    #[allow(dead_code)]
    pub fn new(runtime: RuntimeMetadata, model_path: PathBuf) -> Self {
        Self {
            runtime,
            model_path,
            served_model_name: None,
            host: "127.0.0.1".to_string(),
            port: 8000,
            log_level: "INFO".to_string(),
            timeout: None,
            max_cache_blocks: None,
            api_key: None,
            compatibility: CompatibilityProfile::verified_baseline(),
        }
    }

    pub fn configure_runtime(
        &mut self,
        compatibility: CompatibilityProfile,
        api_key: Option<String>,
    ) {
        self.compatibility = compatibility;
        self.api_key = api_key.filter(|key| !key.is_empty());
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
        RapidMlxConfig {
            host: self.host.clone(),
            api_key: self.api_key.clone(),
            ..Default::default()
        }
        .validate_access(None)?;

        Ok(())
    }

    pub async fn build_launch(&self) -> Result<SupervisedLaunch> {
        let mut builder = RapidMlxCommandBuilder::new(self.model_path.clone())
            .host(self.host.clone())
            .port(self.port);

        if self.log_level != "INFO" {
            builder = builder.log_level(self.log_level.clone());
        }
        if let Some(timeout) = self.timeout {
            builder = builder.timeout(timeout);
        }
        if let Some(blocks) = self.max_cache_blocks {
            builder = builder.max_cache_blocks(blocks);
        }

        if let Some(key) = &self.api_key {
            builder = builder.api_key(key.clone());
        }

        let builder = if let Some(name) = &self.served_model_name {
            builder.served_model_name(name.clone())
        } else {
            builder
        };

        let mut launch = builder.build(
            self.runtime.executable_path.clone(),
            &self.compatibility.capabilities,
        )?;
        launch.redacted_summary.push_str(&format!(
            " ({}, {})",
            self.compatibility.version,
            self.compatibility.state.label()
        ));
        Ok(launch)
    }

    pub async fn await_ready(&self, port: u16, deadline: Instant) -> Result<()> {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(5))
            .build()
            .map_err(|e| anyhow!(e))?;

        let readiness_host = match self.host.as_str() {
            "0.0.0.0" | "::" | "[::]" => "127.0.0.1",
            "::1" => "[::1]",
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
        let poller = self::poller::RapidMlxPoller::new(&self.host, port, self.api_key.as_deref());
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
