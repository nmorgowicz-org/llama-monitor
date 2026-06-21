use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicU64;
use std::time::{SystemTime, UNIX_EPOCH};

use warp::Filter;

use crate::acme::{acme_renew_cert, acme_request_cert};
use crate::config::{AcmeConfig, AppConfig, TLSConfig, TlsMode};
use crate::state::AppState;

use super::common::{bearer_matches_api_token, try_cooldown, with_app_config};
use super::{ApiCtx, ApiReply, ApiRoute, check_api_token, unauthorized_api_token};

pub(crate) fn routes(ctx: ApiCtx) -> ApiRoute {
    let state = ctx.state;
    let config = ctx.config;

    api_get_tls_config(state.clone(), config.clone())
        .or(api_put_tls_config(state.clone(), config.clone()))
        .unify()
        .or(api_tls_acme_request(state.clone(), config.clone()))
        .unify()
        .or(api_tls_acme_renew(state, config))
        .unify()
        .boxed()
}

/// GET /api/tls/config — returns current TLS configuration (non-sensitive).
fn api_get_tls_config(state: AppState, app_config: Arc<AppConfig>) -> ApiRoute {
    let app_config = app_config.clone();

    warp::path!("api" / "tls" / "config")
        .and(warp::get())
        .and(warp::header::optional::<String>("authorization"))
        .and(with_app_config(app_config))
        .and_then(move |auth: Option<String>, cfg: Arc<AppConfig>| {
            let state = state.clone();
            async move {
                let bearer = auth.and_then(|v| v.strip_prefix("Bearer ").map(str::to_string));
                let has_api_token = bearer_matches_api_token(bearer.as_deref(), &cfg);

                if !has_api_token {
                    return Ok::<ApiReply, warp::Rejection>(Box::new(warp::reply::with_status(
                        warp::reply::json(&serde_json::json!({ "error": "unauthorized" })),
                        warp::http::StatusCode::UNAUTHORIZED,
                    )));
                }

                let tls_cfg = state.get_tls_config();

                let mode_str = match tls_cfg.mode {
                    TlsMode::None => "none",
                    TlsMode::SelfSigned => "self-signed",
                    TlsMode::Custom => "custom",
                    TlsMode::Acme => "acme",
                };

                // Build a safe ACME summary (no secrets).
                let acme_summary: serde_json::Value = if matches!(tls_cfg.mode, TlsMode::Acme) {
                    serde_json::json!({
                        "enabled": tls_cfg.acme.enabled,
                        "fqdn": tls_cfg.acme.fqdn,
                        "environment": tls_cfg.acme.environment,
                        "dnsProvider": tls_cfg.acme.dns_provider,
                        "validationDelay": tls_cfg.acme.validation_delay,
                        "lastRenewal": tls_cfg.acme.last_renewal,
                        "certPath": tls_cfg.acme.cert_path.as_ref().map(|p| p.to_string_lossy().to_string()),
                        "keyPath": tls_cfg.acme.key_path.as_ref().map(|p| p.to_string_lossy().to_string()),
                    })
                } else {
                    serde_json::json!({
                        "enabled": tls_cfg.acme.enabled,
                    })
                };

                Ok::<ApiReply, warp::Rejection>(Box::new(warp::reply::json(&serde_json::json!({
                    "mode": mode_str,
                    "customCertPath": tls_cfg.custom_cert_path.as_ref().map(|p| p.to_string_lossy().to_string()),
                    "customKeyPath": tls_cfg.custom_key_path.as_ref().map(|p| p.to_string_lossy().to_string()),
                    "acme": acme_summary,
                }))))
            }
        })
        .boxed()
}

/// PUT /api/tls/config — update TLS configuration (requires api-token).
fn api_put_tls_config(state: AppState, app_config: Arc<AppConfig>) -> ApiRoute {
    warp::path!("api" / "tls" / "config")
        .and(warp::put())
        .and(warp::header::optional::<String>("Authorization"))
        .and(warp::body::json())
        .and_then(
            move |auth_header: Option<String>, body: serde_json::Value| {
                let state = state.clone();
                let app_config = app_config.clone();
                async move {
                    if !check_api_token(&auth_header, &app_config) {
                        return Ok(unauthorized_api_token());
                    }

                    // Extract mode
                    let mode_str = body.get("mode").and_then(|v| v.as_str()).unwrap_or("none");

                    let mode = match mode_str {
                        "none" => TlsMode::None,
                        "self-signed" => TlsMode::SelfSigned,
                        "custom" => TlsMode::Custom,
                        "acme" => TlsMode::Acme,
                        _ => {
                            return Ok::<ApiReply, warp::Rejection>(Box::new(warp::reply::with_status(
                                warp::reply::json(&serde_json::json!({
                                    "ok": false,
                                    "error": format!("Invalid mode: {}", mode_str)
                                })),
                                warp::http::StatusCode::BAD_REQUEST,
                            )));
                        }
                    };

                    // For custom mode, validate cert/key paths
                    if mode == TlsMode::Custom {
                        let cert_path_str = body.get("customCertPath").and_then(|v| v.as_str());
                        let key_path_str = body.get("customKeyPath").and_then(|v| v.as_str());

                        if cert_path_str.is_none() || key_path_str.is_none() {
                            return Ok::<ApiReply, warp::Rejection>(Box::new(warp::reply::with_status(
                                warp::reply::json(&serde_json::json!({
                                    "ok": false,
                                    "error": "custom mode requires customCertPath and customKeyPath"
                                })),
                                warp::http::StatusCode::BAD_REQUEST,
                            )));
                        }
                    }

                    // Build ACME config from request (or keep existing if not acme mode)
                    let existing = state.get_tls_config();
                    let acme_cfg = if mode == TlsMode::Acme {
                        // Read acme fields from body
                        let acme_obj = body.get("acme").and_then(|v| v.as_object());

                        let enabled = acme_obj
                            .and_then(|o| o.get("enabled").and_then(|v| v.as_bool()))
                            .unwrap_or(true);

                        let fqdn = acme_obj
                            .and_then(|o| o.get("fqdn").and_then(|v| v.as_str()))
                            .unwrap_or("")
                            .to_string();

                        let environment = acme_obj
                            .and_then(|o| o.get("environment").and_then(|v| v.as_str()))
                            .unwrap_or("staging")
                            .to_string();

                        let dns_provider = acme_obj
                            .and_then(|o| o.get("dnsProvider").and_then(|v| v.as_str()))
                            .unwrap_or("")
                            .to_string();

                        let validation_delay = acme_obj
                            .and_then(|o| o.get("validationDelay").and_then(|v| v.as_u64()))
                            .unwrap_or(300);

                        // Parse dnsConfig as a map
                        let dns_config: HashMap<String, String> = acme_obj
                            .and_then(|o| o.get("dnsConfig").and_then(|v| v.as_object()))
                            .map(|map| {
                                map.iter()
                                    .filter_map(|(k, v)| {
                                        v.as_str().map(|s| (k.clone(), s.to_string()))
                                    })
                                    .collect()
                            })
                            .unwrap_or_default();

                        // Validate ACME fields
                        if fqdn.is_empty() {
                            return Ok::<ApiReply, warp::Rejection>(Box::new(warp::reply::with_status(
                                warp::reply::json(&serde_json::json!({
                                    "ok": false,
                                    "error": "acme mode requires acme.fqdn"
                                })),
                                warp::http::StatusCode::BAD_REQUEST,
                            )));
                        }

                        if environment != "staging" && environment != "production" {
                            return Ok::<ApiReply, warp::Rejection>(Box::new(warp::reply::with_status(
                                warp::reply::json(&serde_json::json!({
                                    "ok": false,
                                    "error": "acme.environment must be 'staging' or 'production'"
                                })),
                                warp::http::StatusCode::BAD_REQUEST,
                            )));
                        }

                        if dns_provider.is_empty() {
                            return Ok::<ApiReply, warp::Rejection>(Box::new(warp::reply::with_status(
                                warp::reply::json(&serde_json::json!({
                                    "ok": false,
                                    "error": "acme mode requires acme.dnsProvider"
                                })),
                                warp::http::StatusCode::BAD_REQUEST,
                            )));
                        }

                        if dns_config.is_empty() {
                            return Ok::<ApiReply, warp::Rejection>(Box::new(warp::reply::with_status(
                                warp::reply::json(&serde_json::json!({
                                    "ok": false,
                                    "error": "acme mode requires acme.dnsConfig"
                                })),
                                warp::http::StatusCode::BAD_REQUEST,
                            )));
                        }

                        let email = acme_obj
                            .and_then(|o| o.get("email").and_then(|v| v.as_str()))
                            .unwrap_or("")
                            .to_string();

                        AcmeConfig {
                            enabled,
                            fqdn,
                            email,
                            environment,
                            dns_provider,
                            dns_config,
                            validation_delay,
                            last_renewal: existing.acme.last_renewal.clone(),
                            cert_path: existing.acme.cert_path.clone(),
                            key_path: existing.acme.key_path.clone(),
                        }
                    } else {
                        existing.acme
                    };

                    let new_cfg = TLSConfig {
                        mode,
                        custom_cert_path: body
                            .get("customCertPath")
                            .and_then(|v| v.as_str())
                            .map(PathBuf::from),
                        custom_key_path: body
                            .get("customKeyPath")
                            .and_then(|v| v.as_str())
                            .map(PathBuf::from),
                        acme: acme_cfg,
                    };

                    // Update in-memory state
                    state.set_tls_config(new_cfg.clone());

                    // Persist to disk (restart required to apply)
                    if let Err(e) = crate::config::save_tls_config(&app_config.config_dir, &new_cfg)
                    {
                        eprintln!("[error] Failed to save tls-config.json: {}", e);
                    }

                    Ok::<ApiReply, warp::Rejection>(Box::new(warp::reply::with_status(
                        warp::reply::json(&serde_json::json!({
                            "ok": true,
                            "requires_restart": true
                        })),
                        warp::http::StatusCode::OK,
                    )))
                }
            },
        )
        .boxed()
}

/// POST /api/tls/acme/request — trigger ACME certificate request (requires api-token).
fn api_tls_acme_request(state: AppState, app_config: Arc<AppConfig>) -> ApiRoute {
    static LAST_TLS_ACME_REQUEST: AtomicU64 = AtomicU64::new(0);

    warp::path!("api" / "tls" / "acme" / "request")
        .and(warp::post())
        .and(warp::header::optional::<String>("Authorization"))
        .and_then(move |auth_header: Option<String>| {
            let state = state.clone();
            let app_config = app_config.clone();
            async move {
                if !check_api_token(&auth_header, &app_config) {
                    return Ok(unauthorized_api_token());
                }

                let now = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs();
                let (ok, remaining) = try_cooldown(&LAST_TLS_ACME_REQUEST, now, 60);
                if !ok {
                    return Ok::<ApiReply, warp::Rejection>(Box::new(warp::reply::with_status(
                        warp::reply::json(&serde_json::json!({
                            "ok": false,
                            "error": "too soon; please wait",
                            "seconds_remaining": remaining
                        })),
                        warp::http::StatusCode::TOO_MANY_REQUESTS,
                    )));
                }

                let cfg = state.get_tls_config();
                let config_dir = app_config.config_dir.clone();

                match acme_request_cert(&config_dir, &cfg) {
                    Ok(new_cfg) => {
                        eprintln!("[info] ACME certificate request succeeded");
                        state.set_tls_config(new_cfg.clone());
                        if let Err(e) = crate::config::save_tls_config(&config_dir, &new_cfg) {
                            eprintln!(
                                "[error] Failed to save tls-config.json after ACME request: {}",
                                e
                            );
                        }
                        Ok::<ApiReply, warp::Rejection>(Box::new(warp::reply::with_status(
                            warp::reply::json(&serde_json::json!({
                                "ok": true,
                                "requires_restart": true
                            })),
                            warp::http::StatusCode::OK,
                        )))
                    }
                    Err(e) => {
                        eprintln!("[error] ACME certificate request failed: {}", e);
                        Ok::<ApiReply, warp::Rejection>(Box::new(warp::reply::with_status(
                            warp::reply::json(&serde_json::json!({
                                "ok": false,
                                "error": e
                            })),
                            warp::http::StatusCode::BAD_REQUEST,
                        )))
                    }
                }
            }
        })
        .boxed()
}

/// POST /api/tls/acme/renew — trigger ACME certificate renewal (requires api-token).
fn api_tls_acme_renew(state: AppState, app_config: Arc<AppConfig>) -> ApiRoute {
    static LAST_TLS_ACME_RENEW: AtomicU64 = AtomicU64::new(0);

    warp::path!("api" / "tls" / "acme" / "renew")
        .and(warp::post())
        .and(warp::header::optional::<String>("Authorization"))
        .and_then(move |auth_header: Option<String>| {
            let state = state.clone();
            let app_config = app_config.clone();
            async move {
                if !check_api_token(&auth_header, &app_config) {
                    return Ok(unauthorized_api_token());
                }

                let now = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs();
                let (ok, remaining) = try_cooldown(&LAST_TLS_ACME_RENEW, now, 60);
                if !ok {
                    return Ok::<ApiReply, warp::Rejection>(Box::new(warp::reply::with_status(
                        warp::reply::json(&serde_json::json!({
                            "ok": false,
                            "error": "too soon; please wait",
                            "seconds_remaining": remaining
                        })),
                        warp::http::StatusCode::TOO_MANY_REQUESTS,
                    )));
                }

                let cfg = state.get_tls_config();
                let config_dir = app_config.config_dir.clone();

                match acme_renew_cert(&config_dir, &cfg) {
                    Ok(new_cfg) => {
                        eprintln!("[info] ACME renewal succeeded (manual)");
                        state.set_tls_config(new_cfg.clone());
                        if let Err(e) = crate::config::save_tls_config(&config_dir, &new_cfg) {
                            eprintln!(
                                "[error] Failed to save tls-config.json after ACME renewal: {}",
                                e
                            );
                        }
                        Ok::<ApiReply, warp::Rejection>(Box::new(warp::reply::with_status(
                            warp::reply::json(&serde_json::json!({
                                "ok": true,
                                "requires_restart": true
                            })),
                            warp::http::StatusCode::OK,
                        )))
                    }
                    Err(e) => {
                        eprintln!("[error] ACME renewal failed: {}", e);
                        Ok::<ApiReply, warp::Rejection>(Box::new(warp::reply::with_status(
                            warp::reply::json(&serde_json::json!({
                                "ok": false,
                                "error": e
                            })),
                            warp::http::StatusCode::BAD_REQUEST,
                        )))
                    }
                }
            }
        })
        .boxed()
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::path::PathBuf;
    use std::sync::Arc;
    use warp::Filter;

    use crate::chat_storage::ChatStorage;
    use crate::config::{AcmeConfig, AppConfig, TLSConfig, TlsMode};
    use crate::gpu::env::GpuEnv;
    use crate::state::{AppPaths, AppState};

    use super::ApiCtx;

    fn make_test_app_state(tls_config: TLSConfig) -> (AppState, Arc<AppConfig>) {
        let paths = AppPaths {
            presets_path: PathBuf::new(),
            templates_path: PathBuf::new(),
            models_dir: None,
            gpu_env_path: PathBuf::new(),
            ui_settings_path: PathBuf::new(),
            sessions_path: PathBuf::new(),
            model_tags_path: PathBuf::new(),
        };
        let cs = Arc::new(
            ChatStorage::open(&PathBuf::from(":memory:")).expect("open in-memory chat storage"),
        );
        let state = AppState::new(
            vec![],
            paths,
            GpuEnv::default(),
            crate::state::UiSettings::default(),
            cs,
            tls_config,
        );
        let app_config = Arc::new(AppConfig::for_test(Some("test-token".to_string()), None));
        (state, app_config)
    }

    fn tls_routes_filter(
        state: AppState,
        app_config: Arc<AppConfig>,
    ) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
        super::routes(ApiCtx {
            state,
            config: app_config,
            auth: crate::web::auth::AuthManager::new(None, None, &TlsMode::None),
        })
    }

    #[tokio::test]
    async fn tls_config_get_requires_api_token() {
        let (state, app_config) = make_test_app_state(TLSConfig::default());
        let routes = tls_routes_filter(state, app_config);

        // Without token -> 401
        let resp = warp::test::request()
            .method("GET")
            .path("/api/tls/config")
            .reply(&routes)
            .await;
        assert_eq!(resp.status(), 401);

        // With correct token -> 200
        let resp = warp::test::request()
            .method("GET")
            .path("/api/tls/config")
            .header("Authorization", "Bearer test-token")
            .reply(&routes)
            .await;
        assert_eq!(resp.status(), 200);
        let body: serde_json::Value = serde_json::from_slice(resp.body()).expect("valid JSON");
        assert_eq!(body["mode"], "none");
    }

    #[tokio::test]
    async fn tls_config_get_returns_acme_fields() {
        let mut dns_config = HashMap::new();
        dns_config.insert("CF_API_TOKEN".to_string(), "redacted".to_string());

        let tls_config = TLSConfig {
            mode: TlsMode::Acme,
            custom_cert_path: None,
            custom_key_path: None,
            acme: AcmeConfig {
                enabled: true,
                fqdn: "llama-monitor.example.com".to_string(),
                email: String::new(),
                environment: "staging".to_string(),
                dns_provider: "cloudflare".to_string(),
                dns_config,
                validation_delay: 300,
                last_renewal: None,
                cert_path: None,
                key_path: None,
            },
        };

        let (state, app_config) = make_test_app_state(tls_config);
        let routes = tls_routes_filter(state, app_config);

        let resp = warp::test::request()
            .method("GET")
            .path("/api/tls/config")
            .header("Authorization", "Bearer test-token")
            .reply(&routes)
            .await;

        assert_eq!(resp.status(), 200);
        let body: serde_json::Value = serde_json::from_slice(resp.body()).expect("valid JSON");
        assert_eq!(body["mode"], "acme");
        assert_eq!(body["acme"]["fqdn"], "llama-monitor.example.com");
        assert_eq!(body["acme"]["environment"], "staging");
        assert_eq!(body["acme"]["dnsProvider"], "cloudflare");
    }

    #[tokio::test]
    async fn tls_config_put_accepts_valid_acme() {
        let (state, app_config) = make_test_app_state(TLSConfig::default());
        let routes = tls_routes_filter(state.clone(), app_config);

        let payload = serde_json::json!({
            "mode": "acme",
            "acme": {
                "enabled": true,
                "fqdn": "llama-monitor.example.com",
                "environment": "staging",
                "dnsProvider": "cloudflare",
                "validationDelay": 300,
                "dnsConfig": {
                    "CF_API_TOKEN": "test-token"
                }
            }
        });

        let resp = warp::test::request()
            .method("PUT")
            .path("/api/tls/config")
            .header("Authorization", "Bearer test-token")
            .json(&payload)
            .reply(&routes)
            .await;

        assert_eq!(resp.status(), 200);
        let body: serde_json::Value = serde_json::from_slice(resp.body()).expect("valid JSON");
        assert_eq!(body["ok"], true);

        // Verify TLSConfig was updated in state
        let cfg = state.get_tls_config();
        assert_eq!(cfg.mode, TlsMode::Acme);
        assert_eq!(cfg.acme.fqdn, "llama-monitor.example.com");
        assert_eq!(cfg.acme.dns_provider, "cloudflare");
    }

    #[tokio::test]
    async fn tls_config_put_rejects_invalid_acme_missing_provider() {
        let (state, app_config) = make_test_app_state(TLSConfig::default());
        let routes = tls_routes_filter(state, app_config);

        let payload = serde_json::json!({
            "mode": "acme",
            "acme": {
                "enabled": true,
                "fqdn": "llama-monitor.example.com",
                "environment": "staging",
                "dnsProvider": "",
                "dnsConfig": {
                    "CF_API_TOKEN": "test-token"
                }
            }
        });

        let resp = warp::test::request()
            .method("PUT")
            .path("/api/tls/config")
            .header("Authorization", "Bearer test-token")
            .json(&payload)
            .reply(&routes)
            .await;

        assert_eq!(resp.status(), 400);
        let body: serde_json::Value = serde_json::from_slice(resp.body()).expect("valid JSON");
        assert!(
            body["error"]
                .as_str()
                .map(|s| s.contains("dnsProvider"))
                .unwrap_or(false)
        );
    }
}
