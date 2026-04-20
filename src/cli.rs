use clap::Parser;
use std::path::PathBuf;

#[derive(Parser, Debug, Clone)]
#[command(
    name = "llama-monitor",
    about = "Web dashboard for llama.cpp server management and GPU monitoring"
)]
pub struct AppArgs {
    /// Path to the llama-server binary
    #[arg(short = 's', long)]
    pub llama_server_path: Option<PathBuf>,

    /// Working directory for llama-server
    #[arg(long)]
    pub llama_server_cwd: Option<PathBuf>,

    /// Port for the monitor web UI
    #[arg(short, long, default_value_t = 7778)]
    pub port: u16,

    /// Directory containing .gguf model files for auto-discovery
    #[arg(short = 'm', long)]
    pub models_dir: Option<PathBuf>,

    /// Path to presets JSON file
    #[arg(long)]
    pub presets_file: Option<PathBuf>,

    /// Path to sessions JSON file
    #[arg(long)]
    pub sessions_file: Option<PathBuf>,

    /// GPU monitoring backend: auto, rocm, nvidia, none
    #[arg(long, default_value = "auto")]
    pub gpu_backend: String,

    /// GPU architecture for ROCm env (e.g. gfx906, gfx1100, auto)
    #[arg(long)]
    pub gpu_arch: Option<String>,

    /// Visible GPU device indices (e.g. 0,1,2,3)
    #[arg(long)]
    pub gpu_devices: Option<String>,

    /// Llama metrics polling interval in seconds (default: 1)
    #[arg(long, default_value_t = 1)]
    pub llama_poll_interval: u64,

    /// Run in headless mode (no tray, no desktop UI)
    #[arg(long)]
    pub headless: bool,

    /// Disable tray icon (override automatic detection)
    #[arg(long)]
    pub no_tray: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_headless_flag_parsing() {
        let args = AppArgs::parse_from(["llama-monitor", "--headless"]);
        assert!(args.headless);
        assert!(!args.no_tray);
    }

    #[test]
    fn test_no_tray_flag_parsing() {
        let args = AppArgs::parse_from(["llama-monitor", "--no-tray"]);
        assert!(args.no_tray);
        assert!(!args.headless);
    }

    #[test]
    fn test_combined_flags() {
        let args = AppArgs::parse_from(["llama-monitor", "--headless", "--no-tray"]);
        assert!(args.headless);
        assert!(args.no_tray);
    }

    #[test]
    fn test_default_values() {
        let args = AppArgs::parse_from(["llama-monitor"]);
        assert!(!args.headless);
        assert!(!args.no_tray);
        assert_eq!(args.port, 7778);
        assert_eq!(args.gpu_backend, "auto");
    }
}
