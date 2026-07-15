use anyhow::{Result, anyhow};
use reqwest::Client;
use std::ffi::OsString;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{Duration, Instant};
use tokio::process::Command as TokioCommand;

use crate::config::AppConfig;
use crate::gpu::env::{GpuEnv, build_nvidia_env, build_rocm_env};
use crate::inference::InferenceBackend;
use crate::inference::capabilities::CapabilitySet;
use crate::inference::metrics::{HealthState, InferenceMetricsSnapshot};
use crate::inference::supervisor::SupervisedLaunch;
use crate::llama::metrics::{parse_prometheus_metrics, parse_slot_metrics};

fn describe_process_status(status: std::process::ExitStatus) -> String {
    if let Some(code) = status.code() {
        return format!("exit code {code}");
    }

    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        if let Some(signal) = status.signal() {
            return format!("signal {signal}");
        }
    }

    "exit status unknown".to_string()
}

fn readiness_host(bind_host: Option<&str>) -> &str {
    match bind_host.unwrap_or("127.0.0.1") {
        "0.0.0.0" | "::" | "[::]" => "127.0.0.1",
        host => host,
    }
}

fn launch_environment(gpu_backend: &str, gpu_env: &GpuEnv, cwd: &str) -> Vec<(OsString, OsString)> {
    match gpu_backend {
        "nvidia" => build_nvidia_env(gpu_env),
        "none" => Vec::new(),
        _ => build_rocm_env(gpu_env, cwd),
    }
    .into_iter()
    .map(|(key, value)| (key.into(), value.into()))
    .collect()
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct SpecDecodeConfig {
    #[serde(default)]
    pub draft_model: String,
    #[serde(default)]
    pub draft_min: Option<u32>,
    #[serde(default)]
    pub draft_max: Option<u32>,
    #[serde(default)]
    pub spec_ngram_size: Option<u32>,
    #[serde(default)]
    pub spec_type: Option<String>,
    #[serde(default)]
    pub spec_default: bool,
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
    #[serde(default)]
    pub spec_ngram_mod_n_min: Option<u32>,
    #[serde(default)]
    pub spec_ngram_mod_n_max: Option<u32>,
    #[serde(default)]
    pub spec_ngram_mod_n_match: Option<u32>,
    #[serde(default)]
    pub spec_ngram_simple_size_n: Option<u32>,
    #[serde(default)]
    pub spec_ngram_simple_size_m: Option<u32>,
    #[serde(default)]
    pub spec_ngram_simple_min_hits: Option<u32>,
    #[serde(default)]
    pub spec_ngram_map_k_size_n: Option<u32>,
    #[serde(default)]
    pub spec_ngram_map_k_size_m: Option<u32>,
    #[serde(default)]
    pub spec_ngram_map_k_min_hits: Option<u32>,
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
    #[serde(default)]
    pub n_cpu_moe: Option<i32>,
    #[serde(default)]
    pub gpu_layers: Option<i32>,
    #[serde(default)]
    pub mlock: bool,
    #[serde(default)]
    pub flash_attn: String,
    #[serde(default)]
    pub split_mode: String,
    #[serde(default)]
    pub main_gpu: Option<u32>,
    #[serde(default)]
    pub threads: Option<i32>,
    #[serde(default)]
    pub threads_batch: Option<i32>,
    #[serde(default)]
    pub prio: Option<i32>,
    #[serde(default)]
    pub prio_batch: Option<i32>,
    #[serde(default)]
    pub rope_scaling: String,
    #[serde(default)]
    pub rope_freq_base: Option<f64>,
    #[serde(default)]
    pub rope_freq_scale: Option<f64>,
    #[serde(flatten, default)]
    pub spec: SpecDecodeConfig,
    #[serde(default)]
    pub kv_unified: Option<bool>,
    #[serde(default)]
    pub cache_idle_slots: Option<bool>,
    #[serde(default)]
    pub cache_ram_mib: Option<i32>,
    #[serde(default)]
    pub fit_enabled: Option<bool>,
    #[serde(default)]
    pub fit_ctx: Option<u32>,
    #[serde(default)]
    pub fit_target: Option<String>,
    #[serde(default)]
    pub fit_print: Option<bool>,
    #[serde(default)]
    pub seed: Option<i64>,
    pub system_prompt_file: String,
    pub extra_args: String,
    #[serde(default)]
    pub bind_host: Option<String>,
    #[serde(default)]
    pub hf_repo: Option<String>,
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
    #[serde(default)]
    pub enable_thinking: Option<bool>,
    #[serde(default)]
    pub preserve_thinking: Option<bool>,
    #[serde(default)]
    pub tool_call_format: Option<String>,
    #[serde(default)]
    pub reasoning: Option<String>,
    #[serde(default)]
    pub reasoning_budget: Option<i32>,
    #[serde(default)]
    pub reasoning_budget_message: Option<String>,
    #[serde(default)]
    pub image_min_tokens: Option<u32>,
    #[serde(default)]
    pub image_max_tokens: Option<u32>,
}

#[derive(Debug, Clone, Default)]
struct CounterSnapshot {
    prompt_tokens_total: f64,
    prompt_seconds_total: f64,
    predicted_tokens_total: f64,
    predicted_seconds_total: f64,
}

fn counter_rate(
    current_tokens: f64,
    previous_tokens: f64,
    current_seconds: f64,
    previous_seconds: f64,
) -> f64 {
    let token_delta = current_tokens - previous_tokens;
    let second_delta = current_seconds - previous_seconds;

    if token_delta > 0.0 && second_delta > 0.0 {
        token_delta / second_delta
    } else {
        0.0
    }
}

pub struct LlamaCppAdapter {
    pub app_config: AppConfig,
    pub config: ServerConfig,
    gpu_env: GpuEnv,
    previous_counters: Mutex<Option<CounterSnapshot>>,
    previous_counter_session: Mutex<Option<String>>,
}

#[allow(dead_code)]
impl LlamaCppAdapter {
    pub fn new(app_config: AppConfig, config: ServerConfig, gpu_env: GpuEnv) -> Self {
        Self {
            app_config,
            config,
            gpu_env,
            previous_counters: Mutex::new(None),
            previous_counter_session: Mutex::new(None),
        }
    }

    pub async fn validate(&self) -> Result<()> {
        let bin_path = &self.app_config.llama_server_path;
        if bin_path.components().count() > 1 && !bin_path.exists() {
            return Err(anyhow!(
                "llama-server binary not found: {}. Set it in Configuration.",
                bin_path.display()
            ));
        }

        let use_hf = self.config.hf_repo.as_ref().is_some_and(|r| !r.is_empty());
        let has_model_path = !self.config.model_path.is_empty();

        if use_hf && has_model_path {
            return Err(anyhow!(
                "Cannot use both model_path and hf_repo. Choose one."
            ));
        }

        if !use_hf && has_model_path {
            if !std::path::Path::new(&self.config.model_path).exists() {
                return Err(anyhow!("Model file not found: {}", self.config.model_path));
            }
        } else if !use_hf && !has_model_path {
            return Err(anyhow!(
                "No model source specified. Provide model_path or hf_repo."
            ));
        }

        self.validate_binary().await
    }

    async fn validate_binary(&self) -> Result<()> {
        let bin_path = &self.app_config.llama_server_path;

        #[cfg(target_os = "macos")]
        if let Some(bin_dir) = bin_path.parent() {
            let _ = std::process::Command::new("xattr")
                .args(["-rd", "com.apple.quarantine"])
                .arg(bin_dir)
                .output();
        }

        let output = tokio::time::timeout(Duration::from_secs(10), async {
            TokioCommand::new(bin_path)
                .arg("--help")
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::piped())
                .output()
                .await
        })
        .await
        .map_err(|_| anyhow!("llama-server did not respond to its health check within 10 seconds"))?
        .map_err(|error| anyhow!("Failed to execute llama-server health check: {error}"))?;

        if output.status.success() {
            return Ok(());
        }

        let detail = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let status = describe_process_status(output.status);
        if detail.is_empty() {
            Err(anyhow!(
                "llama-server health check failed ({status}). The binary may be corrupted or incompatible."
            ))
        } else {
            Err(anyhow!(
                "llama-server health check failed ({status}): {detail}"
            ))
        }
    }

    pub async fn build_launch(&self) -> Result<SupervisedLaunch> {
        let mut cmd = TokioCommand::new(&self.app_config.llama_server_path);
        crate::platform::no_window_tokio(&mut cmd);
        cmd.current_dir(&self.app_config.llama_server_cwd);

        let use_hf = self.config.hf_repo.as_ref().is_some_and(|r| !r.is_empty());
        if use_hf {
            if let Some(ref repo) = self.config.hf_repo {
                cmd.arg("-hf").arg(repo);
            }
        } else {
            cmd.arg("-m").arg(&self.config.model_path);
        }

        let ngl_str = match self.config.gpu_layers {
            Some(-1) | None => "all".to_string(),
            Some(n) => n.to_string(),
        };
        cmd.arg("-ngl").arg(&ngl_str);
        cmd.arg("-ctk").arg(&self.config.ctk);
        cmd.arg("-ctv").arg(&self.config.ctv);
        cmd.arg("--host")
            .arg(self.config.bind_host.as_deref().unwrap_or("127.0.0.1"));
        cmd.arg("--port").arg(self.config.port.to_string());
        if self.config.context_size > 0 {
            cmd.arg("-c").arg(self.config.context_size.to_string());
        }
        if self.config.batch_size > 0 {
            cmd.arg("-b").arg(self.config.batch_size.to_string());
        }
        if self.config.ubatch_size > 0 {
            cmd.arg("-ub").arg(self.config.ubatch_size.to_string());
        }
        cmd.arg("--no-warmup");
        cmd.arg("--jinja");
        cmd.arg("--webui-mcp-proxy");
        cmd.arg("--no-context-shift");
        cmd.arg("--ctx-checkpoints").arg("32");
        cmd.arg("--keep").arg("-1");

        if self.config.no_mmap {
            cmd.arg("--no-mmap");
        }
        if self.config.mlock {
            cmd.arg("--mlock");
        }

        let fa_value = if self.config.flash_attn == "off" {
            "off"
        } else {
            "on"
        };
        cmd.arg("-fa").arg(fa_value);

        if !self.config.tensor_split.is_empty() {
            cmd.arg("-ts").arg(&self.config.tensor_split);
        }
        if !self.config.split_mode.is_empty() {
            cmd.arg("--split-mode").arg(&self.config.split_mode);
        }
        if let Some(mg) = self.config.main_gpu {
            cmd.arg("-mg").arg(mg.to_string());
        }

        if let Some(t) = self.config.threads
            && (t == -1 || t > 0)
        {
            cmd.arg("-t").arg(t.to_string());
        }
        if let Some(tb) = self.config.threads_batch
            && (tb == -1 || tb > 0)
        {
            cmd.arg("-tb").arg(tb.to_string());
        }

        if let Some(p) = self.config.prio {
            cmd.arg("--prio").arg(p.to_string());
        }
        if let Some(pb) = self.config.prio_batch {
            cmd.arg("--prio-batch").arg(pb.to_string());
        }

        if !self.config.rope_scaling.is_empty() {
            cmd.arg("--rope-scaling").arg(&self.config.rope_scaling);
        } else if self.config.context_size > 262144 {
            cmd.arg("--rope-scaling").arg("yarn");
        }
        if let Some(base) = self.config.rope_freq_base {
            cmd.arg("--rope-freq-base").arg(format!("{:.6}", base));
        }
        if let Some(scale) = self.config.rope_freq_scale {
            cmd.arg("--rope-freq-scale").arg(format!("{:.6}", scale));
        } else if self.config.rope_scaling.is_empty() && self.config.context_size > 262144 {
            let scale = 262144.0 / self.config.context_size as f64;
            cmd.arg("--rope-freq-scale").arg(format!("{:.6}", scale));
            cmd.arg("--yarn-ext-factor").arg("1.0");
            cmd.arg("--yarn-attn-factor").arg("1.0");
            cmd.arg("--yarn-beta-fast").arg("32");
            cmd.arg("--yarn-beta-slow").arg("1");
        }

        let s = &self.config.spec;
        let spec_type_effective = if s.spec_type.is_some() {
            s.spec_type.clone()
        } else if self.config.ngram_spec {
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

        if self.config.ngram_spec {
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

        if self.config.parallel_slots > 0 {
            cmd.arg("--parallel")
                .arg(self.config.parallel_slots.to_string());
        }

        if let Some(t) = self.config.temperature {
            cmd.arg("--temp").arg(format!("{:.2}", t));
        }
        if let Some(tp) = self.config.top_p {
            cmd.arg("--top-p").arg(format!("{:.4}", tp));
        }
        if let Some(tk) = self.config.top_k {
            cmd.arg("--top-k").arg(tk.to_string());
        }
        if let Some(mp) = self.config.min_p {
            cmd.arg("--min-p").arg(format!("{:.4}", mp));
        }
        if let Some(rp) = self.config.repeat_penalty {
            cmd.arg("--repeat-penalty").arg(format!("{:.2}", rp));
        }
        if let Some(pp) = self.config.presence_penalty {
            cmd.arg("--presence-penalty").arg(format!("{:.4}", pp));
        }
        if let Some(n) = self.config.n_cpu_moe {
            cmd.arg("--n-cpu-moe").arg(n.to_string());
        }

        if let Some(seed) = self.config.seed {
            cmd.arg("--seed").arg(seed.to_string());
        }
        if !self.config.system_prompt_file.is_empty() {
            cmd.arg("--system-prompt-file")
                .arg(&self.config.system_prompt_file);
        }

        if let Some(ref ct) = self.config.chat_template_file
            && !ct.is_empty()
        {
            cmd.arg("--chat-template-file").arg(ct);
        }

        {
            let mut kwargs = serde_json::Map::new();
            if let Some(et) = self.config.enable_thinking {
                kwargs.insert("enable_thinking".into(), serde_json::json!(et));
            }
            if let Some(pt) = self.config.preserve_thinking {
                kwargs.insert("preserve_thinking".into(), serde_json::json!(pt));
            }
            if let Some(ref tcf) = self.config.tool_call_format
                && !tcf.is_empty()
            {
                kwargs.insert("tool_call_format".into(), serde_json::json!(tcf));
            }
            if !kwargs.is_empty() {
                let json = serde_json::to_string(&kwargs).unwrap_or_default();
                cmd.arg("--chat-template-kwargs").arg(json);
            }
        }
        if let Some(ref mode) = self.config.reasoning
            && !mode.is_empty()
        {
            cmd.arg("--reasoning").arg(mode);
        }
        if let Some(budget) = self.config.reasoning_budget {
            cmd.arg("--reasoning-budget").arg(budget.to_string());
        }
        if let Some(ref msg) = self.config.reasoning_budget_message
            && !msg.is_empty()
        {
            cmd.arg("--reasoning-budget-message").arg(msg);
        }

        cmd.arg("--metrics");

        if let Some(ref mp) = self.config.mmproj
            && !mp.is_empty()
        {
            cmd.arg("--mmproj").arg(mp);
            if let Some(min) = self.config.image_min_tokens {
                cmd.arg("--image-min-tokens").arg(min.to_string());
            }
            if let Some(max) = self.config.image_max_tokens {
                cmd.arg("--image-max-tokens").arg(max.to_string());
            }
        }

        if let Some(ref g) = self.config.grammar
            && !g.is_empty()
        {
            cmd.arg("--grammar").arg(g);
        }
        if let Some(ref js) = self.config.json_schema
            && !js.is_empty()
        {
            cmd.arg("--json-schema").arg(js);
        }
        if let Some(mt) = self.config.max_tokens {
            cmd.arg("-n").arg(mt.to_string());
        }
        if let Some(ref ak) = self.config.api_key
            && !ak.is_empty()
        {
            cmd.arg("--api-key").arg(ak);
        }
        if let Some(ref al) = self.config.alias
            && !al.is_empty()
        {
            cmd.arg("--alias").arg(al);
        }

        self.append_kv_cache_args(&mut cmd);
        self.append_fit_args(&mut cmd);

        for arg in self.config.extra_args.split_whitespace() {
            cmd.arg(arg);
        }

        let args: Vec<OsString> = cmd.as_std().get_args().map(|a| a.to_owned()).collect();
        let program = PathBuf::from(cmd.as_std().get_program());

        let cwd = self.app_config.llama_server_cwd.display().to_string();
        let env = launch_environment(&self.app_config.gpu_backend, &self.gpu_env, &cwd);

        Ok(SupervisedLaunch {
            program,
            args,
            env,
            cwd: Some(self.app_config.llama_server_cwd.clone()),
            port: self.config.port,
            redacted_summary: format!(
                "llama-server on port={} model={}",
                self.config.port,
                if !self.config.model_path.is_empty() {
                    &self.config.model_path
                } else if let Some(ref r) = self.config.hf_repo {
                    r
                } else {
                    "<unknown>"
                }
            ),
        })
    }

    fn append_kv_cache_args(&self, cmd: &mut TokioCommand) {
        if let Some(v) = self.config.kv_unified {
            cmd.arg(if v { "--kv-unified" } else { "--no-kv-unified" });
        }
        if let Some(v) = self.config.cache_idle_slots {
            if v {
                let cache_enabled = self.config.cache_ram_mib.map(|mib| mib > 0).unwrap_or(true);
                if cache_enabled {
                    if self.config.kv_unified.is_none() {
                        cmd.arg("--kv-unified");
                    }
                    cmd.arg("--cache-idle-slots");
                }
            } else {
                cmd.arg("--no-cache-idle-slots");
            }
        }
        if let Some(v) = self.config.cache_ram_mib {
            cmd.arg("--cache-ram").arg(v.to_string());
        }
    }

    fn append_fit_args(&self, cmd: &mut TokioCommand) {
        match self.config.fit_enabled {
            None => return,
            Some(false) => {
                cmd.arg("--fit").arg("off");
                return;
            }
            Some(true) => {}
        }

        cmd.arg("--fit").arg("on");
        if let Some(ref v) = self.config.fit_target {
            cmd.arg("--fit-target").arg(v);
        } else if let Some(v) = self.config.fit_ctx {
            cmd.arg("--fit-ctx").arg(v.to_string());
        }
    }

    pub async fn await_ready(&self, port: u16, deadline: Instant) -> Result<()> {
        let client = Client::builder().timeout(Duration::from_secs(5)).build()?;

        let host = readiness_host(self.config.bind_host.as_deref());
        let url = format!("http://{host}:{port}/health");
        let api_key = &self.config.api_key;

        loop {
            if Instant::now() > deadline {
                return Err(anyhow!("LlamaCppAdapter: timeout waiting for readiness"));
            }

            let req = if let Some(key) = api_key {
                client
                    .get(&url)
                    .header("Authorization", format!("Bearer {}", key))
            } else {
                client.get(&url)
            };

            if let Ok(resp) = req.send().await
                && resp.status().is_success()
            {
                return Ok(());
            }

            tokio::time::sleep(Duration::from_millis(500)).await;
        }
    }

    pub async fn poll_metrics(
        &self,
        port: u16,
        session_id: &str,
    ) -> Result<InferenceMetricsSnapshot> {
        let client = Client::builder()
            .timeout(Duration::from_secs(5))
            .pool_max_idle_per_host(0)
            .pool_idle_timeout(Duration::from_secs(0))
            .build()?;

        let base = format!("http://127.0.0.1:{}", port);
        let api_key = &self.config.api_key;

        let mut snapshot = InferenceMetricsSnapshot {
            sampled_at: std::time::SystemTime::now(),
            backend: InferenceBackend::LlamaCpp,
            health: None,
            ready: None,
            model: None,
            uptime_seconds: None,
            generation_tokens_per_second: None,
            prompt_tokens_per_second: None,
            running_requests: None,
            waiting_requests: None,
            completed_requests_total: None,
            prompt_tokens_total: None,
            completion_tokens_total: None,
            steps_executed: None,
            global_cache_hit_rate: None,
            global_cache_entries: None,
            ttft: None,
            speculative_acceptance_rate: None,
            active_memory_bytes: None,
            peak_memory_bytes: None,
            cache_memory_bytes: None,
            cache_metrics: None,
            active_requests: None,
            backend_details: None,
        };

        // Health check
        let health_req = if let Some(key) = api_key {
            client
                .get(format!("{base}/health"))
                .header("Authorization", format!("Bearer {}", key))
        } else {
            client.get(format!("{base}/health"))
        };

        if let Ok(resp) = health_req.send().await
            && let Ok(body) = resp.text().await
            && let Ok(json) = serde_json::from_str::<serde_json::Value>(&body)
        {
            snapshot.health = Some(match json.get("status").and_then(|v| v.as_str()) {
                Some("running") => HealthState::Ok,
                Some("degraded") => HealthState::Degraded,
                Some("not_loaded") => HealthState::NotLoaded,
                _ => HealthState::Unreachable,
            });
            snapshot.ready = json.get("ready").and_then(|v| v.as_bool());
        }

        // Prometheus metrics
        let metrics_req = if let Some(key) = api_key {
            client
                .get(format!("{base}/metrics"))
                .header("Authorization", format!("Bearer {}", key))
        } else {
            client.get(format!("{base}/metrics"))
        };

        if let Ok(resp) = metrics_req.send().await
            && let Ok(body) = resp.text().await
        {
            let prom = parse_prometheus_metrics(&body);
            snapshot.prompt_tokens_total = Some(prom.prompt_tokens_total as u64);
            snapshot.completion_tokens_total = Some(prom.predicted_tokens_total as u64);
            snapshot.running_requests = Some(prom.requests_processing as u64);
            snapshot.steps_executed = Some(prom.n_decode_total as u64);

            let current_counters = CounterSnapshot {
                prompt_tokens_total: prom.prompt_tokens_total,
                prompt_seconds_total: prom.prompt_seconds_total,
                predicted_tokens_total: prom.predicted_tokens_total,
                predicted_seconds_total: prom.predicted_seconds_total,
            };

            let (prompt_tps, gen_tps) = {
                let prev_session = self.previous_counter_session.lock().unwrap();
                let prev_counters = self.previous_counters.lock().unwrap();

                if prev_session.as_deref() == Some(session_id) && prev_counters.is_some() {
                    let prev = prev_counters.as_ref().unwrap();
                    (
                        counter_rate(
                            current_counters.prompt_tokens_total,
                            prev.prompt_tokens_total,
                            current_counters.prompt_seconds_total,
                            prev.prompt_seconds_total,
                        ),
                        counter_rate(
                            current_counters.predicted_tokens_total,
                            prev.predicted_tokens_total,
                            current_counters.predicted_seconds_total,
                            prev.predicted_seconds_total,
                        ),
                    )
                } else {
                    (0.0, 0.0)
                }
            };

            *self.previous_counters.lock().unwrap() = Some(current_counters);
            *self.previous_counter_session.lock().unwrap() = Some(session_id.to_string());

            snapshot.prompt_tokens_per_second = Some(prompt_tps);
            snapshot.generation_tokens_per_second = Some(gen_tps);
        }

        // Slots metrics
        let slots_req = if let Some(key) = api_key {
            client
                .get(format!("{base}/slots"))
                .header("Authorization", format!("Bearer {}", key))
        } else {
            client.get(format!("{base}/slots"))
        };

        if let Ok(resp) = slots_req.send().await
            && let Ok(body) = resp.text().await
            && let Some(slots) = parse_slot_metrics(&body)
        {
            snapshot.backend_details = Some(serde_json::json!({
                "slots_idle": slots.slots_idle,
                "slots_processing": slots.slots_processing,
                "kv_cache_max": slots.kv_cache_max,
                "kv_cache_tokens": slots.kv_cache_tokens,
                "kv_cache_tokens_available": slots.kv_cache_tokens_available,
                "kv_cache_tokens_source": slots.kv_cache_tokens_source,
            }));
        }

        // Models metadata
        let models_req = if let Some(key) = api_key {
            client
                .get(format!("{base}/v1/models"))
                .header("Authorization", format!("Bearer {}", key))
        } else {
            client.get(format!("{base}/v1/models"))
        };

        if let Ok(resp) = models_req.send().await
            && let Ok(body) = resp.text().await
            && let Ok(json) = serde_json::from_str::<serde_json::Value>(&body)
            && let Some(model) = json["data"][0].get("id").and_then(|v| v.as_str())
        {
            snapshot.model = Some(model.to_string());
        }

        Ok(snapshot)
    }

    pub async fn cancel_request(&self, _port: u16, _request_id: &str) -> Result<()> {
        Err(anyhow!(
            "The active llama.cpp backend does not support native request cancellation"
        ))
    }

    pub fn capabilities(&self) -> &CapabilitySet {
        static CAPS: CapabilitySet = CapabilitySet {
            vision: true,
            mtp: false,
            cancellation: false,
            embeddings: true,
            guided_generation: true,
            audio: false,
            tool_parsing: true,
            automatic_tool_choice: true,
            reasoning_parser: true,
            thinking_controls: true,
            mcp: true,
            cache_telemetry: true,
            status_memory_telemetry: true,
            self_diagnostic: false,
            interpretability: false,
            one_shot_launch: false,
        };
        &CAPS
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    async fn launch_args(config: ServerConfig) -> Vec<String> {
        let config_dir = tempfile::tempdir().unwrap();
        let args = crate::cli::AppArgs::parse_from([
            "llama-monitor",
            "--config-dir",
            config_dir.path().to_str().unwrap(),
            "--llama-server-path",
            "llama-server",
            "--gpu-backend",
            "none",
        ]);
        let adapter = LlamaCppAdapter::new(AppConfig::from_args(args), config, GpuEnv::default());
        adapter
            .build_launch()
            .await
            .unwrap()
            .args
            .into_iter()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect()
    }

    #[test]
    fn readiness_uses_loopback_for_wildcard_bind_hosts() {
        assert_eq!(readiness_host(None), "127.0.0.1");
        assert_eq!(readiness_host(Some("0.0.0.0")), "127.0.0.1");
        assert_eq!(readiness_host(Some("::")), "127.0.0.1");
        assert_eq!(readiness_host(Some("192.168.1.10")), "192.168.1.10");
    }

    #[test]
    fn launch_environment_preserves_gpu_selection_and_custom_values() {
        let gpu_env = GpuEnv {
            devices: "1,2".into(),
            extra_env: vec![("LLAMA_TEST_ENV".into(), "present".into())],
            ..Default::default()
        };

        let nvidia = launch_environment("nvidia", &gpu_env, "/tmp/llama");
        assert!(nvidia.contains(&("CUDA_VISIBLE_DEVICES".into(), "1,2".into())));
        assert!(nvidia.contains(&("LLAMA_TEST_ENV".into(), "present".into())));
        assert!(launch_environment("none", &gpu_env, "/tmp/llama").is_empty());
    }

    #[test]
    fn server_config_keeps_spawn_v2_cache_fields() {
        let mut value = serde_json::to_value(ServerConfig::default()).unwrap();
        let object = value.as_object_mut().unwrap();
        object.insert("cache_type_k".into(), serde_json::json!("q8_0"));
        object.insert("cache_type_v".into(), serde_json::json!("q4_0"));
        let config: ServerConfig = serde_json::from_value(value).unwrap();

        assert_eq!(config.cache_type_k.as_deref(), Some("q8_0"));
        assert_eq!(config.cache_type_v.as_deref(), Some("q4_0"));
    }

    #[tokio::test]
    async fn default_launch_argv_matches_pre_refactor_contract() {
        let args = launch_args(ServerConfig {
            model_path: "/models/test.gguf".into(),
            ctk: "q8_0".into(),
            ctv: "q8_0".into(),
            port: 8080,
            ..Default::default()
        })
        .await;

        assert_eq!(
            args,
            [
                "-m",
                "/models/test.gguf",
                "-ngl",
                "all",
                "-ctk",
                "q8_0",
                "-ctv",
                "q8_0",
                "--host",
                "127.0.0.1",
                "--port",
                "8080",
                "--no-warmup",
                "--jinja",
                "--webui-mcp-proxy",
                "--no-context-shift",
                "--ctx-checkpoints",
                "32",
                "--keep",
                "-1",
                "-fa",
                "on",
                "--metrics",
            ]
        );
    }

    #[tokio::test]
    async fn optional_launch_argv_preserves_order_and_values() {
        let args = launch_args(ServerConfig {
            model_path: "/models/full.gguf".into(),
            context_size: 4096,
            ctk: "q4_0".into(),
            ctv: "q5_0".into(),
            tensor_split: "3,1".into(),
            batch_size: 512,
            ubatch_size: 128,
            no_mmap: true,
            port: 9090,
            parallel_slots: 2,
            temperature: Some(0.7),
            top_p: Some(0.95),
            top_k: Some(40),
            min_p: Some(0.05),
            repeat_penalty: Some(1.1),
            presence_penalty: Some(0.2),
            n_cpu_moe: Some(4),
            gpu_layers: Some(42),
            mlock: true,
            flash_attn: "off".into(),
            split_mode: "layer".into(),
            main_gpu: Some(1),
            threads: Some(8),
            threads_batch: Some(12),
            prio: Some(2),
            prio_batch: Some(3),
            rope_scaling: "yarn".into(),
            rope_freq_base: Some(10_000.0),
            rope_freq_scale: Some(0.5),
            kv_unified: Some(true),
            cache_idle_slots: Some(true),
            cache_ram_mib: Some(2048),
            fit_enabled: Some(true),
            fit_target: Some("3072".into()),
            seed: Some(7),
            system_prompt_file: "/prompts/system.txt".into(),
            extra_args: "--verbose --log-colors off".into(),
            bind_host: Some("0.0.0.0".into()),
            alias: Some("full-model".into()),
            chat_template_file: Some("/templates/chat.jinja".into()),
            mmproj: Some("/models/mmproj.gguf".into()),
            grammar: Some("root ::= answer".into()),
            json_schema: Some("{\"type\":\"object\"}".into()),
            max_tokens: Some(256),
            api_key: Some("secret".into()),
            reasoning: Some("auto".into()),
            reasoning_budget: Some(512),
            reasoning_budget_message: Some("done".into()),
            image_min_tokens: Some(280),
            image_max_tokens: Some(560),
            ..Default::default()
        })
        .await;

        let expected_tail = [
            "--api-key",
            "secret",
            "--alias",
            "full-model",
            "--kv-unified",
            "--cache-idle-slots",
            "--cache-ram",
            "2048",
            "--fit",
            "on",
            "--fit-target",
            "3072",
            "--verbose",
            "--log-colors",
            "off",
        ];
        assert!(args.ends_with(&expected_tail.map(str::to_string)));
        for required in [
            "--no-mmap",
            "--mlock",
            "--split-mode",
            "--rope-scaling",
            "--parallel",
            "--chat-template-file",
            "--reasoning-budget",
            "--mmproj",
            "--grammar",
            "--json-schema",
            "--image-min-tokens",
            "--image-max-tokens",
        ] {
            assert!(args.iter().any(|arg| arg == required), "missing {required}");
        }
    }
}
