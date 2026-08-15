use serde::Deserialize;
use std::sync::Arc;

use warp::Filter;

use crate::calibration::executor;
use crate::calibration::{
    StartCalibrationRequest,
    executor::{ApplyCalibrationRequest, RollbackCalibrationRequest},
};
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
        .or(api_list(config.clone()).map(box_reply))
        .unify()
        .or(api_resume(state.clone(), config.clone()).map(box_reply))
        .unify()
        .or(api_get(config.clone()).map(box_reply))
        .unify()
        .or(api_receipt(config.clone()).map(box_reply))
        .unify()
        .or(api_apply(state.clone(), config.clone()).map(box_reply))
        .unify()
        .or(api_rollback(state, config.clone()).map(box_reply))
        .unify()
        .or(api_cancel(config.clone()).map(box_reply))
        .unify()
        .or(api_forget(config).map(box_reply))
        .unify()
        .boxed()
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
struct ForgetCalibrationRequest {
    confirmation: String,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
struct ResumeCalibrationRequest {
    confirmation: String,
}

fn api_rollback(
    state: AppState,
    config: Arc<AppConfig>,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "calibrations" / String / "rollback")
        .and(warp::post())
        .and(warp::header::optional::<String>("authorization"))
        .and(safe_json_body::<RollbackCalibrationRequest>())
        .and(with_app_config(config))
        .and_then(
            move |id: String,
                  auth: Option<String>,
                  request: RollbackCalibrationRequest,
                  cfg: Arc<AppConfig>| {
                if !check_db_admin_token(&auth, &cfg) {
                    return futures_util::future::ready(Ok(unauthorized_db_admin_token()));
                }
                let response = match executor::rollback(&cfg, &state, &id, request) {
                    Ok(result) => warp::reply::with_status(
                        warp::reply::json(&serde_json::json!({"ok": true, "rollback": result})),
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
        let response = match executor::get_receipt_view(&cfg, &id) {
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
    let apply_state = state.clone();
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
                let state = apply_state.clone();
                async move {
                    if !check_db_admin_token(&auth, &cfg) {
                        return Ok::<Box<dyn warp::Reply>, warp::Rejection>(Box::new(
                            unauthorized_db_admin_token(),
                        ));
                    }
                    let response = match executor::apply_with_validation(&cfg, &state, &id, request)
                        .await
                    {
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
                    Ok::<Box<dyn warp::Reply>, warp::Rejection>(Box::new(response))
                }
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
 .and(safe_json_body::<executor::CalibrationPreflightRequest>())
 .and(with_app_config(config.clone()))
 .and_then(move |auth: Option<String>, body: executor::CalibrationPreflightRequest, cfg: Arc<AppConfig>| {
  let state = state.clone();
  async move {
  if !check_api_token(&auth, &cfg) {
   return Ok::<Box<dyn warp::Reply>, warp::Rejection>(Box::new(unauthorized_api_token()));
  }
  let response = match executor::preflight(&cfg, &state, &body.preset_id, &body.workload, body.budget) {
   Ok(mut preflight) => match executor::enrich_preflight_with_help(&cfg, &mut preflight).await {
    Ok(()) => warp::reply::with_status(
     warp::reply::json(&serde_json::json!({"ok": true, "preflight": preflight})),
     warp::http::StatusCode::OK,
    ),
    Err(error) => warp::reply::with_status(
     warp::reply::json(&serde_json::json!({"ok": false, "error": format!("Managed llama.cpp capability probe failed: {error}")})),
     warp::http::StatusCode::BAD_REQUEST,
    ),
   },
   Err(error) => warp::reply::with_status(
    warp::reply::json(&serde_json::json!({"ok": false, "error": error.to_string()})),
    warp::http::StatusCode::BAD_REQUEST,
   ),
  };
  Ok::<Box<dyn warp::Reply>, warp::Rejection>(Box::new(response))
  }
 })
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
                let state = state.clone();
                async move {
                    if !check_api_token(&auth, &cfg) {
                        return Ok::<Box<dyn warp::Reply>, warp::Rejection>(Box::new(
                            unauthorized_api_token(),
                        ));
                    }
                    let response = match executor::start(cfg, state, request).await {
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
                    Ok::<Box<dyn warp::Reply>, warp::Rejection>(Box::new(response))
                }
            },
        )
}

fn api_list(
    config: Arc<AppConfig>,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "calibrations")
        .and(warp::get())
        .and(warp::header::optional::<String>("authorization"))
        .and(with_app_config(config))
        .and_then(move |auth: Option<String>, cfg: Arc<AppConfig>| {
            if !check_api_token(&auth, &cfg) {
                return futures_util::future::ready(Ok(unauthorized_api_token()));
            }
            let response = match executor::list(&cfg) {
                Ok(jobs) => warp::reply::with_status(
                    warp::reply::json(&serde_json::json!({
                        "ok": true,
                        "jobs": jobs,
                    })),
                    warp::http::StatusCode::OK,
                ),
                Err(error) => warp::reply::with_status(
                    warp::reply::json(&serde_json::json!({
                        "ok": false,
                        "error": error.to_string(),
                    })),
                    warp::http::StatusCode::BAD_REQUEST,
                ),
            };
            futures_util::future::ready(Ok::<Box<dyn warp::Reply>, warp::Rejection>(Box::new(
                response,
            )))
        })
}

fn api_resume(
    state: AppState,
    config: Arc<AppConfig>,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "calibrations" / String / "resume")
        .and(warp::post())
        .and(warp::header::optional::<String>("authorization"))
        .and(safe_json_body::<ResumeCalibrationRequest>())
        .and(with_app_config(config))
        .and_then(
            move |id: String,
                  auth: Option<String>,
                  request: ResumeCalibrationRequest,
                  cfg: Arc<AppConfig>| {
                if !check_api_token(&auth, &cfg) {
                    return futures_util::future::ready(Ok(unauthorized_api_token()));
                }
                let response =
                    match executor::resume(cfg, state.clone(), &id, &request.confirmation) {
                        Ok(Some(snapshot)) => warp::reply::with_status(
                            warp::reply::json(&serde_json::json!({
                                "ok": true,
                                "job": snapshot,
                            })),
                            warp::http::StatusCode::ACCEPTED,
                        ),
                        Ok(None) => warp::reply::with_status(
                            warp::reply::json(&serde_json::json!({
                                "ok": false,
                                "error": "calibration job not found",
                            })),
                            warp::http::StatusCode::NOT_FOUND,
                        ),
                        Err(error) => warp::reply::with_status(
                            warp::reply::json(&serde_json::json!({
                                "ok": false,
                                "error": error.to_string(),
                            })),
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

fn api_forget(
    config: Arc<AppConfig>,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "calibrations" / String / "forget")
        .and(warp::post())
        .and(warp::header::optional::<String>("authorization"))
        .and(safe_json_body::<ForgetCalibrationRequest>())
        .and(with_app_config(config))
        .and_then(
            move |id: String,
                  auth: Option<String>,
                  request: ForgetCalibrationRequest,
                  cfg: Arc<AppConfig>| {
                if !check_db_admin_token(&auth, &cfg) {
                    return futures_util::future::ready(Ok(unauthorized_db_admin_token()));
                }
                let response = match executor::forget(&cfg, &id, &request.confirmation) {
                    Ok(true) => warp::reply::with_status(
                        warp::reply::json(&serde_json::json!({ "ok": true })),
                        warp::http::StatusCode::OK,
                    ),
                    Ok(false) => warp::reply::with_status(
                        warp::reply::json(&serde_json::json!({
                            "ok": false,
                            "error": "calibration job not found",
                        })),
                        warp::http::StatusCode::NOT_FOUND,
                    ),
                    Err(error) => warp::reply::with_status(
                        warp::reply::json(&serde_json::json!({
                            "ok": false,
                            "error": error.to_string(),
                        })),
                        warp::http::StatusCode::BAD_REQUEST,
                    ),
                };
                futures_util::future::ready(Ok::<Box<dyn warp::Reply>, warp::Rejection>(Box::new(
                    response,
                )))
            },
        )
}
