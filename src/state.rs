use std::collections::{BTreeMap, VecDeque};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use crate::gpu::GpuMetrics;
use crate::gpu::env::GpuEnv;
use crate::llama::metrics::LlamaMetrics;
use crate::llama::server::ServerConfig;
use crate::models::DiscoveredModel;
use crate::presets::ModelPreset;

const MAX_LOG_LINES: usize = 500;

#[derive(Clone)]
pub struct AppState {
    pub gpu_metrics: Arc<Mutex<BTreeMap<String, GpuMetrics>>>,
    pub llama_metrics: Arc<Mutex<LlamaMetrics>>,
    pub server_logs: Arc<Mutex<VecDeque<String>>>,
    pub server_child: Arc<tokio::sync::Mutex<Option<tokio::process::Child>>>,
    pub server_running: Arc<Mutex<bool>>,
    pub server_config: Arc<Mutex<Option<ServerConfig>>>,
    pub llama_poll_notify: Arc<tokio::sync::Notify>,
    pub presets: Arc<Mutex<Vec<ModelPreset>>>,
    pub presets_path: PathBuf,
    pub discovered_models: Arc<Mutex<Vec<DiscoveredModel>>>,
    pub models_dir: Option<PathBuf>,
    pub gpu_env: Arc<Mutex<GpuEnv>>,
    pub gpu_env_path: PathBuf,
}

impl AppState {
    pub fn new(
        presets: Vec<ModelPreset>,
        presets_path: PathBuf,
        models_dir: Option<PathBuf>,
        gpu_env: GpuEnv,
        gpu_env_path: PathBuf,
    ) -> Self {
        let discovered = models_dir
            .as_ref()
            .and_then(|dir| crate::models::scan_models_dir(dir).ok())
            .unwrap_or_default();

        Self {
            gpu_metrics: Arc::new(Mutex::new(BTreeMap::new())),
            llama_metrics: Arc::new(Mutex::new(LlamaMetrics::default())),
            server_logs: Arc::new(Mutex::new(VecDeque::new())),
            server_child: Arc::new(tokio::sync::Mutex::new(None)),
            server_running: Arc::new(Mutex::new(false)),
            server_config: Arc::new(Mutex::new(None)),
            llama_poll_notify: Arc::new(tokio::sync::Notify::new()),
            presets: Arc::new(Mutex::new(presets)),
            presets_path,
            discovered_models: Arc::new(Mutex::new(discovered)),
            models_dir,
            gpu_env: Arc::new(Mutex::new(gpu_env)),
            gpu_env_path,
        }
    }

    pub fn push_log(&self, line: String) {
        let mut logs = self.server_logs.lock().unwrap();
        if logs.len() >= MAX_LOG_LINES {
            logs.pop_front();
        }
        logs.push_back(line);
    }
}
