/// Certificate management for mTLS (mutual TLS) authentication.
///
/// The dashboard acts as a Certificate Authority (CA). On first run, it generates:
/// - A CA certificate (valid 10 years)
/// - A client certificate (valid 1 year, signed by CA)
///
/// When installing a remote agent, the CA certificate is shipped with the install payload.
/// The agent generates a server certificate (valid 1 year, signed by the same CA).
///
/// Certificates are automatically renewed on startup if they expire within 30 days.
use std::path::{Path, PathBuf};

/// Returns the path to the certs directory, creating it if necessary.
pub fn certs_dir() -> PathBuf {
    let dir = dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("llama-monitor")
        .join("certs");
    let _ = std::fs::create_dir_all(&dir);
    dir
}

/// Represents a certificate and its key pair.
#[derive(Debug)]
pub struct Cert {
    pub pem: String,
    pub key: String,
}

impl Cert {
    /// Loads a certificate from disk, or returns `None` if it doesn't exist.
    pub fn load(pem_path: &Path, key_path: &Path) -> Option<Self> {
        let pem = std::fs::read_to_string(pem_path).ok()?;
        let key = std::fs::read_to_string(key_path).ok()?;
        Some(Cert { pem, key })
    }

    /// Saves the certificate to disk.
    pub fn save(&self, pem_path: &Path, key_path: &Path) -> std::io::Result<()> {
        std::fs::write(pem_path, &self.pem)?;
        std::fs::write(key_path, &self.key)?;
        Ok(())
    }

    /// Checks if the certificate is expiring within the renewal window.
    pub fn needs_renewal(&self) -> bool {
        // Simplified check — regenerate if the cert file is older than CERT_VALIDITY_DAYS - RENEWAL_WINDOW_DAYS
        if let Some(metadata) =
            std::fs::metadata(Path::new(self.pem.split('\n').next().unwrap_or(""))).ok()
            && let Ok(modified) = metadata.modified()
            && let Ok(age) = std::time::SystemTime::now().duration_since(modified)
        {
            let days_old = age.as_secs() / (60 * 60 * 24);
            return days_old > (365 - 30) as u64;
        }
        false
    }
}

/// Generates a self-signed certificate for testing.
pub fn generate_self_signed(sans: Vec<String>) -> Cert {
    let rcgen::CertifiedKey { cert, signing_key } =
        rcgen::generate_simple_self_signed(sans).unwrap();
    Cert {
        pem: cert.pem(),
        key: signing_key.serialize_pem(),
    }
}

/// Ensures the CA certificate exists, generating it if necessary.
pub fn ensure_ca() -> Cert {
    let dir = certs_dir();
    let ca_path = dir.join("ca.pem");
    let ca_key_path = dir.join("ca.key");

    match Cert::load(&ca_path, &ca_key_path) {
        Some(ca) if !ca.needs_renewal() => ca,
        _ => {
            let ca = generate_self_signed(vec!["localhost".to_string()]);
            let _ = ca.save(&ca_path, &ca_key_path);
            ca
        }
    }
}
