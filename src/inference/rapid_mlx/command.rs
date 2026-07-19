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
}
