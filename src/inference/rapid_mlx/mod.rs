pub mod capabilities;
pub mod changelog;
pub mod command;
pub mod compatibility;
pub mod discovery;
pub mod escape_hatch;
pub mod info_query;
pub mod mlx_meta;
pub mod model_resolver;
pub mod poller;
pub mod runtime;
pub mod settings;
#[allow(dead_code)]
pub mod updater;

#[allow(unused_imports)]
pub use settings::{
    RapidMlxSetting, ValidationContext, ValidationError, all_settings, check_mutual_exclusions,
};

use self::command::RapidMlxCommandBuilder;
use self::compatibility::CompatibilityProfile;
use self::model_resolver::{
    RapidMlxModelSource, RapidMlxModelSourceView, ResolvedRapidMlxLaunchModel,
};
use self::runtime::RuntimeMetadata;
use crate::inference::capabilities::CapabilitySet;
use crate::inference::metrics::InferenceMetricsSnapshot;
use crate::inference::supervisor::SupervisedLaunch;
use anyhow::{Result, anyhow};
use std::collections::BTreeSet;
use std::collections::HashMap;
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
    /// Phase 6 Part B: prefix cache persistence display toggle (safe default: false).
    /// Controls whether prefix cache budget is shown in VRAM breakdowns and whether
    /// the UI exposes prefix cache configuration. max_cache_blocks and D30 budget
    /// are applied based on capability guidance regardless of this flag.
    #[serde(default)]
    pub prefix_cache_enabled: bool,
    /// Phase 6 Part B: explicit prefix cache budget override in bytes.
    /// When set, overrides the D30 auto-computed budget (configured_ceiling_bytes × 0.10).
    /// User explicit values always win (hard gate).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prefix_cache_budget_bytes: Option<u64>,
    /// Accepted only on launch input. Secrets are never serialized into presets,
    /// sessions, or diagnostics.
    #[serde(default, skip_serializing)]
    pub api_key: Option<String>,
    /// Default applied to chat requests that omit `enable_thinking`, mirroring
    /// llama.cpp's standing `--reasoning` server-level default.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enable_thinking: Option<bool>,
    /// Default applied to chat requests that omit `reasoning_effort`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reasoning_effort: Option<String>,
    /// User consent for HF repos requiring trust_remote_code (custom Python code).
    /// Revision-scoped: must be re-confirmed when revision changes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trust_remote_code_consent: Option<String>,
    /// Diagnostic fix flags: set by the diagnostics panel to patch launch behavior.
    /// These are diagnostic helpers only, not general-purpose escape hatches.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_call_parser: Option<String>,
    #[serde(default)]
    pub auto_tool_choice: bool,
    #[serde(default)]
    pub no_thinking: bool,
    /// Curated escape-hatch flags for advanced tuning (PFlash, spec-decode, hybrid).
    /// Validated against an allowlist at load time; no free-text CLI injection.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub escape_hatch_flags: Vec<(String, serde_json::Value)>,
    /// Computed view of the model source for API responses and UI display.
    /// Never persisted to disk; populated by preset_for_api().
    #[serde(default, skip_deserializing, skip_serializing_if = "Option::is_none")]
    pub model_source_view: Option<RapidMlxModelSourceView>,
    // ── Phase 7: KV/cache policy ──────────────────────────────────────
    /// KV cache dtype configuration (D1/D2: effective value after overrides).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kv_cache_dtype: Option<KvCacheConfig>,
    /// TurboQuant reusable-prompt storage policy (D31).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub turboquant_mode: Option<TurboQuantMode>,
    /// Prefix cache policy (auto/explicit).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prefix_cache_policy: Option<String>,
    /// Hybrid cache entries limit.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hybrid_cache_entries: Option<u64>,
    /// PFlash policy (auto/always/off).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pflash_policy: Option<String>,
    /// Response cache policy.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub response_cache_policy: Option<String>,
    /// Disk checkpoint policy.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub disk_checkpoint_policy: Option<String>,
    // ── Phase 7: batching/concurrency ──────────────────────────────────
    /// Max number of sequences.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_num_seqs: Option<u64>,
    /// Max concurrent requests.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_concurrent_requests: Option<u64>,
    /// Prefill batch size.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prefill_batch_size: Option<u64>,
    /// Completion batch size.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub completion_batch_size: Option<u64>,
    /// Batching policy.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub batching_policy: Option<String>,
    /// Concurrency policy (single_active/allow_overlap).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub concurrency_policy: Option<String>,
    // ── Phase 7: reasoning/speculative ─────────────────────────────────
    /// Reasoning mode (auto/on/off).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reasoning_mode: Option<String>,
    /// Speculative decoding policy.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub speculative_policy: Option<String>,
    // ── Phase 7: MLLM/embeddings ───────────────────────────────────────
    /// MLLM vision support (auto/on/off).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mllm_vision: Option<String>,
    /// Embeddings support (auto/on/off).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub embeddings: Option<String>,
    // ── Phase 7: GPU ───────────────────────────────────────────────────
    /// GPU memory utilization (0.5–1.0).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gpu_memory_utilization: Option<f64>,
    // ── Phase 7: Web UI (D26/A44) ──────────────────────────────────────
    /// Web UI availability (auto/on/off).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub web_ui_availability: Option<String>,
    /// Expert custom static path for Web UI.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub web_ui_static_path: Option<String>,
    /// Validated Web UI config JSON.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub web_ui_config_json: Option<String>,
    // ── Phase 7: endpoint/safety ───────────────────────────────────────
    /// Endpoint compatibility mode.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub endpoint_compatibility: Option<String>,
    /// Request safety policy.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub request_safety_policy: Option<String>,
    /// Sampling mode.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sampling_mode: Option<String>,
    /// Parser policy.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parser_policy: Option<String>,
    /// Security policy.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub security_policy: Option<String>,
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

/// KV cache dtype configuration (D1/D2).
/// Represents the effective KV cache dtype after considering all overrides.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KvCacheConfig {
    /// Use default behavior.
    Auto,
    /// Override to FP16 (e.g., `fp16` for `--kv-cache-dtype`).
    Fp16,
    /// Override to BF16.
    Bf16,
    /// Override to FP8.
    Fp8,
}

impl std::fmt::Display for KvCacheConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            KvCacheConfig::Auto => write!(f, "auto"),
            KvCacheConfig::Fp16 => write!(f, "fp16"),
            KvCacheConfig::Bf16 => write!(f, "bf16"),
            KvCacheConfig::Fp8 => write!(f, "fp8"),
        }
    }
}

/// TurboQuant reusable-prompt storage policy (D31).
/// Values match Rapid-MLX CLI --turboquant flag: v4, k8v4, none.
/// "auto" is our config sentinel; the builder maps it to omitting --turboquant (runtime default).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TurboQuantMode {
    /// Use runtime default (omit --turboquant flag).
    Auto,
    /// V-only TurboQuant (expert legacy/A-B).
    V4,
    /// K8V4 asymmetric TurboQuant (Advanced trial recommendation).
    K8V4,
    /// Disable TurboQuant (Standard retained-storage policy, normally int4).
    Off,
}

impl std::fmt::Display for TurboQuantMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TurboQuantMode::Auto => write!(f, "auto"),
            TurboQuantMode::V4 => write!(f, "v4"),
            TurboQuantMode::K8V4 => write!(f, "k8v4"),
            TurboQuantMode::Off => write!(f, "none"),
        }
    }
}

pub fn ensure_local_platform_supported() -> Result<()> {
    if std::env::var_os("LLAMA_MONITOR_FAKE_RAPID_MLX_LOCAL_AVAILABLE").is_some_and(|v| v == "1") {
        return Ok(());
    }
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
            prefix_cache_enabled: false,
            prefix_cache_budget_bytes: None,
            api_key: None,
            enable_thinking: None,
            reasoning_effort: None,
            trust_remote_code_consent: None,
            tool_call_parser: None,
            auto_tool_choice: false,
            no_thinking: false,
            escape_hatch_flags: Vec::new(),
            model_source_view: None,
            // Phase 7: KV/cache policy
            kv_cache_dtype: None,
            turboquant_mode: None,
            prefix_cache_policy: None,
            hybrid_cache_entries: None,
            pflash_policy: None,
            response_cache_policy: None,
            disk_checkpoint_policy: None,
            // Phase 7: batching/concurrency
            max_num_seqs: None,
            max_concurrent_requests: None,
            prefill_batch_size: None,
            completion_batch_size: None,
            batching_policy: None,
            concurrency_policy: None,
            // Phase 7: reasoning/speculative
            reasoning_mode: None,
            speculative_policy: None,
            // Phase 7: MLLM/embeddings
            mllm_vision: None,
            embeddings: None,
            // Phase 7: GPU
            gpu_memory_utilization: None,
            // Phase 7: Web UI
            web_ui_availability: None,
            web_ui_static_path: None,
            web_ui_config_json: None,
            // Phase 7: endpoint/safety
            endpoint_compatibility: None,
            request_safety_policy: None,
            sampling_mode: None,
            parser_policy: None,
            security_policy: None,
        }
    }
}

impl RapidMlxConfig {
    /// Typed source wins; legacy model_path is only a launch-time fallback.
    /// Never opens legacy data to produce a view (Gap 3.2).
    pub fn effective_model_source(&self) -> Result<RapidMlxModelSource> {
        self.model_source.clone().map(Ok).unwrap_or_else(|| {
            self::model_resolver::source_from_legacy_model_path(&self.model_path)
        })
    }

    /// Typed source view for display/edit/clone/save/estimate/library/launch.
    /// Uses only `model_source`; ignores legacy `model_path` entirely (Gap 3.2).
    /// Returns empty view when no typed source is configured — this is the
    /// "No model configured" signal that the frontend must show.
    #[allow(dead_code)]
    pub fn model_source_view(&self) -> RapidMlxModelSourceView {
        match &self.model_source {
            Some(source) => RapidMlxModelSourceView::from_source(source),
            None => RapidMlxModelSourceView::empty(),
        }
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
    pub enable_thinking: Option<bool>,
    pub reasoning_effort: Option<String>,
    pub trust_remote_code_consent: Option<String>,
    pub tool_call_parser: Option<String>,
    pub auto_tool_choice: bool,
    pub no_thinking: bool,
    pub escape_hatch_flags: Vec<(String, serde_json::Value)>,
    // Phase 7 fields
    pub kv_cache_dtype: Option<KvCacheConfig>,
    pub turboquant_mode: Option<TurboQuantMode>,
    pub prefix_cache_policy: Option<String>,
    pub hybrid_cache_entries: Option<u64>,
    pub pflash_policy: Option<String>,
    pub response_cache_policy: Option<String>,
    pub disk_checkpoint_policy: Option<String>,
    pub max_num_seqs: Option<u64>,
    pub max_concurrent_requests: Option<u64>,
    pub prefill_batch_size: Option<u64>,
    pub completion_batch_size: Option<u64>,
    pub batching_policy: Option<String>,
    pub concurrency_policy: Option<String>,
    pub reasoning_mode: Option<String>,
    pub speculative_policy: Option<String>,
    pub mllm_vision: Option<String>,
    pub embeddings: Option<String>,
    pub gpu_memory_utilization: Option<f64>,
    pub web_ui_availability: Option<String>,
    pub web_ui_static_path: Option<String>,
    pub web_ui_config_json: Option<String>,
    pub endpoint_compatibility: Option<String>,
    pub request_safety_policy: Option<String>,
    pub sampling_mode: Option<String>,
    pub parser_policy: Option<String>,
    pub security_policy: Option<String>,
    api_key: Option<String>,
    compatibility: CompatibilityProfile,
    capabilities: CapabilitySet,
    chat_fields: BTreeSet<&'static str>,
    pollers: std::sync::Mutex<HashMap<u16, std::sync::Arc<self::poller::RapidMlxPoller>>>,
}

impl RapidMlxAdapter {
    fn poller_for(&self, port: u16) -> Result<std::sync::Arc<self::poller::RapidMlxPoller>> {
        let mut pollers = self
            .pollers
            .lock()
            .map_err(|error| anyhow!("Rapid-MLX poller cache lock failed: {error}"))?;
        Ok(pollers
            .entry(port)
            .or_insert_with(|| {
                std::sync::Arc::new(self::poller::RapidMlxPoller::new(
                    &self.host,
                    port,
                    self.api_key.as_deref(),
                ))
            })
            .clone())
    }

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
            enable_thinking: None,
            reasoning_effort: None,
            trust_remote_code_consent: None,
            tool_call_parser: None,
            auto_tool_choice: false,
            no_thinking: false,
            escape_hatch_flags: Vec::new(),
            // Phase 7 defaults
            kv_cache_dtype: None,
            turboquant_mode: None,
            prefix_cache_policy: None,
            hybrid_cache_entries: None,
            pflash_policy: None,
            response_cache_policy: None,
            disk_checkpoint_policy: None,
            max_num_seqs: None,
            max_concurrent_requests: None,
            prefill_batch_size: None,
            completion_batch_size: None,
            batching_policy: None,
            concurrency_policy: None,
            reasoning_mode: None,
            speculative_policy: None,
            mllm_vision: None,
            embeddings: None,
            gpu_memory_utilization: None,
            web_ui_availability: None,
            web_ui_static_path: None,
            web_ui_config_json: None,
            endpoint_compatibility: None,
            request_safety_policy: None,
            sampling_mode: None,
            parser_policy: None,
            security_policy: None,
            api_key: None,
            compatibility: CompatibilityProfile::verified_baseline(),
            capabilities: verified_capabilities(),
            chat_fields: verified_chat_fields(),
            pollers: std::sync::Mutex::new(HashMap::new()),
        }
    }

    pub fn configure_runtime(
        &mut self,
        compatibility: CompatibilityProfile,
        api_key: Option<String>,
    ) {
        let verified = compatibility.state == self::compatibility::CompatibilityState::Verified;

        // Derive capabilities from snapshot if available, otherwise fall back to
        // the verified/provisional baseline sets. The snapshot is the source of
        // truth for per-feature qualification.
        if let Some(ref snapshot) = self.runtime.capability_snapshot {
            self.capabilities = capabilities_from_snapshot(snapshot);
            self.chat_fields = chat_fields_from_snapshot(snapshot);
        } else if verified {
            self.capabilities = verified_capabilities();
            self.chat_fields = verified_chat_fields();
        } else {
            self.capabilities = provisional_capabilities();
            self.chat_fields = provisional_chat_fields();
        }

        self.compatibility = compatibility;
        self.api_key = api_key.filter(|key| !key.is_empty());
        self.pollers
            .get_mut()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clear();
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

        let builder = builder
            .trust_remote_code_consent(self.trust_remote_code_consent.clone())
            .tool_call_parser(self.tool_call_parser.clone())
            .auto_tool_choice(self.auto_tool_choice)
            .no_thinking(self.no_thinking)
            .escape_hatch_flags(self.escape_hatch_flags.clone());

        // Phase 7 config wiring
        let builder = apply_phase7_adapter_config(builder, self);

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
        let poller = self.poller_for(port)?;
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
        let mapped = map_chat_request_with_fields(body, &self.chat_fields)?;
        self.apply_reasoning_defaults(mapped)
    }

    /// Fills in `enable_thinking`/`reasoning_effort` from the launch config when the
    /// caller's request omits them, mirroring llama.cpp's standing `--reasoning`
    /// server-level default (Rapid-MLX has no launch-time reasoning flag; these are
    /// per-request chat fields instead).
    fn apply_reasoning_defaults(&self, mapped: Vec<u8>) -> Result<Vec<u8>> {
        if self.enable_thinking.is_none() && self.reasoning_effort.is_none() {
            return Ok(mapped);
        }
        let mut value: serde_json::Value = serde_json::from_slice(&mapped)?;
        let object = value
            .as_object_mut()
            .ok_or_else(|| anyhow!("Chat request must be a JSON object"))?;
        if self.chat_fields.contains("enable_thinking")
            && let Some(default) = self.enable_thinking
            && !object.contains_key("enable_thinking")
        {
            object.insert("enable_thinking".to_string(), serde_json::json!(default));
        }
        if self.chat_fields.contains("reasoning_effort")
            && let Some(default) = &self.reasoning_effort
            && !object.contains_key("reasoning_effort")
        {
            object.insert("reasoning_effort".to_string(), serde_json::json!(default));
        }
        Ok(serde_json::to_vec(&value)?)
    }
}

fn apply_phase7_adapter_config(
    builder: command::RapidMlxCommandBuilder,
    adapter: &RapidMlxAdapter,
) -> command::RapidMlxCommandBuilder {
    builder
        .kv_cache_dtype(adapter.kv_cache_dtype.as_ref().map(|kv| {
            use crate::inference::rapid_mlx::command::KvCacheDtypeArg;
            match kv {
                KvCacheConfig::Auto => KvCacheDtypeArg::Auto,
                KvCacheConfig::Fp16 => KvCacheDtypeArg::Explicit("fp16".into()),
                KvCacheConfig::Bf16 => KvCacheDtypeArg::Explicit("bf16".into()),
                KvCacheConfig::Fp8 => KvCacheDtypeArg::Explicit("fp8".into()),
            }
        }))
        .turboquant_mode(adapter.turboquant_mode.as_ref().and_then(|t| match t {
            TurboQuantMode::Auto => None,
            TurboQuantMode::V4 => Some("v4".into()),
            TurboQuantMode::K8V4 => Some("k8v4".into()),
            TurboQuantMode::Off => Some("none".into()),
        }))
        .prefix_cache_policy(adapter.prefix_cache_policy.clone())
        .hybrid_cache_entries(adapter.hybrid_cache_entries)
        .pflash_policy(adapter.pflash_policy.clone())
        .response_cache_policy(adapter.response_cache_policy.clone())
        .disk_checkpoint_policy(adapter.disk_checkpoint_policy.clone())
        .max_num_seqs(adapter.max_num_seqs)
        .max_concurrent_requests(adapter.max_concurrent_requests)
        .prefill_batch_size(adapter.prefill_batch_size)
        .completion_batch_size(adapter.completion_batch_size)
        .batching_policy(adapter.batching_policy.clone())
        .concurrency_policy(adapter.concurrency_policy.clone())
        .reasoning_mode(adapter.reasoning_mode.clone())
        .speculative_policy(adapter.speculative_policy.clone())
        .mllm_vision(adapter.mllm_vision.clone())
        .embeddings(adapter.embeddings.clone())
        .gpu_memory_utilization(adapter.gpu_memory_utilization)
        .web_ui_availability(adapter.web_ui_availability.clone())
        .web_ui_static_path(adapter.web_ui_static_path.clone())
        .web_ui_config_json(adapter.web_ui_config_json.clone())
        .endpoint_compatibility(adapter.endpoint_compatibility.clone())
        .request_safety_policy(adapter.request_safety_policy.clone())
        .sampling_mode(adapter.sampling_mode.clone())
        .parser_policy(adapter.parser_policy.clone())
        .security_policy(adapter.security_policy.clone())
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inference::rapid_mlx::runtime::RuntimeSource;

    #[test]
    fn phase7_config_serialization_roundtrip() {
        // Verify RapidMlxConfig with all Phase 7 fields serializes/deserializes without loss.
        let config = RapidMlxConfig {
            model_path: "/model".into(),
            kv_cache_dtype: Some(KvCacheConfig::Fp16),
            turboquant_mode: Some(TurboQuantMode::K8V4),
            prefix_cache_policy: Some("auto".into()),
            hybrid_cache_entries: Some(100),
            pflash_policy: Some("auto".into()),
            response_cache_policy: Some("auto".into()),
            disk_checkpoint_policy: Some("auto".into()),
            max_num_seqs: Some(8),
            max_concurrent_requests: Some(32),
            prefill_batch_size: Some(256),
            completion_batch_size: Some(64),
            batching_policy: Some("auto".into()),
            concurrency_policy: Some("single_active".into()),
            reasoning_mode: Some("auto".into()),
            speculative_policy: Some("auto".into()),
            mllm_vision: Some("auto".into()),
            embeddings: Some("auto".into()),
            gpu_memory_utilization: Some(0.85),
            web_ui_availability: Some("auto".into()),
            web_ui_static_path: Some("ui/".into()),
            web_ui_config_json: Some("{}".into()),
            endpoint_compatibility: Some("openai_v1".into()),
            request_safety_policy: Some("auto".into()),
            sampling_mode: Some("auto".into()),
            parser_policy: Some("auto".into()),
            security_policy: Some("loopback_only".into()),
            ..Default::default()
        };
        let json = serde_json::to_value(&config).unwrap();
        let restored: RapidMlxConfig = serde_json::from_value(json).unwrap();
        assert_eq!(restored.kv_cache_dtype, config.kv_cache_dtype);
        assert_eq!(restored.turboquant_mode, config.turboquant_mode);
        assert_eq!(restored.max_num_seqs, config.max_num_seqs);
        assert_eq!(
            restored.gpu_memory_utilization,
            config.gpu_memory_utilization
        );
        assert_eq!(restored.web_ui_availability, config.web_ui_availability);
        assert_eq!(restored.security_policy, config.security_policy);
    }

    #[test]
    fn phase7_all_settings_iterate_without_panic() {
        // Verify all_settings() can be fully iterated and each returns valid default.
        use crate::inference::rapid_mlx::settings::all_settings;
        for setting in all_settings() {
            let default = setting.default_value();
            assert!(
                default.is_string()
                    || default.is_number()
                    || default.is_object()
                    || default.is_null()
            );
        }
    }

    #[test]
    fn adapter_reuses_poller_per_port_and_separates_ports() {
        let adapter = RapidMlxAdapter::from_resolved(
            RuntimeMetadata {
                executable_path: "rapid-mlx".into(),
                source: RuntimeSource::Managed,
                version: "0.10.10".into(),
                capability_snapshot: None,
                resolved_receipt: None,
                last_probe_result: None,
                prefix_cache_enabled: false,
                prefix_cache_budget_bytes: None,
            },
            ResolvedRapidMlxLaunchModel::validated_alias("model").unwrap(),
        );
        let first = adapter.poller_for(8000).unwrap();
        let second = adapter.poller_for(8000).unwrap();
        let other = adapter.poller_for(8001).unwrap();
        assert!(std::sync::Arc::ptr_eq(&first, &second));
        assert!(!std::sync::Arc::ptr_eq(&first, &other));
    }
}

/// Derive CapabilitySet from a capability snapshot's qualified features.
fn capabilities_from_snapshot(snapshot: &self::capabilities::CapabilitySnapshot) -> CapabilitySet {
    fn is_available(q: &self::capabilities::FeatureQualification) -> bool {
        matches!(q, self::capabilities::FeatureQualification::Available)
    }

    CapabilitySet {
        tool_parsing: is_available(&snapshot.qualified_features.tool_parsing),
        automatic_tool_choice: is_available(&snapshot.qualified_features.automatic_tool_choice),
        reasoning_parser: is_available(&snapshot.qualified_features.reasoning_parser),
        thinking_controls: is_available(&snapshot.qualified_features.thinking_controls),
        guided_generation: is_available(&snapshot.qualified_features.guided_generation),
        vision: is_available(&snapshot.qualified_features.vision),
        embeddings: is_available(&snapshot.qualified_features.embeddings),
        // Core capabilities always available when runtime is validated
        status_memory_telemetry: true,
        one_shot_launch: true,
        ..Default::default()
    }
}

/// Derive chat fields from a capability snapshot.
fn chat_fields_from_snapshot(
    snapshot: &self::capabilities::CapabilitySnapshot,
) -> BTreeSet<&'static str> {
    let mut fields = provisional_chat_fields();

    // Always add fields available on verified runtime
    fields.extend([
        "stream_options",
        "presence_penalty",
        "frequency_penalty",
        "logprobs",
        "timeout",
    ]);

    // Add tool fields if tool parsing is available
    if matches!(
        snapshot.qualified_features.tool_parsing,
        self::capabilities::FeatureQualification::Available
    ) {
        fields.extend(["tools", "tool_choice", "parallel_tool_calls"]);
    }

    // Add response_format if guided is available (structured generation)
    if matches!(
        snapshot.qualified_features.guided_generation,
        self::capabilities::FeatureQualification::Available
    ) {
        fields.insert("response_format");
    }

    // Add thinking fields if reasoning/thinking controls available
    if matches!(
        snapshot.qualified_features.thinking_controls,
        self::capabilities::FeatureQualification::Available
    ) {
        fields.extend([
            "enable_thinking",
            "chat_template_kwargs",
            "reasoning_effort",
        ]);
    }

    fields
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
                version: compatibility::LATEST_QUALIFIED_VERSION_TEXT.into(),
                capability_snapshot: None,
                resolved_receipt: None,
                last_probe_result: None,
                prefix_cache_enabled: false,
                prefix_cache_budget_bytes: None,
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
    fn reasoning_defaults_fill_omitted_fields_without_overriding_caller_choice() {
        let mut with_defaults = adapter();
        with_defaults.enable_thinking = Some(true);
        with_defaults.reasoning_effort = Some("high".into());

        let mapped = with_defaults
            .map_chat_request(br#"{"messages":[],"stream":false}"#)
            .unwrap();
        let value: serde_json::Value = serde_json::from_slice(&mapped).unwrap();
        assert_eq!(value["enable_thinking"], true);
        assert_eq!(value["reasoning_effort"], "high");

        let mapped = with_defaults
            .map_chat_request(
                br#"{"messages":[],"stream":false,"enable_thinking":false,"reasoning_effort":"low"}"#,
            )
            .unwrap();
        let value: serde_json::Value = serde_json::from_slice(&mapped).unwrap();
        assert_eq!(value["enable_thinking"], false);
        assert_eq!(value["reasoning_effort"], "low");
    }

    #[test]
    fn no_reasoning_defaults_configured_leaves_request_untouched() {
        let mapped = adapter()
            .map_chat_request(br#"{"messages":[],"stream":false}"#)
            .unwrap();
        let value: serde_json::Value = serde_json::from_slice(&mapped).unwrap();
        assert!(value.get("enable_thinking").is_none());
        assert!(value.get("reasoning_effort").is_none());
    }

    #[test]
    fn rapid_mapping_rejects_malformed_or_message_less_requests() {
        assert!(adapter().map_chat_request(b"not json").is_err());
        assert!(adapter().map_chat_request(br#"{"stream":true}"#).is_err());
    }
}
