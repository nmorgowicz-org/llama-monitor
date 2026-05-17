use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use crate::cli::AppArgs;

/// TLS operating mode.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum TlsMode {
    #[default]
    None,
    SelfSigned,
    Custom,
    Acme,
}

/// ACME-specific configuration (Let's Encrypt / lego).
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct AcmeConfig {
    pub enabled: bool,
    pub fqdn: String,
    pub environment: String,
    pub dns_provider: String,
    pub dns_config: HashMap<String, String>,
    pub validation_delay: u64,
    pub last_renewal: Option<String>,
    pub cert_path: Option<PathBuf>,
    pub key_path: Option<PathBuf>,
}

impl AcmeConfig {
    pub fn is_valid(&self) -> bool {
        self.enabled
            && !self.fqdn.is_empty()
            && (self.environment == "staging" || self.environment == "production")
            && !self.dns_provider.is_empty()
            && !self.dns_config.is_empty()
    }
}

/// TLS configuration persisted to tls-config.json.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct TLSConfig {
    pub mode: TlsMode,
    pub custom_cert_path: Option<PathBuf>,
    pub custom_key_path: Option<PathBuf>,
    pub acme: AcmeConfig,
}

impl Default for TLSConfig {
    fn default() -> Self {
        Self {
            mode: TlsMode::None,
            custom_cert_path: None,
            custom_key_path: None,
            acme: AcmeConfig::default(),
        }
    }
}

/// Sanitize a TLSConfig so that an Acme mode with missing/invalid fields
/// falls back gracefully instead of panicking or blocking startup.
pub fn sanitize_tls_config(cfg: TLSConfig) -> TLSConfig {
    let mut cfg = cfg;
    if cfg.mode == TlsMode::Acme && !cfg.acme.is_valid() {
        cfg.mode = TlsMode::None;
    }
    cfg
}

/// Load TLSConfig from tls-config.json; on any error, return default.
pub fn load_tls_config(config_dir: &std::path::Path) -> TLSConfig {
    let path = config_dir.join("tls-config.json");
    if !path.exists() {
        return TLSConfig::default();
    }
    let Ok(contents) = fs::read_to_string(&path) else {
        eprintln!("[warn] Failed to read tls-config.json, using defaults");
        return TLSConfig::default();
    };
    let Ok(cfg) = serde_json::from_str::<TLSConfig>(&contents) else {
        eprintln!("[warn] Invalid tls-config.json, using defaults");
        return TLSConfig::default();
    };
    sanitize_tls_config(cfg)
}

/// Persist TLSConfig to tls-config.json (atomic write).
pub fn save_tls_config(config_dir: &std::path::Path, cfg: &TLSConfig) -> std::io::Result<()> {
    let path = config_dir.join("tls-config.json");
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension("json.tmp");
    let json = serde_json::to_string_pretty(cfg)?;
    fs::write(&tmp, json)?;
    fs::rename(&tmp, &path)?;
    Ok(())
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct AppConfig {
    pub config_dir: PathBuf,
    pub llama_server_path: PathBuf,
    pub llama_server_cwd: PathBuf,
    pub port: u16,
    pub gpu_backend: String,
    pub llama_poll_interval: u64,
    pub models_dir: Option<PathBuf>,
    pub presets_file: PathBuf,
    pub templates_file: PathBuf,
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
    pub db_admin_token: Option<String>,
    pub api_token: Option<String>,
    pub tls_config: TLSConfig,
}

impl AppConfig {
    pub fn from_args(args: AppArgs) -> Self {
        let default_server_path = PathBuf::from("llama-server");
        let default_server_cwd = PathBuf::from(".");

        let config_dir = args.config_dir.unwrap_or_else(|| {
            let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
            home.join(".config").join("llama-monitor")
        });

        let presets_file = args
            .presets_file
            .unwrap_or_else(|| config_dir.join("presets.json"));

        Self {
            config_dir: config_dir.clone(),
            llama_server_path: args.llama_server_path.unwrap_or(default_server_path),
            llama_server_cwd: args.llama_server_cwd.unwrap_or(default_server_cwd),
            port: args.port,
            gpu_backend: args.gpu_backend,
            models_dir: args.models_dir,
            presets_file,
            templates_file: config_dir.join("templates.json"),
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
            db_admin_token: ensure_db_admin_token(&config_dir),
            api_token: ensure_api_token(&config_dir),
            tls_config: load_tls_config(&config_dir),
        }
    }
}

fn ensure_db_admin_token(config_dir: &PathBuf) -> Option<String> {
    let token_file = config_dir.join("db-admin-token");

    // Try to read existing token
    if let Ok(content) = fs::read_to_string(&token_file) {
        let trimmed = content.trim().to_string();
        if !trimmed.is_empty() {
            return Some(trimmed);
        }
    }

    // Generate new token
    let token = generate_random_token();
    let _ = fs::create_dir_all(config_dir);
    if fs::write(&token_file, &token).is_ok() {
        eprintln!("[config] Generated db-admin-token");
    }
    Some(token)
}

fn ensure_api_token(config_dir: &PathBuf) -> Option<String> {
    let token_file = config_dir.join("api-token");

    if let Ok(content) = fs::read_to_string(&token_file) {
        let trimmed = content.trim().to_string();
        if !trimmed.is_empty() {
            return Some(trimmed);
        }
    }

    let token = generate_random_token();
    let _ = fs::create_dir_all(config_dir);
    if fs::write(&token_file, &token).is_ok() {
        eprintln!("[config] Generated api-token");
    }
    Some(token)
}

fn generate_random_token() -> String {
    // Read exactly 16 bytes from /dev/urandom (avoid reading all bytes)
    let mut buf = [0u8; 16];
    let value = if let Ok(mut f) = std::fs::File::open("/dev/urandom") {
        if std::io::Read::read_exact(&mut f, &mut buf).is_ok() {
            u128::from_be_bytes(buf)
        } else {
            let ts = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos();
            let pid = std::process::id() as u128;
            ts ^ pid
        }
    } else {
        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let pid = std::process::id() as u128;
        ts ^ pid
    };
    format!("{value:x}")
}
