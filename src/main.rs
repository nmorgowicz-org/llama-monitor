mod cli;
mod config;
mod gpu;
mod llama;
mod models;
mod presets;
mod state;
mod system;
mod web;

use anyhow::Result;
use clap::Parser;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

const GPU_POLL_INTERVAL: Duration = Duration::from_millis(500);
const SYSTEM_POLL_INTERVAL: Duration = Duration::from_secs(5);

#[tokio::main]
async fn main() -> Result<()> {
    let args = cli::AppArgs::parse();
    let app_config = Arc::new(config::AppConfig::from_args(args));

    // Load presets from disk (or defaults)
    let initial_presets = presets::load_presets(&app_config.presets_file);
    println!(
        "[info] Loaded {} presets from {}",
        initial_presets.len(),
        app_config.presets_file.display()
    );

    // Load GPU environment config
    let mut gpu_env = gpu::env::load_gpu_env(&app_config.gpu_env_file);

    // CLI overrides take precedence
    if let Some(ref arch) = app_config.gpu_arch_override {
        gpu_env.arch = arch.clone();
    }
    if let Some(ref devices) = app_config.gpu_devices_override {
        gpu_env.devices = devices.clone();
    }

    // Auto-detect GPUs and log results
    if let Some(detected) = gpu::env::detect_gpus() {
        println!(
            "[info] Detected {}x {} GPU(s)",
            detected.count, detected.arch
        );
        // If arch is "auto" and devices is empty, suggest detected values
        if gpu_env.arch == "auto" && gpu_env.devices.is_empty() {
            gpu_env.devices = gpu::env::device_list_for_count(detected.count);
        }
    }

    println!(
        "[info] GPU env: arch={}, devices={}",
        gpu_env.arch,
        if gpu_env.devices.is_empty() {
            "all"
        } else {
            &gpu_env.devices
        }
    );

    // Load UI settings from disk (or defaults)
    let ui_settings = state::load_ui_settings(&app_config.ui_settings_file);

    let state = state::AppState::new(
        initial_presets,
        app_config.presets_file.clone(),
        app_config.models_dir.clone(),
        gpu_env,
        app_config.gpu_env_file.clone(),
        ui_settings,
        app_config.ui_settings_file.clone(),
    );

    if let Some(ref dir) = app_config.models_dir {
        let count = state.discovered_models.lock().unwrap().len();
        println!("[info] Discovered {count} models in {}", dir.display());
    }

    // Detect and start GPU poller
    let backend = gpu::detect_backend(&app_config.gpu_backend);
    {
        let gpu = state.gpu_metrics.clone();
        thread::spawn(move || {
            loop {
                match backend.read_metrics() {
                    Ok(m) => *gpu.lock().unwrap() = m,
                    Err(e) => eprintln!("[error] GPU metrics: {e}"),
                }
                thread::sleep(GPU_POLL_INTERVAL);
            }
        });
    }

    // System metrics poller
    {
        let s = state.clone();
        thread::spawn(move || {
            loop {
                let metrics = system::get_system_metrics();
                *s.system_metrics.lock().unwrap() = metrics;
                std::thread::sleep(SYSTEM_POLL_INTERVAL);
            }
        });
    }

    // Llama metrics poller
    {
        let s = state.clone();
        tokio::spawn(async move { llama::poller::llama_metrics_poller(s).await });
    }

    let port = app_config.port;
    let routes = web::build_routes(state, app_config);

    println!("[info] Llama Monitor running on http://0.0.0.0:{port}");
    warp::serve(routes).run(([0, 0, 0, 0], port)).await;

    Ok(())
}
