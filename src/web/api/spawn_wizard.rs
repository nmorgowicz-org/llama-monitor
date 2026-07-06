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

fn write_template_install_meta(
    dest: &std::path::Path,
    source_url: &str,
    fetch_url: &str,
    content: &[u8],
) {
    write_template_install_meta_at(dest, source_url, fetch_url, content, None);
}

/// Like `write_template_install_meta`, but allows backdating `installed_at` (e.g. to a
/// legacy install's file mtime when backfilling metadata that never existed).
fn write_template_install_meta_at(
    dest: &std::path::Path,
    source_url: &str,
    fetch_url: &str,
    content: &[u8],
    installed_at_override: Option<String>,
) {
    let meta = ChatTemplateInstallMeta {
        source_url: source_url.to_string(),
        fetch_url: fetch_url.to_string(),
        installed_at: installed_at_override.unwrap_or_else(|| chrono::Utc::now().to_rfc3339()),
        sha256: sha256_hex(content),
    };
    if let Ok(json) = serde_json::to_vec_pretty(&meta) {
        let _ = std::fs::write(template_meta_path(dest), json);
    }
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

                let url = format!("https://huggingface.co/{repo}/raw/main/{file}");
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

                write_template_install_meta(&dest, &source_url, &url, content.as_bytes());

                Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(warp::reply::json(
                    &serde_json::json!({
                        "ok": true,
                        "path": dest.to_string_lossy(),
                        "already_existed": false,
                        "source_url": source_url
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

                write_template_install_meta(&dest, &source, &source, &content);

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

// 8) POST /api/chat-template/check-update
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
        .or(api_chat_template_check_update(
            state.clone(),
            config.clone(),
        ))
        .unify()
        .boxed();
    r
}
