use std::sync::Arc;

use warp::Filter;

use crate::config::AppConfig;
use crate::state::AppState;

use super::common::{ApiCtx, ApiRoute, check_api_token, unauthorized_api_token};

#[derive(serde::Serialize, serde::Deserialize)]
struct ChatTemplateInstallMeta {
    source_url: String,
    fetch_url: String,
    installed_at: String,
    sha256: String,
    /// Pinned HF commit SHA the content was fetched at, when resolvable. `None` for
    /// install-url (no git revision concept — sha256 is the immutability anchor there)
    /// or legacy installs that predate revision pinning.
    #[serde(default)]
    revision: Option<String>,
}

fn template_meta_path(dest: &std::path::Path) -> std::path::PathBuf {
    dest.with_extension("jinja.meta.json")
}

fn sha256_hex(content: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(content);
    hasher
        .finalize()
        .iter()
        .map(|b| format!("{:02x}", b))
        .collect::<String>()
}

/// Allows backdating `installed_at` (e.g. to a
/// legacy install's file mtime when backfilling metadata that never existed) and recording
/// the pinned HF revision the content was fetched at.
fn write_template_install_meta_at(
    dest: &std::path::Path,
    source_url: &str,
    fetch_url: &str,
    content: &[u8],
    installed_at_override: Option<String>,
    revision: Option<String>,
) -> ChatTemplateInstallMeta {
    let meta = ChatTemplateInstallMeta {
        source_url: source_url.to_string(),
        fetch_url: fetch_url.to_string(),
        installed_at: installed_at_override.unwrap_or_else(|| chrono::Utc::now().to_rfc3339()),
        sha256: sha256_hex(content),
        revision,
    };
    if let Ok(json) = serde_json::to_vec_pretty(&meta) {
        let _ = std::fs::write(template_meta_path(dest), json);
    }
    meta
}

#[derive(serde::Serialize, serde::Deserialize, Clone)]
struct ReleaseRecord {
    sha256: String,
    #[serde(default)]
    revision: Option<String>,
    source_url: String,
    fetch_url: String,
    installed_at: String,
    /// Retained copy of the content, relative to the chat-templates dir's `releases/` subdir.
    file: String,
}

fn chat_templates_releases_dir() -> Option<std::path::PathBuf> {
    dirs::home_dir().map(|h| {
        h.join(".config")
            .join("llama-monitor")
            .join("chat-templates")
            .join("releases")
    })
}

fn release_index_path(name: &str) -> Option<std::path::PathBuf> {
    chat_templates_releases_dir().map(|d| d.join(format!("{name}.index.json")))
}

fn read_release_index(name: &str) -> Vec<ReleaseRecord> {
    release_index_path(name)
        .and_then(|p| std::fs::read_to_string(p).ok())
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

/// Records a successful template install into the retained release history for `name`,
/// deduped by sha256. Stores a standalone copy of the content under `releases/` so that
/// rollback works even after the active file has been overwritten by a later update.
fn record_release(name: &str, meta: &ChatTemplateInstallMeta, content: &[u8]) {
    let Some(dir) = chat_templates_releases_dir() else {
        return;
    };
    let _ = std::fs::create_dir_all(&dir);

    let mut index = read_release_index(name);
    if index.iter().any(|r| r.sha256 == meta.sha256) {
        return;
    }

    let file_name = format!("{name}-{}.jinja", &meta.sha256[..16.min(meta.sha256.len())]);
    if std::fs::write(dir.join(&file_name), content).is_err() {
        return;
    }

    index.push(ReleaseRecord {
        sha256: meta.sha256.clone(),
        revision: meta.revision.clone(),
        source_url: meta.source_url.clone(),
        fetch_url: meta.fetch_url.clone(),
        installed_at: meta.installed_at.clone(),
        file: file_name,
    });

    if let (Some(path), Ok(json)) = (release_index_path(name), serde_json::to_vec_pretty(&index)) {
        let _ = std::fs::write(path, json);
    }
}

/// Resolves the exact commit SHA for a HF repo's `main` ref via the models API, so template
/// fetches can be pinned to an immutable revision instead of a moving branch. Returns `None`
/// on any failure (network, parse, missing field) — callers must fall back to unpinned
/// `raw/main/...` fetches rather than failing the install.
async fn resolve_hf_commit_sha(
    client: &reqwest::Client,
    repo: &str,
    hf_token: &Option<String>,
) -> Option<String> {
    let url = format!("https://huggingface.co/api/models/{repo}");
    let mut req = client.get(&url);
    if let Some(tok) = hf_token
        && !tok.is_empty()
    {
        req = req.header("Authorization", format!("Bearer {tok}"));
    }
    let resp = req.send().await.ok()?;
    if !resp.status().is_success() {
        return None;
    }
    let json: serde_json::Value = resp.json().await.ok()?;
    json["sha"].as_str().map(|s| s.to_string())
}

fn read_template_install_meta(path: &std::path::Path) -> Option<ChatTemplateInstallMeta> {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
}

// 1) POST /api/spawn-wizard/mtp-draft-check
fn api_spawn_wizard_mtp_draft_check(
    _state: AppState,
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (Box<dyn warp::reply::Reply>,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "spawn-wizard" / "mtp-draft-check")
        .and(warp::post())
        .and(warp::header::optional::<String>("authorization"))
        .and(super::super::safe_json_body::<serde_json::Value>())
        .and_then(move |auth: Option<String>, body: serde_json::Value| {
            let cfg = app_config.clone();
            async move {
                if !check_api_token(&auth, &cfg) {
                    return Ok(unauthorized_api_token());
                }

                let model_name = body["model_name"].as_str().unwrap_or("").to_string();
                let repo_id = body["repo_id"].as_str().unwrap_or("").to_string();
                let quant_label = body["quant_label"].as_str().unwrap_or("Q8_0").to_string();

                if model_name.is_empty() {
                    return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                        warp::reply::json(&serde_json::json!({
                            "ok": false,
                            "error": "Missing 'model_name' field in request body"
                        })),
                    ));
                }

                // Determine the Gemma4 tier
                let tier = crate::hf::resolve_gemma4_tier(&model_name.to_ascii_lowercase());
                if tier.is_none() {
                    return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                        warp::reply::json(&serde_json::json!({
                            "ok": false,
                            "error": "Model does not appear to be a Gemma4 model (no recognized tier)"
                        })),
                    ));
                }
                let tier = tier.unwrap();

                // Check for local draft model
                let models_dir = cfg.models_dir.as_deref();
                let local_draft = models_dir
                    .and_then(|p| crate::hf::find_compatible_gemma4_mtp_draft(p, &model_name));

                // Resolve Unsloth HF download info
                let hf_info = (!repo_id.is_empty())
                    .then(|| crate::hf::resolve_gemma4_mtp_draft(&repo_id, &quant_label))
                    .flatten();

                let draft_available = local_draft.is_some();

                // Construct HF download URL
                let hf_download_url = hf_info.as_ref().map(|(repo, filename, _)| {
                    format!(
                        "https://huggingface.co/{}/resolve/main/MTP/{}",
                        repo, filename
                    )
                });

                Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                    warp::reply::json(&serde_json::json!({
                        "ok": true,
                        "draft_available": draft_available,
                        "draft_path": local_draft.map(|p| p.to_string_lossy().to_string()),
                        "tier": tier,
                        "hf_download_url": hf_download_url,
                        "hf_repo_id": hf_info.as_ref().map(|(r, _, _)| r.clone()),
                        "hf_filename": hf_info.as_ref().map(|(_, f, _)| f.clone()),
                        "local_filename": hf_info.as_ref().map(|(_, _, l)| l.clone())
                    })),
                ))
            }
        })
}

// 2) POST /api/spawn-wizard/import-launch-file
fn api_spawn_wizard_import_launch_file(
    _state: AppState,
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (Box<dyn warp::reply::Reply>,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "spawn-wizard" / "import-launch-file")
        .and(warp::post())
        .and(warp::header::optional::<String>("authorization"))
        .and(super::super::safe_json_body::<serde_json::Value>())
        .and_then(move |auth: Option<String>, body: serde_json::Value| {
            let cfg = app_config.clone();
            async move {
                if !check_api_token(&auth, &cfg) {
                    return Ok(unauthorized_api_token());
                }

                let file = body["file"].as_str().unwrap_or("").to_string();

                if file.is_empty() {
                    return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                        warp::reply::json(&serde_json::json!({
                            "ok": false,
                            "error": "Missing 'file' field in request body"
                        })),
                    ));
                }

                match crate::llama::batch_import::import_launch_file(&file) {
                    Ok(result) => Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                        warp::reply::json(&serde_json::json!({
                            "ok": true,
                            "preset": result.preset,
                            "warnings": result.warnings
                        })),
                    )),
                    Err(e) => Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                        warp::reply::json(&serde_json::json!({
                            "ok": false,
                            "error": e
                        })),
                    )),
                }
            }
        })
}

/// Return true for hostnames/IP strings that resolve to private or loopback ranges.
/// Used to block SSRF in the chat-template fetch endpoint.
/// Returns `true` if the host is local, private, or a reserved TLD.
///
/// TODO: DNS rebinding is not mitigated by hostname checks alone.
/// For stronger SSRF protection, resolve the hostname to an IP address
/// and validate the resolved IP against private ranges before sending
/// the request (e.g., via a custom reqwest interceptor or pre-resolution).
pub(crate) fn is_private_host(host: &str) -> bool {
    // Loopback / localhost (case-insensitive)
    let lower = host.to_ascii_lowercase();
    if lower == "localhost" || lower == "ip6-localhost" || lower == "[::1]" {
        return true;
    }
    // Strip brackets from IPv6 literals
    let bare = host.trim_start_matches('[').trim_end_matches(']');
    if let Ok(addr) = bare.parse::<std::net::IpAddr>() {
        return match addr {
            std::net::IpAddr::V4(v4) => {
                v4.is_loopback()
                    || v4.is_private()
                    || v4.is_link_local()
                    || v4.is_broadcast()
                    || v4.is_documentation()
                    || v4.is_unspecified()
            }
            std::net::IpAddr::V6(v6) => {
                let s = v6.segments();
                v6.is_loopback()
                    || v6.is_unspecified()
                    // ULA: fc00::/7 (fc00:: – fdff::)
                    || (s[0] & 0xfe00) == 0xfc00
                    // Link-local: fe80::/10
                    || (s[0] & 0xffc0) == 0xfe80
            }
        };
    }
    // Block common internal hostnames.
    // Note: DNS rebinding (evil.com → 192.168.x.x at resolution time) is not
    // mitigated by hostname checks alone. This guard covers direct IP literals
    // and well-known internal names; for a hardened deployment add a DNS resolver
    // check or restrict to an allowlist of known-good domains.
    let lower = host.to_ascii_lowercase();
    lower.ends_with(".local")
        || lower.ends_with(".internal")
        || lower.ends_with(".corp")
        || lower.ends_with(".lan")
}

// 3) POST /api/chat-template/fetch
fn api_chat_template_fetch(
    _state: AppState,
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (Box<dyn warp::reply::Reply>,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "chat-template" / "fetch")
        .and(warp::post())
        .and(warp::header::optional::<String>("authorization"))
        .and(super::super::safe_json_body::<serde_json::Value>())
        .and_then(move |auth: Option<String>, body: serde_json::Value| {
            let cfg = app_config.clone();
            async move {
                if !check_api_token(&auth, &cfg) {
                    return Ok(unauthorized_api_token());
                }

                let source_type = body["source_type"].as_str().unwrap_or("").to_string();
                let source = body["source"].as_str().unwrap_or("").to_string();

                if source_type != "url" {
                    return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                        warp::reply::json(&serde_json::json!({
                            "ok": false,
                            "error": "Unsupported source_type; only 'url' is supported"
                        })),
                    ));
                }

                if source.is_empty() {
                    return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                        warp::reply::json(&serde_json::json!({
                            "ok": false,
                            "error": "Missing 'source' URL"
                        })),
                    ));
                }

                // SSRF guard: only allow https:// to public hosts.
                match reqwest::Url::parse(&source) {
                    Err(_) => {
                        return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                            warp::reply::json(&serde_json::json!({
                                "ok": false,
                                "error": "Invalid URL"
                            })),
                        ));
                    }
                    Ok(ref u) => {
                        if u.scheme() != "https" {
                            return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                                warp::reply::json(&serde_json::json!({
                                    "ok": false,
                                    "error": "Only https:// URLs are supported"
                                })),
                            ));
                        }
                        let host = u.host_str().unwrap_or("");
                        if is_private_host(host) {
                            return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                                warp::reply::json(&serde_json::json!({
                                    "ok": false,
                                    "error": "URL resolves to a private or loopback address"
                                })),
                            ));
                        }
                    }
                }

                let client = match reqwest::Client::builder()
                    .timeout(std::time::Duration::from_secs(30))
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

                match client.get(&source).send().await {
                    Ok(resp) if resp.status().is_success() => match resp.text().await {
                        Ok(text) => Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                            warp::reply::json(&serde_json::json!({
                                "ok": true,
                                "template": text,
                                "source_url": source
                            })),
                        )),
                        Err(e) => Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                            warp::reply::json(&serde_json::json!({
                                "ok": false,
                                "error": format!("Failed to read response body: {}", e)
                            })),
                        )),
                    },
                    Ok(resp) => {
                        let status = resp.status();
                        Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                            warp::reply::json(&serde_json::json!({
                                "ok": false,
                                "error": format!("HTTP {} while fetching template", status.as_u16())
                            })),
                        ))
                    }
                    Err(e) => Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                        warp::reply::json(&serde_json::json!({
                            "ok": false,
                            "error": format!("Failed to fetch URL: {}", e)
                        })),
                    )),
                }
            }
        })
}

// 4) POST /api/chat-template/upload
fn api_chat_template_upload(
    _state: AppState,
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (Box<dyn warp::reply::Reply>,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "chat-template" / "upload")
        .and(warp::post())
        .and(warp::header::optional::<String>("authorization"))
        .and(super::super::safe_json_body::<serde_json::Value>())
        .and_then(move |auth: Option<String>, body: serde_json::Value| {
            let cfg = app_config.clone();
            async move {
                if !check_api_token(&auth, &cfg) {
                    return Ok(unauthorized_api_token());
                }

                let template = body["template"].as_str().unwrap_or("").to_string();

                if template.is_empty() {
                    return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                        warp::reply::json(&serde_json::json!({
                            "ok": false,
                            "error": "Missing 'template' field in request body"
                        })),
                    ));
                }

                let ts = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis();
                let template_id = format!("temp-{}", ts);

                // Persist the template to the config directory so it can be
                // referenced by the spawn wizard via --chat-template-file.
                let saved_path: Option<String> = (|| {
                    let home = dirs::home_dir()?;
                    let dir = home
                        .join(".config")
                        .join("llama-monitor")
                        .join("chat-templates");
                    std::fs::create_dir_all(&dir).ok()?;
                    let path = dir.join(format!("{template_id}.jinja"));
                    std::fs::write(&path, template.as_bytes()).ok()?;
                    Some(path.to_string_lossy().into_owned())
                })();

                Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(warp::reply::json(
                    &serde_json::json!({
                        "ok": true,
                        "template_id": template_id,
                        "template": template,
                        "path": saved_path
                    }),
                )))
            }
        })
}

// 5) GET /api/chat-template/dir
fn api_chat_template_dir(
    _state: AppState,
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (Box<dyn warp::reply::Reply>,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "chat-template" / "dir")
        .and(warp::get())
        .and(warp::header::optional::<String>("authorization"))
        .and_then(move |auth: Option<String>| {
            let cfg = app_config.clone();
            async move {
                if !check_api_token(&auth, &cfg) {
                    return Ok(unauthorized_api_token());
                }

                let Some(home) = dirs::home_dir() else {
                    return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                        warp::reply::json(&serde_json::json!({
                            "ok": false,
                            "error": "Could not determine home directory"
                        })),
                    ));
                };

                let dir = home
                    .join(".config")
                    .join("llama-monitor")
                    .join("chat-templates");
                if let Err(e) = std::fs::create_dir_all(&dir) {
                    return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                        warp::reply::json(&serde_json::json!({
                            "ok": false,
                            "error": format!("Failed to create template directory: {e}")
                        })),
                    ));
                }

                Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(warp::reply::json(
                    &serde_json::json!({
                        "ok": true,
                        "path": dir.to_string_lossy().to_string()
                    }),
                )))
            }
        })
}

// 6) POST /api/chat-template/install-hf
// Downloads a Jinja template from HuggingFace and saves it with a stable name.
// Returns the cached path immediately if the file already exists.
fn api_chat_template_install_hf(
    _state: AppState,
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (Box<dyn warp::reply::Reply>,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "chat-template" / "install-hf")
        .and(warp::post())
        .and(warp::header::optional::<String>("authorization"))
        .and(super::super::safe_json_body::<serde_json::Value>())
        .and_then(move |auth: Option<String>, body: serde_json::Value| {
            let cfg = app_config.clone();
            async move {
                if !check_api_token(&auth, &cfg) {
                    return Ok(unauthorized_api_token());
                }

                let repo = body["repo"].as_str().unwrap_or("").to_string();
                let file = body["file"].as_str().unwrap_or("").to_string();
                let name = body["name"].as_str().unwrap_or("").to_string();
                let force = body["force"].as_bool().unwrap_or(false);

                if repo.is_empty() || file.is_empty() || name.is_empty() {
                    return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                        warp::reply::json(&serde_json::json!({
                            "ok": false,
                            "error": "Missing required fields: repo, file, name"
                        })),
                    ));
                }
                // Safe filename — alphanumeric + hyphens/underscores only
                if !name.chars().all(|c| c.is_alphanumeric() || c == '-' || c == '_') {
                    return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                        warp::reply::json(&serde_json::json!({
                            "ok": false,
                            "error": "name must contain only alphanumeric characters, hyphens, or underscores"
                        })),
                    ));
                }
                // SSRF guard: repo must be "owner/name" — no path traversal, no extra slashes
                if repo.contains("..") || repo.contains("//") || repo.matches('/').count() != 1 {
                    return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                        warp::reply::json(&serde_json::json!({ "ok": false, "error": "Invalid repo format" })),
                    ));
                }
                if file.contains("..") || file.starts_with('/') {
                    return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                        warp::reply::json(&serde_json::json!({ "ok": false, "error": "Invalid file path" })),
                    ));
                }

                // Stable on-disk location
                let dest = match dirs::home_dir() {
                    Some(h) => h
                        .join(".config")
                        .join("llama-monitor")
                        .join("chat-templates")
                        .join(format!("{name}.jinja")),
                    None => {
                        return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                            warp::reply::json(&serde_json::json!({
                                "ok": false,
                                "error": "Could not determine home directory"
                            })),
                        ))
                    }
                };

                // Return cached file if it already exists and force is not set
                if dest.exists() && !force {
                    let existing_meta = read_template_install_meta(&template_meta_path(&dest));
                    let source_url = existing_meta.as_ref().map(|m| m.source_url.clone());
                    let installed_at = existing_meta.as_ref().map(|m| m.installed_at.clone());
                    return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                        warp::reply::json(&serde_json::json!({
                            "ok": true,
                            "path": dest.to_string_lossy(),
                            "already_existed": true,
                            "source_url": source_url,
                            "installed_at": installed_at
                        })),
                    ));
                }

                let hf_token = crate::hf::hf_load_token();

                let client = match reqwest::Client::builder()
                    .timeout(std::time::Duration::from_secs(30))
                    .user_agent("llama-monitor/1.0")
                    .build()
                {
                    Ok(c) => c,
                    Err(e) => {
                        return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                            warp::reply::json(&serde_json::json!({
                                "ok": false,
                                "error": format!("HTTP client error: {e}")
                            })),
                        ))
                    }
                };

                // Best-effort pin to an exact commit SHA so re-fetches are reproducible and
                // history/rollback records an immutable revision, not a moving branch ref.
                let revision = resolve_hf_commit_sha(&client, &repo, &hf_token).await;
                let url = match &revision {
                    Some(sha) => format!("https://huggingface.co/{repo}/raw/{sha}/{file}"),
                    None => format!("https://huggingface.co/{repo}/raw/main/{file}"),
                };

                let mut req = client.get(&url);
                if let Some(ref tok) = hf_token
                    && !tok.is_empty()
                {
                    req = req.header("Authorization", format!("Bearer {tok}"));
                }

                let content = match req.send().await {
                    Ok(resp) if resp.status().is_success() => match resp.text().await {
                        Ok(t) => t,
                        Err(e) => {
                            return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                                warp::reply::json(&serde_json::json!({
                                    "ok": false,
                                    "error": format!("Failed to read response: {e}")
                                })),
                            ))
                        }
                    },
                    Ok(resp) => {
                        let status = resp.status().as_u16();
                        return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                            warp::reply::json(&serde_json::json!({
                                "ok": false,
                                "error": format!("HTTP {status} from HuggingFace")
                            })),
                        ));
                    }
                    Err(e) => {
                        return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                            warp::reply::json(&serde_json::json!({
                                "ok": false,
                                "error": format!("Network error: {e}")
                            })),
                        ))
                    }
                };

                let source_url = format!("https://huggingface.co/{repo}/blob/main/{file}");

                if let Some(parent) = dest.parent() {
                    let _ = std::fs::create_dir_all(parent);
                }
                if let Err(e) = std::fs::write(&dest, content.as_bytes()) {
                    return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                        warp::reply::json(&serde_json::json!({
                            "ok": false,
                            "error": format!("Failed to save template: {e}")
                        })),
                    ));
                }

                let meta = write_template_install_meta_at(
                    &dest,
                    &source_url,
                    &url,
                    content.as_bytes(),
                    None,
                    revision.clone(),
                );
                record_release(&name, &meta, content.as_bytes());

                Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(warp::reply::json(
                    &serde_json::json!({
                        "ok": true,
                        "path": dest.to_string_lossy(),
                        "already_existed": false,
                        "source_url": source_url,
                        "revision": revision
                    }),
                )))
            }
        })
}

// 7) POST /api/chat-template/install-url
// Downloads a community template from raw.githubusercontent.com and saves it
// with a stable name. The host allowlist keeps this separate from arbitrary
// URL fetching and prevents redirects to untrusted hosts.
fn api_chat_template_install_url(
    _state: AppState,
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (Box<dyn warp::reply::Reply>,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "chat-template" / "install-url")
        .and(warp::post())
        .and(warp::header::optional::<String>("authorization"))
        .and(super::super::safe_json_body::<serde_json::Value>())
        .and_then(move |auth: Option<String>, body: serde_json::Value| {
            let cfg = app_config.clone();
            async move {
                if !check_api_token(&auth, &cfg) {
                    return Ok(unauthorized_api_token());
                }

                let source = body["url"].as_str().unwrap_or("").to_string();
                let name = body["name"].as_str().unwrap_or("").to_string();
                let force = body["force"].as_bool().unwrap_or(false);

                if source.is_empty() || name.is_empty() {
                    return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                        warp::reply::json(&serde_json::json!({
                            "ok": false,
                            "error": "Missing required fields: url, name"
                        })),
                    ));
                }
                if !name
                    .chars()
                    .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
                {
                    return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                        warp::reply::json(&serde_json::json!({
                            "ok": false,
                            "error": "name must contain only alphanumeric characters, hyphens, or underscores"
                        })),
                    ));
                }

                let url = match reqwest::Url::parse(&source) {
                    Ok(url)
                        if url.scheme() == "https"
                            && url.host_str() == Some("raw.githubusercontent.com") =>
                    {
                        url
                    }
                    _ => {
                        return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                            warp::reply::json(&serde_json::json!({
                                "ok": false,
                                "error": "Only https://raw.githubusercontent.com URLs are supported"
                            })),
                        ));
                    }
                };

                let dest = match dirs::home_dir() {
                    Some(home) => home
                        .join(".config")
                        .join("llama-monitor")
                        .join("chat-templates")
                        .join(format!("{name}.jinja")),
                    None => {
                        return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                            warp::reply::json(&serde_json::json!({
                                "ok": false,
                                "error": "Could not determine home directory"
                            })),
                        ));
                    }
                };

                if dest.exists() && !force {
                    let existing_meta = read_template_install_meta(&template_meta_path(&dest));
                    let source_url = existing_meta.as_ref().map(|m| m.source_url.clone());
                    let installed_at = existing_meta.as_ref().map(|m| m.installed_at.clone());
                    return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                        warp::reply::json(&serde_json::json!({
                            "ok": true,
                            "path": dest.to_string_lossy(),
                            "already_existed": true,
                            "source_url": source_url,
                            "installed_at": installed_at
                        })),
                    ));
                }

                let client = match reqwest::Client::builder()
                    .timeout(std::time::Duration::from_secs(30))
                    .redirect(reqwest::redirect::Policy::none())
                    .user_agent("llama-monitor/1.0")
                    .build()
                {
                    Ok(client) => client,
                    Err(e) => {
                        return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                            warp::reply::json(&serde_json::json!({
                                "ok": false,
                                "error": format!("HTTP client error: {e}")
                            })),
                        ));
                    }
                };

                let content = match client.get(url).send().await {
                    Ok(resp) if resp.status().is_success() => match resp.bytes().await {
                        Ok(bytes) if bytes.len() <= 1024 * 1024 => bytes,
                        Ok(_) => {
                            return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                                warp::reply::json(&serde_json::json!({
                                    "ok": false,
                                    "error": "Template exceeds the 1 MiB size limit"
                                })),
                            ));
                        }
                        Err(e) => {
                            return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                                warp::reply::json(&serde_json::json!({
                                    "ok": false,
                                    "error": format!("Failed to read response: {e}")
                                })),
                            ));
                        }
                    },
                    Ok(resp) => {
                        return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                            warp::reply::json(&serde_json::json!({
                                "ok": false,
                                "error": format!("HTTP {} from GitHub", resp.status().as_u16())
                            })),
                        ));
                    }
                    Err(e) => {
                        return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                            warp::reply::json(&serde_json::json!({
                                "ok": false,
                                "error": format!("Network error: {e}")
                            })),
                        ));
                    }
                };

                if let Some(parent) = dest.parent()
                    && let Err(e) = std::fs::create_dir_all(parent)
                {
                    return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                        warp::reply::json(&serde_json::json!({
                            "ok": false,
                            "error": format!("Failed to create template directory: {e}")
                        })),
                    ));
                }
                let temp = dest.with_extension("jinja.tmp");
                if let Err(e) =
                    std::fs::write(&temp, &content).and_then(|_| std::fs::rename(&temp, &dest))
                {
                    let _ = std::fs::remove_file(&temp);
                    return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                        warp::reply::json(&serde_json::json!({
                            "ok": false,
                            "error": format!("Failed to save template: {e}")
                        })),
                    ));
                }

                let meta = write_template_install_meta_at(&dest, &source, &source, &content, None, None);
                record_release(&name, &meta, &content);

                Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(warp::reply::json(
                    &serde_json::json!({
                        "ok": true,
                        "path": dest.to_string_lossy(),
                        "already_existed": false,
                        "source_url": source
                    }),
                )))
            }
        })
}

// 7) GET /api/chat-template/active
// Returns all installed community templates (those with a valid meta.json),
// used by the frontend auto-updater to know which templates to check.
fn api_chat_template_active(
    _state: AppState,
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (Box<dyn warp::reply::Reply>,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "chat-template" / "active")
        .and(warp::get())
        .and(warp::header::optional::<String>("authorization"))
        .and_then(move |auth: Option<String>| {
            let cfg = app_config.clone();
            async move {
                if !check_api_token(&auth, &cfg) {
                    return Ok(unauthorized_api_token());
                }

                let Some(home) = dirs::home_dir() else {
                    return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                        warp::reply::json(&serde_json::json!({
                            "ok": false,
                            "error": "Could not determine home directory"
                        })),
                    ));
                };

                let dir = home
                    .join(".config")
                    .join("llama-monitor")
                    .join("chat-templates");

                let mut list: Vec<serde_json::Value> = Vec::new();
                if let Ok(entries) = std::fs::read_dir(&dir) {
                    for entry in entries.flatten() {
                        let path = entry.path();
                        if !path.is_file()
                            || !path.extension().map(|e| e == "jinja").unwrap_or(false)
                        {
                            continue;
                        }
                        let meta_path = template_meta_path(&path);
                        let Some(meta) = read_template_install_meta(&meta_path) else {
                            continue;
                        };
                        if meta.fetch_url.is_empty() {
                            continue;
                        }
                        let name = path
                            .file_stem()
                            .map(|s| s.to_string_lossy().to_string())
                            .unwrap_or_default();

                        list.push(serde_json::json!({
                            "name": name,
                            "path": path.to_string_lossy().to_string(),
                            "fetch_url": meta.fetch_url,
                            "source_url": meta.source_url,
                            "installed_sha256": meta.sha256,
                            "installed_at": meta.installed_at
                        }));
                    }
                }

                Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(warp::reply::json(
                    &serde_json::json!({
                        "ok": true,
                        "templates": list
                    }),
                )))
            }
        })
}

// 8) GET /api/chat-template/releases?name=...
// Returns the retained release history for a named template, newest first, for the
// version-history / rollback UI.
fn api_chat_template_releases(
    _state: AppState,
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (Box<dyn warp::reply::Reply>,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "chat-template" / "releases")
        .and(warp::get())
        .and(warp::header::optional::<String>("authorization"))
        .and(warp::query::<std::collections::HashMap<String, String>>())
        .and_then(move |auth: Option<String>, q: std::collections::HashMap<String, String>| {
            let cfg = app_config.clone();
            async move {
                if !check_api_token(&auth, &cfg) {
                    return Ok(unauthorized_api_token());
                }
                let name = q.get("name").cloned().unwrap_or_default();
                if name.is_empty() || !name.chars().all(|c| c.is_alphanumeric() || c == '-' || c == '_') {
                    return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                        warp::reply::json(&serde_json::json!({ "ok": false, "error": "Missing or invalid name" })),
                    ));
                }

                let mut releases = read_release_index(&name);
                releases.sort_by(|a, b| b.installed_at.cmp(&a.installed_at));

                let active_sha = dirs::home_dir()
                    .map(|h| {
                        h.join(".config")
                            .join("llama-monitor")
                            .join("chat-templates")
                            .join(format!("{name}.jinja"))
                    })
                    .and_then(|dest| read_template_install_meta(&template_meta_path(&dest)))
                    .map(|m| m.sha256);

                Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(warp::reply::json(
                    &serde_json::json!({
                        "ok": true,
                        "releases": releases,
                        "active_sha256": active_sha
                    }),
                )))
            }
        })
}

// 9) POST /api/chat-template/activate
// Rolls the active template back (or forward) to a specific retained release by sha256.
fn api_chat_template_activate(
    _state: AppState,
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (Box<dyn warp::reply::Reply>,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "chat-template" / "activate")
        .and(warp::post())
        .and(warp::header::optional::<String>("authorization"))
        .and(super::super::safe_json_body::<serde_json::Value>())
        .and_then(move |auth: Option<String>, body: serde_json::Value| {
            let cfg = app_config.clone();
            async move {
                if !check_api_token(&auth, &cfg) {
                    return Ok(unauthorized_api_token());
                }
                let name = body["name"].as_str().unwrap_or("").to_string();
                let sha256 = body["sha256"].as_str().unwrap_or("").to_string();
                if name.is_empty()
                    || sha256.is_empty()
                    || !name.chars().all(|c| c.is_alphanumeric() || c == '-' || c == '_')
                {
                    return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                        warp::reply::json(&serde_json::json!({ "ok": false, "error": "Missing or invalid name/sha256" })),
                    ));
                }

                let releases = read_release_index(&name);
                let Some(record) = releases.into_iter().find(|r| r.sha256 == sha256) else {
                    return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                        warp::reply::json(&serde_json::json!({ "ok": false, "error": "Release not found in history" })),
                    ));
                };

                let Some(releases_dir) = chat_templates_releases_dir() else {
                    return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                        warp::reply::json(&serde_json::json!({ "ok": false, "error": "Could not determine home directory" })),
                    ));
                };
                let Ok(content) = std::fs::read(releases_dir.join(&record.file)) else {
                    return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                        warp::reply::json(&serde_json::json!({ "ok": false, "error": "Retained release content missing on disk" })),
                    ));
                };

                let Some(dest) = dirs::home_dir().map(|h| {
                    h.join(".config")
                        .join("llama-monitor")
                        .join("chat-templates")
                        .join(format!("{name}.jinja"))
                }) else {
                    return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                        warp::reply::json(&serde_json::json!({ "ok": false, "error": "Could not determine home directory" })),
                    ));
                };

                if let Err(e) = std::fs::write(&dest, &content) {
                    return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                        warp::reply::json(&serde_json::json!({ "ok": false, "error": format!("Failed to activate release: {e}") })),
                    ));
                }
                write_template_install_meta_at(
                    &dest,
                    &record.source_url,
                    &record.fetch_url,
                    &content,
                    None,
                    record.revision.clone(),
                );

                Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(warp::reply::json(
                    &serde_json::json!({ "ok": true, "path": dest.to_string_lossy(), "sha256": record.sha256 }),
                )))
            }
        })
}

// 10) POST /api/chat-template/check-update
fn api_chat_template_check_update(
    _state: AppState,
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (Box<dyn warp::reply::Reply>,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "chat-template" / "check-update")
        .and(warp::post())
        .and(warp::header::optional::<String>("authorization"))
        .and(super::super::safe_json_body::<serde_json::Value>())
        .and_then(move |auth: Option<String>, body: serde_json::Value| {
            let cfg = app_config.clone();
            async move {
                if !check_api_token(&auth, &cfg) {
                    return Ok(unauthorized_api_token());
                }

                let path_str = body["path"].as_str().unwrap_or("").to_string();
                if path_str.is_empty() {
                    return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                        warp::reply::json(&serde_json::json!({
                            "ok": false,
                            "error": "Missing 'path' field in request body"
                        })),
                    ));
                }

                let path = std::path::Path::new(&path_str);
                let meta_path = template_meta_path(path);
                let existing_meta = read_template_install_meta(&meta_path);

                // Legacy installs (from before update-tracking metadata existed) have no
                // meta.json. Rather than refusing to check, fall back to the fetch_url the
                // caller already knows for this template (from the community-template
                // registry) and diff upstream against the sha256 of the file on disk. If
                // the file is unchanged, backfill meta.json so future checks use it directly.
                let fallback_fetch_url = body["fetch_url"].as_str().map(|s| s.to_string());
                let fallback_source_url = body["source_url"].as_str().map(|s| s.to_string());

                let (fetch_url, baseline_sha, baseline_installed_at, baseline_source_url) =
                    match existing_meta {
                        Some(ref m) => (
                            m.fetch_url.clone(),
                            m.sha256.clone(),
                            Some(m.installed_at.clone()),
                            m.source_url.clone(),
                        ),
                        None => {
                            let Some(fetch_url) = fallback_fetch_url else {
                                return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                                    warp::reply::json(&serde_json::json!({
                                        "ok": false,
                                        "error": "No update history for this install (predates update tracking). Use \"Use Recommended\" to reinstall and enable checks."
                                    })),
                                ));
                            };
                            let local_bytes = match std::fs::read(path) {
                                Ok(b) => b,
                                Err(e) => {
                                    return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                                        warp::reply::json(&serde_json::json!({
                                            "ok": false,
                                            "error": format!("Failed to read installed template: {e}")
                                        })),
                                    ));
                                }
                            };
                            (
                                fetch_url.clone(),
                                sha256_hex(&local_bytes),
                                None,
                                fallback_source_url.unwrap_or(fetch_url),
                            )
                        }
                    };

                let client = match reqwest::Client::builder()
                    .timeout(std::time::Duration::from_secs(15))
                    .user_agent("llama-monitor/1.0")
                    .build()
                {
                    Ok(c) => c,
                    Err(e) => {
                        return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                            warp::reply::json(&serde_json::json!({
                                "ok": false,
                                "error": format!("HTTP client error: {e}")
                            })),
                        ));
                    }
                };

                let mut req = client.get(&fetch_url);
                if fetch_url.contains("huggingface.co")
                    && let Some(ref tok) = crate::hf::hf_load_token()
                    && !tok.is_empty()
                {
                    req = req.header("Authorization", format!("Bearer {tok}"));
                }

                let new_sha = match req.send().await {
                    Ok(resp) if resp.status().is_success() => match resp.bytes().await {
                        Ok(bytes) => sha256_hex(&bytes),
                        Err(e) => {
                            return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                                warp::reply::json(&serde_json::json!({
                                    "ok": false,
                                    "error": format!("Failed to read response: {e}")
                                })),
                            ));
                        }
                    },
                    Ok(resp) => {
                        let status = resp.status().as_u16();
                        return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                            warp::reply::json(&serde_json::json!({
                                "ok": false,
                                "error": format!("HTTP {status} from upstream")
                            })),
                        ));
                    }
                    Err(e) => {
                        return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                            warp::reply::json(&serde_json::json!({
                                "ok": false,
                                "error": format!("Network error: {e}")
                            })),
                        ));
                    }
                };

                let changed = new_sha != baseline_sha;

                // Backfill meta.json for legacy installs once we have a confirmed baseline,
                // so subsequent checks no longer need the fallback fields from the client.
                // Approximate the original install date with the file's mtime, since the
                // true install time was never recorded.
                let mtime_rfc3339 = std::fs::metadata(path)
                    .and_then(|m| m.modified())
                    .ok()
                    .map(|t| chrono::DateTime::<chrono::Utc>::from(t).to_rfc3339());

                if existing_meta.is_none()
                    && let Ok(local_bytes) = std::fs::read(path)
                {
                    write_template_install_meta_at(
                        path,
                        &baseline_source_url,
                        &fetch_url,
                        &local_bytes,
                        mtime_rfc3339.clone(),
                        None,
                    );
                }

                let installed_at = baseline_installed_at
                    .or(mtime_rfc3339)
                    .unwrap_or_else(|| chrono::Utc::now().to_rfc3339());

                Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(warp::reply::json(
                    &serde_json::json!({
                        "ok": true,
                        "changed": changed,
                        "installed_at": installed_at,
                        "source_url": baseline_source_url,
                        "installed_sha256": baseline_sha,
                        "current_sha256": new_sha,
                        "backfilled": existing_meta.is_none()
                    }),
                )))
            }
        })
}

// Tool-call smoke-test fixtures (extracted from rapid-mlx-benchmark-suite.mjs tools suite)
const SMOKE_TEST_TOOLS: &str = r#"
[
  {"type":"function","function":{"name":"read_file","description":"Read a source file.","parameters":{"type":"object","properties":{"path":{"type":"string"}},"required":["path"],"additionalProperties":false}}},
  {"type":"function","function":{"name":"apply_patch","description":"Apply a small source edit.","parameters":{"type":"object","properties":{"path":{"type":"string"},"replacement":{"type":"string"}},"required":["path","replacement"],"additionalProperties":false}}},
  {"type":"function","function":{"name":"list_files","description":"List files in a directory.","parameters":{"type":"object","properties":{"path":{"type":"string"}},"required":["path"],"additionalProperties":false}}},
  {"type":"function","function":{"name":"search_code","description":"Search source files for a pattern.","parameters":{"type":"object","properties":{"pattern":{"type":"string"},"path":{"type":"string"}},"required":["pattern"],"additionalProperties":false}}},
  {"type":"function","function":{"name":"run_command","description":"Execute a shell command and return its output.","parameters":{"type":"object","properties":{"command":{"type":"string"}},"required":["command"],"additionalProperties":false}}}
]"#;

#[derive(serde::Serialize)]
struct SmokeTestResult {
    ok: bool,
    template_sha256: String,
    backend: String,
    model: String,
    tests: Vec<SmokeTestItem>,
    summary: String,
}

#[derive(serde::Serialize, Clone)]
struct SmokeTestItem {
    name: String,
    pass: bool,
    details: String,
}

fn chat_template_dir_path() -> Option<std::path::PathBuf> {
    dirs::home_dir().map(|h| {
        h.join(".config")
            .join("llama-monitor")
            .join("chat-templates")
    })
}

fn resolve_template_file_path(name: &str) -> Option<std::path::PathBuf> {
    let base = chat_template_dir_path()?;
    let path = base.join(format!("{name}.jinja"));
    if path.is_file() { Some(path) } else { None }
}

fn find_available_port() -> Option<u16> {
    std::net::TcpListener::bind("127.0.0.1:0")
        .ok()
        .and_then(|listener| listener.local_addr().ok().map(|a| a.port()))
}

async fn wait_for_server_ready(base_url: &str, timeout_secs: u64) -> bool {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()
        .unwrap_or_else(|_| reqwest::Client::new());
    let start = std::time::Instant::now();
    while start.elapsed() < std::time::Duration::from_secs(timeout_secs) {
        if client
            .get(format!("{}/health", base_url))
            .send()
            .await
            .ok()
            .is_some_and(|r| r.status().is_success() || r.status().as_u16() == 404)
        {
            return true;
        }
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    }
    false
}

async fn run_single_tool_call_test(base_url: &str) -> SmokeTestItem {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .unwrap_or_else(|_| reqwest::Client::new());

    let body = serde_json::json!({
        "model": "test",
        "messages": [
            {"role": "system", "content": "You are a coding assistant. Always use tools when appropriate."},
            {"role": "user", "content": "Use read_file on src/example.ts."}
        ],
        "tools": serde_json::from_str::<serde_json::Value>(SMOKE_TEST_TOOLS).unwrap_or_default(),
        "tool_choice": "required",
        "max_tokens": 512,
        "temperature": 0.0
    });

    match client
        .post(format!("{}/v1/chat/completions", base_url))
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await
    {
        Ok(resp) if resp.status().is_success() => match resp.json::<serde_json::Value>().await {
            Ok(json) => {
                let tool_calls = &json["choices"][0]["message"]["tool_calls"];
                if tool_calls.is_array() && !tool_calls.as_array().unwrap().is_empty() {
                    let tc = &tool_calls[0];
                    let func = &tc["function"];
                    let name = func["name"].as_str().unwrap_or("");
                    let args: serde_json::Value = func["arguments"].clone();
                    let path_arg = args
                        .as_object()
                        .and_then(|m| m.get("path"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("");

                    if name == "read_file" && path_arg == "src/example.ts" {
                        SmokeTestItem {
                            name: "single_tool_call".into(),
                            pass: true,
                            details: "Correctly emitted read_file(path='src/example.ts')".into(),
                        }
                    } else {
                        SmokeTestItem {
                            name: "single_tool_call".into(),
                            pass: false,
                            details: format!(
                                "Tool name or arguments incorrect: got name='{}' path='{}'",
                                name, path_arg
                            ),
                        }
                    }
                } else {
                    let content = json["choices"][0]["message"]["content"]
                        .as_str()
                        .unwrap_or("<no content>");
                    SmokeTestItem {
                        name: "single_tool_call".into(),
                        pass: false,
                        details: format!(
                            "Expected tool_call but got text response: {}",
                            content.chars().take(100).collect::<String>()
                        ),
                    }
                }
            }
            Err(e) => SmokeTestItem {
                name: "single_tool_call".into(),
                pass: false,
                details: format!("Failed to parse response JSON: {}", e),
            },
        },
        Ok(resp) => SmokeTestItem {
            name: "single_tool_call".into(),
            pass: false,
            details: format!("HTTP {} from chat completions", resp.status()),
        },
        Err(e) => SmokeTestItem {
            name: "single_tool_call".into(),
            pass: false,
            details: format!("Request failed: {}", e),
        },
    }
}

async fn run_sequential_tool_call_test(base_url: &str) -> SmokeTestItem {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .unwrap_or_else(|_| reqwest::Client::new());

    // First turn: use read_file
    let body1 = serde_json::json!({
        "model": "test",
        "messages": [
            {"role": "system", "content": "You are a coding assistant."},
            {"role": "user", "content": "Use read_file on src/example.ts. After seeing the file result, use apply_patch to replace the marked value."}
        ],
        "tools": serde_json::from_str::<serde_json::Value>(SMOKE_TEST_TOOLS).unwrap_or_default(),
        "tool_choice": "required",
        "max_tokens": 512,
        "temperature": 0.0
    });

    let first_resp = match client
        .post(format!("{}/v1/chat/completions", base_url))
        .header("Content-Type", "application/json")
        .json(&body1)
        .send()
        .await
    {
        Ok(r) if r.status().is_success() => r.json::<serde_json::Value>().await.ok(),
        _ => None,
    };

    let Some(first_json) = first_resp else {
        return SmokeTestItem {
            name: "sequential_tool_call".into(),
            pass: false,
            details: "First turn failed".into(),
        };
    };

    let tool_calls = &first_json["choices"][0]["message"]["tool_calls"];
    if !tool_calls.is_array() || tool_calls.as_array().unwrap().is_empty() {
        return SmokeTestItem {
            name: "sequential_tool_call".into(),
            pass: false,
            details: "First turn did not call read_file".into(),
        };
    }

    // Second turn: tool result → expect apply_patch
    let body2 = serde_json::json!({
        "model": "test",
        "messages": [
            {"role": "system", "content": "You are a coding assistant."},
            {"role": "user", "content": "Use read_file on src/example.ts. After seeing the file result, use apply_patch to replace the marked value."},
            {"role": "assistant", "tool_calls": tool_calls.clone()},
            {"role": "tool", "tool_call_id": tool_calls[0]["id"].as_str().unwrap_or("call_1"), "content": "export const MARKED_VALUE = \"before\";\n"}
        ],
        "tools": serde_json::from_str::<serde_json::Value>(SMOKE_TEST_TOOLS).unwrap_or_default(),
        "tool_choice": "required",
        "max_tokens": 512,
        "temperature": 0.0
    });

    match client
        .post(format!("{}/v1/chat/completions", base_url))
        .header("Content-Type", "application/json")
        .json(&body2)
        .send()
        .await
    {
        Ok(resp) if resp.status().is_success() => match resp.json::<serde_json::Value>().await {
            Ok(json) => {
                let tc = &json["choices"][0]["message"]["tool_calls"];
                if tc.is_array() && !tc.as_array().unwrap().is_empty() {
                    let func = &tc[0]["function"];
                    let name = func["name"].as_str().unwrap_or("");
                    if name == "apply_patch" {
                        SmokeTestItem {
                            name: "sequential_tool_call".into(),
                            pass: true,
                            details: "Correctly chained read_file → apply_patch".into(),
                        }
                    } else {
                        SmokeTestItem {
                            name: "sequential_tool_call".into(),
                            pass: false,
                            details: format!("Expected apply_patch but got tool name='{}'", name),
                        }
                    }
                } else {
                    SmokeTestItem {
                        name: "sequential_tool_call".into(),
                        pass: false,
                        details: "Second turn did not emit tool_call (loop/text response)".into(),
                    }
                }
            }
            Err(e) => SmokeTestItem {
                name: "sequential_tool_call".into(),
                pass: false,
                details: format!("Failed to parse second-turn response: {}", e),
            },
        },
        Ok(resp) => SmokeTestItem {
            name: "sequential_tool_call".into(),
            pass: false,
            details: format!("HTTP {} from second turn", resp.status()),
        },
        Err(e) => SmokeTestItem {
            name: "sequential_tool_call".into(),
            pass: false,
            details: format!("Second turn request failed: {}", e),
        },
    }
}

async fn spawn_rapid_mlx_server(
    bin_path: &std::path::Path,
    model_dir: &std::path::Path,
    template_path: &std::path::Path,
    port: u16,
    tool_call_parser: Option<String>,
) -> Result<std::process::Child, String> {
    use std::process::Command;

    // Create overlay with candidate template
    let overlay = crate::inference::rapid_mlx::model_resolver::create_template_overlay(
        model_dir.to_string_lossy().as_ref(),
        Some(template_path.to_string_lossy().as_ref()),
    )
    .map_err(|e| format!("Failed to create template overlay: {}", e))?;

    // Build minimal argv: just enough for a tool-call test
    let mut args: Vec<String> = vec![
        "serve".into(),
        overlay,
        "--port".into(),
        port.to_string(),
        "--host".into(),
        "127.0.0.1".into(),
        "--max-num-seqs".into(),
        "1".into(),
        "--max-concurrent-requests".into(),
        "1".into(),
        "--no-telemetry".into(),
        "--disable-prefix-cache".into(),
        "--log-level".into(),
        "WARNING".into(),
    ];

    if let Some(parser) = tool_call_parser {
        args.push("--tool-call-parser".into());
        args.push(parser);
        args.push("--enable-auto-tool-choice".into());
    }

    let mut cmd = Command::new(bin_path);
    cmd.args(&args);
    cmd.stdout(std::process::Stdio::null());
    cmd.stderr(std::process::Stdio::null());

    let child = cmd
        .spawn()
        .map_err(|e| format!("Failed to start Rapid-MLX server: {}", e))?;
    Ok(child)
}

async fn spawn_llama_cpp_server(
    bin_path: &std::path::Path,
    model_path: &std::path::Path,
    template_path: &std::path::Path,
    port: u16,
) -> Result<std::process::Child, String> {
    use std::process::Command;

    let mut cmd = Command::new(bin_path);
    cmd.arg("-m").arg(model_path);
    cmd.arg("--port").arg(port.to_string());
    cmd.arg("--host").arg("127.0.0.1");
    cmd.arg("-c").arg("4096");
    cmd.arg("-n").arg("-1");
    cmd.arg("--jinja");
    cmd.arg("--no-warmup");
    cmd.arg("--no-context-shift");
    cmd.arg("-ngl").arg("all");
    cmd.arg("--chat-template-file").arg(template_path);
    cmd.stdout(std::process::Stdio::null());
    cmd.stderr(std::process::Stdio::null());

    let child = cmd
        .spawn()
        .map_err(|e| format!("Failed to start llama.cpp server: {}", e))?;
    Ok(child)
}

fn kill_server(mut child: std::process::Child) {
    let _ = child.kill();
    let _ = child.wait();
}

// 11) POST /api/chat-template/smoke-test
fn api_chat_template_smoke_test(
    _state: AppState,
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (Box<dyn warp::reply::Reply>,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "chat-template" / "smoke-test")
        .and(warp::post())
        .and(warp::header::optional::<String>("authorization"))
        .and(super::super::safe_json_body::<serde_json::Value>())
        .and_then(move |auth: Option<String>, body: serde_json::Value| {
            let cfg = app_config.clone();
            async move {
                if !check_api_token(&auth, &cfg) {
                    return Ok(unauthorized_api_token());
                }

                let name = body["name"].as_str().unwrap_or("").to_string();
                let model = body["model"].as_str().unwrap_or("").to_string();
                let backend = body["backend"].as_str().unwrap_or("rapid-mlx").to_string();

                if name.is_empty() || model.is_empty() {
                    return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                        warp::reply::json(&serde_json::json!({
                            "ok": false,
                            "error": "Missing 'name' or 'model' field"
                        }))
                    ));
                }

                let template_path = match resolve_template_file_path(&name) {
                    Some(p) => p,
                    None => {
                        return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                            warp::reply::json(&serde_json::json!({
                                "ok": false,
                                "error": format!("Template '{}' not found in active install directory", name)
                            }))
                        ));
                    }
                };

                let template_sha256 = match std::fs::read(&template_path) {
                    Ok(content) => sha256_hex(&content),
                    Err(e) => {
                        return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                            warp::reply::json(&serde_json::json!({
                                "ok": false,
                                "error": format!("Failed to read template: {}", e)
                            }))
                        ));
                    }
                };

                let port = match find_available_port() {
                    Some(p) => p,
                    None => {
                        return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                            warp::reply::json(&serde_json::json!({
                                "ok": false,
                                "error": "Could not find available port for temporary server"
                            }))
                        ));
                    }
                };

                let base_url = format!("http://127.0.0.1:{}", port);

                // Resolve tool_call_parser hint from the model identifier
                let tool_call_parser = if model.to_lowercase().contains("qwen") {
                    Some("qwen".into())
                } else {
                    None
                };

                let server = match backend.as_str() {
                    "rapid-mlx" => {
                        let bin_path = match crate::inference::rapid_mlx::discovery::Discovery::resolve_binary(None, None).await {
                            Ok((p, _)) => p,
                            Err(e) => {
                                return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                                    warp::reply::json(&serde_json::json!({
                                        "ok": false,
                                        "error": format!("Rapid-MLX binary not found: {}", e)
                                    }))
                                ));
                            }
                        };

                        // Resolve model: try as local path first, then as HF repo
                        let model_dir = if std::path::Path::new(&model).exists() {
                            model.clone()
                        } else {
                            let hf_path = format!(
                                "{}/.cache/huggingface/hub/models--{}",
                                dirs::home_dir().unwrap_or_else(|| std::path::PathBuf::from("/tmp")).display(),
                                model.replace('/', "--")
                            );
                            hf_path
                        };

                        if !std::path::Path::new(&model_dir).exists() {
                            return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                                warp::reply::json(&serde_json::json!({
                                    "ok": false,
                                    "error": format!("Model directory not found for '{}'. Ensure the model is downloaded first.", model)
                                }))
                            ));
                        }

                        match spawn_rapid_mlx_server(&bin_path, std::path::Path::new(&model_dir), &template_path, port, tool_call_parser).await {
                            Ok(c) => c,
                            Err(e) => {
                                return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                                    warp::reply::json(&serde_json::json!({
                                        "ok": false,
                                        "error": e
                                    }))
                                ));
                            }
                        }
                    }
                    "llama-cpp" => {
                        let bin_path = std::path::Path::new(&cfg.llama_server_path);
                        if !bin_path.exists() {
                            return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                                warp::reply::json(&serde_json::json!({
                                    "ok": false,
                                    "error": "llama-server binary not found"
                                }))
                            ));
                        }

                        let model_path = if std::path::Path::new(&model).exists() {
                            model.clone()
                        } else {
                            return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                                warp::reply::json(&serde_json::json!({
                                    "ok": false,
                                    "error": format!("Model file not found for '{}'. Provide a local GGUF path.", model)
                                }))
                            ));
                        };

                        match spawn_llama_cpp_server(bin_path, std::path::Path::new(&model_path), &template_path, port).await {
                            Ok(c) => c,
                            Err(e) => {
                                return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                                    warp::reply::json(&serde_json::json!({
                                        "ok": false,
                                        "error": e
                                    }))
                                ));
                            }
                        }
                    }
                    _ => {
                        return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                            warp::reply::json(&serde_json::json!({
                                "ok": false,
                                "error": format!("Unsupported backend '{}'. Use 'rapid-mlx' or 'llama-cpp'.", backend)
                            }))
                        ));
                    }
                };

                // Wait for server to be ready
                if !wait_for_server_ready(&base_url, 45).await {
                    kill_server(server);
                    return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                        warp::reply::json(&serde_json::json!({
                            "ok": false,
                            "error": "Server did not become ready within 45 seconds"
                        }))
                    ));
                }

                // Run tests with overall timeout guard
                let test_start = std::time::Instant::now();
                let test_timeout = std::time::Duration::from_secs(60);

                let single_result = run_single_tool_call_test(&base_url).await;
                let sequential_result = if test_start.elapsed() < test_timeout {
                    run_sequential_tool_call_test(&base_url).await
                } else {
                    SmokeTestItem {
                        name: "sequential_tool_call".into(),
                        pass: false,
                        details: "Skipped: overall test timeout exceeded".into(),
                    }
                };

                kill_server(server);

                let all_pass = single_result.pass && sequential_result.pass;
                let tests = vec![single_result.clone(), sequential_result];
                let summary = if all_pass {
                    "PASS — template handles single and sequential tool calls correctly".into()
                } else {
                    let failures: Vec<_> = tests.iter()
                        .filter(|t| !t.pass)
                        .map(|t| t.details.clone())
                        .collect();
                    format!(
                        "FAIL — {} test(s) did not pass: {}",
                        failures.len(),
                        failures.join("; ")
                    )
                };

                Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(warp::reply::json(
                    &SmokeTestResult {
                        ok: all_pass,
                        template_sha256,
                        backend,
                        model,
                        tests,
                        summary,
                    }
                )))
            }
        })
}

pub(crate) fn routes(ctx: ApiCtx) -> ApiRoute {
    let state = ctx.state.clone();
    let config = ctx.config.clone();

    let mut r = api_spawn_wizard_mtp_draft_check(state.clone(), config.clone())
        .or(api_spawn_wizard_import_launch_file(
            state.clone(),
            config.clone(),
        ))
        .unify()
        .boxed();
    r = r
        .or(api_chat_template_fetch(state.clone(), config.clone()))
        .unify()
        .boxed();
    r = r
        .or(api_chat_template_upload(state.clone(), config.clone()))
        .unify()
        .boxed();
    r = r
        .or(api_chat_template_dir(state.clone(), config.clone()))
        .unify()
        .boxed();
    r = r
        .or(api_chat_template_install_hf(state.clone(), config.clone()))
        .unify()
        .boxed();
    r = r
        .or(api_chat_template_install_url(state.clone(), config.clone()))
        .unify()
        .boxed();
    r = r
        .or(api_chat_template_active(state.clone(), config.clone()))
        .unify()
        .boxed();
    r = r
        .or(api_chat_template_check_update(
            state.clone(),
            config.clone(),
        ))
        .unify()
        .boxed();
    r = r
        .or(api_chat_template_releases(state.clone(), config.clone()))
        .unify()
        .boxed();
    r = r
        .or(api_chat_template_activate(state.clone(), config.clone()))
        .unify()
        .boxed();
    r = r
        .or(api_chat_template_smoke_test(state.clone(), config.clone()))
        .unify()
        .boxed();
    r
}
