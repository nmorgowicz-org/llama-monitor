use std::path::PathBuf;

use crate::cli::AppArgs;

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct AppConfig {
    pub llama_server_path: PathBuf,
    pub llama_server_cwd: PathBuf,
    pub port: u16,
    pub gpu_backend: String,
    pub llama_poll_interval: u64,
    pub models_dir: Option<PathBuf>,
    pub presets_file: PathBuf,
    pub gpu_env_file: PathBuf,
    pub gpu_arch_override: Option<String>,
    pub gpu_devices_override: Option<String>,
    pub ui_settings_file: PathBuf,
    pub sessions_file: PathBuf,
    pub ssh_known_hosts_file: PathBuf,
    pub lhm_disabled_file: PathBuf,
    pub agent_host: String,
    pub agent_port: u16,
    pub agent_token: Option<String>,
    pub remote_agent_url: Option<String>,
    pub remote_agent_token: Option<String>,
    pub remote_agent_ssh_autostart: bool,
    pub remote_agent_ssh_target: Option<String>,
    pub remote_agent_ssh_command: Option<String>,
}

impl AppConfig {
    pub fn from_args(args: AppArgs) -> Self {
        let default_server_path = PathBuf::from("llama-server");
        let default_server_cwd = PathBuf::from(".");

        let config_dir = dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("llama-monitor");

        let presets_file = args
            .presets_file
            .unwrap_or_else(|| config_dir.join("presets.json"));

        Self {
            llama_server_path: args.llama_server_path.unwrap_or(default_server_path),
            llama_server_cwd: args.llama_server_cwd.unwrap_or(default_server_cwd),
            port: args.port,
            gpu_backend: args.gpu_backend,
            models_dir: args.models_dir,
            presets_file,
            gpu_env_file: config_dir.join("gpu-env.json"),
            gpu_arch_override: args.gpu_arch,
            gpu_devices_override: args.gpu_devices,
            ui_settings_file: config_dir.join("ui-settings.json"),
            sessions_file: args
                .sessions_file
                .unwrap_or_else(|| config_dir.join("sessions.json")),
            ssh_known_hosts_file: config_dir.join("ssh-known-hosts.json"),
            llama_poll_interval: args.llama_poll_interval,
            lhm_disabled_file: config_dir.join("lhm-disabled.json"),
            agent_host: args.agent_host,
            agent_port: args.agent_port,
            agent_token: args.agent_token,
            remote_agent_url: args.remote_agent_url,
            remote_agent_token: args.remote_agent_token,
            remote_agent_ssh_autostart: args.remote_agent_ssh_autostart,
            remote_agent_ssh_target: args.remote_agent_ssh_target,
            remote_agent_ssh_command: args.remote_agent_ssh_command,
        }
    }
}
