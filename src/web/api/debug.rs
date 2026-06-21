use warp::Filter;

use super::{ApiCtx, ApiReply, ApiRoute, check_api_token, unauthorized_api_token};

pub(crate) fn routes(ctx: ApiCtx) -> ApiRoute {
    spawn_cmd(ctx.clone()).or(logs(ctx)).unify().boxed()
}

fn spawn_cmd(ctx: ApiCtx) -> ApiRoute {
    let state = ctx.state;
    let config = ctx.config;

    warp::path!("api" / "debug" / "spawn-cmd")
        .and(warp::get())
        .and(warp::header::optional::<String>("authorization"))
        .and_then(move |auth: Option<String>| {
            let state = state.clone();
            let config = config.clone();
            async move {
                if !check_api_token(&auth, &config) {
                    return Ok::<ApiReply, warp::Rejection>(unauthorized_api_token());
                }

                let cmd = state.last_spawn_cmd.lock().unwrap().clone();
                Ok(Box::new(warp::reply::json(&serde_json::json!({
                    "ok": true,
                    "cmd": cmd
                }))) as ApiReply)
            }
        })
        .boxed()
}

fn logs(ctx: ApiCtx) -> ApiRoute {
    let state = ctx.state;
    let config = ctx.config;

    warp::path!("api" / "debug" / "logs")
        .and(warp::get())
        .and(warp::header::optional::<String>("authorization"))
        .and_then(move |auth: Option<String>| {
            let state = state.clone();
            let config = config.clone();
            async move {
                if !check_api_token(&auth, &config) {
                    return Ok::<ApiReply, warp::Rejection>(unauthorized_api_token());
                }

                let logs: Vec<String> = state.server_logs.lock().unwrap().iter().cloned().collect();
                Ok(Box::new(warp::reply::json(&serde_json::json!({
                    "ok": true,
                    "count": logs.len(),
                    "logs": logs
                }))) as ApiReply)
            }
        })
        .boxed()
}
