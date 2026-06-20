use std::sync::Arc;

use warp::Filter;

use crate::config::AppConfig;
#[cfg(target_os = "windows")]
use crate::lhm;

use super::{ApiCtx, ApiRoute, box_reply, check_api_token, unauthorized_api_token};

pub(crate) fn routes(ctx: ApiCtx) -> ApiRoute {
    let config = ctx.config;

    api_sensor_bridge_status(config.clone())
        .map(box_reply)
        .or(api_sensor_bridge_install(config.clone()).map(box_reply))
        .unify()
        .or(api_sensor_bridge_uninstall(config).map(box_reply))
        .unify()
        .boxed()
}

fn api_sensor_bridge_status(
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "sensor-bridge" / "status")
        .and(warp::get())
        .and(warp::header::optional::<String>("authorization"))
        .and_then(move |auth: Option<String>| {
            let cfg = app_config.clone();
            async move {
                if !check_api_token(&auth, &cfg) {
                    return Ok(unauthorized_api_token());
                }
                #[cfg(target_os = "windows")]
                {
                    let installed = lhm::is_local_sensor_bridge_service_installed();
                    let running = lhm::is_local_sensor_bridge_running();
                    Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(warp::reply::json(
                        &serde_json::json!({
                            "installed": installed,
                            "running": running,
                            "available": lhm::is_sensor_bridge_available(),
                        }),
                    )))
                }
                #[cfg(not(target_os = "windows"))]
                {
                    Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(warp::reply::json(
                        &serde_json::json!({
                            "installed": false,
                            "running": false,
                            "available": false,
                        }),
                    )))
                }
            }
        })
}

fn api_sensor_bridge_install(
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "sensor-bridge" / "install")
        .and(warp::post())
        .and(warp::header::optional::<String>("authorization"))
        .and_then(move |auth: Option<String>| {
            let cfg = app_config.clone();
            async move {
                if !check_api_token(&auth, &cfg) {
                    return Ok(unauthorized_api_token());
                }
                #[cfg(target_os = "windows")]
                {
                    match lhm::install_local_sensor_bridge() {
                        Ok(()) => Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(
                            Box::new(warp::reply::json(&serde_json::json!({
                                "started": true,
                                "message": "UAC prompt launched — approve it on your desktop to install the sensor service",
                            }))),
                        ),
                        Err(e) => Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(
                            Box::new(warp::reply::json(&serde_json::json!({
                                "started": false,
                                "error": e,
                            }))),
                        ),
                    }
                }
                #[cfg(not(target_os = "windows"))]
                {
                    Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(
                        Box::new(warp::reply::json(&serde_json::json!({
                            "started": false,
                            "error": "Not supported on this platform",
                        }))),
                    )
                }
            }
        })
}

fn api_sensor_bridge_uninstall(
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "sensor-bridge" / "uninstall")
        .and(warp::post())
        .and(warp::header::optional::<String>("authorization"))
        .and_then(move |auth: Option<String>| {
            let cfg = app_config.clone();
            async move {
                if !check_api_token(&auth, &cfg) {
                    return Ok(unauthorized_api_token());
                }
                #[cfg(target_os = "windows")]
                {
                    match lhm::uninstall_local_sensor_bridge() {
                        Ok(()) => Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(
                            Box::new(warp::reply::json(&serde_json::json!({
                                "started": true,
                                "message": "UAC prompt launched — approve it on your desktop to remove the sensor service",
                            }))),
                        ),
                        Err(e) => Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(
                            Box::new(warp::reply::json(&serde_json::json!({
                                "started": false,
                                "error": e,
                            }))),
                        ),
                    }
                }
                #[cfg(not(target_os = "windows"))]
                {
                    Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(
                        Box::new(warp::reply::json(&serde_json::json!({
                            "started": false,
                            "error": "Not supported on this platform",
                        }))),
                    )
                }
            }
        })
}
