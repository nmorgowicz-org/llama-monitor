//! Model download manager.
//!
//! Provides:
//! - start_download(repo_id, file_path, target_dir, hf_token)
//! - get_download_status(id)
//! - cancel_download(id)
//!
//! Downloads stream in a spawned tokio task. Cancellation is immediate via
//! a shared Notify; progress is tracked via Arc<AtomicU64> so the status
//! endpoint reads live bytes without holding the task mutex.

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};

use anyhow::Result;
use futures_util::StreamExt;
use hf_hub::api::sync::ApiBuilder;
use hf_hub::{Repo, RepoType};
use serde::Serialize;
use tokio::fs::File;
use tokio::io::AsyncWriteExt;
use tokio::sync::Notify;

#[derive(Debug, Serialize)]
pub struct DownloadStatus {
    pub download_id: String,
    pub status: String, // "running" | "completed" | "failed" | "cancelled"
    pub bytes_downloaded: u64,
    pub total_bytes: u64,
    pub speed: f64, // bytes/sec
    pub eta: u64,   // seconds remaining
    pub message: String,
}

#[derive(Debug)]
struct DownloadTask {
    status: String,
    message: String,
    start_time: std::time::Instant,
    /// Live bytes written; updated by the streaming loop without holding the Mutex.
    bytes_downloaded: Arc<AtomicU64>,
    /// Total expected bytes from Content-Length; set once response headers arrive.
    total_bytes: Arc<AtomicU64>,
    /// Signal the streaming loop to abort.
    cancel: Arc<Notify>,
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

/// Start an async HuggingFace download. Returns a download ID immediately.
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

    if let Err(e) = std::fs::create_dir_all(target_dir) {
        anyhow::bail!("Failed to create model directory: {}", e);
    }

    let bytes_downloaded = Arc::new(AtomicU64::new(0));
    let total_bytes = Arc::new(AtomicU64::new(0));
    let cancel = Arc::new(Notify::new());

    let task = Arc::new(Mutex::new(DownloadTask {
        status: "running".into(),
        message: "Starting download...".into(),
        start_time: std::time::Instant::now(),
        bytes_downloaded: bytes_downloaded.clone(),
        total_bytes: total_bytes.clone(),
        cancel: cancel.clone(),
    }));

    {
        let mut mgr = MODEL_DOWNLOAD_MANAGER.lock().unwrap();
        mgr.tasks.insert(download_id.clone(), task.clone());
    }

    tokio::spawn(async move {
        run_download(
            task,
            repo_id,
            file_path,
            local_path,
            hf_token,
            bytes_downloaded,
            total_bytes,
            cancel,
        )
        .await;
    });

    Ok(download_id)
}

async fn run_download(
    task: Arc<Mutex<DownloadTask>>,
    repo_id: String,
    file_path: String,
    local_path: std::path::PathBuf,
    hf_token: Option<String>,
    bytes_atom: Arc<AtomicU64>,
    total_atom: Arc<AtomicU64>,
    cancel: Arc<Notify>,
) {
    // Resolve the HF download URL via hf-hub.
    let api = match ApiBuilder::new().with_token(hf_token.clone()).build() {
        Ok(a) => a,
        Err(e) => {
            set_failed(&task, format!("Failed to build HF API client: {e}"));
            return;
        }
    };

    let url = api
        .repo(Repo::new(repo_id.clone(), RepoType::Model))
        .url(&file_path);
    if url.is_empty() {
        set_failed(
            &task,
            format!("Could not resolve download URL for {repo_id}/{file_path}"),
        );
        return;
    }

    // Determine resume offset from any existing partial file.
    let resume_from = local_path.metadata().ok().map(|m| m.len()).unwrap_or(0);

    // Build the HTTP request.
    let client = reqwest::Client::new();
    let mut req = client.get(&url);
    if let Some(ref tok) = hf_token {
        req = req.bearer_auth(tok);
    }
    if resume_from > 0 {
        req = req.header("Range", format!("bytes={}-", resume_from));
    }

    let resp = match req.send().await {
        Ok(r) => r,
        Err(e) => {
            set_failed(&task, format!("Request failed: {e}"));
            return;
        }
    };

    let status_code = resp.status();
    if !status_code.is_success() && status_code != reqwest::StatusCode::PARTIAL_CONTENT {
        set_failed(&task, format!("HTTP {status_code} from HuggingFace"));
        return;
    }

    // Populate total_bytes from Content-Length so the status endpoint can compute ETA.
    if let Some(cl) = resp.content_length() {
        total_atom.store(resume_from.saturating_add(cl), Ordering::Relaxed);
    }

    // Ensure parent directory exists.
    if let Some(parent) = local_path.parent() {
        if let Err(e) = tokio::fs::create_dir_all(parent).await {
            set_failed(&task, format!("Failed to create directory: {e}"));
            return;
        }
    }

    // Open file: truncate for a fresh download, append only when resuming.
    let mut file = {
        let open_result = if resume_from > 0 {
            File::options()
                .create(true)
                .write(true)
                .append(true)
                .open(&local_path)
                .await
        } else {
            File::create(&local_path).await
        };
        match open_result {
            Ok(f) => f,
            Err(e) => {
                set_failed(&task, format!("Failed to open file: {e}"));
                return;
            }
        }
    };

    // Stream response body, selecting on the cancel signal for immediate abort.
    let mut stream = resp.bytes_stream();
    let mut written = resume_from;

    // Pin the cancel future once so `notify_one()` is never lost between iterations.
    let cancel_signal = cancel.notified();
    tokio::pin!(cancel_signal);

    loop {
        let chunk = tokio::select! {
            biased;
            _ = &mut cancel_signal => {
                // cancel_download() already updated status/message; just stop.
                return;
            }
            chunk = stream.next() => chunk,
        };

        match chunk {
            Some(Ok(data)) => {
                if let Err(e) = file.write_all(&data).await {
                    set_failed(&task, format!("Write error: {e}"));
                    return;
                }
                written += data.len() as u64;
                bytes_atom.store(written, Ordering::Relaxed);
            }
            Some(Err(e)) => {
                set_failed(&task, format!("Stream error: {e}"));
                return;
            }
            None => break,
        }
    }

    let mut t = task.lock().unwrap();
    t.status = "completed".into();
    t.message = "Download completed.".into();
}

fn set_failed(task: &Arc<Mutex<DownloadTask>>, msg: String) {
    let mut t = task.lock().unwrap();
    // Don't overwrite a cancel that raced us here.
    if t.status != "cancelled" {
        t.status = "failed".into();
        t.message = msg;
    }
}

pub fn get_download_status(download_id: &str) -> Option<DownloadStatus> {
    let mgr = MODEL_DOWNLOAD_MANAGER.lock().unwrap();
    let task = mgr.tasks.get(download_id)?;
    let t = task.lock().unwrap();

    let bytes = t.bytes_downloaded.load(Ordering::Relaxed);
    let total = t.total_bytes.load(Ordering::Relaxed);
    let elapsed = t.start_time.elapsed().as_secs_f64();

    let speed = if elapsed > 0.0 {
        bytes as f64 / elapsed
    } else {
        0.0
    };
    let remaining = total.saturating_sub(bytes);
    let eta = if speed > 0.0 {
        (remaining as f64 / speed) as u64
    } else {
        0
    };

    Some(DownloadStatus {
        download_id: download_id.to_string(),
        status: t.status.clone(),
        bytes_downloaded: bytes,
        total_bytes: total,
        speed,
        eta,
        message: t.message.clone(),
    })
}

/// Mark a download as cancelled and signal the streaming loop to stop.
/// Returns false if the download is not found or already finished.
pub fn cancel_download(download_id: &str) -> bool {
    let mgr = MODEL_DOWNLOAD_MANAGER.lock().unwrap();
    if let Some(task) = mgr.tasks.get(download_id) {
        let mut t = task.lock().unwrap();
        if t.status == "running" {
            t.status = "cancelled".into();
            t.message = "Cancelled by user.".into();
            t.cancel.notify_one();
            return true;
        }
    }
    false
}
