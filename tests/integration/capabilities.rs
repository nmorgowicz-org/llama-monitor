use llama_monitor::{
    gpu::env::GpuEnv,
    state::{AppPaths, AppState, Session, UiSettings},
};

#[test]
fn local_spawn_has_all_metrics() {
    let paths = AppPaths {
        presets_path: std::path::PathBuf::new(),
        models_dir: None,
        gpu_env_path: std::path::PathBuf::new(),
        ui_settings_path: std::path::PathBuf::new(),
        sessions_path: std::path::PathBuf::new(),
    };
    let gpu_env = GpuEnv {
        arch: "auto".into(),
        devices: String::new(),
        rocm_path: "/opt/rocm".into(),
        extra_env: vec![],
    };
    let state = AppState::new(vec![], paths, gpu_env, UiSettings::default());

    let session = Session::new_spawn(
        "spawn_test".to_string(),
        "Spawn Test".to_string(),
        8001,
        String::new(),
    );
    state.add_session(session);
    state.set_active_session("spawn_test");

    let caps = state.calculate_capabilities();

    assert!(
        caps.inference,
        "inference should be available in spawn mode"
    );
    assert!(
        caps.system,
        "system metrics should be available in spawn mode"
    );
    assert!(caps.gpu, "gpu metrics should be available in spawn mode");
    assert!(
        caps.cpu_temperature,
        "cpu_temperature should be available in spawn mode"
    );
    assert!(caps.memory, "memory should be available in spawn mode");
    assert!(
        caps.host_metrics,
        "host_metrics should be available in spawn mode"
    );
    assert!(caps.tray, "tray should be available in spawn mode");
}

#[test]
fn local_attach_has_inference_only() {
    let paths = AppPaths {
        presets_path: std::path::PathBuf::new(),
        models_dir: None,
        gpu_env_path: std::path::PathBuf::new(),
        ui_settings_path: std::path::PathBuf::new(),
        sessions_path: std::path::PathBuf::new(),
    };
    let gpu_env = GpuEnv {
        arch: "auto".into(),
        devices: String::new(),
        rocm_path: "/opt/rocm".into(),
        extra_env: vec![],
    };
    let state = AppState::new(vec![], paths, gpu_env, UiSettings::default());

    let session = Session::new_attach(
        "attach_local_test".to_string(),
        "Attach Local Test".to_string(),
        "http://127.0.0.1:8001".to_string(),
    );
    state.add_session(session);
    state.set_active_session("attach_local_test");

    let caps = state.calculate_capabilities();

    assert!(
        caps.inference,
        "inference should be available in attach mode"
    );
    assert!(
        !caps.system,
        "system metrics should not be available in attach mode"
    );
    assert!(
        !caps.gpu,
        "gpu metrics should not be available in attach mode"
    );
    assert!(
        !caps.cpu_temperature,
        "cpu_temperature should not be available in attach mode"
    );
    assert!(
        !caps.memory,
        "memory should not be available in attach mode"
    );
    assert!(
        !caps.host_metrics,
        "host_metrics should not be available in attach mode"
    );
    assert!(caps.tray, "tray should be available in attach mode");
}

#[test]
fn remote_attach_has_inference_only() {
    let paths = AppPaths {
        presets_path: std::path::PathBuf::new(),
        models_dir: None,
        gpu_env_path: std::path::PathBuf::new(),
        ui_settings_path: std::path::PathBuf::new(),
        sessions_path: std::path::PathBuf::new(),
    };
    let gpu_env = GpuEnv {
        arch: "auto".into(),
        devices: String::new(),
        rocm_path: "/opt/rocm".into(),
        extra_env: vec![],
    };
    let state = AppState::new(vec![], paths, gpu_env, UiSettings::default());

    let session = Session::new_attach(
        "attach_remote_test".to_string(),
        "Attach Remote Test".to_string(),
        "http://203.0.113.10:8001".to_string(),
    );
    state.add_session(session);
    state.set_active_session("attach_remote_test");

    let caps = state.calculate_capabilities();

    assert!(
        caps.inference,
        "inference should be available in remote attach mode"
    );
    assert!(
        !caps.system,
        "system metrics should not be available in remote attach mode"
    );
    assert!(
        !caps.gpu,
        "gpu metrics should not be available in remote attach mode"
    );
    assert!(
        !caps.cpu_temperature,
        "cpu_temperature should not be available in remote attach mode"
    );
    assert!(
        !caps.memory,
        "memory should not be available in remote attach mode"
    );
    assert!(
        !caps.host_metrics,
        "host_metrics should not be available in remote attach mode"
    );
    assert!(caps.tray, "tray should be available in remote attach mode");
}

#[test]
fn headless_mode_disables_tray() {
    use llama_monitor::cli::AppArgs;

    let should_start_tray_logic = |args: &AppArgs| -> bool {
        if args.headless || args.no_tray {
            return false;
        }
        #[cfg(target_os = "linux")]
        {
            std::env::var_os("DISPLAY").is_some() || std::env::var_os("WAYLAND_DISPLAY").is_some()
        }
        #[cfg(not(target_os = "linux"))]
        {
            true
        }
    };

    let args = AppArgs {
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
        headless: true,
        no_tray: false,
        agent: false,
        agent_host: "127.0.0.1".to_string(),
        agent_port: 7779,
        agent_token: None,
        remote_agent_url: None,
        remote_agent_token: None,
        remote_agent_ssh_autostart: false,
        remote_agent_ssh_target: None,
        remote_agent_ssh_command: None,
    };

    assert!(
        !should_start_tray_logic(&args),
        "tray should be disabled in headless mode"
    );

    let args_no_tray = AppArgs {
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
        headless: false,
        no_tray: true,
        agent: false,
        agent_host: "127.0.0.1".to_string(),
        agent_port: 7779,
        agent_token: None,
        remote_agent_url: None,
        remote_agent_token: None,
        remote_agent_ssh_autostart: false,
        remote_agent_ssh_target: None,
        remote_agent_ssh_command: None,
    };

    assert!(
        !should_start_tray_logic(&args_no_tray),
        "tray should be disabled with --no-tray"
    );

    let args_both = AppArgs {
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
        headless: true,
        no_tray: true,
        agent: false,
        agent_host: "127.0.0.1".to_string(),
        agent_port: 7779,
        agent_token: None,
        remote_agent_url: None,
        remote_agent_token: None,
        remote_agent_ssh_autostart: false,
        remote_agent_ssh_target: None,
        remote_agent_ssh_command: None,
    };

    assert!(
        !should_start_tray_logic(&args_both),
        "tray should be disabled with both flags"
    );
}
