#![recursion_limit = "256"]

mod acme;
mod agent;
mod certs;
mod chat_storage;
mod cli;
mod config;
mod gpu;
mod hf;
mod lhm;
mod lhm_persistence;
mod llama;
mod model_download;
mod models;
mod presets;
mod remote_ssh;
mod state;
mod system;
#[cfg(feature = "native-tray")]
mod tray;
mod web;

use anyhow::{Context, Result};
use clap::Parser;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use crate::chat_storage::ChatStorage;
use crate::config::{
    DashboardAuthConfig, TlsMode, clear_auth_config, harden_file_permissions, load_auth_config,
    save_auth_config,
};
use crate::web::auth::AuthManager;

const GPU_POLL_INTERVAL: Duration = Duration::from_millis(500);
const SYSTEM_POLL_INTERVAL: Duration = Duration::from_secs(5);

fn main() -> Result<()> {
    let args = cli::AppArgs::parse();
    let app_config = Arc::new(config::AppConfig::from_args(args.clone()));

    if args.clear_auth_config {
        match clear_auth_config(&app_config.config_dir) {
            Ok(true) => {
                println!(
                    "[info] Cleared dashboard auth config at {}",
                    app_config.auth_config_file.display()
                );
            }
            Ok(false) => {
                println!(
                    "[info] No dashboard auth config found at {}",
                    app_config.auth_config_file.display()
                );
            }
            Err(err) => {
                eprintln!(
                    "[error] Failed to clear dashboard auth config at {}: {err}",
                    app_config.auth_config_file.display()
                );
                std::process::exit(1);
            }
        }
        return Ok(());
    }

    // Initialize at-rest encryption (auto-generates key if needed)
    config::init_encryption_key(&app_config.config_dir);

    // Harden permissions on secret files (Unix: 0600)
    harden_file_permissions(&app_config.ui_settings_file);
    harden_file_permissions(&app_config.sessions_file);
    harden_file_permissions(&app_config.ssh_known_hosts_file);
    harden_file_permissions(&app_config.config_dir.join("db-admin-token"));
    harden_file_permissions(&app_config.config_dir.join("api-token"));
    harden_file_permissions(&app_config.config_dir.join("tls-config.json"));
    harden_file_permissions(&app_config.config_dir.join("encryption-key"));
    harden_file_permissions(&app_config.auth_config_file);

    if args.agent {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .thread_stack_size(8 * 1024 * 1024)
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

    // Open chat database
    let chat_db_path = app_config.config_dir.join("chat.db");
    let chat_storage = Arc::new(ChatStorage::open(&chat_db_path).context("opening chat.db")?);

    // Migrate from legacy chat-tabs.json (best-effort)
    let legacy = app_config.config_dir.join("chat-tabs.json");
    if let Err(e) = chat_storage.migrate_from_legacy(&legacy) {
        eprintln!("[warn] chat legacy migration failed: {e}");
    }

    let state = state::AppState::new(
        initial_presets,
        state::AppPaths {
            presets_path: app_config.presets_file.clone(),
            templates_path: app_config.templates_file.clone(),
            models_dir: app_config.models_dir.clone().or(Some(app_config.default_models_dir.clone())),
            gpu_env_path: app_config.gpu_env_file.clone(),
            ui_settings_path: app_config.ui_settings_file.clone(),
            sessions_path: app_config.sessions_file.clone(),
        },
        gpu_env,
        ui_settings,
        chat_storage,
        app_config.tls_config.clone(),
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

    let basic_auth = match args.basic_auth.as_deref() {
        Some(spec) => match AuthManager::parse_credentials(spec) {
            Some(creds) => Some(creds),
            None => {
                eprintln!("[error] Invalid --basic-auth format. Expected: user:password");
                std::process::exit(1);
            }
        },
        None => None,
    };
    let form_auth = match args.form_auth.as_deref() {
        Some(spec) => match AuthManager::parse_credentials(spec) {
            Some(creds) => Some(creds),
            None => {
                eprintln!("[error] Invalid --form-auth format. Expected: user:password");
                std::process::exit(1);
            }
        },
        None => None,
    };

    // Apply CLI TLS overrides to tls_config
    let mut tls_config = app_config.tls_config.clone();
    if args.tls {
        if args.tls_cert.is_none() || args.tls_key.is_none() {
            eprintln!("[error] --tls requires both --tls-cert and --tls-key");
            std::process::exit(1);
        }
        tls_config.mode = TlsMode::Custom;
        tls_config.custom_cert_path = args.tls_cert.clone();
        tls_config.custom_key_path = args.tls_key.clone();
    } else if args.tls_self_signed {
        tls_config.mode = TlsMode::SelfSigned;
    }

    // Update in-memory state with final TLSConfig
    state.set_tls_config(tls_config.clone());

    migrate_legacy_dashboard_auth(
        &app_config.config_dir,
        &app_config.auth_config_file,
        basic_auth.as_ref(),
        form_auth.as_ref(),
    );

    let auth_manager = if basic_auth.is_some() || form_auth.is_some() {
        AuthManager::new(basic_auth.clone(), form_auth.clone(), &tls_config.mode)
    } else {
        AuthManager::from_config(load_auth_config(&app_config.config_dir), &tls_config.mode)
    };

    let routes = web::build_routes(
        state.clone(),
        app_config.clone(),
        auth_manager.clone(),
        host.clone(),
    );

    // Warn when listening on all interfaces without TLS or auth
    if host == "0.0.0.0" && tls_config.mode == TlsMode::None && !auth_manager.has_any() {
        eprintln!(
            "[warn] Listening on all interfaces without TLS or authentication. \
            Anyone on your network can access the UI."
        );
    }

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
        .thread_stack_size(8 * 1024 * 1024)
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
        let sessions_file = app_config.sessions_file.clone();
        runtime.spawn(async move {
            loop {
                tokio::time::sleep(tokio::time::Duration::from_secs(30)).await;
                if let Err(e) = state::save_sessions(&sessions_file, &state.get_sessions()) {
                    eprintln!("[error] Failed to save sessions: {}", e);
                }
            }
        });
    }

    // Hourly database maintenance: WAL checkpoint, ANALYZE, rolling backup (keep 24h)
    {
        let chat_storage = state.chat_storage.clone();
        let config_dir = app_config.config_dir.clone();
        runtime.spawn(async move {
            loop {
                tokio::time::sleep(tokio::time::Duration::from_secs(3600)).await;

                if let Err(e) = chat_storage.checkpoint() {
                    eprintln!("[error] WAL checkpoint failed: {}", e);
                }

                if let Err(e) = chat_storage.analyze() {
                    eprintln!("[error] ANALYZE failed: {}", e);
                }

                let auto_backup_dir = config_dir.join("backups").join("auto");
                if let Err(e) = std::fs::create_dir_all(&auto_backup_dir) {
                    eprintln!("[error] Failed to create auto backup directory: {}", e);
                    continue;
                }

                let timestamp = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_millis().to_string())
                    .unwrap_or_else(|_| "0".to_string());
                let backup_path = auto_backup_dir.join(format!("chat_auto_{}.db", timestamp));

                if let Err(e) = chat_storage.backup(&backup_path) {
                    eprintln!("[error] Hourly auto backup failed: {}", e);
                }

                // Keep the last 24 hourly backups
                if let Ok(entries) = std::fs::read_dir(&auto_backup_dir) {
                    let mut backups: Vec<_> = entries
                        .filter_map(|e| e.ok())
                        .filter(|e| e.file_name().to_string_lossy().starts_with("chat_auto_"))
                        .collect();
                    backups.sort_by_key(|e| e.path());
                    while backups.len() > 24 {
                        let old = backups.remove(0);
                        let _ = std::fs::remove_file(old.path());
                    }
                }
            }
        });
    }

    // Daily database backup: runs every 24 hours, keeps 7 days
    {
        let chat_storage = state.chat_storage.clone();
        let config_dir = app_config.config_dir.clone();
        runtime.spawn(async move {
            loop {
                tokio::time::sleep(tokio::time::Duration::from_secs(86400)).await;

                let daily_backup_dir = config_dir.join("backups").join("daily");
                if let Err(e) = std::fs::create_dir_all(&daily_backup_dir) {
                    eprintln!("[error] Failed to create daily backup directory: {}", e);
                    continue;
                }

                // Use local date (not UTC) so late-night backups match the user's calendar day.
                let date = {
                    use chrono::Datelike;
                    let local = chrono::Local::now();
                    let d = local.date_naive();
                    format!("{}{:02}{:02}", d.year(), d.month(), d.day())
                };
                let backup_path = daily_backup_dir.join(format!("chat_daily_{}.db", date));

                if let Err(e) = chat_storage.backup(&backup_path) {
                    eprintln!("[error] Daily backup failed: {}", e);
                }

                // Keep the last 7 daily backups
                if let Ok(entries) = std::fs::read_dir(&daily_backup_dir) {
                    let mut backups: Vec<_> = entries
                        .filter_map(|e| e.ok())
                        .filter(|e| e.file_name().to_string_lossy().starts_with("chat_daily_"))
                        .collect();
                    backups.sort_by_key(|e| e.path());
                    while backups.len() > 7 {
                        let old = backups.remove(0);
                        let _ = std::fs::remove_file(old.path());
                    }
                }
            }
        });
    }

    // ACME certificate renewal job (runs every 24 hours)
    {
        let state = state.clone();
        let config_dir = app_config.config_dir.clone();
        runtime.spawn(async move {
            loop {
                tokio::time::sleep(tokio::time::Duration::from_secs(86400)).await;

                let cfg = state.get_tls_config();
                if !crate::acme::should_renew(&cfg) {
                    continue;
                }

                match crate::acme::acme_renew_cert(&config_dir, &cfg) {
                    Ok(new_cfg) => {
                        eprintln!("[info] ACME renewal succeeded");
                        state.set_tls_config(new_cfg.clone());
                        if let Err(e) = crate::config::save_tls_config(&config_dir, &new_cfg) {
                            eprintln!(
                                "[error] Failed to save tls-config.json after renewal: {}",
                                e
                            );
                        }
                    }
                    Err(e) => {
                        eprintln!("[warn] ACME renewal failed: {}", e);
                    }
                }
            }
        });
    }

    // Warp server (HTTP or TLS depending on config)
    let tls_mode = tls_config.mode.clone();
    let tls_custom_cert = tls_config.custom_cert_path.clone();
    let tls_custom_key = tls_config.custom_key_path.clone();
    let tls_acme = tls_config.acme.clone();
    let tls_config_dir = app_config.config_dir.clone();
    let tls_host = host.clone();
    let tls_port = port;

    runtime.spawn(async move {
        let addr: std::net::SocketAddr = format!("{}:{}", tls_host, tls_port)
            .parse()
            .expect("Invalid host:port");

        match tls_mode {
            TlsMode::None => {
                println!(
                    "[info] Llama Monitor running on http://{}:{}",
                    tls_host, tls_port
                );
                warp::serve(routes).run(addr).await;
            }
            TlsMode::Acme => {
                // If ACME mode but no cert yet, start HTTP and log.
                if tls_acme.cert_path.is_none() || tls_acme.key_path.is_none() {
                    eprintln!(
                        "[info] ACME mode enabled but no certificate; start with TLS disabled \
                        until ACME request completes."
                    );
                    warp::serve(routes).run(addr).await;
                    return;
                }

                let cert_path = tls_acme.cert_path.clone().unwrap();
                let key_path = tls_acme.key_path.clone().unwrap();

                if !cert_path.exists() || !key_path.exists() {
                    eprintln!(
                        "[warn] ACME cert/key files not found; falling back to HTTP on http://{}:{}",
                        tls_host, tls_port
                    );
                    warp::serve(routes).run(addr).await;
                    return;
                }

                let tls_cfg = match build_tls_config(&cert_path, &key_path) {
                    Ok(cfg) => cfg,
                    Err(e) => {
                        eprintln!("[error] Failed to load ACME TLS config: {}", e);
                        eprintln!(
                            "[warn] Falling back to HTTP on http://{}:{}",
                            tls_host, tls_port
                        );
                        warp::serve(routes).run(addr).await;
                        return;
                    }
                };

                let tls_acceptor =
                    tokio_rustls::TlsAcceptor::from(std::sync::Arc::new(tls_cfg));

                let listener = match tokio::net::TcpListener::bind(addr).await {
                    Ok(l) => l,
                    Err(e) => {
                        eprintln!("[error] Failed to bind ACME TLS listener: {}", e);
                        return;
                    }
                };

                println!(
                    "[info] Llama Monitor running on https://{}:{} (ACME - {})",
                    tls_host,
                    tls_port,
                    if tls_acme.environment == "staging" {
                        "staging"
                    } else {
                        "production"
                    }
                );

                loop {
                    let (stream, _) = match listener.accept().await {
                        Ok(s) => s,
                        Err(e) => {
                            eprintln!("[error] ACME TLS accept error: {}", e);
                            continue;
                        }
                    };

                    let acceptor = tls_acceptor.clone();
                    let routes_clone = routes.clone();
                    tokio::spawn(async move {
                        let tls_stream = match acceptor.accept(stream).await {
                            Ok(s) => s,
                            Err(e) => {
                                eprintln!("[error] ACME TLS handshake error: {}", e);
                                return;
                            }
                        };

                        let svc = warp::service(routes_clone);
                        let svc = hyper_util::service::TowerToHyperService::new(svc);
                        let io = hyper_util::rt::TokioIo::new(tls_stream);

                        if let Err(e) =
                            hyper_util::server::conn::auto::Builder::new(
                                hyper_util::rt::TokioExecutor::new(),
                            )
                            .http1()
                            .serve_connection_with_upgrades(io, svc)
                            .await
                        {
                            eprintln!("[error] ACME TLS connection error: {}", e);
                        }
                    });
                }
            }
            TlsMode::SelfSigned | TlsMode::Custom => {
                // Determine cert and key paths
                let (cert_path, key_path) = if matches!(tls_mode, TlsMode::SelfSigned) {
                    let cp = tls_config_dir.join("tls-server.pem");
                    let kp = tls_config_dir.join("tls-server.key");

                    if !cp.exists() || !kp.exists() {
                        let mut sans = vec!["localhost".to_string(), "127.0.0.1".to_string()];
                        if tls_host != "0.0.0.0"
                            && tls_host != "127.0.0.1"
                            && !tls_host.starts_with('[')
                        {
                            sans.push(tls_host.clone());
                        }
                        let cert = crate::certs::generate_self_signed(sans);
                        if let Err(e) = cert.save(&cp, &kp) {
                            eprintln!("[error] Failed to write self-signed cert: {}", e);
                            eprintln!(
                                "[warn] Falling back to HTTP on http://{}:{}",
                                tls_host, tls_port
                            );
                            warp::serve(routes).run(addr).await;
                            return;
                        }
                        println!(
                            "[info] Generated self-signed TLS certificate at {}",
                            cp.display()
                        );
                    }
                    (cp, kp)
                } else {
                    match (&tls_custom_cert, &tls_custom_key) {
                        (Some(cp), Some(kp)) => (cp.clone(), kp.clone()),
                        _ => {
                            eprintln!(
                                "[warn] TLS mode=custom but cert/key not set; falling back to HTTP"
                            );
                            warp::serve(routes).run(addr).await;
                            return;
                        }
                    }
                };

                if !cert_path.exists() || !key_path.exists() {
                    eprintln!("[warn] TLS certificate or key file not found; falling back to HTTP");
                    warp::serve(routes).run(addr).await;
                    return;
                }

                let tls_config = match build_tls_config(&cert_path, &key_path) {
                    Ok(cfg) => cfg,
                    Err(e) => {
                        eprintln!("[error] Failed to load TLS config: {}", e);
                        eprintln!(
                            "[warn] Falling back to HTTP on http://{}:{}",
                            tls_host, tls_port
                        );
                        warp::serve(routes).run(addr).await;
                        return;
                    }
                };

                let tls_acceptor = tokio_rustls::TlsAcceptor::from(std::sync::Arc::new(tls_config));

                let listener = match tokio::net::TcpListener::bind(addr).await {
                    Ok(l) => l,
                    Err(e) => {
                        eprintln!("[error] Failed to bind TLS listener: {}", e);
                        return;
                    }
                };

                let tls_mode_label = if matches!(tls_mode, TlsMode::SelfSigned) {
                    "self-signed"
                } else {
                    "custom cert"
                };

                println!(
                    "[info] Llama Monitor running on https://{}:{} ({})",
                    tls_host, tls_port, tls_mode_label
                );

                // TLS server: replicate warp's Run pattern but with TLS-wrapped connections
                loop {
                    let (stream, _) = match listener.accept().await {
                        Ok(s) => s,
                        Err(e) => {
                            eprintln!("[error] TLS accept error: {}", e);
                            continue;
                        }
                    };

                    let acceptor = tls_acceptor.clone();
                    let routes_clone = routes.clone();
                    tokio::spawn(async move {
                        let tls_stream = match acceptor.accept(stream).await {
                            Ok(s) => s,
                            Err(e) => {
                                eprintln!("[error] TLS handshake error: {}", e);
                                return;
                            }
                        };

                        let svc = warp::service(routes_clone);
                        let svc = hyper_util::service::TowerToHyperService::new(svc);
                        let io = hyper_util::rt::TokioIo::new(tls_stream);

                        if let Err(e) = hyper_util::server::conn::auto::Builder::new(
                            hyper_util::rt::TokioExecutor::new(),
                        )
                        .http1()
                        .serve_connection_with_upgrades(io, svc)
                        .await
                        {
                            eprintln!("[error] TLS connection error: {}", e);
                        }
                    });
                }
            }
        }
    });

    /// Build a rustls ServerConfig from PEM cert and key files.
    fn build_tls_config(
        cert_path: &std::path::Path,
        key_path: &std::path::Path,
    ) -> Result<rustls::ServerConfig, anyhow::Error> {
        use std::fs::File;
        use std::io::BufReader;

        let mut cert_reader = BufReader::new(File::open(cert_path)?);
        let certs: Vec<rustls::pki_types::CertificateDer> = rustls_pemfile::certs(&mut cert_reader)
            .filter_map(|c| c.ok())
            .collect();
        if certs.is_empty() {
            anyhow::bail!("No certificates found in {}", cert_path.display());
        }

        let mut key_reader = BufReader::new(File::open(key_path)?);
        let key: rustls::pki_types::PrivateKeyDer = rustls_pemfile::private_key(&mut key_reader)
            .map_err(|_| anyhow::anyhow!("Failed to read private key from {}", key_path.display()))?
            .ok_or_else(|| anyhow::anyhow!("No private key found in {}", key_path.display()))?;

        let config = rustls::ServerConfig::builder()
            .with_no_client_auth()
            .with_single_cert(certs, key)?;

        Ok(config)
    }

    // Clone shutdown-related fields before state is moved to tray
    let shutdown_chat_storage = state.chat_storage.clone();
    let shutdown_sessions_path = state.sessions_path.clone();
    let shutdown_state = state.clone();

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

    // Graceful shutdown handler
    {
        let chat_storage = shutdown_chat_storage;
        let sessions_path = shutdown_sessions_path;
        let state = shutdown_state;
        runtime.spawn(async move {
            // Wait for shutdown signal (platform-specific)
            #[cfg(unix)]
            {
                let mut sigint =
                    tokio::signal::unix::signal(tokio::signal::unix::SignalKind::interrupt())
                        .unwrap();
                let mut sigterm =
                    tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
                        .unwrap();

                tokio::select! {
                    _ = sigint.recv() => {},
                    _ = sigterm.recv() => {},
                }
            }

            #[cfg(windows)]
            {
                // On Windows, Ctrl+C is handled by the console handler;
                // we also listen for Ctrl+Break via console control events.
                // For now, block until a console event triggers shutdown.
                tokio::signal::ctrl_c().await.ok();
            }

            println!("\n[info] Shutdown signal received, finalizing...");

            // Checkpoint WAL
            if let Err(e) = chat_storage.checkpoint() {
                eprintln!("[warn] Final checkpoint failed: {}", e);
            }

            // Save sessions
            if let Err(e) = state::save_sessions(&sessions_path, &state.get_sessions()) {
                eprintln!("[warn] Final session save failed: {}", e);
            }

            println!("[info] Shutdown complete");
            std::process::exit(0);
        });
    }

    // Park main thread (tray or headless)
    #[cfg(feature = "native-tray")]
    {
        park_forever();
    }

    #[cfg(not(feature = "native-tray"))]
    {
        park_forever();
    }
}

fn migrate_legacy_dashboard_auth(
    config_dir: &std::path::Path,
    auth_config_file: &std::path::Path,
    basic_auth: Option<&crate::web::auth::AuthCredentials>,
    form_auth: Option<&crate::web::auth::AuthCredentials>,
) {
    if auth_config_file.exists() {
        return;
    }

    let Some(creds) = basic_auth.or(form_auth) else {
        return;
    };

    if let (Some(basic), Some(form)) = (basic_auth, form_auth)
        && (basic.username != form.username || basic.password != form.password)
    {
        eprintln!(
            "[warn] Skipping auth-config migration because --basic-auth and --form-auth use different credentials."
        );
        return;
    }

    let Some(password_hash) = AuthManager::hash_password(&creds.password) else {
        eprintln!(
            "[warn] Failed to hash dashboard auth during migration; skipping auth-config migration."
        );
        return;
    };

    let cfg = DashboardAuthConfig {
        basic_enabled: basic_auth.is_some(),
        form_enabled: form_auth.is_some(),
        username: creds.username.clone(),
        password_hash,
    };

    if let Err(err) = save_auth_config(config_dir, &cfg) {
        eprintln!("[warn] Failed to migrate dashboard auth into auth-config.json: {err}");
    } else {
        eprintln!("[config] Migrated dashboard auth into auth-config.json for future builds.");
    }
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
                form_auth: None,
                clear_auth_config: false,
                agent_port: 7779,
                agent_token: None,
                remote_agent_url: None,
                remote_agent_token: None,
                remote_agent_ssh_autostart: false,
                remote_agent_ssh_target: None,
                remote_agent_ssh_command: None,
                tls: false,
                tls_cert: None,
                tls_key: None,
                tls_self_signed: false,
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
            form_auth: None,
            clear_auth_config: false,
            agent_port: 7779,
            agent_token: None,
            remote_agent_url: None,
            remote_agent_token: None,
            remote_agent_ssh_autostart: false,
            remote_agent_ssh_target: None,
            remote_agent_ssh_command: None,
            tls: false,
            tls_cert: None,
            tls_key: None,
            tls_self_signed: false,
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
            form_auth: None,
            clear_auth_config: false,
            agent_port: 7779,
            agent_token: None,
            remote_agent_url: None,
            remote_agent_token: None,
            remote_agent_ssh_autostart: false,
            remote_agent_ssh_target: None,
            remote_agent_ssh_command: None,
            tls: false,
            tls_cert: None,
            tls_key: None,
            tls_self_signed: false,
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
            form_auth: None,
            clear_auth_config: false,
            agent_port: 7779,
            agent_token: None,
            remote_agent_url: None,
            remote_agent_token: None,
            remote_agent_ssh_autostart: false,
            remote_agent_ssh_target: None,
            remote_agent_ssh_command: None,
            tls: false,
            tls_cert: None,
            tls_key: None,
            tls_self_signed: false,
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
            form_auth: None,
            clear_auth_config: false,
            agent_port: 7779,
            agent_token: None,
            remote_agent_url: None,
            remote_agent_token: None,
            remote_agent_ssh_autostart: false,
            remote_agent_ssh_target: None,
            remote_agent_ssh_command: None,
            tls: false,
            tls_cert: None,
            tls_key: None,
            tls_self_signed: false,
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
            form_auth: None,
            clear_auth_config: false,
            agent_port: 7779,
            agent_token: None,
            remote_agent_url: None,
            remote_agent_token: None,
            remote_agent_ssh_autostart: false,
            remote_agent_ssh_target: None,
            remote_agent_ssh_command: None,
            tls: false,
            tls_cert: None,
            tls_key: None,
            tls_self_signed: false,
        };
        assert!(!should_start_tray(&args));
    }
}
