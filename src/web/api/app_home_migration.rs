use warp::Filter;

use serde::Deserialize;

use crate::app_migration::{
    inspect_default_roots, plan_application_home, plan_application_home_cleanup,
    plan_application_home_rollback, queue_application_home_cleanup,
    queue_application_home_migration, queue_application_home_rollback,
};

use super::common::{
    ApiCtx, ApiReply, ApiRoute, check_api_token, check_db_admin_token, unauthorized_api_token,
    unauthorized_db_admin_token,
};

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct QueueRequest {
    plan_id: String,
    confirmation: String,
}

/// Read-only migration status used by the non-blocking frontend migration
/// toast. It is authenticated because root paths and migration state are
/// private application metadata.
pub(crate) fn routes(ctx: ApiCtx) -> ApiRoute {
    let config = ctx.config.clone();
    let status = warp::path!("api" / "app-home-migration" / "status")
        .and(warp::get())
        .and(warp::header::optional::<String>("authorization"))
        .and_then(move |authorization: Option<String>| {
            let config = config.clone();
            async move {
                if !check_api_token(&authorization, &config) {
                    return Ok::<ApiReply, warp::Rejection>(Box::new(unauthorized_api_token()));
                }
                let inspection = inspect_default_roots()
                    .map_err(|error| warp::reject::custom(super::ApiError::migration(&error)))?;
                Ok(Box::new(warp::reply::json(&serde_json::json!({
                    "ok": true,
                    "state": inspection.state,
                    "canonical_root": inspection.canonical_root,
                    "legacy_root": inspection.legacy_root,
                    "active_root": inspection.active_root,
                    "migration_required": matches!(
                        inspection.state,
                        crate::app_migration::RootState::LegacyActive
                            | crate::app_migration::RootState::MigrationQueued
                    ),
                }))))
            }
        })
        .boxed();

    let preview = {
        let config = ctx.config.clone();
        warp::path!("api" / "app-home-migration" / "preview")
            .and(warp::get())
            .and(warp::header::optional::<String>("authorization"))
            .and_then(move |authorization: Option<String>| {
                let config = config.clone();
                async move {
                    if !check_api_token(&authorization, &config) {
                        return Ok::<ApiReply, warp::Rejection>(Box::new(unauthorized_api_token()));
                    }
                    let inspection = inspect_default_roots().map_err(|error| {
                        warp::reject::custom(super::ApiError::migration(&error))
                    })?;
                    if !matches!(
                        inspection.state,
                        crate::app_migration::RootState::LegacyActive
                    ) {
                        return Ok(Box::new(warp::reply::json(&serde_json::json!({
                            "ok": true,
                            "state": inspection.state,
                            "plan": null,
                        }))));
                    }
                    let plan =
                        plan_application_home(&inspection.legacy_root, &inspection.canonical_root)
                            .map_err(|error| {
                                warp::reject::custom(super::ApiError::migration(&error))
                            })?;
                    Ok(Box::new(warp::reply::json(&serde_json::json!({
                        "ok": true,
                        "state": inspection.state,
                        "plan": plan,
                    }))))
                }
            })
            .boxed()
    };

    let queue = {
        let config = ctx.config.clone();
        warp::path!("api" / "app-home-migration" / "queue")
            .and(warp::post())
            .and(warp::header::optional::<String>("authorization"))
            .and(warp::body::json())
            .and_then(move |authorization: Option<String>, body: QueueRequest| {
                let config = config.clone();
                async move {
                    if !check_db_admin_token(&authorization, &config) {
                        return Ok::<ApiReply, warp::Rejection>(Box::new(
                            unauthorized_db_admin_token(),
                        ));
                    }
                    if body.confirmation != "MIGRATE TO LOCAL LLM FOUNDRY" {
                        return Ok(Box::new(warp::reply::with_status(
                            warp::reply::json(&serde_json::json!({
                                "ok": false,
                                "error": "exact migration confirmation is required"
                            })),
                            warp::http::StatusCode::BAD_REQUEST,
                        )));
                    }
                    let inspection = inspect_default_roots().map_err(|error| {
                        warp::reject::custom(super::ApiError::migration(&error))
                    })?;
                    if !matches!(
                        inspection.state,
                        crate::app_migration::RootState::LegacyActive
                    ) {
                        return Ok(Box::new(warp::reply::json(&serde_json::json!({
                            "ok": false,
                            "state": inspection.state,
                            "error": "application root is not in a migratable legacy-only state"
                        }))));
                    }
                    let plan =
                        plan_application_home(&inspection.legacy_root, &inspection.canonical_root)
                            .map_err(|error| {
                                warp::reject::custom(super::ApiError::migration(&error))
                            })?;
                    if plan.plan_id != body.plan_id {
                        return Ok(Box::new(warp::reply::with_status(
                            warp::reply::json(&serde_json::json!({
                                "ok": false,
                                "error": "migration preview is stale; refresh and try again"
                            })),
                            warp::http::StatusCode::CONFLICT,
                        )));
                    }
                    let request = queue_application_home_migration(&plan).map_err(|error| {
                        warp::reject::custom(super::ApiError::migration(&error))
                    })?;
                    Ok(Box::new(warp::reply::json(&serde_json::json!({
                        "ok": true,
                        "restart_required": true,
                        "request": request,
                    }))))
                }
            })
            .boxed()
    };

    let rollback_preview = {
        let config = ctx.config.clone();
        warp::path!("api" / "app-home-migration" / "rollback" / "preview")
            .and(warp::post())
            .and(warp::header::optional::<String>("authorization"))
            .and_then(move |authorization: Option<String>| {
                let config = config.clone();
                async move {
                    if !check_api_token(&authorization, &config) {
                        return Ok::<ApiReply, warp::Rejection>(Box::new(unauthorized_api_token()));
                    }
                    let inspection = inspect_default_roots().map_err(|error| {
                        warp::reject::custom(super::ApiError::migration(&error))
                    })?;
                    let plan = plan_application_home_rollback(
                        &inspection.canonical_root,
                        &inspection.legacy_root,
                    )
                    .map_err(|error| warp::reject::custom(super::ApiError::migration(&error)))?;
                    Ok(Box::new(warp::reply::json(&serde_json::json!({
                        "ok": true,
                        "plan": plan,
                    }))))
                }
            })
            .boxed()
    };

    let rollback_queue = {
        let config = ctx.config.clone();
        warp::path!("api" / "app-home-migration" / "rollback" / "queue")
            .and(warp::post())
            .and(warp::header::optional::<String>("authorization"))
            .and(warp::body::json())
            .and_then(move |authorization: Option<String>, body: QueueRequest| {
                let config = config.clone();
                async move {
                    if !check_db_admin_token(&authorization, &config) {
                        return Ok::<ApiReply, warp::Rejection>(Box::new(
                            unauthorized_db_admin_token(),
                        ));
                    }
                    if body.confirmation != "ROLL BACK TO LLAMA MONITOR" {
                        return Ok(Box::new(warp::reply::with_status(
                            warp::reply::json(&serde_json::json!({
                                "ok": false,
                                "error": "exact rollback confirmation is required"
                            })),
                            warp::http::StatusCode::BAD_REQUEST,
                        )));
                    }
                    let inspection = inspect_default_roots().map_err(|error| {
                        warp::reject::custom(super::ApiError::migration(&error))
                    })?;
                    let plan = plan_application_home_rollback(
                        &inspection.canonical_root,
                        &inspection.legacy_root,
                    )
                    .map_err(|error| warp::reject::custom(super::ApiError::migration(&error)))?;
                    let request = queue_application_home_rollback(&plan).map_err(|error| {
                        warp::reject::custom(super::ApiError::migration(&error))
                    })?;
                    Ok(Box::new(warp::reply::json(&serde_json::json!({
                        "ok": true,
                        "restart_required": true,
                        "request": request,
                    }))))
                }
            })
            .boxed()
    };

    let cleanup = {
        let config = ctx.config.clone();
        warp::path!("api" / "app-home-migration" / "cleanup")
            .and(warp::post())
            .and(warp::header::optional::<String>("authorization"))
            .and(warp::body::json())
            .and_then(move |authorization: Option<String>, body: QueueRequest| {
                let config = config.clone();
                async move {
                    if !check_db_admin_token(&authorization, &config) {
                        return Ok::<ApiReply, warp::Rejection>(Box::new(
                            unauthorized_db_admin_token(),
                        ));
                    }
                    if body.confirmation != "DELETE LEGACY ROOT AFTER VERIFIED MIGRATION" {
                        return Ok(Box::new(warp::reply::with_status(
                            warp::reply::json(&serde_json::json!({
                                "ok": false,
                                "error": "exact cleanup confirmation is required"
                            })),
                            warp::http::StatusCode::BAD_REQUEST,
                        )));
                    }
                    let inspection = inspect_default_roots().map_err(|error| {
                        warp::reject::custom(super::ApiError::migration(&error))
                    })?;
                    let plan = plan_application_home_cleanup(
                        &inspection.canonical_root,
                        &inspection.legacy_root,
                    )
                    .map_err(|error| warp::reject::custom(super::ApiError::migration(&error)))?;
                    let request = queue_application_home_cleanup(&plan).map_err(|error| {
                        warp::reject::custom(super::ApiError::migration(&error))
                    })?;
                    Ok(Box::new(warp::reply::json(&serde_json::json!({
                        "ok": true,
                        "restart_required": true,
                        "request": request,
                    }))))
                }
            })
            .boxed()
    };

    status
        .or(preview)
        .unify()
        .or(queue)
        .unify()
        .or(rollback_preview)
        .unify()
        .or(rollback_queue)
        .unify()
        .or(cleanup)
        .unify()
        .boxed()
}
