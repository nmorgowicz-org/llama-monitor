use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use aes_gcm::{
    Aes256Gcm,
    aead::{Aead, KeyInit},
};
use generic_array::GenericArray;
use rand_core::{OsRng, RngCore};

use crate::cli::AppArgs;

/// On Unix, restrict file permissions to owner-only (0600).
/// On other platforms, no-op.
pub(crate) fn harden_file_permissions(path: &std::path::Path) {
    if !path.exists() {
        return;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Ok(mut perms) = std::fs::metadata(path).map(|m| m.permissions()) {
            perms.set_mode(0o600);
            let _ = std::fs::set_permissions(path, perms);
        }
    }
    #[cfg(not(unix))]
    {
        let _ = path;
    }
}

const ENCRYPTED_PREFIX: &str = "enc:";

use once_cell::sync::OnceCell;

static ENCRYPTION_KEY_CELL: OnceCell<[u8; 32]> = OnceCell::new();

/// Derive a 256-bit key from a secret using HKDF-SHA-256.
fn derive_key(secret: &[u8]) -> [u8; 32] {
    use hkdf::Hkdf;
    use sha2::Sha256;
    let hk = Hkdf::<Sha256>::new(None, secret);
    let mut key = [0u8; 32];
    hk.expand(b"llama-monitor-encryption-key", &mut key)
        .expect("valid HKDF output length");
    key
}

/// Initialize the encryption key at startup.
///
/// Priority:
/// 1) LLAMA_MONITOR_ENCRYPTION_KEY (if set and non-empty).
/// 2) Auto-generated key stored in config_dir/encryption-key.
///
/// This ensures encryption is always enabled and fully automatic.
pub fn init_encryption_key(config_dir: &std::path::Path) {
    if ENCRYPTION_KEY_CELL.get().is_some() {
        return;
    }

    // 1) Use env var if provided
    if let Ok(secret) = std::env::var("LLAMA_MONITOR_ENCRYPTION_KEY")
        && !secret.is_empty()
        && secret.len() >= 16
    {
        let key = derive_key(secret.as_bytes());
        let _ = ENCRYPTION_KEY_CELL.set(key);
        eprintln!("[info] Using LLAMA_MONITOR_ENCRYPTION_KEY for at-rest encryption.");
        return;
    }

    let key_file = config_dir.join("encryption-key");

    // 2) Try to load existing auto-generated key
    if key_file.exists()
        && let Ok(raw) = std::fs::read(&key_file)
        && raw.len() == 32
    {
        let mut key = [0u8; 32];
        key.copy_from_slice(&raw);
        let _ = ENCRYPTION_KEY_CELL.set(key);
        eprintln!("[info] Loaded auto-generated encryption key from {key_file:?}.");
        return;
    }

    // 3) Generate and persist a new key
    let mut key = [0u8; 32];
    OsRng.fill_bytes(&mut key);
    let _ = std::fs::create_dir_all(config_dir);
    if std::fs::write(&key_file, key).is_ok() {
        harden_file_permissions(&key_file);
        eprintln!("[info] Generated and saved encryption key to {key_file:?}.");
    } else {
        eprintln!(
            "[warn] Failed to write encryption key to {key_file:?}; \
             continuing with in-memory key only."
        );
    }
    let _ = ENCRYPTION_KEY_CELL.set(key);
}

/// Get the active encryption key, or None if initialization failed.
fn encryption_key() -> Option<[u8; 32]> {
    ENCRYPTION_KEY_CELL.get().copied()
}

/// Generate a random 12-byte nonce.
fn random_nonce() -> [u8; 12] {
    let mut buf = [0u8; 12];
    OsRng.fill_bytes(&mut buf);
    buf
}

/// Encrypt a plaintext value using AES-256-GCM if a key is configured.
/// Returns "enc:<base64(nonce || ciphertext)>" on success, or the original plaintext if no key.
pub(crate) fn encrypt_value(plaintext: &str) -> String {
    let key_bytes = match encryption_key() {
        Some(k) => k,
        None => return plaintext.to_string(),
    };

    let key = GenericArray::<u8, _>::from(key_bytes);
    let cipher = Aes256Gcm::new(&key);
    let nonce = aes_gcm::Nonce::clone_from_slice(&random_nonce());

    let ct = match cipher.encrypt(&nonce, plaintext.as_ref()) {
        Ok(c) => c,
        Err(_) => return plaintext.to_string(),
    };

    // Prepend nonce to ciphertext so we can recover it during decryption.
    let mut payload = Vec::with_capacity(12 + ct.len());
    payload.extend_from_slice(&nonce);
    payload.extend_from_slice(&ct);

    let b64 = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &payload);
    format!("{ENCRYPTED_PREFIX}{b64}")
}

/// Decrypt a value if it appears encrypted.
/// If it starts with "enc:", attempts AES-256-GCM decryption using the active key.
/// If no key or decryption fails, logs a warning and falls back to the original value.
pub(crate) fn decrypt_value(ciphertext: &str) -> String {
    if !ciphertext.starts_with(ENCRYPTED_PREFIX) {
        return ciphertext.to_string();
    }

    let key_bytes = match encryption_key() {
        Some(k) => k,
        None => {
            eprintln!(
                "[warn] Decryption requested but no encryption key available; \
                returning raw encrypted blob as-is."
            );
            return ciphertext.to_string();
        }
    };

    let b64_part = &ciphertext[ENCRYPTED_PREFIX.len()..];
    let payload = match base64::Engine::decode(&base64::engine::general_purpose::STANDARD, b64_part)
    {
        Ok(v) => v,
        Err(_) => {
            eprintln!("[warn] Failed to decode encrypted value (bad base64)");
            return ciphertext.to_string();
        }
    };

    if payload.len() < 12 {
        eprintln!("[warn] Encrypted value too short to contain nonce");
        return ciphertext.to_string();
    }

    let (nonce_bytes, ct_bytes) = payload.split_at(12);
    let nonce = aes_gcm::Nonce::clone_from_slice(nonce_bytes);
    let key = GenericArray::<u8, _>::from(key_bytes);
    let cipher = Aes256Gcm::new(&key);

    let pt = match cipher.decrypt(&nonce, ct_bytes) {
        Ok(pt) => pt,
        Err(_) => {
            eprintln!("[warn] Decryption failed (bad key or corrupted data); returning raw value");
            return ciphertext.to_string();
        }
    };

    match String::from_utf8(pt) {
        Ok(s) => s,
        Err(_) => {
            eprintln!("[warn] Decryption produced invalid UTF-8; returning raw value");
            ciphertext.to_string()
        }
    }
}

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
    pub email: String,
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

/// Persisted dashboard auth configuration stored separately from UI settings.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct DashboardAuthConfig {
    pub basic_enabled: bool,
    pub form_enabled: bool,
    pub username: String,
    pub password_hash: String,
}

impl DashboardAuthConfig {
    pub fn is_usable(&self) -> bool {
        (self.basic_enabled || self.form_enabled)
            && !self.username.trim().is_empty()
            && !self.password_hash.trim().is_empty()
    }
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
    let Ok(mut cfg) = serde_json::from_str::<TLSConfig>(&contents) else {
        eprintln!("[warn] Invalid tls-config.json, using defaults");
        return TLSConfig::default();
    };

    // Decrypt ACME dns_config values
    cfg.acme.dns_config = cfg
        .acme
        .dns_config
        .into_iter()
        .map(|(k, v)| (k, decrypt_value(&v)))
        .collect();

    sanitize_tls_config(cfg)
}

/// Persist TLSConfig to tls-config.json (atomic write).
pub fn save_tls_config(config_dir: &std::path::Path, cfg: &TLSConfig) -> std::io::Result<()> {
    let path = config_dir.join("tls-config.json");
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    // Encrypt ACME dns_config values before writing
    let mut to_save = cfg.clone();
    to_save.acme.dns_config = to_save
        .acme
        .dns_config
        .into_iter()
        .map(|(k, v)| (k, encrypt_value(&v)))
        .collect();

    let tmp = path.with_extension("json.tmp");
    let json = serde_json::to_string_pretty(&to_save)?;
    fs::write(&tmp, json)?;
    fs::rename(&tmp, &path)?;
    harden_file_permissions(&path);
    Ok(())
}

pub fn load_auth_config(config_dir: &std::path::Path) -> DashboardAuthConfig {
    let path = config_dir.join("auth-config.json");
    if !path.exists() {
        return DashboardAuthConfig::default();
    }
    let Ok(contents) = fs::read_to_string(&path) else {
        eprintln!("[warn] Failed to read auth-config.json, using defaults");
        return DashboardAuthConfig::default();
    };
    let Ok(cfg) = serde_json::from_str::<DashboardAuthConfig>(&contents) else {
        eprintln!("[warn] Invalid auth-config.json, using defaults");
        return DashboardAuthConfig::default();
    };
    cfg
}

pub fn save_auth_config(
    config_dir: &std::path::Path,
    cfg: &DashboardAuthConfig,
) -> std::io::Result<()> {
    let path = config_dir.join("auth-config.json");
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    let tmp = path.with_extension("json.tmp");
    let json = serde_json::to_string_pretty(cfg)?;
    fs::write(&tmp, json)?;
    fs::rename(&tmp, &path)?;
    harden_file_permissions(&path);
    Ok(())
}

pub fn clear_auth_config(config_dir: &std::path::Path) -> std::io::Result<bool> {
    let path = config_dir.join("auth-config.json");
    if !path.exists() {
        return Ok(false);
    }
    fs::remove_file(path)?;
    Ok(true)
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
    pub auth_config_file: PathBuf,
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
    pub tls_config: TLSConfig,
    // Live token stores — updated in-memory on rotation without requiring a restart.
    // Arc<RwLock> so all Arc<AppConfig> clones share the same backing store.
    live_api_token_store: std::sync::Arc<std::sync::RwLock<Option<String>>>,
    live_db_admin_token_store: std::sync::Arc<std::sync::RwLock<Option<String>>>,
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
            auth_config_file: config_dir.join("auth-config.json"),
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
            tls_config: load_tls_config(&config_dir),
            live_api_token_store: std::sync::Arc::new(std::sync::RwLock::new(ensure_api_token(
                &config_dir,
            ))),
            live_db_admin_token_store: std::sync::Arc::new(std::sync::RwLock::new(
                ensure_db_admin_token(&config_dir),
            )),
        }
    }

    /// Read the current live API token (updated on rotation).
    pub fn live_api_token(&self) -> Option<String> {
        self.live_api_token_store.read().unwrap().clone()
    }

    /// Read the current live DB admin token (updated on rotation).
    pub fn live_db_admin_token(&self) -> Option<String> {
        self.live_db_admin_token_store.read().unwrap().clone()
    }

    /// Update the live API token after rotation.
    pub fn update_live_api_token(&self, token: String) {
        *self.live_api_token_store.write().unwrap() = Some(token);
    }

    /// Update the live DB admin token after rotation.
    pub fn update_live_db_admin_token(&self, token: String) {
        *self.live_db_admin_token_store.write().unwrap() = Some(token);
    }

    /// Construct an `AppConfig` for unit tests without reading from disk.
    #[cfg(test)]
    pub fn for_test(api_token: Option<String>, db_admin_token: Option<String>) -> Self {
        Self {
            config_dir: std::path::PathBuf::from("/tmp/llama-monitor-test"),
            llama_server_path: std::path::PathBuf::from("llama-server"),
            llama_server_cwd: std::path::PathBuf::from("."),
            port: 8001,
            gpu_backend: String::new(),
            llama_poll_interval: 1,
            models_dir: None,
            presets_file: std::path::PathBuf::new(),
            templates_file: std::path::PathBuf::new(),
            gpu_env_file: std::path::PathBuf::new(),
            gpu_arch_override: None,
            gpu_devices_override: None,
            ui_settings_file: std::path::PathBuf::new(),
            auth_config_file: std::path::PathBuf::new(),
            sessions_file: std::path::PathBuf::new(),
            ssh_known_hosts_file: std::path::PathBuf::new(),
            lhm_disabled_file: std::path::PathBuf::new(),
            agent_host: "127.0.0.1".to_string(),
            agent_port: 7777,
            agent_token: None,
            remote_agent_url: None,
            remote_agent_token: None,
            remote_agent_ssh_autostart: false,
            remote_agent_ssh_target: None,
            remote_agent_ssh_command: None,
            tls_config: TLSConfig::default(),
            live_api_token_store: std::sync::Arc::new(std::sync::RwLock::new(api_token)),
            live_db_admin_token_store: std::sync::Arc::new(std::sync::RwLock::new(db_admin_token)),
        }
    }
}

fn ensure_db_admin_token(config_dir: &PathBuf) -> Option<String> {
    let token_file = config_dir.join("db-admin-token");

    // Try to read existing token (may be encrypted)
    if let Ok(content) = fs::read_to_string(&token_file) {
        let trimmed = content.trim().to_string();
        if !trimmed.is_empty() {
            let token = decrypt_value(&trimmed);
            return Some(token);
        }
    }

    // Generate new token
    let token = generate_random_token();
    let _ = fs::create_dir_all(config_dir);
    let stored = encrypt_value(&token);
    if fs::write(&token_file, &stored).is_ok() {
        eprintln!("[config] Generated db-admin-token");
    }
    Some(token)
}

fn ensure_api_token(config_dir: &PathBuf) -> Option<String> {
    let token_file = config_dir.join("api-token");

    if let Ok(content) = fs::read_to_string(&token_file) {
        let trimmed = content.trim().to_string();
        if !trimmed.is_empty() {
            let token = decrypt_value(&trimmed);
            return Some(token);
        }
    }

    let token = generate_random_token();
    let _ = fs::create_dir_all(config_dir);
    let stored = encrypt_value(&token);
    if fs::write(&token_file, &stored).is_ok() {
        eprintln!("[config] Generated api-token");
    }
    Some(token)
}

pub(crate) fn generate_random_token() -> String {
    let mut buf = [0u8; 16];
    OsRng.fill_bytes(&mut buf);
    let value = u128::from_be_bytes(buf);
    format!("{value:x}")
}
