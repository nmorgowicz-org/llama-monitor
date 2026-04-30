#![recursion_limit = "256"]

mod agent;
mod certs;
mod cli;
mod config;
mod gpu;
mod lhm;
mod lhm_persistence;
mod llama;
mod models;
mod presets;
mod remote_ssh;
mod state;
mod system;
#[cfg(feature = "native-tray")]
mod tray;
mod web;

use anyhow::Result;
use clap::Parser;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

const GPU_POLL_INTERVAL: Duration = Duration::from_millis(500);
const SYSTEM_POLL_INTERVAL: Duration = Duration::from_secs(5);

fn main() -> Result<()> {
    let args = cli::AppArgs::parse();
    let app_config = Arc::new(config::AppConfig::from_args(args.clone()));

    if args.agent {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()?;
        return runtime.block_on(agent::run_agent_server(app_config));
    }

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
            templates_path: app_config.templates_file.clone(),
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
        let s = state.clone();
        thread::spawn(move || {
            loop {
                if s.active_session_uses_local_metrics() {
                    match backend.read_metrics() {
                        Ok(m) => {
                            if let Ok(mut gpu_lock) = s.gpu_metrics.lock() {
                                *gpu_lock = m;
                            } else {
                                eprintln!("[error] Failed to acquire gpu lock");
                            }
                            // Feed CPU/SoC temp from GPU backend (Apple only)
                            if let Some(t) = backend.cpu_temp()
                                && let Ok(mut sys_lock) = s.system_metrics.lock()
                            {
                                sys_lock.cpu_temp = t;
                                sys_lock.cpu_temp_available = true;
                            }
                        }
                        Err(e) => eprintln!("[error] GPU metrics: {e}"),
                    };
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
                if s.active_session_uses_local_metrics() {
                    let mut metrics = system::get_system_metrics();
                    if let Ok(mut sys_lock) = s.system_metrics.lock() {
                        // Preserve CPU temp if the GPU backend already provided one
                        // (e.g. Apple mactop) since get_system_metrics() can't read it.
                        if !metrics.cpu_temp_available && sys_lock.cpu_temp_available {
                            metrics.cpu_temp = sys_lock.cpu_temp;
                            metrics.cpu_temp_available = true;
                        }
                        *sys_lock = metrics;
                    } else {
                        eprintln!("[error] Failed to acquire system_metrics lock");
                    }
                }
                std::thread::sleep(SYSTEM_POLL_INTERVAL);
            }
        });
    }

    let port = app_config.port;
    let host = args.host.clone();

    // Parse basic auth credentials
    let basic_auth_enabled = match args.basic_auth.as_ref() {
        Some(s) => {
            let parts: Vec<&str> = s.splitn(2, ':').collect();
            if parts.len() == 2 {
                Some((parts[0].to_string(), parts[1].to_string()))
            } else {
                eprintln!("[error] Invalid --basic-auth format. Expected: user:password");
                std::process::exit(1);
            }
        }
        None => None,
    };

    let routes = web::build_routes(
        state.clone(),
        app_config.clone(),
        basic_auth_enabled.clone(),
    );

    let auth_note = if basic_auth_enabled.is_some() {
        " (Basic Auth enabled)"
    } else {
        ""
    };
    println!(
        "[info] Llama Monitor running on http://{}:{}{}",
        host, port, auth_note
    );

    if args.headless {
        println!("[info] Headless mode enabled (no tray, no desktop UI)");
    } else if args.no_tray {
        println!("[info] Tray disabled via --no-tray");
    }

    // Build tokio runtime for async tasks (warp server, pollers, etc.)
    // The runtime runs in background threads; the main thread is reserved
    // for the system tray, which macOS requires to be on the main thread.
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;

    // Llama metrics poller. It remains idle until the user starts a preset or
    // explicitly attaches to an endpoint.
    {
        let s = state.clone();
        let interval = app_config.llama_poll_interval;
        runtime.spawn(llama::poller::llama_metrics_poller(s, interval));
    }

    // Remote host metrics poller. It is gated by the same user action signal,
    // so app startup never probes saved remote endpoints automatically.
    {
        let s = state.clone();
        let app_config = app_config.clone();
        runtime.spawn(agent::remote_agent_poller(s, app_config));
    }

    // Sessions persistence timer
    {
        let state = state.clone();
        runtime.spawn(async move {
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

    // Warp server
    runtime.spawn(async move {
        let addr: std::net::SocketAddr = format!("{}:{}", host, port)
            .parse()
            .expect("Invalid host:port");
        warp::serve(routes).run(addr).await;
    });

    // Run tray on the main thread when a desktop session is available.
    // Headless Linux servers still keep the web UI/API running.
    #[cfg(feature = "native-tray")]
    {
        if should_start_tray(&args) {
            if let Err(e) = crate::tray::run_tray(state, port) {
                eprintln!("[warn] Tray unavailable: {e}");
                eprintln!("[info] Continuing in headless mode with web/API server");
            }
        } else {
            println!("[info] Tray disabled (no graphical session)");
            park_forever();
        }
    }

    #[cfg(not(feature = "native-tray"))]
    {
        let _ = state;
        println!("[info] Tray disabled in this build");
    }

    park_forever()
}

#[cfg(target_os = "linux")]
pub fn should_start_tray(args: &cli::AppArgs) -> bool {
    if args.headless || args.no_tray {
        return false;
    }
    std::env::var_os("DISPLAY").is_some() || std::env::var_os("WAYLAND_DISPLAY").is_some()
}

#[cfg(not(target_os = "linux"))]
#[cfg(feature = "native-tray")]
pub fn should_start_tray(args: &cli::AppArgs) -> bool {
    if args.headless || args.no_tray {
        return false;
    }
    true
}

#[cfg(not(feature = "native-tray"))]
pub fn should_start_tray(_args: &cli::AppArgs) -> bool {
    false
}

fn park_forever() -> ! {
    loop {
        std::thread::park();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn should_start_tray_flag_combinations_disables_tray() {
        let test_cases = [
            (true, false, false), // headless only
            (false, true, false), // no_tray only
            (true, true, false),  // both flags
        ];

        for (headless, no_tray, expected) in test_cases {
            let args = cli::AppArgs {
                port: 7778,
                gpu_backend: "auto".to_string(),
                models_dir: None,
                gpu_arch: None,
                gpu_devices: None,
                llama_poll_interval: 1,
                llama_server_path: None,
                llama_server_cwd: None,
                presets_file: None,
                sessions_file: None,
                config_dir: None,
                headless,
                no_tray,
                agent: false,
                agent_host: "127.0.0.1".to_string(),
                host: "127.0.0.1".to_string(),
                basic_auth: None,
                agent_port: 7779,
                agent_token: None,
                remote_agent_url: None,
                remote_agent_token: None,
                remote_agent_ssh_autostart: false,
                remote_agent_ssh_target: None,
                remote_agent_ssh_command: None,
            };
            assert_eq!(
                should_start_tray(&args),
                expected,
                "headless={headless}, no_tray={no_tray}"
            );
        }
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn should_start_tray_linux_display_variations() {
        unsafe { std::env::set_var("DISPLAY", ":0") };
        let args = cli::AppArgs {
            port: 7778,
            gpu_backend: "auto".to_string(),
            models_dir: None,
            gpu_arch: None,
            gpu_devices: None,
            llama_poll_interval: 1,
            llama_server_path: None,
            llama_server_cwd: None,
            presets_file: None,
            sessions_file: None,
            config_dir: None,
            headless: false,
            no_tray: false,
            agent: false,
            agent_host: "127.0.0.1".to_string(),
            host: "127.0.0.1".to_string(),
            basic_auth: None,
            agent_port: 7779,
            agent_token: None,
            remote_agent_url: None,
            remote_agent_token: None,
            remote_agent_ssh_autostart: false,
            remote_agent_ssh_target: None,
            remote_agent_ssh_command: None,
        };
        assert!(should_start_tray(&args));
        unsafe { std::env::remove_var("DISPLAY") };

        unsafe { std::env::set_var("WAYLAND_DISPLAY", "wayland-0") };
        let args = cli::AppArgs {
            port: 7778,
            gpu_backend: "auto".to_string(),
            models_dir: None,
            gpu_arch: None,
            gpu_devices: None,
            llama_poll_interval: 1,
            llama_server_path: None,
            llama_server_cwd: None,
            presets_file: None,
            sessions_file: None,
            config_dir: None,
            headless: false,
            no_tray: false,
            agent: false,
            agent_host: "127.0.0.1".to_string(),
            host: "127.0.0.1".to_string(),
            basic_auth: None,
            agent_port: 7779,
            agent_token: None,
            remote_agent_url: None,
            remote_agent_token: None,
            remote_agent_ssh_autostart: false,
            remote_agent_ssh_target: None,
            remote_agent_ssh_command: None,
        };
        assert!(should_start_tray(&args));
        unsafe { std::env::remove_var("WAYLAND_DISPLAY") };

        unsafe { std::env::remove_var("DISPLAY") };
        unsafe { std::env::remove_var("WAYLAND_DISPLAY") };
        let args = cli::AppArgs {
            port: 7778,
            gpu_backend: "auto".to_string(),
            models_dir: None,
            gpu_arch: None,
            gpu_devices: None,
            llama_poll_interval: 1,
            llama_server_path: None,
            llama_server_cwd: None,
            presets_file: None,
            sessions_file: None,
            config_dir: None,
            headless: false,
            no_tray: false,
            agent: false,
            agent_host: "127.0.0.1".to_string(),
            host: "127.0.0.1".to_string(),
            basic_auth: None,
            agent_port: 7779,
            agent_token: None,
            remote_agent_url: None,
            remote_agent_token: None,
            remote_agent_ssh_autostart: false,
            remote_agent_ssh_target: None,
            remote_agent_ssh_command: None,
        };
        assert!(!should_start_tray(&args));
    }

    #[cfg(all(not(target_os = "linux"), feature = "native-tray"))]
    #[test]
    fn should_start_tray_non_linux_default_enabled() {
        let args = cli::AppArgs {
            port: 7778,
            gpu_backend: "auto".to_string(),
            models_dir: None,
            gpu_arch: None,
            gpu_devices: None,
            llama_poll_interval: 1,
            llama_server_path: None,
            llama_server_cwd: None,
            presets_file: None,
            sessions_file: None,
            config_dir: None,
            headless: false,
            no_tray: false,
            agent: false,
            agent_host: "127.0.0.1".to_string(),
            host: "127.0.0.1".to_string(),
            basic_auth: None,
            agent_port: 7779,
            agent_token: None,
            remote_agent_url: None,
            remote_agent_token: None,
            remote_agent_ssh_autostart: false,
            remote_agent_ssh_target: None,
            remote_agent_ssh_command: None,
        };
        assert!(should_start_tray(&args));
    }

    #[cfg(not(feature = "native-tray"))]
    #[test]
    fn should_start_tray_without_desktop_feature_disabled() {
        let args = cli::AppArgs {
            port: 7778,
            gpu_backend: "auto".to_string(),
            models_dir: None,
            gpu_arch: None,
            gpu_devices: None,
            llama_poll_interval: 1,
            llama_server_path: None,
            llama_server_cwd: None,
            presets_file: None,
            sessions_file: None,
            config_dir: None,
            headless: false,
            no_tray: false,
            agent: false,
            agent_host: "127.0.0.1".to_string(),
            host: "127.0.0.1".to_string(),
            basic_auth: None,
            agent_port: 7779,
            agent_token: None,
            remote_agent_url: None,
            remote_agent_token: None,
            remote_agent_ssh_autostart: false,
            remote_agent_ssh_target: None,
            remote_agent_ssh_command: None,
        };
        assert!(!should_start_tray(&args));
    }
}
