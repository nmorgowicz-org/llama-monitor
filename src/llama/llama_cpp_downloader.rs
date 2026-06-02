use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tokio::fs;
use tokio::io::AsyncWriteExt;

fn backend_matches(name: &str, backend: &str) -> bool {
    match backend {
        "cpu" => name.contains("cpu") || name.contains("base") || name.contains("avx2"),
        "avx2" => name.contains("avx2") || name.contains("cpu") || name.contains("base"),
        "cuda" => name.contains("cuda"),
        "cuda12" => {
            name.contains("cuda12")
                || name.contains("cuda-12")
                || name.contains("cu12")
                || name.contains("cuda_12")
        }
        "cuda13" => {
            name.contains("cuda13")
                || name.contains("cuda-13")
                || name.contains("cu13")
                || name.contains("cuda_13")
        }
        "sycl" => name.contains("sycl") || name.contains("oneapi"),
        "vulkan" => name.contains("vulkan"),
        "rocm" | "hip" => name.contains("rocm") || name.contains("hip"),
        "metal" => name.contains("metal") || name.contains("mac"),
        _ => false,
    }
}

/// Metadata for a llama.cpp GitHub release.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlamaCppRelease {
    pub tag_name: String,
    pub assets: Vec<LlamaCppAsset>,
    #[serde(default)]
    pub published_at: String,
    #[serde(default)]
    pub body: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlamaCppAsset {
    pub name: String,
    pub browser_download_url: String,
}

/// Status of an in-progress llama.cpp download.
#[allow(dead_code)]
#[derive(Debug, Clone, Serialize)]
pub struct LlamaCppDownloadStatus {
    pub download_id: String,
    pub status: String,
    pub message: String,
    pub progress: f64,
}

/// List recent releases from ggerganov/llama.cpp.
pub async fn list_releases(client: &Client) -> Result<Vec<LlamaCppRelease>> {
    let url = "https://api.github.com/repos/ggerganov/llama.cpp/releases?per_page=20";
    let resp = client
        .get(url)
        .send()
        .await
        .context("Failed to fetch llama.cpp releases")?;

    if !resp.status().is_success() {
        anyhow::bail!("GitHub API returned {} for releases", resp.status());
    }

    let releases: Vec<LlamaCppRelease> =
        resp.json().await.context("Failed to parse releases JSON")?;
    Ok(releases)
}

/// Select appropriate assets for the given platform/backend.
///
/// This is a best-effort heuristic based on known release asset naming conventions.
pub fn select_assets<'a>(
    release: &'a LlamaCppRelease,
    backend: &str,
    arch: &str,
) -> Vec<&'a LlamaCppAsset> {
    let mut selected = Vec::new();

    let backend_lower = backend.to_lowercase();
    let arch_lower = arch.to_lowercase();

    for asset in &release.assets {
        let name = asset.name.to_lowercase();

        // Skip non-binary assets.
        if name.ends_with(".json") || name.ends_with(".md") || name.ends_with(".txt") {
            continue;
        }

        // Match by backend and architecture.
        let backend_match = backend_matches(&name, backend_lower.as_str());

        let arch_match = match arch_lower.as_str() {
            "x64" | "x86_64" => {
                name.contains("x64") || name.contains("x86_64") || name.contains("amd64")
            }
            "arm64" | "aarch64" => name.contains("arm64") || name.contains("aarch64"),
            _ => true,
        };

        if backend_match && arch_match {
            selected.push(asset);
        }
    }

    selected
}

/// Download and extract llama.cpp assets into binaries_dir.
///
/// For simplicity, this downloads each selected asset into binaries_dir
/// and attempts to extract .zip/.tar.gz if applicable.
pub async fn download_and_extract(
    client: &Client,
    _release: &LlamaCppRelease,
    assets: &[&LlamaCppAsset],
    binaries_dir: &Path,
) -> Result<()> {
    fs::create_dir_all(binaries_dir)
        .await
        .context("Failed to create binaries_dir")?;

    let mut progress = HashMap::new();
    for asset in assets {
        let out_path = binaries_dir.join(&asset.name);
        let tmp_path = out_path.with_extension("part");

        // Download
        let resp = client
            .get(&asset.browser_download_url)
            .send()
            .await
            .context(format!("Failed to start download for {}", asset.name))?;

        if !resp.status().is_success() {
            anyhow::bail!("Download failed for {}: HTTP {}", asset.name, resp.status());
        }

        let total = resp.content_length().unwrap_or(0);
        let mut downloaded: u64 = 0;
        let mut file = fs::File::create(&tmp_path)
            .await
            .context("Failed to create temp file")?;

        let mut stream = resp.bytes_stream();
        use futures_util::StreamExt;
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.context("Stream error")?;
            file.write_all(&chunk)
                .await
                .context("Failed to write chunk")?;
            downloaded += chunk.len() as u64;
            if total > 0 {
                progress.insert(asset.name.clone(), (downloaded as f64) / (total as f64));
            }
        }

        // Move temp -> final
        fs::rename(&tmp_path, &out_path)
            .await
            .context("Failed to finalize download file")?;

        // Extract if archive
        if asset.name.ends_with(".zip")
            || asset.name.ends_with(".tar.gz")
            || asset.name.ends_with(".tgz")
        {
            let _ = extract_archive(&out_path, binaries_dir)
                .await
                .inspect_err(|e| {
                    eprintln!("[warn] Failed to extract {}: {}", asset.name, e);
                });
        }
    }

    Ok(())
}

async fn extract_archive(path: &Path, dest: &Path) -> Result<()> {
    if path.extension().is_some_and(|e| e == "zip") {
        use std::io::Read;
        use zip::ZipArchive;
        // Use std::fs for zip crate (sync, blocking on read).
        let file = std::fs::File::open(path)?;
        let mut archive = ZipArchive::new(std::io::BufReader::new(file))?;
        for i in 0..archive.len() {
            let mut file = archive.by_index(i)?;
            let outpath = dest.join(file.enclosed_name().unwrap_or_else(|| PathBuf::from("out")));
            if file.is_file() {
                fs::create_dir_all(outpath.parent().unwrap_or(dest))
                    .await
                    .ok();
                let mut out_file = fs::File::create(&outpath).await?;
                let mut buf = Vec::new();
                file.read_to_end(&mut buf)?;
                out_file.write_all(&buf).await?;
            }
        }
    } else {
        // For tar.gz/tgz, read bytes then unpack synchronously.
        let bytes = tokio::fs::read(path).await?;
        let tar_gz = std::io::Cursor::new(bytes);
        let decompressed = flate2::read::GzDecoder::new(tar_gz);
        let mut archive = tar::Archive::new(decompressed);
        archive.unpack(dest)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_select_assets_cpu_x64() {
        let release = LlamaCppRelease {
            tag_name: "b6000".into(),
            assets: vec![
                LlamaCppAsset {
                    name: "llama-server-win-x64-cpu.exe".into(),
                    browser_download_url: "https://example.com/cpu".into(),
                },
                LlamaCppAsset {
                    name: "llama-server-win-x64-cuda12.zip".into(),
                    browser_download_url: "https://example.com/cuda".into(),
                },
            ],
            published_at: String::new(),
            body: String::new(),
        };
        let selected = select_assets(&release, "cpu", "x64");
        assert_eq!(selected.len(), 1);
    }

    #[test]
    fn test_select_assets_cuda_x64() {
        let release = LlamaCppRelease {
            tag_name: "b6000".into(),
            assets: vec![
                LlamaCppAsset {
                    name: "llama-server-win-x64-cpu.exe".into(),
                    browser_download_url: "https://example.com/cpu".into(),
                },
                LlamaCppAsset {
                    name: "llama-server-win-x64-cuda12.zip".into(),
                    browser_download_url: "https://example.com/cuda".into(),
                },
            ],
            published_at: String::new(),
            body: String::new(),
        };
        let selected = select_assets(&release, "cuda", "x64");
        assert_eq!(selected.len(), 1);
    }

    #[test]
    fn test_select_assets_avx2_does_not_fall_back_to_everything() {
        let release = LlamaCppRelease {
            tag_name: "b7000".into(),
            assets: vec![
                LlamaCppAsset {
                    name: "llama-server-win-x64-avx2.zip".into(),
                    browser_download_url: "https://example.com/avx2".into(),
                },
                LlamaCppAsset {
                    name: "llama-server-win-x64-cuda12.zip".into(),
                    browser_download_url: "https://example.com/cuda12".into(),
                },
                LlamaCppAsset {
                    name: "llama-server-win-x64-sycl.zip".into(),
                    browser_download_url: "https://example.com/sycl".into(),
                },
            ],
            published_at: String::new(),
            body: String::new(),
        };
        let selected = select_assets(&release, "avx2", "x64");
        assert_eq!(selected.len(), 1);
        assert_eq!(selected[0].name, "llama-server-win-x64-avx2.zip");
    }

    #[test]
    fn test_select_assets_cuda12_matches_only_cuda12() {
        let release = LlamaCppRelease {
            tag_name: "b7000".into(),
            assets: vec![
                LlamaCppAsset {
                    name: "llama-server-win-x64-cuda12.zip".into(),
                    browser_download_url: "https://example.com/cuda12".into(),
                },
                LlamaCppAsset {
                    name: "llama-server-win-x64-cuda13.zip".into(),
                    browser_download_url: "https://example.com/cuda13".into(),
                },
            ],
            published_at: String::new(),
            body: String::new(),
        };
        let selected = select_assets(&release, "cuda12", "x64");
        assert_eq!(selected.len(), 1);
        assert_eq!(selected[0].name, "llama-server-win-x64-cuda12.zip");
    }

    #[test]
    fn test_select_assets_sycl_matches_only_sycl() {
        let release = LlamaCppRelease {
            tag_name: "b7000".into(),
            assets: vec![
                LlamaCppAsset {
                    name: "llama-server-win-x64-sycl.zip".into(),
                    browser_download_url: "https://example.com/sycl".into(),
                },
                LlamaCppAsset {
                    name: "llama-server-win-x64-vulkan.zip".into(),
                    browser_download_url: "https://example.com/vulkan".into(),
                },
            ],
            published_at: String::new(),
            body: String::new(),
        };
        let selected = select_assets(&release, "sycl", "x64");
        assert_eq!(selected.len(), 1);
        assert_eq!(selected[0].name, "llama-server-win-x64-sycl.zip");
    }
}
