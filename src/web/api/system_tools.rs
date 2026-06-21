use serde::Serialize;
use warp::Filter;

use super::{ApiCtx, ApiReply, ApiRoute, check_api_token, unauthorized_api_token};

pub(crate) fn routes(ctx: ApiCtx) -> ApiRoute {
    top_processes(ctx.clone())
        .or(purge_memory(ctx))
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
    // `ps -axm -o pid,rss,comm` on macOS: -m sorts by memory descending.
    // RSS is in KB on macOS.
    let output = std::process::Command::new("ps")
        .args(["-axm", "-o", "pid=,rss=,comm="])
        .output();

    let Ok(output) = output else {
        return vec![];
    };
    let text = String::from_utf8_lossy(&output.stdout);

    let mut procs: Vec<ProcessInfo> = text
        .lines()
        .filter_map(|line| {
            let line = line.trim();
            if line.is_empty() {
                return None;
            }
            let mut parts = line.splitn(3, char::is_whitespace);
            let pid: u32 = parts.next()?.trim().parse().ok()?;
            let rss_kb: f64 = parts.next()?.trim().parse().ok()?;
            let comm = parts.next().unwrap_or("").trim().to_string();

            // Skip kernel threads (rss == 0) and our own server process
            if rss_kb == 0.0 {
                return None;
            }

            let name = comm.split('/').next_back().unwrap_or(&comm).to_string();
            Some(ProcessInfo {
                pid,
                rss_mb: rss_kb / 1024.0,
                name,
                command: comm,
            })
        })
        .collect();

    // Already sorted by ps -m, but take top N excluding already-collected aggregate
    procs.truncate(n);
    procs
}

// ── Purge endpoint ────────────────────────────────────────────────────────────

#[derive(Serialize)]
struct PurgeResult {
    ok: bool,
    message: String,
}

fn purge_memory(ctx: ApiCtx) -> ApiRoute {
    let config = ctx.config;
    warp::path!("system" / "purge")
        .and(warp::post())
        .and(warp::header::optional::<String>("authorization"))
        .and_then(move |auth: Option<String>| {
            let config = config.clone();
            async move {
                if !check_api_token(&auth, &config) {
                    return Ok(unauthorized_api_token());
                }
                let result = run_purge();
                Ok::<ApiReply, warp::Rejection>(Box::new(warp::reply::json(&result)))
            }
        })
        .boxed()
}

fn run_purge() -> PurgeResult {
    // Use osascript to trigger the native macOS privilege dialog so the user
    // can authorise `purge` without the app needing to run as root.
    #[cfg(target_os = "macos")]
    {
        let script = r#"do shell script "purge" with administrator privileges"#;
        let output = std::process::Command::new("osascript")
            .args(["-e", script])
            .output();

        match output {
            Ok(o) if o.status.success() => PurgeResult {
                ok: true,
                message: "Memory purged — inactive pages cleared.".into(),
            },
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
