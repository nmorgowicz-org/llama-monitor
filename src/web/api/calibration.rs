use std::sync::Arc;

use warp::Filter;

use crate::calibration::executor;
use crate::calibration::{StartCalibrationRequest, executor::ApplyCalibrationRequest};
use crate::config::AppConfig;
use crate::state::AppState;
use crate::web::safe_json_body;

use super::{
    ApiCtx, ApiRoute, box_reply, check_api_token, check_db_admin_token, unauthorized_api_token,
    unauthorized_db_admin_token, with_app_config,
};

pub(crate) fn routes(ctx: ApiCtx) -> ApiRoute {
    let state = ctx.state;
    let config = ctx.config;

    api_preflight(state.clone(), config.clone())
        .map(box_reply)
        .or(api_start(state.clone(), config.clone()).map(box_reply))
        .unify()
        .or(api_get(config.clone()).map(box_reply))
        .unify()
        .or(api_receipt(config.clone()).map(box_reply))
        .unify()
        .or(api_apply(state.clone(), config.clone()).map(box_reply))
        .unify()
        .or(api_cancel(config).map(box_reply))
        .unify()
        .boxed()
}

fn api_receipt(
    config: Arc<AppConfig>,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "calibrations" / String / "receipt")
        .and(warp::get())
        .and(warp::header::optional::<String>("authorization"))
        .and(with_app_config(config))
        .and_then(move |id: String, auth: Option<String>, cfg: Arc<AppConfig>| {
            if !check_api_token(&auth, &cfg) {
                return futures_util::future::ready(Ok(unauthorized_api_token()));
            }
            let response = match executor::get_receipt(&cfg, &id) {
                Ok(Some(receipt)) => warp::reply::with_status(
                    warp::reply::json(&serde_json::json!({"ok": true, "receipt": receipt})),
                    warp::http::StatusCode::OK,
                ),
                Ok(None) => warp::reply::with_status(
                    warp::reply::json(&serde_json::json!({"ok": false, "error": "calibration receipt not found"})),
                    warp::http::StatusCode::NOT_FOUND,
                ),
                Err(error) => warp::reply::with_status(
                    warp::reply::json(&serde_json::json!({"ok": false, "error": error.to_string()})),
                    warp::http::StatusCode::BAD_REQUEST,
                ),
            };
            futures_util::future::ready(Ok::<Box<dyn warp::Reply>, warp::Rejection>(Box::new(response)))
        })
}

fn api_apply(
    state: AppState,
    config: Arc<AppConfig>,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "calibrations" / String / "apply")
        .and(warp::post())
        .and(warp::header::optional::<String>("authorization"))
        .and(safe_json_body::<ApplyCalibrationRequest>())
        .and(with_app_config(config))
        .and_then(
            move |id: String,
                  auth: Option<String>,
                  request: ApplyCalibrationRequest,
                  cfg: Arc<AppConfig>| {
                if !check_db_admin_token(&auth, &cfg) {
                    return futures_util::future::ready(Ok(unauthorized_db_admin_token()));
                }
                let response = match executor::apply(&cfg, &state, &id, request) {
                    Ok(result) => warp::reply::with_status(
                        warp::reply::json(&serde_json::json!({"ok": true, "apply": result})),
                        warp::http::StatusCode::OK,
                    ),
                    Err(error) => warp::reply::with_status(
                        warp::reply::json(
                            &serde_json::json!({"ok": false, "error": error.to_string()}),
                        ),
                        warp::http::StatusCode::BAD_REQUEST,
                    ),
                };
                futures_util::future::ready(Ok::<Box<dyn warp::Reply>, warp::Rejection>(Box::new(
                    response,
                )))
            },
        )
}

fn api_preflight(
    state: AppState,
    config: Arc<AppConfig>,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "calibrations" / "preflight")
        .and(warp::post())
        .and(warp::header::optional::<String>("authorization"))
        .and(safe_json_body::<serde_json::Value>())
        .and(with_app_config(config.clone()))
        .and_then(
            move |auth: Option<String>, body: serde_json::Value, cfg: Arc<AppConfig>| {
                if !check_api_token(&auth, &cfg) {
                    return futures_util::future::ready(Ok(unauthorized_api_token()));
                }
                let preset_id = body
                    .get("preset_id")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or_default()
                    .to_string();
                let response = match executor::preflight(&cfg, &state, &preset_id) {
                    Ok(preflight) => warp::reply::with_status(
                        warp::reply::json(&serde_json::json!({"ok": true, "preflight": preflight})),
                        warp::http::StatusCode::OK,
                    ),
                    Err(error) => warp::reply::with_status(
                        warp::reply::json(
                            &serde_json::json!({"ok": false, "error": error.to_string()}),
                        ),
                        warp::http::StatusCode::BAD_REQUEST,
                    ),
                };
                futures_util::future::ready(Ok::<Box<dyn warp::Reply>, warp::Rejection>(Box::new(
                    response,
                )))
            },
        )
}

fn api_start(
    state: AppState,
    config: Arc<AppConfig>,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "calibrations")
        .and(warp::post())
        .and(warp::header::optional::<String>("authorization"))
        .and(safe_json_body::<StartCalibrationRequest>())
        .and(with_app_config(config.clone()))
        .and_then(
            move |auth: Option<String>, request: StartCalibrationRequest, cfg: Arc<AppConfig>| {
                if !check_api_token(&auth, &cfg) {
                    return futures_util::future::ready(Ok(unauthorized_api_token()));
                }
                let response = match executor::start(cfg, state.clone(), request) {
                    Ok(snapshot) => warp::reply::with_status(
                        warp::reply::json(&serde_json::json!({"ok": true, "job": snapshot})),
                        warp::http::StatusCode::ACCEPTED,
                    ),
                    Err(error) => warp::reply::with_status(
                        warp::reply::json(
                            &serde_json::json!({"ok": false, "error": error.to_string()}),
                        ),
                        warp::http::StatusCode::BAD_REQUEST,
                    ),
                };
                futures_util::future::ready(Ok::<Box<dyn warp::Reply>, warp::Rejection>(Box::new(
                    response,
                )))
            },
        )
}

fn api_get(
    config: Arc<AppConfig>,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "calibrations" / String)
        .and(warp::get())
        .and(warp::header::optional::<String>("authorization"))
        .and(with_app_config(config))
        .and_then(
            move |id: String, auth: Option<String>, cfg: Arc<AppConfig>| {
                if !check_api_token(&auth, &cfg) {
                    return futures_util::future::ready(Ok(unauthorized_api_token()));
                }
                let response = match executor::get(&cfg, &id) {
                    Ok(Some(snapshot)) => warp::reply::with_status(
                        warp::reply::json(&serde_json::json!({"ok": true, "job": snapshot})),
                        warp::http::StatusCode::OK,
                    ),
                    Ok(None) => warp::reply::with_status(
                        warp::reply::json(
                            &serde_json::json!({"ok": false, "error": "calibration job not found"}),
                        ),
                        warp::http::StatusCode::NOT_FOUND,
                    ),
                    Err(error) => warp::reply::with_status(
                        warp::reply::json(
                            &serde_json::json!({"ok": false, "error": error.to_string()}),
                        ),
                        warp::http::StatusCode::BAD_REQUEST,
                    ),
                };
                futures_util::future::ready(Ok::<Box<dyn warp::Reply>, warp::Rejection>(Box::new(
                    response,
                )))
            },
        )
}

fn api_cancel(
    config: Arc<AppConfig>,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "calibrations" / String / "cancel")
        .and(warp::post())
        .and(warp::header::optional::<String>("authorization"))
        .and(with_app_config(config))
        .and_then(
            move |id: String, auth: Option<String>, cfg: Arc<AppConfig>| {
                if !check_api_token(&auth, &cfg) {
                    return futures_util::future::ready(Ok(unauthorized_api_token()));
                }
                let response = match executor::cancel(&cfg, &id) {
                    Ok(Some(snapshot)) => warp::reply::with_status(
                        warp::reply::json(&serde_json::json!({"ok": true, "job": snapshot})),
                        warp::http::StatusCode::ACCEPTED,
                    ),
                    Ok(None) => warp::reply::with_status(
                        warp::reply::json(
                            &serde_json::json!({"ok": false, "error": "calibration job not found"}),
                        ),
                        warp::http::StatusCode::NOT_FOUND,
                    ),
                    Err(error) => warp::reply::with_status(
                        warp::reply::json(
                            &serde_json::json!({"ok": false, "error": error.to_string()}),
                        ),
                        warp::http::StatusCode::BAD_REQUEST,
                    ),
                };
                futures_util::future::ready(Ok::<Box<dyn warp::Reply>, warp::Rejection>(Box::new(
                    response,
                )))
            },
        )
}
