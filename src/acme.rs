use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Command;

use crate::config::{TLSConfig, TlsMode};

const LEGO_BINARY: &str = "lego";
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
    let s = input.to_string();
    // Scrub any KEY=VALUE patterns on their own line
    let mut out = String::with_capacity(s.len());
    for line in s.lines() {
        let trimmed = line.trim_start();
        if trimmed.contains('=')
            && trimmed
                .split('=')
                .next()
                .unwrap_or("")
                .chars()
                .all(|c| c.is_alphanumeric() || c == '_')
        {
            let eq_pos = trimmed.find('=').unwrap_or(0);
            out.push_str(&line[..line.len() - line.trim_start().len()]);
            out.push_str(&trimmed[..eq_pos]);
            out.push_str("=[REDACTED]");
            out.push('\n');
        } else {
            out.push_str(line);
            out.push('\n');
        }
    }
    out.trim_end_matches('\n').to_string()
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

/// Build the lego command args and environment map for a given ACME config.
/// Used by both request and renew operations.
fn build_lego_command(
    cfg: &TLSConfig,
    config_dir: &std::path::Path,
    acme_path: &std::path::Path,
    fqdn: &str,
    mode: &str, // "run" or "renew"
) -> (Vec<String>, HashMap<String, String>) {
    let acme = &cfg.acme;

    let server = if acme.environment == "staging" {
        STAGING_SERVER
    } else {
        PROD_SERVER
    };

    // Build environment: all key/value pairs from dns_config are passed as-is.
    // lego providers document their required env vars (e.g., CLOUDFLARE_API_TOKEN).
    let mut env = acme.dns_config.clone();

    // Apply validation delay via lego's propagation check environment if present.
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

    let acme_path_str = acme_path.to_string_lossy().to_string();
    let email = if cfg.acme.email.is_empty() {
        format!("acme@{}", fqdn)
    } else {
        cfg.acme.email.clone()
    };

    let mut args: Vec<String> = vec![
        mode.to_string(),
        "--accept-tos".to_string(),
        "--email".to_string(),
        email,
        "--dns".to_string(),
        acme.dns_provider.clone(),
        "--server".to_string(),
        server.to_string(),
        "--path".to_string(),
        acme_path_str,
    ];

    if mode == "run" {
        args.push("--domains".to_string());
        args.push(fqdn.to_string());
    } else if mode == "renew" {
        let cert_path = acme_cert_path(config_dir, fqdn)
            .to_string_lossy()
            .to_string();
        let key_path = acme_key_path(config_dir, fqdn)
            .to_string_lossy()
            .to_string();
        args.push("--cert-path".to_string());
        args.push(cert_path);
        args.push("--cert-key".to_string());
        args.push(key_path);
    }

    (args, env)
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

    // Check lego is available.
    if !lego_available() {
        return Err("lego binary not found on PATH; install lego or add it to PATH".to_string());
    }

    let acme_path = acme_data_dir(config_dir);
    let _ = std::fs::create_dir_all(&acme_path);

    let fqdn = acme.fqdn.as_str();

    let (args, env) = build_lego_command(cfg, config_dir, &acme_path, fqdn, "run");

    let args_str: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
    let (ok, stdout, stderr) = run_lego(&args_str, &env);

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

    let fqdn = acme.fqdn.as_str();

    let (args, env) = build_lego_command(cfg, config_dir, &acme_path, fqdn, "renew");

    let args_str: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
    let (ok, stdout, stderr) = run_lego(&args_str, &env);

    if !ok {
        let combined = format!("{}\n{}", stdout, stderr);
        let scrubbed = scrub_secrets(&combined);
        return Err(format!("lego renew failed:\n{}", scrubbed.trim()));
    }

    // Verify cert files still exist at their expected paths after renewal.
    let cert = acme_cert_path(config_dir, fqdn);
    let key = acme_key_path(config_dir, fqdn);
    if !cert.exists() || !key.exists() {
        return Err("lego renew succeeded but cert/key not found at expected paths".to_string());
    }

    let mut new_cfg = cfg.clone();
    use chrono::{SecondsFormat, Utc};
    new_cfg.acme.last_renewal = Some(Utc::now().to_rfc3339_opts(SecondsFormat::Secs, false));
    new_cfg.acme.cert_path = Some(cert);
    new_cfg.acme.key_path = Some(key);

    Ok(new_cfg)
}

/// Decide if renewal is needed.
pub fn should_renew(cfg: &TLSConfig) -> bool {
    if cfg.mode != TlsMode::Acme {
        return false;
    }

    // If no cert file exists yet, this is a fresh install awaiting the initial
    // cert request (via API). Renewal is not applicable here.
    let cert_exists = cfg.acme.cert_path.as_ref().is_some_and(|p| p.exists());
    if !cert_exists {
        return false;
    }

    // Cert exists but no renewal record — renew now (cert obtained outside app).
    if cfg.acme.last_renewal.is_none() {
        return true;
    }

    // Renew if last recorded renewal was more than 60 days ago.
    if let Some(ts) = &cfg.acme.last_renewal
        && let Ok(dt) = chrono::DateTime::parse_from_rfc3339(ts)
    {
        let age = chrono::Utc::now() - dt.with_timezone(&chrono::Utc);
        if age.num_days() > 60 {
            return true;
        }
    }

    // Fallback: renew if the cert file itself is older than 60 days.
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
    fn should_renew_false_when_no_cert_exists() {
        // No cert on disk → initial request, not renewal.
        let cfg = TLSConfig {
            mode: TlsMode::Acme,
            acme: AcmeConfig {
                enabled: true,
                fqdn: "example.com".to_string(),
                environment: "production".to_string(),
                dns_provider: "namecheap".to_string(),
                last_renewal: None,
                cert_path: Some(std::path::PathBuf::from("/nonexistent/cert.pem")),
                ..Default::default()
            },
            ..Default::default()
        };
        assert!(!should_renew(&cfg));
    }

    #[test]
    fn should_renew_true_when_cert_exists_no_renewal_record() {
        // Cert file exists but no renewal record → treat as needing renewal.
        let dir = tempfile::tempdir().expect("tempdir");
        let cert = dir.path().join("cert.pem");
        std::fs::write(&cert, b"fake").expect("write cert");
        let cfg = TLSConfig {
            mode: TlsMode::Acme,
            acme: AcmeConfig {
                enabled: true,
                fqdn: "example.com".to_string(),
                environment: "production".to_string(),
                dns_provider: "namecheap".to_string(),
                last_renewal: None,
                cert_path: Some(cert),
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
        let input = "CLOUDFLARE_API_TOKEN=supersecret123\nother line";
        let out = scrub_secrets(input);
        assert!(out.contains("[REDACTED]"));
        assert!(!out.contains("supersecret123"));
    }
}
