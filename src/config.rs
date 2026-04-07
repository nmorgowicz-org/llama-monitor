use std::path::PathBuf;

use crate::cli::AppArgs;

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct AppConfig {
    pub llama_server_path: PathBuf,
    pub llama_server_cwd: PathBuf,
    pub port: u16,
    pub gpu_backend: String,
    pub models_dir: Option<PathBuf>,
    pub presets_file: PathBuf,
    pub gpu_env_file: PathBuf,
    pub gpu_arch_override: Option<String>,
    pub gpu_devices_override: Option<String>,
    pub ui_settings_file: PathBuf,
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
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let args = AppArgs {
            llama_server_path: None,
            llama_server_cwd: None,
            port: 7778,
            models_dir: None,
            presets_file: None,
            gpu_backend: "auto".into(),
            gpu_arch: None,
            gpu_devices: None,
        };
        let config = AppConfig::from_args(args);
        assert_eq!(config.port, 7778);
        assert_eq!(config.gpu_backend, "auto");
        assert!(
            config
                .presets_file
                .to_str()
                .unwrap()
                .contains("llama-monitor")
        );
        assert!(config.gpu_env_file.to_str().unwrap().contains("gpu-env"));
        assert!(config.ui_settings_file.to_str().unwrap().contains("ui-settings"));
    }

    #[test]
    fn test_config_with_overrides() {
        let args = AppArgs {
            llama_server_path: Some(PathBuf::from("/usr/bin/llama-server")),
            llama_server_cwd: Some(PathBuf::from("/tmp")),
            port: 9999,
            models_dir: Some(PathBuf::from("/models")),
            presets_file: Some(PathBuf::from("/custom/presets.json")),
            gpu_backend: "nvidia".into(),
            gpu_arch: Some("gfx1100".into()),
            gpu_devices: Some("0,1".into()),
        };
        let config = AppConfig::from_args(args);
        assert_eq!(
            config.llama_server_path,
            PathBuf::from("/usr/bin/llama-server")
        );
        assert_eq!(config.port, 9999);
        assert_eq!(config.gpu_arch_override, Some("gfx1100".into()));
        assert_eq!(config.gpu_devices_override, Some("0,1".into()));
    }
}
