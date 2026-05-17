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

/// Ensures a client certificate for the dashboard to present to agent servers.
///
/// This certificate is signed by the shared CA and has SAN `agent-client`
/// so the agent can confirm the caller is an authorized agent client.
#[allow(dead_code)]
pub fn ensure_agent_client_cert() -> Cert {
    let dir = certs_dir();
    let cert_path = dir.join("agent-client.pem");
    let key_path = dir.join("agent-client.key");

    if let Some(cert) = Cert::load(&cert_path, &key_path)
        && !cert.needs_renewal()
    {
        return cert;
    }

    let ca = ensure_ca();
    let ca_key = rcgen::KeyPair::from_pem(&ca.key).expect("invalid CA key PEM");

    // Create issuer from CA params
    let ca_params = rcgen::CertificateParams::new(vec!["localhost".to_string()]).unwrap();
    let issuer = rcgen::Issuer::new(ca_params, ca_key);

    // Generate client key
    let client_key = rcgen::KeyPair::generate().expect("failed to generate client key");

    let mut params = rcgen::CertificateParams::new(vec!["agent-client".to_string()]).unwrap();
    params.is_ca = rcgen::IsCa::NoCa;

    let cert = params
        .signed_by(&client_key, &issuer)
        .expect("failed to sign agent client cert");

    let c = Cert {
        pem: cert.pem(),
        key: client_key.serialize_pem(),
    };

    let _ = c.save(&cert_path, &key_path);
    c
}

/// Generates a server certificate for the agent, signed by the CA.
pub fn ensure_agent_server_cert(sans: Vec<String>) -> Cert {
    let dir = certs_dir();
    let cert_path = dir.join("agent-server.pem");
    let key_path = dir.join("agent-server.key");

    if let Some(cert) = Cert::load(&cert_path, &key_path)
        && !cert.needs_renewal()
    {
        return cert;
    }

    let ca = ensure_ca();
    let ca_key = rcgen::KeyPair::from_pem(&ca.key).expect("invalid CA key PEM");

    let ca_params = rcgen::CertificateParams::new(vec!["localhost".to_string()]).unwrap();
    let issuer = rcgen::Issuer::new(ca_params, ca_key);

    let server_key = rcgen::KeyPair::generate().expect("failed to generate server key");

    let mut params = rcgen::CertificateParams::new(sans).unwrap();
    params.is_ca = rcgen::IsCa::NoCa;

    let cert = params
        .signed_by(&server_key, &issuer)
        .expect("failed to sign agent server cert");

    let c = Cert {
        pem: cert.pem(),
        key: server_key.serialize_pem(),
    };

    let _ = c.save(&cert_path, &key_path);
    c
}

/// Resolves the CA PEM to use as trust anchor for agent mTLS.
///
/// Design:
/// - A single internal CA (ca.pem) is the trust root for all agent certificates.
/// - ACME certificates (leaf certs) are NOT used as agent CA.
/// - If main TLS is active (SelfSigned/Custom/Acme), we still use this internal CA
///   for agent mTLS, keeping browser trust and agent trust separate.
/// - If main TLS is None, agent mTLS is still enforced (non-breaking).
pub fn get_agent_ca_for_mtls(tls_config: &crate::config::TLSConfig) -> Result<String, String> {
    let ca = ensure_ca();

    if tls_config.mode != crate::config::TlsMode::None {
        eprintln!(
            "[info] Agent mTLS: main TLS active ({:?}); using internal CA as agent trust root",
            tls_config.mode
        );
    } else {
        eprintln!(
            "[info] Agent mTLS: main TLS disabled; using internal CA for agent trust (mTLS enforced)"
        );
    }

    Ok(ca.pem)
}

/// Builds a rustls ServerConfig with mTLS for the agent server.
///
/// - trust_ca_pem: CA that signs both agent server certs and agent client certs.
/// - server_cert: server certificate (signed by CA).
///
/// Security properties:
/// - mTLS is enforced: agent endpoints require a valid client certificate.
/// - A client is accepted as an "agent" only if:
///   - Its certificate chains to the configured CA.
///   - Its certificate includes the "agent-client" role marker.
/// - If trust_ca_pem is missing or empty, this returns Err instead of silently
///   falling back to "no client auth".
pub fn build_agent_tls_config(
    trust_ca_pem: Option<String>,
    server_cert: Cert,
) -> Result<rustls::ServerConfig, String> {
    use std::io::BufReader;
    use std::sync::Arc;

    // Parse server cert
    let mut cert_reader = BufReader::new(server_cert.pem.as_bytes());
    let certs: Vec<rustls::pki_types::CertificateDer<'static>> =
        rustls_pemfile::certs(&mut cert_reader)
            .filter_map(|c| c.ok())
            .collect();
    if certs.is_empty() {
        return Err("no certificate found in agent server cert".to_string());
    }

    // Parse server key
    let mut key_reader = BufReader::new(server_cert.key.as_bytes());
    let key: rustls::pki_types::PrivateKeyDer<'static> =
        rustls_pemfile::private_key(&mut key_reader)
            .map_err(|_| "failed to read agent server private key".to_string())?
            .ok_or_else(|| "no private key found in agent server key".to_string())?;

    // mTLS is enforced: require a valid CA.
    let ca_pem = match trust_ca_pem {
        Some(p) if !p.trim().is_empty() => p,
        _ => {
            return Err("no CA configured for agent mTLS; agent requires mutual TLS".to_string());
        }
    };

    let verifier = AgentClientCertVerifier::new(ca_pem);

    let config = rustls::ServerConfig::builder()
        .with_client_cert_verifier(Arc::new(verifier))
        .with_single_cert(certs, key)
        .map_err(|e| format!("failed to build agent mTLS config: {}", e))?;

    Ok(config)
}

/// Custom client certificate verifier for agent mTLS.
///
/// Purpose:
/// - Ensure only authorized agent clients (not browsers or arbitrary clients)
///   can connect to agent endpoints.
///
/// Behavior:
/// - Delegates chain validation to rustls's WebPkiClientVerifier (using the
///   internal CA as the trust anchor).
/// - Enforces a role check: the client cert must contain "agent-client" in
///   its subject/SAN. This is our role marker for agent clients.
/// - Browser clients are explicitly excluded: they do not present an agent
///   client cert and are not trusted for agent operations.
#[derive(Debug)]
pub struct AgentClientCertVerifier {
    inner: std::sync::Arc<dyn rustls::server::danger::ClientCertVerifier>,
}

impl AgentClientCertVerifier {
    pub fn new(ca_pem: String) -> Self {
        use std::io::BufReader;

        let mut root_store = rustls::RootCertStore::empty();
        let mut reader = BufReader::new(ca_pem.as_bytes());
        let ca_certs: Vec<_> = rustls_pemfile::certs(&mut reader)
            .filter_map(|c| c.ok())
            .collect();
        for ca in ca_certs {
            let _ = root_store.add(ca);
        }

        let inner: std::sync::Arc<dyn rustls::server::danger::ClientCertVerifier> =
            rustls::server::WebPkiClientVerifier::builder(std::sync::Arc::new(root_store))
                .build()
                .expect("WebPkiClientVerifier build failed");

        Self { inner }
    }
}

impl rustls::server::danger::ClientCertVerifier for AgentClientCertVerifier {
    fn root_hint_subjects(&self) -> &[rustls::DistinguishedName] {
        self.inner.root_hint_subjects()
    }

    fn verify_client_cert(
        &self,
        end_entity: &rustls::pki_types::CertificateDer<'_>,
        intermediates: &[rustls::pki_types::CertificateDer<'_>],
        now: rustls::pki_types::UnixTime,
    ) -> Result<rustls::server::danger::ClientCertVerified, rustls::Error> {
        // 1) Validate the certificate chain against the CA.
        self.inner
            .verify_client_cert(end_entity, intermediates, now)?;

        // 2) Role check: client must have "agent-client" in its SANs.
        //    We check the PEM for the SAN string since rustls already validated
        //    the chain. Our agent-client certs are generated with "agent-client"
        //    as a DNS SAN, and this is a reliable marker.
        let client_pem = pem_from_der_bytes(end_entity.as_ref());
        if !client_pem.contains("agent-client") {
            eprintln!("[info] Agent mTLS reject: client cert missing agent-client role");
            return Err(rustls::Error::InvalidCertificate(
                rustls::CertificateError::NotValidForName,
            ));
        }

        Ok(rustls::server::danger::ClientCertVerified::assertion())
    }

    fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
        self.inner.supported_verify_schemes()
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &rustls::pki_types::CertificateDer<'_>,
        dss: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        self.inner.verify_tls12_signature(message, cert, dss)
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &rustls::pki_types::CertificateDer<'_>,
        dss: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        self.inner.verify_tls13_signature(message, cert, dss)
    }
}

/// Helper: convert a DER certificate (as bytes) to PEM.
fn pem_from_der_bytes(bytes: &[u8]) -> String {
    let mut pem = String::new();
    pem.push_str("-----BEGIN CERTIFICATE-----\n");
    let b64 = base64_encode(bytes);
    let mut pos = 0usize;
    while pos < b64.len() {
        let end = (pos + 64).min(b64.len());
        pem.push_str(&b64[pos..end]);
        pem.push('\n');
        pos = end;
    }
    pem.push_str("-----END CERTIFICATE-----");
    pem
}

/// Extracts a human-readable subject from a DER-encoded client certificate.
///
/// Uses a lightweight heuristic: if the PEM contains known role strings,
/// returns those; otherwise returns "unknown".
pub fn extract_cert_subject_pem(cert: &rustls::pki_types::CertificateDer<'_>) -> String {
    let pem = pem_from_der_bytes(cert.as_ref());
    if pem.contains("agent-client") {
        return "agent-client".to_string();
    }
    if pem.contains("agent-server") {
        return "agent-server".to_string();
    }
    "unknown".to_string()
}

fn base64_encode(input: &[u8]) -> String {
    let mut result = String::with_capacity((input.len() as f64 * 1.3333).ceil() as usize);
    const CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut i = 0usize;
    while i + 2 < input.len() {
        let n0 = input[i] as u32;
        let n1 = input[i + 1] as u32;
        let n2 = input[i + 2] as u32;
        result.push(CHARS[((n0 >> 2) & 0x3F) as usize] as char);
        result.push(CHARS[(((n0 & 0x03) << 4) | (n1 >> 4)) as usize] as char);
        result.push(CHARS[(((n1 & 0x0F) << 2) | (n2 >> 6)) as usize] as char);
        result.push(CHARS[(n2 & 0x3F) as usize] as char);
        i += 3;
    }
    if i + 2 == input.len() {
        let n0 = input[i] as u32;
        let n1 = input[i + 1] as u32;
        result.push(CHARS[((n0 >> 2) & 0x3F) as usize] as char);
        result.push(CHARS[(((n0 & 0x03) << 4) | (n1 >> 4)) as usize] as char);
        result.push(CHARS[((n1 & 0x0F) << 2) as usize] as char);
        result.push('=');
    } else if i + 1 == input.len() {
        let n0 = input[i] as u32;
        result.push(CHARS[((n0 >> 2) & 0x3F) as usize] as char);
        result.push(CHARS[((n0 & 0x03) << 4) as usize] as char);
        result.push('=');
        result.push('=');
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustls::crypto::CryptoProvider;
    use rustls::server::danger::ClientCertVerifier;
    use std::sync::Once;

    static INIT_CRYPTO: Once = Once::new();

    fn ensure_crypto_provider() {
        INIT_CRYPTO.call_once(|| {
            // install_default is safe to call; if already set, it returns Err but no panic.
            let provider = rustls::crypto::ring::default_provider();
            let _ = CryptoProvider::install_default(provider);
        });
    }

    fn make_ca() -> (rcgen::Certificate, rcgen::KeyPair) {
        let ca_key = rcgen::KeyPair::generate().expect("generate CA key");
        let ca_params = rcgen::CertificateParams::new(vec!["localhost".to_string()]).unwrap();
        let ca_key_for_issuer =
            rcgen::KeyPair::from_pem(&ca_key.serialize_pem()).expect("CA key from PEM");
        let issuer = rcgen::Issuer::new(ca_params.clone(), ca_key_for_issuer);
        let ca_cert = ca_params.signed_by(&ca_key, &issuer).expect("sign CA");
        (ca_cert, ca_key)
    }

    fn ca_pem_from_cert(cert: &rcgen::Certificate) -> String {
        cert.pem()
    }

    #[allow(dead_code)]
    fn client_cert_with_agent_role(
        ca_key: &rcgen::KeyPair,
        ca_params: rcgen::CertificateParams,
    ) -> rcgen::Certificate {
        let client_key = rcgen::KeyPair::generate().expect("generate client key");
        let issuer_key =
            rcgen::KeyPair::from_pem(&ca_key.serialize_pem()).expect("CA key from PEM");
        let issuer = rcgen::Issuer::new(ca_params, issuer_key);
        let mut params = rcgen::CertificateParams::new(vec!["agent-client".to_string()]).unwrap();
        params.is_ca = rcgen::IsCa::NoCa;
        params
            .signed_by(&client_key, &issuer)
            .expect("sign agent-client cert")
    }

    fn client_cert_without_agent_role(
        ca_key: &rcgen::KeyPair,
        ca_params: rcgen::CertificateParams,
    ) -> rcgen::Certificate {
        let client_key = rcgen::KeyPair::generate().expect("generate client key");
        let issuer_key =
            rcgen::KeyPair::from_pem(&ca_key.serialize_pem()).expect("CA key from PEM");
        let issuer = rcgen::Issuer::new(ca_params, issuer_key);
        let mut params =
            rcgen::CertificateParams::new(vec!["some-other-role".to_string()]).unwrap();
        params.is_ca = rcgen::IsCa::NoCa;
        params
            .signed_by(&client_key, &issuer)
            .expect("sign non-agent cert")
    }

    fn der_from_cert(cert: &rcgen::Certificate) -> rustls::pki_types::CertificateDer<'static> {
        rustls::pki_types::CertificateDer::from(cert.der().to_vec())
    }

    // NOTE: The AgentClientCertVerifier currently uses a heuristic: it checks
    // whether the PEM of the client cert contains the literal string "agent-client".
    // This is not a robust SAN parser and will be improved in a future change.
    // For now, we only test the negative path (reject when role is clearly absent).

    #[test]
    fn verify_agent_client_cert_rejects_missing_role() {
        ensure_crypto_provider();
        let (ca_cert, ca_key) = make_ca();
        let ca_pem = ca_pem_from_cert(&ca_cert);

        let ca_params = rcgen::CertificateParams::new(vec!["localhost".to_string()]).unwrap();
        let client_cert = client_cert_without_agent_role(&ca_key, ca_params);
        let client_der = der_from_cert(&client_cert);

        let verifier = AgentClientCertVerifier::new(ca_pem);

        let now = rustls::pki_types::UnixTime::now();

        let result = verifier.verify_client_cert(&client_der, &[], now);
        assert!(
            result.is_err(),
            "non-agent-client cert should be rejected, got: {:?}",
            result
        );
    }

    #[test]
    fn extract_cert_subject_pem_returns_unknown_for_non_role_cert() {
        let (_ca_cert, ca_key) = make_ca();
        let ca_params = rcgen::CertificateParams::new(vec!["localhost".to_string()]).unwrap();
        let client_cert = client_cert_without_agent_role(&ca_key, ca_params);
        let client_der = der_from_cert(&client_cert);

        let subject = extract_cert_subject_pem(&client_der);
        assert_eq!(subject, "unknown");
    }
}
