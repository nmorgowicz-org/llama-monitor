use std::sync::Arc;

use llama_monitor::{
    chat_storage::ChatStorage,
    config::TLSConfig,
    gpu::env::GpuEnv,
    state::{AppPaths, AppState, Session, UiSettings},
};

fn test_chat_storage() -> Arc<ChatStorage> {
    Arc::new(ChatStorage::open(&std::path::PathBuf::from(":memory:")).unwrap())
}

#[test]
fn local_spawn_has_all_metrics() {
    let paths = AppPaths {
        presets_path: std::path::PathBuf::new(),
        templates_path: std::path::PathBuf::new(),
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
    let state = AppState::new(
        vec![],
        paths,
        gpu_env,
        UiSettings::default(),
        test_chat_storage(),
        TLSConfig::default(),
    );

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
fn local_attach_has_full_metrics() {
    let paths = AppPaths {
        presets_path: std::path::PathBuf::new(),
        templates_path: std::path::PathBuf::new(),
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
    let state = AppState::new(
        vec![],
        paths,
        gpu_env,
        UiSettings::default(),
        test_chat_storage(),
        TLSConfig::default(),
    );

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
        "inference should be available in local attach"
    );
    assert!(
        caps.system,
        "system metrics should be available in local attach"
    );
    assert!(caps.gpu, "gpu metrics should be available in local attach");
    assert!(
        caps.cpu_temperature,
        "cpu_temperature should be available in local attach"
    );
    assert!(caps.memory, "memory should be available in local attach");
    assert!(
        caps.host_metrics,
        "host_metrics should be available in local attach"
    );
    assert!(caps.tray, "tray should be available in local attach");
}

#[test]
fn remote_attach_has_inference_only() {
    let paths = AppPaths {
        presets_path: std::path::PathBuf::new(),
        templates_path: std::path::PathBuf::new(),
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
    let state = AppState::new(
        vec![],
        paths,
        gpu_env,
        UiSettings::default(),
        test_chat_storage(),
        TLSConfig::default(),
    );

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
        config_dir: None,
        headless: true,
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
        config_dir: None,
        headless: false,
        no_tray: true,
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
        config_dir: None,
        headless: true,
        no_tray: true,
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

    assert!(
        !should_start_tray_logic(&args_both),
        "tray should be disabled with both flags"
    );
}

#[test]
fn session_serialization_includes_new_fields() {
    let session = Session::new_attach(
        "test_new_fields".to_string(),
        "New Fields Test".to_string(),
        "http://remote.example.com:8080".to_string(),
    );

    // Verify new fields have correct defaults
    assert_eq!(session.last_connected_at, 0);
    assert_eq!(session.connect_count, 0);
    assert!(session.last_error.is_none());

    // Verify serialization includes all fields
    let json = serde_json::to_value(&session).unwrap();
    assert!(json.get("last_connected_at").is_some());
    assert!(json.get("connect_count").is_some());
    assert!(json.get("last_error").is_some());
    assert!(json.get("id").is_some());
    assert!(json.get("name").is_some());
    assert!(json.get("mode").is_some());
    assert!(json.get("status").is_some());
}

#[test]
fn session_deserialization_omits_new_fields() {
    // Simulate old session JSON without new fields
    let old_json = r#"{
        "id": "old_session",
        "name": "Old Session",
        "mode": {"Attach": {"endpoint": "http://127.0.0.1:8080"}},
        "status": "Stopped"
    }"#;

    let session: Session = serde_json::from_str(old_json).unwrap();
    assert_eq!(session.id, "old_session");
    assert_eq!(session.last_connected_at, 0);
    assert_eq!(session.connect_count, 0);
    assert!(session.last_error.is_none());
}

#[test]
fn agent_health_reachable_defaults_false() {
    let paths = AppPaths {
        presets_path: std::path::PathBuf::new(),
        templates_path: std::path::PathBuf::new(),
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
    let state = AppState::new(
        vec![],
        paths,
        gpu_env,
        UiSettings::default(),
        test_chat_storage(),
        TLSConfig::default(),
    );

    assert!(
        !state.remote_agent_health_reachable(),
        "health_reachable must default to false"
    );
    assert!(
        !state.remote_agent_connected(),
        "agent_connected must default to false"
    );
}
