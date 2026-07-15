use crate::config::AppConfig;
use crate::inference::InferenceBackend;
use crate::inference::backend::BackendAdapter;
use crate::inference::llama_cpp::{LlamaCppAdapter, ServerConfig};
use crate::inference::rapid_mlx::discovery::Discovery;
use crate::inference::rapid_mlx::runtime::RuntimeMetadata;
use crate::inference::rapid_mlx::{RapidMlxAdapter, RapidMlxConfig};
use crate::presets::ModelPreset;
use crate::state::{AppState, Session, SessionMode};
use anyhow::{Context, Result};
use std::path::PathBuf;
use std::sync::Arc;

/// Backend-owned local launch configurations. Adding another runtime extends
/// this enum without flattening its native controls into llama.cpp's config.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(tag = "backend", content = "config", rename_all = "snake_case")]
pub enum LocalLaunchRequest {
    LlamaCpp(Box<ServerConfig>),
    RapidMlx(RapidMlxConfig),
}

impl LocalLaunchRequest {
    pub fn backend(&self) -> InferenceBackend {
        match self {
            Self::LlamaCpp(_) => InferenceBackend::LlamaCpp,
            Self::RapidMlx(_) => InferenceBackend::RapidMlx,
        }
    }

    pub fn port(&self) -> u16 {
        match self {
            Self::LlamaCpp(config) => config.port,
            Self::RapidMlx(config) => config.port,
        }
    }

    pub fn bind_host(&self) -> Option<String> {
        match self {
            Self::LlamaCpp(config) => config.bind_host.clone(),
            Self::RapidMlx(config) => Some(config.host.clone()),
        }
    }

    pub fn api_key(&self) -> Option<String> {
        match self {
            Self::LlamaCpp(config) => config.api_key.clone(),
            Self::RapidMlx(_) => None,
        }
    }

    pub fn model_identity(&self) -> String {
        match self {
            Self::LlamaCpp(config) if !config.model_path.is_empty() => config.model_path.clone(),
            Self::LlamaCpp(config) => config.hf_repo.clone().unwrap_or_default(),
            Self::RapidMlx(config) => config
                .served_model_name
                .clone()
                .unwrap_or_else(|| config.model_path.clone()),
        }
    }

    /// Clone a launch request for session persistence without credentials.
    pub fn for_persistence(&self) -> Self {
        let mut persisted = self.clone();
        if let Self::LlamaCpp(config) = &mut persisted {
            config.api_key = None;
        }
        persisted
    }
}

#[derive(serde::Deserialize)]
struct BackendDiscriminator {
    #[serde(default)]
    backend: InferenceBackend,
}

pub fn request_from_api_payload(payload: &serde_json::Value) -> Result<LocalLaunchRequest> {
    let selected: BackendDiscriminator =
        serde_json::from_value(payload.clone()).context("Invalid inference backend selection")?;
    if selected.backend == InferenceBackend::LlamaCpp && payload.get("rapid_mlx").is_some() {
        anyhow::bail!(
            "llama_cpp launch must not include a backend-owned rapid_mlx configuration object"
        );
    }
    match selected.backend {
        InferenceBackend::LlamaCpp => serde_json::from_value(payload.clone())
            .map(|config| LocalLaunchRequest::LlamaCpp(Box::new(config)))
            .context("Invalid llama.cpp launch configuration"),
        InferenceBackend::RapidMlx => {
            let config = payload.get("rapid_mlx").ok_or_else(|| {
                anyhow::anyhow!(
                    "Rapid-MLX launch requires a backend-owned rapid_mlx configuration object"
                )
            })?;
            let config: RapidMlxConfig = serde_json::from_value(config.clone())
                .context("Invalid Rapid-MLX launch configuration")?;
            if config.port == 0 {
                anyhow::bail!("Rapid-MLX launch requires a non-zero port");
            }
            Ok(LocalLaunchRequest::RapidMlx(config))
        }
    }
}

pub fn validate_preset_backend_config(preset: &ModelPreset) -> Result<()> {
    match preset.backend {
        InferenceBackend::LlamaCpp if preset.rapid_mlx.is_some() => anyhow::bail!(
            "llama_cpp preset '{}' must not include rapid_mlx configuration",
            preset.name
        ),
        InferenceBackend::RapidMlx => {
            let rapid = preset.rapid_mlx.as_ref().ok_or_else(|| {
                anyhow::anyhow!(
                    "Rapid-MLX preset '{}' is missing its rapid_mlx configuration",
                    preset.name
                )
            })?;
            if rapid.model_path.trim().is_empty() {
                anyhow::bail!("Rapid-MLX preset '{}' requires a model_path", preset.name);
            }
            if rapid.port == 0 {
                anyhow::bail!(
                    "Rapid-MLX preset '{}' requires a non-zero port",
                    preset.name
                );
            }
            Ok(())
        }
        InferenceBackend::LlamaCpp => Ok(()),
    }
}

pub fn request_from_preset(
    preset: &ModelPreset,
    port_override: Option<u16>,
) -> Result<LocalLaunchRequest> {
    validate_preset_backend_config(preset)?;
    match preset.backend {
        InferenceBackend::RapidMlx => {
            let mut config = preset.rapid_mlx.clone().ok_or_else(|| {
                anyhow::anyhow!(
                    "Rapid-MLX preset '{}' is missing its rapid_mlx configuration",
                    preset.name
                )
            })?;
            if let Some(port) = port_override {
                config.port = port;
            }
            if config.port == 0 {
                anyhow::bail!("Rapid-MLX launch requires a non-zero port");
            }
            Ok(LocalLaunchRequest::RapidMlx(config))
        }
        InferenceBackend::LlamaCpp => Ok(LocalLaunchRequest::LlamaCpp(Box::new(ServerConfig {
            model_path: preset.model_path.clone(),
            hf_repo: preset.hf_repo.clone(),
            context_size: preset.context_size,
            ctk: preset.ctk.clone(),
            ctv: preset.ctv.clone(),
            tensor_split: preset.tensor_split.clone(),
            batch_size: preset.batch_size,
            ubatch_size: preset.ubatch_size,
            no_mmap: if cfg!(target_os = "macos") && preset.mlock {
                false
            } else {
                preset.no_mmap
            },
            port: port_override.or(preset.port).unwrap_or(8001),
            ngram_spec: preset.ngram_spec,
            parallel_slots: preset.parallel_slots,
            temperature: preset.temperature,
            top_p: preset.top_p,
            top_k: preset.top_k,
            min_p: preset.min_p,
            repeat_penalty: preset.repeat_penalty,
            presence_penalty: preset.presence_penalty,
            n_cpu_moe: preset.n_cpu_moe,
            gpu_layers: preset.gpu_layers,
            mlock: preset.mlock,
            flash_attn: preset.flash_attn.clone(),
            split_mode: preset.split_mode.clone(),
            main_gpu: preset.main_gpu,
            threads: preset.threads,
            threads_batch: preset.threads_batch,
            rope_scaling: preset.rope_scaling.clone(),
            rope_freq_base: preset.rope_freq_base,
            rope_freq_scale: preset.rope_freq_scale,
            spec: crate::inference::llama_cpp::SpecDecodeConfig {
                draft_model: preset.draft_model.clone(),
                draft_min: preset.draft_min,
                draft_max: preset.draft_max,
                spec_ngram_size: preset.spec_ngram_size,
                spec_type: preset.spec_type.clone(),
                spec_default: preset.spec_default,
                spec_draft_n_max: preset.spec_draft_n_max,
                spec_draft_n_min: preset.spec_draft_n_min,
                spec_draft_p_split: preset.spec_draft_p_split,
                spec_draft_p_min: preset.spec_draft_p_min,
                spec_draft_ngl: preset.spec_draft_ngl,
                spec_draft_device: preset.spec_draft_device.clone(),
                spec_draft_cpu_moe: preset.spec_draft_cpu_moe,
                spec_draft_n_cpu_moe: preset.spec_draft_n_cpu_moe,
                spec_draft_type_k: preset.spec_draft_type_k.clone(),
                spec_draft_type_v: preset.spec_draft_type_v.clone(),
                spec_ngram_mod_n_min: preset.spec_ngram_mod_n_min,
                spec_ngram_mod_n_max: preset.spec_ngram_mod_n_max,
                spec_ngram_mod_n_match: preset.spec_ngram_mod_n_match,
                spec_ngram_simple_size_n: preset.spec_ngram_simple_size_n,
                spec_ngram_simple_size_m: preset.spec_ngram_simple_size_m,
                spec_ngram_simple_min_hits: preset.spec_ngram_simple_min_hits,
                spec_ngram_map_k_size_n: preset.spec_ngram_map_k_size_n,
                spec_ngram_map_k_size_m: preset.spec_ngram_map_k_size_m,
                spec_ngram_map_k_min_hits: preset.spec_ngram_map_k_min_hits,
                spec_ngram_map_k4v_size_n: preset.spec_ngram_map_k4v_size_n,
                spec_ngram_map_k4v_size_m: preset.spec_ngram_map_k4v_size_m,
                spec_ngram_map_k4v_min_hits: preset.spec_ngram_map_k4v_min_hits,
            },
            kv_unified: preset.kv_unified,
            cache_idle_slots: preset.cache_idle_slots,
            cache_ram_mib: preset.cache_ram_mib,
            fit_enabled: preset.fit_enabled,
            fit_ctx: preset.fit_ctx,
            fit_target: preset.fit_target.clone(),
            fit_print: preset.fit_print,
            seed: preset.seed,
            system_prompt_file: preset.system_prompt_file.clone(),
            extra_args: preset.extra_args.clone(),
            bind_host: preset.bind_host.clone(),
            chat_template_file: preset.chat_template_file.clone(),
            mmproj: preset.mmproj.clone(),
            grammar: preset.grammar.clone(),
            json_schema: preset.json_schema.clone(),
            cache_type_k: preset.cache_type_k.clone(),
            cache_type_v: preset.cache_type_v.clone(),
            max_tokens: preset.max_tokens,
            api_key: preset.api_key.clone(),
            alias: preset.alias.clone(),
            benchmark_mode: preset.benchmark_mode,
            enable_thinking: preset.enable_thinking,
            preserve_thinking: preset.preserve_thinking,
            tool_call_format: preset.tool_call_format.clone(),
            reasoning: preset.reasoning.clone(),
            reasoning_budget: preset.reasoning_budget,
            reasoning_budget_message: preset.reasoning_budget_message.clone(),
            image_min_tokens: preset.image_min_tokens,
            image_max_tokens: preset.image_max_tokens,
            ..Default::default()
        }))),
    }
}

pub fn request_from_session(
    session: &Session,
    presets: &[ModelPreset],
    transient_api_key: Option<&str>,
) -> Result<LocalLaunchRequest> {
    let session_api_key = match &session.mode {
        SessionMode::Spawn { api_key, .. } => api_key.clone(),
        SessionMode::Attach { .. } => anyhow::bail!("Attach sessions cannot be locally launched"),
    };

    if let Some(mut request) = session.launch.clone() {
        if request.backend() != session.backend {
            anyhow::bail!("Persisted session backend does not match its launch envelope");
        }
        if let LocalLaunchRequest::LlamaCpp(config) = &mut request {
            config.api_key = transient_api_key
                .filter(|key| !key.is_empty())
                .map(str::to_string)
                .or(session_api_key);
            if session.launch_requires_api_key && config.api_key.is_none() {
                anyhow::bail!(
                    "This restored session requires its llama.cpp API key to be entered again"
                );
            }
        }
        return Ok(request);
    }

    let preset = presets
        .iter()
        .find(|preset| preset.id == session.preset_id)
        .ok_or_else(|| {
            anyhow::anyhow!("Restored session has no launch envelope or matching preset")
        })?;
    let port = match &session.mode {
        SessionMode::Spawn { port, .. } => Some(*port),
        SessionMode::Attach { .. } => None,
    };
    let mut request = request_from_preset(preset, port)?;
    if request.backend() != session.backend {
        anyhow::bail!("Restored session backend no longer matches its preset backend");
    }
    if let LocalLaunchRequest::LlamaCpp(config) = &mut request {
        if let Some(key) = transient_api_key.filter(|key| !key.is_empty()) {
            config.api_key = Some(key.to_string());
        }
        if session.launch_requires_api_key && config.api_key.is_none() {
            anyhow::bail!(
                "This restored session requires its llama.cpp API key to be entered again"
            );
        }
    }
    Ok(request)
}

/// The only production adapter construction point for local inference.
pub async fn construct_adapter(
    request: &LocalLaunchRequest,
    state: &AppState,
    app_config: &AppConfig,
) -> Result<BackendAdapter> {
    match request {
        LocalLaunchRequest::LlamaCpp(config) => {
            let gpu_env = state.gpu_env.lock().unwrap().clone();
            Ok(BackendAdapter::LlamaCpp(Arc::new(LlamaCppAdapter::new(
                app_config.clone(),
                config.as_ref().clone(),
                gpu_env,
            ))))
        }
        LocalLaunchRequest::RapidMlx(config) => {
            if config.model_path.trim().is_empty() {
                anyhow::bail!("Rapid-MLX requires a model_path");
            }

            let (executable_path, source) = Discovery::resolve_binary(
                config.executable_path.as_deref(),
                config.managed_runtime_path.as_deref(),
            )
            .await
            .with_context(|| {
                "Rapid-MLX runtime was not found. Install rapid-mlx or configure an executable path"
            })?;
            let version = Discovery::probe_version(&executable_path)
                .await
                .with_context(|| {
                    format!(
                        "Rapid-MLX runtime at {} is not executable",
                        executable_path.display()
                    )
                })?;
            let mut adapter = RapidMlxAdapter::new(
                RuntimeMetadata {
                    executable_path,
                    source,
                    version,
                },
                PathBuf::from(&config.model_path),
            );
            adapter.served_model_name = config.served_model_name.clone();
            adapter.host = config.host.clone();
            adapter.port = config.port;
            adapter.log_level = config.log_level.clone();
            Ok(BackendAdapter::RapidMlx(Arc::new(adapter)))
        }
    }
}

pub async fn launch_local(
    state: Arc<AppState>,
    request: LocalLaunchRequest,
    app_config: &AppConfig,
) -> Result<()> {
    let adapter = construct_adapter(&request, &state, app_config).await?;
    let legacy_llama_config = match &request {
        LocalLaunchRequest::LlamaCpp(config) => Some(config.as_ref().clone()),
        LocalLaunchRequest::RapidMlx(_) => None,
    };
    let port = request.port();
    let model_identity = request.model_identity();
    crate::llama::server::start_backend(
        state,
        adapter,
        request,
        port,
        model_identity,
        legacy_llama_config,
    )
    .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    fn test_app_config(config_dir: &std::path::Path) -> AppConfig {
        AppConfig::from_args(crate::cli::AppArgs::parse_from([
            "llama-monitor",
            "--config-dir",
            config_dir.to_str().unwrap(),
            "--llama-server-path",
            "llama-server",
            "--gpu-backend",
            "none",
        ]))
    }

    #[test]
    fn direct_payload_without_backend_routes_to_llama_cpp() {
        let mut value = serde_json::to_value(ServerConfig::default()).unwrap();
        value["model_path"] = serde_json::json!("/models/legacy.gguf");
        let request = request_from_api_payload(&value).unwrap();
        assert!(matches!(request, LocalLaunchRequest::LlamaCpp(_)));
    }

    #[test]
    fn rapid_payload_requires_backend_owned_config() {
        let error = request_from_api_payload(&serde_json::json!({
            "backend": "rapid_mlx",
            "model_path": "/models/wrong-level"
        }))
        .unwrap_err();
        assert!(error.to_string().contains("rapid_mlx configuration object"));
    }

    #[test]
    fn direct_llama_payload_rejects_rapid_mlx_config() {
        for payload in [
            serde_json::json!({
                "model_path": "/models/legacy.gguf",
                "rapid_mlx": { "model_path": "/models/rapid" }
            }),
            serde_json::json!({
                "backend": "llama_cpp",
                "model_path": "/models/legacy.gguf",
                "rapid_mlx": { "model_path": "/models/rapid" }
            }),
        ] {
            let error = request_from_api_payload(&payload).unwrap_err();
            assert!(
                error
                    .to_string()
                    .contains("llama_cpp launch must not include")
            );
        }
    }

    #[tokio::test]
    async fn factory_selects_llama_cpp_adapter() {
        let dir = tempfile::tempdir().unwrap();
        let config = test_app_config(dir.path());
        let state = AppState::default();
        let request = LocalLaunchRequest::LlamaCpp(Box::new(ServerConfig::default()));

        let adapter = construct_adapter(&request, &state, &config).await.unwrap();
        assert!(matches!(adapter, BackendAdapter::LlamaCpp(_)));
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn factory_selects_rapid_mlx_adapter() {
        use std::os::unix::fs::PermissionsExt;

        let dir = tempfile::tempdir().unwrap();
        let runtime = dir.path().join("rapid-mlx");
        std::fs::write(&runtime, "#!/bin/sh\necho rapid-mlx-test\n").unwrap();
        std::fs::set_permissions(&runtime, std::fs::Permissions::from_mode(0o755)).unwrap();
        let config = test_app_config(dir.path());
        let state = AppState::default();
        let request = LocalLaunchRequest::RapidMlx(RapidMlxConfig {
            model_path: "/models/rapid".into(),
            executable_path: Some(runtime),
            ..Default::default()
        });

        let adapter = construct_adapter(&request, &state, &config).await.unwrap();
        assert!(matches!(adapter, BackendAdapter::RapidMlx(_)));
    }

    #[test]
    fn rapid_preset_port_uses_nested_value_unless_explicitly_overridden() {
        let preset = ModelPreset {
            name: "Rapid".into(),
            backend: InferenceBackend::RapidMlx,
            port: Some(8001),
            rapid_mlx: Some(RapidMlxConfig {
                model_path: "/models/rapid".into(),
                port: 9123,
                ..Default::default()
            }),
            ..Default::default()
        };
        assert_eq!(request_from_preset(&preset, None).unwrap().port(), 9123);
        assert_eq!(
            request_from_preset(&preset, Some(9222)).unwrap().port(),
            9222
        );
        assert!(request_from_preset(&preset, Some(0)).is_err());
    }

    #[test]
    fn direct_rapid_payload_rejects_zero_port() {
        let error = request_from_api_payload(&serde_json::json!({
            "backend": "rapid_mlx",
            "rapid_mlx": {
                "model_path": "/models/rapid",
                "port": 0
            }
        }))
        .unwrap_err();
        assert!(error.to_string().contains("non-zero port"));
    }

    #[test]
    fn preset_backend_config_mismatches_are_rejected() {
        let llama_with_rapid = ModelPreset {
            name: "Mismatch".into(),
            rapid_mlx: Some(RapidMlxConfig {
                model_path: "/models/rapid".into(),
                ..Default::default()
            }),
            ..Default::default()
        };
        assert!(validate_preset_backend_config(&llama_with_rapid).is_err());

        let rapid_without_config = ModelPreset {
            name: "Missing".into(),
            backend: InferenceBackend::RapidMlx,
            ..Default::default()
        };
        assert!(validate_preset_backend_config(&rapid_without_config).is_err());
    }

    #[test]
    fn persisted_launch_envelope_scrubs_api_key_and_restores_backend() {
        let request = LocalLaunchRequest::LlamaCpp(Box::new(ServerConfig {
            model_path: "/models/private.gguf".into(),
            port: 8001,
            api_key: Some("do-not-persist".into()),
            ..Default::default()
        }));
        let persisted = request.for_persistence();
        let json = serde_json::to_string(&persisted).unwrap();
        assert!(!json.contains("do-not-persist"));
        assert!(matches!(persisted, LocalLaunchRequest::LlamaCpp(_)));
    }

    #[test]
    fn restored_protected_session_uses_only_transient_api_key() {
        let request = LocalLaunchRequest::LlamaCpp(Box::new(ServerConfig {
            model_path: "/models/private.gguf".into(),
            port: 8001,
            api_key: Some("original-secret".into()),
            ..Default::default()
        }));
        let mut session = Session::new_spawn_with_backend(
            "private".into(),
            "Private".into(),
            8001,
            String::new(),
            None,
            Some("original-secret".into()),
            InferenceBackend::LlamaCpp,
            Some("/models/private.gguf".into()),
        );
        session.launch = Some(request.for_persistence());

        let serialized = serde_json::to_string(&session).unwrap();
        assert!(!serialized.contains("original-secret"));
        let restored_session: Session = serde_json::from_str(&serialized).unwrap();
        assert!(request_from_session(&restored_session, &[], None).is_err());

        let restored =
            request_from_session(&restored_session, &[], Some("entered-on-restore")).unwrap();
        assert_eq!(restored.api_key().as_deref(), Some("entered-on-restore"));
        assert!(
            !serde_json::to_string(&session)
                .unwrap()
                .contains("entered-on-restore")
        );
    }

    #[test]
    fn restored_direct_rapid_session_routes_from_launch_envelope() {
        let request = LocalLaunchRequest::RapidMlx(RapidMlxConfig {
            model_path: "/models/rapid".into(),
            port: 8123,
            ..Default::default()
        });
        let mut session = Session::new_spawn_with_backend(
            "s1".into(),
            "Rapid".into(),
            8123,
            String::new(),
            Some("0.0.0.0".into()),
            None,
            InferenceBackend::RapidMlx,
            Some("/models/rapid".into()),
        );
        session.launch = Some(request.for_persistence());

        let restored = request_from_session(&session, &[], None).unwrap();
        assert!(matches!(restored, LocalLaunchRequest::RapidMlx(_)));
        assert_eq!(restored.port(), 8123);
    }
}
