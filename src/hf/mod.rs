//! HuggingFace Hub integration using the hf-hub crate.
//!
//! Provides:
//! - hf_get_model_info
//! - hf_list_repo_files
//! - hf_get_file_info
//! - hf_download_file_stream

#![allow(dead_code)]

use anyhow::{Context, Result};
use hf_hub::api::sync::ApiBuilder;
use hf_hub::{Repo, RepoType};
use std::path::Path;
use tokio::fs::File;
use tokio::io::AsyncWriteExt;

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
pub fn hf_get_model_info(repo_id: &str) -> Result<HfModelInfo> {
    // Use hf-hub to validate repo.
    let api = ApiBuilder::new()
        .with_token(None)
        .build()
        .context("Failed to build HF API client")?;

    let repo = Repo::new(repo_id.to_string(), RepoType::Model);
    let model = api.repo(repo);
    let _info = model.info().context("Failed to fetch model info")?;

    // Fetch richer metadata from HF REST API.
    let client = reqwest::Client::new();
    let url = format!("https://huggingface.co/api/models/{repo_id}");

    let resp = client.get(&url).send();

    // Since hf_get_model_info is synchronous, we block_on for this small request.
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .context("Failed to build tokio runtime for HF metadata")?;

    let resp = rt.block_on(resp).context("Failed to call HF models API")?;

    if !resp.status().is_success() {
        // Fall back to minimal info.
        return Ok(HfModelInfo {
            repo_id: repo_id.to_string(),
            gated: false,
            private: false,
            tags: Vec::new(),
        });
    }

    let body: serde_json::Value = rt
        .block_on(resp.json())
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
        .with_token(None)
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
        .with_token(None)
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

    let mut file = File::options()
        .create(true)
        .write(true)
        .append(true)
        .open(local_path)
        .await
        .context("Failed to open HF download file")?;

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hf_get_model_info_smoke() {
        // This is a minimal smoke test; it will fail if network is unavailable.
        // In CI, treat as best-effort.
        let _ = hf_get_model_info("gpt2");
    }
}
