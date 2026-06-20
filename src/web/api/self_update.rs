use std::sync::Arc;
use std::sync::atomic::AtomicU64;
use std::time::{SystemTime, UNIX_EPOCH};

use warp::Filter;

use crate::config::AppConfig;

use super::common::{ApiCtx, ApiRoute, box_reply, bearer_matches_db_admin_token, try_cooldown};
use super::with_app_config;

static LAST_UPDATE: AtomicU64 = AtomicU64::new(0);

pub(crate) fn routes(ctx: ApiCtx) -> ApiRoute {
    let config = ctx.config;

    api_self_update(config).map(box_reply).boxed()
}

fn api_self_update(
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    let app_config = app_config.clone();

    warp::path!("api" / "self-update")
        .and(warp::post())
        .and(warp::header::optional::<String>("authorization"))
        .and(warp::body::json::<serde_json::Value>())
        .and(with_app_config(app_config))
        .and_then(
            move |auth: Option<String>, body: serde_json::Value, cfg: Arc<AppConfig>| async move {
                let bearer = auth.and_then(|v| v.strip_prefix("Bearer ").map(str::to_string));
                let has_admin_token = bearer_matches_db_admin_token(bearer.as_deref(), &cfg);

                if !has_admin_token {
                    return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                        warp::reply::with_status(
                            warp::reply::json(&serde_json::json!({
                                "error": "unauthorized; db-admin-token required"
                            })),
                            warp::http::StatusCode::UNAUTHORIZED,
                        ),
                    ));
                }

                let confirm = body.get("confirm").and_then(|v| v.as_str()).unwrap_or("");
                if confirm != "update" {
                    return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                        warp::reply::with_status(
                            warp::reply::json(&serde_json::json!({
                                "error": "missing confirmation; send { \"confirm\": \"update\" }"
                            })),
                            warp::http::StatusCode::BAD_REQUEST,
                        ),
                    ));
                }

                let now = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs();
                let (ok, remaining) = try_cooldown(&LAST_UPDATE, now, 300);
                if !ok {
                    return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                        warp::reply::with_status(
                            warp::reply::json(&serde_json::json!({
                                "error": "too soon; please wait",
                                "seconds_remaining": remaining
                            })),
                            warp::http::StatusCode::TOO_MANY_REQUESTS,
                        ),
                    ));
                }

                match crate::agent::self_update_binary().await {
                    Ok(result) => {
                        tokio::spawn(async {
                            tokio::time::sleep(std::time::Duration::from_millis(600)).await;
                            std::process::exit(0);
                        });
                        Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                            warp::reply::json(&serde_json::json!({
                                "ok": true,
                                "tag_name": result.tag_name,
                                "restart_required": true
                            })),
                        ))
                    }
                    Err(e) => Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                        warp::reply::json(&serde_json::json!({
                            "ok": false,
                            "error": e.to_string()
                        })),
                    )),
                }
            },
        )
}
