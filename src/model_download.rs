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
use std::io::IsTerminal;
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
    /// Absolute filesystem path where the file is being saved.
    pub local_path: String,
}

#[derive(Debug)]
struct DownloadTask {
    status: String,
    message: String,
    start_time: std::time::Instant,
    /// Absolute path on disk where the file is/will be written.
    local_path: std::path::PathBuf,
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

    /// Evict completed/failed/cancelled tasks older than 1 hour.
    /// Called on every new download start so the map doesn't grow unbounded.
    fn evict_stale(&mut self) {
        let cutoff = std::time::Instant::now() - std::time::Duration::from_secs(3600);
        self.tasks.retain(|_, task| {
            if let Ok(t) = task.lock() {
                let terminal = matches!(t.status.as_str(), "completed" | "failed" | "cancelled");
                !(terminal && t.start_time < cutoff)
            } else {
                false
            }
        });
    }
}

static MODEL_DOWNLOAD_MANAGER: std::sync::LazyLock<Mutex<DownloadManager>> =
    std::sync::LazyLock::new(|| Mutex::new(DownloadManager::new()));

/// Start an async HuggingFace download. Returns a download ID immediately.
/// `save_as` overrides the local filename (useful for renaming companion files like mmproj).
pub fn start_download(
    repo_id: &str,
    file_path: &str,
    save_as: Option<&str>,
    target_dir: &Path,
    hf_token: Option<String>,
) -> Result<String> {
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let rand_part: u32 = rand::random();
    let download_id = format!("md-{}-{:x}", ts, rand_part);

    let repo_id = repo_id.to_string();
    let file_path = file_path.to_string();
    let local_path = target_dir.join(save_as.unwrap_or(&file_path));

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
        local_path: local_path.clone(),
        bytes_downloaded: bytes_downloaded.clone(),
        total_bytes: total_bytes.clone(),
        cancel: cancel.clone(),
    }));

    {
        let mut mgr = MODEL_DOWNLOAD_MANAGER
            .lock()
            .expect("MODEL_DOWNLOAD_MANAGER lock poisoned");
        mgr.evict_stale();
        mgr.tasks.insert(download_id.clone(), task.clone());
    }

    eprintln!(
        "[info] download started  id={download_id}  repo={repo_id}  file={file_path}  dest={}",
        local_path.display()
    );

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

#[allow(clippy::too_many_arguments)]
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

    // Build the HTTP request — use connect_timeout only; no total timeout so large
    // files can stream for as long as needed without being killed mid-transfer.
    let client = reqwest::Client::builder()
        .connect_timeout(std::time::Duration::from_secs(30))
        .build()
        .unwrap_or_else(|_| reqwest::Client::new());
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
    let total_expected = if let Some(cl) = resp.content_length() {
        let total = resume_from.saturating_add(cl);
        total_atom.store(total, Ordering::Relaxed);
        total
    } else {
        eprintln!(
            "[warn] download: server did not send Content-Length for {repo_id}/{file_path}; progress bar will be indeterminate"
        );
        0
    };
    eprintln!(
        "[info] download stream open  file={file_path}  resume_from={resume_from}  total={total_expected}"
    );

    // Ensure parent directory exists.
    if let Some(parent) = local_path.parent()
        && let Err(e) = tokio::fs::create_dir_all(parent).await
    {
        set_failed(&task, format!("Failed to create directory: {e}"));
        return;
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
    let mut last_log_at = resume_from; // log every ~500 MB (non-TTY only)

    // TTY progress bar: track per-session transfer for accurate speed.
    let is_tty = std::io::stderr().is_terminal();
    let stream_start = std::time::Instant::now();
    let mut last_render: Option<std::time::Instant> = None; // None → render on first chunk

    // Pin the cancel future once so `notify_one()` is never lost between iterations.
    let cancel_signal = cancel.notified();
    tokio::pin!(cancel_signal);

    loop {
        let chunk = tokio::select! {
            biased;
            _ = &mut cancel_signal => {
                // cancel_download() already updated status/message; just stop.
                if is_tty { eprintln!(); } // leave progress bar line
                return;
            }
            chunk = stream.next() => chunk,
        };

        match chunk {
            Some(Ok(data)) => {
                if let Err(e) = file.write_all(&data).await {
                    if is_tty {
                        eprint!("\r\x1b[K");
                    }
                    eprintln!("[error] download write error  file={file_path}  error={e}");
                    set_failed(&task, format!("Write error: {e}"));
                    return;
                }
                written += data.len() as u64;
                bytes_atom.store(written, Ordering::Relaxed);

                if is_tty {
                    let should_render = last_render
                        .map(|t| t.elapsed() >= std::time::Duration::from_millis(200))
                        .unwrap_or(true);
                    if should_render {
                        let elapsed = stream_start.elapsed().as_secs_f64();
                        let transferred = written.saturating_sub(resume_from);
                        let speed = if elapsed > 0.1 {
                            transferred as f64 / elapsed
                        } else {
                            0.0
                        };
                        eprint!(
                            "\r\x1b[K{}",
                            render_progress(&file_path, written, total_expected, speed)
                        );
                        let _ = std::io::Write::flush(&mut std::io::stderr());
                        last_render = Some(std::time::Instant::now());
                    }
                } else if written.saturating_sub(last_log_at) >= 500 * 1024 * 1024 {
                    let pct = if total_expected > 0 {
                        format!(" ({:.0}%)", written as f64 / total_expected as f64 * 100.0)
                    } else {
                        String::new()
                    };
                    eprintln!(
                        "[info] download progress  file={file_path}  written={} MB / {} MB{pct}",
                        written / 1_048_576,
                        total_expected / 1_048_576
                    );
                    last_log_at = written;
                }
            }
            Some(Err(e)) => {
                if is_tty {
                    eprint!("\r\x1b[K");
                }
                eprintln!("[error] download stream error  file={file_path}  error={e}");
                set_failed(&task, format!("Stream error: {e}"));
                return;
            }
            None => break,
        }
    }

    let mut t = task.lock().expect("DownloadTask lock poisoned");
    t.status = "completed".into();
    t.message = "Download completed.".into();
    if is_tty {
        let elapsed = stream_start.elapsed().as_secs_f64();
        let transferred = written.saturating_sub(resume_from);
        let avg_speed = if elapsed > 0.0 {
            transferred as f64 / elapsed
        } else {
            0.0
        };
        let elapsed_str = if elapsed < 60.0 {
            format!("{:.0}s", elapsed)
        } else {
            format!("{:.1}m", elapsed / 60.0)
        };
        eprintln!(
            "\r\x1b[K[info] download completed  file={file_path}  {} MB  {elapsed_str}  avg {}",
            written / 1_048_576,
            fmt_speed(avg_speed),
        );
    } else {
        eprintln!("[info] download completed  file={file_path}  bytes={written}");
    }
}

fn set_failed(task: &Arc<Mutex<DownloadTask>>, msg: String) {
    let mut t = task.lock().expect("DownloadTask lock poisoned");
    // Don't overwrite a cancel that raced us here.
    if t.status != "cancelled" {
        // Clear any in-progress progress bar line before printing the error.
        if std::io::stderr().is_terminal() {
            eprint!("\r\x1b[K");
        }
        eprintln!(
            "[error] download failed  path={}  reason={msg}",
            t.local_path.display()
        );
        t.status = "failed".into();
        t.message = msg;
    }

    // Rename partial file to .part so it is not mistaken for a complete download.
    let path = t.local_path.clone();
    if path.exists() {
        let part_path = path.with_extension("part");
        eprintln!(
            "[warn] renaming partial download  from={}  to={}",
            path.display(),
            part_path.display()
        );
        let _ = std::fs::rename(&path, &part_path);
    }
}

pub fn get_download_status(download_id: &str) -> Option<DownloadStatus> {
    let mgr = MODEL_DOWNLOAD_MANAGER
        .lock()
        .expect("MODEL_DOWNLOAD_MANAGER lock poisoned");
    let task = mgr.tasks.get(download_id)?;
    let t = task.lock().expect("DownloadTask lock poisoned");

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
        local_path: t.local_path.to_string_lossy().into_owned(),
    })
}

fn fmt_speed(bps: f64) -> String {
    if bps >= 1_048_576.0 {
        format!("{:.1} MB/s", bps / 1_048_576.0)
    } else if bps >= 1024.0 {
        format!("{:.0} KB/s", bps / 1024.0)
    } else if bps > 0.0 {
        format!("{:.0} B/s", bps)
    } else {
        "--".to_string()
    }
}

/// Build the one-line TTY progress bar string (no newline, no ANSI prefix).
fn render_progress(file_path: &str, written: u64, total: u64, bps: f64) -> String {
    // Show only the filename component, truncated to 30 chars from the right.
    let fname = file_path.rsplit('/').next().unwrap_or(file_path);
    let fname: String = if fname.len() > 30 {
        format!("...{}", &fname[fname.len() - 27..])
    } else {
        fname.to_string()
    };

    let mb = written / 1_048_576;
    let pct = if total > 0 {
        format!("{:.1}%", written as f64 / total as f64 * 100.0)
    } else {
        "?%".to_string()
    };
    let total_mb = if total > 0 {
        format!("/{}", total / 1_048_576)
    } else {
        String::new()
    };

    const BAR_W: usize = 22;
    let filled = if total > 0 {
        ((written as f64 / total as f64 * BAR_W as f64) as usize).min(BAR_W)
    } else {
        0
    };
    let bar = format!("[{}{}]", "=".repeat(filled), " ".repeat(BAR_W - filled));

    let eta_str = if bps > 0.0 && total > written {
        let secs = (total - written) as f64 / bps;
        if secs < 60.0 {
            format!("  ETA {:.0}s", secs)
        } else if secs < 3600.0 {
            format!("  ETA {:.0}m", secs / 60.0)
        } else {
            format!("  ETA {:.1}h", secs / 3600.0)
        }
    } else {
        String::new()
    };

    format!(
        "{fname} {bar} {pct}  {mb}{total_mb} MB  {}{eta_str}",
        fmt_speed(bps)
    )
}

/// Mark a download as cancelled and signal the streaming loop to stop.
/// Returns false if the download is not found or already finished.
pub fn cancel_download(download_id: &str) -> bool {
    let mgr = MODEL_DOWNLOAD_MANAGER
        .lock()
        .expect("MODEL_DOWNLOAD_MANAGER lock poisoned");
    if let Some(task) = mgr.tasks.get(download_id) {
        let mut t = task.lock().expect("DownloadTask lock poisoned");
        if t.status == "running" {
            t.status = "cancelled".into();
            t.message = "Cancelled by user.".into();
            t.cancel.notify_one();
            return true;
        }
    }
    false
}
