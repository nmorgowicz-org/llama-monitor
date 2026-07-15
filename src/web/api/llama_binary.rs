use std::path::Path;
use std::sync::Arc;

use warp::Filter;

use crate::config::AppConfig;
use crate::inference::launch::launch_local;
use crate::inference::llama_cpp::ServerConfig;
use crate::llama::llama_cpp_downloader::{
    ReleaseQuery, cleanup_old_binaries, download_and_extract, get_release_by_tag, list_releases,
    select_assets,
};
use crate::llama::server::{start_server, stop_server};
use crate::state::AppState;
use crate::web::safe_json_body;

use super::{ApiCtx, ApiRoute, box_reply, check_api_token, unauthorized_api_token};

pub(crate) fn routes(ctx: ApiCtx) -> ApiRoute {
    let state = ctx.state;
    let config = ctx.config;

    api_llama_binary_version(config.clone())
        .map(box_reply)
        .or(api_llama_binary_latest(config.clone()).map(box_reply))
        .unify()
        .or(api_llama_binary_releases(config.clone()).map(box_reply))
        .unify()
        .or(api_llama_binary_release(config.clone()).map(box_reply))
        .unify()
        .or(api_llama_binary_platform_info(config.clone()).map(box_reply))
        .unify()
        .or(api_llama_binary_update(state.clone(), config.clone()).map(box_reply))
        .unify()
        .or(api_llama_restart(state, config).map(box_reply))
        .unify()
        .boxed()
}

/// GET /api/llama-binary/version
fn api_llama_binary_version(
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (impl warp::reply::Reply,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "llama-binary" / "version")
        .and(warp::get())
        .and(warp::header::optional::<String>("authorization"))
        .and_then(move |auth: Option<String>| {
            let cfg = app_config.clone();
            async move {
                if !check_api_token(&auth, &cfg) {
                    return Ok(unauthorized_api_token());
                }

                let binary_path = cfg.llama_server_path.clone();
                let path_str = binary_path.display().to_string();

                let result = tokio::task::spawn_blocking(move || {
                    std::process::Command::new(&binary_path)
                        .arg("--version")
                        .output()
                })
                .await;

                let output = match result {
                    Ok(Ok(o)) => o,
                    _ => {
                        return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                            warp::reply::json(&serde_json::json!({
                                "build": serde_json::Value::Null,
                                "version": serde_json::Value::Null,
                                "path": path_str
                            })),
                        ));
                    }
                };

                let stdout = String::from_utf8_lossy(&output.stdout);
                let stderr = String::from_utf8_lossy(&output.stderr);
                let combined = format!("{}{}", stdout, stderr);

                // Try to parse build number from "version: 1234" or "build: 1234"
                let build_num: Option<u64> = {
                    use regex::Regex;
                    static VERSION_RE: std::sync::OnceLock<Regex> = std::sync::OnceLock::new();
                    let re = VERSION_RE
                        .get_or_init(|| Regex::new(r"(?:version|build)[:\s]+(\d+)").unwrap());
                    re.captures(&combined)
                        .and_then(|c| c.get(1))
                        .and_then(|m| m.as_str().parse().ok())
                };

                match build_num {
                    Some(n) => Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                        warp::reply::json(&serde_json::json!({
                            "build": n,
                            "version": format!("b{}", n),
                            "path": path_str
                        })),
                    )),
                    None => Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                        warp::reply::json(&serde_json::json!({
                            "build": serde_json::Value::Null,
                            "version": serde_json::Value::Null,
                            "path": path_str
                        })),
                    )),
                }
            }
        })
}

/// GET /api/llama-binary/latest — fetches latest release from GitHub with 30-min cache
fn api_llama_binary_latest(
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (impl warp::reply::Reply,), Error = warp::Rejection> + Clone {
    use std::sync::LazyLock;
    use tokio::sync::Mutex;

    static LATEST_CACHE: LazyLock<Mutex<Option<(std::time::Instant, serde_json::Value)>>> =
        LazyLock::new(|| Mutex::new(None));

    warp::path!("api" / "llama-binary" / "latest")
        .and(warp::get())
        .and(warp::header::optional::<String>("authorization"))
        .and_then(move |auth: Option<String>| {
            let cfg = app_config.clone();
            async move {
                if !check_api_token(&auth, &cfg) {
                    return Ok(unauthorized_api_token());
                }

                // Check cache
                {
                    let guard = LATEST_CACHE.lock().await;
                    if let Some((ts, ref cached)) = *guard
                        && ts.elapsed() < std::time::Duration::from_secs(30 * 60)
                    {
                        return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                            warp::reply::json(cached),
                        ));
                    }
                }
                // Fetch from GitHub
                let client = match reqwest::Client::builder()
                    .timeout(std::time::Duration::from_secs(20))
                    .user_agent("llama-monitor")
                    .build()
                {
                    Ok(c) => c,
                    Err(e) => {
                        return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                            warp::reply::json(&serde_json::json!({
                                "error": format!("Failed to create HTTP client: {}", e)
                            })),
                        ));
                    }
                };

                let url = "https://api.github.com/repos/ggml-org/llama.cpp/releases/latest";
                let resp = match client.get(url).send().await {
                    Ok(r) => r,
                    Err(e) => {
                        return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                            warp::reply::json(&serde_json::json!({
                                "error": format!("GitHub API request failed: {}", e)
                            })),
                        ));
                    }
                };

                if !resp.status().is_success() {
                    let status = resp.status();
                    return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                        warp::reply::json(&serde_json::json!({
                            "error": format!("GitHub API returned {}", status)
                        })),
                    ));
                }

                let release: serde_json::Value = match resp.json().await {
                    Ok(v) => v,
                    Err(e) => {
                        return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                            warp::reply::json(&serde_json::json!({
                                "error": format!("Failed to parse GitHub response: {}", e)
                            })),
                        ));
                    }
                };

                let tag = release["tag_name"].as_str().unwrap_or("").to_string();
                let published_at = release["published_at"].as_str().unwrap_or("").to_string();
                let asset_names: Vec<String> = release["assets"]
                    .as_array()
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|a| a["name"].as_str().map(str::to_string))
                            .collect()
                    })
                    .unwrap_or_default();

                // Parse build number from tag like "b4567"
                let build_num: Option<u64> = tag.trim_start_matches('b').parse().ok();

                let result = serde_json::json!({
                    "tag": tag,
                    "build": build_num,
                    "assets": asset_names,
                    "published_at": published_at
                });

                // Store in cache
                {
                    let mut guard = LATEST_CACHE.lock().await;
                    *guard = Some((std::time::Instant::now(), result.clone()));
                }

                Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(warp::reply::json(
                    &result,
                )))
            }
        })
}

/// GET /api/llama-binary/releases — lists the last 8 llama.cpp releases for the version picker
fn api_llama_binary_releases(
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (impl warp::reply::Reply,), Error = warp::Rejection> + Clone {
    use std::sync::LazyLock;
    use tokio::sync::Mutex;

    static RELEASES_CACHE: LazyLock<Mutex<Option<(std::time::Instant, serde_json::Value)>>> =
        LazyLock::new(|| Mutex::new(None));

    warp::path!("api" / "llama-binary" / "releases")
        .and(warp::get())
        .and(warp::header::optional::<String>("authorization"))
        .and_then(move |auth: Option<String>| {
            let cfg = app_config.clone();
            async move {
                if !check_api_token(&auth, &cfg) {
                    return Ok(unauthorized_api_token());
                }

                // Check 30-minute cache
                {
                    let guard = RELEASES_CACHE.lock().await;
                    if let Some((ts, ref cached)) = *guard
                        && ts.elapsed() < std::time::Duration::from_secs(30 * 60)
                    {
                        return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                            warp::reply::json(cached),
                        ));
                    }
                }

                let client = match reqwest::Client::builder()
                    .timeout(std::time::Duration::from_secs(20))
                    .user_agent("llama-monitor")
                    .build()
                {
                    Ok(c) => c,
                    Err(e) => {
                        return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                            warp::reply::json(&serde_json::json!({
                                "error": format!("Failed to create HTTP client: {}", e)
                            })),
                        ));
                    }
                };

                match list_releases(&client).await {
                    Ok(releases) => {
                        let items: Vec<serde_json::Value> = releases
                            .into_iter()
                            .take(8)
                            .map(|r| {
                                let build: Option<u64> =
                                    r.tag_name.trim_start_matches('b').parse().ok();
                                serde_json::json!({
                                    "tag": r.tag_name,
                                    "build": build,
                                    "published_at": r.published_at,
                                    "body": r.body,
                                })
                            })
                            .collect();
                        let result = serde_json::json!({ "releases": items });
                        {
                            let mut guard = RELEASES_CACHE.lock().await;
                            *guard = Some((std::time::Instant::now(), result.clone()));
                        }
                        Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                            warp::reply::json(&result),
                        ))
                    }
                    Err(e) => Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                        warp::reply::json(&serde_json::json!({
                            "error": format!("Failed to fetch releases: {}", e)
                        })),
                    )),
                }
            }
        })
}

/// GET /api/llama-binary/release?build=XXXXX — fetches a specific release by build number
fn api_llama_binary_release(
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (impl warp::reply::Reply,), Error = warp::Rejection> + Clone {
    use std::sync::LazyLock;
    use tokio::sync::Mutex;

    static RELEASE_SINGLE_CACHE: LazyLock<Mutex<Option<(std::time::Instant, serde_json::Value)>>> =
        LazyLock::new(|| Mutex::new(None));

    warp::path!("api" / "llama-binary" / "release")
        .and(warp::get())
        .and(warp::query::<ReleaseQuery>())
        .and(warp::header::optional::<String>("authorization"))
        .and_then(move |query: ReleaseQuery, auth: Option<String>| {
            let cfg = app_config.clone();
            async move {
                if !check_api_token(&auth, &cfg) {
                    return Ok(unauthorized_api_token());
                }

                let build = query.build;

                // Check per-build cache (5 min)
                {
                    let guard = RELEASE_SINGLE_CACHE.lock().await;
                    if let Some((ts, ref cached)) = *guard
                        && ts.elapsed() < std::time::Duration::from_secs(5 * 60)
                    {
                        return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                            warp::reply::json(cached),
                        ));
                    }
                }

                let client = match reqwest::Client::builder()
                    .timeout(std::time::Duration::from_secs(20))
                    .user_agent("llama-monitor")
                    .build()
                {
                    Ok(c) => c,
                    Err(e) => {
                        return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                            warp::reply::json(&serde_json::json!({
                                "error": format!("Failed to create HTTP client: {}", e)
                            })),
                        ));
                    }
                };

                let tag = format!("b{}", build);
                match get_release_by_tag(&client, &tag).await {
                    Ok(release) => {
                        let result = serde_json::json!({
                            "tag": release.tag_name,
                            "build": build,
                            "published_at": release.published_at,
                            "body": release.body,
                        });
                        {
                            let mut guard = RELEASE_SINGLE_CACHE.lock().await;
                            *guard = Some((std::time::Instant::now(), result.clone()));
                        }
                        Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                            warp::reply::json(&result),
                        ))
                    }
                    Err(e) => Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                        warp::reply::json(&serde_json::json!({
                            "error": format!("Failed to fetch release: {}", e)
                        })),
                    )),
                }
            }
        })
}

/// GET /api/llama-binary/platform-info — returns platform/backend info for the download UI
fn api_llama_binary_platform_info(
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (impl warp::reply::Reply,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "llama-binary" / "platform-info")
        .and(warp::get())
        .and(warp::header::optional::<String>("authorization"))
        .and_then(move |auth: Option<String>| {
            let cfg = app_config.clone();
            async move {
                if !check_api_token(&auth, &cfg) {
                    return Ok(unauthorized_api_token());
                }

                let os   = std::env::consts::OS;
                let arch = std::env::consts::ARCH;

                // Human-readable arch label
                let arch_label = match arch {
                    "aarch64" => "ARM64 (Apple Silicon)",
                    "x86_64"  => "x86-64",
                    other     => other,
                };

                // The backend this machine will auto-select on download
                let auto_backend = match os {
                    "macos"   => "metal",
                    "linux"   => "cpu",
                    _         => "avx2",  // Windows default
                };

                // Human-readable label shown before the download button
                let label = match (os, arch) {
                    ("macos", "aarch64") => "Apple Silicon Metal".to_string(),
                    ("macos", _)         => "macOS Metal (x86-64)".to_string(),
                    ("linux", "aarch64") => "Linux ARM64 (CPU)".to_string(),
                    ("linux", _)         => "Linux x86-64 (CPU)".to_string(),
                    ("windows", _)       => "Windows CPU (AVX2)".to_string(),
                    _                    => format!("{} / {}", os, arch),
                };

                // For multi-backend platforms, expose all selectable backends.
                // Windows has the most variety; Linux has a few; macOS is Metal-only.
                let backends: Vec<serde_json::Value> = match os {
                    "windows" => vec![
                        serde_json::json!({
                            "id": "avx2",
                            "label": "CPU (AVX2) — no GPU driver needed",
                            "note": "Universal fallback. Works on any CPU that supports AVX2 (2013+).",
                            "recommended": false
                        }),
                        serde_json::json!({
                            "id": "vulkan",
                            "label": "Vulkan — AMD / Intel / NVIDIA",
                            "note": "Best for AMD Radeon or Intel Arc. Also works on NVIDIA without CUDA.",
                            "recommended": false
                        }),
                        serde_json::json!({
                            "id": "cuda12",
                            "label": "CUDA 12.x — NVIDIA RTX 20/30/40 series",
                            "note": "Requires CUDA 12.x runtime. Typical for GTX 10xx through RTX 40xx.",
                            "recommended": false
                        }),
                        serde_json::json!({
                            "id": "cuda13",
                            "label": "CUDA 13.x — NVIDIA RTX 50 series (Blackwell)",
                            "note": "Requires CUDA 13.x runtime. For RTX 5070, 5080, 5090.",
                            "recommended": false
                        }),
                        serde_json::json!({
                            "id": "sycl",
                            "label": "SYCL / oneAPI — Intel Arc & Xe GPUs",
                            "note": "Requires Intel oneAPI runtime. For Arc A-series and Xe-HPC.",
                            "recommended": false
                        }),
                    ],
                    "linux" => vec![
                        serde_json::json!({
                            "id": "cpu",
                            "label": "CPU — universal",
                            "note": "No GPU driver required.",
                            "recommended": false
                        }),
                        serde_json::json!({
                            "id": "cuda12",
                            "label": "CUDA 12.x — NVIDIA GPU",
                            "note": "Requires NVIDIA CUDA 12.x runtime.",
                            "recommended": false
                        }),
                        serde_json::json!({
                            "id": "vulkan",
                            "label": "Vulkan — AMD / Intel / NVIDIA",
                            "note": "GPU acceleration via Vulkan driver.",
                            "recommended": false
                        }),
                        serde_json::json!({
                            "id": "rocm",
                            "label": "ROCm — AMD GPU",
                            "note": "Requires AMD ROCm runtime.",
                            "recommended": false
                        }),
                    ],
                    // macOS: Metal only — no choice needed
                    _ => vec![
                        serde_json::json!({
                            "id": "metal",
                            "label": if arch == "aarch64" {
                                "Metal — Apple Silicon (recommended)"
                            } else {
                                "Metal — Intel Mac"
                            },
                            "note": "Uses the GPU via Metal. Built in to macOS.",
                            "recommended": true
                        }),
                    ],
                };

                Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(warp::reply::json(
                    &serde_json::json!({
                        "os":           os,
                        "arch":         arch,
                        "arch_label":   arch_label,
                        "auto_backend": auto_backend,
                        "label":        label,
                        "backends":     backends,
                        "multi_backend": os == "windows" || os == "linux",
                    }),
                )))
            }
        })
}

fn describe_process_status(status: std::process::ExitStatus) -> String {
    if let Some(code) = status.code() {
        return format!("exit code {code}");
    }

    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        if let Some(signal) = status.signal() {
            return format!("signal {signal}");
        }
    }

    "exit status unknown".to_string()
}

async fn check_llama_server_binary(binary: &Path) -> Result<(), String> {
    let output = tokio::time::timeout(std::time::Duration::from_secs(10), async {
        tokio::process::Command::new(binary)
            .arg("--help")
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::piped())
            .output()
            .await
    })
    .await
    .map_err(|_| "health check timed out".to_string())?
    .map_err(|e| format!("spawn error: {e}"))?;

    if output.status.success() {
        return Ok(());
    }

    let status = describe_process_status(output.status);
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    if stderr.is_empty() {
        Err(status)
    } else {
        Err(format!("{status}: {stderr}"))
    }
}

/// POST /api/llama-binary/update — downloads latest release and overwrites llama-server binary
fn api_llama_binary_update(
    state: AppState,
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (impl warp::reply::Reply,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "llama-binary" / "update")
        .and(warp::post())
        .and(warp::header::optional::<String>("authorization"))
        .and(safe_json_body::<serde_json::Value>())
        .and_then(move |auth: Option<String>, _body: serde_json::Value| {
            let state = state.clone();
            let cfg = app_config.clone();
            async move {
                if !check_api_token(&auth, &cfg) {
                    return Ok(unauthorized_api_token());
                }

                // Allow updating while running: keep the current server alive while
                // network/download/preflight work happens, then stop only for the
                // final install window.
                let previous_config = llama_update_restart_config(&state);
                let restart_applicable = previous_config.is_some();

                let dest_path = cfg.llama_server_path.clone();

                let os = std::env::consts::OS;
                let arch = std::env::consts::ARCH;

                // Caller may override the backend (e.g. "cuda13" on Windows).
                // Fall back to the platform default if not provided.
                let default_backend = match os {
                    "macos" => "metal",
                    "linux" => "cpu",
                    _ => "avx2",
                };
                let backend_owned: String = _body
                    .get("backend")
                    .and_then(|v| v.as_str())
                    .filter(|s| !s.is_empty())
                    .unwrap_or(default_backend)
                    .to_string();
                let backend = backend_owned.as_str();

                // Caller may specify a specific tag (e.g. "b4567") to install a previous build.
                let requested_tag: Option<String> = _body
                    .get("tag")
                    .and_then(|v| v.as_str())
                    .filter(|s| !s.is_empty())
                    .map(str::to_string);

                let arch_str = match arch {
                    "aarch64" => "arm64",
                    "x86_64" => "x86_64",
                    other => other,
                };

                let client = match reqwest::Client::builder()
                    .timeout(std::time::Duration::from_secs(300))
                    .user_agent("llama-monitor")
                    .build()
                {
                    Ok(c) => c,
                    Err(e) => {
                        return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                            warp::reply::json(&serde_json::json!({
                                "ok": false,
                                "error": format!("Failed to create HTTP client: {}", e)
                            })),
                        ));
                    }
                };

                // Fetch release list; pick specific tag if requested, otherwise take latest.
                let mut releases =
                    match list_releases(&client).await {
                        Ok(r) => r,
                        Err(e) => {
                            return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                                warp::reply::json(&serde_json::json!({
                                    "ok": false,
                                    "error": format!("Failed to list releases: {}", e)
                                })),
                            ));
                        }
                    };

                if releases.is_empty() {
                    return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                        warp::reply::json(&serde_json::json!({
                            "ok": false,
                            "error": "No releases found on GitHub"
                        })),
                    ));
                }

                let release = if let Some(ref wanted) = requested_tag {
                    match releases.iter().position(|r| &r.tag_name == wanted) {
                        Some(idx) => releases.remove(idx),
                        None => {
                            return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                                warp::reply::json(&serde_json::json!({
                                    "ok": false,
                                    "error": format!("Tag {} not found in the last {} releases", wanted, releases.len())
                                })),
                            ));
                        }
                    }
                } else {
                    releases.remove(0)
                };
                let tag = release.tag_name.clone();

                let assets =
                    select_assets(&release, backend, arch_str);

                if assets.is_empty() {
                    return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                        warp::reply::json(&serde_json::json!({
                            "ok": false,
                            "error": format!(
                                "No matching assets for OS={} arch={} backend={}",
                                os, arch_str, backend
                            )
                        })),
                    ));
                }

                // Download + extract to a temp dir
                let tmp_dir = match tempfile::tempdir() {
                    Ok(d) => d,
                    Err(e) => {
                        return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                            warp::reply::json(&serde_json::json!({
                                "ok": false,
                                "error": format!("Failed to create temp dir: {}", e)
                            })),
                        ));
                    }
                };

                if let Err(e) = download_and_extract(
                    &client,
                    &release,
                    &assets,
                    tmp_dir.path(),
                )
                .await
                {
                    return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                        warp::reply::json(&serde_json::json!({
                            "ok": false,
                            "error": format!("Download/extract failed: {}", e)
                        })),
                    ));
                }

                let binary_name = if os == "windows" { "llama-server.exe" } else { "llama-server" };
                let dest_dir = dest_path.parent().unwrap_or(&dest_path);

                // Locate extracted binary in temp dir. Releases may place it at the root
                // or inside a subdirectory (e.g. llama-bXXXX-bin-...).
                fn find_binary(root: &std::path::Path, name: &str) -> Option<std::path::PathBuf> {
                    let direct = root.join(name);
                    if direct.is_file() {
                        return Some(direct);
                    }
                    for entry in std::fs::read_dir(root).ok()? {
                        let entry = entry.ok()?;
                        let path = entry.path();
                        if path.is_dir()
                            && let Some(p) = find_binary(&path, name)
                        {
                            return Some(p);
                        }
                    }
                    None
                }

                let tmp_binary = match find_binary(tmp_dir.path(), binary_name) {
                    Some(p) => p,
                    None => {
                        return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                            warp::reply::json(&serde_json::json!({
                                "ok": false,
                                "error": format!(
                                    "Could not find '{}' in extracted archive",
                                    binary_name
                                )
                            })),
                        ));
                    }
                };

                // Set executable bit before health check (unix).
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    let _ = std::fs::set_permissions(
                        &tmp_binary,
                        std::fs::Permissions::from_mode(0o755),
                    );
                }

                // On macOS, Gatekeeper quarantines the entire extracted archive — the
                // executable, dylibs, and Metal shaders alike. Strip recursively from the
                // whole temp dir so that both the health check and the subsequent
                // copy_all_files into dest_dir carry clean files.
                #[cfg(target_os = "macos")]
                {
                    let _ = std::process::Command::new("xattr")
                        .args(["-rd", "com.apple.quarantine"])
                        .arg(tmp_dir.path())
                        .output();
                }

                // If a server is currently running, stop it BEFORE health-checking the
                // new binary. When a model is loaded, Metal/GPU resources are in use and
                // a second llama-server process (even for --help) can block or time out.
                if previous_config.is_some() {
                    state.push_log(
                        "[monitor] llama-binary/update: server is running; stopping to allow update"
                            .into(),
                    );
                    if let Err(e) = stop_server(&state).await {
                        return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                            warp::reply::json(&serde_json::json!({
                                "ok": false,
                                "error": format!("Failed to stop running llama-server before update: {}", e)
                            })),
                        ));
                    }
                }

                // Health check on the temp binary BEFORE writing anything to dest_dir.
                // This ensures the live binary is never overwritten with a bad one.
                // Capture stderr to diagnose failures (Gatekeeper, missing dylib, etc.).
                if let Err(detail) = check_llama_server_binary(&tmp_binary).await {
                    state.push_log(format!(
                        "[monitor] llama-binary/update: new binary failed health check (llama-server --help): {}. Not installing.",
                        detail
                    ));
                    return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                        warp::reply::json(&serde_json::json!({
                            "ok": false,
                            "error": "New llama-server binary failed basic health check. \
                                downloaded file may be corrupted or incompatible. \
                                Try updating again or install manually."
                        })),
                    ));
                }

                // Log update intent.
                state.push_log(format!(
                    "[monitor] llama-binary/update: installing {} to {}",
                    tag,
                    dest_path.display()
                ));

                fn copy_all_files(
                    src: &std::path::Path,
                    dest: &std::path::Path,
                ) -> std::io::Result<()> {
                    for entry in std::fs::read_dir(src)?.filter_map(|e| e.ok()) {
                        let path = entry.path();
                        if path.is_dir() {
                            copy_all_files(&path, dest)?;
                        } else if let Some(fname) = path.file_name() {
                            std::fs::copy(&path, dest.join(fname))?;
                        }
                    }
                    Ok(())
                }

                fn configured_binary_path(
                    install_dir: &std::path::Path,
                    binary_name: &str,
                    dest_path: &std::path::Path,
                ) -> std::io::Result<std::path::PathBuf> {
                    let configured_name = dest_path
                        .file_name()
                        .unwrap_or_else(|| std::ffi::OsStr::new(binary_name));
                    let archive_path = install_dir.join(binary_name);
                    let configured_path = install_dir.join(configured_name);
                    if configured_path != archive_path && archive_path.exists() {
                        match std::fs::rename(&archive_path, &configured_path) {
                            Ok(()) => {}
                            Err(_) => {
                                std::fs::copy(&archive_path, &configured_path)?;
                                let _ = std::fs::remove_file(&archive_path);
                            }
                        }
                    }
                    Ok(configured_path)
                }

                #[cfg(target_os = "macos")]
                {
                    let dest_parent = dest_dir.parent().unwrap_or_else(|| std::path::Path::new("."));
                    let dest_name = dest_dir
                        .file_name()
                        .and_then(|name| name.to_str())
                        .unwrap_or("bin");
                    let stamp = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .map(|d| d.as_secs())
                        .unwrap_or(0);
                    let staging_dir = dest_parent.join(format!(
                        ".{dest_name}.llama-update-{tag}-{}-{stamp}",
                        std::process::id()
                    ));
                    let backup_dir = dest_parent.join(format!(
                        "{dest_name}-previous-{tag}-{}-{stamp}",
                        std::process::id()
                    ));

                    if staging_dir.exists() {
                        let _ = std::fs::remove_dir_all(&staging_dir);
                    }
                    if let Err(e) = std::fs::create_dir_all(&staging_dir) {
                        return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                            warp::reply::json(&serde_json::json!({
                                "ok": false,
                                "error": format!("Failed create staging bin dir {}: {}", staging_dir.display(), e)
                            })),
                        ));
                    }

                    if let Err(e) = copy_all_files(tmp_dir.path(), &staging_dir) {
                        return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                            warp::reply::json(&serde_json::json!({
                                "ok": false,
                                "error": format!("Failed copy release files to staging dir {}: {}", staging_dir.display(), e)
                            })),
                        ));
                    }

                    let staged_binary = match configured_binary_path(&staging_dir, binary_name, &dest_path) {
                        Ok(path) => path,
                        Err(e) => {
                            return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                                warp::reply::json(&serde_json::json!({
                                    "ok": false,
                                    "error": format!("Failed prepare staged binary name: {}", e)
                                })),
                            ));
                        }
                    };

                    if let Err(e) = cleanup_old_binaries(&staging_dir).await {
                        eprintln!("[warn] llama.cpp binary cleanup failed: {}", e);
                    }

                    if let Err(detail) = check_llama_server_binary(&staged_binary).await {
                        let _ = std::fs::remove_dir_all(&staging_dir);
                        state.push_log(format!(
                            "[monitor] llama-binary/update: staged binary failed health check (llama-server --help): {}. Not installing.",
                            detail
                        ));
                        return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                            warp::reply::json(&serde_json::json!({
                                "ok": false,
                                "error": format!("Staged llama-server binary failed basic health check: {}", detail)
                            })),
                        ));
                    }

                    if dest_dir.exists() {
                        if backup_dir.exists() {
                            let _ = std::fs::remove_dir_all(&backup_dir);
                        }
                        if let Err(e) = std::fs::rename(dest_dir, &backup_dir) {
                            let _ = std::fs::remove_dir_all(&staging_dir);
                            return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                                warp::reply::json(&serde_json::json!({
                                    "ok": false,
                                    "error": format!("Failed move current bin dir to backup {}: {}", backup_dir.display(), e)
                                })),
                            ));
                        }
                    }

                    if let Err(e) = std::fs::rename(&staging_dir, dest_dir) {
                        if backup_dir.exists() && !dest_dir.exists() {
                            let _ = std::fs::rename(&backup_dir, dest_dir);
                        }
                        return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                            warp::reply::json(&serde_json::json!({
                                "ok": false,
                                "error": format!("Failed promote staged llama.cpp bin dir {} to {}: {}", staging_dir.display(), dest_dir.display(), e)
                            })),
                        ));
                    }

                    if let Err(detail) = check_llama_server_binary(&dest_path).await {
                        state.push_log(format!(
                            "[monitor] llama-binary/update: installed binary failed health check after promote: {}.",
                            detail
                        ));
                        return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                            warp::reply::json(&serde_json::json!({
                                "ok": false,
                                "error": format!("Installed llama-server binary failed health check after promote: {}", detail)
                            })),
                        ));
                    }
                }

                #[cfg(not(target_os = "macos"))]
                {
                    if let Err(e) = std::fs::create_dir_all(dest_dir) {
                        return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                            warp::reply::json(&serde_json::json!({
                                "ok": false,
                                "error": format!("Failed create bin dir {}: {}", dest_dir.display(), e)
                            })),
                        ));
                    }

                    if let Err(e) = copy_all_files(tmp_dir.path(), dest_dir) {
                        return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                            warp::reply::json(&serde_json::json!({
                                "ok": false,
                                "error": format!("Failed copy release files to {}: {}", dest_dir.display(), e)
                            })),
                        ));
                    }

                    if let Err(e) = configured_binary_path(dest_dir, binary_name, &dest_path) {
                        return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                            warp::reply::json(&serde_json::json!({
                                "ok": false,
                                "error": format!("Failed prepare installed binary name: {}", e)
                            })),
                        ));
                    }

                    if let Err(e) = cleanup_old_binaries(dest_dir).await {
                        eprintln!("[warn] llama.cpp binary cleanup failed: {}", e);
                    }

                    if let Err(detail) = check_llama_server_binary(&dest_path).await {
                        state.push_log(format!(
                            "[monitor] llama-binary/update: installed binary failed health check (llama-server --help): {}.",
                            detail
                        ));
                        return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                            warp::reply::json(&serde_json::json!({
                                "ok": false,
                                "error": format!("Installed llama-server binary failed health check: {}", detail)
                            })),
                        ));
                    }
                }

                state.push_log(format!(
                    "[monitor] llama-binary/update: successfully installed {} (binary: {})",
                    tag,
                    dest_path.display()
                ));

                // Restart llama-server with previous config if it was running.
                // Track whether we restarted so the frontend skips its own restart call.
                let server_restarted = if let Some(rc) = previous_config {
                    state.push_log(
                        "[monitor] llama-binary/update: restarting llama-server with previous config".into(),
                    );

                    match start_server(Arc::new(state.clone()), rc, &cfg).await {
                        Ok(()) => {
                            state.push_log(
                                "[monitor] llama-binary/update: llama-server restarted successfully".into(),
                            );
                            true
                        }
                        Err(e) => {
                            state.push_log(format!(
                                "[monitor] llama-binary/update: restart failed (binary updated; start manually if needed): {}",
                                e
                            ));
                            false
                        }
                    }
                } else {
                    false
                };

                // Compute SHA256 of the llama-server binary so users can
                // verify integrity out-of-band (e.g. `sha256sum llama-server`).
                let installed_path = &dest_path;
                let sha256_hex = std::fs::read(installed_path).ok().map(|bytes| {
                    use sha2::Digest;
                    let mut hasher = sha2::Sha256::new();
                    hasher.update(&bytes);
                    hasher
                        .finalize()
                        .iter()
                        .map(|b| format!("{b:02x}"))
                        .collect::<String>()
                });

                Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(warp::reply::json(
                    &serde_json::json!({
                        "ok": true,
                        "version": tag,
                        "backend": backend,
                        "arch": arch_str,
                        "sha256": sha256_hex,
                        // True when the backend already restarted the server; frontend
                        // must skip its own /api/llama/restart call to avoid a double-restart.
                        "server_restarted": server_restarted,
                        // False when no llama.cpp backend was active (including Rapid-MLX).
                        "restart_applicable": restart_applicable,
                    }),
                )))
            }
        })
}

fn llama_update_restart_config(state: &AppState) -> Option<ServerConfig> {
    let local_running = *state.local_server_running.lock().unwrap();
    let llama_cpp_active = matches!(
        state.local_launch_request.lock().unwrap().as_ref(),
        Some(crate::inference::launch::LocalLaunchRequest::LlamaCpp(_))
    );
    if local_running && llama_cpp_active {
        state.server_config.lock().unwrap().clone()
    } else {
        None
    }
}

/// POST /api/llama/restart — restart the running llama-server with the current
/// binary (useful after installing a new llama-server version).
fn api_llama_restart(
    state: AppState,
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (impl warp::reply::Reply,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "llama" / "restart")
        .and(warp::post())
        .and(warp::header::optional::<String>("authorization"))
        .and_then(move |auth: Option<String>| {
            let state = state.clone();
            let cfg = app_config.clone();
            async move {
                if !check_api_token(&auth, &cfg) {
                    return Ok(unauthorized_api_token());
                }

                let state_clone = state.clone();
                let local_running = *state.local_server_running.lock().unwrap();

                if !local_running {
                    return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                        warp::reply::json(&serde_json::json!({
                            "ok": false,
                            "error": "No local llama-server is running."
                        })),
                    ));
                }

                // Preserve the backend-owned request before stop clears it.
                let saved_request = {
                    let guard = state_clone.local_launch_request.lock().unwrap();
                    guard.clone()
                };

                let Some(request) = saved_request else {
                    return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                        warp::reply::json(&serde_json::json!({
                            "ok": false,
                            "error": "No saved local launch configuration found."
                        })),
                    ));
                };

                state.push_log("[monitor] restart: stopping existing server".into());

                // Stop current server
                if let Err(e) = stop_server(&state_clone).await {
                    return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                        warp::reply::json(&serde_json::json!({
                            "ok": false,
                            "error": format!("Failed to stop server: {}", e)
                        })),
                    ));
                }

                // Brief pause to let the old process fully shut down
                let pause_start = std::time::Instant::now();
                tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
                state.push_log(format!(
                    "[monitor] restart: re-spawning after {}ms",
                    pause_start.elapsed().as_millis()
                ));

                // Restart the same backend with its backend-owned configuration.
                if let Err(e) = launch_local(Arc::new(state_clone.clone()), request, &cfg).await {
                    state.push_log(format!("[monitor] restart: start_server failed: {}", e));
                    return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                        warp::reply::json(&serde_json::json!({
                            "ok": false,
                            "error": format!("Failed to restart server: {}", e)
                        })),
                    ));
                }

                Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(warp::reply::json(
                    &serde_json::json!({
                        "ok": true,
                        "message": "Server restart initiated."
                    }),
                )))
            }
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn llama_update_does_not_restart_rapid_mlx_backend() {
        let state = AppState::default();
        *state.local_server_running.lock().unwrap() = true;
        *state.server_config.lock().unwrap() = Some(ServerConfig::default());
        *state.local_launch_request.lock().unwrap() =
            Some(crate::inference::launch::LocalLaunchRequest::RapidMlx(
                crate::inference::rapid_mlx::RapidMlxConfig {
                    model_path: "/models/rapid".into(),
                    ..Default::default()
                },
            ));

        assert!(llama_update_restart_config(&state).is_none());
    }
}
