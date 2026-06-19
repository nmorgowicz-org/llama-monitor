use std::sync::Arc;

use warp::Filter;

use crate::config::AppConfig;
#[cfg(target_os = "windows")]
use crate::lhm;
use crate::lhm_persistence as lhm_persist;

use super::{ApiCtx, ApiReply, ApiRoute, check_api_token, unauthorized_api_token};

pub(crate) fn routes(ctx: ApiCtx) -> ApiRoute {
    let config = ctx.config;

    api_check_lhm(config.clone())
        .map(box_reply)
        .or(api_lhm_start(config.clone()).map(box_reply))
        .unify()
        .or(api_lhm_progress(config.clone()).map(box_reply))
        .unify()
        .or(api_lhm_status(config.clone()).map(box_reply))
        .unify()
        .or(api_lhm_install(config.clone()).map(box_reply))
        .unify()
        .or(api_lhm_uninstall(config.clone()).map(box_reply))
        .unify()
        .or(api_disable_lhm(config).map(box_reply))
        .unify()
        .boxed()
}

fn box_reply<R>(reply: R) -> ApiReply
where
    R: warp::Reply + 'static,
{
    Box::new(reply)
}

fn api_check_lhm(
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "lhm" / "check")
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
                    let running = lhm::is_lhm_running();
                    let installed = lhm::is_lhm_installed();
                    Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(warp::reply::json(
                        &serde_json::json!({
                            "running": running,
                            "installed": installed,
                            "available": running
                        }),
                    )))
                }

                #[cfg(not(target_os = "windows"))]
                {
                    Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(warp::reply::json(
                        &serde_json::json!({
                            "running": false,
                            "installed": false,
                            "available": false,
                            "error": "Not supported on this platform"
                        }),
                    )))
                }
            }
        })
}

fn api_lhm_start(
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "lhm" / "start")
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
                    match lhm::start_lhm().await {
                        Ok(()) => Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(
                            Box::new(warp::reply::json(&serde_json::json!({"success": true}))),
                        ),
                        Err(e) => Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(
                            Box::new(warp::reply::json(&serde_json::json!({"success": false, "error": e}))),
                        ),
                    }
                }

                #[cfg(not(target_os = "windows"))]
                {
                    Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(
                        Box::new(warp::reply::json(&serde_json::json!({"success": false, "error": "Not supported on this platform"}))),
                    )
                }
            }
        })
}

fn api_lhm_status(
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    let lhm_disabled_file = app_config.lhm_disabled_file.clone();
    warp::path!("api" / "lhm" / "status")
        .and(warp::get())
        .and(warp::header::optional::<String>("authorization"))
        .and_then(move |auth: Option<String>| {
            let cfg = app_config.clone();
            #[allow(unused_variables)]
            let file = lhm_disabled_file.clone();
            async move {
                if !check_api_token(&auth, &cfg) {
                    return Ok(unauthorized_api_token());
                }
                #[cfg(target_os = "windows")]
                {
                    match lhm_persist::load_lhm_disabled(&file) {
                        Ok(disabled) => {
                            Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                                warp::reply::json(&serde_json::json!({"disabled": disabled})),
                            ))
                        }
                        Err(_) => Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                            warp::reply::json(&serde_json::json!({"disabled": false})),
                        )),
                    }
                }

                #[cfg(not(target_os = "windows"))]
                {
                    Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(warp::reply::json(
                        &serde_json::json!({"disabled": false}),
                    )))
                }
            }
        })
}

fn api_lhm_progress(
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "lhm" / "progress")
        .and(warp::get())
        .and(warp::header::optional::<String>("authorization"))
        .and_then(move |auth: Option<String>| {
            let cfg = app_config.clone();
            #[cfg(target_os = "windows")]
            {
                async move {
                    use std::fs;

                    if !check_api_token(&auth, &cfg) {
                        return Ok(unauthorized_api_token());
                    }

                    let local_app_data = match std::env::var("LOCALAPPDATA") {
                        Ok(val) => val,
                        Err(_) => {
                            return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                                warp::reply::json(
                                    &serde_json::json!({"progress": "error: LOCALAPPDATA not set"}),
                                ),
                            ));
                        }
                    };
                    let progress_file = std::path::Path::new(&local_app_data)
                        .join("LibreHardwareMonitor")
                        .join("install_progress.txt");

                    let progress = fs::read_to_string(&progress_file)
                        .map(|s| s.trim().to_string())
                        .unwrap_or_else(|_| "not_started".to_string());

                    Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(warp::reply::json(
                        &serde_json::json!({"progress": progress}),
                    )))
                }
            }

            #[cfg(not(target_os = "windows"))]
            {
                async move {
                    if !check_api_token(&auth, &cfg) {
                        return Ok(unauthorized_api_token());
                    }
                    Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(warp::reply::json(
                        &serde_json::json!({"progress": "not_supported"}),
                    )))
                }
            }
        })
}

fn api_lhm_install(
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "lhm" / "install")
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
                    eprintln!("[API] /api/lhm/install called");
                    match lhm::download_and_install_lhm().await {
                        Ok(()) => {
                            eprintln!("[API] LHM install succeeded");
                            Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(
                                Box::new(warp::reply::json(&serde_json::json!({"success": true}))),
                            )
                        }
                        Err(e) => {
                            eprintln!("[API] LHM install failed: {}", e);
                            Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(
                                Box::new(warp::reply::json(&serde_json::json!({"success": false, "error": e}))),
                            )
                        }
                    }
                }

                #[cfg(not(target_os = "windows"))]
                {
                    eprintln!("[API] /api/lhm/install called (non-Windows, not supported)");
                    Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(
                        Box::new(warp::reply::json(&serde_json::json!({"success": false, "error": "Not supported on this platform"}))),
                    )
                }
            }
        })
}

fn api_lhm_uninstall(
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "lhm" / "uninstall")
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
                    eprintln!("[API] /api/lhm/uninstall called");
                    match lhm::uninstall_lhm() {
                        Ok(()) => {
                            eprintln!("[API] LHM uninstall succeeded");
                            Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(
                                Box::new(warp::reply::json(&serde_json::json!({"success": true}))),
                            )
                        }
                        Err(e) => {
                            eprintln!("[API] LHM uninstall failed: {}", e);
                            Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(
                                Box::new(warp::reply::json(&serde_json::json!({"success": false, "error": e}))),
                            )
                        }
                    }
                }

                #[cfg(not(target_os = "windows"))]
                {
                    eprintln!("[API] /api/lhm/uninstall called (non-Windows, not supported)");
                    Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(
                        Box::new(warp::reply::json(&serde_json::json!({"success": false, "error": "Not supported on this platform"}))),
                    )
                }
            }
        })
}
fn api_disable_lhm(
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    let lhm_disabled_file = app_config.lhm_disabled_file.clone();
    warp::path!("api" / "lhm" / "disable")
        .and(warp::post())
        .and(warp::header::optional::<String>("authorization"))
        .and(warp::body::json())
        .and_then(move |auth: Option<String>, body: serde_json::Value| {
            let cfg = app_config.clone();
            let disabled = body["disabled"].as_bool().unwrap_or(false);
            #[allow(unused_variables)]
            let file = lhm_disabled_file.clone();
            async move {
                if !check_api_token(&auth, &cfg) {
                    return Ok(unauthorized_api_token());
                }
                let result = lhm_persist::save_lhm_disabled(&file, disabled)
                    .map(|_| {
                        Box::new(warp::reply::json(&serde_json::json!({"ok": true})))
                            as Box<dyn warp::reply::Reply>
                    })
                    .unwrap_or_else(|e| {
                        Box::new(warp::reply::json(
                            &serde_json::json!({"ok": false, "error": e}),
                        ))
                    });
                Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(result)
            }
        })
}
