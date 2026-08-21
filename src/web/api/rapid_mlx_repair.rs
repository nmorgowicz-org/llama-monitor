//! Authenticated lifecycle API for local Rapid-MLX MTP repair jobs.

use serde_json::json;
use warp::Filter;
use warp::http::StatusCode;

use super::common::{ApiCtx, ApiReply, ApiRoute, check_api_token, unauthorized_api_token};
use crate::inference::rapid_mlx::repair::{self, RepairRequest};

pub(crate) fn routes(ctx: ApiCtx) -> ApiRoute {
    let start = start_route(ctx.clone());
    let requalification = requalification_route(ctx.clone());
    let list = list_route(ctx.clone());
    let status = status_route(ctx.clone());
    let cancel = cancel_route(ctx);
    start
        .or(requalification)
        .unify()
        .or(list)
        .unify()
        .or(status)
        .unify()
        .or(cancel)
        .unify()
        .boxed()
}

fn requalification_route(ctx: ApiCtx) -> ApiRoute {
    warp::path!("api" / "rapid-mlx" / "mtp-requalification")
        .and(warp::post())
        .and(warp::header::optional::<String>("authorization"))
        .and(super::super::safe_json_body::<RepairRequest>())
        .and_then(move |auth: Option<String>, mut request: RepairRequest| {
            let config = ctx.config.clone();
            async move {
                if !check_api_token(&auth, &config) {
                    return Ok::<ApiReply, warp::Rejection>(unauthorized_api_token());
                }
                request.operation = "requalify".into();
                match repair::start_job(request, &config.scripts_dir) {
                    Ok(snapshot) => Ok(Box::new(warp::reply::with_status(
                        warp::reply::json(&json!({ "ok": true, "job": snapshot })),
                        StatusCode::ACCEPTED,
                    )) as ApiReply),
                    Err(error) => Ok(Box::new(warp::reply::with_status(
                        warp::reply::json(&json!({ "ok": false, "error": error.to_string() })),
                        StatusCode::BAD_REQUEST,
                    )) as ApiReply),
                }
            }
        })
        .boxed()
}

fn start_route(ctx: ApiCtx) -> ApiRoute {
    warp::path!("api" / "rapid-mlx" / "mtp-repair")
        .and(warp::post())
        .and(warp::header::optional::<String>("authorization"))
        .and(super::super::safe_json_body::<RepairRequest>())
        .and_then(move |auth: Option<String>, request: RepairRequest| {
            let config = ctx.config.clone();
            async move {
                if !check_api_token(&auth, &config) {
                    return Ok::<ApiReply, warp::Rejection>(unauthorized_api_token());
                }
                match repair::start_job(request, &config.scripts_dir) {
                    Ok(snapshot) => Ok(Box::new(warp::reply::with_status(
                        warp::reply::json(&json!({ "ok": true, "job": snapshot })),
                        StatusCode::ACCEPTED,
                    )) as ApiReply),
                    Err(error) => Ok(Box::new(warp::reply::with_status(
                        warp::reply::json(&json!({ "ok": false, "error": error.to_string() })),
                        StatusCode::BAD_REQUEST,
                    )) as ApiReply),
                }
            }
        })
        .boxed()
}

fn list_route(ctx: ApiCtx) -> ApiRoute {
    warp::path!("api" / "rapid-mlx" / "mtp-repair")
        .and(warp::get())
        .and(warp::header::optional::<String>("authorization"))
        .and_then(move |auth: Option<String>| {
            let config = ctx.config.clone();
            async move {
                if !check_api_token(&auth, &config) {
                    return Ok::<ApiReply, warp::Rejection>(unauthorized_api_token());
                }
                match repair::list_sidecars() {
                    Ok(sidecars) => Ok(Box::new(warp::reply::json(&json!({
                        "ok": true,
                        "jobs": repair::list_jobs(),
                        "sidecars": sidecars,
                    }))) as ApiReply),
                    Err(error) => Ok(Box::new(warp::reply::with_status(
                        warp::reply::json(&json!({ "ok": false, "error": error.to_string() })),
                        StatusCode::INTERNAL_SERVER_ERROR,
                    )) as ApiReply),
                }
            }
        })
        .boxed()
}

fn status_route(ctx: ApiCtx) -> ApiRoute {
    warp::path!("api" / "rapid-mlx" / "mtp-repair" / String)
        .and(warp::get())
        .and(warp::header::optional::<String>("authorization"))
        .and_then(move |job_id: String, auth: Option<String>| {
            let config = ctx.config.clone();
            async move {
                if !check_api_token(&auth, &config) {
                    return Ok::<ApiReply, warp::Rejection>(unauthorized_api_token());
                }
                match repair::get_job(&job_id) {
                    Some(job) => Ok(
                        Box::new(warp::reply::json(&json!({ "ok": true, "job": job }))) as ApiReply,
                    ),
                    None => Ok(Box::new(warp::reply::with_status(
                        warp::reply::json(&json!({ "ok": false, "error": "repair job not found" })),
                        StatusCode::NOT_FOUND,
                    )) as ApiReply),
                }
            }
        })
        .boxed()
}

fn cancel_route(ctx: ApiCtx) -> ApiRoute {
    warp::path!("api" / "rapid-mlx" / "mtp-repair" / String / "cancel")
        .and(warp::post())
        .and(warp::header::optional::<String>("authorization"))
        .and_then(move |job_id: String, auth: Option<String>| {
            let config = ctx.config.clone();
            async move {
                if !check_api_token(&auth, &config) {
                    return Ok::<ApiReply, warp::Rejection>(unauthorized_api_token());
                }
                match repair::cancel_job(&job_id) {
                    Ok(job) => Ok(
                        Box::new(warp::reply::json(&json!({ "ok": true, "job": job }))) as ApiReply,
                    ),
                    Err(error) => Ok(Box::new(warp::reply::with_status(
                        warp::reply::json(&json!({ "ok": false, "error": error.to_string() })),
                        StatusCode::NOT_FOUND,
                    )) as ApiReply),
                }
            }
        })
        .boxed()
}
