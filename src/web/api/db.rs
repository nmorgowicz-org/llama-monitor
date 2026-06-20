use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use warp::Filter;

use crate::chat_storage::ChatStorage;
use crate::config::AppConfig;

use super::common::{
    bearer_matches_api_token, bearer_matches_db_admin_token, try_cooldown, with_app_config,
    with_chat_storage,
};
use super::{ApiCtx, ApiRoute};

pub(crate) fn routes(ctx: ApiCtx, chat_storage: Arc<ChatStorage>) -> ApiRoute {
    let config = ctx.config;

    api_db_stats(chat_storage.clone(), config.clone())
        .or(api_db_integrity(chat_storage.clone(), config.clone()))
        .unify()
        .or(api_db_maintenance(chat_storage.clone(), config.clone()))
        .unify()
        .or(api_db_backup(chat_storage.clone(), config.clone()))
        .unify()
        .or(api_db_indexes(chat_storage.clone(), config.clone()))
        .unify()
        .or(api_db_query(chat_storage.clone(), config.clone()))
        .unify()
        .or(api_db_backups(config.clone()))
        .unify()
        .or(api_db_restore(chat_storage.clone(), config.clone()))
        .unify()
        .or(api_db_repair(chat_storage.clone(), config.clone()))
        .unify()
        .or(api_db_delete_backup(config))
        .unify()
        .boxed()
}

fn api_db_stats(storage: Arc<ChatStorage>, app_config: Arc<AppConfig>) -> ApiRoute {
    let app_config = app_config.clone();

    warp::path!("api" / "db" / "stats")
        .and(warp::get())
        .and(warp::header::optional::<String>("authorization"))
        .and(with_chat_storage(storage))
        .and(with_app_config(app_config))
        .and_then(
            move |auth: Option<String>, store: Arc<ChatStorage>, cfg: Arc<AppConfig>| async move {
                let bearer = auth.and_then(|v| v.strip_prefix("Bearer ").map(str::to_string));
                let has_api_token = bearer_matches_api_token(bearer.as_deref(), &cfg);

                if !has_api_token {
                    return Ok::<_, warp::Rejection>(Box::new(warp::reply::with_status(
                        warp::reply::json(&serde_json::json!({ "error": "unauthorized" })),
                        warp::http::StatusCode::UNAUTHORIZED,
                    ))
                        as Box<dyn warp::reply::Reply>);
                }

                match store.database_stats() {
                    Ok(stats) => Ok::<_, warp::Rejection>(
                        Box::new(warp::reply::json(&stats)) as Box<dyn warp::reply::Reply>
                    ),
                    Err(e) => {
                        eprintln!("db stats error: {e}");
                        Ok::<_, warp::Rejection>(Box::new(warp::reply::json(
                            &serde_json::json!({"error": e.to_string()}),
                        ))
                            as Box<dyn warp::reply::Reply>)
                    }
                }
            },
        )
        .boxed()
}

fn api_db_integrity(storage: Arc<ChatStorage>, app_config: Arc<AppConfig>) -> ApiRoute {
    let app_config = app_config.clone();

    warp::path!("api" / "db" / "integrity")
        .and(warp::get())
        .and(warp::header::optional::<String>("authorization"))
        .and(with_chat_storage(storage))
        .and(with_app_config(app_config))
        .and_then(
            move |auth: Option<String>, store: Arc<ChatStorage>, cfg: Arc<AppConfig>| async move {
                let bearer = auth.and_then(|v| v.strip_prefix("Bearer ").map(str::to_string));
                let has_api_token = bearer_matches_api_token(bearer.as_deref(), &cfg);

                if !has_api_token {
                    return Ok::<_, warp::Rejection>(Box::new(warp::reply::with_status(
                        warp::reply::json(&serde_json::json!({ "error": "unauthorized" })),
                        warp::http::StatusCode::UNAUTHORIZED,
                    ))
                        as Box<dyn warp::reply::Reply>);
                }

                match store.integrity_check() {
                    Ok(result) => {
                        let status = if result == "ok" {
                            "healthy"
                        } else {
                            "corrupted"
                        };
                        Ok::<_, warp::Rejection>(Box::new(warp::reply::json(&serde_json::json!({
                            "status": status,
                            "detail": result,
                        })))
                            as Box<dyn warp::reply::Reply>)
                    }
                    Err(e) => {
                        eprintln!("integrity check error: {e}");
                        Ok::<_, warp::Rejection>(Box::new(warp::reply::json(
                            &serde_json::json!({"error": e.to_string()}),
                        ))
                            as Box<dyn warp::reply::Reply>)
                    }
                }
            },
        )
        .boxed()
}

fn api_db_maintenance(storage: Arc<ChatStorage>, app_config: Arc<AppConfig>) -> ApiRoute {
    #[derive(serde::Deserialize)]
    struct MaintenanceRequest {
        operation: String,
    }

    let app_config = app_config.clone();

    warp::path!("api" / "db" / "maintenance")
        .and(warp::post())
        .and(warp::header::optional::<String>("authorization"))
        .and(warp::body::json::<MaintenanceRequest>())
        .and(with_chat_storage(storage))
        .and(with_app_config(app_config))
        .and_then(
            move |auth: Option<String>, req: MaintenanceRequest, store: Arc<ChatStorage>, cfg: Arc<AppConfig>| {
                async move {
                    let bearer = auth.and_then(|v| v.strip_prefix("Bearer ").map(str::to_string));
                    let has_api_token =
                        bearer_matches_api_token(bearer.as_deref(), &cfg);

                    if !has_api_token {
                        return Ok::<_, warp::Rejection>(
                            Box::new(warp::reply::with_status(
                                warp::reply::json(&serde_json::json!({ "error": "unauthorized" })),
                                warp::http::StatusCode::UNAUTHORIZED,
                            )) as Box<dyn warp::reply::Reply>,
                        );
                    }

                    let result = match req.operation.as_str() {
                        "checkpoint" => store.checkpoint().map(
                            |(a, b, c)| serde_json::json!({"backfilled": a, "deleted": b, "log": c}),
                        ),
                        "vacuum" => store
                            .vacuum()
                            .map(|_| serde_json::json!({"status": "vacuumed"})),
                        "rebuild_fts" => store
                            .rebuild_fts_index()
                            .map(|_| serde_json::json!({"status": "fts_rebuilt"})),
                        "analyze" => store
                            .analyze()
                            .map(|_| serde_json::json!({"status": "analyzed"})),
                        _ => Err(anyhow::anyhow!("Unknown operation: {}", req.operation)),
                    };

                    match result {
                        Ok(response) => Ok::<_, warp::Rejection>(
                            Box::new(warp::reply::json(&response)) as Box<dyn warp::reply::Reply>,
                        ),
                        Err(e) => {
                            eprintln!("maintenance error: {e}");
                            Ok::<_, warp::Rejection>(Box::new(warp::reply::json(
                                &serde_json::json!({"error": e.to_string()}),
                            )) as Box<dyn warp::reply::Reply>)
                        }
                    }
                }
            },
        )
        .boxed()
}

fn api_db_backup(storage: Arc<ChatStorage>, app_config: Arc<AppConfig>) -> ApiRoute {
    static LAST_DB_BACKUP: AtomicU64 = AtomicU64::new(0);

    warp::path!("api" / "db" / "backup")
        .and(warp::post())
        .and(warp::header::optional::<String>("authorization"))
        .and(with_chat_storage(storage))
        .and(with_app_config(app_config))
        .and_then(
            move |auth: Option<String>, store: Arc<ChatStorage>, cfg: Arc<AppConfig>| {
                let cfg = cfg.clone();
                async move {
                    let bearer = auth.and_then(|v| v.strip_prefix("Bearer ").map(str::to_string));
                    let has_api_token = bearer_matches_api_token(bearer.as_deref(), &cfg);

                    if !has_api_token {
                        return Ok::<_, warp::Rejection>(
                            Box::new(warp::reply::with_status(
                                warp::reply::json(&serde_json::json!({ "error": "unauthorized" })),
                                warp::http::StatusCode::UNAUTHORIZED,
                            )) as Box<dyn warp::reply::Reply>,
                        );
                    }

                    let now = SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs();
                    let last = LAST_DB_BACKUP.load(Ordering::Acquire);
                    if now - last < 10 {
                        let remaining = 10 - (now - last);
                        return Ok::<_, warp::Rejection>(
                            Box::new(warp::reply::with_status(
                                warp::reply::json(&serde_json::json!({
                                    "ok": false,
                                    "error": "too soon; please wait",
                                    "seconds_remaining": remaining
                                })),
                                warp::http::StatusCode::TOO_MANY_REQUESTS,
                            )) as Box<dyn warp::reply::Reply>,
                        );
                    }
                    LAST_DB_BACKUP.store(now, Ordering::Release);

                    let config_dir = cfg.config_dir.clone();
                    let backup_dir = config_dir.join("backups").join("manual");

                    if let Err(e) = std::fs::create_dir_all(&backup_dir) {
                        eprintln!("Failed to create manual backup directory: {e}");
                        return Ok::<_, warp::Rejection>(
                            Box::new(warp::reply::with_status(
                                warp::reply::json(&serde_json::json!({"error": e.to_string()})),
                                warp::http::StatusCode::OK,
                            )) as Box<dyn warp::reply::Reply>,
                        );
                    }

                    let timestamp = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .map(|d| d.as_millis().to_string())
                        .unwrap_or_else(|_| "0".to_string());
                    let backup_path = backup_dir.join(format!("chat_{}.db", timestamp));

                    match store.backup(&backup_path) {
                        Ok(()) => {
                            let file_size = std::fs::metadata(&backup_path)
                                .ok()
                                .map(|m| m.len())
                                .unwrap_or(0);

                            if let Ok(entries) = std::fs::read_dir(&backup_dir) {
                                let mut backups: Vec<_> = entries
                                    .filter_map(|e| e.ok())
                                    .filter(|e| {
                                        e.file_name().to_string_lossy().starts_with("chat_")
                                    })
                                    .collect();
                                backups.sort_by_key(|e| e.path());
                                while backups.len() > 7 {
                                    let old = backups.remove(0);
                                    let _ = std::fs::remove_file(old.path());
                                }
                            }

                            Ok::<_, warp::Rejection>(
                                Box::new(warp::reply::with_status(
                                    warp::reply::json(&serde_json::json!({
                                        "status": "backup_created",
                                        "name": format!("manual/{}", backup_path.file_name().unwrap_or_default().to_string_lossy()),
                                        "size_bytes": file_size,
                                    })),
                                    warp::http::StatusCode::OK,
                                )) as Box<dyn warp::reply::Reply>,
                            )
                        }
                        Err(e) => {
                            eprintln!("backup error: {e}");
                            Ok::<_, warp::Rejection>(
                                Box::new(warp::reply::with_status(
                                    warp::reply::json(&serde_json::json!({"error": e.to_string()})),
                                    warp::http::StatusCode::OK,
                                )) as Box<dyn warp::reply::Reply>,
                            )
                        }
                    }
                }
            },
        )
        .boxed()
}

fn api_db_indexes(storage: Arc<ChatStorage>, app_config: Arc<AppConfig>) -> ApiRoute {
    let app_config = app_config.clone();

    warp::path!("api" / "db" / "indexes")
        .and(warp::get())
        .and(warp::header::optional::<String>("authorization"))
        .and(with_chat_storage(storage))
        .and(with_app_config(app_config))
        .and_then(
            move |auth: Option<String>, store: Arc<ChatStorage>, cfg: Arc<AppConfig>| async move {
                let bearer = auth.and_then(|v| v.strip_prefix("Bearer ").map(str::to_string));
                let has_api_token = bearer_matches_api_token(bearer.as_deref(), &cfg);

                if !has_api_token {
                    return Ok::<_, warp::Rejection>(Box::new(warp::reply::with_status(
                        warp::reply::json(&serde_json::json!({ "error": "unauthorized" })),
                        warp::http::StatusCode::UNAUTHORIZED,
                    ))
                        as Box<dyn warp::reply::Reply>);
                }

                match store.list_indexes() {
                    Ok(indexes) => Ok::<_, warp::Rejection>(
                        Box::new(warp::reply::json(&indexes)) as Box<dyn warp::reply::Reply>
                    ),
                    Err(e) => {
                        eprintln!("list indexes error: {e}");
                        Ok::<_, warp::Rejection>(Box::new(warp::reply::json(
                            &serde_json::json!({"error": e.to_string()}),
                        ))
                            as Box<dyn warp::reply::Reply>)
                    }
                }
            },
        )
        .boxed()
}

fn api_db_query(storage: Arc<ChatStorage>, app_config: Arc<AppConfig>) -> ApiRoute {
    #[derive(serde::Deserialize)]
    struct QueryRequest {
        sql: String,
    }

    let storage = storage.clone();
    let app_config = app_config.clone();

    warp::path!("api" / "db" / "query")
        .and(warp::post())
        .and(warp::body::content_length_limit(256 * 1024))
        .and(warp::header::optional::<String>("authorization"))
        .and(warp::body::json::<QueryRequest>())
        .and(with_chat_storage(storage))
        .and(with_app_config(app_config))
        .and_then(
            move |auth: Option<String>,
                  req: QueryRequest,
                  store: Arc<ChatStorage>,
                  cfg: Arc<AppConfig>| {
                let bearer = auth.and_then(|v| v.strip_prefix("Bearer ").map(str::to_string));

                let store_clone = store.clone();
                async move {
                    let has_api_token = bearer_matches_api_token(bearer.as_deref(), &cfg);
                    let is_admin = bearer_matches_db_admin_token(bearer.as_deref(), &cfg);

                    if !has_api_token && !is_admin {
                        return Ok::<_, warp::Rejection>(Box::new(warp::reply::with_status(
                            warp::reply::json(&serde_json::json!({ "error": "unauthorized" })),
                            warp::http::StatusCode::UNAUTHORIZED,
                        ))
                            as Box<dyn warp::reply::Reply>);
                    }

                    if req.sql.len() > 16_000 {
                        return Ok::<_, warp::Rejection>(Box::new(warp::reply::with_status(
                            warp::reply::json(&serde_json::json!({ "error": "query too long" })),
                            warp::http::StatusCode::BAD_REQUEST,
                        ))
                            as Box<dyn warp::reply::Reply>);
                    }

                    let store = store_clone.clone();
                    let sql = req.sql.clone();
                    let result =
                        tokio::time::timeout(std::time::Duration::from_secs(10), async move {
                            store.execute_query(&sql, is_admin)
                        })
                        .await;

                    match result {
                        Ok(Ok(result)) => {
                            Ok::<_, warp::Rejection>(Box::new(warp::reply::with_status(
                                warp::reply::json(&result),
                                warp::http::StatusCode::OK,
                            ))
                                as Box<dyn warp::reply::Reply>)
                        }
                        Ok(Err(e)) => {
                            eprintln!("query error: {e}");
                            Ok::<_, warp::Rejection>(Box::new(warp::reply::with_status(
                                warp::reply::json(&serde_json::json!({"error": e.to_string()})),
                                warp::http::StatusCode::OK,
                            ))
                                as Box<dyn warp::reply::Reply>)
                        }
                        Err(_) => {
                            eprintln!("query timeout");
                            Ok::<_, warp::Rejection>(Box::new(warp::reply::with_status(
                                warp::reply::json(&serde_json::json!({"error": "query timed out"})),
                                warp::http::StatusCode::REQUEST_TIMEOUT,
                            ))
                                as Box<dyn warp::reply::Reply>)
                        }
                    }
                }
            },
        )
        .boxed()
}

fn api_db_backups(app_config: Arc<AppConfig>) -> ApiRoute {
    warp::path!("api" / "db" / "backups")
        .and(warp::get())
        .and(warp::header::optional::<String>("authorization"))
        .and(with_app_config(app_config))
        .and_then(move |auth: Option<String>, cfg: Arc<AppConfig>| {
            let cfg = cfg.clone();
            async move {
                let bearer = auth.and_then(|v| v.strip_prefix("Bearer ").map(str::to_string));
                let has_api_token = bearer_matches_api_token(bearer.as_deref(), &cfg);

                if !has_api_token {
                    return Ok::<_, warp::Rejection>(Box::new(warp::reply::with_status(
                        warp::reply::json(&serde_json::json!({ "error": "unauthorized" })),
                        warp::http::StatusCode::UNAUTHORIZED,
                    ))
                        as Box<dyn warp::reply::Reply>);
                }

                let backups_root = cfg.config_dir.join("backups");
                let mut backups = Vec::new();
                let mut total_size = 0u64;

                for (kind, subdir) in [("auto", "auto"), ("daily", "daily"), ("manual", "manual")] {
                    let dir = backups_root.join(subdir);
                    if let Ok(entries) = std::fs::read_dir(&dir) {
                        for entry in entries.filter_map(|e| e.ok()) {
                            if let Ok(metadata) = entry.metadata()
                                && metadata.is_file()
                            {
                                let filename = entry.file_name().to_string_lossy().to_string();
                                if !filename.ends_with(".db") {
                                    continue;
                                }
                                let size = metadata.len();
                                total_size += size;
                                let modified = metadata
                                    .modified()
                                    .ok()
                                    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                                    .map(|d| d.as_millis() as i64)
                                    .unwrap_or(0);
                                backups.push(serde_json::json!({
                                    "name": format!("{}/{}", subdir, filename),
                                    "kind": kind,
                                    "size": size,
                                    "modified": modified,
                                }));
                            }
                        }
                    }
                }

                backups.sort_by_key(|b| b["modified"].as_i64().unwrap_or(0));
                backups.reverse();

                Ok::<_, warp::Rejection>(Box::new(warp::reply::with_status(
                    warp::reply::json(&serde_json::json!({
                        "backups": backups,
                        "total_size": total_size,
                    })),
                    warp::http::StatusCode::OK,
                )) as Box<dyn warp::reply::Reply>)
            }
        })
        .boxed()
}

fn api_db_restore(storage: Arc<ChatStorage>, app_config: Arc<AppConfig>) -> ApiRoute {
    #[derive(serde::Deserialize)]
    struct RestoreRequest {
        backup_name: String,
    }

    warp::path!("api" / "db" / "restore")
        .and(warp::post())
        .and(warp::header::optional::<String>("authorization"))
        .and(warp::body::json::<RestoreRequest>())
        .and(with_chat_storage(storage))
        .and(with_app_config(app_config))
        .and_then(
            move |auth: Option<String>,
                  req: RestoreRequest,
                  store: Arc<ChatStorage>,
                  cfg: Arc<AppConfig>| {
                static LAST_DB_RESTORE: AtomicU64 = AtomicU64::new(0);

                let cfg = cfg.clone();
                async move {
                    let bearer = auth
                        .and_then(|v| v.strip_prefix("Bearer ").map(str::to_string));

                    let has_admin_token =
                        bearer_matches_db_admin_token(bearer.as_deref(), &cfg);

                    if !has_admin_token {
                        return Ok::<_, warp::Rejection>(
                            Box::new(warp::reply::with_status(
                                warp::reply::json(&serde_json::json!({ "error": "unauthorized" })),
                                warp::http::StatusCode::UNAUTHORIZED,
                            )) as Box<dyn warp::reply::Reply>,
                        );
                    }

                    let now = SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs();
                    let (ok, remaining) = try_cooldown(&LAST_DB_RESTORE, now, 30);
                    if !ok {
                        return Ok::<_, warp::Rejection>(
                            Box::new(warp::reply::with_status(
                                warp::reply::json(&serde_json::json!({
                                    "ok": false,
                                    "error": "too soon; please wait",
                                    "seconds_remaining": remaining
                                })),
                                warp::http::StatusCode::TOO_MANY_REQUESTS,
                            )) as Box<dyn warp::reply::Reply>,
                        );
                    }

                    let backup_name = req.backup_name.trim();
                    if backup_name.is_empty()
                        || backup_name.contains("..")
                        || backup_name.starts_with('/')
                        || backup_name.contains('\\')
                    {
                        return Ok::<_, warp::Rejection>(
                            Box::new(warp::reply::with_status(
                                warp::reply::json(&serde_json::json!({ "error": "invalid backup name" })),
                                warp::http::StatusCode::BAD_REQUEST,
                            )) as Box<dyn warp::reply::Reply>,
                        );
                    }

                    let backup_dir = cfg.config_dir.join("backups");
                    let backup_path = backup_dir.join(backup_name);

                    if matches!(
                        (backup_path.canonicalize(), backup_dir.canonicalize()),
                        (Ok(ref canonical), Ok(ref base)) if !canonical.starts_with(base)
                    ) {
                        return Ok::<_, warp::Rejection>(
                            Box::new(warp::reply::with_status(
                                warp::reply::json(&serde_json::json!({ "error": "path not allowed" })),
                                warp::http::StatusCode::BAD_REQUEST,
                            )) as Box<dyn warp::reply::Reply>,
                        );
                    }

                    if !backup_path.exists() {
                        return Ok::<_, warp::Rejection>(
                            Box::new(warp::reply::with_status(
                                warp::reply::json(&serde_json::json!({
                                    "error": format!("Backup not found: {}", backup_name)
                                })),
                                warp::http::StatusCode::OK,
                            )) as Box<dyn warp::reply::Reply>,
                        );
                    }

                    let manual_dir = cfg.config_dir.join("backups").join("manual");
                    let _ = std::fs::create_dir_all(&manual_dir);
                    let safety_backup = manual_dir.join(format!(
                        "pre_restore_{}.db",
                        std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .map(|d| d.as_millis().to_string())
                            .unwrap_or_else(|_| "0".to_string())
                    ));
                    let _ = store.backup(&safety_backup);

                    match store.restore_from_path(&backup_path) {
                        Ok(()) => {
                            match store.integrity_check() {
                                Ok(_) => {
                                    Ok::<_, warp::Rejection>(
                                        Box::new(warp::reply::with_status(
                                            warp::reply::json(&serde_json::json!({
                                                "status": "restored",
                                                "backup": backup_name,
                                            })),
                                            warp::http::StatusCode::OK,
                                        )) as Box<dyn warp::reply::Reply>,
                                    )
                                }
                                Err(e) => {
                                    eprintln!("Restored database integrity check failed: {e}");
                                    Ok::<_, warp::Rejection>(
                                        Box::new(warp::reply::with_status(
                                            warp::reply::json(&serde_json::json!({
                                                "error": "Restore succeeded but integrity check failed",
                                                "safety_backup": safety_backup.to_string_lossy().to_string(),
                                            })),
                                            warp::http::StatusCode::OK,
                                        )) as Box<dyn warp::reply::Reply>,
                                    )
                                }
                            }
                        }
                        Err(e) => {
                            eprintln!("Restore error: {e}");
                            Ok::<_, warp::Rejection>(
                                Box::new(warp::reply::with_status(
                                    warp::reply::json(&serde_json::json!({"error": e.to_string()})),
                                    warp::http::StatusCode::OK,
                                )) as Box<dyn warp::reply::Reply>,
                            )
                        }
                    }
                }
            },
        )
        .boxed()
}

fn api_db_repair(storage: Arc<ChatStorage>, app_config: Arc<AppConfig>) -> ApiRoute {
    #[derive(serde::Deserialize)]
    struct RepairRequest {
        operation: String,
    }

    let app_config = app_config.clone();

    warp::path!("api" / "db" / "repair")
        .and(warp::post())
        .and(warp::header::optional::<String>("authorization"))
        .and(warp::body::json::<RepairRequest>())
        .and(with_chat_storage(storage))
        .and(with_app_config(app_config))
        .and_then(
            move |auth: Option<String>,
                  req: RepairRequest,
                  store: Arc<ChatStorage>,
                  cfg: Arc<AppConfig>| {
                async move {
                    let bearer = auth.and_then(|v| v.strip_prefix("Bearer ").map(str::to_string));
                    let has_db_admin_token = bearer_matches_db_admin_token(bearer.as_deref(), &cfg);

                    if !has_db_admin_token {
                        return Ok::<_, warp::Rejection>(Box::new(warp::reply::with_status(
                            warp::reply::json(&serde_json::json!({ "error": "unauthorized" })),
                            warp::http::StatusCode::UNAUTHORIZED,
                        ))
                            as Box<dyn warp::reply::Reply>);
                    }

                    match req.operation.as_str() {
                        "repair_indexes" => match store.repair_indexes() {
                            Ok(_) => Ok::<_, warp::Rejection>(Box::new(warp::reply::json(
                                &serde_json::json!({
                                    "status": "indexes_repaired",
                                }),
                            ))
                                as Box<dyn warp::reply::Reply>),
                            Err(e) => {
                                eprintln!("Repair indexes error: {e}");
                                Ok::<_, warp::Rejection>(Box::new(warp::reply::json(
                                    &serde_json::json!({"error": e.to_string()}),
                                ))
                                    as Box<dyn warp::reply::Reply>)
                            }
                        },
                        "emergency_recovery" => match store.emergency_recovery() {
                            Ok(_) => Ok::<_, warp::Rejection>(Box::new(warp::reply::json(
                                &serde_json::json!({
                                    "status": "recovery_attempted",
                                }),
                            ))
                                as Box<dyn warp::reply::Reply>),
                            Err(e) => {
                                eprintln!("Emergency recovery error: {e}");
                                Ok::<_, warp::Rejection>(Box::new(warp::reply::json(
                                    &serde_json::json!({"error": e.to_string()}),
                                ))
                                    as Box<dyn warp::reply::Reply>)
                            }
                        },
                        _ => Ok::<_, warp::Rejection>(Box::new(warp::reply::json(
                            &serde_json::json!({
                                "error": format!("Unknown repair operation: {}", req.operation)
                            }),
                        ))
                            as Box<dyn warp::reply::Reply>),
                    }
                }
            },
        )
        .boxed()
}

fn api_db_delete_backup(app_config: Arc<AppConfig>) -> ApiRoute {
    #[derive(serde::Deserialize)]
    struct DeleteBackupRequest {
        backup_name: String,
    }

    warp::path!("api" / "db" / "backup")
        .and(warp::delete())
        .and(warp::header::optional::<String>("authorization"))
        .and(warp::body::json::<DeleteBackupRequest>())
        .and(with_app_config(app_config))
        .and_then(
            move |auth: Option<String>, req: DeleteBackupRequest, cfg: Arc<AppConfig>| {
                let cfg = cfg.clone();
                async move {
                    let bearer = auth.and_then(|v| v.strip_prefix("Bearer ").map(str::to_string));

                    let has_admin_token = bearer_matches_db_admin_token(bearer.as_deref(), &cfg);

                    if !has_admin_token {
                        return Ok::<_, warp::Rejection>(Box::new(warp::reply::with_status(
                            warp::reply::json(&serde_json::json!({ "error": "unauthorized" })),
                            warp::http::StatusCode::UNAUTHORIZED,
                        ))
                            as Box<dyn warp::reply::Reply>);
                    }

                    let backup_name = req.backup_name.trim();
                    if backup_name.is_empty()
                        || backup_name.contains("..")
                        || backup_name.starts_with('/')
                        || backup_name.contains('\\')
                    {
                        return Ok::<_, warp::Rejection>(Box::new(warp::reply::with_status(
                            warp::reply::json(
                                &serde_json::json!({ "error": "invalid backup name" }),
                            ),
                            warp::http::StatusCode::BAD_REQUEST,
                        ))
                            as Box<dyn warp::reply::Reply>);
                    }

                    let backup_dir = cfg.config_dir.join("backups");
                    let backup_path = backup_dir.join(backup_name);

                    if matches!(
                        (backup_path.canonicalize(), backup_dir.canonicalize()),
                        (Ok(ref canonical), Ok(ref base)) if !canonical.starts_with(base)
                    ) {
                        return Ok::<_, warp::Rejection>(Box::new(warp::reply::with_status(
                            warp::reply::json(&serde_json::json!({ "error": "path not allowed" })),
                            warp::http::StatusCode::BAD_REQUEST,
                        ))
                            as Box<dyn warp::reply::Reply>);
                    }

                    if !backup_path.exists() {
                        return Ok::<_, warp::Rejection>(Box::new(warp::reply::with_status(
                            warp::reply::json(&serde_json::json!({
                                "error": format!("Backup not found: {}", backup_name)
                            })),
                            warp::http::StatusCode::OK,
                        ))
                            as Box<dyn warp::reply::Reply>);
                    }

                    match std::fs::remove_file(&backup_path) {
                        Ok(_) => Ok::<_, warp::Rejection>(Box::new(warp::reply::with_status(
                            warp::reply::json(&serde_json::json!({
                                "status": "deleted",
                                "backup": backup_name,
                            })),
                            warp::http::StatusCode::OK,
                        ))
                            as Box<dyn warp::reply::Reply>),
                        Err(e) => {
                            eprintln!("Delete backup error: {e}");
                            Ok::<_, warp::Rejection>(Box::new(warp::reply::with_status(
                                warp::reply::json(&serde_json::json!({"error": e.to_string()})),
                                warp::http::StatusCode::OK,
                            ))
                                as Box<dyn warp::reply::Reply>)
                        }
                    }
                }
            },
        )
        .boxed()
}
