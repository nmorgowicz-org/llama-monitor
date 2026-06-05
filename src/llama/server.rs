use anyhow::Result;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command as TokioCommand;

use crate::config::AppConfig;
use crate::gpu::env::{build_nvidia_env, build_rocm_env};
use crate::state::AppState;

/// Speculative-decoding knobs, kept in a separate struct so ServerConfig
/// stays readable. Flattened into the parent so JSON serialization is
/// identical to before — saved presets and API payloads are unaffected.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct SpecDecodeConfig {
    // Legacy / shared
    #[serde(default)]
    pub draft_model: String,
    #[serde(default)]
    pub draft_min: Option<u32>,
    #[serde(default)]
    pub draft_max: Option<u32>,
    #[serde(default)]
    pub spec_ngram_size: Option<u32>,
    // Spec type selector
    #[serde(default)]
    pub spec_type: Option<String>,
    #[serde(default)]
    pub spec_default: bool,
    // Draft model knobs
    #[serde(default)]
    pub spec_draft_n_max: Option<u32>,
    #[serde(default)]
    pub spec_draft_n_min: Option<u32>,
    #[serde(default)]
    pub spec_draft_p_split: Option<f32>,
    #[serde(default)]
    pub spec_draft_p_min: Option<f32>,
    #[serde(default)]
    pub spec_draft_ngl: Option<i32>,
    #[serde(default)]
    pub spec_draft_device: Option<String>,
    #[serde(default)]
    pub spec_draft_cpu_moe: bool,
    #[serde(default)]
    pub spec_draft_n_cpu_moe: Option<i32>,
    #[serde(default)]
    pub spec_draft_type_k: Option<String>,
    #[serde(default)]
    pub spec_draft_type_v: Option<String>,
    // ngram-mod
    #[serde(default)]
    pub spec_ngram_mod_n_min: Option<u32>,
    #[serde(default)]
    pub spec_ngram_mod_n_max: Option<u32>,
    #[serde(default)]
    pub spec_ngram_mod_n_match: Option<u32>,
    // ngram-simple
    #[serde(default)]
    pub spec_ngram_simple_size_n: Option<u32>,
    #[serde(default)]
    pub spec_ngram_simple_size_m: Option<u32>,
    #[serde(default)]
    pub spec_ngram_simple_min_hits: Option<u32>,
    // ngram-map-k
    #[serde(default)]
    pub spec_ngram_map_k_size_n: Option<u32>,
    #[serde(default)]
    pub spec_ngram_map_k_size_m: Option<u32>,
    #[serde(default)]
    pub spec_ngram_map_k_min_hits: Option<u32>,
    // ngram-map-k4v
    #[serde(default)]
    pub spec_ngram_map_k4v_size_n: Option<u32>,
    #[serde(default)]
    pub spec_ngram_map_k4v_size_m: Option<u32>,
    #[serde(default)]
    pub spec_ngram_map_k4v_min_hits: Option<u32>,
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct ServerConfig {
    pub model_path: String,
    pub context_size: u64,
    pub ctk: String,
    pub ctv: String,
    pub tensor_split: String,
    pub batch_size: u32,
    pub ubatch_size: u32,
    pub no_mmap: bool,
    pub port: u16,
    pub ngram_spec: bool,
    pub parallel_slots: u32,
    // Generation
    #[serde(default)]
    pub temperature: Option<f64>,
    #[serde(default)]
    pub top_p: Option<f64>,
    #[serde(default)]
    pub top_k: Option<i32>,
    #[serde(default)]
    pub min_p: Option<f64>,
    #[serde(default)]
    pub repeat_penalty: Option<f64>,
    #[serde(default)]
    pub presence_penalty: Option<f64>,
    // CPU MOE
    #[serde(default)]
    pub n_cpu_moe: Option<i32>,
    #[serde(default)]
    pub gpu_layers: Option<i32>,
    #[serde(default)]
    pub mlock: bool,
    // Attention
    #[serde(default)]
    pub flash_attn: String,
    // GPU distribution
    #[serde(default)]
    pub split_mode: String,
    #[serde(default)]
    pub main_gpu: Option<u32>,
    // Threading
    #[serde(default)]
    pub threads: Option<u32>,
    #[serde(default)]
    pub threads_batch: Option<u32>,
    // Priority
    #[serde(default)]
    pub prio: Option<i32>,
    #[serde(default)]
    pub prio_batch: Option<i32>,
    // Rope scaling
    #[serde(default)]
    pub rope_scaling: String,
    #[serde(default)]
    pub rope_freq_base: Option<f64>,
    #[serde(default)]
    pub rope_freq_scale: Option<f64>,
    // Speculative decoding — all fields flattened so JSON serialization stays flat
    // (saved presets and API payloads use the same top-level keys).
    #[serde(flatten, default)]
    pub spec: SpecDecodeConfig,
    // KV cache
    #[serde(default)]
    pub kv_unified: Option<bool>,
    #[serde(default)]
    pub cache_idle_slots: Option<bool>,
    /// -cram / --cache-ram N: max KV prefix-cache in MiB. Default: 8192. -1 = no limit, 0 = disable.
    #[serde(default)]
    pub cache_ram_mib: Option<i32>,
    // Fit
    #[serde(default)]
    pub fit_enabled: Option<bool>,
    #[serde(default)]
    pub fit_ctx: Option<u32>,
    #[serde(default)]
    pub fit_target: Option<String>,
    #[serde(default)]
    pub fit_print: Option<bool>,
    // Misc
    #[serde(default)]
    pub ignore_eos: bool,
    // Advanced
    #[serde(default)]
    pub seed: Option<i64>,
    #[serde(default)]
    pub system_prompt_file: String,
    #[serde(default)]
    pub extra_args: String,
    #[serde(default)]
    pub bind_host: Option<String>,

    // Spawn V2: extended fields
    #[serde(default)]
    pub hf_repo: Option<String>,
    /// --alias STRING: model name reported in /v1/models and shown in API clients.
    #[serde(default)]
    pub alias: Option<String>,
    #[serde(default)]
    pub chat_template_file: Option<String>,
    #[serde(default)]
    pub mmproj: Option<String>,
    #[serde(default)]
    pub grammar: Option<String>,
    #[serde(default)]
    pub json_schema: Option<String>,
    #[serde(default)]
    pub cache_type_k: Option<String>,
    #[serde(default)]
    pub cache_type_v: Option<String>,
    #[serde(default)]
    pub max_tokens: Option<u64>,
    #[serde(default)]
    pub api_key: Option<String>,
    #[serde(default)]
    pub benchmark_mode: bool,
    // Thinking / chain-of-thought control
    // enable_thinking and preserve_thinking are passed as --chat-template-kwargs JSON.
    // Command::arg() bypasses the shell on all platforms — no quoting needed.
    #[serde(default)]
    pub enable_thinking: Option<bool>,
    #[serde(default)]
    pub preserve_thinking: Option<bool>,
    /// --reasoning on|off|auto
    #[serde(default)]
    pub reasoning: Option<String>,
    /// --reasoning-budget N: -1 = unlimited, 0 = disabled, N = token cap
    #[serde(default)]
    pub reasoning_budget: Option<i32>,
    /// --reasoning-budget-message STRING: text appended after thinking block
    #[serde(default)]
    pub reasoning_budget_message: Option<String>,
    /// --image-min-tokens / --image-max-tokens for multimodal (mmproj) models.
    /// Qwen3.6 vision: 1024 / 4096. Gemma4 vision: 280 / 560.
    #[serde(default)]
    pub image_min_tokens: Option<u32>,
    #[serde(default)]
    pub image_max_tokens: Option<u32>,
}

pub async fn start_server(
    state: &AppState,
    config: ServerConfig,
    app_config: &AppConfig,
) -> Result<()> {
    // Spawn V2: model source selection: either -m (local path) or -hf (HF repo), but not both.
    let use_hf = config.hf_repo.as_ref().is_some_and(|r| !r.is_empty());
    let has_model_path = !config.model_path.is_empty();

    if use_hf && has_model_path {
        anyhow::bail!("Cannot use both model_path and hf_repo. Choose one.");
    }

    if use_hf {
        // Validate server binary (skip PATH lookup for bare names like "llama-server")
        let server_path = &app_config.llama_server_path;
        if server_path.components().count() > 1 && !server_path.exists() {
            anyhow::bail!(
                "llama-server binary not found: {}. Set it in Configuration.",
                server_path.display()
            );
        }
        // HF repo is used; no local file check.
    } else if has_model_path {
        // Validate model path before starting
        if !std::path::Path::new(&config.model_path).exists() {
            anyhow::bail!("Model file not found: {}", config.model_path);
        }

        // Validate server binary (skip PATH lookup for bare names like "llama-server")
        let server_path = &app_config.llama_server_path;
        if server_path.components().count() > 1 && !server_path.exists() {
            anyhow::bail!(
                "llama-server binary not found: {}. Set it in Configuration.",
                server_path.display()
            );
        }
    } else {
        anyhow::bail!("No model source specified. Provide model_path or hf_repo.");
    }

    // Clear old logs
    {
        let mut logs = state.server_logs.lock().unwrap();
        logs.clear();
    }

    let mut cmd = TokioCommand::new(&app_config.llama_server_path);
    cmd.current_dir(&app_config.llama_server_cwd);

    // Set GPU-specific environment variables
    let gpu_env = state.gpu_env.lock().unwrap().clone();
    let cwd = app_config.llama_server_cwd.display().to_string();
    match app_config.gpu_backend.as_str() {
        "nvidia" => {
            for (key, val) in build_nvidia_env(&gpu_env) {
                cmd.env(key, val);
            }
        }
        "none" => {}
        _ => {
            for (key, val) in build_rocm_env(&gpu_env, &cwd) {
                cmd.env(key, val);
            }
        }
    }

    // Build args — model source
    if use_hf {
        if let Some(ref repo) = config.hf_repo {
            cmd.arg("-hf").arg(repo);
        }
    } else {
        cmd.arg("-m").arg(&config.model_path);
    }

    cmd.arg("-ngl")
        .arg(config.gpu_layers.unwrap_or(99).to_string());
    cmd.arg("-ctk").arg(&config.ctk);
    cmd.arg("-ctv").arg(&config.ctv);
    cmd.arg("--host")
        .arg(config.bind_host.as_deref().unwrap_or("127.0.0.1"));
    cmd.arg("--port").arg(config.port.to_string());
    cmd.arg("-c").arg(config.context_size.to_string());
    cmd.arg("-b").arg(config.batch_size.to_string());
    cmd.arg("-ub").arg(config.ubatch_size.to_string());
    cmd.arg("--no-warmup");
    cmd.arg("--jinja");
    cmd.arg("--metrics");
    cmd.arg("--webui-mcp-proxy");
    // Disable context-shift so long conversations error clearly rather than silently
    // dropping prompt tokens. Users can extend context or use KV offload instead.
    cmd.arg("--no-context-shift");
    // Context checkpointing — saves KV cache state every N tokens for faster
    // context restore after eviction. 32 is the llama.cpp recommended default.
    cmd.arg("--ctx-checkpoints").arg("32");
    // Keep all system/prompt tokens when context overflows (-1 = keep all).
    cmd.arg("--keep").arg("-1");

    // Memory
    if config.no_mmap {
        cmd.arg("--no-mmap");
    }
    if config.mlock {
        cmd.arg("--mlock");
    }

    // Flash attention
    if !config.flash_attn.is_empty() {
        cmd.arg("-fa").arg(&config.flash_attn);
    }

    // GPU distribution
    if !config.tensor_split.is_empty() {
        cmd.arg("-ts").arg(&config.tensor_split);
    }
    if !config.split_mode.is_empty() {
        cmd.arg("--split-mode").arg(&config.split_mode);
    }
    if let Some(mg) = config.main_gpu {
        cmd.arg("-mg").arg(mg.to_string());
    }

    // Threading
    if let Some(t) = config.threads {
        cmd.arg("-t").arg(t.to_string());
    }
    if let Some(tb) = config.threads_batch {
        cmd.arg("-tb").arg(tb.to_string());
    }

    // Priority
    if let Some(p) = config.prio {
        cmd.arg("--prio").arg(p.to_string());
    }
    if let Some(pb) = config.prio_batch {
        cmd.arg("--prio-batch").arg(pb.to_string());
    }

    // Rope scaling: explicit config takes priority, else auto-YaRN for large contexts
    if !config.rope_scaling.is_empty() {
        cmd.arg("--rope-scaling").arg(&config.rope_scaling);
    } else if config.context_size > 262144 {
        cmd.arg("--rope-scaling").arg("yarn");
    }
    if let Some(base) = config.rope_freq_base {
        cmd.arg("--rope-freq-base").arg(format!("{:.6}", base));
    }
    if let Some(scale) = config.rope_freq_scale {
        cmd.arg("--rope-freq-scale").arg(format!("{:.6}", scale));
    } else if config.rope_scaling.is_empty() && config.context_size > 262144 {
        // Auto-calculate YaRN scale when no explicit rope config
        let scale = 262144.0 / config.context_size as f64;
        cmd.arg("--rope-freq-scale").arg(format!("{:.6}", scale));
        cmd.arg("--yarn-ext-factor").arg("1.0");
        cmd.arg("--yarn-attn-factor").arg("1.0");
        cmd.arg("--yarn-beta-fast").arg("32");
        cmd.arg("--yarn-beta-slow").arg("1");
    }

    // Speculative decoding (all config in config.spec.*)
    let s = &config.spec;
    // Backward compat: if old ngram_spec is true and no spec_type set, default to ngram-mod
    let spec_type_effective = if s.spec_type.is_some() {
        s.spec_type.clone()
    } else if config.ngram_spec {
        Some("ngram-mod".to_string())
    } else {
        None
    };

    if let Some(ref st) = spec_type_effective {
        cmd.arg("--spec-type").arg(st);
    }
    if s.spec_default {
        cmd.arg("--spec-default");
    }
    if !s.draft_model.is_empty() {
        cmd.arg("-md").arg(&s.draft_model);
    }
    if let Some(v) = s.spec_draft_n_max {
        cmd.arg("--spec-draft-n-max").arg(v.to_string());
    }
    if let Some(v) = s.spec_draft_n_min {
        cmd.arg("--spec-draft-n-min").arg(v.to_string());
    }
    if let Some(v) = s.spec_draft_p_split {
        cmd.arg("--spec-draft-p-split").arg(format!("{:.4}", v));
    }
    if let Some(v) = s.spec_draft_p_min {
        cmd.arg("--spec-draft-p-min").arg(format!("{:.4}", v));
    }
    if let Some(v) = s.spec_draft_ngl {
        cmd.arg("--spec-draft-ngl").arg(v.to_string());
    }
    if let Some(ref v) = s.spec_draft_device {
        cmd.arg("--spec-draft-device").arg(v);
    }
    if s.spec_draft_cpu_moe {
        cmd.arg("--spec-draft-cpu-moe");
    }
    if let Some(v) = s.spec_draft_n_cpu_moe {
        cmd.arg("--spec-draft-n-cpu-moe").arg(v.to_string());
    }
    if let Some(ref v) = s.spec_draft_type_k {
        cmd.arg("--spec-draft-type-k").arg(v);
    }
    if let Some(ref v) = s.spec_draft_type_v {
        cmd.arg("--spec-draft-type-v").arg(v);
    }
    if let Some(v) = s.spec_ngram_mod_n_min {
        cmd.arg("--spec-ngram-mod-n-min").arg(v.to_string());
    }
    if let Some(v) = s.spec_ngram_mod_n_max {
        cmd.arg("--spec-ngram-mod-n-max").arg(v.to_string());
    }
    if let Some(v) = s.spec_ngram_mod_n_match {
        cmd.arg("--spec-ngram-mod-n-match").arg(v.to_string());
    }
    if let Some(v) = s.spec_ngram_simple_size_n {
        cmd.arg("--spec-ngram-simple-size-n").arg(v.to_string());
    }
    if let Some(v) = s.spec_ngram_simple_size_m {
        cmd.arg("--spec-ngram-simple-size-m").arg(v.to_string());
    }
    if let Some(v) = s.spec_ngram_simple_min_hits {
        cmd.arg("--spec-ngram-simple-min-hits").arg(v.to_string());
    }
    if let Some(v) = s.spec_ngram_map_k_size_n {
        cmd.arg("--spec-ngram-map-k-size-n").arg(v.to_string());
    }
    if let Some(v) = s.spec_ngram_map_k_size_m {
        cmd.arg("--spec-ngram-map-k-size-m").arg(v.to_string());
    }
    if let Some(v) = s.spec_ngram_map_k_min_hits {
        cmd.arg("--spec-ngram-map-k-min-hits").arg(v.to_string());
    }
    if let Some(v) = s.spec_ngram_map_k4v_size_n {
        cmd.arg("--spec-ngram-map-k4v-size-n").arg(v.to_string());
    }
    if let Some(v) = s.spec_ngram_map_k4v_size_m {
        cmd.arg("--spec-ngram-map-k4v-size-m").arg(v.to_string());
    }
    if let Some(v) = s.spec_ngram_map_k4v_min_hits {
        cmd.arg("--spec-ngram-map-k4v-min-hits").arg(v.to_string());
    }

    // Legacy ngram_spec backward compat
    if config.ngram_spec {
        if let Some(v) = s.spec_ngram_size {
            cmd.arg("--spec-ngram-size-n").arg(v.to_string());
        }
        if let Some(v) = s.draft_min {
            cmd.arg("--draft-min").arg(v.to_string());
        }
        if let Some(v) = s.draft_max {
            cmd.arg("--draft-max").arg(v.to_string());
        }
    }

    // Slots
    if config.parallel_slots > 0 {
        cmd.arg("--parallel").arg(config.parallel_slots.to_string());
    }

    // Generation
    if let Some(t) = config.temperature {
        cmd.arg("--temp").arg(format!("{:.2}", t));
    }
    if let Some(tp) = config.top_p {
        cmd.arg("--top-p").arg(format!("{:.4}", tp));
    }
    if let Some(tk) = config.top_k {
        cmd.arg("--top-k").arg(tk.to_string());
    }
    if let Some(mp) = config.min_p {
        cmd.arg("--min-p").arg(format!("{:.4}", mp));
    }
    if let Some(rp) = config.repeat_penalty {
        cmd.arg("--repeat-penalty").arg(format!("{:.2}", rp));
    }
    if let Some(pp) = config.presence_penalty {
        cmd.arg("--presence-penalty").arg(format!("{:.4}", pp));
    }
    if let Some(n) = config.n_cpu_moe {
        cmd.arg("--n-cpu-moe").arg(n.to_string());
    }

    // Advanced
    if let Some(seed) = config.seed {
        cmd.arg("--seed").arg(seed.to_string());
    }
    if !config.system_prompt_file.is_empty() {
        cmd.arg("--system-prompt-file")
            .arg(&config.system_prompt_file);
    }

    // Spawn V2: chat template file
    if let Some(ref ct) = config.chat_template_file
        && !ct.is_empty()
    {
        cmd.arg("--chat-template-file").arg(ct);
    }

    // Thinking / chain-of-thought control.
    // --chat-template-kwargs takes a raw JSON string. Command::arg() passes it
    // directly to the process without shell interpretation on all platforms,
    // so no quoting or escaping is needed regardless of OS.
    {
        let mut kwargs = serde_json::Map::new();
        if let Some(et) = config.enable_thinking {
            kwargs.insert("enable_thinking".into(), serde_json::json!(et));
        }
        if let Some(pt) = config.preserve_thinking {
            kwargs.insert("preserve_thinking".into(), serde_json::json!(pt));
        }
        if !kwargs.is_empty() {
            let json = serde_json::to_string(&kwargs).unwrap_or_default();
            cmd.arg("--chat-template-kwargs").arg(json);
        }
    }
    if let Some(ref mode) = config.reasoning
        && !mode.is_empty()
    {
        cmd.arg("--reasoning").arg(mode);
    }
    if let Some(budget) = config.reasoning_budget {
        cmd.arg("--reasoning-budget").arg(budget.to_string());
    }
    if let Some(ref msg) = config.reasoning_budget_message
        && !msg.is_empty()
    {
        cmd.arg("--reasoning-budget-message").arg(msg);
    }

    // Always enable metrics endpoint (/metrics) — used by the dashboard
    cmd.arg("--metrics");

    // Spawn V2: multimodal projector
    if let Some(ref mp) = config.mmproj
        && !mp.is_empty()
    {
        cmd.arg("--mmproj").arg(mp);
        // Image token budget — only meaningful when a vision projector is loaded.
        // Qwen3.6 vision: 1024/4096. Gemma4 vision: 280/560.
        if let Some(min) = config.image_min_tokens {
            cmd.arg("--image-min-tokens").arg(min.to_string());
        }
        if let Some(max) = config.image_max_tokens {
            cmd.arg("--image-max-tokens").arg(max.to_string());
        }
    }

    // Spawn V2: grammar
    if let Some(ref g) = config.grammar
        && !g.is_empty()
    {
        cmd.arg("--grammar").arg(g);
    }

    // Spawn V2: JSON schema
    if let Some(ref js) = config.json_schema
        && !js.is_empty()
    {
        cmd.arg("--json-schema").arg(js);
    }

    // Spawn V2: max_tokens / n-predict
    if let Some(mt) = config.max_tokens {
        cmd.arg("-n").arg(mt.to_string());
    }

    // Spawn V2: API key
    if let Some(ref ak) = config.api_key
        && !ak.is_empty()
    {
        cmd.arg("--api-key").arg(ak);
    }

    // Spawn V2: alias (model name exposed in /v1/models)
    if let Some(ref al) = config.alias
        && !al.is_empty()
    {
        cmd.arg("--alias").arg(al);
    }

    // KV cache
    if let Some(v) = config.kv_unified {
        if v {
            cmd.arg("--kv-unified");
        } else {
            cmd.arg("--no-kv-unified");
        }
    }
    if let Some(v) = config.cache_idle_slots {
        if v {
            cmd.arg("--cache-idle-slots");
        } else {
            cmd.arg("--no-cache-idle-slots");
        }
    }
    if let Some(v) = config.cache_ram_mib {
        cmd.arg("--cache-ram").arg(v.to_string());
    }

    // Fit
    if let Some(v) = config.fit_enabled {
        if v {
            cmd.arg("--fit").arg("on");
        } else {
            cmd.arg("--fit").arg("off");
        }
    }
    if let Some(v) = config.fit_ctx {
        cmd.arg("--fit-ctx").arg(v.to_string());
    }
    if let Some(ref v) = config.fit_target {
        cmd.arg("--fit-target").arg(v);
    }
    if let Some(v) = config.fit_print {
        if v {
            cmd.arg("--fit-print").arg("on");
        } else {
            cmd.arg("--fit-print").arg("off");
        }
    }

    // Misc
    if config.ignore_eos {
        cmd.arg("--ignore-eos");
    }

    // Extra args (arbitrary flags)
    for arg in config.extra_args.split_whitespace() {
        cmd.arg(arg);
    }

    // Record the full spawn command for debugging (available at /api/debug/spawn-cmd).
    {
        fn shell_quote(s: &std::ffi::OsStr) -> String {
            let s = s.to_string_lossy();
            if s.contains(' ') || s.contains('"') || s.contains('\'') || s.is_empty() {
                format!("\"{}\"", s.replace('"', "\\\""))
            } else {
                s.into_owned()
            }
        }
        let mut parts = vec![shell_quote(cmd.as_std().get_program())];
        for arg in cmd.as_std().get_args() {
            parts.push(shell_quote(arg));
        }
        let cmd_str = parts.join(" \\\n  ");
        if let Ok(mut lock) = state.last_spawn_cmd.lock() {
            *lock = cmd_str;
        }
    }

    cmd.stdout(std::process::Stdio::piped());
    cmd.stderr(std::process::Stdio::piped());

    let mut child = cmd.spawn()?;

    // Capture stdout
    if let Some(stdout) = child.stdout.take() {
        let state_clone = state.clone();
        tokio::spawn(async move {
            let reader = BufReader::new(stdout);
            let mut lines = reader.lines();
            while let Ok(Some(line)) = lines.next_line().await {
                state_clone.push_log(line);
            }
        });
    }

    // Capture stderr
    if let Some(stderr) = child.stderr.take() {
        let state_clone = state.clone();
        tokio::spawn(async move {
            let reader = BufReader::new(stderr);
            let mut lines = reader.lines();
            while let Ok(Some(line)) = lines.next_line().await {
                state_clone.push_log(line);
            }
        });
    }

    {
        let mut guard = state.server_child.lock().await;
        *guard = Some(child);
    }
    {
        let mut running = state.server_running.lock().unwrap();
        *running = true;
    }
    {
        let mut local_running = state.local_server_running.lock().unwrap();
        *local_running = true;
    }
    {
        let mut cfg = state.server_config.lock().unwrap();
        *cfg = Some(config);
    }

    // Notify user-gated pollers to start after an explicit UI start action.
    state.llama_poll_notify.notify_waiters();

    Ok(())
}

pub async fn stop_server(state: &AppState) -> Result<()> {
    let mut guard = state.server_child.lock().await;
    if let Some(ref mut child) = *guard {
        child.kill().await.ok();
        child.wait().await.ok();
    }
    *guard = None;
    {
        let mut local_running = state.local_server_running.lock().unwrap();
        *local_running = false;
    }
    {
        let mut cfg = state.server_config.lock().unwrap();
        *cfg = None;
    }
    {
        let mut m = state.llama_metrics.lock().unwrap();
        *m = crate::llama::metrics::LlamaMetrics::default();
    }
    state.push_log("[monitor] Server stopped.".into());
    Ok(())
}
