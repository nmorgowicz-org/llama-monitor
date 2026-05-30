//! Model download manager.
//!
//! Provides:
//! - start_download(repo_id, file_path, target_dir)
//! - get_download_status(id)
//! - cancel_download(id)
//!
//! Uses in-memory state protected by a Mutex for simplicity.

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use std::sync::Mutex;

use anyhow::Result;
use serde::Serialize;
use tokio::sync::Notify;

use crate::hf;

#[derive(Debug, Serialize)]
pub struct DownloadStatus {
    pub download_id: String,
    pub status: String, // "running", "completed", "failed", "cancelled"
    pub bytes_downloaded: u64,
    pub total_bytes: u64,
    pub speed: f64, // bytes/sec
    pub eta: u64,   // seconds
    pub message: String,
}

#[derive(Debug)]
struct DownloadTask {
    status: String,
    bytes_downloaded: u64,
    total_bytes: u64,
    start_time: std::time::Instant,
    message: String,
    cancel: Arc<tokio::sync::Notify>,
}

struct DownloadManager {
    tasks: HashMap<String, Arc<Mutex<DownloadTask>>>,
}

impl DownloadManager {
    fn new() -> Self {
        Self {
            tasks: HashMap::new(),
        }
    }
}

static MODEL_DOWNLOAD_MANAGER: std::sync::LazyLock<Mutex<DownloadManager>> =
    std::sync::LazyLock::new(|| Mutex::new(DownloadManager::new()));

pub fn start_download(
    repo_id: &str,
    file_path: &str,
    target_dir: &Path,
    hf_token: Option<String>,
) -> Result<String> {
    let download_id = format!(
        "md-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis()
    );

    let repo_id = repo_id.to_string();
    let file_path = file_path.to_string();
    let local_path = target_dir.join(&file_path);

    // Ensure target directory exists.
    if let Err(e) = std::fs::create_dir_all(target_dir) {
        anyhow::bail!("Failed to create model directory: {}", e);
    }

    let task = Arc::new(Mutex::new(DownloadTask {
        status: "running".into(),
        bytes_downloaded: 0,
        total_bytes: 0,
        start_time: std::time::Instant::now(),
        message: "Starting download...".into(),
        cancel: Arc::new(Notify::new()),
    }));

    {
        let mut mgr = MODEL_DOWNLOAD_MANAGER.lock().unwrap();
        mgr.tasks.insert(download_id.clone(), task.clone());
    }

    tokio::spawn(async move {
        let token = hf_token.as_deref();
        let result = hf::hf_download_file_stream(&repo_id, &file_path, token, &local_path, 0).await;

        let mut t = task.lock().unwrap();
        match result {
            Ok(bytes) => {
                t.bytes_downloaded = bytes;
                t.status = "completed".into();
                t.message = "Download completed.".into();
            }
            Err(e) => {
                t.status = "failed".into();
                t.message = format!("Download failed: {}", e);
            }
        }
        drop(t);
    });

    Ok(download_id)
}

pub fn get_download_status(download_id: &str) -> Option<DownloadStatus> {
    let mgr = MODEL_DOWNLOAD_MANAGER.lock().unwrap();
    let task = mgr.tasks.get(download_id)?;
    let t = task.lock().unwrap();

    let elapsed = t.start_time.elapsed();
    let speed = if elapsed.as_secs_f64() > 0.0 {
        t.bytes_downloaded as f64 / elapsed.as_secs_f64()
    } else {
        0.0
    };
    let remaining = t.total_bytes.saturating_sub(t.bytes_downloaded);
    let eta = if speed > 0.0 {
        (remaining as f64 / speed) as u64
    } else {
        0
    };

    Some(DownloadStatus {
        download_id: download_id.to_string(),
        status: t.status.clone(),
        bytes_downloaded: t.bytes_downloaded,
        total_bytes: t.total_bytes,
        speed,
        eta,
        message: t.message.clone(),
    })
}

pub fn cancel_download(download_id: &str) -> bool {
    let mgr = MODEL_DOWNLOAD_MANAGER.lock().unwrap();
    if let Some(task) = mgr.tasks.get(download_id) {
        let mut t = task.lock().unwrap();
        t.status = "cancelled".into();
        t.message = "Cancelled by user.".into();
        t.cancel.notify_one();
        return true;
    }
    false
}
