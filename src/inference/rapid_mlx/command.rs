use crate::inference::rapid_mlx::compatibility::ServeCapabilities;
use crate::inference::rapid_mlx::model_resolver::ResolvedRapidMlxLaunchModel;
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
    tool_call_parser: bool,
    auto_tool_choice: bool,
    no_thinking: bool,
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
            tool_call_parser: false,
            auto_tool_choice: false,
            no_thinking: false,
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

    pub fn tool_call_parser(mut self, enable: bool) -> Self {
        self.tool_call_parser = enable;
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
        if self.tool_call_parser {
            args.push("--tool-call-parser".to_string());
        }
        if self.auto_tool_choice {
            args.push("--auto-tool-choice".to_string());
        }
        if self.no_thinking {
            args.push("--no-thinking".to_string());
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
}
