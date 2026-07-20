use serde::{Deserialize, Serialize};
#[cfg(target_os = "macos")]
use std::sync::{Mutex, OnceLock};
#[cfg(target_os = "macos")]
use std::time::{Duration, Instant};
use warp::Filter;

use super::common::{check_db_admin_token, unauthorized_db_admin_token};
use super::{ApiCtx, ApiReply, ApiRoute, check_api_token, unauthorized_api_token};

pub(crate) fn routes(ctx: ApiCtx) -> ApiRoute {
    top_processes(ctx.clone())
        .or(purge_memory(ctx.clone()))
        .unify()
        .or(wired_limit_get(ctx.clone()))
        .unify()
        .or(wired_limit_set(ctx.clone()))
        .unify()
        .boxed()
}

// ── Top-processes endpoint ────────────────────────────────────────────────────

#[derive(Serialize)]
struct ProcessInfo {
    pid: u32,
    name: String,
    rss_mb: f64,
    command: String,
}

fn top_processes(ctx: ApiCtx) -> ApiRoute {
    let config = ctx.config;
    warp::path!("system" / "top-processes")
        .and(warp::get())
        .and(warp::header::optional::<String>("authorization"))
        .and_then(move |auth: Option<String>| {
            let config = config.clone();
            async move {
                if !check_api_token(&auth, &config) {
                    return Ok(unauthorized_api_token());
                }
                let procs = get_top_processes(15);
                Ok::<ApiReply, warp::Rejection>(Box::new(warp::reply::json(&procs)))
            }
        })
        .boxed()
}

fn get_top_processes(n: usize) -> Vec<ProcessInfo> {
    let sys = sysinfo::System::new_all();
    let mut procs: Vec<ProcessInfo> = sys
        .processes()
        .iter()
        .filter_map(|(pid, process)| {
            let rss_bytes = process.memory();
            if rss_bytes == 0 {
                return None;
            }
            let command = if process.cmd().is_empty() {
                process.name().to_string_lossy().into_owned()
            } else {
                process
                    .cmd()
                    .iter()
                    .map(|part| part.to_string_lossy())
                    .collect::<Vec<_>>()
                    .join(" ")
            };
            Some(ProcessInfo {
                pid: pid.as_u32(),
                name: process.name().to_string_lossy().into_owned(),
                rss_mb: rss_bytes as f64 / 1024.0 / 1024.0,
                command,
            })
        })
        .collect();

    procs.sort_by(|a, b| {
        b.rss_mb
            .partial_cmp(&a.rss_mb)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    procs.truncate(n);
    procs
}

// ── Purge endpoint ────────────────────────────────────────────────────────────

#[derive(Serialize)]
struct PurgeResult {
    ok: bool,
    message: String,
}

#[derive(Deserialize)]
struct PurgeRequest {
    #[serde(default)]
    confirm: String,
}

fn purge_memory(ctx: ApiCtx) -> ApiRoute {
    let config = ctx.config;
    warp::path!("system" / "purge")
        .and(warp::post())
        .and(warp::header::optional::<String>("authorization"))
        .and(warp::body::json())
        .and_then(move |auth: Option<String>, body: PurgeRequest| {
            let config = config.clone();
            async move {
                if !check_db_admin_token(&auth, &config) {
                    return Ok(unauthorized_db_admin_token());
                }
                if body.confirm != "purge-memory" {
                    return Ok::<ApiReply, warp::Rejection>(Box::new(warp::reply::json(
                        &PurgeResult {
                            ok: false,
                            message: "Missing confirmation for memory purge.".to_string(),
                        },
                    )));
                }
                let result = run_purge();
                Ok::<ApiReply, warp::Rejection>(Box::new(warp::reply::json(&result)))
            }
        })
        .boxed()
}

fn run_purge() -> PurgeResult {
    #[cfg(target_os = "macos")]
    {
        static LAST_PURGE: OnceLock<Mutex<Option<Instant>>> = OnceLock::new();
        let mut last = LAST_PURGE
            .get_or_init(|| Mutex::new(None))
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if let Some(instant) = *last {
            let elapsed = instant.elapsed();
            if elapsed < Duration::from_secs(120) {
                let wait = Duration::from_secs(120) - elapsed;
                return PurgeResult {
                    ok: false,
                    message: format!(
                        "Memory purge is cooling down; try again in {} seconds.",
                        wait.as_secs().max(1)
                    ),
                };
            }
        }

        // Use osascript to trigger the native macOS privilege dialog so the user
        // can authorize `purge` without the app needing to run as root. This is
        // intentionally manual; automated privileged purging should use a
        // dedicated helper, not repeated hidden sudo calls.
        let script = r#"do shell script "purge" with administrator privileges"#;
        let output = std::process::Command::new("osascript")
            .args(["-e", script])
            .output();

        match output {
            Ok(o) if o.status.success() => {
                *last = Some(Instant::now());
                PurgeResult {
                    ok: true,
                    message: "macOS was asked to flush disk cache. This will not free mlock, wired, or model heap memory.".into(),
                }
            }
            Ok(o) => {
                let stderr = String::from_utf8_lossy(&o.stderr).into_owned();
                // User cancelled the auth dialog: osascript exits 1 with "User cancelled"
                if stderr.contains("cancelled") || stderr.contains("canceled") {
                    PurgeResult {
                        ok: false,
                        message: "Purge cancelled by user.".into(),
                    }
                } else {
                    PurgeResult {
                        ok: false,
                        message: format!("purge failed: {}", stderr.trim()),
                    }
                }
            }
            Err(e) => PurgeResult {
                ok: false,
                message: format!("Could not run osascript: {e}"),
            },
        }
    }

    #[cfg(not(target_os = "macos"))]
    PurgeResult {
        ok: false,
        message: "Memory purge is only supported on macOS.".into(),
    }
}

// ── Wired-limit endpoints ─────────────────────────────────────────────────────

#[derive(Serialize)]
struct WiredLimitGetResult {
    /// Current value of iogpu.wired_limit_mb from sysctl. 0 means using macOS default.
    current_mb: u64,
    /// Maximum allowed value based on RAM-relative safe bound (88% of total RAM).
    /// 0 if not available (non-macOS or RAM unknown).
    max_mb: u64,
    /// RAM-relative safe default when sysctl is unset (75% of total RAM).
    /// 0 if not available.
    safe_default_mb: u64,
    /// Total system RAM in bytes.
    total_ram_bytes: u64,
    /// Behavioral notes about persistence and restart requirements.
    behavior_notes: String,
}

#[derive(Deserialize)]
struct WiredLimitSetRequest {
    /// Requested wired limit in MiB. 0 clears to macOS default.
    value_mb: u64,
    /// Confirmation required: must be "set-wired-limit".
    confirm: String,
}

fn wired_limit_get(ctx: ApiCtx) -> ApiRoute {
    let config = ctx.config;
    warp::path!("system" / "wired-limit")
        .and(warp::get())
        .and(warp::header::optional::<String>("authorization"))
        .and_then(move |auth: Option<String>| {
            let config = config.clone();
            async move {
                if !check_api_token(&auth, &config) {
                    return Ok(unauthorized_api_token());
                }
                #[cfg(target_os = "macos")]
                {
                    let sys_info = crate::system::get_system_metrics();
                    let total_ram_bytes = (sys_info.ram_total_gb * 1024.0 * 1024.0 * 1024.0) as u64;
                    let current_mb = crate::gpu::apple::read_iogpu_wired_limit_mb();
                    let max_mb =
                        crate::gpu::apple::wired_limit_max_mb(total_ram_bytes).unwrap_or(0);
                    let safe_default_mb =
                        crate::gpu::apple::wired_limit_safe_default_mb(total_ram_bytes)
                            .unwrap_or(0);
                    let behavior_notes =
                        crate::gpu::apple::wired_limit_behavior_notes().to_string();
                    Ok::<ApiReply, warp::Rejection>(Box::new(warp::reply::json(
                        &WiredLimitGetResult {
                            current_mb,
                            max_mb,
                            safe_default_mb,
                            total_ram_bytes,
                            behavior_notes,
                        },
                    )))
                }
                #[cfg(not(target_os = "macos"))]
                {
                    let result = WiredLimitGetResult {
                        current_mb: 0,
                        max_mb: 0,
                        safe_default_mb: 0,
                        total_ram_bytes: 0,
                        behavior_notes:
                            "Wired limit is only applicable on macOS with Apple Silicon.".into(),
                    };
                    Ok::<ApiReply, warp::Rejection>(Box::new(warp::reply::json(&result)))
                }
            }
        })
        .boxed()
}

fn wired_limit_set(ctx: ApiCtx) -> ApiRoute {
    let config = ctx.config;
    warp::path!("system" / "wired-limit")
        .and(warp::post())
        .and(warp::header::optional::<String>("authorization"))
        .and(warp::body::json())
        .and_then(move |auth: Option<String>, body: WiredLimitSetRequest| {
            let config = config.clone();
            async move {
                if !check_db_admin_token(&auth, &config) {
                    return Ok(unauthorized_db_admin_token());
                }
                if body.confirm != "set-wired-limit" {
                    return Ok::<ApiReply, warp::Rejection>(Box::new(warp::reply::json(
                        &crate::gpu::apple::WiredLimitSetResult {
                            success: false,
                            actual_mb: 0,
                            previous_mb: 0,
                            error: Some(crate::gpu::apple::WiredLimitError::SysctlFailed {
                                reason: "Missing confirmation: confirm must be 'set-wired-limit'"
                                    .into(),
                            }),
                        },
                    )));
                }
                #[cfg(target_os = "macos")]
                {
                    let sys_info = crate::system::get_system_metrics();
                    let total_ram_bytes = (sys_info.ram_total_gb * 1024.0 * 1024.0 * 1024.0) as u64;
                    let result =
                        crate::gpu::apple::set_iogpu_wired_limit_mb(body.value_mb, total_ram_bytes);
                    Ok::<ApiReply, warp::Rejection>(Box::new(warp::reply::json(&result)))
                }
                #[cfg(not(target_os = "macos"))]
                {
                    let result = crate::gpu::apple::WiredLimitSetResult {
                        success: false,
                        actual_mb: 0,
                        previous_mb: 0,
                        error: Some(crate::gpu::apple::WiredLimitError::SysctlFailed {
                            reason: "Wired limit is only supported on macOS with Apple Silicon."
                                .into(),
                        }),
                    };
                    Ok::<ApiReply, warp::Rejection>(Box::new(warp::reply::json(&result)))
                }
            }
        })
        .boxed()
}
