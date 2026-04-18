#![recursion_limit = "256"]

mod cli;
mod config;
mod gpu;
mod lhm;
mod lhm_persistence;
mod llama;
mod models;
mod presets;
mod state;
mod system;
mod tray;
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

    // Load sessions from disk (or defaults)
    let _sessions = state::load_sessions(&app_config.sessions_file);

    let state = state::AppState::new(
        initial_presets,
        state::AppPaths {
            presets_path: app_config.presets_file.clone(),
            models_dir: app_config.models_dir.clone(),
            gpu_env_path: app_config.gpu_env_file.clone(),
            ui_settings_path: app_config.ui_settings_file.clone(),
            sessions_path: app_config.sessions_file.clone(),
        },
        gpu_env,
        ui_settings,
    );

    if let Some(ref dir) = app_config.models_dir {
        match state.discovered_models.lock() {
            Ok(models) => {
                let count = models.len();
                println!("[info] Discovered {count} models in {}", dir.display());
            }
            Err(e) => {
                eprintln!("[error] Failed to acquire discovered_models lock: {e}");
            }
        }
    }

    // Detect and start GPU poller
    let backend = gpu::detect_backend(&app_config.gpu_backend);
    {
        let gpu = state.gpu_metrics.clone();
        thread::spawn(move || {
            loop {
                match backend.read_metrics() {
                    Ok(m) => {
                        if let Ok(mut gpu_lock) = gpu.lock() {
                            *gpu_lock = m;
                        } else {
                            eprintln!("[error] Failed to acquire gpu lock");
                        }
                    }
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
                if let Ok(mut sys_lock) = s.system_metrics.lock() {
                    *sys_lock = metrics;
                } else {
                    eprintln!("[error] Failed to acquire system_metrics lock");
                }
                std::thread::sleep(SYSTEM_POLL_INTERVAL);
            }
        });
    }

    // Llama metrics poller
    {
        let s = state.clone();
        let interval = app_config.llama_poll_interval;
        tokio::spawn(llama::poller::llama_metrics_poller(s, interval));
    }

    let port = app_config.port;
    let routes = web::build_routes(state.clone(), app_config.clone());

    println!("[info] Llama Monitor running on http://0.0.0.0:{port}");

    // Start system tray (blocking call, runs in separate thread)
    thread::spawn({
        let state = state.clone();
        move || {
            crate::tray::run_tray(state, port);
        }
    });

    // Start sessions persistence timer
    {
        let state = state.clone();
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(tokio::time::Duration::from_secs(30)).await;
                if let Err(e) =
                    state::save_sessions(&app_config.sessions_file, &state.get_sessions())
                {
                    eprintln!("[error] Failed to save sessions: {}", e);
                }
            }
        });
    }

    warp::serve(routes).run(([0, 0, 0, 0], port)).await;

    Ok(())
}
