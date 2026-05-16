use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Command;

use crate::config::{TLSConfig, TlsMode};

const LEGO_BINARY: &str = "lego";
const ACME_EMAIL: &str = "acme@llama-monitor.local";
const STAGING_SERVER: &str = "https://acme-staging-v02.api.letsencrypt.org/directory";
const PROD_SERVER: &str = "https://acme-v02.api.letsencrypt.org/directory";

/// Build the path where lego stores ACME state.
fn acme_data_dir(config_dir: &std::path::Path) -> PathBuf {
    config_dir.join("acme-data")
}

/// Build the expected certificate path for a given FQDN.
fn acme_cert_path(config_dir: &std::path::Path, fqdn: &str) -> PathBuf {
    acme_data_dir(config_dir)
        .join("live")
        .join(fqdn)
        .join("certificate.pem")
}

/// Build the expected key path for a given FQDN.
fn acme_key_path(config_dir: &std::path::Path, fqdn: &str) -> PathBuf {
    acme_data_dir(config_dir)
        .join("live")
        .join(fqdn)
        .join("private_key.pem")
}

/// Scrub sensitive values from a command or error string.
fn scrub_secrets(input: &str) -> String {
    let mut s = input.to_string();
    for key in [
        "NAMECHEAP_USERNAME",
        "NAMECHEAP_API_KEY",
        "NAMECHEAP_SOURCEIP",
    ] {
        if let Some(pos) = s.find(key) {
            let rest = &s[pos..];
            if let Some(end) = rest.find('\n').or(Some(rest.len())) {
                s.replace_range(pos..pos + end, &format!("{}=[REDACTED]", key));
            }
        }
    }
    s
}

/// Run a lego subprocess, returning (exit_ok, stdout, stderr).
fn run_lego(args: &[&str], env: &HashMap<String, String>) -> (bool, String, String) {
    let mut cmd = Command::new(LEGO_BINARY);
    cmd.args(args);
    cmd.env_clear();

    // Preserve a minimal safe environment
    #[cfg(unix)]
    {
        if let Ok(path) = std::env::var("PATH") {
            cmd.env("PATH", path);
        }
        if let Ok(lang) = std::env::var("LANG") {
            cmd.env("LANG", lang);
        }
    }
    #[cfg(windows)]
    {
        if let Ok(systemroot) = std::env::var("SYSTEMROOT") {
            cmd.env("SYSTEMROOT", systemroot);
        }
        if let Ok(system32) = std::env::var("SYSTEM32") {
            cmd.env("SYSTEM32", system32);
        }
        if let Ok(path) = std::env::var("PATH") {
            cmd.env("PATH", path);
        }
    }

    // Apply ACME-specific env
    for (k, v) in env {
        cmd.env(k, v);
    }

    let output = cmd.output();
    match output {
        Ok(o) => {
            let stdout = String::from_utf8_lossy(&o.stdout).to_string();
            let stderr = String::from_utf8_lossy(&o.stderr).to_string();
            (o.status.success(), stdout, stderr)
        }
        Err(e) => (false, String::new(), format!("Failed to exec lego: {}", e)),
    }
}

/// High-level ACME certificate request using lego.
///
/// Validates config, runs lego, and returns updated TLSConfig or an error.
pub fn acme_request_cert(
    config_dir: &std::path::Path,
    cfg: &TLSConfig,
) -> Result<TLSConfig, String> {
    if cfg.mode != TlsMode::Acme {
        return Err("TLS mode is not acme".to_string());
    }

    let acme = &cfg.acme;

    if !acme.enabled {
        return Err("acme_enabled is false".to_string());
    }

    if acme.fqdn.is_empty() {
        return Err("acme_fqdn is required".to_string());
    }

    if acme.environment != "staging" && acme.environment != "production" {
        return Err(format!(
            "acme_environment must be 'staging' or 'production', got: '{}'",
            acme.environment
        ));
    }

    if acme.dns_provider.is_empty() {
        return Err("acme_dns_provider is required".to_string());
    }

    if acme.dns_config.is_empty() {
        return Err("acme_dns_config is required".to_string());
    }

    // For Namecheap, require username and api_key.
    if acme.dns_provider.to_lowercase() == "namecheap" && !acme.has_namecheap_creds() {
        return Err(
            "Namecheap provider requires 'username' and 'api_key' in acme_dns_config".to_string(),
        );
    }

    // Check lego is available.
    if !lego_available() {
        return Err("lego binary not found on PATH; install lego or add it to PATH".to_string());
    }

    let acme_path = acme_data_dir(config_dir);
    let _ = std::fs::create_dir_all(&acme_path);

    let server = if acme.environment == "staging" {
        STAGING_SERVER
    } else {
        PROD_SERVER
    };

    // Build environment for lego (Namecheap-specific mapping).
    let mut env = HashMap::new();
    if acme.dns_provider.to_lowercase() == "namecheap" {
        if let Some(u) = acme.dns_config.get("username") {
            env.insert("NAMECHEAP_USERNAME".to_string(), u.clone());
        }
        if let Some(k) = acme.dns_config.get("api_key") {
            env.insert("NAMECHEAP_API_KEY".to_string(), k.clone());
        }
        if let Some(ip) = acme.dns_config.get("source_ip") {
            env.insert("NAMECHEAP_SOURCEIP".to_string(), ip.clone());
        }
    }

    // Apply validation delay via lego's propagation check environment if present.
    // lego uses DNS_PROPAGATION_TIMEOUT and DNS_POLL_INTERVAL.
    if acme.validation_delay > 0 {
        env.insert(
            "DNS_PROPAGATION_TIMEOUT".to_string(),
            format!("{}s", acme.validation_delay),
        );
        env.insert(
            "DNS_POLL_INTERVAL".to_string(),
            format!("{}s", (acme.validation_delay / 4).max(5)),
        );
    }

    let fqdn = acme.fqdn.as_str();
    let dns_provider = acme.dns_provider.as_str();
    let acme_path_str = acme_path.to_string_lossy().to_string();
    let domains_arg = format!("--domains={}", fqdn);

    let args: Vec<&str> = vec![
        "run",
        "--accept-tos",
        "--email",
        ACME_EMAIL,
        "--dns",
        dns_provider,
        "--server",
        server,
        "--path",
        &acme_path_str,
        &domains_arg[..],
    ];

    let (ok, stdout, stderr) = run_lego(&args, &env);

    if !ok {
        let combined = format!("{}\n{}", stdout, stderr);
        let scrubbed = scrub_secrets(&combined);
        return Err(format!("lego run failed:\n{}", scrubbed.trim()));
    }

    // On success, update TLSConfig with paths and last_renewal.
    let mut new_cfg = cfg.clone();
    let cert = acme_cert_path(config_dir, fqdn);
    let key = acme_key_path(config_dir, fqdn);

    // Verify files exist.
    if !cert.exists() || !key.exists() {
        return Err("lego succeeded but cert/key not found at expected paths".to_string());
    }

    use chrono::{SecondsFormat, Utc};
    new_cfg.acme.cert_path = Some(cert);
    new_cfg.acme.key_path = Some(key);
    new_cfg.acme.last_renewal = Some(Utc::now().to_rfc3339_opts(SecondsFormat::Secs, false));

    Ok(new_cfg)
}

/// High-level ACME certificate renewal using lego.
pub fn acme_renew_cert(config_dir: &std::path::Path, cfg: &TLSConfig) -> Result<TLSConfig, String> {
    if cfg.mode != TlsMode::Acme {
        return Err("TLS mode is not acme".to_string());
    }

    let acme = &cfg.acme;

    if !acme.enabled {
        return Err("acme_enabled is false".to_string());
    }

    if acme.fqdn.is_empty() {
        return Err("acme_fqdn is required".to_string());
    }

    if acme.environment != "staging" && acme.environment != "production" {
        return Err("acme_environment must be 'staging' or 'production'".to_string());
    }

    if acme.dns_provider.is_empty() {
        return Err("acme_dns_provider is required".to_string());
    }

    if acme.dns_config.is_empty() {
        return Err("acme_dns_config is required".to_string());
    }

    if !lego_available() {
        return Err("lego binary not found on PATH".to_string());
    }

    let acme_path = acme_data_dir(config_dir);
    let _ = std::fs::create_dir_all(&acme_path);

    let server = if acme.environment == "staging" {
        STAGING_SERVER
    } else {
        PROD_SERVER
    };

    // Build environment (same as request).
    let mut env = HashMap::new();
    if acme.dns_provider.to_lowercase() == "namecheap" {
        if let Some(u) = acme.dns_config.get("username") {
            env.insert("NAMECHEAP_USERNAME".to_string(), u.clone());
        }
        if let Some(k) = acme.dns_config.get("api_key") {
            env.insert("NAMECHEAP_API_KEY".to_string(), k.clone());
        }
        if let Some(ip) = acme.dns_config.get("source_ip") {
            env.insert("NAMECHEAP_SOURCEIP".to_string(), ip.clone());
        }
    }

    if acme.validation_delay > 0 {
        env.insert(
            "DNS_PROPAGATION_TIMEOUT".to_string(),
            format!("{}s", acme.validation_delay),
        );
        env.insert(
            "DNS_POLL_INTERVAL".to_string(),
            format!("{}s", (acme.validation_delay / 4).max(5)),
        );
    }

    let fqdn = acme.fqdn.as_str();
    let dns_provider = acme.dns_provider.as_str();
    let acme_path_str = acme_path.to_string_lossy().to_string();

    // lego renew uses --cert-path and --cert-key to target specific certs.
    let cert_path = acme_cert_path(config_dir, fqdn)
        .to_string_lossy()
        .to_string();
    let key_path = acme_key_path(config_dir, fqdn)
        .to_string_lossy()
        .to_string();

    let args: Vec<&str> = vec![
        "renew",
        "--accept-tos",
        "--dns",
        dns_provider,
        "--server",
        server,
        "--path",
        &acme_path_str,
        "--cert-path",
        &cert_path[..],
        "--cert-key",
        &key_path[..],
    ];

    let (ok, stdout, stderr) = run_lego(&args, &env);

    if !ok {
        let combined = format!("{}\n{}", stdout, stderr);
        let scrubbed = scrub_secrets(&combined);
        return Err(format!("lego renew failed:\n{}", scrubbed.trim()));
    }

    // Update last_renewal.
    let mut new_cfg = cfg.clone();
    use chrono::{SecondsFormat, Utc};
    new_cfg.acme.last_renewal = Some(Utc::now().to_rfc3339_opts(SecondsFormat::Secs, false));

    Ok(new_cfg)
}

/// Decide if renewal is needed.
pub fn should_renew(cfg: &TLSConfig) -> bool {
    if cfg.mode != TlsMode::Acme {
        return false;
    }

    // If never renewed, treat as needing initial request/renew.
    if cfg.acme.last_renewal.is_none() {
        return true;
    }

    // If last renewal > 60 days ago, renew.
    if let Some(ts) = &cfg.acme.last_renewal
        && let Ok(dt) = chrono::DateTime::parse_from_rfc3339(ts)
    {
        let age = chrono::Utc::now() - dt.with_timezone(&chrono::Utc);
        if age.num_days() > 60 {
            return true;
        }
    }

    // If cert file is older than 60 days, renew.
    if let Some(ref path) = cfg.acme.cert_path
        && let Ok(meta) = std::fs::metadata(path)
        && let Ok(modified) = meta.modified()
        && let Ok(age) = std::time::SystemTime::now().duration_since(modified)
        && age.as_secs() > 60 * 24 * 60 * 60
    {
        return true;
    }

    false
}

/// Check if lego is available on PATH.
fn lego_available() -> bool {
    let output = Command::new(LEGO_BINARY).arg("version").output();
    match output {
        Ok(o) => o.status.success(),
        Err(_) => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::AcmeConfig;

    #[test]
    fn should_renew_false_for_non_acme() {
        let cfg = TLSConfig {
            mode: TlsMode::None,
            ..Default::default()
        };
        assert!(!should_renew(&cfg));

        let cfg2 = TLSConfig {
            mode: TlsMode::SelfSigned,
            ..Default::default()
        };
        assert!(!should_renew(&cfg2));
    }

    #[test]
    fn should_renew_true_for_acme_no_last_renewal() {
        let cfg = TLSConfig {
            mode: TlsMode::Acme,
            acme: AcmeConfig {
                enabled: true,
                fqdn: "example.com".to_string(),
                environment: "production".to_string(),
                dns_provider: "namecheap".to_string(),
                last_renewal: None,
                ..Default::default()
            },
            ..Default::default()
        };
        assert!(should_renew(&cfg));
    }

    #[test]
    fn should_renew_false_for_recent_renewal() {
        use chrono::{SecondsFormat, Utc};
        let now = Utc::now().to_rfc3339_opts(SecondsFormat::Secs, false);
        let cfg = TLSConfig {
            mode: TlsMode::Acme,
            acme: AcmeConfig {
                enabled: true,
                fqdn: "example.com".to_string(),
                environment: "production".to_string(),
                dns_provider: "namecheap".to_string(),
                last_renewal: Some(now),
                ..Default::default()
            },
            ..Default::default()
        };
        assert!(!should_renew(&cfg));
    }

    #[test]
    fn scrub_secrets_removes_api_key() {
        let input = "NAMECHEAP_API_KEY=supersecret123\nother line";
        let out = scrub_secrets(input);
        assert!(out.contains("[REDACTED]"));
        assert!(!out.contains("supersecret123"));
    }
}
