use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use argon2::Argon2;
use argon2::password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use rand_core::OsRng;
use subtle::ConstantTimeEq;

use crate::config::{DashboardAuthConfig, TlsMode, generate_random_token};

const FORM_SESSION_COOKIE: &str = "llama_monitor_session";
const FORM_SESSION_TTL_SECS: u64 = 60 * 60 * 12;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AuthCredentials {
    pub username: String,
    pub password: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AuthMethod {
    Basic,
    Form,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AuthSource {
    None,
    Config,
    Cli,
}

#[derive(Clone, Debug)]
pub struct AuthStatus {
    pub authenticated: bool,
    pub method: Option<AuthMethod>,
    pub username: Option<String>,
}

#[derive(Clone, Debug)]
pub struct AuthConfigView {
    pub source: AuthSource,
    pub basic_enabled: bool,
    pub form_enabled: bool,
    pub username: Option<String>,
}

#[derive(Clone, Debug)]
struct PasswordCredential {
    username: String,
    password_hash: String,
}

#[derive(Clone, Debug)]
struct FormSession {
    username: String,
    expires_at: u64,
}

#[derive(Clone, Debug)]
struct AuthState {
    source: AuthSource,
    basic: Option<PasswordCredential>,
    form: Option<PasswordCredential>,
}

#[derive(Clone)]
pub struct AuthManager {
    state: Arc<Mutex<AuthState>>,
    sessions: Arc<Mutex<HashMap<String, FormSession>>>,
    tls_enabled: bool,
}

impl PasswordCredential {
    fn from_plaintext(username: &str, password: &str) -> Option<Self> {
        let username = username.trim();
        if username.is_empty() || password.is_empty() {
            return None;
        }
        Some(Self {
            username: username.to_string(),
            password_hash: hash_password(password)?,
        })
    }

    fn from_hash(username: &str, password_hash: &str) -> Option<Self> {
        let username = username.trim();
        if username.is_empty() || password_hash.trim().is_empty() {
            return None;
        }
        Some(Self {
            username: username.to_string(),
            password_hash: password_hash.to_string(),
        })
    }

    fn verify(&self, username: &str, password: &str) -> bool {
        if self
            .username
            .as_bytes()
            .ct_eq(username.trim().as_bytes())
            .unwrap_u8()
            != 1
        {
            return false;
        }
        verify_password(&self.password_hash, password)
    }
}

impl AuthManager {
    pub fn new(
        basic: Option<AuthCredentials>,
        form: Option<AuthCredentials>,
        tls_mode: &TlsMode,
    ) -> Self {
        let basic = basic
            .and_then(|creds| PasswordCredential::from_plaintext(&creds.username, &creds.password));
        let form = form
            .and_then(|creds| PasswordCredential::from_plaintext(&creds.username, &creds.password));
        let source = if basic.is_some() || form.is_some() {
            AuthSource::Cli
        } else {
            AuthSource::None
        };

        Self {
            state: Arc::new(Mutex::new(AuthState {
                source,
                basic,
                form,
            })),
            sessions: Arc::new(Mutex::new(HashMap::new())),
            tls_enabled: !matches!(tls_mode, TlsMode::None),
        }
    }

    pub fn from_config(cfg: DashboardAuthConfig, tls_mode: &TlsMode) -> Self {
        let basic = if cfg.is_usable() && cfg.basic_enabled {
            PasswordCredential::from_hash(&cfg.username, &cfg.password_hash)
        } else {
            None
        };
        let form = if cfg.is_usable() && cfg.form_enabled {
            PasswordCredential::from_hash(&cfg.username, &cfg.password_hash)
        } else {
            None
        };
        let source = if basic.is_some() || form.is_some() {
            AuthSource::Config
        } else {
            AuthSource::None
        };

        Self {
            state: Arc::new(Mutex::new(AuthState {
                source,
                basic,
                form,
            })),
            sessions: Arc::new(Mutex::new(HashMap::new())),
            tls_enabled: !matches!(tls_mode, TlsMode::None),
        }
    }

    pub fn parse_credentials(spec: &str) -> Option<AuthCredentials> {
        let parts: Vec<&str> = spec.splitn(2, ':').collect();
        if parts.len() != 2 || parts[0].trim().is_empty() || parts[1].is_empty() {
            return None;
        }
        Some(AuthCredentials {
            username: parts[0].trim().to_string(),
            password: parts[1].to_string(),
        })
    }

    pub fn hash_password(password: &str) -> Option<String> {
        hash_password(password)
    }

    pub fn has_basic(&self) -> bool {
        self.snapshot().basic.is_some()
    }

    pub fn has_form(&self) -> bool {
        self.snapshot().form.is_some()
    }

    pub fn has_any(&self) -> bool {
        let state = self.snapshot();
        state.basic.is_some() || state.form.is_some()
    }

    pub fn source(&self) -> AuthSource {
        self.snapshot().source
    }

    pub fn config_view(&self) -> AuthConfigView {
        let state = self.snapshot();
        let username = state
            .form
            .as_ref()
            .or(state.basic.as_ref())
            .map(|creds| creds.username.clone());
        AuthConfigView {
            source: state.source,
            basic_enabled: state.basic.is_some(),
            form_enabled: state.form.is_some(),
            username,
        }
    }

    pub fn status(&self, auth_header: Option<&str>, cookie_header: Option<&str>) -> AuthStatus {
        if let Some(username) = self.authenticate_basic(auth_header) {
            return AuthStatus {
                authenticated: true,
                method: Some(AuthMethod::Basic),
                username: Some(username),
            };
        }
        if let Some(username) = self.authenticate_form(cookie_header) {
            return AuthStatus {
                authenticated: true,
                method: Some(AuthMethod::Form),
                username: Some(username),
            };
        }
        AuthStatus {
            authenticated: false,
            method: None,
            username: None,
        }
    }

    pub fn authenticate_request(
        &self,
        auth_header: Option<&str>,
        cookie_header: Option<&str>,
    ) -> bool {
        self.status(auth_header, cookie_header).authenticated
    }

    pub fn authenticate_basic(&self, auth_header: Option<&str>) -> Option<String> {
        let state = self.snapshot();
        let expected = state.basic.as_ref()?;
        let header = auth_header?;
        let encoded = header.strip_prefix("Basic ")?;
        let decoded = base64::Engine::decode(
            &base64::engine::general_purpose::STANDARD,
            encoded.as_bytes(),
        )
        .ok()?;
        let decoded_str = std::str::from_utf8(&decoded).ok()?;
        let colon_pos = decoded_str.find(':')?;
        let username = &decoded_str[..colon_pos];
        let password = &decoded_str[colon_pos + 1..];
        if expected.verify(username, password) {
            Some(expected.username.clone())
        } else {
            None
        }
    }

    pub fn authenticate_form(&self, cookie_header: Option<&str>) -> Option<String> {
        let _ = self.snapshot().form.as_ref()?;
        let token = parse_cookie(cookie_header, FORM_SESSION_COOKIE)?;
        let now = unix_now();
        let mut sessions = self.sessions.lock().ok()?;
        sessions.retain(|_, session| session.expires_at > now);
        let session = sessions.get_mut(&token)?;
        session.expires_at = now + FORM_SESSION_TTL_SECS;
        Some(session.username.clone())
    }

    pub fn verify_form_credentials(&self, username: &str, password: &str) -> bool {
        let state = self.snapshot();
        let Some(expected) = state.form.as_ref() else {
            return false;
        };
        expected.verify(username, password)
    }

    pub fn verify_any_password(&self, password: &str) -> bool {
        let state = self.snapshot();
        state
            .form
            .as_ref()
            .or(state.basic.as_ref())
            .is_some_and(|creds| verify_password(&creds.password_hash, password))
    }

    pub fn create_form_session(&self, username: &str) -> Option<String> {
        let state = self.snapshot();
        let expected = state.form.as_ref()?;
        if expected
            .username
            .as_bytes()
            .ct_eq(username.trim().as_bytes())
            .unwrap_u8()
            != 1
        {
            return None;
        }
        let token = generate_random_token();
        let mut sessions = self.sessions.lock().ok()?;
        sessions.insert(
            token.clone(),
            FormSession {
                username: expected.username.clone(),
                expires_at: unix_now() + FORM_SESSION_TTL_SECS,
            },
        );
        Some(token)
    }

    pub fn revoke_form_session(&self, cookie_header: Option<&str>) {
        let Some(token) = parse_cookie(cookie_header, FORM_SESSION_COOKIE) else {
            return;
        };
        if let Ok(mut sessions) = self.sessions.lock() {
            sessions.remove(&token);
        }
    }

    pub fn replace_with_config(&self, cfg: DashboardAuthConfig) {
        let basic = if cfg.is_usable() && cfg.basic_enabled {
            PasswordCredential::from_hash(&cfg.username, &cfg.password_hash)
        } else {
            None
        };
        let form = if cfg.is_usable() && cfg.form_enabled {
            PasswordCredential::from_hash(&cfg.username, &cfg.password_hash)
        } else {
            None
        };
        if let Ok(mut state) = self.state.lock() {
            state.source = if basic.is_some() || form.is_some() {
                AuthSource::Config
            } else {
                AuthSource::None
            };
            state.basic = basic;
            state.form = form;
        }
        self.clear_all_sessions();
    }

    pub fn disable(&self) {
        if let Ok(mut state) = self.state.lock() {
            state.source = AuthSource::None;
            state.basic = None;
            state.form = None;
        }
        self.clear_all_sessions();
    }

    pub fn session_cookie_header(&self, token: &str) -> String {
        let mut cookie = format!(
            "{FORM_SESSION_COOKIE}={token}; Path=/; HttpOnly; SameSite=Lax; Max-Age={FORM_SESSION_TTL_SECS}"
        );
        if self.tls_enabled {
            cookie.push_str("; Secure");
        }
        cookie
    }

    pub fn expired_session_cookie_header(&self) -> String {
        let mut cookie =
            format!("{FORM_SESSION_COOKIE}=; Path=/; HttpOnly; SameSite=Lax; Max-Age=0");
        if self.tls_enabled {
            cookie.push_str("; Secure");
        }
        cookie
    }

    fn clear_all_sessions(&self) {
        if let Ok(mut sessions) = self.sessions.lock() {
            sessions.clear();
        }
    }

    fn snapshot(&self) -> AuthState {
        self.state.lock().map(|s| s.clone()).unwrap_or(AuthState {
            source: AuthSource::None,
            basic: None,
            form: None,
        })
    }
}

fn parse_cookie(cookie_header: Option<&str>, name: &str) -> Option<String> {
    let header = cookie_header?;
    for part in header.split(';') {
        let trimmed = part.trim();
        // split_once preserves '=' inside the value; skip parts with no '='
        if let Some((cookie_name, cookie_value)) = trimmed.split_once('=')
            && cookie_name.trim() == name
        {
            return Some(cookie_value.to_string());
        }
    }
    None
}

fn hash_password(password: &str) -> Option<String> {
    let salt = SaltString::generate(&mut OsRng);
    Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .ok()
        .map(|hash| hash.to_string())
}

fn verify_password(password_hash: &str, password: &str) -> bool {
    let Ok(parsed) = PasswordHash::new(password_hash) else {
        return false;
    };
    Argon2::default()
        .verify_password(password.as_bytes(), &parsed)
        .is_ok()
}

fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
