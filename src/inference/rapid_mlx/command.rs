use crate::inference::rapid_mlx::compatibility::ServeCapabilities;
use crate::inference::rapid_mlx::model_resolver::{
    RapidMlxModelSource, ResolvedRapidMlxLaunchModel,
};
use crate::inference::supervisor::SupervisedLaunch;
use anyhow::Result;
use std::ffi::OsString;
use std::path::PathBuf;

pub struct RapidMlxCommandBuilder {
    model: ResolvedRapidMlxLaunchModel,
    served_model_name: Option<String>,
    host: String,
    port: u16,
    log_level: Option<String>,
    timeout: Option<u32>,
    max_cache_blocks: Option<u32>,
    api_key: Option<String>,
    tool_call_parser: Option<String>,
    auto_tool_choice: bool,
    no_thinking: bool,
    trust_remote_code_consent: Option<String>,
    escape_hatch_flags: Vec<(String, serde_json::Value)>,
    // Phase 7: KV/cache policy
    kv_cache_dtype: Option<KvCacheDtypeArg>,
    turboquant_mode: Option<String>,
    prefix_cache_policy: Option<String>,
    hybrid_cache_entries: Option<u64>,
    pflash_policy: Option<String>,
    response_cache_policy: Option<String>,
    disk_checkpoint_policy: Option<String>,
    // Phase 7: batching/concurrency
    max_num_seqs: Option<u64>,
    max_concurrent_requests: Option<u64>,
    prefill_batch_size: Option<u64>,
    completion_batch_size: Option<u64>,
    batching_policy: Option<String>,
    concurrency_policy: Option<String>,
    // Phase 7: reasoning/speculative
    reasoning_mode: Option<String>,
    speculative_policy: Option<String>,
    // Phase 7: MLLM/embeddings
    mllm_vision: Option<String>,
    embeddings: Option<String>,
    // Phase 7: GPU
    gpu_memory_utilization: Option<f64>,
    // Phase 7: Web UI
    web_ui_availability: Option<String>,
    web_ui_static_path: Option<String>,
    web_ui_config_json: Option<String>,
    // Phase 7: endpoint/safety
    endpoint_compatibility: Option<String>,
    request_safety_policy: Option<String>,
    sampling_mode: Option<String>,
    parser_policy: Option<String>,
    security_policy: Option<String>,
}

/// KV cache dtype argument for CLI.
#[derive(Debug, Clone)]
pub enum KvCacheDtypeArg {
    Auto,
    Explicit(String),
}

impl RapidMlxCommandBuilder {
    pub fn new(model: ResolvedRapidMlxLaunchModel) -> Self {
        Self {
            model,
            served_model_name: None,
            host: "127.0.0.1".to_string(),
            port: 8000,
            log_level: None,
            timeout: None,
            max_cache_blocks: None,
            api_key: None,
            tool_call_parser: None,
            auto_tool_choice: false,
            no_thinking: false,
            trust_remote_code_consent: None,
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
        }
    }

    pub fn served_model_name(mut self, name: String) -> Self {
        self.served_model_name = Some(name);
        self
    }

    pub fn host(mut self, host: String) -> Self {
        self.host = host;
        self
    }

    pub fn port(mut self, port: u16) -> Self {
        self.port = port;
        self
    }

    pub fn log_level(mut self, level: String) -> Self {
        self.log_level = Some(level);
        self
    }

    pub fn timeout(mut self, timeout: u32) -> Self {
        self.timeout = Some(timeout);
        self
    }

    pub fn max_cache_blocks(mut self, blocks: u32) -> Self {
        self.max_cache_blocks = Some(blocks);
        self
    }

    pub fn api_key(mut self, api_key: String) -> Self {
        self.api_key = Some(api_key);
        self
    }

    pub fn tool_call_parser(mut self, parser: Option<String>) -> Self {
        self.tool_call_parser = parser;
        self
    }

    pub fn auto_tool_choice(mut self, enable: bool) -> Self {
        self.auto_tool_choice = enable;
        self
    }

    pub fn no_thinking(mut self, enable: bool) -> Self {
        self.no_thinking = enable;
        self
    }

    pub fn trust_remote_code_consent(mut self, consent: Option<String>) -> Self {
        self.trust_remote_code_consent = consent;
        self
    }

    pub fn escape_hatch_flags(mut self, flags: Vec<(String, serde_json::Value)>) -> Self {
        self.escape_hatch_flags = flags;
        self
    }
    // Phase 7 setters
    pub fn kv_cache_dtype(mut self, dtype: Option<KvCacheDtypeArg>) -> Self {
        self.kv_cache_dtype = dtype;
        self
    }
    pub fn turboquant_mode(mut self, mode: Option<String>) -> Self {
        self.turboquant_mode = mode;
        self
    }
    pub fn prefix_cache_policy(mut self, policy: Option<String>) -> Self {
        self.prefix_cache_policy = policy;
        self
    }
    pub fn hybrid_cache_entries(mut self, entries: Option<u64>) -> Self {
        self.hybrid_cache_entries = entries;
        self
    }
    pub fn pflash_policy(mut self, policy: Option<String>) -> Self {
        self.pflash_policy = policy;
        self
    }
    pub fn response_cache_policy(mut self, policy: Option<String>) -> Self {
        self.response_cache_policy = policy;
        self
    }
    pub fn disk_checkpoint_policy(mut self, policy: Option<String>) -> Self {
        self.disk_checkpoint_policy = policy;
        self
    }
    pub fn max_num_seqs(mut self, seqs: Option<u64>) -> Self {
        self.max_num_seqs = seqs;
        self
    }
    pub fn max_concurrent_requests(mut self, requests: Option<u64>) -> Self {
        self.max_concurrent_requests = requests;
        self
    }
    pub fn prefill_batch_size(mut self, size: Option<u64>) -> Self {
        self.prefill_batch_size = size;
        self
    }
    pub fn completion_batch_size(mut self, size: Option<u64>) -> Self {
        self.completion_batch_size = size;
        self
    }
    pub fn batching_policy(mut self, policy: Option<String>) -> Self {
        self.batching_policy = policy;
        self
    }
    pub fn concurrency_policy(mut self, policy: Option<String>) -> Self {
        self.concurrency_policy = policy;
        self
    }
    pub fn reasoning_mode(mut self, mode: Option<String>) -> Self {
        self.reasoning_mode = mode;
        self
    }
    pub fn speculative_policy(mut self, policy: Option<String>) -> Self {
        self.speculative_policy = policy;
        self
    }
    pub fn mllm_vision(mut self, vision: Option<String>) -> Self {
        self.mllm_vision = vision;
        self
    }
    pub fn embeddings(mut self, emb: Option<String>) -> Self {
        self.embeddings = emb;
        self
    }
    pub fn gpu_memory_utilization(mut self, util: Option<f64>) -> Self {
        self.gpu_memory_utilization = util;
        self
    }
    pub fn web_ui_availability(mut self, avail: Option<String>) -> Self {
        self.web_ui_availability = avail;
        self
    }
    pub fn web_ui_static_path(mut self, path: Option<String>) -> Self {
        self.web_ui_static_path = path;
        self
    }
    pub fn web_ui_config_json(mut self, config: Option<String>) -> Self {
        self.web_ui_config_json = config;
        self
    }
    pub fn endpoint_compatibility(mut self, compat: Option<String>) -> Self {
        self.endpoint_compatibility = compat;
        self
    }
    pub fn request_safety_policy(mut self, policy: Option<String>) -> Self {
        self.request_safety_policy = policy;
        self
    }
    pub fn sampling_mode(mut self, mode: Option<String>) -> Self {
        self.sampling_mode = mode;
        self
    }
    pub fn parser_policy(mut self, policy: Option<String>) -> Self {
        self.parser_policy = policy;
        self
    }
    pub fn security_policy(mut self, policy: Option<String>) -> Self {
        self.security_policy = policy;
        self
    }

    pub fn build(
        self,
        binary_path: PathBuf,
        capabilities: &ServeCapabilities,
    ) -> Result<SupervisedLaunch> {
        let mut args = vec!["serve".to_string()];
        args.push(self.model.launch_argument.clone());

        if let Some(name) = self.served_model_name {
            capabilities.require("--served-model-name")?;
            args.push("--served-model-name".to_string());
            args.push(name);
        }

        capabilities.require("--host")?;
        args.push("--host".to_string());
        args.push(self.host);

        capabilities.require("--port")?;
        args.push("--port".to_string());
        args.push(self.port.to_string());

        if let Some(log_level) = self.log_level {
            capabilities.require("--log-level")?;
            args.push("--log-level".to_string());
            args.push(log_level);
        }

        if let Some(timeout) = self.timeout {
            capabilities.require("--timeout")?;
            args.push("--timeout".to_string());
            args.push(timeout.to_string());
        }

        if let Some(blocks) = self.max_cache_blocks {
            capabilities.require("--max-cache-blocks")?;
            args.push("--max-cache-blocks".to_string());
            args.push(blocks.to_string());
        }

        // Diagnostic fix flags — not guarded by capability checks since they are
        // only activated by the diagnostics panel, never by default.
        if let Some(parser) = self.tool_call_parser {
            args.push("--tool-call-parser".to_string());
            args.push(parser);
        }
        if self.auto_tool_choice {
            args.push("--enable-auto-tool-choice".to_string());
        }
        if self.no_thinking {
            args.push("--no-thinking".to_string());
        }

        // Apply validated escape-hatch flags (already allowlisted at load time).
        // Bool flags are boolean switches: true = presence of flag, false = omitted.
        for (name, value) in &self.escape_hatch_flags {
            match value {
                serde_json::Value::Bool(true) => {
                    args.push(format!("--{}", name));
                }
                serde_json::Value::Bool(false) => {
                    // Omitted: false means "use default" for switch flags.
                }
                _ => {
                    args.push(format!("--{}", name));
                    args.push(serde_value_to_flag_arg(value));
                }
            }
        }
        // Phase 7: KV/cache policy flags
        if let Some(ref dtype) = self.kv_cache_dtype {
            match dtype {
                KvCacheDtypeArg::Explicit(effective) => {
                    capabilities.require("--kv-cache-dtype")?;
                    args.push("--kv-cache-dtype".to_string());
                    args.push(format!("{{\"effective\":\"{}\"}}", effective));
                }
                KvCacheDtypeArg::Auto => {}
            }
        }
        if let Some(ref mode) = self.turboquant_mode
            && *mode != "none"
        {
            capabilities.require("--turboquant")?;
            args.push("--turboquant".to_string());
            args.push(mode.clone());
        }
        if let Some(ref policy) = self.prefix_cache_policy
            && policy != "auto"
        {
            capabilities.require("--max-cache-blocks")?;
            args.push("--max-cache-blocks".to_string());
            args.push(policy.clone());
        }
        if let Some(entries) = self.hybrid_cache_entries {
            capabilities.require("--hybrid-cache-entries")?;
            args.push("--hybrid-cache-entries".to_string());
            args.push(entries.to_string());
        }
        if let Some(ref policy) = self.pflash_policy
            && policy != "auto"
        {
            capabilities.require("--pflash")?;
            args.push("--pflash".to_string());
            args.push(policy.clone());
        }
        if let Some(ref policy) = self.response_cache_policy
            && policy != "auto"
        {
            capabilities.require("--response-cache")?;
            args.push("--response-cache".to_string());
            args.push(policy.clone());
        }
        if let Some(ref policy) = self.disk_checkpoint_policy
            && policy != "auto"
        {
            capabilities.require("--disk-checkpoint")?;
            args.push("--disk-checkpoint".to_string());
            args.push(policy.clone());
        }
        // Phase 7: batching/concurrency flags
        if let Some(seqs) = self.max_num_seqs {
            capabilities.require("--max-num-seqs")?;
            args.push("--max-num-seqs".to_string());
            args.push(seqs.to_string());
        }
        if let Some(requests) = self.max_concurrent_requests {
            capabilities.require("--max-concurrent-requests")?;
            args.push("--max-concurrent-requests".to_string());
            args.push(requests.to_string());
        }
        if let Some(size) = self.prefill_batch_size {
            capabilities.require("--prefill-batch-size")?;
            args.push("--prefill-batch-size".to_string());
            args.push(size.to_string());
        }
        if let Some(size) = self.completion_batch_size {
            capabilities.require("--completion-batch-size")?;
            args.push("--completion-batch-size".to_string());
            args.push(size.to_string());
        }
        if let Some(ref policy) = self.batching_policy
            && policy != "auto"
        {
            capabilities.require("--batching-policy")?;
            args.push("--batching-policy".to_string());
            args.push(policy.clone());
        }
        if let Some(ref policy) = self.concurrency_policy
            && policy != "single_active"
        {
            capabilities.require("--concurrency-policy")?;
            args.push("--concurrency-policy".to_string());
            args.push(policy.clone());
        }
        // Phase 7: reasoning/speculative flags
        if let Some(ref mode) = self.reasoning_mode
            && mode != "auto"
        {
            capabilities.require("--reasoning")?;
            args.push("--reasoning".to_string());
            args.push(mode.clone());
        }
        if let Some(ref policy) = self.speculative_policy {
            capabilities.require("--speculative")?;
            args.push("--speculative".to_string());
            args.push(policy.clone());
        }
        // Phase 7: MLLM/embeddings flags
        if let Some(ref vision) = self.mllm_vision
            && vision != "auto"
        {
            capabilities.require("--vision")?;
            args.push("--vision".to_string());
            args.push(vision.clone());
        }
        if let Some(ref emb) = self.embeddings
            && emb != "auto"
        {
            capabilities.require("--embeddings")?;
            args.push("--embeddings".to_string());
            args.push(emb.clone());
        }
        // Phase 7: GPU flags
        if let Some(util) = self.gpu_memory_utilization {
            capabilities.require("--gpu-memory-utilization")?;
            args.push("--gpu-memory-utilization".to_string());
            args.push(util.to_string());
        }
        // Phase 7: Web UI flags (D26/A44)
        if let Some(ref avail) = self.web_ui_availability
            && avail != "auto"
        {
            if avail == "off" {
                capabilities.require("--no-ui")?;
                args.push("--no-ui".to_string());
            } else {
                capabilities.require("--ui")?;
                args.push("--ui".to_string());
                args.push(avail.clone());
            }
        }
        if let Some(ref path) = self.web_ui_static_path {
            capabilities.require("--path")?;
            args.push("--path".to_string());
            args.push(path.clone());
        }
        if let Some(ref config) = self.web_ui_config_json {
            capabilities.require("--ui-config")?;
            args.push("--ui-config".to_string());
            args.push(config.clone());
        }
        // Phase 7: endpoint/safety flags
        if let Some(ref compat) = self.endpoint_compatibility
            && compat != "openai_v1"
        {
            capabilities.require("--endpoint-compatibility")?;
            args.push("--endpoint-compatibility".to_string());
            args.push(compat.clone());
        }
        if let Some(ref policy) = self.request_safety_policy
            && policy != "auto"
        {
            capabilities.require("--request-safety-policy")?;
            args.push("--request-safety-policy".to_string());
            args.push(policy.clone());
        }
        if let Some(ref mode) = self.sampling_mode
            && mode != "auto"
        {
            capabilities.require("--sampling-mode")?;
            args.push("--sampling-mode".to_string());
            args.push(mode.clone());
        }
        if let Some(ref policy) = self.parser_policy
            && policy != "auto"
        {
            capabilities.require("--parser-policy")?;
            args.push("--parser-policy".to_string());
            args.push(policy.clone());
        }
        if let Some(ref policy) = self.security_policy
            && policy != "loopback_only"
        {
            capabilities.require("--security-policy")?;
            args.push("--security-policy".to_string());
            args.push(policy.clone());
        }

        let os_args: Vec<OsString> = args.into_iter().map(OsString::from).collect();

        // Prevent Rapid-MLX's first-run telemetry question from blocking an
        // unattended app launch. The user can opt in outside this process.
        let mut env = vec![(OsString::from("RAPID_MLX_TELEMETRY"), OsString::from("0"))];
        if let Some(key) = self.api_key {
            env.push((OsString::from("RAPID_MLX_API_KEY"), OsString::from(key)));
        }
        env.extend(
            self.model
                .environment()
                .map(|(name, value)| (name.clone(), value.clone())),
        );

        // Security: enforce revision-scoped consent for repos requiring trust_remote_code.
        // When the resolved model marks trust_remote_code_required=true, launch is blocked
        // unless the user has explicitly consented for that specific repo@revision.
        if self.model.trust_remote_code_required == Some(true) {
            validate_trust_consent(&self.model, &self.trust_remote_code_consent)?;
            env.push((OsString::from("HF_TRUST_REMOTE_CODE"), OsString::from("1")));
        }

        Ok(SupervisedLaunch {
            program: binary_path,
            args: os_args,
            env,
            cwd: None,
            port: self.port,
            redacted_summary: format!(
                "Rapid-MLX serve: {} on port {}",
                self.model.display_name, self.port
            ),
        })
    }
}

/// Validate trust_remote_code consent matches "repo_id@revision" format and corresponds to the
/// resolved model's HF source. Blocks launch on missing consent, format error, or mismatch.
fn validate_trust_consent(
    model: &ResolvedRapidMlxLaunchModel,
    consent: &Option<String>,
) -> Result<()> {
    let consent_str = consent
        .as_deref()
        .ok_or_else(|| anyhow::anyhow!("This model requires trust_remote_code (custom Python code execution). Consent must be granted for this specific repo and revision before launching."))?;

    if consent_str.is_empty() {
        anyhow::bail!("trust_remote_code consent must not be empty");
    }

    let (consent_repo, consent_revision) = consent_str.rsplit_once('@').ok_or_else(|| {
        anyhow::anyhow!(
            "trust_remote_code consent must be in format repo_id@revision (e.g. org/model@main)"
        )
    })?;

    match &model.original_input {
        RapidMlxModelSource::HuggingFaceRepo { repo_id, revision } => {
            if consent_repo != repo_id {
                anyhow::bail!(
                    "trust_remote_code consent repo mismatch: expected {repo_id}, got {consent_repo}"
                );
            }
            if consent_revision != revision {
                anyhow::bail!(
                    "trust_remote_code consent revision mismatch for {repo_id}: expected {revision}, got {consent_revision}"
                );
            }
        }
        RapidMlxModelSource::MlxDirectory { .. }
        | RapidMlxModelSource::GgufFile { .. }
        | RapidMlxModelSource::Alias { .. }
        | RapidMlxModelSource::AuthoritativeSafetensors { .. } => {
            anyhow::bail!(
                "trust_remote_code consent requires an HF repo source; model source kind does not support revision-scoped consent"
            );
        }
    }

    Ok(())
}

fn serde_value_to_flag_arg(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::Bool(true) => String::new(),
        serde_json::Value::Bool(false) => String::new(),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                i.to_string()
            } else if let Some(f) = n.as_f64() {
                format!("{f}")
            } else {
                String::new()
            }
        }
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Array(arr) => arr
            .iter()
            .map(serde_value_to_flag_arg)
            .collect::<Vec<_>>()
            .join(","),
        _ => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(launch: &SupervisedLaunch) -> Vec<String> {
        launch
            .args
            .iter()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect()
    }

    #[test]
    fn secure_defaults_omit_upstream_default_tuning_flags() {
        let launch = RapidMlxCommandBuilder::new(
            ResolvedRapidMlxLaunchModel::validated_alias("model").unwrap(),
        )
        .build("rapid-mlx".into(), &ServeCapabilities::verified_baseline())
        .unwrap();
        assert_eq!(
            args(&launch),
            ["serve", "model", "--host", "127.0.0.1", "--port", "8000"]
        );
        assert!(
            launch
                .env
                .iter()
                .any(|(name, value)| { name == "RAPID_MLX_TELEMETRY" && value == "0" })
        );
    }

    #[test]
    fn current_flag_names_and_secret_environment_are_used() {
        let launch = RapidMlxCommandBuilder::new(
            ResolvedRapidMlxLaunchModel::validated_alias("model").unwrap(),
        )
        .timeout(90)
        .max_cache_blocks(200)
        .api_key("do-not-log".into())
        .build("rapid-mlx".into(), &ServeCapabilities::verified_baseline())
        .unwrap();
        let args = args(&launch);
        assert!(args.windows(2).any(|pair| pair == ["--timeout", "90"]));
        assert!(
            args.windows(2)
                .any(|pair| pair == ["--max-cache-blocks", "200"])
        );
        assert!(!args.iter().any(|arg| arg == "--request-timeout"));
        assert!(!args.iter().any(|arg| arg == "--max-blocks"));
        assert!(
            launch
                .env
                .iter()
                .any(|(name, value)| { name == "RAPID_MLX_API_KEY" && value == "do-not-log" })
        );
        assert!(!launch.redacted_summary.contains("do-not-log"));
    }

    #[test]
    fn explicitly_configured_unsupported_option_fails_closed() {
        let capabilities = ServeCapabilities::from_help("--host --port");
        let error = RapidMlxCommandBuilder::new(
            ResolvedRapidMlxLaunchModel::validated_alias("model").unwrap(),
        )
        .timeout(90)
        .build("rapid-mlx".into(), &capabilities)
        .unwrap_err();
        assert!(error.to_string().contains("--timeout"));
    }

    #[test]
    fn escape_hatch_flags_are_applied_correctly() {
        let flags = vec![
            ("force-hybrid".into(), serde_json::Value::Bool(true)),
            ("no-hybrid".into(), serde_json::Value::Bool(false)),
            ("pflash".into(), serde_json::Value::String("always".into())),
            (
                "pflash-threshold".into(),
                serde_json::Value::Number(serde_json::Number::from(128)),
            ),
            (
                "pflash-keep-ratio".into(),
                serde_json::Value::Number(serde_json::Number::from_f64(0.7).unwrap()),
            ),
        ];
        let launch = RapidMlxCommandBuilder::new(
            ResolvedRapidMlxLaunchModel::validated_alias("model").unwrap(),
        )
        .escape_hatch_flags(flags)
        .build("rapid-mlx".into(), &ServeCapabilities::verified_baseline())
        .unwrap();
        let args = args(&launch);
        assert!(args.contains(&"--force-hybrid".to_string()));
        assert!(!args.contains(&"--no-hybrid".to_string()));
        assert!(args.windows(2).any(|p| p == ["--pflash", "always"]));
        assert!(args.windows(2).any(|p| p == ["--pflash-threshold", "128"]));
        assert!(args.windows(2).any(|p| p == ["--pflash-keep-ratio", "0.7"]));
    }

    #[test]
    fn trust_consent_blocks_without_consent() {
        let model = ResolvedRapidMlxLaunchModel {
            launch_argument: "org/model".into(),
            display_name: "org/model".into(),
            source_kind: crate::inference::rapid_mlx::model_resolver::ResolvedRapidMlxSourceKind::FreeFormAlias,
            original_input: RapidMlxModelSource::HuggingFaceRepo {
                repo_id: "org/model".into(),
                revision: "main".into(),
            },
            conversion: None,
            required_environment: Vec::new(),
            warnings: Vec::new(),
            remediation: Vec::new(),
            trust_remote_code_required: Some(true),
            environment: std::collections::BTreeMap::new(),
        };
        let launch = RapidMlxCommandBuilder::new(model)
            .build("rapid-mlx".into(), &ServeCapabilities::verified_baseline());
        let err = launch.unwrap_err().to_string();
        assert!(
            err.contains("trust_remote_code"),
            "expected trust error, got: {err}"
        );
    }

    #[test]
    fn trust_consent_accepts_valid_match() {
        let model = ResolvedRapidMlxLaunchModel {
            launch_argument: "org/model".into(),
            display_name: "org/model".into(),
            source_kind: crate::inference::rapid_mlx::model_resolver::ResolvedRapidMlxSourceKind::FreeFormAlias,
            original_input: RapidMlxModelSource::HuggingFaceRepo {
                repo_id: "org/model".into(),
                revision: "main".into(),
            },
            conversion: None,
            required_environment: Vec::new(),
            warnings: Vec::new(),
            remediation: Vec::new(),
            trust_remote_code_required: Some(true),
            environment: std::collections::BTreeMap::new(),
        };
        let launch = RapidMlxCommandBuilder::new(model)
            .trust_remote_code_consent(Some("org/model@main".into()))
            .build("rapid-mlx".into(), &ServeCapabilities::verified_baseline());
        assert!(launch.is_ok(), "unexpected error: {:?}", launch);
        let envs: Vec<_> = launch.unwrap().env;
        assert!(
            envs.iter()
                .any(|(n, v)| n == "HF_TRUST_REMOTE_CODE" && v == "1")
        );
    }

    #[test]
    fn trust_consent_rejects_repo_mismatch() {
        let model = ResolvedRapidMlxLaunchModel {
            launch_argument: "org/model".into(),
            display_name: "org/model".into(),
            source_kind: crate::inference::rapid_mlx::model_resolver::ResolvedRapidMlxSourceKind::FreeFormAlias,
            original_input: RapidMlxModelSource::HuggingFaceRepo {
                repo_id: "org/model".into(),
                revision: "main".into(),
            },
            conversion: None,
            required_environment: Vec::new(),
            warnings: Vec::new(),
            remediation: Vec::new(),
            trust_remote_code_required: Some(true),
            environment: std::collections::BTreeMap::new(),
        };
        let launch = RapidMlxCommandBuilder::new(model)
            .trust_remote_code_consent(Some("other/model@main".into()))
            .build("rapid-mlx".into(), &ServeCapabilities::verified_baseline());
        let err = launch.unwrap_err().to_string();
        assert!(
            err.contains("repo mismatch"),
            "expected repo mismatch, got: {err}"
        );
    }

    #[test]
    fn trust_consent_rejects_revision_mismatch() {
        let model = ResolvedRapidMlxLaunchModel {
            launch_argument: "org/model".into(),
            display_name: "org/model".into(),
            source_kind: crate::inference::rapid_mlx::model_resolver::ResolvedRapidMlxSourceKind::FreeFormAlias,
            original_input: RapidMlxModelSource::HuggingFaceRepo {
                repo_id: "org/model".into(),
                revision: "main".into(),
            },
            conversion: None,
            required_environment: Vec::new(),
            warnings: Vec::new(),
            remediation: Vec::new(),
            trust_remote_code_required: Some(true),
            environment: std::collections::BTreeMap::new(),
        };
        let launch = RapidMlxCommandBuilder::new(model)
            .trust_remote_code_consent(Some("org/model@bad-revision".into()))
            .build("rapid-mlx".into(), &ServeCapabilities::verified_baseline());
        let err = launch.unwrap_err().to_string();
        assert!(
            err.contains("revision mismatch"),
            "expected revision mismatch, got: {err}"
        );
    }

    #[test]
    fn trust_consent_rejects_invalid_format() {
        let model = ResolvedRapidMlxLaunchModel {
            launch_argument: "org/model".into(),
            display_name: "org/model".into(),
            source_kind: crate::inference::rapid_mlx::model_resolver::ResolvedRapidMlxSourceKind::FreeFormAlias,
            original_input: RapidMlxModelSource::HuggingFaceRepo {
                repo_id: "org/model".into(),
                revision: "main".into(),
            },
            conversion: None,
            required_environment: Vec::new(),
            warnings: Vec::new(),
            remediation: Vec::new(),
            trust_remote_code_required: Some(true),
            environment: std::collections::BTreeMap::new(),
        };
        let launch = RapidMlxCommandBuilder::new(model)
            .trust_remote_code_consent(Some("just-repo".into()))
            .build("rapid-mlx".into(), &ServeCapabilities::verified_baseline());
        let err = launch.unwrap_err().to_string();
        assert!(
            err.contains("repo_id@revision"),
            "expected format error, got: {err}"
        );
    }

    #[test]
    fn trust_consent_rejects_non_hf_source() {
        let model = ResolvedRapidMlxLaunchModel {
            launch_argument: "/local/path".into(),
            display_name: "/local/path".into(),
            source_kind: crate::inference::rapid_mlx::model_resolver::ResolvedRapidMlxSourceKind::FreeFormAlias,
            original_input: RapidMlxModelSource::MlxDirectory {
                path: PathBuf::from("/local/path"),
            },
            conversion: None,
            required_environment: Vec::new(),
            warnings: Vec::new(),
            remediation: Vec::new(),
            trust_remote_code_required: Some(true),
            environment: std::collections::BTreeMap::new(),
        };
        let launch = RapidMlxCommandBuilder::new(model)
            .trust_remote_code_consent(Some("org/model@main".into()))
            .build("rapid-mlx".into(), &ServeCapabilities::verified_baseline());
        let err = launch.unwrap_err().to_string();
        assert!(
            err.contains("revision-scoped consent"),
            "expected source error, got: {err}"
        );
    }

    #[test]
    fn phase7_config_produces_valid_argv() {
        use crate::inference::rapid_mlx::compatibility::ServeCapabilities;
        // Phase 7 runtime with all flags present
        let capabilities = ServeCapabilities::from_help(
            "--host --port --served-model-name --timeout --max-cache-blocks \
             --kv-cache-dtype --turboquant --max-num-seqs --max-concurrent-requests \
             --prefill-batch-size --completion-batch-size --batching-policy --concurrency-policy \
             --reasoning --speculative --vision --embeddings --gpu-memory-utilization \
             --ui --no-ui --path --ui-config --pflash --hybrid-cache-entries \
             --response-cache --disk-checkpoint --endpoint-compatibility \
             --request-safety-policy --sampling-mode --parser-policy --security-policy",
        );
        let launch = RapidMlxCommandBuilder::new(
            ResolvedRapidMlxLaunchModel::validated_alias("model").unwrap(),
        )
        .port(9000)
        .kv_cache_dtype(Some(KvCacheDtypeArg::Explicit("int8".into())))
        .turboquant_mode(Some("k8v4".into()))
        .pflash_policy(Some("always".into()))
        .hybrid_cache_entries(Some(512))
        .max_num_seqs(Some(8))
        .max_concurrent_requests(Some(32))
        .prefill_batch_size(Some(256))
        .completion_batch_size(Some(64))
        .batching_policy(Some("fixed".into()))
        .concurrency_policy(Some("allow_overlap".into()))
        .reasoning_mode(Some("on".into()))
        .speculative_policy(Some("mtp_v1".into()))
        .mllm_vision(Some("on".into()))
        .embeddings(Some("on".into()))
        .gpu_memory_utilization(Some(0.85))
        .web_ui_availability(Some("on".into()))
        .web_ui_static_path(Some("custom-ui/".into()))
        .web_ui_config_json(Some("{\"theme\":\"dark\"}".into()))
        .endpoint_compatibility(Some("openai_v1,anthropic".into()))
        .request_safety_policy(Some("strict".into()))
        .sampling_mode(Some("explicit_client".into()))
        .parser_policy(Some("native".into()))
        .security_policy(Some("authenticated".into()))
        .build("rapid-mlx".into(), &capabilities)
        .unwrap();
        let args = args(&launch);
        assert!(
            args.windows(2)
                .any(|p| p == ["--kv-cache-dtype", "{\"effective\":\"int8\"}"])
        );
        assert!(args.windows(2).any(|p| p == ["--turboquant", "k8v4"]));
        assert!(args.windows(2).any(|p| p == ["--pflash", "always"]));
        assert!(
            args.windows(2)
                .any(|p| p == ["--hybrid-cache-entries", "512"])
        );
        assert!(args.windows(2).any(|p| p == ["--max-num-seqs", "8"]));
        assert!(
            args.windows(2)
                .any(|p| p == ["--max-concurrent-requests", "32"])
        );
        assert!(
            args.windows(2)
                .any(|p| p == ["--prefill-batch-size", "256"])
        );
        assert!(
            args.windows(2)
                .any(|p| p == ["--completion-batch-size", "64"])
        );
        assert!(args.windows(2).any(|p| p == ["--batching-policy", "fixed"]));
        assert!(
            args.windows(2)
                .any(|p| p == ["--concurrency-policy", "allow_overlap"])
        );
        assert!(args.windows(2).any(|p| p == ["--reasoning", "on"]));
        assert!(args.windows(2).any(|p| p == ["--speculative", "mtp_v1"]));
        assert!(args.windows(2).any(|p| p == ["--vision", "on"]));
        assert!(args.windows(2).any(|p| p == ["--embeddings", "on"]));
        assert!(
            args.windows(2)
                .any(|p| p == ["--gpu-memory-utilization", "0.85"])
        );
        assert!(args.windows(2).any(|p| p == ["--ui", "on"]));
        assert!(args.windows(2).any(|p| p == ["--path", "custom-ui/"]));
        assert!(
            args.windows(2)
                .any(|p| p == ["--ui-config", "{\"theme\":\"dark\"}"])
        );
        assert!(
            args.windows(2)
                .any(|p| p == ["--endpoint-compatibility", "openai_v1,anthropic"])
        );
        assert!(
            args.windows(2)
                .any(|p| p == ["--request-safety-policy", "strict"])
        );
        assert!(
            args.windows(2)
                .any(|p| p == ["--sampling-mode", "explicit_client"])
        );
        assert!(args.windows(2).any(|p| p == ["--parser-policy", "native"]));
        assert!(
            args.windows(2)
                .any(|p| p == ["--security-policy", "authenticated"])
        );
    }

    #[test]
    fn phase7_auto_defaults_are_omitted() {
        let launch = RapidMlxCommandBuilder::new(
            ResolvedRapidMlxLaunchModel::validated_alias("model").unwrap(),
        )
        .kv_cache_dtype(Some(KvCacheDtypeArg::Auto))
        .turboquant_mode(Some("none".into()))
        .reasoning_mode(Some("auto".into()))
        .mllm_vision(Some("auto".into()))
        .embeddings(Some("auto".into()))
        .web_ui_availability(Some("auto".into()))
        .batching_policy(Some("auto".into()))
        .concurrency_policy(Some("single_active".into()))
        .endpoint_compatibility(Some("openai_v1".into()))
        .request_safety_policy(Some("auto".into()))
        .sampling_mode(Some("auto".into()))
        .parser_policy(Some("auto".into()))
        .security_policy(Some("loopback_only".into()))
        .build("rapid-mlx".into(), &ServeCapabilities::verified_baseline())
        .unwrap();
        let args = args(&launch);
        assert!(!args.iter().any(|a| a.starts_with("--kv-cache-dtype")));
        assert!(!args.iter().any(|a| a.starts_with("--turboquant")));
        assert!(!args.iter().any(|a| a.starts_with("--reasoning")));
        assert!(!args.iter().any(|a| a.starts_with("--vision")));
        assert!(!args.iter().any(|a| a.starts_with("--embeddings")));
        assert!(!args.iter().any(|a| a.starts_with("--ui")));
        assert!(!args.iter().any(|a| a.starts_with("--no-ui")));
        assert!(!args.iter().any(|a| a.starts_with("--batching-policy")));
        assert!(!args.iter().any(|a| a.starts_with("--concurrency-policy")));
        assert!(
            !args
                .iter()
                .any(|a| a.starts_with("--endpoint-compatibility"))
        );
        assert!(
            !args
                .iter()
                .any(|a| a.starts_with("--request-safety-policy"))
        );
        assert!(!args.iter().any(|a| a.starts_with("--sampling-mode")));
        assert!(!args.iter().any(|a| a.starts_with("--parser-policy")));
        assert!(!args.iter().any(|a| a.starts_with("--security-policy")));
    }

    #[test]
    fn phase7_unsupported_flag_fails_closed() {
        let capabilities = ServeCapabilities::from_help("--host --port --served-model-name");
        let error = RapidMlxCommandBuilder::new(
            ResolvedRapidMlxLaunchModel::validated_alias("model").unwrap(),
        )
        .turboquant_mode(Some("k8v4".into()))
        .build("rapid-mlx".into(), &capabilities)
        .unwrap_err();
        assert!(error.to_string().contains("--turboquant"));
    }
}
