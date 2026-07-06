//! Model download manager.
//!
//! Provides:
//! - start_download(repo_id, file_path, target_dir, hf_token)
//! - get_download_status(id)
//! - cancel_download(id)
//!
//! Features:
//! - Duplicate guard: prevents starting a second task for the same (repo, file).
//! - Existing-file guard: refuses to re-download if the target file already exists.
//! - Concurrency limit: max 2 simultaneous running downloads.
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

use futures_util::StreamExt;
use hf_hub::api::sync::ApiBuilder;
use hf_hub::{Repo, RepoType};
use serde::Serialize;
use tokio::fs::File;
use tokio::io::AsyncWriteExt;
use tokio::sync::Notify;

/// Max number of concurrent running downloads.
const MAX_CONCURRENT_DOWNLOADS: usize = 2;

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

/// Specific reasons why `start_download` refused to begin.
/// Returned via the `"error"` field in JSON; intentionally plain strings
/// so the frontend can do simple substring checks.
pub enum DownloadStartError {
    AlreadyDownloading(String), // same (repo, file) in progress
    AlreadyExists(String),      // target file already on disk
    TooManyDownloads,           // concurrency limit reached
    Generic(String),
}

impl std::fmt::Display for DownloadStartError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DownloadStartError::AlreadyDownloading(name) => {
                write!(
                    f,
                    "Already downloading: {name}. Please wait until it completes."
                )
            }
            DownloadStartError::AlreadyExists(path) => {
                write!(
                    f,
                    "File already exists at: {path}. It may be available in your library."
                )
            }
            DownloadStartError::TooManyDownloads => {
                write!(
                    f,
                    "Too many downloads in progress. Please wait for one to finish."
                )
            }
            DownloadStartError::Generic(msg) => write!(f, "{msg}"),
        }
    }
}

impl From<DownloadStartError> for String {
    fn from(e: DownloadStartError) -> String {
        e.to_string()
    }
}

#[derive(Debug)]
struct DownloadTask {
    status: String,
    message: String,
    start_time: std::time::Instant,
    last_accessed: std::time::Instant,
    /// Absolute path on disk where the file is/will be written.
    local_path: std::path::PathBuf,
    /// Source repo and file path (for duplicate detection).
    repo_id: String,
    file_path: String,
    /// Live bytes written; updated by the streaming loop without holding the Mutex.
    bytes_downloaded: Arc<AtomicU64>,
    /// Total expected bytes from Content-Length; set once response headers arrive.
    total_bytes: Arc<AtomicU64>,
    /// Bytes already on disk before this download session started (for resume).
    resume_from: u64,
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
    /// Also keep tasks that have been accessed (status read) within the last 5 minutes
    /// to avoid race with frontend polling.
    fn evict_stale(&mut self) {
        let now = std::time::Instant::now();
        let stale_cutoff = now - std::time::Duration::from_secs(3600);
        let active_cutoff = now - std::time::Duration::from_secs(300); // 5 minutes
        self.tasks.retain(|_, task| {
            if let Ok(t) = task.lock() {
                let terminal = matches!(t.status.as_str(), "completed" | "failed" | "cancelled");
                let recently_accessed = t.last_accessed >= active_cutoff;
                if terminal && !recently_accessed && t.start_time < stale_cutoff {
                    return false; // evict
                }
                true
            } else {
                false
            }
        });
    }
}

static MODEL_DOWNLOAD_MANAGER: std::sync::LazyLock<Mutex<DownloadManager>> =
    std::sync::LazyLock::new(|| Mutex::new(DownloadManager::new()));

/// Track when the last download failed so the HF download endpoint can
/// skip its cooldown and allow immediate retry.
pub(crate) static MODEL_DOWNLOAD_LAST_FAILED: std::sync::LazyLock<std::sync::atomic::AtomicU64> =
    std::sync::LazyLock::new(|| std::sync::atomic::AtomicU64::new(0));

/// Start an async HuggingFace download. Returns a download ID immediately.
/// `save_as` overrides the local filename (useful for renaming companion files like mmproj).
pub fn start_download(
    repo_id: &str,
    file_path: &str,
    save_as: Option<&str>,
    target_dir: &Path,
    hf_token: Option<String>,
) -> std::result::Result<String, DownloadStartError> {
    let repo_id_owned = repo_id.to_string();
    let file_path_owned = file_path.to_string();
    let local_path = target_dir.join(save_as.unwrap_or(&file_path_owned));

    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let rand_part: u32 = rand::random();
    let download_id = format!("md-{}-{:x}", ts, rand_part);

    // Evict stale tasks and run pre-flight checks.
    {
        let mut mgr = match MODEL_DOWNLOAD_MANAGER.lock() {
            Ok(g) => g,
            Err(poisoned) => {
                eprintln!("[error] start_download: manager lock poisoned; recovering");
                poisoned.into_inner()
            }
        };
        mgr.evict_stale();

        // Duplicate guard: reject if a running task exists for same (repo, file).
        for t in mgr.tasks.values() {
            let guard = match t.lock() {
                Ok(g) => g,
                Err(_) => continue,
            };
            if guard.status != "running" {
                continue;
            }
            if guard.repo_id == repo_id_owned && guard.file_path == file_path_owned {
                return Err(DownloadStartError::AlreadyDownloading(display_file_name(
                    &file_path_owned,
                )));
            }
        }

        // Existing-file guard: if the target already exists (complete, not partial),
        // assume the user already has this download.
        if local_path.exists() && local_path.extension().is_none_or(|ext| ext != "part") {
            return Err(DownloadStartError::AlreadyExists(
                local_path.to_string_lossy().into_owned(),
            ));
        }

        // Concurrency limit.
        let running = mgr
            .tasks
            .values()
            .filter_map(|t| t.lock().ok())
            .filter(|t| t.status == "running")
            .count();
        if running >= MAX_CONCURRENT_DOWNLOADS {
            return Err(DownloadStartError::TooManyDownloads);
        }
    }

    if let Err(e) = std::fs::create_dir_all(target_dir) {
        return Err(DownloadStartError::Generic(format!(
            "Failed to create model directory: {e}"
        )));
    }

    // Restore .part to real name so run_download can resume from existing file.
    if !local_path.exists() {
        let part_path = local_path.with_extension("part");
        if part_path.exists() {
            let _ = std::fs::rename(&part_path, &local_path);
        }
    }

    // Capture resume_from before creating the task (used later by get_download_status).
    let resume_from = local_path.metadata().ok().map(|m| m.len()).unwrap_or(0);

    // Initialize bytes_downloaded to existing file size so progress bar reflects
    // previously downloaded data instead of jumping from zero.
    let bytes_downloaded = Arc::new(AtomicU64::new(resume_from));
    let total_bytes = Arc::new(AtomicU64::new(0));
    let cancel = Arc::new(Notify::new());

    let task = Arc::new(Mutex::new(DownloadTask {
        status: "running".into(),
        message: "Starting download...".into(),
        start_time: std::time::Instant::now(),
        last_accessed: std::time::Instant::now(),
        local_path: local_path.clone(),
        repo_id: repo_id_owned.clone(),
        file_path: file_path_owned.clone(),
        bytes_downloaded: bytes_downloaded.clone(),
        total_bytes: total_bytes.clone(),
        resume_from,
        cancel: cancel.clone(),
    }));

    {
        let mut mgr = match MODEL_DOWNLOAD_MANAGER.lock() {
            Ok(g) => g,
            Err(poisoned) => {
                eprintln!("[error] start_download: manager lock poisoned on insert; recovering");
                poisoned.into_inner()
            }
        };
        mgr.tasks.insert(download_id.clone(), task.clone());
    }

    eprintln!(
        "[info] download started  id={download_id}  repo={repo_id_owned}  file={file_path_owned}  dest={}",
        local_path.display()
    );

    tokio::spawn(async move {
        run_download(
            task,
            repo_id_owned,
            file_path_owned,
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

fn display_file_name(file_path: &str) -> String {
    file_path
        .rsplit('/')
        .next()
        .unwrap_or(file_path)
        .to_string()
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
    // Read resume_from from the task (pre-computed by start_download).
    let resume_from = match task.lock() {
        Ok(guard) => guard.resume_from,
        Err(poisoned) => {
            eprintln!(
                "[error] download: task lock poisoned on resume_from read for {}",
                file_path
            );
            let guard = poisoned.into_inner();
            guard.resume_from
        }
    };

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

    // Build the HTTP request.
    let client = reqwest::Client::builder()
        .connect_timeout(std::time::Duration::from_secs(30))
        .tcp_keepalive(std::time::Duration::from_secs(60))
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
    let content_length = resp.content_length();

    // When resuming, perform a basic sanity check to avoid corrupting the file
    // if the server-side content changed or our partial is invalid.
    if resume_from > 0 {
        // Ensure partial file actually exists and matches our expected size.
        match local_path.metadata() {
            Ok(meta) if meta.len() == resume_from => { /* OK */ }
            Ok(meta) => {
                eprintln!(
                    "[warn] download: partial size mismatch (have={}, expected={}); restarting",
                    meta.len(),
                    resume_from
                );
                let _ = tokio::fs::remove_file(&local_path).await;
                // Restart fresh (resume_from becomes 0).
                std::mem::drop(bytes_atom.clone());
                bytes_atom.store(0, Ordering::Relaxed);
            }
            Err(e) => {
                eprintln!(
                    "[warn] download: cannot read partial metadata: {}; restarting",
                    e
                );
                bytes_atom.store(0, Ordering::Relaxed);
            }
        }
    }

    // Recompute resume_from if we restarted above.
    let resume_from = bytes_atom.load(Ordering::Relaxed);

    let total_expected = if let Some(cl) = content_length {
        // If we are resuming but server reports no remaining bytes, restart fresh.
        if resume_from > 0 && cl == 0 {
            eprintln!(
                "[warn] download: server reports 0 remaining bytes while resuming; restarting"
            );
            let _ = tokio::fs::remove_file(&local_path).await;
            bytes_atom.store(0, Ordering::Relaxed);
            // We'll re-request without Range after adjusting.
        }
        let final_resume = bytes_atom.load(Ordering::Relaxed);
        let total = final_resume.saturating_add(cl);
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

    // Idle timeout: kill the download if no data received for 120 seconds.
    let mut idle_timeout_after = std::time::Instant::now();

    // Pin the cancel future once so `notify_one()` is never lost between iterations.
    let cancel_signal = cancel.notified();
    tokio::pin!(cancel_signal);

    loop {
        // Idle-timeout future: fires once after 120s of no data.
        let idle_deadline = idle_timeout_after
            .checked_add(std::time::Duration::from_secs(120))
            .unwrap_or_else(|| std::time::Instant::now() + std::time::Duration::from_secs(120));
        let idle_sleep_dur = idle_deadline
            .saturating_duration_since(std::time::Instant::now());
        let idle_future = tokio::time::sleep(idle_sleep_dur);

        let chunk = tokio::select! {
            biased;
            _ = &mut cancel_signal => {
                // cancel_download() already updated status/message; just stop.
                if is_tty { eprintln!(); }
                // Rename any partial file so it is not mistaken for complete.
                if local_path.exists() {
                    let part_path = local_path.with_extension("part");
                    let _ = std::fs::rename(&local_path, &part_path);
                }
                return;
            }
            _ = idle_future => {
                if is_tty { eprint!("\r\x1b[K"); }
                set_failed(&task, "Idle timeout: no data for 120 seconds. You can retry to resume.".into());
                return;
            }
            chunk = stream.next() => chunk,
        };

        match chunk {
            Some(Ok(data)) => {
                idle_timeout_after = std::time::Instant::now();

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
                let msg = classify_stream_error(&e);
                if is_tty {
                    eprint!("\r\x1b[K");
                }
                eprintln!("[error] download stream error  file={file_path}  error={msg}");
                set_failed(&task, msg);
                return;
            }
            None => break,
        }
    }

    let mut t = match task.lock() {
        Ok(g) => g,
        Err(poisoned) => {
            eprintln!("[error] run_download: lock poisoned on completion; recovering");
            poisoned.into_inner()
        }
    };
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

fn is_transient_error(msg: &str) -> bool {
    let s = msg.to_ascii_lowercase();
    s.contains("connection dropped")
        || s.contains("timed out")
        || s.contains("idle timeout")
        || s.contains("connection reset")
        || s.contains("eof while reading")
        || s.contains("broken pipe")
        || s.contains("write error")
}

fn set_failed(task: &Arc<Mutex<DownloadTask>>, msg: String) {
    use std::sync::atomic::Ordering;
    let now = std::time::SystemTime::UNIX_EPOCH
        .elapsed()
        .unwrap_or_default()
        .as_secs();
    MODEL_DOWNLOAD_LAST_FAILED.store(now, Ordering::Relaxed);

    let is_transient = is_transient_error(&msg);

    let mut t = match task.lock() {
        Ok(g) => g,
        Err(poisoned) => {
            eprintln!("[error] set_failed: lock poisoned; recovering");
            poisoned.into_inner()
        }
    };
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

    let path = t.local_path.clone();
    if !path.exists() {
        return;
    }

    if is_transient {
        // Rename to .part so it can be resumed later.
        let part_path = path.with_extension("part");
        eprintln!(
            "[warn] renaming partial download  from={}  to={}",
            path.display(),
            part_path.display()
        );
        let _ = std::fs::rename(&path, &part_path);
    } else {
        // Non-retryable: remove partial to avoid stale garbage.
        eprintln!(
            "[info] removing partial download (non-retryable)  path={}",
            path.display()
        );
        let _ = std::fs::remove_file(&path);
    }
}

/// Give a clearer, actionable message for stream errors.
fn classify_stream_error(err: &reqwest::Error) -> String {
    let s = err.to_string().to_ascii_lowercase();

    // Known transient / load-related issues
    if s.contains("error decoding response body")
        || s.contains("connection reset")
        || s.contains("eof while reading")
        || s.contains("broken pipe")
    {
        return "Connection dropped. You can retry to resume from where it left off.".into();
    }

    // Timeout or too many retries
    if s.contains("timed out") || s.contains("timeout") {
        return "Request timed out. You can retry to resume.".into();
    }

    // Auth / gating
    if s.contains("401") || s.contains("403") {
        return "Access denied (check HF token or model access).".into();
    }

    // Not found
    if s.contains("404") {
        return "File not found on HuggingFace.".into();
    }

    // Generic fallback (still useful to know it can be retried)
    format!("Download error: {err}. You may be able to retry to resume.")
}

pub fn get_download_status(download_id: &str) -> Option<DownloadStatus> {
    let mgr = match MODEL_DOWNLOAD_MANAGER.lock() {
        Ok(g) => g,
        Err(_poisoned) => {
            eprintln!("[error] get_download_status: manager lock poisoned; recovering");
            return Some(DownloadStatus {
                download_id: download_id.to_string(),
                status: "failed".into(),
                bytes_downloaded: 0,
                total_bytes: 0,
                speed: 0.0,
                eta: 0,
                message: "Internal error: lock poisoned".into(),
                local_path: String::new(),
            });
        }
    };
    let task = mgr.tasks.get(download_id)?;
    let mut t = match task.lock() {
        Ok(g) => g,
        Err(poisoned) => {
            eprintln!("[error] get_download_status: task lock poisoned for {download_id}");
            let inner = poisoned.into_inner();
            return Some(DownloadStatus {
                download_id: download_id.to_string(),
                status: "failed".into(),
                bytes_downloaded: 0,
                total_bytes: 0,
                speed: 0.0,
                eta: 0,
                message: format!("Internal error: lock poisoned (was {})", inner.message),
                local_path: inner.local_path.to_string_lossy().into_owned(),
            });
        }
    };
    // Update last_accessed so evict_stale does not remove tasks being actively polled.
    t.last_accessed = std::time::Instant::now();

    let bytes = t.bytes_downloaded.load(Ordering::Relaxed);
    let total = t.total_bytes.load(Ordering::Relaxed);
    let elapsed = t.start_time.elapsed().as_secs_f64();

    // Use only bytes transferred since session start for speed, not total bytes.
    let bytes_transferred = bytes.saturating_sub(t.resume_from) as f64;
    let speed = if elapsed > 0.1 {
        bytes_transferred / elapsed
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
    let mgr = match MODEL_DOWNLOAD_MANAGER.lock() {
        Ok(g) => g,
        Err(_poisoned) => {
            eprintln!("[error] cancel_download: manager lock poisoned");
            return false;
        }
    };
    if let Some(task) = mgr.tasks.get(download_id) {
        let mut t = match task.lock() {
            Ok(g) => g,
            Err(_poisoned) => {
                eprintln!("[error] cancel_download: task lock poisoned for {download_id}");
                return false;
            }
        };
        if t.status == "running" {
            t.status = "cancelled".into();
            t.message = "Cancelled by user.".into();
            t.cancel.notify_one();
            return true;
        }
    }
    false
}
