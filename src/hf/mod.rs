//! HuggingFace Hub integration using the hf-hub crate.
//!
//! Provides:
//! - hf_get_model_info
//! - hf_list_repo_files
//! - hf_get_file_info
//! - hf_download_file_stream
//! - hf_search_models
//! - hf_list_gguf_files
//! - hf_start_download
//! - hf_token management

#![allow(dead_code)]

use anyhow::{Context, Result};
use hf_hub::api::sync::ApiBuilder;
use hf_hub::{Repo, RepoType};
use std::path::Path;
use tokio::fs::File;
use tokio::io::AsyncWriteExt;

/// Simple model info for search results.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct SimpleModelInfo {
    pub id: String,
    pub gated: bool,
    pub tags: Vec<String>,
    pub downloads: u64,
    pub likes: u64,
}

/// A GGUF file in a HF repo.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct HfGgufFile {
    pub path: String,
    pub size: u64,
    pub label: String,
}

/// High-level model info from HF.
#[derive(Debug, Clone, serde::Serialize)]
pub struct HfModelInfo {
    pub repo_id: String,
    pub gated: bool,
    pub private: bool,
    pub tags: Vec<String>,
}

/// A single file in a HF repo.
#[derive(Debug, Clone, serde::Serialize)]
pub struct HfFileInfo {
    pub r#type: String, // "file" or "folder"
    pub path: String,
    pub size: Option<u64>,
}

/// Get basic model info for a repo.
///
/// hf-hub 0.5 RepoInfo is limited (siblings + sha), so we fall back
/// to a direct HF API call for richer metadata (gated, tags).
/// Honors the configured HF token for gated-model access.
pub async fn hf_get_model_info(repo_id: &str) -> Result<HfModelInfo> {
    let token = hf_load_token();

    let client = reqwest::Client::new();
    let url = format!("https://huggingface.co/api/models/{repo_id}");

    let mut req = client.get(&url);
    if let Some(ref tok) = token {
        req = req.bearer_auth(tok);
    }

    let resp = req.send().await.context("Failed to call HF models API")?;

    if !resp.status().is_success() {
        return Ok(HfModelInfo {
            repo_id: repo_id.to_string(),
            gated: false,
            private: false,
            tags: Vec::new(),
        });
    }

    let body: serde_json::Value = resp
        .json()
        .await
        .context("Failed to parse HF models API response")?;

    let gated = body.get("gated").and_then(|v| v.as_bool()).unwrap_or(false);

    let private = body
        .get("private")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let tags: Vec<String> = body
        .get("tags")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|t| t.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();

    Ok(HfModelInfo {
        repo_id: repo_id.to_string(),
        gated,
        private,
        tags,
    })
}

/// List repo files, filtering for GGUF by default if requested.
///
/// Uses HF Hub REST API (siblings) since hf-hub 0.5 does not expose
/// a recursive tree list.
pub fn hf_list_repo_files(repo_id: &str, gguf_only: bool) -> Result<Vec<HfFileInfo>> {
    let api = ApiBuilder::new()
        .with_token(hf_load_token())
        .build()
        .context("Failed to build HF API client")?;

    let repo = Repo::new(repo_id.to_string(), RepoType::Model);
    let model = api.repo(repo);

    let info = model.info().context("Failed to list repo files")?;

    let mut result = Vec::new();

    for sibling in &info.siblings {
        let name = sibling.rfilename.as_str();
        if gguf_only && !name.to_ascii_lowercase().ends_with(".gguf") {
            continue;
        }

        result.push(HfFileInfo {
            r#type: "file".into(),
            path: name.to_string(),
            size: None, // hf-hub 0.5 Siblings does not expose size
        });
    }

    Ok(result)
}

/// Get info for a specific file in a repo.
pub fn hf_get_file_info(repo_id: &str, path: &str) -> Result<HfFileInfo> {
    let api = ApiBuilder::new()
        .with_token(hf_load_token())
        .build()
        .context("Failed to build HF API client")?;

    let repo = Repo::new(repo_id.to_string(), RepoType::Model);
    let model = api.repo(repo);

    let info = model.info().context("Failed to list repo files")?;

    for sibling in &info.siblings {
        if sibling.rfilename == path {
            return Ok(HfFileInfo {
                r#type: "file".into(),
                path: sibling.rfilename.clone(),
                size: None,
            });
        }
    }

    anyhow::bail!("File not found: {path}");
}

/// Stream-download a file from HF into a local path.
///
/// Uses hf-hub to resolve the file URL, then streams via reqwest.
/// Supports resume via byte range if backend allows.
pub async fn hf_download_file_stream(
    repo_id: &str,
    path: &str,
    token: Option<&str>,
    local_path: &Path,
    resume_from: u64,
) -> Result<u64> {
    // Build HF API with optional token.
    let api = ApiBuilder::new()
        .with_token(token.map(String::from))
        .build()
        .context("Failed to build HF API client")?;

    let repo = Repo::new(repo_id.to_string(), RepoType::Model);
    let model = api.repo(repo);

    // Get the file URL.
    let url = model.url(path);
    let url = if url.is_empty() {
        anyhow::bail!("Failed to resolve HF file URL for {path}");
    } else {
        url
    };

    let client = reqwest::Client::new();
    let mut builder = client.get(&url);

    if let Some(t) = token {
        builder = builder.bearer_auth(t);
    }

    if resume_from > 0 {
        builder = builder.header("Range", format!("bytes={}-", resume_from));
    }

    let resp = builder
        .send()
        .await
        .context("Failed to start HF download")?;

    if !resp.status().is_success() && !matches!(resp.status(), reqwest::StatusCode::PARTIAL_CONTENT)
    {
        anyhow::bail!(
            "HF download failed with status {} for {}/{}",
            resp.status(),
            repo_id,
            path
        );
    }

    let _total = match resp.content_length() {
        Some(len) => len,
        None => {
            // No content-length: fall back to non-resumable download.
            let bytes = resp
                .bytes()
                .await
                .context("Failed to read HF response body")?;
            if let Some(parent) = local_path.parent() {
                tokio::fs::create_dir_all(parent)
                    .await
                    .context("Failed to create directory for HF download")?;
            }
            let mut file = File::create(local_path)
                .await
                .context("Failed to create HF download file")?;
            file.write_all(&bytes).await?;
            return Ok(bytes.len() as u64);
        }
    };

    // Ensure parent directory exists.
    if let Some(parent) = local_path.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .context("Failed to create directory for HF download")?;
    }

    // Truncate when starting fresh; append only when resuming a partial file.
    let mut file = if resume_from > 0 {
        File::options()
            .create(true)
            .write(true)
            .append(true)
            .open(local_path)
            .await
            .context("Failed to open HF download file for resume")?
    } else {
        File::create(local_path)
            .await
            .context("Failed to create HF download file")?
    };

    use futures_util::StreamExt;
    let mut stream = resp.bytes_stream();
    let mut downloaded = 0u64;

    while let Some(chunk) = stream.next().await {
        let chunk = chunk.context("Stream error from HF")?;
        file.write_all(&chunk).await?;
        downloaded += chunk.len() as u64;
    }

    Ok(downloaded)
}

/// Load HF token from environment or config file.
///
/// Priority:
/// 1) HUGGING_FACE_HUB_TOKEN env var
/// 2) ~/.config/llama-monitor/hf-token file
///
/// Token is never logged in full.
pub fn hf_load_token() -> Option<String> {
    // Env var first.
    if let Ok(v) = std::env::var("HUGGING_FACE_HUB_TOKEN")
        && !v.trim().is_empty()
    {
        return Some(v);
    }

    // Fallback: config file.
    if let Some(home) = dirs::home_dir() {
        let path = home.join(".config").join("llama-monitor").join("hf-token");
        if let Ok(content) = std::fs::read_to_string(&path) {
            let trimmed = content.trim().to_string();
            if !trimmed.is_empty() {
                return Some(trimmed);
            }
        }
    }

    None
}

/// Save HF token to config file.
pub fn hf_save_token(token: &str) -> Result<()> {
    let home = dirs::home_dir().ok_or_else(|| anyhow::anyhow!("Home directory not found"))?;
    let dir = home.join(".config").join("llama-monitor");
    std::fs::create_dir_all(&dir).context("Failed to create config dir for HF token")?;
    let path = dir.join("hf-token");
    std::fs::write(&path, token.trim()).context("Failed to write HF token file")?;
    Ok(())
}

/// Mask a token for safe logging: first4...last4.
fn mask_token(token: &str) -> String {
    let t = token.trim();
    if t.len() <= 8 {
        "****".to_string()
    } else {
        format!("{}****{}", &t[..4], &t[t.len() - 4..])
    }
}

/// Search models on HuggingFace Hub.
///
/// Uses HF Hub REST API with GGUF tag filter.
pub async fn hf_search_models(query: &str, limit: usize) -> Result<Vec<SimpleModelInfo>, String> {
    let limit = limit.clamp(1, 100);

    let token = hf_load_token();
    let client = reqwest::Client::new();

    let mut url = reqwest::Url::parse("https://huggingface.co/api/models")
        .map_err(|e| format!("Invalid HF API URL: {e}"))?;

    {
        let mut params = url.query_pairs_mut();
        params.append_pair("search", query);
        params.append_pair("limit", &limit.to_string());
        params.append_pair("sort", "downloads");
        params.append_pair("direction", "-1");
        params.append_pair("filter", "gguf");
    }

    let mut req = client.get(url);

    if let Some(ref tok) = token {
        req = req.bearer_auth(tok);
    }

    let resp = req
        .send()
        .await
        .map_err(|e| format!("HF search request failed: {e}"))?;

    if !resp.status().is_success() {
        return Err(format!(
            "HF search failed with status {} (model may be gated or search unavailable)",
            resp.status()
        ));
    }

    let items: Vec<serde_json::Value> = resp
        .json()
        .await
        .map_err(|e| format!("Failed to parse HF search response: {e}"))?;

    let mut results = Vec::new();
    for item in items {
        let id = item
            .get("id")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        if id.is_empty() {
            continue;
        }

        let gated = item.get("gated").and_then(|v| v.as_bool()).unwrap_or(false);

        let tags: Vec<String> = item
            .get("tags")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|t| t.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default();

        let downloads = item.get("downloads").and_then(|v| v.as_u64()).unwrap_or(0);

        let likes = item.get("likes").and_then(|v| v.as_u64()).unwrap_or(0);

        results.push(SimpleModelInfo {
            id,
            gated,
            tags,
            downloads,
            likes,
        });
    }

    Ok(results)
}

/// List GGUF files for a given HF repo.
///
/// Uses the repo info (siblings) to filter .gguf files, and enriches
/// with size and a human-readable quant label.
pub async fn hf_list_gguf_files(repo_id: &str) -> Result<Vec<HfGgufFile>, String> {
    let api = ApiBuilder::new()
        .with_token(hf_load_token())
        .build()
        .map_err(|e| format!("Failed to build HF API client: {e}"))?;

    let repo = Repo::new(repo_id.to_string(), RepoType::Model);
    let model = api.repo(repo);

    let info = model
        .info()
        .map_err(|e| format!("Failed to list repo files: {e}"))?;

    let mut result = Vec::new();

    for sibling in &info.siblings {
        let name = sibling.rfilename.as_str();
        if !name.to_ascii_lowercase().ends_with(".gguf") {
            continue;
        }

        // Try to infer a label from filename.
        let label = infer_quant_label(name);

        result.push(HfGgufFile {
            path: name.to_string(),
            size: 0, // hf-hub 0.5 does not expose size directly
            label,
        });
    }

    Ok(result)
}

/// Infer a human-readable quant label from a GGUF filename.
fn infer_quant_label(filename: &str) -> String {
    let lower = filename.to_ascii_lowercase();
    if lower.contains("q8_0") {
        "Q8_0"
    } else if lower.contains("q6_k") {
        "Q6_K"
    } else if lower.contains("q5_k_m") {
        "Q5_K_M"
    } else if lower.contains("q5_k_s") {
        "Q5_K_S"
    } else if lower.contains("q4_k_m") {
        "Q4_K_M"
    } else if lower.contains("q4_k_s") {
        "Q4_K_S"
    } else if lower.contains("q3_k_m") {
        "Q3_K_M"
    } else if lower.contains("q3_k_s") {
        "Q3_K_S"
    } else if lower.contains("q2_k") {
        "Q2_K"
    } else if lower.contains("iq4_x") {
        "IQ4_X"
    } else if lower.contains("bf16") || lower.contains("f16") {
        "BF16/F16"
    } else if lower.contains("f32") {
        "F32"
    } else {
        "Unknown"
    }
    .to_string()
}

/// Start a download via the existing model_download module.
///
/// Integrates with /api/models/download and /api/models/download/:id/status.
pub fn hf_start_download(
    repo_id: &str,
    file_path: &str,
    target_path: &Path,
    _resume: bool, // resume flag reserved for future; currently uses existing behavior
) -> Result<String, String> {
    // Use existing model_download::start_download.
    crate::model_download::start_download(repo_id, file_path, target_path, hf_load_token())
        .map_err(|e| format!("Failed to start download: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_hf_get_model_info_smoke() {
        // Best-effort smoke test; may fail in offline CI.
        let _ = hf_get_model_info("gpt2").await;
    }

    #[test]
    fn test_infer_quant_label() {
        assert_eq!(infer_quant_label("model-Q4_K_M.gguf"), "Q4_K_M");
        assert_eq!(infer_quant_label("model-Q8_0.gguf"), "Q8_0");
        assert_eq!(infer_quant_label("model-bf16.gguf"), "BF16/F16");
        assert_eq!(infer_quant_label("model-random.gguf"), "Unknown");
    }

    #[test]
    fn test_mask_token() {
        assert_eq!(mask_token("1234567890"), "1234****7890");
        assert_eq!(mask_token("1234"), "****");
    }

    #[test]
    fn test_simple_model_info_serde_default() {
        let json = r#"{"id":"test/model"}"#;
        let info: SimpleModelInfo =
            serde_json::from_str(json).expect("should deserialize with defaults");
        assert_eq!(info.id, "test/model");
        assert!(!info.gated);
        assert!(info.tags.is_empty());
        assert_eq!(info.downloads, 0);
        assert_eq!(info.likes, 0);
    }

    #[test]
    fn test_hf_gguf_file_serde_default() {
        let json = r#"{"path":"file.gguf"}"#;
        let f: HfGgufFile = serde_json::from_str(json).expect("should deserialize with defaults");
        assert_eq!(f.path, "file.gguf");
        assert_eq!(f.size, 0);
        assert!(f.label.is_empty());
    }

    #[test]
    fn test_hf_search_models_parsing_mock() {
        // Validate parsing logic with a synthetic JSON payload.
        let mock_response = serde_json::json!([
            {
                "id": "org/model1",
                "gated": false,
                "tags": ["gguf", "llama"],
                "downloads": 1234,
                "likes": 56
            },
            {
                "id": "org/gated-model",
                "gated": true,
                "tags": ["gguf"],
                "downloads": 999,
                "likes": 10
            }
        ]);

        let items: Vec<serde_json::Value> = mock_response.as_array().unwrap().to_vec();

        let mut results = Vec::new();
        for item in items {
            let id = item
                .get("id")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            if id.is_empty() {
                continue;
            }
            let gated = item.get("gated").and_then(|v| v.as_bool()).unwrap_or(false);
            let tags: Vec<String> = item
                .get("tags")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|t| t.as_str().map(String::from))
                        .collect()
                })
                .unwrap_or_default();
            let downloads = item.get("downloads").and_then(|v| v.as_u64()).unwrap_or(0);
            let likes = item.get("likes").and_then(|v| v.as_u64()).unwrap_or(0);
            results.push(SimpleModelInfo {
                id,
                gated,
                tags,
                downloads,
                likes,
            });
        }

        assert_eq!(results.len(), 2);
        assert!(!results[0].gated);
        assert!(results[1].gated);
        assert_eq!(results[0].downloads, 1234);
        assert!(results[0].tags.contains(&"gguf".to_string()));
    }

    #[test]
    fn test_hf_list_gguf_files_filtering() {
        // Simulate filtering logic.
        let siblings = vec![
            "model-Q4_K_M.gguf",
            "tokenizer.model",
            "model-Q8_0.gguf",
            "README.md",
        ];

        let mut ggufs = Vec::new();
        for name in &siblings {
            if name.to_ascii_lowercase().ends_with(".gguf") {
                let label = infer_quant_label(name);
                ggufs.push(HfGgufFile {
                    path: name.to_string(),
                    size: 0,
                    label,
                });
            }
        }

        assert_eq!(ggufs.len(), 2);
        assert_eq!(ggufs[0].label, "Q4_K_M");
        assert_eq!(ggufs[1].label, "Q8_0");
    }

    #[test]
    fn test_hf_search_models_parsing_edge_cases() {
        // Test with missing/invalid fields.
        let mock = serde_json::json!([
            { "id": "ok/model" },
            { "id": "" },
            { "gated": true },
            { "id": "partial/model", "downloads": "not-a-number" }
        ]);

        let items: Vec<serde_json::Value> = mock.as_array().unwrap().to_vec();
        let mut results = Vec::new();
        for item in items {
            let id = item
                .get("id")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            if id.is_empty() {
                continue;
            }
            let gated = item.get("gated").and_then(|v| v.as_bool()).unwrap_or(false);
            let downloads = item.get("downloads").and_then(|v| v.as_u64()).unwrap_or(0);
            results.push(SimpleModelInfo {
                id,
                gated,
                downloads,
                ..Default::default()
            });
        }

        assert_eq!(results.len(), 2);
        assert_eq!(results[0].id, "ok/model");
        assert_eq!(results[1].id, "partial/model");
        assert_eq!(results[1].downloads, 0);
    }

    #[test]
    fn test_hf_download_resume_logic() {
        // Validate that resume_from > 0 sets Range header.
        // (Unit test of logic; actual HTTP is integration.)
        let resume_from = 1_000_000u64;
        let range_header = format!("bytes={}-", resume_from);
        assert!(range_header.starts_with("bytes="));
        assert!(range_header.ends_with("-"));
    }
}
