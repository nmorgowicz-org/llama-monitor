use warp::Filter;

use super::{ApiCtx, ApiReply, ApiRoute, check_api_token, record_activity, unauthorized_api_token};

pub(crate) fn routes(ctx: ApiCtx) -> ApiRoute {
    gpu_metrics(ctx.clone())
        .or(system_metrics(ctx.clone()))
        .unify()
        .or(all_metrics(ctx))
        .unify()
        .boxed()
}

fn gpu_metrics(ctx: ApiCtx) -> ApiRoute {
    let state = ctx.state;
    let config = ctx.config;

    warp::path!("metrics" / "gpu")
        .and(warp::get())
        .and(warp::header::optional::<String>("authorization"))
        .and_then(move |auth: Option<String>| {
            let state = state.clone();
            let config = config.clone();
            async move {
                if !check_api_token(&auth, &config) {
                    return Ok(unauthorized_api_token());
                }

                // T-051: wake-on-activity when /api/metrics/gpu is called
                record_activity(&state);
                let gpu = state
                    .gpu_metrics
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .clone();
                Ok::<ApiReply, warp::Rejection>(Box::new(warp::reply::json(&gpu)))
            }
        })
        .boxed()
}

fn system_metrics(ctx: ApiCtx) -> ApiRoute {
    let state = ctx.state;
    let config = ctx.config;

    warp::path!("metrics" / "system")
        .and(warp::get())
        .and(warp::header::optional::<String>("authorization"))
        .and_then(move |auth: Option<String>| {
            let state = state.clone();
            let config = config.clone();
            async move {
                if !check_api_token(&auth, &config) {
                    return Ok(unauthorized_api_token());
                }

                // T-051: wake-on-activity when /api/metrics/system is called
                record_activity(&state);
                let sys = state
                    .system_metrics
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .clone();
                Ok::<ApiReply, warp::Rejection>(Box::new(warp::reply::json(&sys)))
            }
        })
        .boxed()
}

fn all_metrics(ctx: ApiCtx) -> ApiRoute {
    let state = ctx.state;
    let config = ctx.config;

    warp::path!("metrics")
        .and(warp::get())
        .and(warp::header::optional::<String>("authorization"))
        .and_then(move |auth: Option<String>| {
            let state = state.clone();
            let config = config.clone();
            async move {
                if !check_api_token(&auth, &config) {
                    return Ok(unauthorized_api_token());
                }

                let system = state
                    .system_metrics
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .clone();
                let gpu = state
                    .gpu_metrics
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .clone();
                Ok::<ApiReply, warp::Rejection>(Box::new(warp::reply::json(&serde_json::json!({
                    "system": system,
                    "gpu": gpu
                }))))
            }
        })
        .boxed()
}
