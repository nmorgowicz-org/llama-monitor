pub mod command;
pub mod compatibility;
pub mod discovery;
pub mod model_resolver;
pub mod poller;
pub mod runtime;

use self::command::RapidMlxCommandBuilder;
use self::compatibility::CompatibilityProfile;
use self::model_resolver::{RapidMlxModelSource, ResolvedRapidMlxLaunchModel};
use self::runtime::RuntimeMetadata;
use crate::inference::capabilities::CapabilitySet;
use crate::inference::metrics::InferenceMetricsSnapshot;
use crate::inference::supervisor::SupervisedLaunch;
use anyhow::{Result, anyhow};
use std::collections::BTreeSet;
use std::path::PathBuf;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RapidMlxConfig {
    #[serde(default)]
    pub model_path: String,
    /// Typed source for new configurations. `model_path` remains the migration fallback.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_source: Option<RapidMlxModelSource>,
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

pub fn ensure_local_platform_supported() -> Result<()> {
    if std::env::consts::OS != "macos" {
        return Err(anyhow!(
            "Rapid-MLX local execution requires macOS on Apple Silicon. Detected OS: {}",
            std::env::consts::OS
        ));
    }
    if std::env::consts::ARCH != "aarch64" {
        return Err(anyhow!(
            "Rapid-MLX local execution requires Apple Silicon (aarch64). Detected architecture: {}",
            std::env::consts::ARCH
        ));
    }
    Ok(())
}

impl Default for RapidMlxConfig {
    fn default() -> Self {
        Self {
            model_path: String::new(),
            model_source: None,
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
    pub fn effective_model_source(&self) -> Result<RapidMlxModelSource> {
        self.model_source.clone().map(Ok).unwrap_or_else(|| {
            self::model_resolver::source_from_legacy_model_path(&self.model_path)
        })
    }

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
    pub resolved_model: ResolvedRapidMlxLaunchModel,
    pub served_model_name: Option<String>,
    pub host: String,
    pub port: u16,
    pub log_level: String,
    pub timeout: Option<u32>,
    pub max_cache_blocks: Option<u32>,
    api_key: Option<String>,
    compatibility: CompatibilityProfile,
    capabilities: CapabilitySet,
    chat_fields: BTreeSet<&'static str>,
}

impl RapidMlxAdapter {
    pub fn from_resolved(
        runtime: RuntimeMetadata,
        resolved_model: ResolvedRapidMlxLaunchModel,
    ) -> Self {
        Self {
            runtime,
            resolved_model,
            served_model_name: None,
            host: "127.0.0.1".to_string(),
            port: 8000,
            log_level: "INFO".to_string(),
            timeout: None,
            max_cache_blocks: None,
            api_key: None,
            compatibility: CompatibilityProfile::verified_baseline(),
            capabilities: verified_capabilities(),
            chat_fields: verified_chat_fields(),
        }
    }

    pub fn configure_runtime(
        &mut self,
        compatibility: CompatibilityProfile,
        api_key: Option<String>,
    ) {
        let verified = compatibility.state == self::compatibility::CompatibilityState::Verified;
        self.capabilities = if verified {
            verified_capabilities()
        } else {
            provisional_capabilities()
        };
        self.chat_fields = if verified {
            verified_chat_fields()
        } else {
            provisional_chat_fields()
        };
        self.compatibility = compatibility;
        self.api_key = api_key.filter(|key| !key.is_empty());
    }

    pub async fn validate(&self) -> Result<()> {
        ensure_local_platform_supported()?;

        if !self.runtime.executable_path.is_file() {
            return Err(anyhow!(
                "Rapid-MLX executable does not exist: {}",
                self.runtime.executable_path.display()
            ));
        }
        if self.resolved_model.launch_argument.trim().is_empty() {
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
        let mut builder = RapidMlxCommandBuilder::new(self.resolved_model.clone())
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
        if _request_id.is_empty()
            || _request_id.len() > 128
            || !_request_id
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
        {
            return Err(anyhow!("Rapid-MLX returned an invalid request ID"));
        }
        if !self.capabilities.cancellation {
            return Err(anyhow!(
                "Rapid-MLX native request cancellation is unavailable because the active runtime does not expose a compatible public request ID"
            ));
        }
        let host = match self.host.as_str() {
            "0.0.0.0" | "::" | "[::]" => "127.0.0.1",
            "::1" => "[::1]",
            host => host,
        };
        let url = format!("http://{host}:{}/v1/requests/{}/cancel", _port, _request_id);
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(5))
            .build()?;
        let mut request = client.post(url);
        if let Some(key) = &self.api_key {
            request = request.bearer_auth(key);
        }
        let response = request.send().await?;
        if !response.status().is_success() {
            return Err(anyhow!(
                "Rapid-MLX cancellation returned HTTP {}",
                response.status()
            ));
        }
        Ok(())
    }

    pub fn capabilities(&self) -> &CapabilitySet {
        &self.capabilities
    }

    pub fn map_chat_request(&self, body: &[u8]) -> Result<Vec<u8>> {
        map_chat_request_with_fields(body, &self.chat_fields)
    }
}

pub fn map_provisional_chat_request(body: &[u8]) -> Result<Vec<u8>> {
    map_chat_request_with_fields(body, &provisional_chat_fields())
}

fn map_chat_request_with_fields(body: &[u8], fields: &BTreeSet<&'static str>) -> Result<Vec<u8>> {
    let value: serde_json::Value = serde_json::from_slice(body)
        .map_err(|error| anyhow!("Invalid chat request JSON: {error}"))?;
    let object = value
        .as_object()
        .ok_or_else(|| anyhow!("Chat request must be a JSON object"))?;
    let mut mapped = serde_json::Map::new();
    for (key, value) in object {
        let output_key = if key == "repeat_penalty" {
            "repetition_penalty"
        } else {
            key.as_str()
        };
        if fields.contains(output_key) {
            mapped.insert(output_key.to_string(), value.clone());
        }
    }
    if !mapped.contains_key("messages") {
        return Err(anyhow!("Rapid-MLX chat requests require messages"));
    }
    if fields.contains("stream_options")
        && mapped.get("stream").and_then(serde_json::Value::as_bool) == Some(true)
    {
        match mapped.get_mut("stream_options") {
            Some(serde_json::Value::Object(options)) => {
                options
                    .entry("include_usage".to_string())
                    .or_insert(serde_json::Value::Bool(true));
            }
            None => {
                mapped.insert(
                    "stream_options".to_string(),
                    serde_json::json!({"include_usage": true}),
                );
            }
            Some(_) => return Err(anyhow!("Rapid-MLX stream_options must be a JSON object")),
        }
    }
    Ok(serde_json::to_vec(&mapped)?)
}

fn provisional_capabilities() -> CapabilitySet {
    CapabilitySet {
        status_memory_telemetry: true,
        one_shot_launch: true,
        ..Default::default()
    }
}

fn verified_capabilities() -> CapabilitySet {
    CapabilitySet {
        // 0.10.9 exposes a cancellation endpoint for its private scheduler ID,
        // but that ID is not exposed in OpenAI SSE chunks or response headers.
        // The public chatcmpl-* response ID is not a compatible contract.
        cancellation: false,
        guided_generation: true,
        tool_parsing: true,
        automatic_tool_choice: true,
        reasoning_parser: true,
        thinking_controls: true,
        status_memory_telemetry: true,
        one_shot_launch: true,
        ..Default::default()
    }
}

fn provisional_chat_fields() -> BTreeSet<&'static str> {
    [
        "messages",
        "model",
        "stream",
        "temperature",
        "top_p",
        "top_k",
        "min_p",
        "max_tokens",
        "max_completion_tokens",
        "stop",
        "repetition_penalty",
    ]
    .into_iter()
    .collect()
}

fn verified_chat_fields() -> BTreeSet<&'static str> {
    let mut fields = provisional_chat_fields();
    fields.extend([
        "stream_options",
        "presence_penalty",
        "frequency_penalty",
        "tools",
        "tool_choice",
        "parallel_tool_calls",
        "response_format",
        "logprobs",
        "timeout",
        "enable_thinking",
        "chat_template_kwargs",
        "reasoning_effort",
    ]);
    fields
}

#[cfg(test)]
mod chat_tests {
    use super::*;

    fn adapter() -> RapidMlxAdapter {
        RapidMlxAdapter::from_resolved(
            RuntimeMetadata {
                executable_path: "rapid-mlx".into(),
                source: runtime::RuntimeSource::Managed,
                version: "0.10.9".into(),
            },
            ResolvedRapidMlxLaunchModel::validated_alias("model").unwrap(),
        )
    }

    #[test]
    fn verified_mapping_filters_llama_fields_and_preserves_supported_controls() {
        let mapped = adapter()
            .map_chat_request(
                br#"{
                    "messages":[{"role":"user","content":"hi"}],
                    "stream":true,
                    "repeat_penalty":1.1,
                    "seed":42,
                    "cache_prompt":true,
                    "thinking_budget_tokens":2048,
                    "stream_options":{"include_usage":true},
                    "tools":[{"type":"function"}],
                    "reasoning_effort":"high"
                }"#,
            )
            .unwrap();
        let value: serde_json::Value = serde_json::from_slice(&mapped).unwrap();
        assert_eq!(value["repetition_penalty"], 1.1);
        assert_eq!(value["stream_options"]["include_usage"], true);
        assert!(value["tools"].is_array());
        assert_eq!(value["reasoning_effort"], "high");
        assert!(value.get("repeat_penalty").is_none());
        assert!(value.get("seed").is_none());
        assert!(value.get("cache_prompt").is_none());
        assert!(value.get("thinking_budget_tokens").is_none());
    }

    #[test]
    fn provisional_mapping_omits_unproven_optional_fields() {
        let mapped = map_provisional_chat_request(
            br#"{"messages":[],"stream":true,"tools":[],"response_format":{"type":"json_object"},"top_k":20}"#,
        )
        .unwrap();
        let value: serde_json::Value = serde_json::from_slice(&mapped).unwrap();
        assert_eq!(value["top_k"], 20);
        assert!(value.get("tools").is_none());
        assert!(value.get("response_format").is_none());
    }

    #[test]
    fn verified_stream_mapping_requests_usage_without_overriding_user_choice() {
        let mapped = adapter()
            .map_chat_request(br#"{"messages":[],"stream":true}"#)
            .unwrap();
        let value: serde_json::Value = serde_json::from_slice(&mapped).unwrap();
        assert_eq!(value["stream_options"]["include_usage"], true);

        let mapped = adapter()
            .map_chat_request(
                br#"{"messages":[],"stream":true,"stream_options":{"include_usage":false}}"#,
            )
            .unwrap();
        let value: serde_json::Value = serde_json::from_slice(&mapped).unwrap();
        assert_eq!(value["stream_options"]["include_usage"], false);
    }

    #[test]
    fn rapid_mapping_rejects_malformed_or_message_less_requests() {
        assert!(adapter().map_chat_request(b"not json").is_err());
        assert!(adapter().map_chat_request(br#"{"stream":true}"#).is_err());
    }
}
