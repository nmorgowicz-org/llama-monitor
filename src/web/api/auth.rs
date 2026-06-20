use warp::Filter;

use crate::web::auth::{AuthManager, AuthMethod, AuthSource};

#[cfg(not(test))]
use super::common::try_cooldown;
use super::{ApiReply, ApiRoute};

pub(crate) fn routes(auth_manager: AuthManager) -> ApiRoute {
    api_auth_status(auth_manager.clone())
        .or(api_auth_login(auth_manager.clone()))
        .unify()
        .or(api_auth_logout(auth_manager))
        .unify()
        .boxed()
}

fn api_auth_status(auth_manager: AuthManager) -> ApiRoute {
    warp::path!("api" / "auth" / "status")
        .and(warp::get())
        .and(warp::header::optional::<String>("Authorization"))
        .and(warp::header::optional::<String>("cookie"))
        .and_then(
            move |auth_header: Option<String>, cookie_header: Option<String>| {
                let auth_manager = auth_manager.clone();
                async move {
                    let status =
                        auth_manager.status(auth_header.as_deref(), cookie_header.as_deref());
                    let method = match status.method {
                        Some(AuthMethod::Basic) => Some("basic"),
                        Some(AuthMethod::Form) => Some("form"),
                        None => None,
                    };
                    Ok::<ApiReply, warp::Rejection>(Box::new(warp::reply::json(
                        &serde_json::json!({
                            "enabled": auth_manager.has_any(),
                            "methods": {
                                "basic": auth_manager.has_basic(),
                                "form": auth_manager.has_form(),
                            },
                            "managedByCli": matches!(auth_manager.source(), AuthSource::Cli),
                            "recoveryCommand": "llama-monitor --clear-auth-config",
                            "authenticated": status.authenticated,
                            "method": method,
                            "username": status.username,
                        }),
                    )))
                }
            },
        )
        .boxed()
}

fn api_auth_login(auth_manager: AuthManager) -> ApiRoute {
    #[derive(serde::Deserialize)]
    struct LoginRequest {
        username: String,
        password: String,
    }

    warp::path!("api" / "auth" / "login")
        .and(warp::post())
        .and(warp::body::content_length_limit(32 * 1024))
        .and(warp::body::json())
        .and_then(move |req: LoginRequest| {
            let auth_manager = auth_manager.clone();
            async move {
                #[cfg(not(test))]
                {
                    use std::sync::atomic::AtomicU64;
                    use std::time::{SystemTime, UNIX_EPOCH};

                    static LOGIN_LAST_ATTEMPT: AtomicU64 = AtomicU64::new(0);

                    let now = SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs();
                    let (ok, _remaining) = try_cooldown(&LOGIN_LAST_ATTEMPT, now, 10);
                    if !ok {
                        return Ok::<ApiReply, warp::Rejection>(Box::new(
                            warp::reply::with_status(
                                warp::reply::json(&serde_json::json!({
                                    "ok": false,
                                    "error": "too_many_login_attempts"
                                })),
                                warp::http::StatusCode::TOO_MANY_REQUESTS,
                            ),
                        ));
                    }
                }

                if !auth_manager.has_form() {
                    return Ok::<ApiReply, warp::Rejection>(Box::new(warp::reply::with_status(
                        warp::reply::json(&serde_json::json!({ "error": "form_auth_not_enabled" })),
                        warp::http::StatusCode::BAD_REQUEST,
                    )));
                }
                if !auth_manager.verify_form_credentials(&req.username, &req.password) {
                    return Ok::<ApiReply, warp::Rejection>(Box::new(warp::reply::with_status(
                        warp::reply::json(&serde_json::json!({ "error": "invalid_credentials" })),
                        warp::http::StatusCode::UNAUTHORIZED,
                    )));
                }
                let Some(token) = auth_manager.create_form_session(&req.username) else {
                    return Ok::<ApiReply, warp::Rejection>(Box::new(warp::reply::with_status(
                        warp::reply::json(&serde_json::json!({ "error": "form_auth_not_enabled" })),
                        warp::http::StatusCode::BAD_REQUEST,
                    )));
                };
                Ok::<ApiReply, warp::Rejection>(Box::new(warp::reply::with_header(
                    warp::reply::json(&serde_json::json!({ "ok": true })),
                    "Set-Cookie",
                    auth_manager.session_cookie_header(&token),
                )))
            }
        })
        .boxed()
}

fn api_auth_logout(auth_manager: AuthManager) -> ApiRoute {
    warp::path!("api" / "auth" / "logout")
        .and(warp::post())
        .and(warp::header::optional::<String>("cookie"))
        .and_then(move |cookie_header: Option<String>| {
            let auth_manager = auth_manager.clone();
            async move {
                auth_manager.revoke_form_session(cookie_header.as_deref());
                Ok::<ApiReply, warp::Rejection>(Box::new(warp::reply::with_header(
                    warp::reply::json(&serde_json::json!({ "ok": true })),
                    "Set-Cookie",
                    auth_manager.expired_session_cookie_header(),
                )))
            }
        })
        .boxed()
}

#[cfg(test)]
mod tests {
    use warp::Filter;

    use crate::config::TlsMode;
    use crate::web::auth::AuthManager;

    fn auth_routes_filter(
        auth_manager: AuthManager,
    ) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
        super::routes(auth_manager)
    }

    #[tokio::test]
    async fn form_auth_login_sets_session_cookie_and_status_reflects_it() {
        let auth = AuthManager::new(
            None,
            AuthManager::parse_credentials("admin:secret123"),
            &TlsMode::None,
        );
        let routes = auth_routes_filter(auth);

        let login_resp = warp::test::request()
            .method("POST")
            .path("/api/auth/login")
            .json(&serde_json::json!({
                "username": "admin",
                "password": "secret123",
            }))
            .reply(&routes)
            .await;

        assert_eq!(login_resp.status(), 200);
        let set_cookie = login_resp
            .headers()
            .get("set-cookie")
            .and_then(|value| value.to_str().ok())
            .expect("set-cookie header");
        assert!(set_cookie.contains("llama_monitor_session="));

        let status_resp = warp::test::request()
            .method("GET")
            .path("/api/auth/status")
            .header("cookie", set_cookie)
            .reply(&routes)
            .await;

        assert_eq!(status_resp.status(), 200);
        let body: serde_json::Value =
            serde_json::from_slice(status_resp.body()).expect("valid JSON");
        assert_eq!(body["enabled"], true);
        assert_eq!(body["methods"]["form"], true);
        assert_eq!(body["authenticated"], true);
        assert_eq!(body["method"], "form");
        assert_eq!(body["username"], "admin");
    }

    #[tokio::test]
    async fn form_auth_logout_clears_session_cookie() {
        let auth = AuthManager::new(
            None,
            AuthManager::parse_credentials("admin:secret123"),
            &TlsMode::None,
        );
        let routes = auth_routes_filter(auth);

        let login_resp = warp::test::request()
            .method("POST")
            .path("/api/auth/login")
            .json(&serde_json::json!({
                "username": "admin",
                "password": "secret123",
            }))
            .reply(&routes)
            .await;

        let set_cookie = login_resp
            .headers()
            .get("set-cookie")
            .and_then(|value| value.to_str().ok())
            .expect("set-cookie header");

        let logout_resp = warp::test::request()
            .method("POST")
            .path("/api/auth/logout")
            .header("cookie", set_cookie)
            .reply(&routes)
            .await;

        assert_eq!(logout_resp.status(), 200);
        let clear_cookie = logout_resp
            .headers()
            .get("set-cookie")
            .and_then(|value| value.to_str().ok())
            .expect("set-cookie clear header");
        assert!(clear_cookie.contains("Max-Age=0"));
    }
}
