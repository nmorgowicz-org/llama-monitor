use std::collections::HashMap;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use once_cell::sync::Lazy;
use sha2::{Digest, Sha256};
use warp::Filter;
use warp::http::StatusCode;
use warp::reject::Reject;

static HF_REPO_RE: Lazy<regex::Regex> =
    Lazy::new(|| regex::Regex::new(r"^[a-zA-Z0-9_-]+/[a-zA-Z0-9._-]+$").unwrap());

fn validate_hf_repo_id(repo_id: &str) -> bool {
    HF_REPO_RE.is_match(repo_id)
}

fn get_effective_models_dir(state: &AppState) -> Option<PathBuf> {
    // Prefer explicit --models-dir or user-configured models_dir
    if let Some(ref d) = state.models_dir {
        return Some(d.clone());
    }
    let s = state.ui_settings.lock().unwrap();
    if !s.models_dir.is_empty() {
        return Some(PathBuf::from(&s.models_dir));
    }
    None
}

fn resolve_hf_target_dir(models_dir: &Path, target_path: Option<&str>) -> Result<PathBuf, String> {
    std::fs::create_dir_all(models_dir).map_err(|e| {
        format!(
            "Failed to create models_dir {}: {}",
            models_dir.display(),
            e
        )
    })?;
    let models_dir_canon = models_dir.canonicalize().map_err(|e| {
        format!(
            "Failed to resolve models_dir {}: {}",
            models_dir.display(),
            e
        )
    })?;

    let Some(tp) = target_path else {
        return Ok(models_dir.to_path_buf());
    };

    if tp.contains("..") || tp.starts_with('\\') || tp.starts_with('/') {
        return Err("Invalid target_path: path traversal not allowed".to_string());
    }

    let candidate = models_dir.join(tp);
    std::fs::create_dir_all(&candidate)
        .map_err(|e| format!("Failed to create target_path: {}", e))?;
    let candidate_canon = candidate.canonicalize().map_err(|e| {
        format!(
            "Failed to resolve target_path {}: {}",
            candidate.display(),
            e
        )
    })?;

    if !candidate_canon.starts_with(&models_dir_canon) {
        return Err("target_path escapes models_dir".to_string());
    }

    Ok(candidate_canon)
}

#[derive(Debug)]
pub(crate) struct ApiError {
    pub(crate) status: StatusCode,
    pub(crate) message: String,
}

impl ApiError {
    fn new(status: StatusCode, message: impl Into<String>) -> Self {
        Self {
            status,
            message: message.into(),
        }
    }

    fn internal(message: impl Into<String>) -> Self {
        Self::new(StatusCode::INTERNAL_SERVER_ERROR, message)
    }

    fn busy(message: impl Into<String>) -> Self {
        Self::new(StatusCode::TOO_MANY_REQUESTS, message)
    }

    fn gateway(message: impl Into<String>) -> Self {
        Self::new(StatusCode::BAD_GATEWAY, message)
    }

    fn gateway_timeout(message: impl Into<String>) -> Self {
        Self::new(StatusCode::GATEWAY_TIMEOUT, message)
    }

    fn from_reqwest(err: reqwest::Error) -> Self {
        if err.is_timeout() {
            return Self::gateway_timeout("Timed out waiting for the active llama-server.");
        }

        if err.is_connect() {
            return Self::gateway("Cannot connect to the active llama-server.");
        }

        let detail = err.to_string();
        if detail.contains("error sending request") || detail.contains("connection reset") {
            return Self::gateway(
                "The active llama-server dropped the request before streaming started.",
            );
        }

        Self::gateway(format!("Upstream request failed: {detail}"))
    }

    fn from_upstream_status(status: StatusCode, body: String) -> Self {
        let detail = body.trim();
        let lower = detail.to_ascii_lowercase();
        if status == StatusCode::TOO_MANY_REQUESTS
            || lower.contains("busy")
            || lower.contains("no slot")
            || lower.contains("no available slot")
        {
            let message = if detail.is_empty() {
                "The active llama-server is busy with another request."
            } else {
                detail
            };
            return Self::busy(message.to_string());
        }

        let message = if detail.is_empty() {
            format!("Upstream llama-server returned HTTP {}.", status.as_u16())
        } else {
            format!("Upstream HTTP {}: {detail}", status.as_u16())
        };
        Self::new(status, message)
    }
}

impl fmt::Display for ApiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for ApiError {}

impl Reject for ApiError {}

const MONITOR_INFERENCE_QUEUE_TIMEOUT: Duration = Duration::from_secs(330);
const UPSTREAM_BUSY_WAIT_TIMEOUT: Duration = Duration::from_secs(300);
const UPSTREAM_BUSY_POLL_INTERVAL: Duration = Duration::from_millis(500);
const UPSTREAM_SEND_RETRIES: usize = 3;
const UPSTREAM_SEND_RETRY_BACKOFF_MS: u64 = 250;

fn active_chat_completions_url(state: &AppState) -> Result<String, warp::Rejection> {
    let session = state
        .get_active_session()
        .ok_or(warp::reject::not_found())?;
    Ok(match &session.mode {
        crate::state::SessionMode::Spawn { port, .. } => {
            format!("http://127.0.0.1:{port}/v1/chat/completions")
        }
        crate::state::SessionMode::Attach { endpoint, .. } => {
            format!("{endpoint}/v1/chat/completions")
        }
    })
}

fn upstream_has_capacity(state: &AppState) -> Result<bool, warp::Rejection> {
    let server_running = *state.server_running.lock().map_err(|e| {
        warp::reject::custom(ApiError::internal(format!(
            "Failed to read server state: {e}"
        )))
    })?;
    if !server_running {
        return Err(warp::reject::custom(ApiError::gateway(
            "Cannot reach the active llama-server.",
        )));
    }

    let metrics = state.llama_metrics.lock().map_err(|e| {
        warp::reject::custom(ApiError::internal(format!(
            "Failed to read llama metrics: {e}"
        )))
    })?;
    let total_slots = metrics.slots_idle.saturating_add(metrics.slots_processing);
    if total_slots > 0 {
        return Ok(metrics.slots_idle > 0 || metrics.slots_processing == 0);
    }

    Ok(metrics.requests_processing == 0)
}

async fn wait_for_upstream_capacity(state: &AppState) -> Result<(), warp::Rejection> {
    let deadline = tokio::time::Instant::now() + UPSTREAM_BUSY_WAIT_TIMEOUT;

    loop {
        if upstream_has_capacity(state)? {
            return Ok(());
        }

        if tokio::time::Instant::now() >= deadline {
            return Err(warp::reject::custom(ApiError::busy(
                "The active llama-server has been busy for too long. The server may be running a very large inference. Wait for it to finish and retry.",
            )));
        }

        tokio::time::sleep(UPSTREAM_BUSY_POLL_INTERVAL).await;
    }
}

async fn acquire_inference_permit(
    state: &AppState,
) -> Result<tokio::sync::OwnedSemaphorePermit, warp::Rejection> {
    tokio::time::timeout(
        MONITOR_INFERENCE_QUEUE_TIMEOUT,
        state.monitor_inference_gate.clone().acquire_owned(),
    )
    .await
    .map_err(|_| {
        warp::reject::custom(ApiError::busy(
            "Another llama-monitor inference request has been queued for too long. Retry in a moment.",
        ))
    })?
    .map_err(|_| warp::reject::custom(ApiError::internal("Inference gate was closed.")))
}

async fn prepare_inference_request(
    state: &AppState,
) -> Result<(String, tokio::sync::OwnedSemaphorePermit), warp::Rejection> {
    let url = active_chat_completions_url(state)?;
    let permit = acquire_inference_permit(state).await?;
    wait_for_upstream_capacity(state).await?;
    Ok((url, permit))
}

fn build_upstream_client(timeout: Duration) -> Result<reqwest::Client, warp::Rejection> {
    reqwest::Client::builder()
        .timeout(timeout)
        .build()
        .map_err(|e| {
            warp::reject::custom(ApiError::internal(format!(
                "Failed to create HTTP client: {e}"
            )))
        })
}

fn should_retry_send_error(err: &reqwest::Error) -> bool {
    err.is_timeout()
        || err.is_connect()
        || err.to_string().contains("error sending request")
        || err.to_string().contains("connection reset")
}

async fn send_upstream_request_with_retry<F>(
    mut make_request: F,
) -> Result<reqwest::Response, warp::Rejection>
where
    F: FnMut() -> reqwest::RequestBuilder,
{
    let mut attempt = 0usize;
    let mut backoff_ms = UPSTREAM_SEND_RETRY_BACKOFF_MS;

    loop {
        attempt += 1;

        match make_request().send().await {
            Ok(resp) if resp.status().is_success() => return Ok(resp),
            Ok(resp) => {
                let status = resp.status();
                let err_body = resp.text().await.unwrap_or_default();
                let mapped = ApiError::from_upstream_status(status, err_body);
                if attempt < UPSTREAM_SEND_RETRIES && mapped.status == StatusCode::TOO_MANY_REQUESTS
                {
                    tokio::time::sleep(Duration::from_millis(backoff_ms)).await;
                    backoff_ms *= 2;
                    continue;
                }
                return Err(warp::reject::custom(mapped));
            }
            Err(err) => {
                if attempt < UPSTREAM_SEND_RETRIES && should_retry_send_error(&err) {
                    tokio::time::sleep(Duration::from_millis(backoff_ms)).await;
                    backoff_ms *= 2;
                    continue;
                }
                return Err(warp::reject::custom(ApiError::from_reqwest(err)));
            }
        }
    }
}

/// Extract bearer token from Authorization header.
fn extract_bearer(auth: Option<String>) -> Option<String> {
    auth.and_then(|v| v.strip_prefix("Bearer ").map(str::to_string))
}

/// 401 JSON reply for missing api-token.
fn unauthorized_api_token() -> Box<dyn warp::reply::Reply> {
    Box::new(warp::reply::with_status(
        warp::reply::json(&serde_json::json!({
            "ok": false,
            "error": "unauthorized; api-token required"
        })),
        warp::http::StatusCode::UNAUTHORIZED,
    ))
}

/// 401 JSON reply for missing db-admin-token.
fn unauthorized_db_admin_token() -> Box<dyn warp::reply::Reply> {
    Box::new(warp::reply::with_status(
        warp::reply::json(&serde_json::json!({
            "ok": false,
            "error": "unauthorized; db-admin-token required"
        })),
        warp::http::StatusCode::UNAUTHORIZED,
    ))
}

/// Check if the Authorization header matches the configured api-token.
pub fn check_api_token(auth: &Option<String>, cfg: &AppConfig) -> bool {
    let bearer = auth.as_ref().and_then(|v| v.strip_prefix("Bearer "));
    bearer_matches_api_token(bearer, cfg)
}

/// Check if the Authorization header matches the configured db-admin-token.
fn check_db_admin_token(auth: &Option<String>, cfg: &AppConfig) -> bool {
    let bearer = auth.as_ref().and_then(|v| v.strip_prefix("Bearer "));
    bearer_matches_db_admin_token(bearer, cfg)
}

/// Compare an already-extracted bearer token against the live api-token (constant-time).
/// If no api-token is configured, allow the request (local-first mode).
/// If api-token is configured but no bearer is provided, reject.
fn bearer_matches_api_token(bearer: Option<&str>, cfg: &AppConfig) -> bool {
    use subtle::ConstantTimeEq;
    let live = cfg.live_api_token();
    match (bearer, live.as_deref()) {
        (Some(got), Some(expected)) if !expected.is_empty() => {
            got.as_bytes().ct_eq(expected.as_bytes()).into()
        }
        // Token is configured but no bearer provided → reject.
        (None, Some(expected)) if !expected.is_empty() => false,
        // No token configured → allow (local-first mode).
        _ => true,
    }
}

/// Compare an already-extracted bearer token against the live db-admin-token (constant-time).
/// If no db-admin-token is configured, allow the request (local-first mode).
/// If db-admin-token is configured but no bearer is provided, reject.
fn bearer_matches_db_admin_token(bearer: Option<&str>, cfg: &AppConfig) -> bool {
    use subtle::ConstantTimeEq;
    let live = cfg.live_db_admin_token();
    match (bearer, live.as_deref()) {
        (Some(got), Some(expected)) if !expected.is_empty() => {
            got.as_bytes().ct_eq(expected.as_bytes()).into()
        }
        // Token is configured but no bearer provided → reject.
        (None, Some(expected)) if !expected.is_empty() => false,
        // No token configured → allow (local-first mode).
        _ => true,
    }
}

use crate::config::{AppConfig, DashboardAuthConfig, TlsMode, clear_auth_config, save_auth_config};
use crate::gpu::env::{self as gpu_env, GPU_ARCHITECTURES, GpuEnv};

#[cfg(target_os = "windows")]
use crate::lhm;

use crate::lhm_persistence as lhm_persist;
use crate::llama::server::{self, ServerConfig};
use crate::models;
use crate::presets::{self, ModelPreset};
use crate::remote_ssh::{self, SshConnection};
use crate::state::{self as app_state, AppState, SessionStatus, UiSettings};
use crate::web::auth::{AuthManager, AuthMethod, AuthSource};

#[allow(dead_code)]
mod legacy_chat_types {
    use super::ContextNote;
    /// Chat message structure for persistence (legacy flat-file types, used by tests)
    #[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
    pub struct CompactionPreview {
        pub role: String,
        pub snippet: String,
    }

    #[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
    pub struct ChatMessage {
        pub role: String,
        pub content: String,
        pub timestamp_ms: u64,
        #[serde(default)]
        pub input_tokens: Option<u64>,
        #[serde(default)]
        pub output_tokens: Option<u64>,
        #[serde(default, alias = "cumulativeInputTokens")]
        pub cumulative_input_tokens: Option<u64>,
        #[serde(default, alias = "cumulativeOutputTokens")]
        pub cumulative_output_tokens: Option<u64>,
        #[serde(default)]
        pub compaction_marker: Option<bool>,
        #[serde(default)]
        pub summarized: Option<bool>,
        #[serde(default)]
        pub dropped_count: Option<u64>,
        #[serde(default)]
        pub dropped_preview: Option<Vec<CompactionPreview>>,
        #[serde(default)]
        pub tokens_freed_estimate: Option<u64>,
        #[serde(default)]
        pub ctx_pct_before: Option<f32>,
        #[serde(default)]
        pub memory_version: Option<u32>,
        #[serde(default)]
        pub memory_domain: Option<String>,
        #[serde(default)]
        pub summary_kind: Option<String>,
        #[serde(default)]
        pub compacted_at: Option<u64>,
        #[serde(default)]
        pub compacted_message_count_total: Option<u64>,
        #[serde(default)]
        pub recent_tail_kept: Option<u64>,
        #[serde(default)]
        pub thinking_content: Option<String>,
    }

    #[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
    pub struct ArmedStoryBeat {
        pub id: String,
        pub kind: String,
        pub instruction: String,
        #[serde(default)]
        pub remaining_turns: u32,
        #[serde(default)]
        pub created_at: u64,
        #[serde(default = "default_true")]
        pub enabled: bool,
    }

    fn default_true() -> bool {
        true
    }

    /// Model parameters for a chat tab
    #[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
    pub struct ChatModelParams {
        pub temperature: f32,
        pub top_p: f32,
        pub top_k: u32,
        pub min_p: f32,
        pub repeat_penalty: f32,
        pub max_tokens: Option<u32>,
    }

    impl Default for ChatModelParams {
        fn default() -> Self {
            Self {
                temperature: 0.7,
                top_p: 0.9,
                top_k: 40,
                min_p: 0.01,
                repeat_penalty: 1.0,
                max_tokens: None,
            }
        }
    }

    /// Deserialize explicit_level from bool (legacy), u8, or null.
    /// Tolerates corrupted disk data by defaulting to 0.
    fn deserialize_explicit_level<'de, D>(deserializer: D) -> Result<Option<u8>, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = <Option<serde_json::Value> as serde::Deserialize>::deserialize(deserializer)?;
        match value {
            Some(v) if v.is_null() => Ok(None),
            Some(v) => {
                if let Some(n) = v.as_u64() {
                    return Ok(Some(n as u8));
                }
                if let Some(b) = v.as_bool() {
                    return Ok(Some(if b { 1 } else { 0 }));
                }
                if let Some(n) = v.as_i64() {
                    return Ok(Some(n as u8));
                }
                // Tolerate corrupted data: default to unlocked (1)
                Ok(Some(0))
            }
            None => Ok(None),
        }
    }

    /// Chat tab structure for persistence (legacy flat-file)
    #[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
    pub struct ChatTab {
        pub id: String,
        pub name: String,
        pub system_prompt: String,
        #[serde(default)]
        pub ai_name: Option<String>,
        #[serde(default)]
        pub user_name: Option<String>,
        #[serde(
            default,
            rename = "explicitLevel",
            alias = "explicit_mode",
            alias = "explicit_level",
            deserialize_with = "deserialize_explicit_level"
        )]
        pub explicit_level: Option<u8>,
        pub messages: Vec<ChatMessage>,
        // Standardized on snake_case to avoid duplicate field errors.
        #[serde(default)]
        pub total_input_tokens: Option<u64>,
        #[serde(default)]
        pub total_output_tokens: Option<u64>,
        #[serde(default)]
        pub model_params: ChatModelParams,
        #[serde(default)]
        pub created_at: u64,
        #[serde(default)]
        pub updated_at: u64,
        #[serde(default)]
        pub auto_compact: Option<bool>,
        #[serde(default)]
        pub auto_compact_summarize: Option<bool>,
        #[serde(default)]
        pub compact_threshold: Option<f32>,
        #[serde(default)]
        pub compact_mode: Option<String>,
        /// Last known context window percentage (0–100). Derived client-side and
        /// persisted so the dashboard can show it on page load without a live session.
        #[serde(rename = "lastCtxPct", default)]
        pub last_ctx_pct: Option<f32>,
        #[serde(rename = "activeTemplateId", default)]
        pub active_template_id: Option<String>,
        /// Guided generation: persistent context notes (character, setting, plot details)
        #[serde(default)]
        pub context_notes: Vec<ContextNote>,
        /// Guided generation: remembered sidebar width in pixels (default: 280)
        #[serde(default, rename = "sidebarWidth")]
        pub sidebar_width: u32,
        /// Guided generation: persistent quick guide applied to subsequent replies
        #[serde(default)]
        pub quick_guide_active: String,
        /// Guided generation: delayed hidden story beats for future assistant turns
        #[serde(default)]
        pub armed_story_beats: Vec<ArmedStoryBeat>,
        /// Custom role boundary instruction (overrides the default if set)
        #[serde(default)]
        pub role_boundary_custom: Option<String>,
        /// Gender for {{gender}} token substitution: "male", "female", or "neutral"
        #[serde(default)]
        pub ai_gender: Option<String>,
    }
}

/// Context note for guided generation (character details, setting info, etc.)
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct ContextNote {
    pub section: String,
    pub content: String,
    #[serde(default)]
    pub created_at: u64,
}

/// Suggestion request for guided generation
#[derive(serde::Deserialize, Debug)]
pub struct SuggestionRequest {
    pub tab_id: String,
    pub category: String,
    #[serde(default)]
    pub count: Option<u32>,
    #[serde(default)]
    pub context_depth: Option<u32>,
    #[serde(default)]
    pub messages: Option<Vec<SuggestionContextMessage>>,
    #[serde(default)]
    pub system_prompt: Option<String>,
    #[serde(default)]
    pub context_notes: Option<Vec<ContextNote>>,
    #[serde(default)]
    pub quick_guide_active: Option<String>,
    pub prompt: Option<String>,
}

#[derive(serde::Deserialize, Clone, Debug)]
pub struct SuggestionContextMessage {
    pub role: String,
    pub content: String,
}

/// Suggestion response
#[derive(serde::Serialize, Debug)]
pub struct SuggestionResponse {
    pub suggestions: Vec<String>,
    #[serde(default)]
    pub cards: Vec<SuggestionCard>,
    pub category: String,
    pub count: u32,
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
pub struct SuggestionCard {
    #[serde(rename = "type")]
    pub suggestion_type: String,
    pub title: String,
    pub effect: String,
    #[serde(default)]
    pub detail: String,
}

/// Keyword generation request
#[derive(serde::Deserialize, Debug)]
pub struct KeywordRequest {
    pub category: String,
}

/// Keyword generation response
#[derive(serde::Serialize, Debug)]
pub struct KeywordResponse {
    pub keywords: Vec<String>,
}

/// Context notes analysis request
#[derive(serde::Deserialize, Debug)]
pub struct ContextNotesAnalyzeRequest {
    pub messages: Vec<SuggestionContextMessage>,
    #[serde(default)]
    pub system_prompt: Option<String>,
    pub existing_notes: Vec<ContextNote>,
    pub sections: Vec<String>,
}

/// Per-section analysis result
#[derive(serde::Serialize, Debug)]
pub struct SectionAnalysis {
    pub section: String,
    pub suggested: String,
    /// "new" | "current" | "stale"
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

/// Context notes analysis response
#[derive(serde::Serialize, Debug)]
pub struct ContextNotesAnalyzeResponse {
    pub sections: Vec<SectionAnalysis>,
}

fn default_suggestions_output_contract(count: u32) -> String {
    format!(
        "\n\nFINAL OUTPUT REQUIREMENTS:\n\
Return ONLY valid JSON. Do not include reasoning, planning, commentary, markdown, code fences, or preamble.\n\
Return exactly {count} items in this shape:\n\
{{\"suggestions\":[{{\"title\":\"Short Title\",\"description\":\"One concise actionable next beat.\"}}]}}\n\
Rules:\n\
- `title` must be plain text, under 8 words, and user-facing.\n\
- `description` must be plain text, concise, specific, and actionable.\n\
- Do not output keys other than `suggestions`, `title`, and `description`.\n\
- Do not mention your thinking process.\n\
- Do not number the suggestions.\n"
    )
}

fn director_output_contract(count: u32) -> String {
    format!(
        "\n\nFINAL OUTPUT REQUIREMENTS:\n\
Return ONLY valid JSON. Do not include reasoning, planning, commentary, markdown, code fences, or preamble.\n\
Return exactly {count} items in this shape:\n\
{{\"suggestions\":[{{\"type\":\"pressure\",\"title\":\"Short Title\",\"effect\":\"Short immediate effect line.\",\"detail\":\"One concise sentence explaining how the assistant should continue the scene.\"}}]}}\n\
Rules:\n\
- `type` must be one of: pressure, reveal, escalation, interruption, twist, tone-shift, reversal, intimacy, investigation, confrontation.\n\
- `title` must be plain text, under 8 words, and user-facing.\n\
- `effect` must be a compact summary line under 12 words.\n\
- `detail` must be one concise actionable sentence.\n\
- Do not output keys other than `suggestions`, `type`, `title`, `effect`, and `detail`.\n\
- Do not mention your thinking process.\n\
- Do not number the suggestions.\n"
    )
}

fn suggestions_output_contract(category: &str, count: u32) -> String {
    if category == "director" {
        director_output_contract(count)
    } else {
        default_suggestions_output_contract(count)
    }
}

fn suggestion_temperature(category: &str) -> f32 {
    match category {
        "director" => 0.75,
        "general" => 0.8,
        "plot-twist" => 0.95,
        "new-character" => 0.9,
        "explicit" => 0.9,
        "action" | "comedy" | "fantasy" | "horror" | "mystery" | "noir" | "romance" | "sci-fi"
        | "thriller" | "character" => 0.85,
        _ => 0.85,
    }
}

/// Load prompts from static/prompts directory
fn load_prompts_from_files() -> HashMap<String, String> {
    let mut prompts = HashMap::new();

    // Try to load from static/prompts directory relative to executable
    let prompts_dir = PathBuf::from("static/prompts");

    if let Ok(entries) = fs::read_dir(&prompts_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().is_some_and(|ext| ext == "md")
                && let Some(stem) = path.file_stem().and_then(|s| s.to_str())
                && let Ok(content) = fs::read_to_string(&path)
            {
                prompts.insert(stem.to_string(), content);
            }
        }
    }

    prompts
}

use crate::chat_storage::{ChatStorage, TabVisibility};

/// Attempt to claim a cooldown window atomically.
///
/// Returns `(allowed, remaining_secs)`. `allowed` is `true` only if the cooldown
/// has elapsed AND this caller atomically claimed the slot (preventing concurrent
/// double-execution). `remaining_secs` gives a hint for the retry-after header.
fn try_cooldown(last: &std::sync::atomic::AtomicU64, now: u64, cooldown_secs: u64) -> (bool, u64) {
    use std::sync::atomic::Ordering;
    let prev = last.load(Ordering::Acquire);
    let elapsed = now.saturating_sub(prev);
    if elapsed < cooldown_secs {
        return (false, cooldown_secs - elapsed);
    }
    let ok = last
        .compare_exchange(prev, now, Ordering::AcqRel, Ordering::Relaxed)
        .is_ok();
    (ok, 0)
}

fn parse_visibility_param(param: &str) -> Vec<TabVisibility> {
    if param.is_empty() || param == "active" {
        vec![TabVisibility::Active]
    } else if param == "all" {
        vec![
            TabVisibility::Active,
            TabVisibility::Archived,
            TabVisibility::Hidden,
        ]
    } else if param == "archived" {
        vec![TabVisibility::Archived]
    } else if param == "hidden" {
        vec![TabVisibility::Hidden]
    } else {
        param
            .split(',')
            .map(|s| s.trim().parse().unwrap_or(TabVisibility::Active))
            .collect()
    }
}

fn api_check_lhm(
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "lhm" / "check")
        .and(warp::get())
        .and(warp::header::optional::<String>("authorization"))
        .and_then(move |auth: Option<String>| {
            let cfg = app_config.clone();
            async move {
                if !check_api_token(&auth, &cfg) {
                    return Ok(unauthorized_api_token());
                }
                #[cfg(target_os = "windows")]
                {
                    let running = lhm::is_lhm_running();
                    let installed = lhm::is_lhm_installed();
                    Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(warp::reply::json(
                        &serde_json::json!({
                            "running": running,
                            "installed": installed,
                            "available": running
                        }),
                    )))
                }

                #[cfg(not(target_os = "windows"))]
                {
                    Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(warp::reply::json(
                        &serde_json::json!({
                            "running": false,
                            "installed": false,
                            "available": false,
                            "error": "Not supported on this platform"
                        }),
                    )))
                }
            }
        })
}

fn api_lhm_start(
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "lhm" / "start")
        .and(warp::post())
        .and(warp::header::optional::<String>("authorization"))
        .and_then(move |auth: Option<String>| {
            let cfg = app_config.clone();
            async move {
                if !check_api_token(&auth, &cfg) {
                    return Ok(unauthorized_api_token());
                }
                #[cfg(target_os = "windows")]
                {
                    match lhm::start_lhm().await {
                        Ok(()) => Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(
                            Box::new(warp::reply::json(&serde_json::json!({"success": true}))),
                        ),
                        Err(e) => Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(
                            Box::new(warp::reply::json(&serde_json::json!({"success": false, "error": e}))),
                        ),
                    }
                }

                #[cfg(not(target_os = "windows"))]
                {
                    Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(
                        Box::new(warp::reply::json(&serde_json::json!({"success": false, "error": "Not supported on this platform"}))),
                    )
                }
            }
        })
}

fn api_lhm_status(
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    let lhm_disabled_file = app_config.lhm_disabled_file.clone();
    warp::path!("api" / "lhm" / "status")
        .and(warp::get())
        .and(warp::header::optional::<String>("authorization"))
        .and_then(move |auth: Option<String>| {
            let cfg = app_config.clone();
            #[allow(unused_variables)]
            let file = lhm_disabled_file.clone();
            async move {
                if !check_api_token(&auth, &cfg) {
                    return Ok(unauthorized_api_token());
                }
                #[cfg(target_os = "windows")]
                {
                    match lhm_persist::load_lhm_disabled(&file) {
                        Ok(disabled) => {
                            Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                                warp::reply::json(&serde_json::json!({"disabled": disabled})),
                            ))
                        }
                        Err(_) => Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                            warp::reply::json(&serde_json::json!({"disabled": false})),
                        )),
                    }
                }

                #[cfg(not(target_os = "windows"))]
                {
                    Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(warp::reply::json(
                        &serde_json::json!({"disabled": false}),
                    )))
                }
            }
        })
}

fn api_lhm_progress(
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "lhm" / "progress")
        .and(warp::get())
        .and(warp::header::optional::<String>("authorization"))
        .and_then(move |auth: Option<String>| {
            let cfg = app_config.clone();
            #[cfg(target_os = "windows")]
            {
                async move {
                    use std::fs;

                    if !check_api_token(&auth, &cfg) {
                        return Ok(unauthorized_api_token());
                    }

                    let local_app_data = match std::env::var("LOCALAPPDATA") {
                        Ok(val) => val,
                        Err(_) => {
                            return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                                warp::reply::json(
                                    &serde_json::json!({"progress": "error: LOCALAPPDATA not set"}),
                                ),
                            ));
                        }
                    };
                    let progress_file = std::path::Path::new(&local_app_data)
                        .join("LibreHardwareMonitor")
                        .join("install_progress.txt");

                    let progress = fs::read_to_string(&progress_file)
                        .map(|s| s.trim().to_string())
                        .unwrap_or_else(|_| "not_started".to_string());

                    Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(warp::reply::json(
                        &serde_json::json!({"progress": progress}),
                    )))
                }
            }

            #[cfg(not(target_os = "windows"))]
            {
                async move {
                    if !check_api_token(&auth, &cfg) {
                        return Ok(unauthorized_api_token());
                    }
                    Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(warp::reply::json(
                        &serde_json::json!({"progress": "not_supported"}),
                    )))
                }
            }
        })
}

fn api_lhm_install(
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "lhm" / "install")
        .and(warp::post())
        .and(warp::header::optional::<String>("authorization"))
        .and_then(move |auth: Option<String>| {
            let cfg = app_config.clone();
            async move {
                if !check_api_token(&auth, &cfg) {
                    return Ok(unauthorized_api_token());
                }
                #[cfg(target_os = "windows")]
                {
                    eprintln!("[API] /api/lhm/install called");
                    match lhm::download_and_install_lhm().await {
                        Ok(()) => {
                            eprintln!("[API] LHM install succeeded");
                            Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(
                                Box::new(warp::reply::json(&serde_json::json!({"success": true}))),
                            )
                        }
                        Err(e) => {
                            eprintln!("[API] LHM install failed: {}", e);
                            Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(
                                Box::new(warp::reply::json(&serde_json::json!({"success": false, "error": e}))),
                            )
                        }
                    }
                }

                #[cfg(not(target_os = "windows"))]
                {
                    eprintln!("[API] /api/lhm/install called (non-Windows, not supported)");
                    Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(
                        Box::new(warp::reply::json(&serde_json::json!({"success": false, "error": "Not supported on this platform"}))),
                    )
                }
            }
        })
}

fn api_lhm_uninstall(
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "lhm" / "uninstall")
        .and(warp::post())
        .and(warp::header::optional::<String>("authorization"))
        .and_then(move |auth: Option<String>| {
            let cfg = app_config.clone();
            async move {
                if !check_api_token(&auth, &cfg) {
                    return Ok(unauthorized_api_token());
                }
                #[cfg(target_os = "windows")]
                {
                    eprintln!("[API] /api/lhm/uninstall called");
                    match lhm::uninstall_lhm() {
                        Ok(()) => {
                            eprintln!("[API] LHM uninstall succeeded");
                            Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(
                                Box::new(warp::reply::json(&serde_json::json!({"success": true}))),
                            )
                        }
                        Err(e) => {
                            eprintln!("[API] LHM uninstall failed: {}", e);
                            Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(
                                Box::new(warp::reply::json(&serde_json::json!({"success": false, "error": e}))),
                            )
                        }
                    }
                }

                #[cfg(not(target_os = "windows"))]
                {
                    eprintln!("[API] /api/lhm/uninstall called (non-Windows, not supported)");
                    Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(
                        Box::new(warp::reply::json(&serde_json::json!({"success": false, "error": "Not supported on this platform"}))),
                    )
                }
            }
        })
}

fn api_sensor_bridge_status(
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "sensor-bridge" / "status")
        .and(warp::get())
        .and(warp::header::optional::<String>("authorization"))
        .and_then(move |auth: Option<String>| {
            let cfg = app_config.clone();
            async move {
                if !check_api_token(&auth, &cfg) {
                    return Ok(unauthorized_api_token());
                }
                #[cfg(target_os = "windows")]
                {
                    let installed = lhm::is_local_sensor_bridge_service_installed();
                    let running = lhm::is_local_sensor_bridge_running();
                    Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(warp::reply::json(
                        &serde_json::json!({
                            "installed": installed,
                            "running": running,
                            "available": lhm::is_sensor_bridge_available(),
                        }),
                    )))
                }
                #[cfg(not(target_os = "windows"))]
                {
                    Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(warp::reply::json(
                        &serde_json::json!({
                            "installed": false,
                            "running": false,
                            "available": false,
                        }),
                    )))
                }
            }
        })
}

fn api_sensor_bridge_install(
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "sensor-bridge" / "install")
        .and(warp::post())
        .and(warp::header::optional::<String>("authorization"))
        .and_then(move |auth: Option<String>| {
            let cfg = app_config.clone();
            async move {
                if !check_api_token(&auth, &cfg) {
                    return Ok(unauthorized_api_token());
                }
                #[cfg(target_os = "windows")]
                {
                    match lhm::install_local_sensor_bridge() {
                        Ok(()) => Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(
                            Box::new(warp::reply::json(&serde_json::json!({
                                "started": true,
                                "message": "UAC prompt launched — approve it on your desktop to install the sensor service",
                            }))),
                        ),
                        Err(e) => Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(
                            Box::new(warp::reply::json(&serde_json::json!({
                                "started": false,
                                "error": e,
                            }))),
                        ),
                    }
                }
                #[cfg(not(target_os = "windows"))]
                {
                    Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(
                        Box::new(warp::reply::json(&serde_json::json!({
                            "started": false,
                            "error": "Not supported on this platform",
                        }))),
                    )
                }
            }
        })
}

fn api_sensor_bridge_uninstall(
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "sensor-bridge" / "uninstall")
        .and(warp::post())
        .and(warp::header::optional::<String>("authorization"))
        .and_then(move |auth: Option<String>| {
            let cfg = app_config.clone();
            async move {
                if !check_api_token(&auth, &cfg) {
                    return Ok(unauthorized_api_token());
                }
                #[cfg(target_os = "windows")]
                {
                    match lhm::uninstall_local_sensor_bridge() {
                        Ok(()) => Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(
                            Box::new(warp::reply::json(&serde_json::json!({
                                "started": true,
                                "message": "UAC prompt launched — approve it on your desktop to remove the sensor service",
                            }))),
                        ),
                        Err(e) => Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(
                            Box::new(warp::reply::json(&serde_json::json!({
                                "started": false,
                                "error": e,
                            }))),
                        ),
                    }
                }
                #[cfg(not(target_os = "windows"))]
                {
                    Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(
                        Box::new(warp::reply::json(&serde_json::json!({
                            "started": false,
                            "error": "Not supported on this platform",
                        }))),
                    )
                }
            }
        })
}

fn api_disable_lhm(
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    let lhm_disabled_file = app_config.lhm_disabled_file.clone();
    warp::path!("api" / "lhm" / "disable")
        .and(warp::post())
        .and(warp::header::optional::<String>("authorization"))
        .and(warp::body::json())
        .and_then(move |auth: Option<String>, body: serde_json::Value| {
            let cfg = app_config.clone();
            let disabled = body["disabled"].as_bool().unwrap_or(false);
            #[allow(unused_variables)]
            let file = lhm_disabled_file.clone();
            async move {
                if !check_api_token(&auth, &cfg) {
                    return Ok(unauthorized_api_token());
                }
                let result = lhm_persist::save_lhm_disabled(&file, disabled)
                    .map(|_| {
                        Box::new(warp::reply::json(&serde_json::json!({"ok": true})))
                            as Box<dyn warp::reply::Reply>
                    })
                    .unwrap_or_else(|e| {
                        Box::new(warp::reply::json(
                            &serde_json::json!({"ok": false, "error": e}),
                        ))
                    });
                Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(result)
            }
        })
}

fn api_generate_keywords(
    state: AppState,
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "keywords" / "generate")
        .and(warp::post())
        .and(warp::header::optional::<String>("authorization"))
        .and(warp::body::json::<KeywordRequest>())
        .and_then(move |auth: Option<String>, req: KeywordRequest| {
            let cfg = app_config.clone();
            let state = state.clone();
            async move {
                if !check_api_token(&auth, &cfg) {
                    return Ok(unauthorized_api_token());
                }
                let (url, _permit) = prepare_inference_request(&state).await?;

                let prompt = format!(
                    "Generate 3-5 focus keywords for a story category called \"{}\". Return only the keywords, separated by commas. No explanation.",
                    req.category
                );

                let client = build_upstream_client(Duration::from_secs(30))?;
                let payload = serde_json::json!({
                    "messages": [
                        {"role": "system", "content": "You generate focus keywords. Return only the keywords, comma-separated, with no explanation."},
                        {"role": "user", "content": prompt},
                    ],
                    "stream": true,
                    "thinking_budget_tokens": 0,
                    "chat_template_kwargs": {"enable_thinking": false},
                    "temperature": 0.7,
                    "max_tokens": 128,
                });

                let response = send_upstream_request_with_retry(|| {
                    client
                        .post(&url)
                        .header("Content-Type", "application/json")
                        .json(&payload)
                })
                .await?;

                use futures_util::StreamExt;
                let mut upstream = response.bytes_stream();
                let mut buf = String::new();
                let mut content = String::new();

                while let Some(chunk) = upstream.next().await {
                    let chunk = chunk.map_err(|e| warp::reject::custom(ApiError::from_reqwest(e)))?;
                    buf.push_str(&String::from_utf8_lossy(&chunk));

                    while let Some(pos) = buf.find('\n') {
                        let line = buf[..pos].trim().to_string();
                        buf = buf[pos + 1..].to_string();

                        let Some(data) = line.strip_prefix("data: ") else { continue; };
                        let data = data.trim();
                        if data.is_empty() || data == "[DONE]" { continue; }

                        let event: serde_json::Value = serde_json::from_str(data)
                            .map_err(|e| warp::reject::custom(ApiError::internal(e.to_string())))?;
                        if let Some(delta) = event.get("choices")
                            .and_then(|c| c.as_array())
                            .and_then(|c| c.first())
                            .and_then(|c| c.get("delta"))
                            .and_then(|d| d.get("content"))
                            .and_then(|c| c.as_str())
                        {
                            content.push_str(delta);
                        }
                    }
                }

                let content = content.trim().to_string();
                if content.is_empty() {
                    return Err(warp::reject::custom(ApiError::gateway(
                        "upstream returned empty keyword content".to_string(),
                    )));
                }

                let keywords: Vec<String> = content
                    .split(',')
                    .map(|k| k.trim().to_string())
                    .filter(|k| !k.is_empty())
                    .collect();

                if keywords.is_empty() {
                    return Err(warp::reject::custom(ApiError::gateway(
                        "upstream returned no parseable keywords".to_string(),
                    )));
                }

                Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                    warp::reply::json(&KeywordResponse { keywords }),
                ))
            }
        })
}

fn api_analyze_context_notes(
    state: AppState,
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "context-notes" / "analyze")
        .and(warp::post())
        .and(warp::header::optional::<String>("authorization"))
        .and(warp::body::json::<ContextNotesAnalyzeRequest>())
        .and_then(move |auth: Option<String>, req: ContextNotesAnalyzeRequest| {
            let cfg = app_config.clone();
            let state = state.clone();
            async move {
                if !check_api_token(&auth, &cfg) {
                    return Ok(unauthorized_api_token());
                }
                let (url, _permit) = prepare_inference_request(&state).await?;

                // Build a trimmed conversation excerpt (last 20 messages)
                let recent: Vec<_> = req.messages.iter().rev().take(20).rev().collect();
                let convo_text = recent.iter()
                    .map(|m| format!("{}: {}", m.role, m.content))
                    .collect::<Vec<_>>()
                    .join("\n\n");

                // Summarise existing notes for the prompt
                let existing_summary = if req.existing_notes.is_empty() {
                    "None".to_string()
                } else {
                    req.existing_notes.iter()
                        .map(|n| format!("[{}] {}", n.section, n.content))
                        .collect::<Vec<_>>()
                        .join("\n")
                };

                let sections_list = req.sections.join(", ");

                let system_prompt_block = req.system_prompt
                    .as_deref()
                    .filter(|s| !s.trim().is_empty())
                    .map(|s| format!("\nSYSTEM PROMPT (describes the overall scenario/character setup):\n{s}\n"))
                    .unwrap_or_default();

                let system_msg = "You are a creative-writing assistant that analyses a conversation and produces structured context notes. \
                    You MUST return valid JSON only — no markdown fences, no explanation, no extra text. \
                    Follow the schema exactly.";

                let user_msg = format!(
                    "Analyse the conversation below and fill in context notes for each section.\n\
                    Sections to analyse: {sections_list}\n\n\
                    For each section:\n\
                    - Write a concise, high-signal note (1-3 sentences) that a language model would find useful.\n\
                    - Compare your suggestion to the EXISTING note for that section (if any).\n\
                    - Set \"status\" to:\n\
                        \"new\"     — no existing note; you are providing a first suggestion\n\
                        \"current\" — existing note still accurately reflects the conversation\n\
                        \"stale\"   — existing note is outdated or contradicted by recent events\n\
                    - If \"stale\", add a short \"reason\" explaining what changed.\n\
                    {system_prompt_block}\n\
                    EXISTING NOTES:\n{existing_summary}\n\n\
                    CONVERSATION (most recent last):\n{convo_text}\n\n\
                    Return ONLY this JSON structure:\n\
                    {{\"sections\":[{{\"section\":\"<name>\",\"suggested\":\"<note text>\",\"status\":\"new|current|stale\",\"reason\":\"<only if stale>\"}}]}}"
                );

                let client = build_upstream_client(Duration::from_secs(60))?;
                let payload = serde_json::json!({
                    "messages": [
                        {"role": "system", "content": system_msg},
                        {"role": "user", "content": user_msg},
                    ],
                    "stream": true,
                    "thinking_budget_tokens": 0,
                    "chat_template_kwargs": {"enable_thinking": false},
                    "temperature": 0.4,
                    "max_tokens": 1024,
                });

                let response = send_upstream_request_with_retry(|| {
                    client
                        .post(&url)
                        .header("Content-Type", "application/json")
                        .json(&payload)
                })
                .await?;

                use futures_util::StreamExt;
                let mut upstream = response.bytes_stream();
                let mut buf = String::new();
                let mut content = String::new();

                while let Some(chunk) = upstream.next().await {
                    let chunk = chunk.map_err(|e| warp::reject::custom(ApiError::from_reqwest(e)))?;
                    buf.push_str(&String::from_utf8_lossy(&chunk));

                    while let Some(pos) = buf.find('\n') {
                        let line = buf[..pos].trim().to_string();
                        buf = buf[pos + 1..].to_string();

                        let Some(data) = line.strip_prefix("data: ") else { continue; };
                        let data = data.trim();
                        if data.is_empty() || data == "[DONE]" { continue; }

                        let event: serde_json::Value = match serde_json::from_str(data) {
                            Ok(v) => v,
                            Err(_) => continue,
                        };
                        if let Some(delta) = event.get("choices")
                            .and_then(|c| c.as_array())
                            .and_then(|c| c.first())
                            .and_then(|c| c.get("delta"))
                            .and_then(|d| d.get("content"))
                            .and_then(|c| c.as_str())
                        {
                            content.push_str(delta);
                        }
                    }
                }

                let content = content.trim().to_string();
                if content.is_empty() {
                    return Err(warp::reject::custom(ApiError::gateway(
                        "upstream returned empty analysis".to_string(),
                    )));
                }

                // Strip markdown fences if present
                let json_str = {
                    let s = content.trim();
                    let s = s.strip_prefix("```json").unwrap_or(s);
                    let s = s.strip_prefix("```").unwrap_or(s);
                    let s = s.strip_suffix("```").unwrap_or(s);
                    s.trim().to_string()
                };

                let parsed: serde_json::Value = serde_json::from_str(&json_str)
                    .map_err(|e| warp::reject::custom(ApiError::internal(format!(
                        "failed to parse analysis JSON: {e} — raw: {json_str}"
                    ))))?;

                let sections_arr = parsed.get("sections")
                    .and_then(|v| v.as_array())
                    .ok_or_else(|| warp::reject::custom(ApiError::internal(
                        "analysis JSON missing 'sections' array".to_string()
                    )))?;

                let mut sections: Vec<SectionAnalysis> = Vec::new();
                for entry in sections_arr {
                    let section = entry.get("section").and_then(|v| v.as_str()).unwrap_or("").to_string();
                    let suggested = entry.get("suggested").and_then(|v| v.as_str()).unwrap_or("").to_string();
                    let status = entry.get("status").and_then(|v| v.as_str()).unwrap_or("new").to_string();
                    let reason = entry.get("reason").and_then(|v| v.as_str()).map(|s| s.to_string());

                    if section.is_empty() || suggested.is_empty() { continue; }

                    sections.push(SectionAnalysis { section, suggested, status, reason });
                }

                if sections.is_empty() {
                    return Err(warp::reject::custom(ApiError::gateway(
                        "analysis returned no usable section data".to_string(),
                    )));
                }

                Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                    warp::reply::json(&ContextNotesAnalyzeResponse { sections }),
                ))
            }
        })
}

// ========================
// Phase 0: Spawn Llama-Server v2 endpoints
// ========================

// 1) POST /api/spawn-wizard/import-launch-file
fn api_spawn_wizard_import_launch_file(
    _state: AppState,
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (impl warp::reply::Reply,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "spawn-wizard" / "import-launch-file")
        .and(warp::post())
        .and(warp::header::optional::<String>("authorization"))
        .and(super::safe_json_body::<serde_json::Value>())
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
fn is_private_host(host: &str) -> bool {
    // Loopback / localhost
    if host == "localhost" || host == "ip6-localhost" || host == "[::1]" {
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

// 2) POST /api/chat-template/fetch
fn api_chat_template_fetch(
    _state: AppState,
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (impl warp::reply::Reply,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "chat-template" / "fetch")
        .and(warp::post())
        .and(warp::header::optional::<String>("authorization"))
        .and(super::safe_json_body::<serde_json::Value>())
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

// 3) POST /api/chat-template/upload
fn api_chat_template_upload(
    _state: AppState,
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (impl warp::reply::Reply,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "chat-template" / "upload")
        .and(warp::post())
        .and(warp::header::optional::<String>("authorization"))
        .and(super::safe_json_body::<serde_json::Value>())
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

// 4) POST /api/chat-template/install-hf
// Downloads a Jinja template from HuggingFace and saves it with a stable name.
// Returns the cached path immediately if the file already exists.
fn api_chat_template_install_hf(
    _state: AppState,
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (impl warp::reply::Reply,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "chat-template" / "install-hf")
        .and(warp::post())
        .and(warp::header::optional::<String>("authorization"))
        .and(super::safe_json_body::<serde_json::Value>())
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
                    return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                        warp::reply::json(&serde_json::json!({
                            "ok": true,
                            "path": dest.to_string_lossy(),
                            "already_existed": true
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

                Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(warp::reply::json(
                    &serde_json::json!({
                        "ok": true,
                        "path": dest.to_string_lossy(),
                        "already_existed": false
                    }),
                )))
            }
        })
}

// 5) POST /api/vram-estimate (architecture-aware breakdown)
fn api_vram_estimate_breakdown(
    _state: AppState,
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (impl warp::reply::Reply,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "vram-estimate")
        .and(warp::post())
        .and(warp::header::optional::<String>("authorization"))
        .and(super::safe_json_body::<serde_json::Value>())
        .and_then(move |auth: Option<String>, body: serde_json::Value| {
            let cfg = app_config.clone();
            async move {
                if !check_api_token(&auth, &cfg) {
                    return Ok(unauthorized_api_token());
                }

                let model_path = body["model_path"].as_str().unwrap_or("").to_string();
                let n_ctx = body["n_ctx"].as_u64().unwrap_or(4096);
                let _gpu_layers = body["gpu_layers"].as_i64().unwrap_or(-1);
                let parallel_slots = body["parallel_slots"].as_u64().unwrap_or(1) as u32;
                let ubatch_size = body["ubatch_size"].as_u64().unwrap_or(2048) as u32;
                let ctk = body["ctk"].as_str().unwrap_or("q8_0").to_string();
                let ctv = body["ctv"].as_str().unwrap_or("q8_0").to_string();
                let n_cpu_moe = body["n_cpu_moe"].as_i64().map(|v| v as i32).unwrap_or(0);
                let available_vram_bytes = body["available_vram_bytes"].as_u64().unwrap_or(0);
                let is_unified_memory = body["is_unified_memory"].as_bool().unwrap_or(false);

                if model_path.is_empty() {
                    return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(
                        Box::new(warp::reply::json(&serde_json::json!({
                            "ok": false,
                            "error": "model_path is required"
                        }))),
                    );
                }

                let model_size_bytes = match std::fs::metadata(&model_path) {
                    Ok(m) => m.len(),
                    Err(e) => {
                        return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(
                            Box::new(warp::reply::json(&serde_json::json!({
                                "ok": false,
                                "error": format!("Cannot stat model file: {e}")
                            }))),
                        );
                    }
                };

                let arch = match crate::llama::gguf_meta::read_gguf_metadata(
                    std::path::Path::new(&model_path),
                ) {
                    Ok(meta) => {
                        let mm = meta.to_model_metadata();
                        let param_b = meta.param_b().unwrap_or(0.0);
                        mm.to_arch(&model_path, param_b)
                    }
                    Err(_) => crate::llama::vram_estimator::ModelArch::from_name_and_params(
                        &model_path,
                        (model_size_bytes as f64) / 1e9 / 4.85,
                    ),
                };

                let breakdown = crate::llama::vram_estimator::full_estimate(
                    model_size_bytes,
                    &arch,
                    n_ctx,
                    &ctk,
                    &ctv,
                    parallel_slots,
                    ubatch_size,
                    n_cpu_moe,
                    available_vram_bytes,
                    is_unified_memory,
                );

                Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(
                    Box::new(warp::reply::json(&serde_json::json!({
                        "ok": true,
                        "weights_bytes": breakdown.weights_bytes,
                        "kv_cache_bytes": breakdown.kv_cache_bytes,
                        "linear_attn_state_bytes": breakdown.linear_attn_state_bytes,
                        "mmproj_bytes": breakdown.mmproj_bytes,
                        "mtp_bytes": breakdown.mtp_bytes,
                        "overhead_bytes": breakdown.overhead_bytes,
                        "total_bytes": breakdown.total_bytes,
                        "available_bytes": breakdown.available_bytes,
                        "headroom_bytes": breakdown.headroom_bytes,
                        "ram_bytes": breakdown.ram_bytes,
                        "recommendation": serde_json::to_value(&breakdown.recommendation).unwrap_or(serde_json::Value::Null),
                        "note": breakdown.note
                    }))),
                )
            }
        })
}

// 4b) POST /api/vram/estimate (legacy)
fn api_vram_estimate(
    _state: AppState,
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (impl warp::reply::Reply,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "vram" / "estimate")
        .and(warp::post())
        .and(warp::header::optional::<String>("authorization"))
        .and(super::safe_json_body::<serde_json::Value>())
        .and_then(move |auth: Option<String>, body: serde_json::Value| {
            let cfg = app_config.clone();
            async move {
                if !check_api_token(&auth, &cfg) {
                    return Ok(unauthorized_api_token());
                }

                // model: local path used to determine file size (optional when
                // model_size_bytes is provided explicitly).
                let model = body["model"].as_str().unwrap_or("").to_string();
                let context_length = body["context_length"].as_u64().unwrap_or(4096);
                // n_cpu_moe: number of MoE experts to keep on CPU (0 = all on GPU).
                let n_cpu_moe = body["n_cpu_moe"].as_i64().map(|v| v as i32);

                // model_size_bytes can be supplied explicitly (e.g. for HF models where
                // there is no local file yet), otherwise inferred from the filesystem.
                let model_size_bytes = body["model_size_bytes"].as_u64().unwrap_or_else(|| {
                    if model.is_empty() {
                        0
                    } else {
                        std::fs::metadata(&model).map(|m| m.len()).unwrap_or(0)
                    }
                });

                if model_size_bytes == 0 {
                    return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(
                        Box::new(warp::reply::json(&serde_json::json!({
                            "ok": false,
                            "error": "Could not determine model size. Provide a local model path or set 'model_size_bytes' explicitly."
                        }))),
                    );
                }

                let kv_quant = body["kv_quant"].as_str().unwrap_or("q8_0").to_string();
                let batch_size = body["batch_size"].as_u64().unwrap_or(2048) as u32;
                let ubatch_size = body["ubatch_size"].as_u64().unwrap_or(2048) as u32;
                let speculative_decoding = body["speculative_decoding"].as_bool().unwrap_or(false);
                let mmproj_size_bytes = body["mmproj_size_bytes"].as_u64().unwrap_or(0);
                let available_vram_bytes = body["available_vram_bytes"].as_u64().unwrap_or(0);

                let estimate = crate::llama::vram_estimator::estimate_vram(
                    model_size_bytes,
                    context_length,
                    &kv_quant,
                    batch_size,
                    ubatch_size,
                    speculative_decoding,
                    mmproj_size_bytes,
                    n_cpu_moe,
                    available_vram_bytes,
                );

                let estimated_vram_mb =
                    (estimate.estimated_vram_bytes as f64) / (1024.0 * 1024.0);

                Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(
                    Box::new(warp::reply::json(&serde_json::json!({
                        "ok": true,
                        "estimated_vram_mb": estimated_vram_mb,
                        "estimated_vram_bytes": estimate.estimated_vram_bytes,
                        "estimated_ram_bytes": estimate.estimated_ram_bytes,
                        "available_vram_bytes": estimate.available_vram_bytes,
                        "recommendation": serde_json::to_value(&estimate.recommendation).unwrap_or(serde_json::Value::Null),
                        "note": estimate.note
                    }))),
                )
            }
        })
}

// 5) POST /api/models/download/start
fn api_models_download_start(
    state: AppState,
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (impl warp::reply::Reply,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "models" / "download" / "start")
        .and(warp::post())
        .and(warp::header::optional::<String>("authorization"))
        .and(super::safe_json_body::<serde_json::Value>())
        .and_then(move |auth: Option<String>, body: serde_json::Value| {
            let cfg = app_config.clone();
            let state = state.clone();
            async move {
                if !check_db_admin_token(&auth, &cfg)
                    && !check_api_token(&auth, &cfg)
                {
                    return Ok(unauthorized_api_token());
                }

                let model = body["model"]
                    .as_str()
                    .unwrap_or("")
                    .to_string();
                let source = body["source"]
                    .as_str()
                    .unwrap_or("")
                    .to_string();

                if model.is_empty() {
                    return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(
                        Box::new(warp::reply::json(&serde_json::json!({
                            "ok": false,
                            "error": "Missing 'model' field"
                        }))),
                    );
                }

                // For now, only "hf" is implemented via crate::model_download::start_download.
                // For "url" and "local", return unsupported.
                if source != "hf" {
                    return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(
                        Box::new(warp::reply::json(&serde_json::json!({
                            "ok": false,
                            "error": format!("Unsupported source '{}'; only 'hf' is currently supported", source)
                        }))),
                    );
                }

                // Interpret model as "repo_id/file_path" or just "repo_id".
                let (repo_id, file_path) = if model.contains('/') {
                    // Use as-is: repo_id is everything before last segment, file_path is last segment.
                    // For HF-style IDs like "org/model/file.gguf", split at first '/'.
                    let parts: Vec<&str> = model.splitn(2, '/').collect();
                    (parts[0].to_string(), parts.get(1).unwrap_or(&"").to_string())
                } else {
                    (model.clone(), "model.gguf".to_string())
                };

                let target_dir = get_effective_models_dir(&state)
                    .unwrap_or_else(|| cfg.default_models_dir.clone());

                let hf_token = crate::hf::hf_load_token();

                match crate::model_download::start_download(
                    &repo_id,
                    &file_path,
                    None,
                    &target_dir,
                    hf_token,
                ) {
                    Ok(download_id) => {
                        Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(
                            Box::new(warp::reply::json(&serde_json::json!({
                                "ok": true,
                                "download_id": download_id
                            }))),
                        )
                    }
                    Err(e) => {
                        Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(
                            Box::new(warp::reply::json(&serde_json::json!({
                                "ok": false,
                                "error": format!("Failed to start download: {}", e)
                            }))),
                        )
                    }
                }
            }
        })
}

// 6) GET /api/models/download/:id/status
fn api_models_download_status(
    _state: AppState,
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (impl warp::reply::Reply,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "models" / "download" / String / "status")
        .and(warp::get())
        .and(warp::header::optional::<String>("authorization"))
        .and_then(move |id: String, auth: Option<String>| {
            let cfg = app_config.clone();
            async move {
                if !check_api_token(&auth, &cfg) {
                    return Ok(unauthorized_api_token());
                }

                match crate::model_download::get_download_status(&id) {
                    Some(status) => Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                        warp::reply::json(&serde_json::json!({
                            "ok": true,
                            "status": status
                        })),
                    )),
                    None => Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                        warp::reply::with_status(
                            warp::reply::json(&serde_json::json!({
                                "ok": false,
                                "error": "Download not found"
                            })),
                            warp::http::StatusCode::NOT_FOUND,
                        ),
                    )),
                }
            }
        })
}

// 7) POST /api/models/download/:id/cancel
fn api_models_download_cancel(
    _state: AppState,
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (impl warp::reply::Reply,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "models" / "download" / String / "cancel")
        .and(warp::post())
        .and(warp::header::optional::<String>("authorization"))
        .and_then(move |id: String, auth: Option<String>| {
            let cfg = app_config.clone();
            async move {
                if !check_api_token(&auth, &cfg) {
                    return Ok(unauthorized_api_token());
                }

                let ok = crate::model_download::cancel_download(&id);
                if ok {
                    Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(warp::reply::json(
                        &serde_json::json!({
                            "ok": true
                        }),
                    )))
                } else {
                    Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(warp::reply::json(
                        &serde_json::json!({
                            "ok": false,
                            "error": "Download not found or already finished"
                        }),
                    )))
                }
            }
        })
}

// ── POST /api/vram/quant-compare ─────────────────────────────────────────────
// Pre-download quant advisor: returns a comparison table of all quants for a
// given model (identified by param count + optional name) and available VRAM.

fn api_vram_quant_compare(
    _state: AppState,
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (impl warp::reply::Reply,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "vram" / "quant-compare")
        .and(warp::post())
        .and(warp::header::optional::<String>("authorization"))
        .and(super::safe_json_body::<serde_json::Value>())
        .and_then(move |auth: Option<String>, body: serde_json::Value| {
            let cfg = app_config.clone();
            async move {
                if !check_api_token(&auth, &cfg) {
                    return Ok(unauthorized_api_token());
                }

                let param_b = body["param_b"].as_f64().unwrap_or(0.0);
                if param_b <= 0.0 {
                    return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                        warp::reply::json(&serde_json::json!({
                            "ok": false,
                            "error": "param_b must be a positive number (model parameter count in billions)"
                        })),
                    ));
                }

                let model_name = body["model_name"].as_str().unwrap_or("").to_string();
                let available_vram_bytes = body["available_vram_bytes"].as_u64().unwrap_or(0);
                let parallel_slots = body["parallel_slots"].as_u64().unwrap_or(1) as u32;
                let is_unified_memory = body["is_unified_memory"].as_bool().unwrap_or(false);

                let use_case = match body["use_case"].as_str().unwrap_or("general") {
                    "agentic" => crate::llama::vram_estimator::UseCase::Agentic,
                    "roleplay" => crate::llama::vram_estimator::UseCase::Roleplay,
                    _ => crate::llama::vram_estimator::UseCase::General,
                };

                // Optionally accept explicit arch fields to improve accuracy when
                // called after introspection.
                let arch = build_arch_from_body(&body, &model_name, param_b);

                let table = crate::llama::vram_estimator::quant_comparison_table(
                    param_b,
                    &arch,
                    available_vram_bytes,
                    use_case,
                    parallel_slots,
                    is_unified_memory,
                );

                Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(warp::reply::json(
                    &serde_json::json!({ "ok": true, "quants": table }),
                )))
            }
        })
}

// ── POST /api/vram/auto-size ──────────────────────────────────────────────────
// Given model metadata + available VRAM + use case, return recommended settings
// plus a set of alternative scenarios for the scenario cards.

fn api_vram_auto_size(
    _state: AppState,
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (impl warp::reply::Reply,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "vram" / "auto-size")
        .and(warp::post())
        .and(warp::header::optional::<String>("authorization"))
        .and(super::safe_json_body::<serde_json::Value>())
        .and_then(move |auth: Option<String>, body: serde_json::Value| {
            let cfg = app_config.clone();
            async move {
                if !check_api_token(&auth, &cfg) {
                    return Ok(unauthorized_api_token());
                }

                let model_name = body["model_name"].as_str().unwrap_or("").to_string();
                let param_b = body["param_b"].as_f64().unwrap_or(0.0);
                let available_vram_bytes = body["available_vram_bytes"].as_u64().unwrap_or(0);
                let parallel_slots = body["parallel_slots"].as_u64().unwrap_or(1).max(1) as u32;
                let fit_granularity = body["fit_granularity"].as_u64().unwrap_or(1024).max(512);
                let is_unified_memory = body["is_unified_memory"].as_bool().unwrap_or(false);

                let use_case = match body["use_case"].as_str().unwrap_or("general") {
                    "agentic" => crate::llama::vram_estimator::UseCase::Agentic,
                    "roleplay" => crate::llama::vram_estimator::UseCase::Roleplay,
                    _ => crate::llama::vram_estimator::UseCase::General,
                };

                // Model size: explicit bytes > local file stat > param_b heuristic
                let model_size_bytes = body["model_size_bytes"].as_u64().unwrap_or_else(|| {
                    let path = body["model_path"].as_str().unwrap_or("");
                    if !path.is_empty() {
                        std::fs::metadata(path).map(|m| m.len()).unwrap_or(0)
                    } else {
                        0
                    }
                });

                // We need *some* size info
                if model_size_bytes == 0 && param_b <= 0.0 {
                    return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                        warp::reply::json(&serde_json::json!({
                            "ok": false,
                            "error": "Provide model_size_bytes, model_path, or param_b"
                        })),
                    ));
                }

                let arch = build_arch_from_body(&body, &model_name, param_b);

                // If model_size_bytes is not given, estimate from param_b + quant
                let quant_hint = body["quant"].as_str().unwrap_or("q4_k_m");
                let model_bytes = if model_size_bytes > 0 {
                    model_size_bytes
                } else {
                    crate::llama::vram_estimator::estimate_model_size_bytes(param_b, quant_hint)
                };

                let result = crate::llama::vram_estimator::auto_size(
                    model_bytes,
                    &arch,
                    available_vram_bytes,
                    use_case,
                    parallel_slots,
                    fit_granularity,
                    is_unified_memory,
                );

                Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(warp::reply::json(
                    &serde_json::json!({ "ok": true, "result": result }),
                )))
            }
        })
}

/// Build a `ModelArch` from a JSON request body, falling back to heuristics
/// when introspection fields are absent.
fn build_arch_from_body(
    body: &serde_json::Value,
    model_name: &str,
    param_b: f64,
) -> crate::llama::vram_estimator::ModelArch {
    let heuristic =
        crate::llama::vram_estimator::ModelArch::from_name_and_params(model_name, param_b);

    let n_layers = body["n_layers"]
        .as_u64()
        .map(|v| v as u32)
        .unwrap_or(heuristic.n_layers);
    let n_kv_heads = body["n_kv_heads"]
        .as_u64()
        .map(|v| v as u32)
        .unwrap_or(heuristic.n_kv_heads);
    let head_dim = body["head_dim"]
        .as_u64()
        .map(|v| v as u32)
        .unwrap_or(heuristic.head_dim);
    let n_experts = body["n_experts"]
        .as_u64()
        .map(|v| v as u32)
        .unwrap_or(heuristic.n_experts);
    let n_exp_used = body["n_experts_used"]
        .as_u64()
        .map(|v| v as u32)
        .unwrap_or(heuristic.n_experts_used);
    let mtp_depth = body["mtp_depth"]
        .as_u64()
        .map(|v| v as u32)
        .unwrap_or(heuristic.mtp_depth);
    let mmproj_bytes = body["mmproj_bytes"]
        .as_u64()
        .unwrap_or(heuristic.mmproj_bytes);
    let expert_frac = body["expert_fraction"]
        .as_f64()
        .unwrap_or(heuristic.expert_fraction);

    crate::llama::vram_estimator::ModelArch {
        n_layers,
        n_kv_heads,
        head_dim,
        n_global_attn_layers: heuristic.n_global_attn_layers,
        local_attn_window: heuristic.local_attn_window,
        local_kv_heads: heuristic.local_kv_heads,
        // Hybrid DeltaNet: override from body if provided, otherwise preserve heuristic
        n_attn_layers: body["n_attn_layers"]
            .as_u64()
            .map(|v| v as u32)
            .unwrap_or(heuristic.n_attn_layers),
        linear_attn_state_bytes: body["linear_attn_state_bytes"]
            .as_u64()
            .unwrap_or(heuristic.linear_attn_state_bytes),
        n_experts,
        n_experts_used: n_exp_used,
        expert_fraction: expert_frac,
        global_head_dim: heuristic.global_head_dim,
        mtp_depth,
        mmproj_bytes,
        param_b,
    }
}

// ── Phase 2: POST /api/benchmark ─────────────────────────────────────────────

fn api_benchmark(
    state: AppState,
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (impl warp::reply::Reply,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "benchmark")
        .and(warp::post())
        .and(warp::header::optional::<String>("authorization"))
        .and(super::hf_json_body::<serde_json::Value>())
        .and_then(
            move |auth: Option<String>,
                  _body: serde_json::Value|
                  {
                let state = state.clone();
                let cfg = app_config.clone();
                async move {
                    // Auth
                    if !check_api_token(&auth, &cfg) {
                        return Ok(unauthorized_api_token());
                    }

                    // Ensure a server is running
                    let running = match state.server_running.lock() {
                        Ok(g) => *g,
                        Err(_) => {
                            return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(
                                Box::new(warp::reply::json(&serde_json::json!({
                                    "error": "No llama-server is currently running."
                                }))),
                            );
                        }
                    };
                    if !running {
                        return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(
                            Box::new(warp::reply::json(&serde_json::json!({
                                "error": "No llama-server is currently running."
                            }))),
                        );
                    }

                    // Build upstream URL
                    let session = match state.get_active_session() {
                        Some(s) => s,
                        None => {
                            return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(
                                Box::new(warp::reply::json(&serde_json::json!({
                                    "error": "No active session."
                                }))),
                            );
                        }
                    };
                    let url = match &session.mode {
                        crate::state::SessionMode::Spawn { port, .. } => {
                            format!("http://127.0.0.1:{port}/v1/chat/completions")
                        }
                        crate::state::SessionMode::Attach { endpoint, .. } => {
                            format!("{endpoint}/v1/chat/completions")
                        }
                    };

                    let prompt =
                        "Explain in one sentence what llama.cpp is used for.";
                    let max_tokens: u64 = 512;

                    let payload = serde_json::json!({
                        "messages": [{"role": "user", "content": prompt}],
                        "max_tokens": max_tokens,
                        "temperature": 0.5,
                        "stream": true,
                        // Disable thinking mode so Qwen3 reasoning tokens don't inflate TTFT
                        "chat_template_kwargs": {"enable_thinking": false},
                    });

                    let client = match reqwest::Client::builder()
                        .timeout(Duration::from_secs(55))
                        .build()
                    {
                        Ok(c) => c,
                        Err(_) => {
                            return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(
                                Box::new(warp::reply::json(&serde_json::json!({
                                    "error": "Failed to create HTTP client."
                                }))),
                            );
                        }
                    };

                    let result = tokio::time::timeout(
                        Duration::from_secs(60),
                        async {
                            let start = std::time::Instant::now();
                            let mut first_token_time = None;
                            let mut buf = String::new();
                            let mut generated_tokens = 0u64;
                            let mut prompt_tokens_reported = 0u64;

                            let resp = match client
                                .post(&url)
                                .header("Content-Type", "application/json")
                                .json(&payload)
                                .send()
                                .await
                            {
                                Ok(r) => r,
                                Err(_) => return None,
                            };

                            if !resp.status().is_success() {
                                return None;
                            }

                            let mut stream = resp.bytes_stream();
                            use futures_util::StreamExt;

                            while let Some(Ok(chunk)) = stream.next().await {
                                let s = match std::str::from_utf8(&chunk) {
                                    Ok(s) => s.to_string(),
                                    Err(_) => continue,
                                };

                                // Try to parse streaming tokens
                                for line in s.lines() {
                                    let trimmed = line.trim();
                                    if let Some(data) = trimmed.strip_prefix("data: ") {
                                        if data == "[DONE]" {
                                            break;
                                        }
                                        if let Ok(v) =
                                            serde_json::from_str::<serde_json::Value>(data)
                                        {
                                            // Attempt to read token count
                                            if let Some(c) = v["usage"]["completion_tokens"]
                                                .as_u64()
                                            {
                                                generated_tokens = c;
                                            }
                                            // Count tokens from content
                                            if let Some(content) =
                                                v["choices"][0]["delta"]["content"]
                                                    .as_str()
                                            {
                                                if first_token_time.is_none() && !content.is_empty() {
                                                    first_token_time =
                                                        Some(start.elapsed().as_millis() as f64);
                                                }
                                                // Track prompt token count if server reports it
                                                if let Some(p) = v["usage"]["prompt_tokens"].as_u64() {
                                                    prompt_tokens_reported = p;
                                                }
                                                // Each content-bearing chunk ≈ 1 token
                                                if v["usage"]["completion_tokens"].is_null() {
                                                    generated_tokens =
                                                        generated_tokens.saturating_add(1);
                                                }
                                            }
                                        }
                                    }
                                }
                                buf.push_str(&s);
                            }

                            let end = start.elapsed();
                            let ttft_ms =
                                first_token_time.unwrap_or(end.as_millis() as f64);
                            let gen_dur_ms =
                                end.as_millis() as f64 - ttft_ms;
                            let gen_dur_s = gen_dur_ms.max(1.0) / 1000.0;

                            // Fallback: estimate from raw buffer if server didn't report counts
                            if generated_tokens == 0 {
                                generated_tokens = (buf.len() as u64 / 4).max(1);
                            }

                            let ttft_s = ttft_ms / 1000.0;
                            // Use server-reported prompt tokens; fall back to ~¼ char estimate
                            let effective_prompt_tokens = if prompt_tokens_reported > 0 {
                                prompt_tokens_reported as f64
                            } else {
                                (prompt.len() as f64 / 4.0).max(1.0)
                            };
                            let prompt_tps = if ttft_s > 0.0 {
                                effective_prompt_tokens / ttft_s
                            } else {
                                0.0
                            };
                            let gen_tps = if generated_tokens > 0 {
                                (generated_tokens as f64) / gen_dur_s
                            } else {
                                0.0
                            };

                            Some((prompt_tps, gen_tps, ttft_ms))
                        },
                    )
                    .await;

                    match result {
                        Ok(Some((prompt_tps, gen_tps, ttft_ms))) => {
                            let benchmark =
                                crate::llama::spawn_wizard::classify_benchmark_result(
                                    prompt_tps,
                                    gen_tps,
                                    ttft_ms,
                                    None,
                                    None,
                                    0,
                                );
                            Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(
                                Box::new(warp::reply::json(&serde_json::json!({
                                    "prompt_tokens_per_second": (benchmark.prompt_tokens_per_second * 100.0).round() / 100.0,
                                    "gen_tokens_per_second": (benchmark.gen_tokens_per_second * 100.0).round() / 100.0,
                                    "time_to_first_token_ms": (benchmark.time_to_first_token_ms * 100.0).round() / 100.0,
                                    "verdict": benchmark.verdict,
                                    "hints": benchmark.hints,
                                    "suggestions": benchmark.suggestions,
                                }))),
                            )
                        }
                        _ => {
                            Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(
                                Box::new(warp::reply::json(&serde_json::json!({
                                    "error": "Benchmark timed out or failed."
                                }))),
                            )
                        }
                    }
                }
            },
        )
}

// ── Phase 2: POST /api/model-defaults ────────────────────────────────────────

fn api_model_defaults(
    _state: AppState,
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (impl warp::reply::Reply,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "model-defaults")
        .and(warp::post())
        .and(warp::header::optional::<String>("authorization"))
        .and(super::hf_json_body::<serde_json::Value>())
        .and_then(move |auth: Option<String>, body: serde_json::Value| {
            let cfg = app_config.clone();
            async move {
                if !check_api_token(&auth, &cfg) {
                    return Ok(unauthorized_api_token());
                }

                let name_or_repo = body["model_name_or_repo"]
                    .as_str()
                    .unwrap_or("")
                    .to_string();
                let size_bytes = body["size_bytes"].as_u64().unwrap_or(0);
                let tags: Vec<String> = body["tags"]
                    .as_array()
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|v| v.as_str().map(String::from))
                            .collect()
                    })
                    .unwrap_or_default();

                if name_or_repo.is_empty() {
                    return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                        warp::reply::json(&serde_json::json!({
                            "error": "Missing 'model_name_or_repo'."
                        })),
                    ));
                }

                let presets = crate::llama::model_defaults::get_model_presets(
                    &name_or_repo,
                    size_bytes,
                    &tags,
                );
                let defaults = presets
                    .first()
                    .map(|p| p.defaults.clone())
                    .unwrap_or_default();

                Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(warp::reply::json(
                    &serde_json::json!({
                        "temperature": defaults.temperature,
                        "top_p": defaults.top_p,
                        "top_k": defaults.top_k,
                        "min_p": defaults.min_p,
                        "repeat_penalty": defaults.repeat_penalty,
                        "presence_penalty": defaults.presence_penalty,
                        "max_tokens": defaults.max_tokens,
                        "enable_thinking": defaults.enable_thinking,
                        "preserve_thinking": defaults.preserve_thinking,
                        "reasoning_budget": defaults.reasoning_budget,
                        "presets": presets,
                    }),
                )))
            }
        })
}

// ── Phase 2: POST /api/moe-tune ──────────────────────────────────────────────

fn api_moe_tune(
    _state: AppState,
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (impl warp::reply::Reply,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "moe-tune")
        .and(warp::post())
        .and(warp::header::optional::<String>("authorization"))
        .and(super::hf_json_body::<serde_json::Value>())
        .and_then(move |auth: Option<String>, body: serde_json::Value| {
            let cfg = app_config.clone();
            async move {
                if !check_api_token(&auth, &cfg) {
                    return Ok(unauthorized_api_token());
                }

                let model_size_bytes = body["model_size_bytes"].as_u64().unwrap_or(0);
                let available_vram_bytes = body["available_vram_bytes"].as_u64().unwrap_or(0);
                let total_experts: u64 = body["total_experts"].as_u64().unwrap_or(8);

                let suggestion = crate::llama::spawn_wizard::suggest_moe_tuning(
                    model_size_bytes,
                    available_vram_bytes,
                    total_experts,
                );

                Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(warp::reply::json(
                    &serde_json::json!({
                        "recommended_n_cpu_moe": suggestion.recommended_n_cpu_moe,
                        "note": suggestion.note,
                    }),
                )))
            }
        })
}

// ── Apple Silicon: set Metal GPU wired memory limit ───────────────────────────
// Uses osascript to invoke `sysctl iogpu.wired_limit_mb=N` with administrator
// privileges via the macOS native password dialog. No password touches the app.
// Only compiled on macOS; on other platforms returns a not-supported error.

#[cfg(target_os = "macos")]
fn api_set_metal_gpu_limit(
    _state: AppState,
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (impl warp::reply::Reply,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "system" / "set-metal-gpu-limit")
        .and(warp::post())
        .and(warp::header::optional::<String>("authorization"))
        .and(super::safe_json_body::<serde_json::Value>())
        .and_then(move |auth: Option<String>, body: serde_json::Value| {
            let cfg = app_config.clone();
            async move {
                if !check_api_token(&auth, &cfg) {
                    return Ok(unauthorized_api_token());
                }

                let limit_mb = match body["limit_mb"].as_u64() {
                    Some(v) if v > 0 => v,
                    _ => {
                        return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                            warp::reply::json(&serde_json::json!({
                                "ok": false,
                                "error": "limit_mb must be a positive integer (MiB)"
                            })),
                        ));
                    }
                };

                // Single-line osascript command (AppleScript string literals cannot span
                // newlines). Use full binary paths so the restricted do-shell-script PATH
                // (/usr/bin:/bin:/usr/sbin:/sbin) is never an issue.
                // Logic: apply sysctl immediately, then upsert the line in /etc/sysctl.conf
                // for persistence across reboots. Subshell grouping avoids if/then/fi.
                let manual_cmd = format!(
                    "sudo /usr/sbin/sysctl -w iogpu.wired_limit_mb={n} && grep -q '^iogpu.wired_limit_mb=' /etc/sysctl.conf 2>/dev/null && sudo /usr/bin/sed -i '' 's/iogpu.wired_limit_mb=.*/iogpu.wired_limit_mb={n}/' /etc/sysctl.conf || echo 'iogpu.wired_limit_mb={n}' | sudo /usr/bin/tee -a /etc/sysctl.conf",
                    n = limit_mb
                );
                let shell_cmd = format!(
                    "/usr/sbin/sysctl iogpu.wired_limit_mb={n} && (/usr/bin/grep -q '^iogpu.wired_limit_mb=' /etc/sysctl.conf 2>/dev/null && /usr/bin/sed -i '' 's/iogpu.wired_limit_mb=.*/iogpu.wired_limit_mb={n}/' /etc/sysctl.conf || /bin/echo 'iogpu.wired_limit_mb={n}' >> /etc/sysctl.conf)",
                    n = limit_mb
                );
                let script = format!(
                    "do shell script \"{cmd}\" with administrator privileges",
                    cmd = shell_cmd.replace('"', "\\\"")
                );

                let run_result = tokio::task::spawn_blocking(move || {
                    std::process::Command::new("/usr/bin/osascript")
                        .args(["-e", &script])
                        .output()
                })
                .await;

                let reply = match run_result {
                    Ok(Ok(output)) if output.status.success() => {
                        let actual = crate::gpu::apple::read_iogpu_wired_limit_mb();
                        serde_json::json!({
                            "ok": true,
                            "limit_mb": actual,
                            "note": "Applied immediately and saved to /etc/sysctl.conf — will persist across reboots."
                        })
                    }
                    Ok(Ok(output)) => {
                        let stderr = String::from_utf8_lossy(&output.stderr);
                        let stdout = String::from_utf8_lossy(&output.stdout);
                        let combined = format!("{}{}", stdout.trim(), stderr.trim());
                        let msg = if combined.contains("User canceled")
                            || combined.contains("cancelled")
                            || combined.contains("(-128)")
                        {
                            "Cancelled — password dialog was dismissed.".to_string()
                        } else {
                            format!("osascript failed: {combined}")
                        };
                        serde_json::json!({ "ok": false, "error": msg, "manual_cmd": manual_cmd })
                    }
                    Ok(Err(e)) => {
                        serde_json::json!({ "ok": false, "error": format!("Failed to launch osascript: {e}"), "manual_cmd": manual_cmd })
                    }
                    Err(e) => {
                        serde_json::json!({ "ok": false, "error": format!("Task error: {e}"), "manual_cmd": manual_cmd })
                    }
                };

                Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                    warp::reply::json(&reply),
                ))
            }
        })
}

#[cfg(not(target_os = "macos"))]
fn api_set_metal_gpu_limit(
    _state: AppState,
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (impl warp::reply::Reply,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "system" / "set-metal-gpu-limit")
        .and(warp::post())
        .and(warp::header::optional::<String>("authorization"))
        .and(super::safe_json_body::<serde_json::Value>())
        .and_then(move |auth: Option<String>, _body: serde_json::Value| {
            let cfg = app_config.clone();
            async move {
                if !check_api_token(&auth, &cfg) {
                    return Ok(unauthorized_api_token());
                }
                Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(warp::reply::json(
                    &serde_json::json!({
                        "ok": false,
                        "error": "Metal GPU limit tuning is only available on macOS."
                    }),
                )))
            }
        })
}

// ── P3.1: HF Search (with rate limiting) ─────────────────────────────────────
// Rate limit: 10 requests per 60 seconds (global; per-instance).

fn api_hf_search(
    _state: AppState,
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (impl warp::reply::Reply,), Error = warp::Rejection> + Clone {
    // (window_start_secs, request_count) — protected by Mutex to avoid TOCTOU races.
    static HF_SEARCH_RATE: std::sync::LazyLock<std::sync::Mutex<(u64, u64)>> =
        std::sync::LazyLock::new(|| std::sync::Mutex::new((0, 0)));

    warp::path!("api" / "hf" / "search")
        .and(warp::post())
        .and(warp::header::optional::<String>("authorization"))
        .and(super::hf_json_body::<serde_json::Value>())
        .and_then(move |auth: Option<String>, body: serde_json::Value| {
            let cfg = app_config.clone();
            async move {
                if !check_api_token(&auth, &cfg) {
                    return Ok(unauthorized_api_token());
                }

                let now = std::time::SystemTime::UNIX_EPOCH
                    .elapsed()
                    .unwrap_or_default()
                    .as_secs();

                // Check and update rate limit atomically under the Mutex.
                let rate_limited = {
                    let mut guard = HF_SEARCH_RATE.lock().unwrap();
                    let (ref mut window_start, ref mut count) = *guard;
                    if now.saturating_sub(*window_start) >= 60 {
                        *window_start = now;
                        *count = 1;
                        false
                    } else if *count >= 10 {
                        true
                    } else {
                        *count += 1;
                        false
                    }
                };

                if rate_limited {
                    return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(
                        Box::new(warp::reply::with_status(
                            warp::reply::json(&serde_json::json!({
                                "ok": false,
                                "error": "Rate limited: too many HF search requests. Try again in 60 seconds."
                            })),
                            StatusCode::TOO_MANY_REQUESTS,
                        )),
                    );
                }

                let query = body["query"].as_str().unwrap_or("").trim().to_string();
                let author = body["author"].as_str().map(|s| s.trim().to_string()).filter(|s| !s.is_empty());
                let limit: u64 = body["limit"].as_u64().unwrap_or(20).min(100);

                let sort = match body["sort"].as_str().unwrap_or("downloads") {
                    "likes"     => crate::hf::HfSort::Likes,
                    "newest"    | "createdAt" => crate::hf::HfSort::CreatedAt,
                    "trending"  => crate::hf::HfSort::Trending,
                    _           => crate::hf::HfSort::Downloads,
                };

                // Require at least a query or an author — unless sorting by trending or
                // downloads (in which case empty query returns a global popular/trending list).
                if query.is_empty()
                    && author.is_none()
                    && sort != crate::hf::HfSort::Trending
                    && sort != crate::hf::HfSort::Downloads
                {
                    return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(
                        Box::new(warp::reply::json(&serde_json::json!({
                            "ok": false,
                            "error": "Provide 'query' or 'author' (or both)"
                        }))),
                    );
                }

                let params = crate::hf::HfSearchParams { query, author, sort, limit: limit as usize };

                match crate::hf::hf_search_models(&params).await {
                    Ok(models) => Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(
                        Box::new(warp::reply::json(&serde_json::json!({
                            "ok": true,
                            "models": models
                        }))),
                    ),
                    Err(e) => Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(
                        Box::new(warp::reply::json(&serde_json::json!({
                            "ok": false,
                            "error": e
                        }))),
                    ),
                }
            }
        })
}

// ── P3.1: HF Files ───────────────────────────────────────────────────────────

fn api_hf_files(
    _state: AppState,
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (impl warp::reply::Reply,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "hf" / "files")
        .and(warp::post())
        .and(warp::header::optional::<String>("authorization"))
        .and(super::hf_json_body::<serde_json::Value>())
        .and_then(move |auth: Option<String>, body: serde_json::Value| {
            let cfg = app_config.clone();
            async move {
                if !check_api_token(&auth, &cfg) {
                    return Ok(unauthorized_api_token());
                }

                let repo_id = body["repo_id"].as_str().unwrap_or("").trim().to_string();
                if repo_id.is_empty() {
                    return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                        warp::reply::json(&serde_json::json!({
                            "ok": false,
                            "error": "Missing 'repo_id' field"
                        })),
                    ));
                }
                if !validate_hf_repo_id(&repo_id) {
                    return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                        warp::reply::with_status(
                            warp::reply::json(&serde_json::json!({
                                "ok": false,
                                "error": "Invalid repo_id format. Expected: owner/repo"
                            })),
                            StatusCode::BAD_REQUEST,
                        ),
                    ));
                }

                match crate::hf::hf_list_gguf_files(&repo_id).await {
                    Ok(files) => Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                        warp::reply::json(&serde_json::json!({
                            "ok": true,
                            "files": files
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

// ── GET /api/hf/community-picks ───────────────────────────────────────────────
// Reads ~/.config/llama-monitor/community-picks.json if present.
// Produced externally (e.g. by a Hermes cron scraping r/LocalLLaMA).

fn api_hf_community_picks(
    _state: AppState,
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (impl warp::reply::Reply,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "hf" / "community-picks")
        .and(warp::get())
        .and(warp::header::optional::<String>("authorization"))
        .and_then(move |auth: Option<String>| {
            let cfg = app_config.clone();
            async move {
                if !check_api_token(&auth, &cfg) {
                    return Ok(unauthorized_api_token());
                }
                let path = cfg.config_dir.join("community-picks.json");
                let body = if path.exists() {
                    match std::fs::read_to_string(&path) {
                        Ok(s) => match serde_json::from_str::<serde_json::Value>(&s) {
                            Ok(v) => serde_json::json!({ "ok": true, "data": v }),
                            Err(e) => serde_json::json!({
                                "ok": false,
                                "error": format!("community-picks.json parse error: {e}")
                            }),
                        },
                        Err(e) => serde_json::json!({
                            "ok": false,
                            "error": format!("community-picks.json read error: {e}")
                        }),
                    }
                } else {
                    serde_json::json!({ "ok": true, "data": null })
                };
                Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(warp::reply::json(
                    &body,
                )))
            }
        })
}

// ── GET /api/hf/quantizers ────────────────────────────────────────────────────
// Returns the active quantizer list for the wizard quick-picks.
// If hf-quantizers.json exists in config_dir, that list is returned (is_custom=true).
// Otherwise the built-in defaults are returned (is_custom=false).

fn api_hf_quantizers(
    _state: AppState,
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (impl warp::reply::Reply,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "hf" / "quantizers")
        .and(warp::get())
        .and(warp::header::optional::<String>("authorization"))
        .and_then(move |auth: Option<String>| {
            let cfg = app_config.clone();
            async move {
                if !check_api_token(&auth, &cfg) {
                    return Ok(unauthorized_api_token());
                }
                if let Some(user_list) = crate::hf::load_user_quantizers(&cfg.config_dir) {
                    return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(warp::reply::json(
                        &serde_json::json!({ "ok": true, "quantizers": user_list, "is_custom": true }),
                    )));
                }
                let defaults: Vec<crate::hf::UserQuantizer> = crate::hf::known_gguf_quantizers()
                    .iter().map(Into::into).collect();
                Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(warp::reply::json(
                    &serde_json::json!({ "ok": true, "quantizers": defaults, "is_custom": false }),
                )))
            }
        })
}

// ── PUT /api/hf/quantizers ────────────────────────────────────────────────────
// Saves a user-customized quantizer list to hf-quantizers.json.
// Send an empty array to reset to defaults (deletes the file).

fn api_hf_quantizers_put(
    _state: AppState,
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (impl warp::reply::Reply,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "hf" / "quantizers")
        .and(warp::put())
        .and(warp::header::optional::<String>("authorization"))
        .and(
            warp::body::content_length_limit(256 * 1024)
                .and(warp::body::json::<Vec<crate::hf::UserQuantizer>>()),
        )
        .and_then(
            move |auth: Option<String>, body: Vec<crate::hf::UserQuantizer>| {
                let cfg = app_config.clone();
                async move {
                    if !check_api_token(&auth, &cfg) {
                        return Ok(unauthorized_api_token());
                    }
                    // Empty list = reset to defaults (remove user file)
                    if body.is_empty() {
                        let _ = std::fs::remove_file(cfg.config_dir.join("hf-quantizers.json"));
                        return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                            warp::reply::json(&serde_json::json!({ "ok": true, "reset": true })),
                        ));
                    }
                    match crate::hf::save_user_quantizers(&cfg.config_dir, &body) {
                        Ok(()) => Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                            warp::reply::json(&serde_json::json!({ "ok": true })),
                        )),
                        Err(e) => Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                            warp::reply::json(
                                &serde_json::json!({ "ok": false, "error": format!("{e}") }),
                            ),
                        )),
                    }
                }
            },
        )
}

// ── GET /api/hf/download-dir ─────────────────────────────────────────────────
// Returns the directory where HF downloads will be saved (effective models dir).
fn api_hf_download_dir(
    state: AppState,
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "hf" / "download-dir")
        .and(warp::get())
        .and(warp::header::optional::<String>("authorization"))
        .and_then(move |auth: Option<String>| {
            let cfg = app_config.clone();
            let st = state.clone();
            async move {
                if !check_api_token(&auth, &cfg) {
                    return Ok(unauthorized_api_token());
                }
                let dir =
                    get_effective_models_dir(&st).unwrap_or_else(|| cfg.default_models_dir.clone());
                let configured = get_effective_models_dir(&st).is_some();
                Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(warp::reply::json(
                    &serde_json::json!({
                        "ok": true,
                        "dir": dir.to_string_lossy(),
                        "configured": configured
                    }),
                )))
            }
        })
}

// ── GET /api/hf/card?repo=owner/model ─────────────────────────────────────────
// Fetches the raw README.md for a HuggingFace repo and returns it as markdown text.
// Uses the stored HF token if present (required for gated models).
fn api_hf_card(
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "hf" / "card")
        .and(warp::get())
        .and(warp::header::optional::<String>("authorization"))
        .and(warp::query::<std::collections::HashMap<String, String>>())
        .and_then(
            move |auth: Option<String>, params: std::collections::HashMap<String, String>| {
                let cfg = app_config.clone();
                async move {
                    if !check_api_token(&auth, &cfg) {
                        return Ok(unauthorized_api_token());
                    }

                    let repo = match params.get("repo") {
                        Some(r) if !r.is_empty() => r.clone(),
                        _ => {
                            return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                                warp::reply::with_status(
                                    warp::reply::json(&serde_json::json!({
                                        "error": "Missing required query param: repo"
                                    })),
                                    warp::http::StatusCode::BAD_REQUEST,
                                ),
                            ));
                        }
                    };

                    // Basic path-traversal guard: repo must be "owner/name" with no dots or slashes beyond that
                    let parts: Vec<&str> = repo.splitn(3, '/').collect();
                    if parts.len() != 2 || parts.iter().any(|p| p.is_empty() || p.contains("..")) {
                        return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                            warp::reply::with_status(
                                warp::reply::json(
                                    &serde_json::json!({ "error": "Invalid repo id" }),
                                ),
                                warp::http::StatusCode::BAD_REQUEST,
                            ),
                        ));
                    }

                    let url = format!("https://huggingface.co/{}/raw/main/README.md", repo);

                    let builder = reqwest::Client::builder()
                        .timeout(std::time::Duration::from_secs(20))
                        .user_agent("llama-monitor");

                    let client = match builder.build() {
                        Ok(c) => c,
                        Err(e) => {
                            return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                                warp::reply::json(&serde_json::json!({ "error": e.to_string() })),
                            ));
                        }
                    };

                    let mut req = client.get(&url);
                    if let Some(token) = crate::hf::hf_load_token() {
                        req = req.header("Authorization", format!("Bearer {}", token));
                    }

                    let resp = match req.send().await {
                        Ok(r) => r,
                        Err(e) => {
                            return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                                warp::reply::json(&serde_json::json!({ "error": e.to_string() })),
                            ));
                        }
                    };

                    if resp.status() == reqwest::StatusCode::NOT_FOUND {
                        return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                            warp::reply::json(&serde_json::json!({ "markdown": "" })),
                        ));
                    }

                    if !resp.status().is_success() {
                        let status = resp.status().as_u16();
                        return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                            warp::reply::json(&serde_json::json!({
                                "error": format!("HuggingFace returned HTTP {}", status)
                            })),
                        ));
                    }

                    // Cap at 256 KB — large READMEs still render, but we don't buffer unlimited data
                    let bytes = match resp.bytes().await {
                        Ok(b) => b,
                        Err(e) => {
                            return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                                warp::reply::json(&serde_json::json!({ "error": e.to_string() })),
                            ));
                        }
                    };
                    let markdown =
                        String::from_utf8_lossy(&bytes[..bytes.len().min(256 * 1024)]).into_owned();

                    Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(warp::reply::json(
                        &serde_json::json!({ "markdown": markdown }),
                    )))
                }
            },
        )
}

// ── GET /api/hf/token ─────────────────────────────────────────────────────────
// Returns whether an HF token is saved; never returns the token itself.
fn api_hf_token_get(
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "hf" / "token")
        .and(warp::get())
        .and(warp::header::optional::<String>("authorization"))
        .and_then(move |auth: Option<String>| {
            let cfg = app_config.clone();
            async move {
                if !check_api_token(&auth, &cfg) {
                    return Ok(unauthorized_api_token());
                }
                let set = crate::hf::hf_load_token().is_some();
                Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(warp::reply::json(
                    &serde_json::json!({ "set": set }),
                )))
            }
        })
}

// ── PUT /api/hf/token ─────────────────────────────────────────────────────────
// Saves an HF token to ~/.config/llama-monitor/hf-token.
fn api_hf_token_put(
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "hf" / "token")
        .and(warp::put())
        .and(warp::header::optional::<String>("authorization"))
        .and(super::hf_json_body::<serde_json::Value>())
        .and_then(move |auth: Option<String>, body: serde_json::Value| {
            let cfg = app_config.clone();
            async move {
                if !check_api_token(&auth, &cfg) {
                    return Ok(unauthorized_api_token());
                }
                let token = body["token"].as_str().unwrap_or("").trim().to_string();
                if token.is_empty() {
                    return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                        warp::reply::json(
                            &serde_json::json!({ "ok": false, "error": "token is required" }),
                        ),
                    ));
                }
                match crate::hf::hf_save_token(&token) {
                    Ok(()) => Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                        warp::reply::json(&serde_json::json!({ "ok": true })),
                    )),
                    Err(e) => Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                        warp::reply::json(
                            &serde_json::json!({ "ok": false, "error": e.to_string() }),
                        ),
                    )),
                }
            }
        })
}

// ── DELETE /api/hf/token ──────────────────────────────────────────────────────
// Removes the saved HF token file.
fn api_hf_token_delete(
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "hf" / "token")
        .and(warp::delete())
        .and(warp::header::optional::<String>("authorization"))
        .and_then(move |auth: Option<String>| {
            let cfg = app_config.clone();
            async move {
                if !check_api_token(&auth, &cfg) {
                    return Ok(unauthorized_api_token());
                }
                let token_path = cfg.config_dir.join("hf-token");
                if token_path.exists() {
                    let _ = std::fs::remove_file(&token_path);
                }
                Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(warp::reply::json(
                    &serde_json::json!({ "ok": true }),
                )))
            }
        })
}

// ── P3.1: HF Download (with concurrency + cooldown) ──────────────────────────
// - Max 5 concurrent downloads.
// - 10-second cooldown between download starts.

fn api_hf_download(
    state: AppState,
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (impl warp::reply::Reply,), Error = warp::Rejection> + Clone {
    use std::sync::atomic::AtomicU64;
    static HF_DOWNLOAD_LAST_START: AtomicU64 = AtomicU64::new(0);

    warp::path!("api" / "hf" / "download")
        .and(warp::post())
        .and(warp::header::optional::<String>("authorization"))
        .and(super::hf_json_body::<serde_json::Value>())
        .and_then(move |auth: Option<String>, body: serde_json::Value| {
            let cfg = app_config.clone();
            let state = state.clone();
            async move {
                if !check_api_token(&auth, &cfg) {
                    return Ok(unauthorized_api_token());
                }

                let repo_id = body["repo_id"].as_str().unwrap_or("").trim().to_string();
                let file_path = body["file_path"].as_str().unwrap_or("").trim().to_string();
                let target_path: Option<String> =
                    body["target_path"].as_str().map(|s| s.trim().to_string());
                let save_as: Option<String> =
                    body["save_as"].as_str().map(|s| s.trim().to_string());
                let resume: bool = body["resume"].as_bool().unwrap_or(false);
                // Companion downloads (e.g. mmproj alongside a model) bypass the
                // 10-second cooldown so both files can start simultaneously.
                let companion: bool = body["companion"].as_bool().unwrap_or(false);

                if repo_id.is_empty() || file_path.is_empty() {
                    return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                        warp::reply::json(&serde_json::json!({
                            "ok": false,
                            "error": "Missing 'repo_id' or 'file_path'"
                        })),
                    ));
                }
                if !validate_hf_repo_id(&repo_id) {
                    return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                        warp::reply::with_status(
                            warp::reply::json(&serde_json::json!({
                                "ok": false,
                                "error": "Invalid repo_id format. Expected: owner/repo"
                            })),
                            StatusCode::BAD_REQUEST,
                        ),
                    ));
                }

                // Path traversal guard: reject "..", leading "/", leading "\\"
                if file_path.contains("..")
                    || file_path.starts_with('/')
                    || file_path.starts_with("\\")
                {
                    return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                        warp::reply::json(&serde_json::json!({
                            "ok": false,
                            "error": "Invalid file_path: path traversal not allowed"
                        })),
                    ));
                }
                if save_as
                    .as_ref()
                    .is_some_and(|sa| sa.contains("..") || sa.contains('/') || sa.contains('\\'))
                {
                    return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                        warp::reply::json(&serde_json::json!({
                            "ok": false,
                            "error": "Invalid save_as: must be a plain filename"
                        })),
                    ));
                }

                // Determine target directory.
                let models_dir = get_effective_models_dir(&state)
                    .unwrap_or_else(|| cfg.default_models_dir.clone());
                let target_dir = match resolve_hf_target_dir(&models_dir, target_path.as_deref()) {
                    Ok(path) => path,
                    Err(error) => {
                        return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                            warp::reply::json(&serde_json::json!({
                                "ok": false,
                                "error": error
                            })),
                        ));
                    }
                };

                // Cooldown between starts: 10 seconds. Companion downloads (e.g.
                // mmproj alongside a model) are exempt so both can start together.
                if !companion {
                    let now = std::time::SystemTime::UNIX_EPOCH
                        .elapsed()
                        .unwrap_or_default()
                        .as_secs();
                    let (dl_ok, _) = try_cooldown(&HF_DOWNLOAD_LAST_START, now, 10);
                    if !dl_ok {
                        return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                            warp::reply::with_status(
                                warp::reply::json(&serde_json::json!({
                                    "ok": false,
                                    "error": "Too soon; please wait 10 seconds between downloads."
                                })),
                                StatusCode::TOO_MANY_REQUESTS,
                            ),
                        ));
                    }
                }

                let effective_filename = save_as.as_deref().unwrap_or(&file_path);
                let local_path = target_dir
                    .join(effective_filename)
                    .to_string_lossy()
                    .into_owned();
                match crate::hf::hf_start_download(
                    &repo_id,
                    &file_path,
                    save_as.as_deref(),
                    &target_dir,
                    resume,
                ) {
                    Ok(download_id) => Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(
                        Box::new(warp::reply::json(&serde_json::json!({
                            "ok": true,
                            "download_id": download_id,
                            "local_path": local_path
                        }))),
                    ),
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

// ── P3.2: Third-Party Models ─────────────────────────────────────────────────

fn api_third_party_models(
    state: AppState,
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (impl warp::reply::Reply,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "third-party-models")
        .and(warp::post())
        .and(warp::header::optional::<String>("authorization"))
        .and(super::safe_json_body::<serde_json::Value>())
        .and_then(move |auth: Option<String>, body: serde_json::Value| {
            let cfg = app_config.clone();
            let state = state.clone();
            async move {
                if !check_api_token(&auth, &cfg) {
                    return Ok(unauthorized_api_token());
                }

                let _ = body["include_subdirs"].as_bool().unwrap_or(true);

                let extra_dirs = state
                    .ui_settings
                    .lock()
                    .map(|s| s.extra_models_dirs.clone())
                    .unwrap_or_default();
                let models = crate::llama::spawn_wizard::scan_third_party_models(&extra_dirs);
                let models: Vec<serde_json::Value> = models
                    .into_iter()
                    .map(|m| {
                        serde_json::json!({
                            "path": m.path,
                            "name": m.name,
                            "source_tool": m.source_tool,
                            "size": m.size,
                        })
                    })
                    .collect();

                Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(warp::reply::json(
                    &serde_json::json!({
                        "ok": true,
                        "models": models
                    }),
                )))
            }
        })
}

// ── P3.3: Model Introspection ────────────────────────────────────────────────

fn api_model_introspect(
    state: AppState,
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (impl warp::reply::Reply,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "model" / "introspect")
        .and(warp::post())
        .and(warp::header::optional::<String>("authorization"))
        .and(super::safe_json_body::<serde_json::Value>())
        .and_then(move |auth: Option<String>, body: serde_json::Value| {
            let cfg = app_config.clone();
            let state = state.clone();
            async move {
                if !check_api_token(&auth, &cfg) {
                    return Ok(unauthorized_api_token());
                }

                let model_path = body["model_path"].as_str().unwrap_or("").trim().to_string();
                if model_path.is_empty() {
                    return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                        warp::reply::json(&serde_json::json!({
                            "ok": false,
                            "error": "Missing 'model_path' field"
                        })),
                    ));
                }

                // Security: only allow .gguf files or Ollama content-addressed blobs
                // (sha256-<hash> files in a blobs/ directory — valid GGUFs without extension).
                let is_gguf_ext = model_path.to_ascii_lowercase().ends_with(".gguf");
                let is_ollama_blob = model_path.contains("/blobs/sha256-")
                    || model_path.contains("\\blobs\\sha256-");
                if !is_gguf_ext && !is_ollama_blob {
                    return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                        warp::reply::json(&serde_json::json!({
                            "ok": false,
                            "error": "model_path must point to a .gguf file"
                        })),
                    ));
                }
                // Security: resolve the path and confirm it's under an allowed root
                // (home directory or configured models directory).
                let canon = match std::path::Path::new(&model_path).canonicalize() {
                    Ok(p) => p,
                    Err(_) => {
                        return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                            warp::reply::json(&serde_json::json!({
                                "ok": false,
                                "error": "Model file not found"
                            })),
                        ));
                    }
                };
                let models_dir = get_effective_models_dir(&state)
                    .unwrap_or_else(|| cfg.default_models_dir.clone());
                let in_models_dir = models_dir
                    .canonicalize()
                    .map(|d| canon.starts_with(&d))
                    .unwrap_or(false);
                let in_home = dirs::home_dir()
                    .and_then(|h| h.canonicalize().ok())
                    .map(|h| canon.starts_with(&h))
                    .unwrap_or(false);
                let in_extra = state
                    .ui_settings
                    .lock()
                    .map(|s| {
                        s.extra_models_dirs.iter().any(|d| {
                            std::path::Path::new(d)
                                .canonicalize()
                                .map(|cd| canon.starts_with(&cd))
                                .unwrap_or(false)
                        })
                    })
                    .unwrap_or(false);
                if !in_models_dir && !in_home && !in_extra {
                    return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                        warp::reply::json(&serde_json::json!({
                            "ok": false,
                            "error": "model_path is outside allowed directories"
                        })),
                    ));
                }

                let llama_server_path = cfg.llama_server_path.clone();
                // Timeout: 30 seconds.
                let result = tokio::time::timeout(
                    std::time::Duration::from_secs(30),
                    crate::llama::spawn_wizard::introspect_model(
                        &model_path,
                        llama_server_path.to_string_lossy().as_ref(),
                    ),
                )
                .await;

                let metadata = match result {
                    Ok(Ok(meta)) => meta,
                    Ok(Err(e)) => {
                        return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                            warp::reply::json(&serde_json::json!({
                                "ok": false,
                                "error": e
                            })),
                        ));
                    }
                    Err(_) => {
                        return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                            warp::reply::json(&serde_json::json!({
                                "ok": false,
                                "error": "Introspection timed out (30s)"
                            })),
                        ));
                    }
                };

                // Include actual file size so the frontend can use it directly
                // instead of estimating from param count + quant heuristic.
                let file_size_bytes = std::fs::metadata(&model_path).map(|m| m.len()).unwrap_or(0);

                Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(warp::reply::json(
                    &serde_json::json!({
                        "ok": true,
                        "metadata": metadata,
                        "cached": metadata.cached,
                        "file_size_bytes": file_size_bytes
                    }),
                )))
            }
        })
}

pub fn api_routes(
    state: AppState,
    app_config: Arc<AppConfig>,
    auth_manager: AuthManager,
    _bind_host: String,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    let start = api_start(state.clone(), app_config.clone());
    let stop = api_stop(state.clone(), app_config.clone());
    let kill_llama = api_kill_llama(state.clone(), app_config.clone());
    let get_presets = api_get_presets(state.clone(), app_config.clone());
    let create_preset = api_create_preset(state.clone(), app_config.clone());
    let update_preset = api_update_preset(state.clone(), app_config.clone());
    let delete_preset = api_delete_preset(state.clone(), app_config.clone());
    let reset_presets = api_reset_presets(state.clone(), app_config.clone());
    let get_templates = api_get_templates(state.clone(), app_config.clone());
    let create_template = api_create_template(state.clone(), app_config.clone());
    let update_template = api_update_template(state.clone(), app_config.clone());
    let delete_template = api_delete_template(state.clone(), app_config.clone());
    let get_models = api_get_models(state.clone(), app_config.clone());
    let refresh_models = api_refresh_models(state.clone(), app_config.clone());
    let delete_model_file = api_delete_model_file(state.clone(), app_config.clone());
    let get_model_tags = api_get_model_tags(state.clone(), app_config.clone());
    let put_model_tags = api_put_model_tags(state.clone(), app_config.clone());
    let get_gpu_env = api_get_gpu_env(state.clone(), app_config.clone());
    let put_gpu_env = api_put_gpu_env(state.clone(), app_config.clone());
    let get_settings = api_get_settings(state.clone(), app_config.clone());
    let get_settings_full = api_get_settings_full(state.clone(), app_config.clone());
    let put_settings = api_put_settings(state.clone(), app_config.clone());
    let browse = api_browse(state.clone(), app_config.clone());
    let chat = api_chat(state.clone(), app_config.clone());
    let chat_abort = api_chat_abort(state.clone(), app_config.clone());
    let chat_suggestions = api_chat_suggestions(state.clone(), app_config.clone());
    let generate_keywords = api_generate_keywords(state.clone(), app_config.clone());
    let chat_storage = state.chat_storage.clone();
    let chat_list_tabs = api_list_tabs(chat_storage.clone(), app_config.clone());
    let chat_create_tab = api_create_tab(chat_storage.clone(), app_config.clone());
    let chat_get_tab = api_get_tab(chat_storage.clone(), app_config.clone());
    let chat_put_tab = api_put_tab(chat_storage.clone(), app_config.clone());
    let chat_delete_tab = api_delete_tab(chat_storage.clone(), app_config.clone());
    let chat_patch_tab_meta = api_patch_tab_meta(chat_storage.clone(), app_config.clone());
    let chat_append_messages = api_append_messages(chat_storage.clone(), app_config.clone());
    let chat_reorder_tabs = api_reorder_tabs(chat_storage.clone(), app_config.clone());
    let chat_search = api_chat_search(chat_storage.clone(), app_config.clone());
    let chat_archive_tab = api_archive_tab(chat_storage.clone(), app_config.clone());
    let chat_hide_tab = api_hide_tab(chat_storage.clone(), app_config.clone());
    let chat_restore_tab = api_restore_tab(chat_storage.clone(), app_config.clone());

    // Agent token rotation routes
    let rotate_agent_token = api_rotate_agent_token(state.clone(), app_config.clone());
    let rotate_api_token = api_rotate_api_token(app_config.clone());
    let rotate_db_admin_token = api_rotate_db_admin_token(app_config.clone());
    let get_auth_config = api_get_auth_config(app_config.clone(), auth_manager.clone());
    let put_auth_config = api_put_auth_config(app_config.clone(), auth_manager.clone());

    // Database admin routes
    let db_stats = api_db_stats(chat_storage.clone(), app_config.clone());
    let db_integrity = api_db_integrity(chat_storage.clone(), app_config.clone());
    let db_maintenance = api_db_maintenance(chat_storage.clone(), app_config.clone());
    let db_backup = api_db_backup(chat_storage.clone(), app_config.clone());
    let db_delete_backup = api_db_delete_backup(app_config.clone());
    let db_backups = api_db_backups(app_config.clone());
    let db_restore = api_db_restore(chat_storage.clone(), app_config.clone());
    let db_repair = api_db_repair(chat_storage.clone(), app_config.clone());
    let db_indexes = api_db_indexes(chat_storage.clone(), app_config.clone());
    let db_query = api_db_query(chat_storage.clone(), app_config.clone());

    let get_sessions = api_get_sessions(state.clone(), app_config.clone());
    let get_recent_sessions = api_get_recent_sessions(state.clone(), app_config.clone());
    let check_endpoint_health = api_check_endpoint_health(app_config.clone());
    let create_session = api_create_session(state.clone(), app_config.clone());
    let delete_session = api_delete_session(state.clone(), app_config.clone());
    let get_active_session = api_get_active_session(state.clone(), app_config.clone());
    let get_active_session_readiness =
        api_get_active_session_readiness(state.clone(), app_config.clone());
    let set_active_session = api_set_active_session(state.clone(), app_config.clone());
    let get_capabilities = api_get_capabilities(state.clone(), app_config.clone());
    let spawn_session_with_preset =
        api_spawn_session_with_preset(state.clone(), app_config.clone());
    let attach = api_attach(state.clone(), app_config.clone());
    let detach = api_detach(state.clone(), app_config.clone());
    let check_lhm = api_check_lhm(app_config.clone());
    let start_lhm = api_lhm_start(app_config.clone());
    let install_lhm = api_lhm_install(app_config.clone());
    let uninstall_lhm = api_lhm_uninstall(app_config.clone());
    let progress_lhm = api_lhm_progress(app_config.clone());
    let status_lhm = api_lhm_status(app_config.clone());
    let disable_lhm = api_disable_lhm(app_config.clone());
    let remote_agent_latest = api_remote_agent_latest_release(app_config.clone());
    let remote_agent_detect = api_remote_agent_detect(app_config.clone());
    let remote_agent_host_key = api_remote_agent_ssh_host_key(app_config.clone());
    let remote_agent_trust_host = api_remote_agent_ssh_trust(app_config.clone());
    let remote_agent_status = api_remote_agent_status(app_config.clone());
    let remote_agent_remove = api_remote_agent_remove(app_config.clone());
    let remote_agent_tls_status = api_remote_agent_tls_status(app_config.clone());
    let sensor_bridge_status = api_sensor_bridge_status(app_config.clone());
    let sensor_bridge_install = api_sensor_bridge_install(app_config.clone());
    let sensor_bridge_uninstall = api_sensor_bridge_uninstall(app_config.clone());

    // GPU / system metrics routes (used by spawn wizard VRAM estimation)
    let get_gpu_metrics = {
        let state = state.clone();
        let cfg = app_config.clone();
        warp::path!("metrics" / "gpu")
            .and(warp::get())
            .and(warp::header::optional::<String>("authorization"))
            .and_then(move |auth: Option<String>| {
                let state = state.clone();
                let cfg = cfg.clone();
                async move {
                    if !check_api_token(&auth, &cfg) {
                        return Ok(unauthorized_api_token());
                    }
                    let gpu = state
                        .gpu_metrics
                        .lock()
                        .unwrap_or_else(|e| e.into_inner())
                        .clone();
                    Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(warp::reply::json(
                        &gpu,
                    )))
                }
            })
    };
    let get_system_metrics = {
        let state = state.clone();
        let cfg = app_config.clone();
        warp::path!("metrics" / "system")
            .and(warp::get())
            .and(warp::header::optional::<String>("authorization"))
            .and_then(move |auth: Option<String>| {
                let state = state.clone();
                let cfg = cfg.clone();
                async move {
                    if !check_api_token(&auth, &cfg) {
                        return Ok(unauthorized_api_token());
                    }
                    let sys = state
                        .system_metrics
                        .lock()
                        .unwrap_or_else(|e| e.into_inner())
                        .clone();
                    Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(warp::reply::json(
                        &sys,
                    )))
                }
            })
    };
    let get_all_metrics = {
        let state = state.clone();
        let cfg = app_config.clone();
        warp::path!("metrics")
            .and(warp::get())
            .and(warp::header::optional::<String>("authorization"))
            .and_then(move |auth: Option<String>| {
                let state = state.clone();
                let cfg = cfg.clone();
                async move {
                    if !check_api_token(&auth, &cfg) {
                        return Ok(unauthorized_api_token());
                    }
                    let system = state
                        .system_metrics
                        .lock()
                        .unwrap_or_else(|e| e.into_inner())
                        .clone();
                    let gpu = state
                        .gpu_metrics
                        .lock()
                        .unwrap_or_else(|e| e.into_inner())
                        .clone();
                    Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(warp::reply::json(
                        &serde_json::json!({ "system": system, "gpu": gpu }),
                    )))
                }
            })
    };

    // Phase 0/2/3: Spawn Llama-Server v2 routes
    let spawn_wizard_import =
        api_spawn_wizard_import_launch_file(state.clone(), app_config.clone());
    let chat_template_fetch = api_chat_template_fetch(state.clone(), app_config.clone());
    let chat_template_upload = api_chat_template_upload(state.clone(), app_config.clone());
    let chat_template_install_hf = api_chat_template_install_hf(state.clone(), app_config.clone());
    let vram_estimate = api_vram_estimate(state.clone(), app_config.clone());
    let vram_estimate_breakdown = api_vram_estimate_breakdown(state.clone(), app_config.clone());
    let models_download_start = api_models_download_start(state.clone(), app_config.clone());
    let models_download_status = api_models_download_status(state.clone(), app_config.clone());
    let models_download_cancel = api_models_download_cancel(state.clone(), app_config.clone());
    let benchmark_route = api_benchmark(state.clone(), app_config.clone());
    let model_defaults_route = api_model_defaults(state.clone(), app_config.clone());
    let moe_tune_route = api_moe_tune(state.clone(), app_config.clone());
    let hf_search_route = api_hf_search(state.clone(), app_config.clone());
    let hf_files_route = api_hf_files(state.clone(), app_config.clone());
    let hf_download_route = api_hf_download(state.clone(), app_config.clone());
    let hf_quantizers_route = api_hf_quantizers(state.clone(), app_config.clone());
    let hf_quantizers_put_route = api_hf_quantizers_put(state.clone(), app_config.clone());
    let hf_download_dir_route = api_hf_download_dir(state.clone(), app_config.clone());
    let hf_token_get_route = api_hf_token_get(app_config.clone());
    let hf_token_put_route = api_hf_token_put(app_config.clone());
    let hf_token_delete_route = api_hf_token_delete(app_config.clone());
    let hf_card_route = api_hf_card(app_config.clone());
    let hf_community_picks_route = api_hf_community_picks(state.clone(), app_config.clone());
    let third_party_models_route = api_third_party_models(state.clone(), app_config.clone());
    let model_introspect_route = api_model_introspect(state.clone(), app_config.clone());
    let vram_quant_compare_route = api_vram_quant_compare(state.clone(), app_config.clone());
    let vram_auto_size_route = api_vram_auto_size(state.clone(), app_config.clone());
    let set_metal_gpu_limit_route = api_set_metal_gpu_limit(state.clone(), app_config.clone());

    // Group routes to avoid compiler overflow on long .or() chains
    let server_routes = start.or(stop).or(kill_llama).or(attach).or(detach);
    let preset_routes = get_presets
        .or(create_preset)
        .or(update_preset)
        .or(delete_preset)
        .or(reset_presets);
    let template_routes = get_templates
        .or(create_template)
        .or(update_template)
        .or(delete_template);
    let model_routes = get_models
        .or(refresh_models)
        .or(delete_model_file)
        .or(get_model_tags)
        .or(put_model_tags);
    let config_routes = get_gpu_env
        .or(put_gpu_env)
        .or(get_settings_full)
        .or(get_settings)
        .or(put_settings)
        .or(rotate_agent_token)
        .or(rotate_api_token)
        .or(rotate_db_admin_token)
        .or(get_auth_config)
        .or(put_auth_config);
    let analyze_context_notes = api_analyze_context_notes(state.clone(), app_config.clone());
    let chat_routes = browse
        .or(chat)
        .or(chat_abort)
        .or(chat_suggestions)
        .or(generate_keywords)
        .or(analyze_context_notes)
        .or(chat_list_tabs)
        .or(chat_create_tab)
        .or(chat_get_tab)
        .or(chat_put_tab)
        .or(chat_delete_tab)
        .or(chat_patch_tab_meta)
        .or(chat_append_messages)
        .or(chat_reorder_tabs)
        .or(chat_search)
        .or(chat_archive_tab)
        .or(chat_hide_tab)
        .or(chat_restore_tab);
    let db_routes = db_stats
        .or(db_integrity)
        .or(db_maintenance)
        .or(db_backup)
        .or(db_delete_backup)
        .or(db_backups)
        .or(db_restore)
        .or(db_repair)
        .or(db_indexes)
        .or(db_query);
    let session_routes = get_sessions
        .or(get_recent_sessions)
        .or(create_session)
        .or(delete_session)
        .or(get_active_session)
        .or(get_active_session_readiness)
        .or(set_active_session)
        .or(get_capabilities)
        .or(spawn_session_with_preset)
        .or(check_endpoint_health);
    let lhm_routes = check_lhm
        .or(start_lhm)
        .or(progress_lhm)
        .or(status_lhm)
        .or(install_lhm)
        .or(uninstall_lhm)
        .or(disable_lhm);
    let bridge_routes = remote_agent_remove
        .or(remote_agent_tls_status)
        .or(sensor_bridge_status)
        .or(sensor_bridge_install)
        .or(sensor_bridge_uninstall);

    // TLS config routes
    let tls_get_config = api_get_tls_config(state.clone(), app_config.clone());
    let tls_put_config = api_put_tls_config(state.clone(), app_config.clone());
    let tls_acme_request = api_tls_acme_request(state.clone(), app_config.clone());
    let tls_acme_renew = api_tls_acme_renew(state.clone(), app_config.clone());
    let tls_routes = tls_get_config
        .or(tls_put_config)
        .or(tls_acme_request)
        .or(tls_acme_renew);

    // Phase 0/2/3: Spawn Llama-Server v2 route group
    let phase0_routes = spawn_wizard_import
        .or(chat_template_fetch)
        .or(chat_template_upload)
        .or(chat_template_install_hf)
        .or(vram_estimate)
        .or(vram_estimate_breakdown)
        .or(models_download_start)
        .or(models_download_status)
        .or(models_download_cancel)
        .or(benchmark_route)
        .or(model_defaults_route)
        .or(moe_tune_route)
        .or(hf_search_route)
        .or(hf_files_route)
        .or(hf_download_route)
        .or(hf_quantizers_route)
        .or(hf_quantizers_put_route)
        .or(hf_download_dir_route)
        .or(hf_token_get_route)
        .or(hf_token_put_route)
        .or(hf_token_delete_route)
        .or(hf_card_route)
        .or(hf_community_picks_route)
        .or(third_party_models_route)
        .or(model_introspect_route)
        .or(vram_quant_compare_route)
        .or(vram_auto_size_route)
        .or(set_metal_gpu_limit_route);

    let agent_routes = remote_agent_latest
        .or(remote_agent_detect)
        .or(remote_agent_host_key)
        .or(remote_agent_trust_host)
        .or(remote_agent_status)
        .or(api_remote_agent_install(app_config.clone()))
        .or(api_remote_agent_start(app_config.clone()))
        .or(api_remote_agent_update(app_config.clone()))
        .or(api_remote_agent_stop(app_config.clone()));

    let llama_binary_routes = api_llama_binary_version(app_config.clone())
        .or(api_llama_binary_latest(app_config.clone()))
        .or(api_llama_binary_releases(app_config.clone()))
        .or(api_llama_binary_platform_info(app_config.clone()))
        .or(api_llama_binary_update(app_config.clone()));

    server_routes
        .or(get_gpu_metrics)
        .or(get_system_metrics)
        .or(get_all_metrics)
        .or(preset_routes)
        .or(template_routes)
        .or(model_routes)
        .or(config_routes)
        .or(chat_routes)
        .or(db_routes)
        .or(session_routes)
        .or(lhm_routes)
        .or(agent_routes)
        .or(bridge_routes)
        .or(tls_routes)
        .or(llama_binary_routes)
        .or(phase0_routes)
        .or(api_self_update(app_config.clone()))
}

pub fn auth_api_routes(
    auth_manager: AuthManager,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    api_auth_status(auth_manager.clone())
        .or(api_auth_login(auth_manager.clone()))
        .or(api_auth_logout(auth_manager))
}

/// Public token bootstrap routes.
///
/// Exposed before auth_guard so the frontend can retrieve api-token / db-admin-token
/// without needing to be logged in via form/basic auth. Access is still constrained
/// by token_bootstrap_allowed (loopback or no auth) and the caller's Origin.
pub fn public_tokens_routes(
    app_config: Arc<AppConfig>,
    auth_manager: AuthManager,
    bind_host: String,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    api_internal_token(app_config.clone(), auth_manager.clone(), bind_host.clone())
        .or(api_db_admin_token(app_config, auth_manager, bind_host))
}

fn api_auth_status(
    auth_manager: AuthManager,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "auth" / "status")
        .and(warp::get())
        .and(warp::header::optional::<String>("Authorization"))
        .and(warp::header::optional::<String>("cookie"))
        .map(
            move |auth_header: Option<String>, cookie_header: Option<String>| {
                let status = auth_manager.status(auth_header.as_deref(), cookie_header.as_deref());
                let method = match status.method {
                    Some(AuthMethod::Basic) => Some("basic"),
                    Some(AuthMethod::Form) => Some("form"),
                    None => None,
                };
                warp::reply::json(&serde_json::json!({
                    "enabled": auth_manager.has_any(),
                    "methods": {
                        "basic": auth_manager.has_basic(),
                        "form": auth_manager.has_form(),
                    },
                    "managedByCli": matches!(auth_manager.source(), AuthSource::Cli),
                    "recoveryCommand": "llama-monitor --clear-auth-config",
                    "authenticated": status.authenticated,
                    "method": method,
                    "username": status.username,
                }))
            },
        )
}

fn api_auth_login(
    auth_manager: AuthManager,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    #[derive(serde::Deserialize)]
    struct LoginRequest {
        username: String,
        password: String,
    }

    warp::path!("api" / "auth" / "login")
        .and(warp::post())
        .and(warp::body::content_length_limit(32 * 1024))
        .and(warp::body::json())
        .map(move |req: LoginRequest| {
            #[cfg(not(test))]
            {
                use std::sync::atomic::AtomicU64;
                use std::time::{SystemTime, UNIX_EPOCH};

                static LOGIN_LAST_ATTEMPT: AtomicU64 = AtomicU64::new(0);

                let now = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs();
                let (ok, _remaining) = try_cooldown(&LOGIN_LAST_ATTEMPT, now, 10);
                if !ok {
                    return Box::new(warp::reply::with_status(
                        warp::reply::json(&serde_json::json!({
                            "ok": false,
                            "error": "too_many_login_attempts"
                        })),
                        warp::http::StatusCode::TOO_MANY_REQUESTS,
                    )) as Box<dyn warp::reply::Reply>;
                }
            }

            if !auth_manager.has_form() {
                return Box::new(warp::reply::with_status(
                    warp::reply::json(&serde_json::json!({ "error": "form_auth_not_enabled" })),
                    warp::http::StatusCode::BAD_REQUEST,
                )) as Box<dyn warp::reply::Reply>;
            }
            if !auth_manager.verify_form_credentials(&req.username, &req.password) {
                return Box::new(warp::reply::with_status(
                    warp::reply::json(&serde_json::json!({ "error": "invalid_credentials" })),
                    warp::http::StatusCode::UNAUTHORIZED,
                )) as Box<dyn warp::reply::Reply>;
            }
            let Some(token) = auth_manager.create_form_session(&req.username) else {
                return Box::new(warp::reply::with_status(
                    warp::reply::json(&serde_json::json!({ "error": "form_auth_not_enabled" })),
                    warp::http::StatusCode::BAD_REQUEST,
                )) as Box<dyn warp::reply::Reply>;
            };
            Box::new(warp::reply::with_header(
                warp::reply::json(&serde_json::json!({ "ok": true })),
                "Set-Cookie",
                auth_manager.session_cookie_header(&token),
            )) as Box<dyn warp::reply::Reply>
        })
}

fn api_auth_logout(
    auth_manager: AuthManager,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "auth" / "logout")
        .and(warp::post())
        .and(warp::header::optional::<String>("cookie"))
        .map(move |cookie_header: Option<String>| {
            auth_manager.revoke_form_session(cookie_header.as_deref());
            warp::reply::with_header(
                warp::reply::json(&serde_json::json!({ "ok": true })),
                "Set-Cookie",
                auth_manager.expired_session_cookie_header(),
            )
        })
}

fn api_get_auth_config(
    app_config: Arc<AppConfig>,
    auth_manager: AuthManager,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "auth" / "config")
        .and(warp::get())
        .and(warp::header::optional::<String>("authorization"))
        .map(move |auth: Option<String>| {
            let bearer = auth.and_then(|v| v.strip_prefix("Bearer ").map(str::to_string));
            if !bearer_matches_api_token(bearer.as_deref(), &app_config) {
                return Box::new(warp::reply::with_status(
                    warp::reply::json(&serde_json::json!({ "error": "unauthorized" })),
                    warp::http::StatusCode::UNAUTHORIZED,
                )) as Box<dyn warp::reply::Reply>;
            }
            let view = auth_manager.config_view();
            Box::new(warp::reply::json(&serde_json::json!({
                "source": match view.source {
                    AuthSource::None => "none",
                    AuthSource::Config => "config",
                    AuthSource::Cli => "cli",
                },
                "basicEnabled": view.basic_enabled,
                "formEnabled": view.form_enabled,
                "username": view.username,
                "managedByCli": matches!(view.source, AuthSource::Cli),
                "recoveryCommand": "llama-monitor --clear-auth-config",
                "recoveryFile": app_config.auth_config_file.display().to_string(),
            }))) as Box<dyn warp::reply::Reply>
        })
}

fn api_put_auth_config(
    app_config: Arc<AppConfig>,
    auth_manager: AuthManager,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    #[derive(serde::Deserialize)]
    struct UpdateAuthConfigRequest {
        basic_enabled: bool,
        form_enabled: bool,
        username: String,
        #[serde(default)]
        current_password: String,
        #[serde(default)]
        new_password: String,
    }

    warp::path!("api" / "auth" / "config")
        .and(warp::put())
        .and(warp::header::optional::<String>("authorization"))
        .and(warp::body::content_length_limit(64 * 1024))
        .and(warp::body::json())
        .map(move |auth: Option<String>, req: UpdateAuthConfigRequest| {
            let bearer = auth.and_then(|v| v.strip_prefix("Bearer ").map(str::to_string));
            if !bearer_matches_api_token(bearer.as_deref(), &app_config) {
                return Box::new(warp::reply::with_status(
                    warp::reply::json(&serde_json::json!({ "error": "unauthorized" })),
                    warp::http::StatusCode::UNAUTHORIZED,
                )) as Box<dyn warp::reply::Reply>;
            }

            if matches!(auth_manager.source(), AuthSource::Cli) {
                return Box::new(warp::reply::with_status(
                    warp::reply::json(&serde_json::json!({
                        "error": "managed_by_cli",
                        "message": "This instance is using startup auth flags. Remove --basic-auth/--form-auth to manage dashboard access in the app."
                    })),
                    warp::http::StatusCode::CONFLICT,
                )) as Box<dyn warp::reply::Reply>;
            }

            if !req.basic_enabled && !req.form_enabled {
                if let Err(err) = clear_auth_config(&app_config.config_dir) {
                    return Box::new(warp::reply::with_status(
                        warp::reply::json(&serde_json::json!({
                            "error": "save_failed",
                            "message": err.to_string(),
                        })),
                        warp::http::StatusCode::INTERNAL_SERVER_ERROR,
                    )) as Box<dyn warp::reply::Reply>;
                }
                auth_manager.disable();
                return Box::new(warp::reply::json(&serde_json::json!({
                    "ok": true,
                    "message": "Dashboard auth disabled.",
                }))) as Box<dyn warp::reply::Reply>;
            }

            let username = req.username.trim();
            if username.is_empty() {
                return Box::new(warp::reply::with_status(
                    warp::reply::json(&serde_json::json!({
                        "error": "invalid_username",
                        "message": "Username is required when dashboard auth is enabled."
                    })),
                    warp::http::StatusCode::BAD_REQUEST,
                )) as Box<dyn warp::reply::Reply>;
            }

            let existing_view = auth_manager.config_view();
            let changing_password = !req.new_password.trim().is_empty();
            if changing_password && req.new_password.len() < 8 {
                return Box::new(warp::reply::with_status(
                    warp::reply::json(&serde_json::json!({
                        "error": "weak_password",
                        "message": "Use at least 8 characters for the dashboard password."
                    })),
                    warp::http::StatusCode::BAD_REQUEST,
                )) as Box<dyn warp::reply::Reply>;
            }

            if matches!(existing_view.source, AuthSource::Config)
                && existing_view.username.is_some()
                && changing_password
                && !auth_manager.verify_any_password(&req.current_password)
            {
                return Box::new(warp::reply::with_status(
                    warp::reply::json(&serde_json::json!({
                        "error": "invalid_current_password",
                        "message": "Current password did not match the stored dashboard password."
                    })),
                    warp::http::StatusCode::UNAUTHORIZED,
                )) as Box<dyn warp::reply::Reply>;
            }

            let password_hash = if changing_password {
                match AuthManager::hash_password(&req.new_password) {
                    Some(hash) => hash,
                    None => {
                        return Box::new(warp::reply::with_status(
                            warp::reply::json(&serde_json::json!({
                                "error": "hash_failed",
                                "message": "Failed to hash the new password."
                            })),
                            warp::http::StatusCode::INTERNAL_SERVER_ERROR,
                        )) as Box<dyn warp::reply::Reply>;
                    }
                }
            } else {
                let current = crate::config::load_auth_config(&app_config.config_dir);
                if current.password_hash.is_empty() {
                    return Box::new(warp::reply::with_status(
                        warp::reply::json(&serde_json::json!({
                            "error": "missing_password",
                            "message": "Enter a new password to enable dashboard auth."
                        })),
                        warp::http::StatusCode::BAD_REQUEST,
                    )) as Box<dyn warp::reply::Reply>;
                }
                current.password_hash
            };

            let cfg = DashboardAuthConfig {
                basic_enabled: req.basic_enabled,
                form_enabled: req.form_enabled,
                username: username.to_string(),
                password_hash,
            };

            if let Err(err) = save_auth_config(&app_config.config_dir, &cfg) {
                return Box::new(warp::reply::with_status(
                    warp::reply::json(&serde_json::json!({
                        "error": "save_failed",
                        "message": err.to_string(),
                    })),
                    warp::http::StatusCode::INTERNAL_SERVER_ERROR,
                )) as Box<dyn warp::reply::Reply>;
            }

            auth_manager.replace_with_config(cfg);

            Box::new(warp::reply::json(&serde_json::json!({
                "ok": true,
                "message": if changing_password {
                    "Dashboard access updated and sessions refreshed."
                } else {
                    "Dashboard access updated."
                }
            }))) as Box<dyn warp::reply::Reply>
        })
}

fn api_remote_agent_latest_release(
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "remote-agent" / "releases" / "latest")
        .and(warp::get())
        .and(warp::header::optional::<String>("authorization"))
        .and(with_app_config(app_config))
        .and_then(
            move |auth: Option<String>, cfg: Arc<AppConfig>| async move {
                let bearer = extract_bearer(auth);
                if !bearer_matches_api_token(bearer.as_deref(), &cfg) {
                    return Ok(unauthorized_api_token());
                }

                // latest_release_info() has its own 60-second in-memory cache, so
                // rapid re-calls (e.g. reopening the setup modal) are served from
                // cache without hitting GitHub. A separate API-level rate limiter
                // was redundant and caused "Unavailable" when the modal was
                // reopened within 30 seconds.
                match crate::agent::latest_release_info().await {
                    Ok(release) => Ok::<_, warp::Rejection>(Box::new(warp::reply::json(
                        &serde_json::json!({"ok": true, "release": release}),
                    ))
                        as Box<dyn warp::reply::Reply>),
                    Err(e) => Ok::<_, warp::Rejection>(Box::new(warp::reply::json(
                        &serde_json::json!({"ok": false, "error": e.to_string()}),
                    ))
                        as Box<dyn warp::reply::Reply>),
                }
            },
        )
}

fn api_remote_agent_detect(
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    static LAST_REMOTE_AGENT_DETECT: AtomicU64 = AtomicU64::new(0);

    warp::path!("api" / "remote-agent" / "detect")
        .and(warp::post())
        .and(warp::header::optional::<String>("authorization"))
        .and(warp::body::json())
        .and_then(
            move |auth: Option<String>, mut request: crate::agent::RemoteAgentDetectRequest| {
                let app_config = app_config.clone();
                async move {
                    let bearer = extract_bearer(auth);
                    if !bearer_matches_api_token(bearer.as_deref(), &app_config) {
                        return Ok(unauthorized_api_token());
                    }

                    let now = SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs();
                    let last = LAST_REMOTE_AGENT_DETECT.load(Ordering::Acquire);
                    if now - last < 10 {
                        let remaining = 10 - (now - last);
                        return Ok::<_, warp::Rejection>(Box::new(warp::reply::with_status(
                            warp::reply::json(&serde_json::json!({
                                "ok": false,
                                "error": "too soon; please wait",
                                "seconds_remaining": remaining
                            })),
                            warp::http::StatusCode::TOO_MANY_REQUESTS,
                        ))
                            as Box<dyn warp::reply::Reply>);
                    }
                    LAST_REMOTE_AGENT_DETECT.store(now, Ordering::Release);

                    match hydrate_ssh_connection(
                        request.ssh_connection.take(),
                        &request.ssh_target,
                        &app_config,
                    ) {
                        Ok(connection) => request.ssh_connection = Some(connection),
                        Err(e) => {
                            return Ok::<_, warp::Rejection>(Box::new(warp::reply::json(
                                &serde_json::json!({"ok": false, "error": e.to_string()}),
                            ))
                                as Box<dyn warp::reply::Reply>);
                        }
                    }
                    let response = crate::agent::detect_remote_agent(request).await;
                    Ok::<_, warp::Rejection>(
                        Box::new(warp::reply::json(&response)) as Box<dyn warp::reply::Reply>
                    )
                }
            },
        )
}

fn api_remote_agent_ssh_host_key(
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    static LAST_REMOTE_AGENT_SSH_HOST_KEY: AtomicU64 = AtomicU64::new(0);

    warp::path!("api" / "remote-agent" / "ssh" / "host-key")
        .and(warp::post())
        .and(warp::header::optional::<String>("authorization"))
        .and(warp::body::json())
        .and_then(
            move |auth: Option<String>, request: serde_json::Map<String, serde_json::Value>| {
                let app_config = app_config.clone();
                async move {
                    let bearer = extract_bearer(auth);
                    if !bearer_matches_api_token(bearer.as_deref(), &app_config) {
                        return Ok(unauthorized_api_token());
                    }

                    let now = SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs();
                    let last = LAST_REMOTE_AGENT_SSH_HOST_KEY.load(Ordering::Acquire);
                    if now - last < 10 {
                        let remaining = 10 - (now - last);
                        return Ok::<_, warp::Rejection>(Box::new(warp::reply::with_status(
                            warp::reply::json(&serde_json::json!({
                                "ok": false,
                                "error": "too soon; please wait",
                                "seconds_remaining": remaining
                            })),
                            warp::http::StatusCode::TOO_MANY_REQUESTS,
                        ))
                            as Box<dyn warp::reply::Reply>);
                    }
                    LAST_REMOTE_AGENT_SSH_HOST_KEY.store(now, Ordering::Release);

                    let target = request
                        .get("ssh_target")
                        .and_then(|value| value.as_str())
                        .unwrap_or("");
                    let connection = ssh_connection_from_request(&request, target);
                    match remote_ssh::scan_host_key(
                        connection,
                        app_config.ssh_known_hosts_file.clone(),
                    )
                    .await
                    {
                        Ok(info) => Ok::<_, warp::Rejection>(Box::new(warp::reply::json(
                            &serde_json::json!({"ok": true, "host_key": info}),
                        ))
                            as Box<dyn warp::reply::Reply>),
                        Err(e) => Ok::<_, warp::Rejection>(Box::new(warp::reply::json(
                            &serde_json::json!({"ok": false, "error": e.to_string()}),
                        ))
                            as Box<dyn warp::reply::Reply>),
                    }
                }
            },
        )
}

fn api_remote_agent_ssh_trust(
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    static LAST_REMOTE_AGENT_SSH_TRUST: AtomicU64 = AtomicU64::new(0);

    warp::path!("api" / "remote-agent" / "ssh" / "trust")
        .and(warp::post())
        .and(warp::header::optional::<String>("authorization"))
        .and(warp::body::json())
        .and_then(move |auth: Option<String>, request: serde_json::Map<String, serde_json::Value>| {
            let app_config = app_config.clone();
            async move {
                let bearer = extract_bearer(auth);
                if !bearer_matches_api_token(bearer.as_deref(), &app_config) {
                    return Ok(unauthorized_api_token());
                }

                let now = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs();
                let last = LAST_REMOTE_AGENT_SSH_TRUST.load(Ordering::Acquire);
                if now - last < 10 {
                    let remaining = 10 - (now - last);
                    return Ok::<_, warp::Rejection>(Box::new(warp::reply::with_status(
                        warp::reply::json(&serde_json::json!({
                            "ok": false,
                            "error": "too soon; please wait",
                            "seconds_remaining": remaining
                        })),
                        warp::http::StatusCode::TOO_MANY_REQUESTS,
                    )) as Box<dyn warp::reply::Reply>);
                }
                LAST_REMOTE_AGENT_SSH_TRUST.store(now, Ordering::Release);

                let target = request
                    .get("ssh_target")
                    .and_then(|value| value.as_str())
                    .unwrap_or("");
                let key_hex = request
                    .get("key_hex")
                    .and_then(|value| value.as_str())
                    .unwrap_or("");
                let connection = ssh_connection_from_request(&request, target);
                match remote_ssh::scan_host_key(
                    connection.clone(),
                    app_config.ssh_known_hosts_file.clone(),
                )
                .await
                {
                    Ok(info) if info.key_hex == key_hex.trim().to_ascii_lowercase() => {}
                    Ok(_) => {
                        return Ok::<_, warp::Rejection>(Box::new(warp::reply::json(
                            &serde_json::json!({"ok": false, "error": "Host key changed between scan and trust confirmation"}),
                        )) as Box<dyn warp::reply::Reply>);
                    }
                    Err(e) => {
                        return Ok::<_, warp::Rejection>(Box::new(warp::reply::json(
                            &serde_json::json!({"ok": false, "error": e.to_string()}),
                        )) as Box<dyn warp::reply::Reply>);
                    }
                }
                match remote_ssh::trust_host_key(
                    &app_config.ssh_known_hosts_file,
                    &connection,
                    key_hex,
                ) {
                    Ok(()) => Ok::<_, warp::Rejection>(Box::new(warp::reply::json(
                        &serde_json::json!({"ok": true}),
                    )) as Box<dyn warp::reply::Reply>),
                    Err(e) => Ok::<_, warp::Rejection>(Box::new(warp::reply::json(
                        &serde_json::json!({"ok": false, "error": e.to_string()}),
                    )) as Box<dyn warp::reply::Reply>),
                }
            }
        })
}

fn ssh_connection_from_request(
    request: &serde_json::Map<String, serde_json::Value>,
    target: &str,
) -> SshConnection {
    request
        .get("ssh_connection")
        .and_then(|value| serde_json::from_value(value.clone()).ok())
        .unwrap_or_else(|| SshConnection::from_target(target))
}

fn hydrate_ssh_connection(
    connection: Option<SshConnection>,
    target: &str,
    app_config: &AppConfig,
) -> anyhow::Result<SshConnection> {
    let connection = connection.unwrap_or_else(|| SshConnection::from_target(target));
    remote_ssh::with_trusted_host_key(connection, &app_config.ssh_known_hosts_file)
}

fn api_remote_agent_install(
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    static LAST_REMOTE_AGENT_INSTALL: AtomicU64 = AtomicU64::new(0);

    warp::path!("api" / "remote-agent" / "install")
        .and(warp::post())
        .and(warp::header::optional::<String>("authorization"))
        .and(warp::body::json())
        .and_then(
            move |auth: Option<String>, mut request: crate::agent::RemoteAgentInstallRequest| {
                let app_config = app_config.clone();
                async move {
                    let bearer = extract_bearer(auth);
                    if !bearer_matches_db_admin_token(bearer.as_deref(), &app_config) {
                        return Ok(unauthorized_db_admin_token());
                    }

                    let now = SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs();
                    let last = LAST_REMOTE_AGENT_INSTALL.load(Ordering::Acquire);
                    if now - last < 30 {
                        let remaining = 30 - (now - last);
                        return Ok::<_, warp::Rejection>(Box::new(warp::reply::with_status(
                            warp::reply::json(&serde_json::json!({
                                "ok": false,
                                "error": "too soon; please wait",
                                "seconds_remaining": remaining
                            })),
                            warp::http::StatusCode::TOO_MANY_REQUESTS,
                        ))
                            as Box<dyn warp::reply::Reply>);
                    }
                    LAST_REMOTE_AGENT_INSTALL.store(now, Ordering::Release);

                    crate::agent::suppress_remote_agent_autostart();
                    request.ssh_connection = match hydrate_ssh_connection(
                        request.ssh_connection.take(),
                        &request.ssh_target,
                        &app_config,
                    ) {
                        Ok(connection) => Some(connection),
                        Err(e) => {
                            return Ok::<_, warp::Rejection>(Box::new(warp::reply::json(
                                &serde_json::json!({"ok": false, "error": e.to_string()}),
                            ))
                                as Box<dyn warp::reply::Reply>);
                        }
                    };
                    let remote_os = if let Some(connection) = request.ssh_connection.clone() {
                        crate::agent::detect_remote_os_for_connection(connection).await
                    } else {
                        crate::agent::detect_remote_os_simple(&request.ssh_target).await
                    };
                    let api_token = app_config.live_api_token();
                    match crate::agent::install_remote_agent(
                        request.ssh_target.trim(),
                        request.ssh_connection.clone(),
                        &request.asset,
                        request.install_path.clone(),
                        remote_os,
                        api_token,
                    )
                    .await
                    {
                        Ok(response) => {
                            Ok::<_, warp::Rejection>(Box::new(warp::reply::json(&response))
                                as Box<dyn warp::reply::Reply>)
                        }
                        Err(e) => Ok::<_, warp::Rejection>(Box::new(warp::reply::json(
                            &serde_json::json!({"ok": false, "error": e.to_string()}),
                        ))
                            as Box<dyn warp::reply::Reply>),
                    }
                }
            },
        )
}

fn api_remote_agent_status(
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    static LAST_REMOTE_AGENT_STATUS: AtomicU64 = AtomicU64::new(0);

    warp::path!("api" / "remote-agent" / "status")
        .and(warp::post())
        .and(warp::header::optional::<String>("authorization"))
        .and(warp::body::json())
        .and_then(
            move |auth: Option<String>, request: serde_json::Map<String, serde_json::Value>| {
                let app_config = app_config.clone();
                async move {
                    let bearer = extract_bearer(auth);
                    if !bearer_matches_api_token(bearer.as_deref(), &app_config) {
                        return Ok(unauthorized_api_token());
                    }

                    let now = SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs();
                    let last = LAST_REMOTE_AGENT_STATUS.load(Ordering::Acquire);
                    if now - last < 5 {
                        let remaining = 5 - (now - last);
                        return Ok::<_, warp::Rejection>(Box::new(warp::reply::with_status(
                            warp::reply::json(&serde_json::json!({
                                "ok": false,
                                "error": "too soon; please wait",
                                "seconds_remaining": remaining
                            })),
                            warp::http::StatusCode::TOO_MANY_REQUESTS,
                        ))
                            as Box<dyn warp::reply::Reply>);
                    }
                    LAST_REMOTE_AGENT_STATUS.store(now, Ordering::Release);

                    let ssh_target = match request.get("ssh_target") {
                        Some(v) => v.as_str().unwrap_or("").to_string(),
                        None => {
                            return Ok::<_, warp::Rejection>(Box::new(warp::reply::json(
                                &serde_json::json!({"ok": false, "error": "Missing ssh_target"}),
                            ))
                                as Box<dyn warp::reply::Reply>);
                        }
                    };
                    let ssh_connection = match hydrate_ssh_connection(
                        request
                            .get("ssh_connection")
                            .and_then(|value| serde_json::from_value(value.clone()).ok()),
                        &ssh_target,
                        &app_config,
                    ) {
                        Ok(connection) => Some(connection),
                        Err(e) => {
                            return Ok::<_, warp::Rejection>(Box::new(warp::reply::json(
                                &serde_json::json!({"ok": false, "error": e.to_string()}),
                            ))
                                as Box<dyn warp::reply::Reply>);
                        }
                    };
                    match crate::agent::status_remote_agent(&ssh_target, ssh_connection).await {
                        Ok(response) => {
                            Ok::<_, warp::Rejection>(Box::new(warp::reply::json(&response))
                                as Box<dyn warp::reply::Reply>)
                        }
                        Err(e) => Ok::<_, warp::Rejection>(Box::new(warp::reply::json(
                            &serde_json::json!({"ok": false, "error": e.to_string()}),
                        ))
                            as Box<dyn warp::reply::Reply>),
                    }
                }
            },
        )
}

fn api_remote_agent_start(
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    static LAST_REMOTE_AGENT_START: AtomicU64 = AtomicU64::new(0);

    warp::path!("api" / "remote-agent" / "start")
        .and(warp::post())
        .and(warp::header::optional::<String>("authorization"))
        .and(warp::body::json())
        .and_then(
            move |auth: Option<String>, request: serde_json::Map<String, serde_json::Value>| {
                let app_config = app_config.clone();
                async move {
                    let bearer = extract_bearer(auth);
                    if !bearer_matches_api_token(bearer.as_deref(), &app_config) {
                        return Ok(unauthorized_api_token());
                    }

                    let now = SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs();
                    let last = LAST_REMOTE_AGENT_START.load(Ordering::Acquire);
                    if now - last < 10 {
                        let remaining = 10 - (now - last);
                        return Ok::<_, warp::Rejection>(Box::new(warp::reply::with_status(
                            warp::reply::json(&serde_json::json!({
                                "ok": false,
                                "error": "too soon; please wait",
                                "seconds_remaining": remaining
                            })),
                            warp::http::StatusCode::TOO_MANY_REQUESTS,
                        ))
                            as Box<dyn warp::reply::Reply>);
                    }
                    LAST_REMOTE_AGENT_START.store(now, Ordering::Release);

                    crate::agent::suppress_remote_agent_autostart();
                    let ssh_target = match request.get("ssh_target") {
                        Some(v) => v.as_str().unwrap_or("").to_string(),
                        None => {
                            return Ok::<_, warp::Rejection>(Box::new(warp::reply::json(
                                &serde_json::json!({"ok": false, "error": "Missing ssh_target"}),
                            ))
                                as Box<dyn warp::reply::Reply>);
                        }
                    };
                    // Hydrate the SSH connection before resolving install_path so that
                    // the OS detection fallback uses an authenticated connection rather
                    // than a bare target string (which fails auth → Unknown OS → wrong path).
                    let ssh_connection = match hydrate_ssh_connection(
                        request
                            .get("ssh_connection")
                            .and_then(|value| serde_json::from_value(value.clone()).ok()),
                        &ssh_target,
                        &app_config,
                    ) {
                        Ok(connection) => Some(connection),
                        Err(e) => {
                            return Ok::<_, warp::Rejection>(Box::new(warp::reply::json(
                                &serde_json::json!({"ok": false, "error": e.to_string()}),
                            ))
                                as Box<dyn warp::reply::Reply>);
                        }
                    };
                    // Detect OS once using the hydrated connection and reuse for
                    // both install_path resolution and command generation.
                    let remote_os = if let Some(ref conn) = ssh_connection {
                        crate::agent::detect_remote_os_with(conn).await
                    } else {
                        crate::agent::detect_remote_os_simple(&ssh_target).await
                    };
                    let install_path = match request.get("install_path").and_then(|v| v.as_str()) {
                        Some(p) if !p.is_empty() => p.to_string(),
                        _ => crate::agent::default_install_path_for_os(remote_os),
                    };
                    let command = if let Some(ref conn) = ssh_connection {
                        crate::agent::default_start_command_for_os_with(
                            conn,
                            remote_os,
                            &install_path,
                        )
                        .await
                    } else {
                        match request.get("start_command") {
                            Some(v) => {
                                let cmd = v.as_str().unwrap_or("").to_string();
                                if crate::agent::validate_remote_command(&cmd) {
                                    cmd
                                } else {
                                    crate::agent::default_start_command_for_target(
                                        &ssh_target,
                                        &install_path,
                                    )
                                    .await
                                }
                            }
                            None => {
                                crate::agent::default_start_command_for_target(
                                    &ssh_target,
                                    &install_path,
                                )
                                .await
                            }
                        }
                    };
                    match crate::agent::start_remote_agent(
                        &ssh_target,
                        ssh_connection,
                        &install_path,
                        &command,
                    )
                    .await
                    {
                        Ok(response) => {
                            Ok::<_, warp::Rejection>(Box::new(warp::reply::json(&response))
                                as Box<dyn warp::reply::Reply>)
                        }
                        Err(e) => Ok::<_, warp::Rejection>(Box::new(warp::reply::json(
                            &serde_json::json!({"ok": false, "error": e.to_string()}),
                        ))
                            as Box<dyn warp::reply::Reply>),
                    }
                }
            },
        )
}

fn api_remote_agent_update(
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    static LAST_REMOTE_AGENT_UPDATE: AtomicU64 = AtomicU64::new(0);

    warp::path!("api" / "remote-agent" / "update")
        .and(warp::post())
        .and(warp::header::optional::<String>("authorization"))
        .and(warp::body::json())
        .and_then(
            move |auth: Option<String>, request: serde_json::Map<String, serde_json::Value>| {
                let app_config = app_config.clone();
                async move {
                    let bearer = extract_bearer(auth);
                    if !bearer_matches_api_token(bearer.as_deref(), &app_config) {
                        return Ok(unauthorized_api_token());
                    }

                    let now = SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs();
                    let last = LAST_REMOTE_AGENT_UPDATE.load(Ordering::Acquire);
                    if now - last < 30 {
                        let remaining = 30 - (now - last);
                        return Ok::<_, warp::Rejection>(Box::new(warp::reply::with_status(
                            warp::reply::json(&serde_json::json!({
                                "ok": false,
                                "error": "too soon; please wait",
                                "seconds_remaining": remaining
                            })),
                            warp::http::StatusCode::TOO_MANY_REQUESTS,
                        ))
                            as Box<dyn warp::reply::Reply>);
                    }
                    LAST_REMOTE_AGENT_UPDATE.store(now, Ordering::Release);

                    crate::agent::suppress_remote_agent_autostart();
                    let ssh_target = match request.get("ssh_target") {
                        Some(v) => v.as_str().unwrap_or("").to_string(),
                        None => {
                            return Ok::<_, warp::Rejection>(Box::new(warp::reply::json(
                                &serde_json::json!({"ok": false, "error": "Missing ssh_target"}),
                            ))
                                as Box<dyn warp::reply::Reply>);
                        }
                    };
                    let ssh_connection = match hydrate_ssh_connection(
                        request
                            .get("ssh_connection")
                            .and_then(|value| serde_json::from_value(value.clone()).ok()),
                        &ssh_target,
                        &app_config,
                    ) {
                        Ok(connection) => Some(connection),
                        Err(e) => {
                            return Ok::<_, warp::Rejection>(Box::new(warp::reply::json(
                                &serde_json::json!({"ok": false, "error": e.to_string()}),
                            ))
                                as Box<dyn warp::reply::Reply>);
                        }
                    };
                    match crate::agent::update_remote_agent(&ssh_target, ssh_connection).await {
                        Ok(response) => {
                            Ok::<_, warp::Rejection>(Box::new(warp::reply::json(&response))
                                as Box<dyn warp::reply::Reply>)
                        }
                        Err(e) => Ok::<_, warp::Rejection>(Box::new(warp::reply::json(
                            &serde_json::json!({"ok": false, "error": e.to_string()}),
                        ))
                            as Box<dyn warp::reply::Reply>),
                    }
                }
            },
        )
}

fn api_remote_agent_stop(
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    static LAST_REMOTE_AGENT_STOP: AtomicU64 = AtomicU64::new(0);

    warp::path!("api" / "remote-agent" / "stop")
        .and(warp::post())
        .and(warp::header::optional::<String>("authorization"))
        .and(warp::body::json())
        .and_then(
            move |auth: Option<String>, request: serde_json::Map<String, serde_json::Value>| {
                let app_config = app_config.clone();
                async move {
                    let bearer = extract_bearer(auth);
                    if !bearer_matches_api_token(bearer.as_deref(), &app_config) {
                        return Ok(unauthorized_api_token());
                    }

                    let now = SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs();
                    let last = LAST_REMOTE_AGENT_STOP.load(Ordering::Acquire);
                    if now - last < 10 {
                        let remaining = 10 - (now - last);
                        return Ok::<_, warp::Rejection>(Box::new(warp::reply::with_status(
                            warp::reply::json(&serde_json::json!({
                                "ok": false,
                                "error": "too soon; please wait",
                                "seconds_remaining": remaining
                            })),
                            warp::http::StatusCode::TOO_MANY_REQUESTS,
                        ))
                            as Box<dyn warp::reply::Reply>);
                    }
                    LAST_REMOTE_AGENT_STOP.store(now, Ordering::Release);

                    crate::agent::suppress_remote_agent_autostart();
                    let ssh_target = match request.get("ssh_target") {
                        Some(v) => v.as_str().unwrap_or("").to_string(),
                        None => {
                            return Ok::<_, warp::Rejection>(Box::new(warp::reply::json(
                                &serde_json::json!({"ok": false, "error": "Missing ssh_target"}),
                            ))
                                as Box<dyn warp::reply::Reply>);
                        }
                    };
                    let ssh_connection = match hydrate_ssh_connection(
                        request
                            .get("ssh_connection")
                            .and_then(|value| serde_json::from_value(value.clone()).ok()),
                        &ssh_target,
                        &app_config,
                    ) {
                        Ok(connection) => Some(connection),
                        Err(e) => {
                            return Ok::<_, warp::Rejection>(Box::new(warp::reply::json(
                                &serde_json::json!({"ok": false, "error": e.to_string()}),
                            ))
                                as Box<dyn warp::reply::Reply>);
                        }
                    };
                    match crate::agent::stop_remote_agent(&ssh_target, ssh_connection).await {
                        Ok(response) => {
                            Ok::<_, warp::Rejection>(Box::new(warp::reply::json(&response))
                                as Box<dyn warp::reply::Reply>)
                        }
                        Err(e) => Ok::<_, warp::Rejection>(Box::new(warp::reply::json(
                            &serde_json::json!({"ok": false, "error": e.to_string()}),
                        ))
                            as Box<dyn warp::reply::Reply>),
                    }
                }
            },
        )
}

fn api_remote_agent_remove(
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    static LAST_REMOTE_AGENT_REMOVE: AtomicU64 = AtomicU64::new(0);

    warp::path!("api" / "remote-agent" / "remove")
        .and(warp::post())
        .and(warp::header::optional::<String>("authorization"))
        .and(warp::body::json())
        .and_then(
            move |auth: Option<String>, request: serde_json::Map<String, serde_json::Value>| {
                let app_config = app_config.clone();
                async move {
                    let bearer = extract_bearer(auth);
                    if !bearer_matches_db_admin_token(bearer.as_deref(), &app_config) {
                        return Ok(unauthorized_db_admin_token());
                    }

                    let now = SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs();
                    let last = LAST_REMOTE_AGENT_REMOVE.load(Ordering::Acquire);
                    if now - last < 15 {
                        let remaining = 15 - (now - last);
                        return Ok::<_, warp::Rejection>(Box::new(warp::reply::with_status(
                            warp::reply::json(&serde_json::json!({
                                "ok": false,
                                "error": "too soon; please wait",
                                "seconds_remaining": remaining
                            })),
                            warp::http::StatusCode::TOO_MANY_REQUESTS,
                        ))
                            as Box<dyn warp::reply::Reply>);
                    }
                    LAST_REMOTE_AGENT_REMOVE.store(now, Ordering::Release);

                    crate::agent::suppress_remote_agent_autostart();
                    let ssh_target = match request.get("ssh_target") {
                        Some(v) => v.as_str().unwrap_or("").to_string(),
                        None => {
                            return Ok::<_, warp::Rejection>(Box::new(warp::reply::json(
                                &serde_json::json!({"ok": false, "error": "Missing ssh_target"}),
                            ))
                                as Box<dyn warp::reply::Reply>);
                        }
                    };
                    let ssh_connection = match hydrate_ssh_connection(
                        request
                            .get("ssh_connection")
                            .and_then(|value| serde_json::from_value(value.clone()).ok()),
                        &ssh_target,
                        &app_config,
                    ) {
                        Ok(connection) => Some(connection),
                        Err(e) => {
                            return Ok::<_, warp::Rejection>(Box::new(warp::reply::json(
                                &serde_json::json!({"ok": false, "error": e.to_string()}),
                            ))
                                as Box<dyn warp::reply::Reply>);
                        }
                    };
                    match crate::agent::remove_remote_agent(&ssh_target, ssh_connection).await {
                        Ok(response) => {
                            Ok::<_, warp::Rejection>(Box::new(warp::reply::json(&response))
                                as Box<dyn warp::reply::Reply>)
                        }
                        Err(e) => Ok::<_, warp::Rejection>(Box::new(warp::reply::json(
                            &serde_json::json!({"ok": false, "error": e.to_string()}),
                        ))
                            as Box<dyn warp::reply::Reply>),
                    }
                }
            },
        )
}

fn api_remote_agent_tls_status(
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "remote-agent" / "tls-status")
        .and(warp::get())
        .and(warp::header::optional::<String>("authorization"))
        .map(move |auth: Option<String>| {
            let bearer = extract_bearer(auth);
            if !bearer_matches_api_token(bearer.as_deref(), &app_config) {
                return unauthorized_api_token();
            }
            let certs_dir = crate::certs::certs_dir();
            let ca_present = certs_dir.join("ca.pem").exists();
            let server_present = certs_dir.join("agent-server.pem").exists();
            let client_present = certs_dir.join("agent-client.pem").exists();
            Box::new(warp::reply::json(&serde_json::json!({
                "mtls_enforced": true,
                "ca_present": ca_present,
                "server_cert_present": server_present,
                "client_cert_present": client_present,
            }))) as Box<dyn warp::reply::Reply>
        })
}

fn api_start(
    state: AppState,
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "start")
        .and(warp::post())
        .and(warp::header::optional::<String>("authorization"))
        .and(warp::body::json())
        .and_then(move |auth: Option<String>, config: ServerConfig| {
            let state = state.clone();
            let app_config = app_config.clone();
            async move {
                if !check_api_token(&auth, &app_config) {
                    return Ok(unauthorized_api_token());
                }
                let ui = state.ui_settings.lock().unwrap().clone();
                let mut eff_config = (*app_config).clone();
                if !ui.llama_server_path.is_empty() {
                    eff_config.llama_server_path = PathBuf::from(&ui.llama_server_path);
                }
                if !ui.llama_server_cwd.is_empty() {
                    eff_config.llama_server_cwd = PathBuf::from(&ui.llama_server_cwd);
                }
                match server::start_server(&state, config, &eff_config).await {
                    Ok(()) => Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                        warp::reply::json(&serde_json::json!({"ok": true})),
                    )),
                    Err(e) => Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                        warp::reply::json(
                            &serde_json::json!({"ok": false, "error": e.to_string()}),
                        ),
                    )),
                }
            }
        })
}

fn api_stop(
    state: AppState,
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "stop")
        .and(warp::post())
        .and(warp::header::optional::<String>("authorization"))
        .and_then(move |auth: Option<String>| {
            let state = state.clone();
            let app_config = app_config.clone();
            async move {
                if !check_api_token(&auth, &app_config) {
                    return Ok(unauthorized_api_token());
                }
                match server::stop_server(&state).await {
                    Ok(()) => Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                        warp::reply::json(&serde_json::json!({"ok": true})),
                    )),
                    Err(e) => Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                        warp::reply::json(
                            &serde_json::json!({"ok": false, "error": e.to_string()}),
                        ),
                    )),
                }
            }
        })
}

fn api_get_presets(
    state: AppState,
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "presets")
        .and(warp::get())
        .and(warp::header::optional::<String>("authorization"))
        .and(with_app_config(app_config))
        .and_then(move |auth: Option<String>, cfg: Arc<AppConfig>| {
            let presets = state.presets.lock().unwrap().clone();
            if !check_api_token(&auth, &cfg) {
                return futures_util::future::ready(Ok(unauthorized_api_token()));
            }
            futures_util::future::ready(Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(
                Box::new(warp::reply::json(&presets)),
            ))
        })
}

fn api_create_preset(
    state: AppState,
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "presets")
        .and(warp::post())
        .and(warp::header::optional::<String>("authorization"))
        .and(warp::body::json())
        .and_then(move |auth: Option<String>, preset: ModelPreset| {
            let cfg = app_config.clone();
            if !check_api_token(&auth, &cfg) {
                return futures_util::future::ready(Ok(unauthorized_api_token()));
            }
            let mut presets = state.presets.lock().unwrap();
            presets.push(preset.clone());
            let _ = presets::save_presets(&state.presets_path, &presets);
            futures_util::future::ready(Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(
                Box::new(warp::reply::json(
                    &serde_json::json!({"ok": true, "preset": preset}),
                )),
            ))
        })
}

fn api_update_preset(
    state: AppState,
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "presets" / String)
        .and(warp::put())
        .and(warp::header::optional::<String>("authorization"))
        .and(warp::body::json())
        .and_then(
            move |id: String, auth: Option<String>, updated: ModelPreset| {
                let cfg = app_config.clone();
                if !check_api_token(&auth, &cfg) {
                    return futures_util::future::ready(Ok(unauthorized_api_token()));
                }
                let mut presets = state.presets.lock().unwrap();
                if let Some(existing) = presets.iter_mut().find(|p| p.id == id) {
                    *existing = updated.clone();
                    let _ = presets::save_presets(&state.presets_path, &presets);
                    futures_util::future::ready(Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(
                        Box::new(warp::reply::json(
                            &serde_json::json!({"ok": true, "preset": updated}),
                        )),
                    ))
                } else {
                    futures_util::future::ready(Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(
                        Box::new(warp::reply::json(
                            &serde_json::json!({"ok": false, "error": "preset not found"}),
                        )),
                    ))
                }
            },
        )
}

fn api_delete_preset(
    state: AppState,
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "presets" / String)
        .and(warp::delete())
        .and(warp::header::optional::<String>("authorization"))
        .and_then(move |id: String, auth: Option<String>| {
            let cfg = app_config.clone();
            if !check_api_token(&auth, &cfg) {
                return futures_util::future::ready(Ok(unauthorized_api_token()));
            }
            let mut presets = state.presets.lock().unwrap();
            let before = presets.len();
            presets.retain(|p| p.id != id);
            if presets.len() < before {
                let _ = presets::save_presets(&state.presets_path, &presets);
                futures_util::future::ready(Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(
                    Box::new(warp::reply::json(&serde_json::json!({"ok": true}))),
                ))
            } else {
                futures_util::future::ready(Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(
                    Box::new(warp::reply::json(
                        &serde_json::json!({"ok": false, "error": "preset not found"}),
                    )),
                ))
            }
        })
}

fn api_reset_presets(
    state: AppState,
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "presets" / "reset")
        .and(warp::post())
        .and(warp::header::optional::<String>("authorization"))
        .and(with_app_config(app_config))
        .and_then(move |auth: Option<String>, cfg: Arc<AppConfig>| {
            if !check_api_token(&auth, &cfg) {
                return futures_util::future::ready(Ok(unauthorized_api_token()));
            }
            let defaults = presets::default_presets();
            let mut presets = state.presets.lock().unwrap();
            *presets = defaults;
            let _ = presets::save_presets(&state.presets_path, &presets);
            futures_util::future::ready(Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(
                Box::new(warp::reply::json(&serde_json::json!({"ok": true}))),
            ))
        })
}

// ── Template API ───────────────────────────────────────────────────────

fn api_get_templates(
    state: AppState,
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "templates")
        .and(warp::get())
        .and(warp::header::optional::<String>("authorization"))
        .and(with_app_config(app_config))
        .and_then(move |auth: Option<String>, cfg: Arc<AppConfig>| {
            let templates = state.templates.lock().unwrap().clone();
            if !check_api_token(&auth, &cfg) {
                return futures_util::future::ready(Ok(unauthorized_api_token()));
            }
            futures_util::future::ready(Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(
                Box::new(warp::reply::json(&templates)),
            ))
        })
}

// ── Personas API ───────────────────────────────────────────────────────

fn api_create_template(
    state: AppState,
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "templates")
        .and(warp::post())
        .and(warp::header::optional::<String>("authorization"))
        .and(warp::body::json())
        .and_then(
            move |auth: Option<String>, template: presets::SystemPromptTemplate| {
                let cfg = app_config.clone();
                if !check_api_token(&auth, &cfg) {
                    return futures_util::future::ready(Ok(unauthorized_api_token()));
                }
                let mut templates = state.templates.lock().unwrap();
                templates.push(template.clone());
                let _ = presets::save_templates(&state.templates_path, &templates);
                futures_util::future::ready(Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(
                    Box::new(warp::reply::json(
                        &serde_json::json!({"ok": true, "template": template}),
                    )),
                ))
            },
        )
}

fn api_update_template(
    state: AppState,
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "templates" / String)
        .and(warp::put())
        .and(warp::header::optional::<String>("authorization"))
        .and(warp::body::json())
        .and_then(
            move |id: String, auth: Option<String>, updated: presets::SystemPromptTemplate| {
                let cfg = app_config.clone();
                if !check_api_token(&auth, &cfg) {
                    return futures_util::future::ready(Ok(unauthorized_api_token()));
                }
                let mut templates = state.templates.lock().unwrap();
                if let Some(existing) = templates.iter_mut().find(|t| t.id == id) {
                    *existing = updated.clone();
                    let _ = presets::save_templates(&state.templates_path, &templates);
                    futures_util::future::ready(Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(
                        Box::new(warp::reply::json(
                            &serde_json::json!({"ok": true, "template": updated}),
                        )),
                    ))
                } else {
                    futures_util::future::ready(Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(
                        Box::new(warp::reply::json(
                            &serde_json::json!({"ok": false, "error": "template not found"}),
                        )),
                    ))
                }
            },
        )
}

fn api_delete_template(
    state: AppState,
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "templates" / String)
        .and(warp::delete())
        .and(warp::header::optional::<String>("authorization"))
        .and_then(move |id: String, auth: Option<String>| {
            let cfg = app_config.clone();
            if !check_api_token(&auth, &cfg) {
                return futures_util::future::ready(Ok(unauthorized_api_token()));
            }
            let mut templates = state.templates.lock().unwrap();
            let before = templates.len();
            templates.retain(|t| t.id != id);
            if templates.len() < before {
                let _ = presets::save_templates(&state.templates_path, &templates);
                futures_util::future::ready(Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(
                    Box::new(warp::reply::json(&serde_json::json!({"ok": true}))),
                ))
            } else {
                futures_util::future::ready(Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(
                    Box::new(warp::reply::json(
                        &serde_json::json!({"ok": false, "error": "template not found"}),
                    )),
                ))
            }
        })
}

fn api_get_models(
    state: AppState,
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "models")
        .and(warp::get())
        .and(warp::header::optional::<String>("authorization"))
        .and_then(move |auth: Option<String>| {
            let cfg = app_config.clone();
            if !check_api_token(&auth, &cfg) {
                return futures_util::future::ready(Ok(unauthorized_api_token()));
            }
            let models = state.discovered_models.lock().unwrap().clone();
            let tags = state.model_tags.lock().unwrap().tags.clone();
            let models_with_tags: Vec<serde_json::Value> = models
                .into_iter()
                .map(|m| {
                    let model_path = m.path.to_string_lossy().to_string();
                    let mut obj = serde_json::to_value(m).unwrap_or_default();
                    if let Some(model_obj) = obj.as_object_mut() {
                        let model_tags = tags.get(&model_path).cloned().unwrap_or_default();
                        model_obj.insert("tags".into(), serde_json::json!(model_tags));
                    }
                    obj
                })
                .collect();
            futures_util::future::ready(Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(
                Box::new(warp::reply::json(&models_with_tags)),
            ))
        })
}

fn api_refresh_models(
    state: AppState,
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "models" / "refresh")
        .and(warp::post())
        .and(warp::header::optional::<String>("authorization"))
        .and_then(move |auth: Option<String>| {
            let cfg = app_config.clone();
            if !check_api_token(&auth, &cfg) {
                return futures_util::future::ready(Ok(unauthorized_api_token()));
            }
            if let Some(ref dir) = state.models_dir {
                match models::scan_models_dir(dir) {
                    Ok(discovered) => {
                        let count = discovered.len();
                        *state.discovered_models.lock().unwrap() = discovered;
                        futures_util::future::ready(Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(
                            Box::new(warp::reply::json(&serde_json::json!({"ok": true, "count": count}))),
                        ))
                    }
                    Err(e) => {
                        futures_util::future::ready(Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(
                            Box::new(warp::reply::json(&serde_json::json!({"ok": false, "error": e.to_string()}))),
                        ))
                    }
                }
            } else {
                futures_util::future::ready(Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(
                    Box::new(warp::reply::json(&serde_json::json!({"ok": false, "error": "no models directory configured (use --models-dir)"}))),
                ))
            }
        })
}

/// DELETE /api/models/file — delete a .gguf file from disk and remove from in-memory cache
fn api_delete_model_file(
    state: AppState,
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "models" / "file")
        .and(warp::delete())
        .and(warp::header::optional::<String>("authorization"))
        .and(super::safe_json_body::<serde_json::Value>())
        .and_then(move |auth: Option<String>, body: serde_json::Value| {
            let cfg = app_config.clone();
            let st = state.clone();
            async move {
                if !check_api_token(&auth, &cfg) {
                    return Ok(unauthorized_api_token());
                }

                let path_str = match body.get("path").and_then(|v| v.as_str()) {
                    Some(p) => p.to_string(),
                    None => {
                        return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                            warp::reply::json(&serde_json::json!({"ok": false, "error": "missing path"})),
                        ));
                    }
                };

                if !path_str.to_lowercase().ends_with(".gguf") {
                    return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                        warp::reply::json(
                            &serde_json::json!({"ok": false, "error": "only .gguf files can be deleted"}),
                        ),
                    ));
                }

                let path = std::path::Path::new(&path_str);
                match std::fs::remove_file(path) {
                    Ok(_) => {
                        // Remove from in-memory cache so UI reflects the deletion immediately
                        let mut models = st.discovered_models.lock().unwrap();
                        models.retain(|m| m.path.to_str() != Some(&path_str));
                        Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                            warp::reply::json(&serde_json::json!({"ok": true})),
                        ))
                    }
                    Err(e) => Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                        warp::reply::json(
                            &serde_json::json!({"ok": false, "error": format!("Failed to delete: {e}")}),
                        ),
                    )),
                }
            }
        })
}

/// GET /api/models/tags — return all model tags
fn api_get_model_tags(
    state: AppState,
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "models" / "tags")
        .and(warp::get())
        .and(warp::header::optional::<String>("authorization"))
        .and_then(move |auth: Option<String>| {
            let cfg = app_config.clone();
            if !check_api_token(&auth, &cfg) {
                return futures_util::future::ready(Ok(unauthorized_api_token()));
            }
            let tags = state.model_tags.lock().unwrap().clone();
            futures_util::future::ready(Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(
                Box::new(warp::reply::json(&tags)),
            ))
        })
}

/// PUT /api/models/tags — update tags for a model
fn api_put_model_tags(
    state: AppState,
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "models" / "tags")
        .and(warp::put())
        .and(warp::header::optional::<String>("authorization"))
        .and(super::safe_json_body::<serde_json::Value>())
        .and_then(move |auth: Option<String>, body: serde_json::Value| {
            let st = state.clone();
            let cfg = app_config.clone();
            async move {
                if !check_api_token(&auth, &cfg) {
                    return Ok(unauthorized_api_token());
                }

                let model_path = match body.get("model_path").and_then(|v| v.as_str()) {
                    Some(p) => p.to_string(),
                    None => {
                        return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                            warp::reply::json(&serde_json::json!({"ok": false, "error": "missing model_path"})),
                        ));
                    }
                };

                let new_tags = match body.get("tags") {
                    Some(t) => match t.as_array() {
                        Some(arr) => arr.iter().filter_map(|v| v.as_str().map(String::from)).collect::<Vec<String>>(),
                        None => {
                            return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                                warp::reply::json(&serde_json::json!({"ok": false, "error": "tags must be an array of strings"})),
                            ));
                        }
                    },
                    None => {
                        return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                            warp::reply::json(&serde_json::json!({"ok": false, "error": "missing tags"})),
                        ));
                    }
                };

                let mut tags = st.model_tags.lock().unwrap();
                tags.tags.insert(model_path, new_tags);
                let tags_path = st.model_tags_path.clone();
                tags.save(&tags_path);

                Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                    warp::reply::json(&serde_json::json!({"ok": true})),
                ))
            }
        })
}

fn api_get_gpu_env(
    state: AppState,
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "gpu-env")
        .and(warp::get())
        .and(warp::header::optional::<String>("authorization"))
        .and_then(move |auth: Option<String>| {
            let cfg = app_config.clone();
            if !check_api_token(&auth, &cfg) {
                return futures_util::future::ready(Ok(unauthorized_api_token()));
            }
            let env = state.gpu_env.lock().unwrap().clone();
            let detected = gpu_env::detect_gpus();
            futures_util::future::ready(Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(
                Box::new(warp::reply::json(&serde_json::json!({
                    "env": env,
                    "architectures": GPU_ARCHITECTURES,
                    "detected": detected,
                }))),
            ))
        })
}

fn api_put_gpu_env(
    state: AppState,
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "gpu-env")
        .and(warp::put())
        .and(warp::header::optional::<String>("authorization"))
        .and(warp::body::json())
        .and_then(move |auth: Option<String>, updated: GpuEnv| {
            let cfg = app_config.clone();
            if !check_api_token(&auth, &cfg) {
                return futures_util::future::ready(Ok(unauthorized_api_token()));
            }
            let mut env = state.gpu_env.lock().unwrap();
            *env = updated;
            let _ = gpu_env::save_gpu_env(&state.gpu_env_path, &env);
            futures_util::future::ready(Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(
                Box::new(warp::reply::json(&serde_json::json!({"ok": true}))),
            ))
        })
}

fn api_get_settings(
    state: AppState,
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "settings")
        .and(warp::get())
        .and(warp::header::optional::<String>("authorization"))
        .and(with_app_config(app_config))
        .and_then(move |auth: Option<String>, cfg: Arc<AppConfig>| {
            let settings = state.ui_settings.lock().unwrap().clone();
            if !check_api_token(&auth, &cfg) {
                return futures_util::future::ready(Ok(unauthorized_api_token()));
            }
            let masked = mask_remote_agent_token(settings);
            futures_util::future::ready(Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(
                Box::new(warp::reply::json(&masked)),
            ))
        })
}

fn api_get_settings_full(
    state: AppState,
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    let app_config = app_config.clone();

    warp::path!("api" / "settings" / "full")
        .and(warp::get())
        .and(warp::header::optional::<String>("authorization"))
        .and(with_app_config(app_config))
        .map(move |auth: Option<String>, cfg: Arc<AppConfig>| {
            let bearer = auth.and_then(|v| v.strip_prefix("Bearer ").map(str::to_string));
            let has_api_token = bearer_matches_api_token(bearer.as_deref(), &cfg);

            if !has_api_token {
                return Box::new(warp::reply::with_status(
                    warp::reply::json(&serde_json::json!({ "error": "unauthorized" })),
                    warp::http::StatusCode::UNAUTHORIZED,
                )) as Box<dyn warp::reply::Reply>;
            }

            let settings = state.ui_settings.lock().unwrap().clone();
            Box::new(warp::reply::json(&settings))
        })
}

/// Check if a token looks like a masked token (contains bullet characters).
fn is_masked_token(t: &str) -> bool {
    t.contains('•')
}

/// Mask remote_agent_token for normal GET /api/settings.
/// Keeps first 4 and last 4 characters visible; replaces middle with bullets.
fn mask_remote_agent_token(mut s: UiSettings) -> UiSettings {
    if s.remote_agent_token.len() <= 8 {
        if !s.remote_agent_token.is_empty() {
            s.remote_agent_token = "••••".to_string();
        }
    } else {
        let t = &s.remote_agent_token;
        let masked = format!("{}••••••••••••••••{}", &t[..4], &t[t.len() - 4..]);
        s.remote_agent_token = masked;
    }
    s
}

fn api_put_settings(
    state: AppState,
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "settings")
        .and(warp::put())
        .and(warp::header::optional::<String>("authorization"))
        .and(warp::body::json())
        .and_then(move |auth: Option<String>, mut updated: UiSettings| {
            let cfg = app_config.clone();
            if !check_api_token(&auth, &cfg) {
                return futures_util::future::ready(Ok(unauthorized_api_token()));
            }
            // Detect if this is a partial update (only ws_push_interval_ms set, rest are defaults)
            let is_partial = updated.preset_id.is_empty()
                && updated.port == 8001
                && updated.llama_server_path.is_empty()
                && updated.llama_server_cwd.is_empty()
                && updated.models_dir.is_empty()
                && updated.server_endpoint.is_empty()
                && updated.llama_poll_interval == 1
                && updated.remote_agent_url.is_empty()
                && updated.remote_agent_token.is_empty()
                && !updated.remote_agent_ssh_autostart
                && updated.remote_agent_ssh_target.is_empty()
                && updated.remote_agent_ssh_command.is_empty()
                && updated.explicit_mode_policy.is_empty()
                && updated.context_card_view == "gauge";

            let mut settings = state.ui_settings.lock().unwrap();
            let old_dir = settings.models_dir.clone();
            let old_token = settings.remote_agent_token.clone();
            let old_push_interval = settings.ws_push_interval_ms;

            if is_partial {
                // Partial update: only apply ws_push_interval_ms
                settings.ws_push_interval_ms = updated.ws_push_interval_ms;
            } else {
                // Full update: replace all settings
                // But protect against overwriting with a masked token.
                let incoming_token = updated.remote_agent_token.clone();
                if is_masked_token(&incoming_token) && !old_token.is_empty() {
                    // Client sent a masked token; preserve the real one.
                    updated.remote_agent_token = old_token.clone();
                }
                *settings = updated;
            }

            let new_dir = settings.models_dir.clone();
            let token_changed =
                settings.remote_agent_token != old_token && !settings.remote_agent_token.is_empty();
            let push_interval_changed = settings.ws_push_interval_ms != old_push_interval;

            let _ = app_state::save_ui_settings(&state.ui_settings_path, &settings);
            drop(settings);

            if token_changed {
                state.agent_poll_notify.notify_waiters();
            }
            if push_interval_changed {
                state.llama_poll_notify.notify_waiters();
            }

            if new_dir != old_dir
                && !new_dir.is_empty()
                && let Ok(discovered) = crate::models::scan_models_dir(&PathBuf::from(&new_dir))
            {
                *state.discovered_models.lock().unwrap() = discovered;
            }

            futures_util::future::ready(Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(
                Box::new(warp::reply::json(&serde_json::json!({"ok": true}))),
            ))
        })
}

fn api_rotate_agent_token(
    state: AppState,
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    let state = state.clone();
    let app_config = app_config.clone();

    warp::path!("api" / "rotate-agent-token")
        .and(warp::post())
        .and(warp::header::optional::<String>("authorization"))
        .and(with_app_config(app_config))
        .and_then(move |auth: Option<String>, cfg: Arc<AppConfig>| {
            let state = state.clone();
            async move {
                // Require api-token
                let bearer = auth.and_then(|v| v.strip_prefix("Bearer ").map(str::to_string));
                let has_api_token = bearer_matches_api_token(bearer.as_deref(), &cfg);

                if !has_api_token {
                    return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                        warp::reply::with_status(
                            warp::reply::json(&serde_json::json!({ "error": "unauthorized" })),
                            warp::http::StatusCode::UNAUTHORIZED,
                        ),
                    ));
                }

                // Generate new token
                let new_token = crate::config::generate_random_token();

                // Update ui_settings
                let mut settings = state.ui_settings.lock().unwrap();
                settings.remote_agent_token = new_token;
                let _ = app_state::save_ui_settings(&state.ui_settings_path, &settings);
                drop(settings);

                // Notify agent poll loop to pick up new token
                state.agent_poll_notify.notify_waiters();

                Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(warp::reply::json(
                    &serde_json::json!({
                        "ok": true,
                        "message": "Agent token rotated"
                    }),
                )))
            }
        })
}

fn api_rotate_api_token(
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    let app_config = app_config.clone();

    warp::path!("api" / "rotate-api-token")
        .and(warp::post())
        .and(warp::header::optional::<String>("authorization"))
        .and(with_app_config(app_config))
        .and_then(move |auth: Option<String>, cfg: Arc<AppConfig>| {
            async move {
                // Require api-token
                let bearer = auth.and_then(|v| v.strip_prefix("Bearer ").map(str::to_string));
                let has_api_token = bearer_matches_api_token(bearer.as_deref(), &cfg);

                if !has_api_token {
                    return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                        warp::reply::with_status(
                            warp::reply::json(&serde_json::json!({ "error": "unauthorized" })),
                            warp::http::StatusCode::UNAUTHORIZED,
                        ),
                    ));
                }

                // Generate new token
                let new_token = crate::config::generate_random_token();

                // Persist to file (use encryption if configured)
                let config_dir = cfg.config_dir.clone();
                let token_file = config_dir.join("api-token");
                let stored = crate::config::encrypt_value(&new_token);
                if let Err(e) = std::fs::write(&token_file, &stored) {
                    eprintln!("[api] Failed to write rotated api-token to {token_file:?}: {e}");
                }
                crate::config::harden_file_permissions(&token_file);
                cfg.update_live_api_token(new_token);

                Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(warp::reply::json(
                    &serde_json::json!({
                        "ok": true,
                        "message": "API token rotated successfully."
                    }),
                )))
            }
        })
}

fn api_rotate_db_admin_token(
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    let app_config = app_config.clone();

    warp::path!("api" / "rotate-db-admin-token")
        .and(warp::post())
        .and(warp::header::optional::<String>("authorization"))
        .and(with_app_config(app_config))
        .and_then(move |auth: Option<String>, cfg: Arc<AppConfig>| {
            async move {
                // Require api-token
                let bearer = auth.and_then(|v| v.strip_prefix("Bearer ").map(str::to_string));
                let has_api_token = bearer_matches_api_token(bearer.as_deref(), &cfg);

                if !has_api_token {
                    return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                        warp::reply::with_status(
                            warp::reply::json(&serde_json::json!({ "error": "unauthorized" })),
                            warp::http::StatusCode::UNAUTHORIZED,
                        ),
                    ));
                }

                // Generate new token
                let new_token = crate::config::generate_random_token();

                // Persist to file (use encryption if configured)
                let config_dir = cfg.config_dir.clone();
                let token_file = config_dir.join("db-admin-token");
                let stored = crate::config::encrypt_value(&new_token);
                if let Err(e) = std::fs::write(&token_file, &stored) {
                    eprintln!(
                        "[api] Failed to write rotated db-admin-token to {token_file:?}: {e}"
                    );
                }
                crate::config::harden_file_permissions(&token_file);
                cfg.update_live_db_admin_token(new_token);

                Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(warp::reply::json(
                    &serde_json::json!({
                        "ok": true,
                        "message": "DB admin token rotated successfully."
                    }),
                )))
            }
        })
}

fn api_browse(
    state: AppState,
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    static LAST_BROWSE: AtomicU64 = AtomicU64::new(0);

    warp::path!("api" / "browse")
        .and(warp::get())
        .and(warp::header::optional::<String>("authorization"))
        .and(warp::query::<std::collections::HashMap<String, String>>())
        .and_then(
            move |auth: Option<String>, query: std::collections::HashMap<String, String>| {
                let cfg = app_config.clone();
                let state = state.clone();
                async move {
                    if !check_api_token(&auth, &cfg) {
                        return Ok(unauthorized_api_token());
                    }
                    // Cooldown: 1 second
                    let now = SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs();
                    let last = LAST_BROWSE.load(Ordering::Acquire);
                    if now - last < 1 {
                        return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                            warp::reply::with_status(
                                warp::reply::json(&serde_json::json!({
                                    "error": "too soon; please wait",
                                    "seconds_remaining": 1
                                })),
                                warp::http::StatusCode::TOO_MANY_REQUESTS,
                            ),
                        ));
                    }
                    LAST_BROWSE.store(now, Ordering::Release);

                    // Build allowed roots:
                    // - Home directory (primary root).
                    // - Directories used for models, TLS certs, etc.
                    let mut allowed_roots: Vec<PathBuf> = Vec::new();

                    // Always allow home directory
                    if let Some(home) = dirs::home_dir()
                        && let Ok(canon) = home.canonicalize()
                    {
                        allowed_roots.push(canon);
                    }

                    // Allow models_dir (parent directory)
                    if let Some(ref models_dir) = state.models_dir
                        && let Some(parent) = models_dir.parent()
                        && let Ok(canon) = parent.canonicalize()
                    {
                        allowed_roots.push(canon);
                    }

                    // Allow extra configured model directories and their parents
                    if let Ok(settings) = state.ui_settings.lock() {
                        for dir_str in &settings.extra_models_dirs {
                            let dir = std::path::Path::new(dir_str);
                            if let Ok(canon) = dir.canonicalize() {
                                allowed_roots.push(canon);
                            }
                            if let Some(parent) = dir.parent()
                                && let Ok(canon) = parent.canonicalize()
                            {
                                allowed_roots.push(canon);
                            }
                        }
                    }

                    // Allow TLS custom cert/key parent directories
                    if let Ok(tls) = state.tls_config.lock() {
                        if let Some(ref cert_path) = tls.custom_cert_path
                            && let Some(parent) = cert_path.parent()
                            && let Ok(canon) = parent.canonicalize()
                        {
                            allowed_roots.push(canon);
                        }
                        if let Some(ref key_path) = tls.custom_key_path
                            && let Some(parent) = key_path.parent()
                            && let Ok(canon) = parent.canonicalize()
                        {
                            allowed_roots.push(canon);
                        }
                    }

                    // Remove duplicates
                    allowed_roots.sort();
                    allowed_roots.dedup();

                    let requested = query.get("path").cloned().unwrap_or_default();
                    let filter = query.get("filter").cloned().unwrap_or_default();

                    let dir = if requested.is_empty() {
                        dirs::home_dir().unwrap_or_else(|| PathBuf::from("/"))
                    } else {
                        PathBuf::from(&requested)
                    };

                    let dir = match dir.canonicalize() {
                        Ok(p) => p,
                        Err(_) => {
                            return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                                warp::reply::with_status(
                                    warp::reply::json(&serde_json::json!({
                                        "path": requested,
                                        "error": "Path not found"
                                    })),
                                    warp::http::StatusCode::NOT_FOUND,
                                ),
                            ));
                        }
                    };

                    // Enforce allowlist: directory must be under one of the allowed roots
                    if !allowed_roots.iter().any(|root| dir.starts_with(root)) {
                        return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                            warp::reply::with_status(
                                warp::reply::json(&serde_json::json!({
                                    "path": dir.display().to_string(),
                                    "error": "Path not allowed"
                                })),
                                warp::http::StatusCode::FORBIDDEN,
                            ),
                        ));
                    }

                    if !dir.is_dir() {
                        return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                            warp::reply::with_status(
                                warp::reply::json(&serde_json::json!({
                                    "path": dir.display().to_string(),
                                    "error": "Not a directory"
                                })),
                                warp::http::StatusCode::BAD_REQUEST,
                            ),
                        ));
                    }

                    let parent = dir
                        .parent()
                        .map(|p| p.display().to_string())
                        .unwrap_or_default();

                    let mut entries: Vec<serde_json::Value> = Vec::new();
                    if let Ok(read_dir) = std::fs::read_dir(&dir) {
                        for entry in read_dir.flatten() {
                            let name = entry.file_name().to_string_lossy().to_string();
                            let meta = entry.metadata().ok();
                            let is_dir = meta.as_ref().is_some_and(|m| m.is_dir());
                            // Hide hidden files (e.g. .DS_Store) but show hidden directories
                            // so users can navigate into paths like ~/.config
                            if name.starts_with('.') && !is_dir {
                                continue;
                            }

                            if !is_dir && !filter.is_empty() {
                                let pass = match filter.as_str() {
                                    "gguf" => name.ends_with(".gguf"),
                                    "executable" => {
                                        #[cfg(unix)]
                                        {
                                            use std::os::unix::fs::PermissionsExt;
                                            meta.as_ref().is_some_and(|m| {
                                                m.permissions().mode() & 0o111 != 0
                                            })
                                        }
                                        #[cfg(not(unix))]
                                        {
                                            true
                                        }
                                    }
                                    _ => true,
                                };
                                if !pass {
                                    continue;
                                }
                            }

                            let size = if is_dir {
                                0
                            } else {
                                meta.as_ref().map(|m| m.len()).unwrap_or(0)
                            };
                            let size_display = if is_dir {
                                String::new()
                            } else if size >= 1_000_000_000 {
                                format!("{:.1} GB", size as f64 / 1_000_000_000.0)
                            } else if size >= 1_000_000 {
                                format!("{:.0} MB", size as f64 / 1_000_000.0)
                            } else {
                                format!("{:.0} KB", size as f64 / 1_000.0)
                            };

                            entries.push(serde_json::json!({
                                "name": name,
                                "is_dir": is_dir,
                                "size": size,
                                "size_display": size_display,
                                "path": entry.path().display().to_string(),
                            }));
                        }
                    }

                    entries.sort_by(|a, b| {
                        let a_dir = a["is_dir"].as_bool().unwrap_or(false);
                        let b_dir = b["is_dir"].as_bool().unwrap_or(false);
                        b_dir.cmp(&a_dir).then_with(|| {
                            a["name"]
                                .as_str()
                                .unwrap_or("")
                                .to_lowercase()
                                .cmp(&b["name"].as_str().unwrap_or("").to_lowercase())
                        })
                    });

                    Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                        warp::reply::with_status(
                            warp::reply::json(&serde_json::json!({
                                "path": dir.display().to_string(),
                                "parent": parent,
                                "entries": entries,
                            })),
                            warp::http::StatusCode::OK,
                        ),
                    ))
                }
            },
        )
}

fn api_chat(
    state: AppState,
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "chat")
        .and(warp::post())
        .and(warp::header::optional::<String>("authorization"))
        .and(warp::body::content_length_limit(2 * 1024 * 1024))
        .and(warp::body::bytes())
        .and_then(move |auth: Option<String>, body: bytes::Bytes| {
            let cfg = app_config.clone();
            let state = state.clone();
            async move {
                if !check_api_token(&auth, &cfg) {
                    return Ok(unauthorized_api_token());
                }
                // Derive endpoint from active session — no user-controlled input
                let (url, permit) = prepare_inference_request(&state).await?;
                let client = build_upstream_client(Duration::from_secs(120))?;
                let request_body = body.to_vec();

                // Stream response from upstream — Tokio cancels automatically on client disconnect
                let resp = send_upstream_request_with_retry(|| {
                    client
                        .post(&url)
                        .header("Content-Type", "application/json")
                        .body(request_body.clone())
                })
                .await?;

                let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
                let stream = tokio_stream::wrappers::UnboundedReceiverStream::new(rx);

                // Forward SSE events to client — stops if client disconnects (tx closed)
                tokio::spawn(async move {
                    use futures_util::StreamExt;
                    let _permit = permit;
                    let mut stream = resp.bytes_stream();
                    let mut buf = String::new();

                    while let Some(chunk) = stream.next().await {
                        match chunk {
                            Ok(bytes) => {
                                // Stop if client disconnected
                                if tx.is_closed() {
                                    return;
                                }

                                buf.push_str(&String::from_utf8_lossy(&bytes));

                                // Process complete lines
                                while let Some(pos) = buf.find('\n') {
                                    let line = buf[..pos].to_string();
                                    buf = buf[pos + 1..].to_string();

                                    if let Some(data) = line.strip_prefix("data: ")
                                        && !data.trim().is_empty()
                                    {
                                        let _ = tx.send(Ok::<_, warp::Error>(
                                            warp::sse::Event::default().data(data.to_string()),
                                        ));
                                    }
                                }
                            }
                            Err(e) => {
                                let _ = tx.send(Ok::<_, warp::Error>(
                                    warp::sse::Event::default().data(format!(
                                        "{{\"error\":\"{}\"}}",
                                        e.to_string().replace('"', "'")
                                    )),
                                ));
                                break;
                            }
                        }
                    }
                });

                Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(warp::sse::reply(
                    stream,
                )))
            }
        })
}

fn api_chat_abort(
    _state: AppState,
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "chat" / "abort")
        .and(warp::post())
        .and(warp::header::optional::<String>("authorization"))
        .and_then(move |auth: Option<String>| {
            let cfg = app_config.clone();
            async move {
                if !check_api_token(&auth, &cfg) {
                    return Ok(unauthorized_api_token());
                }
                Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(warp::reply::json(
                    &serde_json::json!({"ok": true}),
                )))
            }
        })
}

fn api_chat_suggestions(
    state: AppState,
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "chat" / "suggestions")
        .and(warp::post())
        .and(warp::header::optional::<String>("authorization"))
        .and(warp::body::json::<SuggestionRequest>())
        .and_then(move |auth: Option<String>, req: SuggestionRequest| {
            let cfg = app_config.clone();
            let state = state.clone();
            async move {
                if !check_api_token(&auth, &cfg) {
                    return Ok(unauthorized_api_token());
                }
                let (all_messages, system_prompt, context_notes, quick_guide_active) =
                    if let Some(messages) = req.messages.clone() {
                        (
                            messages,
                            req.system_prompt.clone().unwrap_or_default(),
                            req.context_notes.clone().unwrap_or_default(),
                            req.quick_guide_active.clone().unwrap_or_default(),
                        )
                    } else {
                        // Fallback for older clients: load tab from chat_storage.
                        let tab = state
                            .chat_storage
                            .get_tab(&req.tab_id)
                            .ok()
                            .ok_or(warp::reject::not_found())?;

                        let messages: Vec<SuggestionContextMessage> = tab
                            .messages
                            .iter()
                            .map(|msg| SuggestionContextMessage {
                                role: msg.role.clone(),
                                content: msg.content.clone(),
                            })
                            .collect();

                        let system_prompt = tab.system_prompt.clone();
                        let context_notes: Vec<ContextNote> =
                            serde_json::from_value(tab.context_notes.clone()).unwrap_or_default();

                        (
                            messages,
                            system_prompt,
                            context_notes,
                            String::new(),
                        )
                    };

                // Get last N messages
                let depth = req.context_depth.unwrap_or(10) as usize;
                let mut messages: Vec<SuggestionContextMessage> = all_messages
                    .iter()
                    .rev()
                    .take(depth)
                    .cloned()
                    .collect();
                messages.reverse();

                // Build conversation context
                let mut context = String::new();
                let trimmed_system_prompt = system_prompt.trim();
                if !trimmed_system_prompt.is_empty() {
                    context.push_str("System Prompt:\n");
                    context.push_str(trimmed_system_prompt);
                    context.push_str("\n\n");
                }

                let filtered_notes: Vec<ContextNote> = context_notes
                    .into_iter()
                    .filter(|note| !note.content.trim().is_empty())
                    .collect();
                if !filtered_notes.is_empty() {
                    context.push_str("Context Notes:\n");
                    for note in filtered_notes {
                        context.push_str(&format!(
                            "- {}: {}\n",
                            note.section.trim(),
                            note.content.trim()
                        ));
                    }
                    context.push('\n');
                }

                let trimmed_quick_guide = quick_guide_active.trim();
                if !trimmed_quick_guide.is_empty() {
                    context.push_str("Active Quick Guide:\n");
                    context.push_str(trimmed_quick_guide);
                    context.push_str("\n\n");
                }

                for msg in &messages {
                    let role = if msg.role == "user" { "User" } else { "Assistant" };
                    context.push_str(&format!("{}: {}\n", role, msg.content));
                }

                // Load prompts from files
                let file_prompts = load_prompts_from_files();

                // Get or use default prompt (file-based or hardcoded fallback)
                let default_prompt = req.prompt.or_else(|| {
                    file_prompts.get(&req.category).cloned().or_else(|| {
                        match req.category.as_str() {
                            "plot-twist" => Some("You are a plot twist specialist. Based on the conversation below, suggest {} unexpected, surprising events that could happen next.\n\nFormat as a numbered list. Prioritize: betrayals, revelations, power reversals, unexpected arrivals, hidden truths.\n\n[conversation context]".to_string()),
                            "new-character" => Some("You are a character introduction specialist. Based on the conversation below, suggest {} new characters that could enter the story.\n\nFormat as: [Character Name]: [Brief description and how they connect to current story]\n\n[conversation context]".to_string()),
                            _ => Some("You are a creative brainstorming partner. Based on the conversation below, suggest {} varied, actionable next steps the user could take.\n\nFormat as a numbered list. Prioritize variety: dialogue, action, investigation, social, creative approaches.\n\n[conversation context]".to_string()),
                        }
                    })
                });

                let count = req.count.unwrap_or(5);
                let prompt = default_prompt
                    .unwrap_or_default()
                    .replace("{count}", &count.to_string())
                    .replace("[STORY CONTEXT]", &context)
                    .replace("[conversation context]", &context)
                    .replace("[CONVERSATION CONTEXT]", &context)
                    + &suggestions_output_contract(&req.category, count);
                let temperature = suggestion_temperature(&req.category);

                // Build messages for suggestion request
                let suggestion_messages = vec![
                    serde_json::json!({"role": "system", "content": "You are a helpful creative writing assistant. Keep all reasoning internal. Return only the final answer in the exact requested format with no preamble or analysis."}),
                    serde_json::json!({"role": "user", "content": prompt}),
                ];

                // Call llama.cpp
                let (url, _permit) = prepare_inference_request(&state).await?;
                let client = build_upstream_client(Duration::from_secs(30))?;
                let payload = serde_json::json!({
                    "messages": suggestion_messages,
                    "stream": true,
                    "thinking_budget_tokens": 0,
                    "chat_template_kwargs": {
                        "enable_thinking": false
                    },
                    "temperature": temperature,
                    "max_tokens": 512,
                });

                let response = send_upstream_request_with_retry(|| {
                    client
                        .post(&url)
                        .header("Content-Type", "application/json")
                        .json(&payload)
                })
                .await?;

                use futures_util::StreamExt;

                let mut upstream = response.bytes_stream();
                let mut buf = String::new();
                let mut content = String::new();
                let mut reasoning_content = String::new();

                while let Some(chunk) = upstream.next().await {
                    let chunk = chunk
                        .map_err(|e| warp::reject::custom(ApiError::from_reqwest(e)))?;
                    buf.push_str(&String::from_utf8_lossy(&chunk));

                    while let Some(pos) = buf.find('\n') {
                        let line = buf[..pos].trim().to_string();
                        buf = buf[pos + 1..].to_string();

                        let Some(data) = line.strip_prefix("data: ") else {
                            continue;
                        };
                        let data = data.trim();
                        if data.is_empty() || data == "[DONE]" {
                            continue;
                        }

                        let event: serde_json::Value = serde_json::from_str(data)
                            .map_err(|e| warp::reject::custom(ApiError::internal(e.to_string())))?;
                        if let Some(delta) = event.get("choices")
                            .and_then(|c| c.as_array())
                            .and_then(|c| c.first())
                            .and_then(|c| c.get("delta"))
                            .and_then(|d| d.get("content"))
                            .and_then(|c| c.as_str())
                        {
                            content.push_str(delta);
                        } else if let Some(reasoning) = event.get("choices")
                            .and_then(|c| c.as_array())
                            .and_then(|c| c.first())
                            .and_then(|c| c.get("delta"))
                            .and_then(|d| d.get("reasoning_content"))
                            .and_then(|c| c.as_str())
                        {
                            reasoning_content.push_str(reasoning);
                        } else if let Some(message) = event.get("choices")
                            .and_then(|c| c.as_array())
                            .and_then(|c| c.first())
                            .and_then(|c| c.get("message"))
                            .and_then(|m| m.get("content"))
                            .and_then(|c| c.as_str())
                        {
                            content.push_str(message);
                        }
                    }
                }

                let content = if content.trim().is_empty() {
                    reasoning_content.trim()
                } else {
                    content.trim()
                };
                if content.is_empty() {
                    return Err(warp::reject::custom(ApiError::gateway(
                        "upstream returned empty suggestion content".to_string(),
                    )));
                }

                let cards = if req.category == "director" {
                    parse_director_cards(content)
                } else {
                    Vec::new()
                };
                let suggestions = if req.category == "director" && !cards.is_empty() {
                    cards
                        .iter()
                        .map(|card| format!("{}\n{}", card.title, card.detail))
                        .collect()
                } else {
                    parse_suggestions(content)
                };
                if suggestions.is_empty() {
                    return Err(warp::reject::custom(ApiError::gateway(
                        "upstream returned no parseable suggestions".to_string(),
                    )));
                }
                let count = if req.category == "director" && !cards.is_empty() {
                    cards.len() as u32
                } else {
                    suggestions.len() as u32
                };

                Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                    warp::reply::json(&SuggestionResponse {
                        suggestions,
                        cards,
                        category: req.category,
                        count,
                    }),
                ))
            }
        })
}

fn parse_suggestions(text: &str) -> Vec<String> {
    fn clean_fragment(value: &str) -> String {
        value
            .replace("**", "")
            .replace(['*', '`'], "")
            .trim()
            .to_string()
    }

    if let Ok(value) = serde_json::from_str::<serde_json::Value>(text) {
        let suggestions_json = value
            .get("suggestions")
            .and_then(|v| v.as_array())
            .cloned()
            .or_else(|| value.as_array().cloned());

        if let Some(entries) = suggestions_json {
            let extracted: Vec<String> = entries
                .into_iter()
                .filter_map(|entry| {
                    if let Some(obj) = entry.as_object() {
                        let title = obj
                            .get("title")
                            .and_then(|v| v.as_str())
                            .map(clean_fragment)
                            .unwrap_or_default();
                        let description = obj
                            .get("description")
                            .and_then(|v| v.as_str())
                            .map(clean_fragment)
                            .unwrap_or_default();
                        if title.is_empty() || description.is_empty() {
                            return None;
                        }
                        return Some(format!("{}\n{}", title, description));
                    }

                    entry
                        .as_str()
                        .map(clean_fragment)
                        .and_then(|value| if value.is_empty() { None } else { Some(value) })
                })
                .collect();

            if !extracted.is_empty() {
                return extracted;
            }
        }
    }

    // Pathweaver format: [EMOJI] TITLE\nDESCRIPTION with --- separators
    // Split by --- separator and parse each block
    let blocks: Vec<&str> = text.split("---").collect();
    let mut suggestions = Vec::new();

    for block in blocks {
        let lines: Vec<&str> = block
            .lines()
            .map(|l| l.trim())
            .filter(|l| !l.is_empty())
            .collect();
        if lines.len() >= 2 {
            // First line is title with emoji, rest is description
            let title = clean_fragment(lines[0]);
            let description = clean_fragment(&lines[1..].join(" "));
            let title_lower = title.to_ascii_lowercase();
            let description_lower = description.to_ascii_lowercase();
            let is_meta = title_lower.contains("thinking process")
                || description_lower.contains("analyze user input")
                || description_lower.contains("output format")
                || description_lower.contains("guidelines")
                || description_lower.contains("deconstruct key elements");
            if !title.is_empty() && !description.is_empty() && !is_meta {
                // Combine title and description with newline
                suggestions.push(format!("{}\n{}", title, description));
            }
        }
    }

    // Thinking-mode fallback: extract explicit "Idea N" / "Suggestion N" brainstorm bullets.
    if suggestions.is_empty() {
        let normalized = text.replace('\n', " ");
        let brainstorm_slice = normalized
            .split("Brainstorming Suggestions")
            .nth(1)
            .unwrap_or(&normalized);
        let inline_ideas = regex::Regex::new(
            r"(?:Idea|Suggestion)\s*\d+\s*(?:\(([^)]+)\))?:\s*(.+?)(?:\s+-\s+\*?(?:Idea|Suggestion)\s*\d+\s*(?:\(|:)|$)",
        )
        .ok();
        if let Some(re) = inline_ideas {
            let extracted: Vec<String> = re
                .captures_iter(brainstorm_slice)
                .filter_map(|caps| {
                    let raw_description = caps.get(2)?.as_str();
                    let description = clean_fragment(raw_description);
                    if description.is_empty() {
                        return None;
                    }
                    let raw_title = caps
                        .get(1)
                        .map(|m| m.as_str())
                        .filter(|title| !title.trim().is_empty())
                        .unwrap_or("Next Beat");
                    Some(format!("{}\n{}", clean_fragment(raw_title), description))
                })
                .collect();
            if !extracted.is_empty() {
                return extracted;
            }
        }

        let idea_bullets = regex::Regex::new(
            r"(?m)^\s*[-*]\s*\*?(?:Idea|Suggestion)\s*\d+\s*(?:\(([^)]+)\))?:\*?\s*(.+)$",
        )
        .ok();
        if let Some(re) = idea_bullets {
            let extracted: Vec<String> = text
                .lines()
                .filter_map(|line| re.captures(line))
                .filter_map(|caps| {
                    let raw_description = caps.get(2)?.as_str();
                    let description = clean_fragment(raw_description);
                    if description.is_empty() {
                        return None;
                    }
                    let raw_title = caps
                        .get(1)
                        .map(|m| m.as_str())
                        .filter(|title| !title.trim().is_empty())
                        .unwrap_or("Next Beat");
                    Some(format!("{}\n{}", clean_fragment(raw_title), description))
                })
                .collect();
            if !extracted.is_empty() {
                return extracted;
            }
        }
    }

    // If Pathweaver format yielded nothing, try numbered list: "1. Option" or "1) Option"
    if suggestions.is_empty() {
        let numbered = regex::Regex::new(r"^\d+[\.\)]\s*(.+)$").ok();
        if let Some(re) = numbered {
            let numbered_suggestions: Vec<String> = text
                .lines()
                .filter_map(|line| re.captures(line))
                .filter_map(|caps| caps.get(1))
                .map(|m| clean_fragment(m.as_str()))
                .filter(|s| !s.is_empty())
                .filter(|s| {
                    let lowercase = s.to_ascii_lowercase();
                    !lowercase.contains("analyze user input")
                        && !lowercase.contains("output format")
                        && !lowercase.contains("guidelines")
                })
                .collect();
            if !numbered_suggestions.is_empty() {
                return numbered_suggestions;
            }
        }
    }

    // Try bullet list: "- Option" or "* Option"
    if suggestions.is_empty() {
        let bullets = regex::Regex::new(r"^[-*]\s+(.+)$").ok();
        if let Some(re) = bullets {
            let bullet_suggestions: Vec<String> = text
                .lines()
                .filter_map(|line| re.captures(line))
                .filter_map(|caps| caps.get(1))
                .map(|m| clean_fragment(m.as_str()))
                .filter(|s| !s.is_empty())
                .filter(|s| {
                    let lowercase = s.to_ascii_lowercase();
                    !lowercase.starts_with("role:")
                        && !lowercase.starts_with("task:")
                        && !lowercase.starts_with("goal:")
                        && !lowercase.starts_with("output format:")
                })
                .collect();
            if !bullet_suggestions.is_empty() {
                return bullet_suggestions;
            }
        }
    }

    // Fallback: split by newlines, filter empty and short lines
    if suggestions.is_empty() {
        suggestions = text
            .lines()
            .map(clean_fragment)
            .filter(|s| !s.is_empty() && s.len() > 2)
            .filter(|s| !s.starts_with('['))
            .filter(|s| {
                let lowercase = s.to_ascii_lowercase();
                !lowercase.contains("here's a thinking process")
                    && !lowercase.contains("analyze user input")
                    && !lowercase.contains("output format")
                    && !lowercase.contains("guidelines")
            })
            .collect();
    }

    suggestions
}

fn parse_director_cards(text: &str) -> Vec<SuggestionCard> {
    fn clean_fragment(value: &str) -> String {
        value
            .replace("**", "")
            .replace(['*', '`'], "")
            .trim()
            .to_string()
    }

    fn normalize_type(value: &str) -> String {
        let normalized = value.trim().to_ascii_lowercase().replace(['_', ' '], "-");
        match normalized.as_str() {
            "revelation" => "reveal".to_string(),
            "pressure" | "reveal" | "escalation" | "interruption" | "twist" | "tone-shift"
            | "reversal" | "intimacy" | "investigation" | "confrontation" => normalized,
            _ => "pressure".to_string(),
        }
    }

    fn infer_type(title: &str, body: &str) -> String {
        let haystack = format!("{} {}", title, body).to_ascii_lowercase();
        if haystack.contains("reveal") || haystack.contains("truth") || haystack.contains("secret")
        {
            return "reveal".to_string();
        }
        if haystack.contains("interrupt")
            || haystack.contains("arrives")
            || haystack.contains("appears")
        {
            return "interruption".to_string();
        }
        if haystack.contains("twist")
            || haystack.contains("betray")
            || haystack.contains("reversal")
        {
            return "twist".to_string();
        }
        if haystack.contains("close-quarters")
            || haystack.contains("lunges")
            || haystack.contains("fight")
            || haystack.contains("draw")
            || haystack.contains("gun")
        {
            return "confrontation".to_string();
        }
        if haystack.contains("intimate") || haystack.contains("quiet") || haystack.contains("soft")
        {
            return "intimacy".to_string();
        }
        if haystack.contains("investigate")
            || haystack.contains("photo")
            || haystack.contains("clue")
        {
            return "investigation".to_string();
        }
        if haystack.contains("escalate")
            || haystack.contains("danger")
            || haystack.contains("violence")
            || haystack.contains("shot")
        {
            return "escalation".to_string();
        }
        "pressure".to_string()
    }

    fn split_effect_detail(description: &str) -> (String, String) {
        let trimmed = clean_fragment(description);
        if trimmed.is_empty() {
            return (String::new(), String::new());
        }
        let mut parts = trimmed.splitn(2, ". ");
        let first = parts.next().unwrap_or("").trim().trim_end_matches('.');
        let rest = parts.next().unwrap_or("").trim();
        let effect = first.to_string();
        let detail = if rest.is_empty() {
            first.to_string()
        } else {
            rest.to_string()
        };
        (effect, detail)
    }

    if let Ok(value) = serde_json::from_str::<serde_json::Value>(text) {
        let suggestions_json = value
            .get("suggestions")
            .and_then(|v| v.as_array())
            .cloned()
            .or_else(|| value.as_array().cloned());

        if let Some(entries) = suggestions_json {
            let cards: Vec<SuggestionCard> = entries
                .into_iter()
                .filter_map(|entry| {
                    let obj = entry.as_object()?;
                    let title = clean_fragment(obj.get("title")?.as_str()?);
                    if title.is_empty() {
                        return None;
                    }
                    let effect = obj
                        .get("effect")
                        .and_then(|v| v.as_str())
                        .map(clean_fragment)
                        .unwrap_or_default();
                    let detail = obj
                        .get("detail")
                        .or_else(|| obj.get("description"))
                        .and_then(|v| v.as_str())
                        .map(clean_fragment)
                        .unwrap_or_default();
                    let suggestion_type = obj
                        .get("type")
                        .and_then(|v| v.as_str())
                        .map(normalize_type)
                        .unwrap_or_else(|| infer_type(&title, &format!("{} {}", effect, detail)));
                    let fallback_effect = if effect.is_empty() {
                        title.clone()
                    } else {
                        effect
                    };
                    let fallback_detail = if detail.is_empty() {
                        fallback_effect.clone()
                    } else {
                        detail
                    };
                    Some(SuggestionCard {
                        suggestion_type,
                        title,
                        effect: fallback_effect,
                        detail: fallback_detail,
                    })
                })
                .collect();
            if !cards.is_empty() {
                return cards;
            }
        }
    }

    parse_suggestions(text)
        .into_iter()
        .filter_map(|entry| {
            let mut parts = entry.splitn(2, '\n');
            let title = clean_fragment(parts.next().unwrap_or(""));
            if title.is_empty() {
                return None;
            }
            let description = clean_fragment(parts.next().unwrap_or(""));
            let (effect, detail) = split_effect_detail(&description);
            Some(SuggestionCard {
                suggestion_type: infer_type(&title, &description),
                title,
                effect: if effect.is_empty() {
                    description.clone()
                } else {
                    effect
                },
                detail: if detail.is_empty() {
                    description
                } else {
                    detail
                },
            })
        })
        .collect()
}

// ── Chat storage helper ─────────────────────────────────────────────

fn with_chat_storage(
    storage: Arc<ChatStorage>,
) -> impl Filter<Extract = (Arc<ChatStorage>,), Error = std::convert::Infallible> + Clone {
    warp::any().map(move || storage.clone())
}

fn with_app_config(
    cfg: Arc<AppConfig>,
) -> impl Filter<Extract = (Arc<AppConfig>,), Error = std::convert::Infallible> + Clone {
    warp::any().map(move || cfg.clone())
}

fn now_ts() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}

fn new_tab_id() -> String {
    format!("tab_{}", now_ts())
}

// GET /api/chat/tabs — metadata only (no messages)
fn api_list_tabs(
    storage: Arc<ChatStorage>,
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "chat" / "tabs")
        .and(warp::get())
        .and(warp::header::optional::<String>("authorization"))
        .and(warp::query::<HashMap<String, String>>())
        .and(with_chat_storage(storage))
        .and_then(
            move |auth: Option<String>, query: HashMap<String, String>, store: Arc<ChatStorage>| {
                let cfg = app_config.clone();
                async move {
                    if !check_api_token(&auth, &cfg) {
                        return Ok(unauthorized_api_token());
                    }
                    let visibilities = parse_visibility_param(
                        &query.get("visibility").cloned().unwrap_or_default(),
                    );
                    match store.list_tabs(&visibilities) {
                        Ok(tabs) => Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                            warp::reply::json(&tabs),
                        )),
                        Err(e) => {
                            eprintln!("list_tabs error: {e}");
                            Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                                warp::reply::json(&Vec::<crate::chat_storage::TabMeta>::new()),
                            ))
                        }
                    }
                }
            },
        )
}

/// Compute a short, stable hash of the template/system prompt content
/// for tracking which version of a persona was used for a tab.
fn compute_template_hash(prompt: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prompt.as_bytes());
    let result = hasher.finalize();
    result
        .iter()
        .take(6)
        .map(|b| format!("{:02x}", b))
        .collect::<String>()
}

// POST /api/chat/tabs — create new tab
fn api_create_tab(
    storage: Arc<ChatStorage>,
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "chat" / "tabs")
        .and(warp::post())
        .and(warp::header::optional::<String>("authorization"))
        .and(super::safe_json_body::<crate::chat_storage::ChatTabRow>())
        .and(with_chat_storage(storage))
        .and_then(
            move |auth: Option<String>,
                  mut tab: crate::chat_storage::ChatTabRow,
                  store: Arc<ChatStorage>| {
                let cfg = app_config.clone();
                async move {
                    if !check_api_token(&auth, &cfg) {
                        return Ok(unauthorized_api_token());
                    }
                    if tab.id.is_empty() {
                        tab.id = new_tab_id();
                    }
                    tab.created_at = now_ts();
                    tab.updated_at = tab.created_at;
                    // If created from a template, store a short hash of the prompt
                    // to track which persona version was used.
                    if tab.active_template_id.is_some() && tab.template_version_or_hash.is_none() {
                        tab.template_version_or_hash =
                            Some(compute_template_hash(&tab.system_prompt));
                    }
                    match store.create_tab(&tab) {
                        Ok(_) => Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                            warp::reply::json(&tab),
                        )),
                        Err(e) => Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                            warp::reply::json(
                                &serde_json::json!({"ok":false,"error":e.to_string()}),
                            ),
                        )),
                    }
                }
            },
        )
}

// GET /api/chat/tabs/:id — full tab with messages
fn api_get_tab(
    storage: Arc<ChatStorage>,
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "chat" / "tabs" / String)
        .and(warp::get())
        .and(warp::header::optional::<String>("authorization"))
        .and(with_chat_storage(storage))
        .and_then(
            move |id: String, auth: Option<String>, store: Arc<ChatStorage>| {
                let cfg = app_config.clone();
                async move {
                    if !check_api_token(&auth, &cfg) {
                        return Ok(unauthorized_api_token());
                    }
                    match store.get_tab(&id) {
                        Ok(tab) => Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                            warp::reply::json(&tab),
                        )),
                        Err(e) => Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                            warp::reply::json(
                                &serde_json::json!({"ok":false,"error":e.to_string()}),
                            ),
                        )),
                    }
                }
            },
        )
}

// PUT /api/chat/tabs/:id — full save (meta + replace messages)
fn api_put_tab(
    storage: Arc<ChatStorage>,
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "chat" / "tabs" / String)
        .and(warp::put())
        .and(warp::header::optional::<String>("authorization"))
        .and(super::safe_json_body::<crate::chat_storage::ChatTabRow>())
        .and(with_chat_storage(storage))
        .and_then(
            move |id: String,
                  auth: Option<String>,
                  mut tab: crate::chat_storage::ChatTabRow,
                  store: Arc<ChatStorage>| {
                let cfg = app_config.clone();
                async move {
                    if !check_api_token(&auth, &cfg) {
                        return Ok(unauthorized_api_token());
                    }
                    tab.id = id;
                    tab.updated_at = now_ts();
                    // If tab uses a template but has no hash yet, set it.
                    if tab.active_template_id.is_some() && tab.template_version_or_hash.is_none() {
                        tab.template_version_or_hash =
                            Some(compute_template_hash(&tab.system_prompt));
                    }
                    let messages = std::mem::take(&mut tab.messages);
                    let msg_rows: Vec<crate::chat_storage::MessageRow> = messages
                        .into_iter()
                        .enumerate()
                        .map(|(seq, m)| crate::chat_storage::MessageRow {
                            seq: seq as i64,
                            tab_id: tab.id.clone(),
                            ..m
                        })
                        .collect();
                    let result = store
                        .update_tab_meta(&tab)
                        .and_then(|_| store.replace_messages(&tab.id, &msg_rows));
                    match result {
                        Ok(_) => Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                            warp::reply::json(&serde_json::json!({"ok":true})),
                        )),
                        Err(e) => Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                            warp::reply::json(
                                &serde_json::json!({"ok":false,"error":e.to_string()}),
                            ),
                        )),
                    }
                }
            },
        )
}

// DELETE /api/chat/tabs/:id
fn api_delete_tab(
    storage: Arc<ChatStorage>,
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "chat" / "tabs" / String)
        .and(warp::delete())
        .and(warp::header::optional::<String>("authorization"))
        .and(with_chat_storage(storage))
        .and_then(
            move |id: String, auth: Option<String>, store: Arc<ChatStorage>| {
                let cfg = app_config.clone();
                async move {
                    if !check_api_token(&auth, &cfg) {
                        return Ok(unauthorized_api_token());
                    }
                    match store.delete_tab(&id) {
                        Ok(_) => Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                            warp::reply::json(&serde_json::json!({"ok":true})),
                        )),
                        Err(e) => Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                            warp::reply::json(
                                &serde_json::json!({"ok":false,"error":e.to_string()}),
                            ),
                        )),
                    }
                }
            },
        )
}

// PATCH /api/chat/tabs/:id/meta — metadata only, no messages
fn api_patch_tab_meta(
    storage: Arc<ChatStorage>,
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "chat" / "tabs" / String / "meta")
        .and(warp::patch())
        .and(warp::header::optional::<String>("authorization"))
        .and(super::safe_json_body::<crate::chat_storage::ChatTabRow>())
        .and(with_chat_storage(storage))
        .and_then(
            move |id: String,
                  auth: Option<String>,
                  mut tab: crate::chat_storage::ChatTabRow,
                  store: Arc<ChatStorage>| {
                let cfg = app_config.clone();
                async move {
                    if !check_api_token(&auth, &cfg) {
                        return Ok(unauthorized_api_token());
                    }
                    tab.id = id;
                    tab.updated_at = now_ts();
                    match store.update_tab_meta(&tab) {
                        Ok(_) => Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                            warp::reply::json(&serde_json::json!({"ok":true})),
                        )),
                        Err(e) => Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                            warp::reply::json(
                                &serde_json::json!({"ok":false,"error":e.to_string()}),
                            ),
                        )),
                    }
                }
            },
        )
}

// POST /api/chat/tabs/:id/messages — append one or more messages
fn api_append_messages(
    storage: Arc<ChatStorage>,
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "chat" / "tabs" / String / "messages")
        .and(warp::post())
        .and(warp::header::optional::<String>("authorization"))
        .and(warp::body::json::<serde_json::Value>())
        .and(with_chat_storage(storage))
        .and_then(
            move |id: String,
                  auth: Option<String>,
                  body: serde_json::Value,
                  store: Arc<ChatStorage>| {
                let cfg = app_config.clone();
                async move {
                    if !check_api_token(&auth, &cfg) {
                        return Ok(unauthorized_api_token());
                    }
                    let msgs = body["messages"].as_array().cloned().unwrap_or_default();
                    let mut last_id = 0i64;
                    for msg_val in msgs {
                        let msg: crate::chat_storage::MessageRow = serde_json::from_value(msg_val)
                            .unwrap_or_else(|_| crate::chat_storage::MessageRow {
                                tab_id: id.clone(),
                                role: "user".into(),
                                content: "".into(),
                                id: 0,
                                timestamp_ms: 0,
                                input_tokens: None,
                                output_tokens: None,
                                cumulative_input_tokens: None,
                                cumulative_output_tokens: None,
                                compaction_marker: false,
                                variants: None,
                                variant_index: None,
                                seq: 0,
                            });
                        let mut m = msg;
                        m.tab_id = id.clone();
                        match store.append_message(&m) {
                            Ok(row_id) => last_id = row_id,
                            Err(e) => eprintln!("append_message error: {e}"),
                        }
                    }
                    Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(warp::reply::json(
                        &serde_json::json!({"ok":true,"last_id":last_id}),
                    )))
                }
            },
        )
}

// PATCH /api/chat/tabs/order
fn api_reorder_tabs(
    storage: Arc<ChatStorage>,
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "chat" / "tabs" / "order")
        .and(warp::patch())
        .and(warp::header::optional::<String>("authorization"))
        .and(warp::body::json::<serde_json::Value>())
        .and(with_chat_storage(storage))
        .and_then(
            move |auth: Option<String>, body: serde_json::Value, store: Arc<ChatStorage>| {
                let cfg = app_config.clone();
                async move {
                    if !check_api_token(&auth, &cfg) {
                        return Ok(unauthorized_api_token());
                    }
                    let ids: Vec<String> = body["tab_order"]
                        .as_array()
                        .map(|a| {
                            a.iter()
                                .filter_map(|v| v.as_str().map(String::from))
                                .collect()
                        })
                        .unwrap_or_default();
                    match store.reorder_tabs(&ids) {
                        Ok(_) => Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                            warp::reply::json(&serde_json::json!({"ok":true})),
                        )),
                        Err(e) => Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                            warp::reply::json(
                                &serde_json::json!({"ok":false,"error":e.to_string()}),
                            ),
                        )),
                    }
                }
            },
        )
}

// GET /api/chat/search?q=…&limit=20&offset=0
fn api_chat_search(
    storage: Arc<ChatStorage>,
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    #[derive(serde::Deserialize)]
    struct SearchParams {
        q: String,
        #[serde(default = "default_limit")]
        limit: usize,
        #[serde(default)]
        offset: usize,
        #[serde(default)]
        visibility: String,
        /// Optional tab ID to restrict results to a single conversation.
        tab_id: Option<String>,
    }
    fn default_limit() -> usize {
        20
    }

    warp::path!("api" / "chat" / "search")
        .and(warp::get())
        .and(warp::header::optional::<String>("authorization"))
        .and(warp::query::<SearchParams>())
        .and(with_chat_storage(storage))
        .and_then(
            move |auth: Option<String>, p: SearchParams, store: Arc<ChatStorage>| {
                let cfg = app_config.clone();
                async move {
                    if !check_api_token(&auth, &cfg) {
                        return Ok(unauthorized_api_token());
                    }

                    let limit = p.limit.clamp(1, 100);
                    let visibilities = parse_visibility_param(&p.visibility);
                    let tab_id_ref = p.tab_id.as_deref();
                    match store.search(&p.q, limit, p.offset, &visibilities, tab_id_ref) {
                        Ok(results) => Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(
                            Box::new(warp::reply::json(&results)),
                        ),
                        Err(e) => {
                            eprintln!("search error: {e}");
                            Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                                warp::reply::json(&crate::chat_storage::SearchResultsPage {
                                    results: Vec::new(),
                                    total: 0,
                                    limit,
                                    offset: p.offset,
                                    has_more: false,
                                }),
                            ))
                        }
                    }
                }
            },
        )
}

fn api_archive_tab(
    storage: Arc<ChatStorage>,
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "chat" / "tabs" / String)
        .and(warp::post())
        .and(warp::path!("archive"))
        .and(warp::header::optional::<String>("authorization"))
        .and(with_chat_storage(storage))
        .and_then(
            move |id: String, auth: Option<String>, store: Arc<ChatStorage>| {
                let cfg = app_config.clone();
                async move {
                    if !check_api_token(&auth, &cfg) {
                        return Ok(unauthorized_api_token());
                    }
                    match store.set_visibility(&id, &TabVisibility::Archived) {
                        Ok(tab) => Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                            warp::reply::json(&tab),
                        )),
                        Err(_) => Ok(Box::new(warp::reply::with_status(
                            warp::reply::json(
                                &serde_json::json!({"ok": false, "error": "not_found"}),
                            ),
                            warp::http::StatusCode::NOT_FOUND,
                        ))),
                    }
                }
            },
        )
}

fn api_hide_tab(
    storage: Arc<ChatStorage>,
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "chat" / "tabs" / String)
        .and(warp::post())
        .and(warp::path!("hide"))
        .and(warp::header::optional::<String>("authorization"))
        .and(with_chat_storage(storage))
        .and_then(
            move |id: String, auth: Option<String>, store: Arc<ChatStorage>| {
                let cfg = app_config.clone();
                async move {
                    if !check_api_token(&auth, &cfg) {
                        return Ok(unauthorized_api_token());
                    }
                    match store.set_visibility(&id, &TabVisibility::Hidden) {
                        Ok(tab) => Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                            warp::reply::json(&tab),
                        )),
                        Err(_) => Ok(Box::new(warp::reply::with_status(
                            warp::reply::json(
                                &serde_json::json!({"ok": false, "error": "not_found"}),
                            ),
                            warp::http::StatusCode::NOT_FOUND,
                        ))),
                    }
                }
            },
        )
}

fn api_restore_tab(
    storage: Arc<ChatStorage>,
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "chat" / "tabs" / String)
        .and(warp::post())
        .and(warp::path!("restore"))
        .and(warp::header::optional::<String>("authorization"))
        .and(with_chat_storage(storage))
        .and_then(
            move |id: String, auth: Option<String>, store: Arc<ChatStorage>| {
                let cfg = app_config.clone();
                async move {
                    if !check_api_token(&auth, &cfg) {
                        return Ok(unauthorized_api_token());
                    }
                    match store.set_visibility(&id, &TabVisibility::Active) {
                        Ok(tab) => Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                            warp::reply::json(&tab),
                        )),
                        Err(_) => Ok(Box::new(warp::reply::with_status(
                            warp::reply::json(
                                &serde_json::json!({"ok": false, "error": "not_found"}),
                            ),
                            warp::http::StatusCode::NOT_FOUND,
                        ))),
                    }
                }
            },
        )
}

// ── Database Admin Endpoints ──────────────────────────────────────────────────

// GET /api/db/stats (requires api-token)
fn api_db_stats(
    storage: Arc<ChatStorage>,
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    let app_config = app_config.clone();

    warp::path!("api" / "db" / "stats")
        .and(warp::get())
        .and(warp::header::optional::<String>("authorization"))
        .and(with_chat_storage(storage))
        .and(with_app_config(app_config))
        .and_then(
            move |auth: Option<String>, store: Arc<ChatStorage>, cfg: Arc<AppConfig>| async move {
                let bearer = auth.and_then(|v| v.strip_prefix("Bearer ").map(str::to_string));
                let has_api_token = bearer_matches_api_token(bearer.as_deref(), &cfg);

                if !has_api_token {
                    return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                        warp::reply::with_status(
                            warp::reply::json(&serde_json::json!({ "error": "unauthorized" })),
                            warp::http::StatusCode::UNAUTHORIZED,
                        ),
                    ));
                }

                match store.database_stats() {
                    Ok(stats) => Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                        warp::reply::json(&stats),
                    )),
                    Err(e) => {
                        eprintln!("db stats error: {e}");
                        Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                            warp::reply::json(&serde_json::json!({"error": e.to_string()})),
                        ))
                    }
                }
            },
        )
}

// GET /api/db/integrity (requires api-token)
fn api_db_integrity(
    storage: Arc<ChatStorage>,
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    let app_config = app_config.clone();

    warp::path!("api" / "db" / "integrity")
        .and(warp::get())
        .and(warp::header::optional::<String>("authorization"))
        .and(with_chat_storage(storage))
        .and(with_app_config(app_config))
        .and_then(
            move |auth: Option<String>, store: Arc<ChatStorage>, cfg: Arc<AppConfig>| async move {
                let bearer = auth.and_then(|v| v.strip_prefix("Bearer ").map(str::to_string));
                let has_api_token = bearer_matches_api_token(bearer.as_deref(), &cfg);

                if !has_api_token {
                    return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                        warp::reply::with_status(
                            warp::reply::json(&serde_json::json!({ "error": "unauthorized" })),
                            warp::http::StatusCode::UNAUTHORIZED,
                        ),
                    ));
                }

                match store.integrity_check() {
                    Ok(result) => {
                        let status = if result == "ok" {
                            "healthy"
                        } else {
                            "corrupted"
                        };
                        Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                            warp::reply::json(&serde_json::json!({
                                "status": status,
                                "detail": result,
                            })),
                        ))
                    }
                    Err(e) => {
                        eprintln!("integrity check error: {e}");
                        Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                            warp::reply::json(&serde_json::json!({"error": e.to_string()})),
                        ))
                    }
                }
            },
        )
}

// POST /api/db/maintenance - Run maintenance operations (requires api-token)
fn api_db_maintenance(
    storage: Arc<ChatStorage>,
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    #[derive(serde::Deserialize)]
    struct MaintenanceRequest {
        operation: String,
    }

    let app_config = app_config.clone();

    warp::path!("api" / "db" / "maintenance")
        .and(warp::post())
        .and(warp::header::optional::<String>("authorization"))
        .and(warp::body::json::<MaintenanceRequest>())
        .and(with_chat_storage(storage))
        .and(with_app_config(app_config))
        .and_then(
            move |auth: Option<String>, req: MaintenanceRequest, store: Arc<ChatStorage>, cfg: Arc<AppConfig>| {
                async move {
                    // Require api-token
                    let bearer = auth.and_then(|v| v.strip_prefix("Bearer ").map(str::to_string));
                    let has_api_token =
                        bearer_matches_api_token(bearer.as_deref(), &cfg);

                    if !has_api_token {
                        return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                            warp::reply::with_status(
                                warp::reply::json(&serde_json::json!({ "error": "unauthorized" })),
                                warp::http::StatusCode::UNAUTHORIZED,
                            ),
                        ));
                    }

                    let result = match req.operation.as_str() {
                        "checkpoint" => store.checkpoint().map(
                            |(a, b, c)| serde_json::json!({"backfilled": a, "deleted": b, "log": c}),
                        ),
                        "vacuum" => store
                            .vacuum()
                            .map(|_| serde_json::json!({"status": "vacuumed"})),
                        "rebuild_fts" => store
                            .rebuild_fts_index()
                            .map(|_| serde_json::json!({"status": "fts_rebuilt"})),
                        "analyze" => store
                            .analyze()
                            .map(|_| serde_json::json!({"status": "analyzed"})),
                        _ => Err(anyhow::anyhow!("Unknown operation: {}", req.operation)),
                    };

                    match result {
                        Ok(response) => Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(
                            Box::new(warp::reply::json(&response)),
                        ),
                        Err(e) => {
                            eprintln!("maintenance error: {e}");
                            Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                                warp::reply::json(
                                    &serde_json::json!({"error": e.to_string()}),
                                ),
                            ))
                        }
                    }
                }
            },
        )
}

// POST /api/db/backup - Create database backup (requires api-token)
fn api_db_backup(
    storage: Arc<ChatStorage>,
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    static LAST_DB_BACKUP: AtomicU64 = AtomicU64::new(0);

    warp::path!("api" / "db" / "backup")
        .and(warp::post())
        .and(warp::header::optional::<String>("authorization"))
        .and(with_chat_storage(storage))
        .and(with_app_config(app_config))
        .and_then(
            move |auth: Option<String>, store: Arc<ChatStorage>, cfg: Arc<AppConfig>| {
                let cfg = cfg.clone();
                async move {
                    let bearer = auth.and_then(|v| v.strip_prefix("Bearer ").map(str::to_string));

                    let has_api_token =
                        bearer_matches_api_token(bearer.as_deref(), &cfg);

                    if !has_api_token {
                        return Ok::<_, warp::Rejection>(warp::reply::with_status(
                            warp::reply::json(&serde_json::json!({ "error": "unauthorized" })),
                            warp::http::StatusCode::UNAUTHORIZED,
                        ));
                    }

                    let now = SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs();
                    let last = LAST_DB_BACKUP.load(Ordering::Acquire);
                    if now - last < 10 {
                        let remaining = 10 - (now - last);
                        return Ok::<_, warp::Rejection>(warp::reply::with_status(
                            warp::reply::json(&serde_json::json!({
                                "ok": false,
                                "error": "too soon; please wait",
                                "seconds_remaining": remaining
                            })),
                            warp::http::StatusCode::TOO_MANY_REQUESTS,
                        ));
                    }
                    LAST_DB_BACKUP.store(now, Ordering::Release);

                    let config_dir = cfg.config_dir.clone();
                    // Manual backups live in their own subdirectory, separate from auto backups.
                    let backup_dir = config_dir.join("backups").join("manual");

                    if let Err(e) = std::fs::create_dir_all(&backup_dir) {
                        eprintln!("Failed to create manual backup directory: {e}");
                        return Ok::<_, warp::Rejection>(warp::reply::with_status(
                            warp::reply::json(&serde_json::json!({"error": e.to_string()})),
                            warp::http::StatusCode::OK,
                        ));
                    }

                    let timestamp = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .map(|d| d.as_millis().to_string())
                        .unwrap_or_else(|_| "0".to_string());
                    let backup_path = backup_dir.join(format!("chat_{}.db", timestamp));

                    match store.backup(&backup_path) {
                        Ok(()) => {
                            let file_size = std::fs::metadata(&backup_path)
                                .ok()
                                .map(|m| m.len())
                                .unwrap_or(0);

                            // Keep the last 7 manual backups
                            if let Ok(entries) = std::fs::read_dir(&backup_dir) {
                                let mut backups: Vec<_> = entries
                                    .filter_map(|e| e.ok())
                                    .filter(|e| {
                                        e.file_name().to_string_lossy().starts_with("chat_")
                                    })
                                    .collect();
                                backups.sort_by_key(|e| e.path());
                                while backups.len() > 7 {
                                    let old = backups.remove(0);
                                    let _ = std::fs::remove_file(old.path());
                                }
                            }

                            Ok::<_, warp::Rejection>(warp::reply::with_status(
                                warp::reply::json(&serde_json::json!({
                                    "status": "backup_created",
                                    "name": format!("manual/{}", backup_path.file_name().unwrap_or_default().to_string_lossy()),
                                    "size_bytes": file_size,
                                })),
                                warp::http::StatusCode::OK,
                            ))
                        }
                        Err(e) => {
                            eprintln!("backup error: {e}");
                            Ok::<_, warp::Rejection>(warp::reply::with_status(
                                warp::reply::json(&serde_json::json!({"error": e.to_string()})),
                                warp::http::StatusCode::OK,
                            ))
                        }
                    }
                }
            },
        )
}

// GET /api/db/indexes - List database indexes
// GET /api/db/indexes (requires api-token)
fn api_db_indexes(
    storage: Arc<ChatStorage>,
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    let app_config = app_config.clone();

    warp::path!("api" / "db" / "indexes")
        .and(warp::get())
        .and(warp::header::optional::<String>("authorization"))
        .and(with_chat_storage(storage))
        .and(with_app_config(app_config))
        .and_then(
            move |auth: Option<String>, store: Arc<ChatStorage>, cfg: Arc<AppConfig>| async move {
                let bearer = auth.and_then(|v| v.strip_prefix("Bearer ").map(str::to_string));
                let has_api_token = bearer_matches_api_token(bearer.as_deref(), &cfg);

                if !has_api_token {
                    return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                        warp::reply::with_status(
                            warp::reply::json(&serde_json::json!({ "error": "unauthorized" })),
                            warp::http::StatusCode::UNAUTHORIZED,
                        ),
                    ));
                }

                match store.list_indexes() {
                    Ok(indexes) => Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                        warp::reply::json(&indexes),
                    )),
                    Err(e) => {
                        eprintln!("list indexes error: {e}");
                        Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                            warp::reply::json(&serde_json::json!({"error": e.to_string()})),
                        ))
                    }
                }
            },
        )
}

/// Token bootstrap policy:
/// - When any auth mode is configured, the surrounding auth guard already
///   authenticated the request, so bootstrap is allowed.
/// - With no auth configured, bootstrap is restricted to loopback binds.
fn bind_host_is_loopback(bind_host: &str) -> bool {
    let host = bind_host.trim().trim_matches(['[', ']']);
    matches!(host, "localhost" | "127.0.0.1" | "::1")
}

fn token_bootstrap_allowed(auth_manager: &AuthManager, bind_host: &str) -> bool {
    // No Auth mode: fully open (local-first).
    if !auth_manager.has_any() {
        return true;
    }
    // Auth configured: only allow bootstrap when bound to loopback.
    // Do NOT trust the Host header — it is attacker-controlled.
    bind_host_is_loopback(bind_host)
}

// GET /api/internal/api-token - Return internal API token for UI use
fn api_internal_token(
    app_config: Arc<AppConfig>,
    auth_manager: AuthManager,
    bind_host: String,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "internal" / "api-token")
        .and(warp::get())
        .and(warp::header::optional::<String>("authorization"))
        .and(warp::header::optional::<String>("cookie"))
        .and(with_app_config(app_config))
        .map(
            move |auth: Option<String>, cookie: Option<String>, cfg: Arc<AppConfig>| {
                let already_authenticated = auth_manager
                    .authenticate_request(auth.as_deref(), cookie.as_deref())
                    || check_api_token(&auth, &cfg);
                if !already_authenticated && !token_bootstrap_allowed(&auth_manager, &bind_host) {
                    return Box::new(warp::reply::with_status(
                        warp::reply::json(&serde_json::json!({ "error": "forbidden" })),
                        warp::http::StatusCode::FORBIDDEN,
                    )) as Box<dyn warp::reply::Reply>;
                }
                let live = cfg.live_api_token();
                let token = live.as_deref().unwrap_or("");
                Box::new(warp::reply::json(&serde_json::json!({ "token": token })))
            },
        )
}

// GET /api/db/admin-token - Return DB admin token for authenticated UI use
fn api_db_admin_token(
    app_config: Arc<AppConfig>,
    auth_manager: AuthManager,
    bind_host: String,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "db" / "admin-token")
        .and(warp::get())
        .and(warp::header::optional::<String>("authorization"))
        .and(warp::header::optional::<String>("cookie"))
        .and(with_app_config(app_config))
        .map(
            move |auth: Option<String>, cookie: Option<String>, cfg: Arc<AppConfig>| {
                let already_authenticated = auth_manager
                    .authenticate_request(auth.as_deref(), cookie.as_deref())
                    || check_api_token(&auth, &cfg);
                if !already_authenticated && !token_bootstrap_allowed(&auth_manager, &bind_host) {
                    return Box::new(warp::reply::with_status(
                        warp::reply::json(&serde_json::json!({ "error": "forbidden" })),
                        warp::http::StatusCode::FORBIDDEN,
                    )) as Box<dyn warp::reply::Reply>;
                }
                let live = cfg.live_db_admin_token();
                let token = live.as_deref().unwrap_or("");
                Box::new(warp::reply::json(&serde_json::json!({ "token": token })))
            },
        )
}

// POST /api/db/query - Execute admin query (SELECT only)
fn api_db_query(
    storage: Arc<ChatStorage>,
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    #[derive(serde::Deserialize)]
    struct QueryRequest {
        sql: String,
    }

    let storage = storage.clone();
    let app_config = app_config.clone();

    warp::path!("api" / "db" / "query")
        .and(warp::post())
        .and(warp::body::content_length_limit(256 * 1024))
        .and(warp::header::optional::<String>("authorization"))
        .and(warp::body::json::<QueryRequest>())
        .and(with_chat_storage(storage))
        .and(with_app_config(app_config))
        .and_then(
            move |auth: Option<String>,
                  req: QueryRequest,
                  store: Arc<ChatStorage>,
                  cfg: Arc<AppConfig>| {
                let bearer = auth.and_then(|v| v.strip_prefix("Bearer ").map(str::to_string));

                let store_clone = store.clone();
                async move {
                    // Accept either api-token or db-admin-token; admin mode iff db-admin-token used.
                    let has_api_token = bearer_matches_api_token(bearer.as_deref(), &cfg);
                    let is_admin = bearer_matches_db_admin_token(bearer.as_deref(), &cfg);

                    if !has_api_token && !is_admin {
                        return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                            warp::reply::with_status(
                                warp::reply::json(&serde_json::json!({ "error": "unauthorized" })),
                                warp::http::StatusCode::UNAUTHORIZED,
                            ),
                        ));
                    }

                    // SQL length cap: 16KB
                    if req.sql.len() > 16_000 {
                        return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                            warp::reply::with_status(
                                warp::reply::json(
                                    &serde_json::json!({ "error": "query too long" }),
                                ),
                                warp::http::StatusCode::BAD_REQUEST,
                            ),
                        ));
                    }

                    let store = store_clone.clone();
                    let sql = req.sql.clone();
                    let result =
                        tokio::time::timeout(std::time::Duration::from_secs(10), async move {
                            store.execute_query(&sql, is_admin)
                        })
                        .await;

                    match result {
                        Ok(Ok(result)) => Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(
                            Box::new(warp::reply::with_status(
                                warp::reply::json(&result),
                                warp::http::StatusCode::OK,
                            )),
                        ),
                        Ok(Err(e)) => {
                            eprintln!("query error: {e}");
                            Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                                warp::reply::with_status(
                                    warp::reply::json(&serde_json::json!({"error": e.to_string()})),
                                    warp::http::StatusCode::OK,
                                ),
                            ))
                        }
                        Err(_) => {
                            eprintln!("query timeout");
                            Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                                warp::reply::with_status(
                                    warp::reply::json(
                                        &serde_json::json!({"error": "query timed out"}),
                                    ),
                                    warp::http::StatusCode::REQUEST_TIMEOUT,
                                ),
                            ))
                        }
                    }
                }
            },
        )
}

// GET /api/db/backups - List available backups (requires api-token)
fn api_db_backups(
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "db" / "backups")
        .and(warp::get())
        .and(warp::header::optional::<String>("authorization"))
        .and(with_app_config(app_config))
        .and_then(move |auth: Option<String>, cfg: Arc<AppConfig>| {
            let cfg = cfg.clone();
            async move {
                let bearer = auth.and_then(|v| v.strip_prefix("Bearer ").map(str::to_string));

                let has_api_token = bearer_matches_api_token(bearer.as_deref(), &cfg);

                if !has_api_token {
                    return Ok::<_, warp::Rejection>(warp::reply::with_status(
                        warp::reply::json(&serde_json::json!({ "error": "unauthorized" })),
                        warp::http::StatusCode::UNAUTHORIZED,
                    ));
                }

                let backups_root = cfg.config_dir.join("backups");

                let mut backups = Vec::new();
                let mut total_size = 0u64;

                // Scan auto/, daily/, and manual/ subdirectories.
                for (kind, subdir) in [("auto", "auto"), ("daily", "daily"), ("manual", "manual")] {
                    let dir = backups_root.join(subdir);
                    if let Ok(entries) = std::fs::read_dir(&dir) {
                        for entry in entries.filter_map(|e| e.ok()) {
                            if let Ok(metadata) = entry.metadata()
                                && metadata.is_file()
                            {
                                let filename = entry.file_name().to_string_lossy().to_string();
                                if !filename.ends_with(".db") {
                                    continue;
                                }
                                let size = metadata.len();
                                total_size += size;
                                let modified = metadata
                                    .modified()
                                    .ok()
                                    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                                    .map(|d| d.as_millis() as i64)
                                    .unwrap_or(0);
                                // Name includes the subdirectory so restore/delete can resolve it.
                                backups.push(serde_json::json!({
                                    "name": format!("{}/{}", subdir, filename),
                                    "kind": kind,
                                    "size": size,
                                    "modified": modified,
                                }));
                            }
                        }
                    }
                }

                backups.sort_by_key(|b| b["modified"].as_i64().unwrap_or(0));
                backups.reverse();

                Ok::<_, warp::Rejection>(warp::reply::with_status(
                    warp::reply::json(&serde_json::json!({
                        "backups": backups,
                        "total_size": total_size,
                    })),
                    warp::http::StatusCode::OK,
                ))
            }
        })
}

// POST /api/db/restore - Restore from backup (requires db-admin-token)
fn api_db_restore(
    storage: Arc<ChatStorage>,
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    #[derive(serde::Deserialize)]
    struct RestoreRequest {
        backup_name: String,
    }

    warp::path!("api" / "db" / "restore")
        .and(warp::post())
        .and(warp::header::optional::<String>("authorization"))
        .and(warp::body::json::<RestoreRequest>())
        .and(with_chat_storage(storage))
        .and(with_app_config(app_config))
        .and_then(
            move |auth: Option<String>,
                  req: RestoreRequest,
                  store: Arc<ChatStorage>,
                  cfg: Arc<AppConfig>| {
                use std::sync::atomic::AtomicU64;
                use std::time::{SystemTime, UNIX_EPOCH};

                static LAST_DB_RESTORE: AtomicU64 = AtomicU64::new(0);

                let cfg = cfg.clone();
                async move {
                    let bearer = auth
                        .and_then(|v| v.strip_prefix("Bearer ").map(str::to_string));

                    // Require db-admin-token for restore (high-impact operation)
                    let has_admin_token =
                        bearer_matches_db_admin_token(bearer.as_deref(), &cfg);

                    if !has_admin_token {
                        return Ok::<_, warp::Rejection>(warp::reply::with_status(
                            warp::reply::json(&serde_json::json!({ "error": "unauthorized" })),
                            warp::http::StatusCode::UNAUTHORIZED,
                        ));
                    }

                    let now = SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs();
                    let (ok, remaining) = try_cooldown(&LAST_DB_RESTORE, now, 30);
                    if !ok {
                        return Ok::<_, warp::Rejection>(warp::reply::with_status(
                            warp::reply::json(&serde_json::json!({
                                "ok": false,
                                "error": "too soon; please wait",
                                "seconds_remaining": remaining
                            })),
                            warp::http::StatusCode::TOO_MANY_REQUESTS,
                        ));
                    }

                    // Validate backup_name (prevent directory traversal)
                    let backup_name = req.backup_name.trim();
                    if backup_name.is_empty()
                        || backup_name.contains("..")
                        || backup_name.starts_with('/')
                        || backup_name.contains('\\')
                    {
                        return Ok::<_, warp::Rejection>(warp::reply::with_status(
                            warp::reply::json(&serde_json::json!({ "error": "invalid backup name" })),
                            warp::http::StatusCode::BAD_REQUEST,
                        ));
                    }

                    let backup_dir = cfg.config_dir.join("backups");
                    let backup_path = backup_dir.join(backup_name);

                    // Ensure resolved path is within backup_dir
                    if matches!(
                        (backup_path.canonicalize(), backup_dir.canonicalize()),
                        (Ok(ref canonical), Ok(ref base)) if !canonical.starts_with(base)
                    ) {
                        return Ok::<_, warp::Rejection>(warp::reply::with_status(
                            warp::reply::json(&serde_json::json!({ "error": "path not allowed" })),
                            warp::http::StatusCode::BAD_REQUEST,
                        ));
                    }

                    if !backup_path.exists() {
                        return Ok::<_, warp::Rejection>(warp::reply::with_status(
                            warp::reply::json(&serde_json::json!({
                                "error": format!("Backup not found: {}", backup_name)
                            })),
                            warp::http::StatusCode::OK,
                        ));
                    }

                    // Safety backup (via SQLite backup API) so the live connection
                    // is used safely — no raw fs::copy on an open database.
                    let manual_dir = cfg.config_dir.join("backups").join("manual");
                    let _ = std::fs::create_dir_all(&manual_dir);
                    let safety_backup = manual_dir.join(format!(
                        "pre_restore_{}.db",
                        std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .map(|d| d.as_millis().to_string())
                            .unwrap_or_else(|_| "0".to_string())
                    ));
                    let _ = store.backup(&safety_backup);

                    // Atomically close connection, swap in backup file, reopen.
                    match store.restore_from_path(&backup_path) {
                        Ok(()) => {
                            // Verify the restored database
                            match store.integrity_check() {
                                Ok(_) => {
                                    Ok::<_, warp::Rejection>(warp::reply::with_status(
                                        warp::reply::json(&serde_json::json!({
                                            "status": "restored",
                                            "backup": backup_name,
                                        })),
                                        warp::http::StatusCode::OK,
                                    ))
                                }
                                Err(e) => {
                                    eprintln!("Restored database integrity check failed: {e}");
                                    Ok::<_, warp::Rejection>(warp::reply::with_status(
                                        warp::reply::json(&serde_json::json!({
                                            "error": "Restore succeeded but integrity check failed",
                                            "safety_backup": safety_backup.to_string_lossy().to_string(),
                                        })),
                                        warp::http::StatusCode::OK,
                                    ))
                                }
                            }
                        }
                        Err(e) => {
                            eprintln!("Restore error: {e}");
                            Ok::<_, warp::Rejection>(warp::reply::with_status(
                                warp::reply::json(&serde_json::json!({"error": e.to_string()})),
                                warp::http::StatusCode::OK,
                            ))
                        }
                    }
                }
            },
        )
}

// POST /api/db/repair - Database repair operations (requires db-admin-token)
fn api_db_repair(
    storage: Arc<ChatStorage>,
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    #[derive(serde::Deserialize)]
    struct RepairRequest {
        operation: String,
    }

    let app_config = app_config.clone();

    warp::path!("api" / "db" / "repair")
        .and(warp::post())
        .and(warp::header::optional::<String>("authorization"))
        .and(warp::body::json::<RepairRequest>())
        .and(with_chat_storage(storage))
        .and(with_app_config(app_config))
        .and_then(
            move |auth: Option<String>,
                  req: RepairRequest,
                  store: Arc<ChatStorage>,
                  cfg: Arc<AppConfig>| {
                async move {
                    // Require db-admin-token
                    let bearer = auth.and_then(|v| v.strip_prefix("Bearer ").map(str::to_string));
                    let has_db_admin_token = bearer_matches_db_admin_token(bearer.as_deref(), &cfg);

                    if !has_db_admin_token {
                        return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                            warp::reply::with_status(
                                warp::reply::json(&serde_json::json!({ "error": "unauthorized" })),
                                warp::http::StatusCode::UNAUTHORIZED,
                            ),
                        ));
                    }

                    match req.operation.as_str() {
                        "repair_indexes" => match store.repair_indexes() {
                            Ok(_) => Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                                warp::reply::json(&serde_json::json!({
                                    "status": "indexes_repaired",
                                })),
                            )),
                            Err(e) => {
                                eprintln!("Repair indexes error: {e}");
                                Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                                    warp::reply::json(&serde_json::json!({"error": e.to_string()})),
                                ))
                            }
                        },
                        "emergency_recovery" => match store.emergency_recovery() {
                            Ok(_) => Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                                warp::reply::json(&serde_json::json!({
                                    "status": "recovery_attempted",
                                })),
                            )),
                            Err(e) => {
                                eprintln!("Emergency recovery error: {e}");
                                Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                                    warp::reply::json(&serde_json::json!({"error": e.to_string()})),
                                ))
                            }
                        },
                        _ => Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                            warp::reply::json(&serde_json::json!({
                                "error": format!("Unknown repair operation: {}", req.operation)
                            })),
                        )),
                    }
                }
            },
        )
}

// DELETE /api/db/backup - Delete a specific backup (requires db-admin-token)
fn api_db_delete_backup(
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    #[derive(serde::Deserialize)]
    struct DeleteBackupRequest {
        backup_name: String,
    }

    warp::path!("api" / "db" / "backup")
        .and(warp::delete())
        .and(warp::header::optional::<String>("authorization"))
        .and(warp::body::json::<DeleteBackupRequest>())
        .and(with_app_config(app_config))
        .and_then(
            move |auth: Option<String>, req: DeleteBackupRequest, cfg: Arc<AppConfig>| {
                let cfg = cfg.clone();
                async move {
                    let bearer = auth.and_then(|v| v.strip_prefix("Bearer ").map(str::to_string));

                    // Require db-admin-token for delete (high-impact operation)
                    let has_admin_token = bearer_matches_db_admin_token(bearer.as_deref(), &cfg);

                    if !has_admin_token {
                        return Ok::<_, warp::Rejection>(warp::reply::with_status(
                            warp::reply::json(&serde_json::json!({ "error": "unauthorized" })),
                            warp::http::StatusCode::UNAUTHORIZED,
                        ));
                    }

                    // Validate backup_name (prevent directory traversal)
                    let backup_name = req.backup_name.trim();
                    if backup_name.is_empty()
                        || backup_name.contains("..")
                        || backup_name.starts_with('/')
                        || backup_name.contains('\\')
                    {
                        return Ok::<_, warp::Rejection>(warp::reply::with_status(
                            warp::reply::json(
                                &serde_json::json!({ "error": "invalid backup name" }),
                            ),
                            warp::http::StatusCode::BAD_REQUEST,
                        ));
                    }

                    let backup_dir = cfg.config_dir.join("backups");
                    let backup_path = backup_dir.join(backup_name);

                    // Ensure resolved path is within backup_dir
                    if matches!(
                        (backup_path.canonicalize(), backup_dir.canonicalize()),
                        (Ok(ref canonical), Ok(ref base)) if !canonical.starts_with(base)
                    ) {
                        return Ok::<_, warp::Rejection>(warp::reply::with_status(
                            warp::reply::json(&serde_json::json!({ "error": "path not allowed" })),
                            warp::http::StatusCode::BAD_REQUEST,
                        ));
                    }

                    if !backup_path.exists() {
                        return Ok::<_, warp::Rejection>(warp::reply::with_status(
                            warp::reply::json(&serde_json::json!({
                                "error": format!("Backup not found: {}", backup_name)
                            })),
                            warp::http::StatusCode::OK,
                        ));
                    }

                    match std::fs::remove_file(&backup_path) {
                        Ok(_) => Ok::<_, warp::Rejection>(warp::reply::with_status(
                            warp::reply::json(&serde_json::json!({
                                "status": "deleted",
                                "backup": backup_name,
                            })),
                            warp::http::StatusCode::OK,
                        )),
                        Err(e) => {
                            eprintln!("Delete backup error: {e}");
                            Ok::<_, warp::Rejection>(warp::reply::with_status(
                                warp::reply::json(&serde_json::json!({"error": e.to_string()})),
                                warp::http::StatusCode::OK,
                            ))
                        }
                    }
                }
            },
        )
}

fn api_get_sessions(
    state: AppState,
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    let app_config = app_config.clone();
    warp::path!("api" / "sessions")
        .and(warp::path::end())
        .and(warp::get())
        .and(warp::header::optional::<String>("authorization"))
        .and(with_app_config(app_config))
        .and_then(move |auth: Option<String>, cfg: Arc<AppConfig>| {
            let state = state.clone();
            async move {
                if !check_api_token(&auth, &cfg) {
                    return Ok(unauthorized_api_token());
                }
                let sessions = state.get_sessions();
                Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(warp::reply::json(
                    &sessions,
                )))
            }
        })
}

fn api_get_recent_sessions(
    state: AppState,
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    let app_config = app_config.clone();
    warp::path!("api" / "sessions" / "recent")
        .and(warp::path::end())
        .and(warp::get())
        .and(warp::header::optional::<String>("authorization"))
        .and(with_app_config(app_config))
        .and_then(move |auth: Option<String>, cfg: Arc<AppConfig>| {
            let state = state.clone();
            async move {
                if !check_api_token(&auth, &cfg) {
                    return Ok(unauthorized_api_token());
                }
                let mut sessions = state.get_sessions();
                // Sort by last_connected_at descending
                sessions.sort_by_key(|s| std::cmp::Reverse(s.last_connected_at));
                // Limit to 10
                sessions.truncate(10);
                let active_id = state.active_session_id.lock().unwrap().clone();
                Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(warp::reply::json(
                    &serde_json::json!({
                        "sessions": sessions,
                        "active_session_id": active_id
                    }),
                )))
            }
        })
}

fn api_check_endpoint_health(
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "sessions" / "check-endpoint")
        .and(warp::path::end())
        .and(warp::get())
        .and(warp::header::optional::<String>("authorization"))
        .and(warp::query::<std::collections::HashMap<String, String>>())
        .and(with_app_config(app_config))
        .and_then(
            move |auth: Option<String>,
                  params: std::collections::HashMap<String, String>,
                  cfg: Arc<AppConfig>| async move {
                if !check_api_token(&auth, &cfg) {
                    return Ok(unauthorized_api_token());
                }
                let url = match params.get("url") {
                    Some(u) if !u.is_empty() => u.clone(),
                    _ => {
                        return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                            warp::reply::with_status(
                                warp::reply::json(&serde_json::json!({"error": "missing url"})),
                                warp::http::StatusCode::BAD_REQUEST,
                            ),
                        ));
                    }
                };
                // Proxy health check: try /health endpoint server-side
                let health_url = format!("{}/health", url.trim_end_matches('/'));
                let client = reqwest::Client::builder()
                    .timeout(std::time::Duration::from_secs(4))
                    .build()
                    .unwrap_or_default();
                let reachable = client
                    .get(&health_url)
                    .send()
                    .await
                    .map(|r| r.status().as_u16() < 500)
                    .unwrap_or(false);
                Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(warp::reply::json(
                    &serde_json::json!({ "reachable": reachable }),
                )))
            },
        )
}

fn api_create_session(
    state: AppState,
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    let app_config = app_config.clone();
    warp::path!("api" / "sessions")
        .and(warp::path::end())
        .and(warp::post())
        .and(warp::header::optional::<String>("authorization"))
        .and(warp::body::json())
        .and(with_app_config(app_config))
        .and_then(move |auth: Option<String>, session: app_state::Session, cfg: Arc<AppConfig>| {
            let state = state.clone();
            async move {
                if !check_api_token(&auth, &cfg) {
                    return Ok(unauthorized_api_token());
                }
                if state.add_session(session) {
                    Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(
                        Box::new(warp::reply::json(&serde_json::json!({"ok": true}))),
                    )
                } else {
                    Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(
                        Box::new(warp::reply::json(
                            &serde_json::json!({"ok": false, "error": "Maximum sessions reached"}),
                        )),
                    )
                }
            }
        })
}

fn api_delete_session(
    state: AppState,
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    static LAST_DELETE_SESSION: AtomicU64 = AtomicU64::new(0);

    let app_config = app_config.clone();
    warp::path!("api" / "sessions" / String)
        .and(warp::path::end())
        .and(warp::delete())
        .and(warp::header::optional::<String>("authorization"))
        .and(with_app_config(app_config))
        .and_then(
            move |session_id: String, auth: Option<String>, cfg: Arc<AppConfig>| {
                let state = state.clone();
                async move {
                    if !check_db_admin_token(&auth, &cfg) {
                        return Ok(unauthorized_db_admin_token());
                    }

                    let now = SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs();
                    let last = LAST_DELETE_SESSION.load(Ordering::Acquire);
                    if now - last < 5 {
                        let remaining = 5 - (now - last);
                        return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                            warp::reply::with_status(
                                warp::reply::json(&serde_json::json!({
                                    "ok": false,
                                    "error": "too soon; please wait",
                                    "seconds_remaining": remaining
                                })),
                                warp::http::StatusCode::TOO_MANY_REQUESTS,
                            ),
                        ));
                    }
                    LAST_DELETE_SESSION.store(now, Ordering::Release);

                    if state.remove_session(&session_id) {
                        Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                            warp::reply::json(&serde_json::json!({"ok": true})),
                        ))
                    } else {
                        Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                            warp::reply::json(
                                &serde_json::json!({"ok": false, "error": "Session not found"}),
                            ),
                        ))
                    }
                }
            },
        )
}

fn api_get_active_session(
    state: AppState,
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    let app_config = app_config.clone();
    warp::path!("api" / "sessions" / "active")
        .and(warp::path::end())
        .and(warp::get())
        .and(warp::header::optional::<String>("authorization"))
        .and(with_app_config(app_config))
        .and_then(move |auth: Option<String>, cfg: Arc<AppConfig>| {
            let state = state.clone();
            async move {
                if !check_api_token(&auth, &cfg) {
                    return Ok(unauthorized_api_token());
                }
                let session_id = state.active_session_id.lock().unwrap().clone();
                let sessions = state.sessions.lock().unwrap();
                let session = sessions.iter().find(|s| s.id == session_id).cloned();
                drop(sessions);

                match session {
                    Some(s) => {
                        let mode_str = match s.mode {
                            crate::state::SessionMode::Spawn { port, .. } => {
                                format!("Spawn:{}", port)
                            }
                            crate::state::SessionMode::Attach { endpoint, .. } => {
                                format!("Attach:{}", endpoint)
                            }
                        };
                        Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                            warp::reply::json(&serde_json::json!({
                                "id": s.id,
                                "name": s.name,
                                "mode": mode_str,
                                "status": s.status,
                                "last_active": s.last_active
                            })),
                        ))
                    }
                    None => Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                        warp::reply::json(&serde_json::json!({"error": "No active session"})),
                    )),
                }
            }
        })
}

fn api_get_active_session_readiness(
    state: AppState,
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "sessions" / "active" / "readiness")
        .and(warp::path::end())
        .and(warp::get())
        .and(warp::header::optional::<String>("authorization"))
        .and(with_app_config(app_config))
        .and_then(move |auth: Option<String>, cfg: Arc<AppConfig>| {
            let state = state.clone();
            async move {
                if !check_api_token(&auth, &cfg) {
                    return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                        unauthorized_api_token(),
                    ));
                }

                let Some(session) = state.get_active_session() else {
                    return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                        warp::reply::json(
                            &serde_json::json!({"ok": false, "ready": false, "error": "No active session"}),
                        ),
                    ));
                };

                let (endpoint, api_key) = match session.mode {
                    crate::state::SessionMode::Spawn { port, api_key, .. } => {
                        (format!("http://127.0.0.1:{port}"), api_key)
                    }
                    crate::state::SessionMode::Attach { endpoint, api_key } => (endpoint, api_key),
                };

                let client = reqwest::Client::builder()
                    .timeout(std::time::Duration::from_secs(2))
                    .build()
                    .map_err(|e| warp::reject::custom(ApiError::internal(e.to_string())))?;

                let with_auth = |mut req: reqwest::RequestBuilder| {
                    if let Some(key) = &api_key {
                        req = req.header("Authorization", format!("Bearer {}", key));
                    }
                    req
                };

                let root_ok = with_auth(client.get(&endpoint)).send().await.is_ok();
                let health_ok = with_auth(client.get(format!("{endpoint}/health")))
                    .send()
                    .await
                    .is_ok();
                let ready = root_ok || health_ok;

                Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(warp::reply::json(
                    &serde_json::json!({
                        "ok": true,
                        "ready": ready,
                        "endpoint": endpoint,
                        "status": session.status,
                    }),
                )))
            }
        })
}

fn api_get_capabilities(
    state: AppState,
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "capabilities")
        .and(warp::path::end())
        .and(warp::get())
        .and(warp::header::optional::<String>("authorization"))
        .and(with_app_config(app_config))
        .and_then(move |auth: Option<String>, cfg: Arc<AppConfig>| {
            let state = state.clone();
            async move {
                if !check_api_token(&auth, &cfg) {
                    return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(
                        unauthorized_api_token(),
                    );
                }
                let capabilities = state.calculate_capabilities();
                let endpoint_kind = state.current_endpoint_kind();
                let session_kind = state.current_session_kind();
                let tray_mode = state.tray_mode.lock().unwrap().clone();

                let (system_reason, gpu_reason, cpu_temp_reason) =
                    state.calculate_availability_reasons();

                Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(warp::reply::json(
                    &serde_json::json!({
                        "capabilities": capabilities,
                        "endpoint_kind": endpoint_kind,
                        "session_kind": session_kind,
                        "tray_mode": tray_mode,
                        "availability": {
                            "system": system_reason,
                            "gpu": gpu_reason,
                            "cpu_temp": cpu_temp_reason
                        }
                    }),
                )))
            }
        })
}

fn api_set_active_session(
    state: AppState,
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    let app_config = app_config.clone();
    warp::path!("api" / "sessions" / "active")
        .and(warp::path::end())
        .and(warp::post())
        .and(warp::header::optional::<String>("authorization"))
        .and(warp::body::json())
        .and(with_app_config(app_config))
        .and_then(move |auth: Option<String>, payload: serde_json::Value, cfg: Arc<AppConfig>| {
            let state = state.clone();
            async move {
                if !check_api_token(&auth, &cfg) {
                    return Ok(unauthorized_api_token());
                }
                let session_id = match payload.get("id") {
                    Some(v) => v.as_str().unwrap_or("").to_string(),
                    None => {
                        return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(warp::reply::json(
                            &serde_json::json!({"ok": false, "error": "Missing session id"}),
                        )));
                    }
                };
                if state.set_active_session(&session_id) {
                    Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(
                        Box::new(warp::reply::json(&serde_json::json!({"ok": true}))),
                    )
                } else {
                    Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(
                        Box::new(warp::reply::json(
                            &serde_json::json!({"ok": false, "error": "Session not found"}),
                        )),
                    )
                }
            }
        })
}

fn api_spawn_session_with_preset(
    state: AppState,
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    static LAST_SPAWN_SESSION: AtomicU64 = AtomicU64::new(0);

    let app_config_inner = app_config.clone();
    warp::path!("api" / "sessions" / "spawn")
        .and(warp::path::end())
        .and(warp::post())
        .and(warp::header::optional::<String>("authorization"))
        .and(warp::body::json())
        .and(with_app_config(app_config))
        .and_then(move |auth: Option<String>, payload: serde_json::Value, cfg: Arc<AppConfig>| {
            let state = state.clone();
            let app_config = app_config_inner.clone();
            async move {
                if !check_db_admin_token(&auth, &cfg) {
                    return Ok(unauthorized_db_admin_token());
                }

                let now = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs();
                let last = LAST_SPAWN_SESSION.load(Ordering::Acquire);
                if now - last < 15 {
                    let remaining = 15 - (now - last);
                    return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                        warp::reply::with_status(
                            warp::reply::json(&serde_json::json!({
                                "ok": false,
                                "error": "too soon; please wait",
                                "seconds_remaining": remaining
                            })),
                            warp::http::StatusCode::TOO_MANY_REQUESTS,
                        ),
                    ));
                }
                LAST_SPAWN_SESSION.store(now, Ordering::Release);

                let port: u16 = match payload.get("port") {
                    Some(v) => {
                        if let Some(p) = v.as_u64() {
                            p as u16
                        } else {
                            8001
                        }
                    }
                    None => 8001,
                };
                let name: String = match payload.get("name") {
                    Some(v) => {
                        if let Some(s) = v.as_str() {
                            s.to_string()
                        } else {
                            format!("Session on port {}", port)
                        }
                    }
                    None => format!("Session on port {}", port),
                };

                let Some(preset_id) = payload
                    .get("preset_id")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
                else {
                    let config: ServerConfig = match serde_json::from_value(payload.clone()) {
                        Ok(config) => config,
                        Err(e) => {
                            return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                                warp::reply::json(
                                    &serde_json::json!({"ok": false, "error": format!("Invalid spawn payload: {}", e)}),
                                ),
                            ));
                        }
                    };

                    let session_name = if name != format!("Session on port {}", port) {
                        name.clone()
                    } else if !config.model_path.is_empty() {
                        let filename = std::path::Path::new(&config.model_path)
                            .file_name()
                            .and_then(|s| s.to_str())
                            .unwrap_or(&config.model_path);
                        format!("Local: {}", filename)
                    } else if let Some(repo) = config.hf_repo.as_ref() {
                        format!("HF: {}", repo)
                    } else {
                        name.clone()
                    };

                    let session_id = app_state::generate_session_id();
                    let session = app_state::Session::new_spawn(
                        session_id.clone(),
                        session_name,
                        config.port,
                        String::new(),
                        config.bind_host.clone(),
                        config.api_key.clone(),
                    );

                    if !state.add_session(session) {
                        return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                            warp::reply::json(
                                &serde_json::json!({"ok": false, "error": "Failed to create session"}),
                            ),
                        ));
                    }

                    state.set_active_session(&session_id);

                    match crate::llama::server::start_server(&state, config, &app_config).await {
                        Ok(()) => {
                            state.update_session_status(&session_id, SessionStatus::Running);
                            return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                                warp::reply::json(
                                    &serde_json::json!({"ok": true, "session_id": session_id}),
                                ),
                            ));
                        }
                        Err(e) => {
                            state.remove_session(&session_id);
                            return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                                warp::reply::json(
                                    &serde_json::json!({"ok": false, "error": e.to_string()}),
                                ),
                            ));
                        }
                    }
                };

                let preset = {
                    let presets = state.presets.lock().unwrap();
                    match presets.iter().find(|p| p.id == preset_id).cloned() {
                        Some(p) => p,
                        None => {
                            return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(warp::reply::json(
                                &serde_json::json!({"ok": false, "error": "Preset not found"}),
                            )));
                        }
                    }
                };

                let session_id = app_state::generate_session_id();
                let session = app_state::Session::new_spawn(
                    session_id.clone(),
                    name.clone(),
                    port,
                    preset_id,
                    preset.bind_host.clone(),
                    preset.api_key.clone(),
                );

                if !state.add_session(session) {
                    return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(warp::reply::json(
                        &serde_json::json!({"ok": false, "error": "Failed to create session"}),
                    )));
                }

                state.set_active_session(&session_id);

                let config = crate::llama::server::ServerConfig {
                    model_path: preset.model_path.clone(),
                    context_size: preset.context_size,
                    ctk: preset.ctk.clone(),
                    ctv: preset.ctv.clone(),
                    tensor_split: preset.tensor_split.clone(),
                    batch_size: preset.batch_size,
                    ubatch_size: preset.ubatch_size,
                    no_mmap: preset.no_mmap,
                    port,
                    ngram_spec: preset.ngram_spec,
                    parallel_slots: preset.parallel_slots,
                    temperature: preset.temperature,
                    top_p: preset.top_p,
                    top_k: preset.top_k,
                    min_p: preset.min_p,
                    repeat_penalty: preset.repeat_penalty,
                    n_cpu_moe: preset.n_cpu_moe,
                    gpu_layers: preset.gpu_layers,
                    mlock: preset.mlock,
                    flash_attn: preset.flash_attn.clone(),
                    split_mode: preset.split_mode.clone(),
                    main_gpu: preset.main_gpu,
                    threads: preset.threads,
                    threads_batch: preset.threads_batch,
                    rope_scaling: preset.rope_scaling.clone(),
                    rope_freq_base: preset.rope_freq_base,
                    rope_freq_scale: preset.rope_freq_scale,
                    spec: crate::llama::server::SpecDecodeConfig {
                        draft_model: preset.draft_model.clone(),
                        draft_min: preset.draft_min,
                        draft_max: preset.draft_max,
                        spec_ngram_size: preset.spec_ngram_size,
                        ..Default::default()
                    },
                    seed: preset.seed,
                    system_prompt_file: preset.system_prompt_file.clone(),
                    extra_args: preset.extra_args.clone(),
                    bind_host: preset.bind_host.clone(),
                    api_key: preset.api_key.clone(),
                    alias: preset.alias.clone(),
                    ..Default::default()
                };

                match crate::llama::server::start_server(&state, config, &app_config).await {
                    Ok(()) => {
                        state.update_session_status(&session_id, SessionStatus::Running);
                        Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(warp::reply::json(
                            &serde_json::json!({"ok": true, "session_id": session_id}),
                        )))
                    }
                    Err(e) => {
                        state.remove_session(&session_id);
                        Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(warp::reply::json(
                            &serde_json::json!({"ok": false, "error": e.to_string()}),
                        )))
                    }
                }
            }
        })
}

fn api_attach(
    state: AppState,
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    static LAST_ATTACH: AtomicU64 = AtomicU64::new(0);

    warp::path!("api" / "attach")
        .and(warp::path::end())
        .and(warp::post())
        .and(warp::header::headers_cloned())
        .and(warp::body::json())
        .and(with_app_config(app_config))
        .and_then(move |headers: warp::http::HeaderMap,
                      payload: serde_json::Map<String, serde_json::Value>,
                      cfg: Arc<AppConfig>| {
            let state = state.clone();
            async move {
                // Require api-token for attach (constant-time comparison via helper)
                let bearer_str = headers
                    .get("authorization")
                    .and_then(|v| v.to_str().ok())
                    .and_then(|v| v.strip_prefix("Bearer "));
                let has_token = bearer_matches_api_token(bearer_str, &cfg);

                if !has_token {
                    return Ok::<_, warp::Rejection>(warp::reply::with_status(
                        warp::reply::json(&serde_json::json!({ "ok": false, "error": "unauthorized" })),
                        warp::http::StatusCode::UNAUTHORIZED,
                    ));
                }

                let now = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs();
                let last = LAST_ATTACH.load(Ordering::Acquire);
                if now - last < 10 {
                    let remaining = 10 - (now - last);
                    return Ok::<_, warp::Rejection>(warp::reply::with_status(
                        warp::reply::json(&serde_json::json!({
                            "ok": false,
                            "error": "too soon; please wait",
                            "seconds_remaining": remaining
                        })),
                        warp::http::StatusCode::TOO_MANY_REQUESTS,
                    ));
                }
                LAST_ATTACH.store(now, Ordering::Release);

                let endpoint: String = match payload.get("endpoint") {
                    Some(v) => {
                        if let Some(s) = v.as_str() {
                            // Validate: must be http/https scheme with private/loopback host
                            let parsed = url::Url::parse(s).map_err(|_| warp::reject::not_found())?;
                            if !["http", "https"].contains(&parsed.scheme()) {
                                return Ok::<_, warp::Rejection>(warp::reply::with_status(
                                    warp::reply::json(&serde_json::json!({"ok": false, "error": "Endpoint must use http:// or https://"})),
                                    warp::http::StatusCode::OK,
                                ));
                            }
                            if let Some(host) = parsed.host_str()
                                && let Ok(ip) = host.parse::<std::net::IpAddr>() {
                                    // is_private() is unstable; inline the check
                                    let private = matches!(ip, std::net::IpAddr::V4(v4)
                                        if v4.octets()[0] == 10
                                            || (v4.octets()[0] == 172 && (4..=11).contains(&v4.octets()[1]))
                                            || (v4.octets()[0] == 192 && v4.octets()[1] == 168));
                                    if !ip.is_loopback() && !private {
                                        return Ok::<_, warp::Rejection>(warp::reply::with_status(
                                            warp::reply::json(&serde_json::json!({"ok": false, "error": "Endpoint must be on a private network"})),
                                            warp::http::StatusCode::OK,
                                        ));
                                    }
                                }
                            s.to_string()
                        } else {
                            return Ok::<_, warp::Rejection>(warp::reply::with_status(
                                warp::reply::json(&serde_json::json!({"ok": false, "error": "Invalid endpoint"})),
                                warp::http::StatusCode::OK,
                            ));
                        }
                    }
                    None => {
                        return Ok::<_, warp::Rejection>(warp::reply::with_status(
                            warp::reply::json(&serde_json::json!({"ok": false, "error": "Missing endpoint"})),
                            warp::http::StatusCode::OK,
                        ));
                    }
                };

                // Extract optional API key for remote server authentication
                let api_key: Option<String> = payload.get("api_key")
                    .and_then(|v| v.as_str())
                    .filter(|s| !s.is_empty())
                    .map(|s| s.to_string());

                // Pre-attach health check
                let client = match reqwest::Client::builder()
                    .timeout(std::time::Duration::from_secs(15))
                    .build()
                {
                    Ok(c) => c,
                    Err(e) => {
                        return Ok::<_, warp::Rejection>(warp::reply::with_status(
                            warp::reply::json(&serde_json::json!({
                                "ok": false,
                                "error": format!("Failed to create HTTP client: {}", e)
                            })),
                            warp::http::StatusCode::OK,
                        ));
                    }
                };

                // Check if server is reachable (with API key if provided)
                eprintln!("[info] Health-checking llama-server at {}", endpoint);
                let mut health_req = client.get(&endpoint);
                if let Some(ref key) = api_key {
                    health_req = health_req.header("Authorization", format!("Bearer {}", key));
                }
                let server_up = match health_req.send().await {
                    Ok(resp) => {
                        eprintln!("[info] llama-server health check status: {}", resp.status());
                        true
                    }
                    Err(e) => {
                        eprintln!("[warn] llama-server health check failed: {}", e);
                        false
                    }
                };
                if !server_up {
                    return Ok::<_, warp::Rejection>(warp::reply::with_status(
                        warp::reply::json(&serde_json::json!({
                            "ok": false,
                            "error": format!("Cannot reach llama-server at {}. Is it running?", endpoint)
                        })),
                        warp::http::StatusCode::OK,
                    ));
                }

                // Check if metrics endpoint is available (with API key if provided)
                let mut metrics_req = client.get(format!("{}/health", endpoint.trim_end_matches('/')));
                if let Some(ref key) = api_key {
                    metrics_req = metrics_req.header("Authorization", format!("Bearer {}", key));
                }
                let metrics_available = metrics_req.send().await.is_ok();

                // Check if there's already an attach session for this endpoint
                let existing_session_id = {
                    let sessions = state.sessions.lock().unwrap();
                    sessions.iter().find(|s| {
                        if let crate::state::SessionMode::Attach { endpoint: ep, .. } = &s.mode {
                            *ep == endpoint
                        } else {
                            false
                        }
                    }).map(|s| s.id.clone())
                };

                let session_id = if let Some(id) = existing_session_id {
                    // Reuse existing session
                    eprintln!("[info] Reusing existing attach session for {}", endpoint);
                    id
                } else {
                    // Create new session
                    let session_id = crate::state::generate_session_id();
                    let session = crate::state::Session::new_attach(
                        session_id.clone(),
                        format!("Attached: {}", endpoint),
                        endpoint,
                        api_key.clone(),
                    );
                    if !state.add_session(session) {
                        return Ok::<_, warp::Rejection>(warp::reply::with_status(
                            warp::reply::json(&serde_json::json!({"ok": false, "error": "Maximum sessions reached"})),
                            warp::http::StatusCode::OK,
                        ));
                    }
                    session_id
                };

                // Update session metadata for connection tracking
                {
                    let mut sessions = state.sessions.lock().unwrap();
                    if let Some(s) = sessions.iter_mut().find(|s| s.id == session_id) {
                        s.last_connected_at = now;
                        s.connect_count += 1;
                        s.last_error = None;
                    }
                }

                state.set_active_session(&session_id);
                state.llama_poll_notify.notify_waiters();
                Ok::<_, warp::Rejection>(warp::reply::with_status(
                    warp::reply::json(&serde_json::json!({
                        "ok": true,
                        "warning": if !metrics_available {
                            Some("llama-server is running but metrics endpoint (/health) is unavailable. Inference metrics will not be available. Start llama-server with --metrics flag to enable metrics.")
                        } else {
                            None
                        }
                    })),
                    warp::http::StatusCode::OK,
                ))
            }
        })
}

fn api_detach(
    state: AppState,
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    let app_config = app_config.clone();
    warp::path!("api" / "detach")
        .and(warp::path::end())
        .and(warp::post())
        .and(warp::header::optional::<String>("authorization"))
        .and(with_app_config(app_config))
        .and_then(move |auth: Option<String>, cfg: Arc<AppConfig>| {
            let state = state.clone();
            async move {
                if !check_api_token(&auth, &cfg) {
                    return Ok(unauthorized_api_token());
                }
                let active_id = state.active_session_id.lock().unwrap().clone();
                if active_id.is_empty() {
                    return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(warp::reply::json(
                        &serde_json::json!({"ok": false, "error": "No active session to detach from"}),
                    )));
                }

                // Check if the active session is an attach session
                let sessions = state.sessions.lock().unwrap();
                let session = sessions.iter().find(|s| s.id == active_id);

                let is_attach = session.map(|s| matches!(s.mode, crate::state::SessionMode::Attach { .. }));

                if !is_attach.unwrap_or(false) {
                    return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(warp::reply::json(
                        &serde_json::json!({"ok": false, "error": "Active session is not an attach session"}),
                    )));
                }

                // Update last_active on detach
                {
                    let mut sessions = state.sessions.lock().unwrap();
                    if let Some(s) = sessions.iter_mut().find(|s| s.id == active_id) {
                        s.last_active = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_secs();
                    }
                }

                drop(sessions);
                // Clear the active session only - server_running is managed by the poller
                state.set_active_session("");
                // Notify poller so it stops polling immediately
                state.llama_poll_notify.notify_waiters();

                Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(warp::reply::json(&serde_json::json!({"ok": true}))))
            }
        })
}

fn api_kill_llama(
    _state: AppState,
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    static LAST_KILL: AtomicU64 = AtomicU64::new(0);

    let app_config = app_config.clone();

    warp::path!("api" / "kill-llama")
        .and(warp::post())
        .and(warp::header::optional::<String>("authorization"))
        .and(warp::body::json::<serde_json::Value>())
        .and(with_app_config(app_config))
        .and_then(move |auth: Option<String>, body: serde_json::Value, cfg: Arc<AppConfig>| {
            async move {
                // Require db-admin-token (elevated operation).
                let bearer = auth.and_then(|v| v.strip_prefix("Bearer ").map(str::to_string));
                let has_admin_token =
                    bearer_matches_db_admin_token(bearer.as_deref(), &cfg);

                if !has_admin_token {
                    return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                        warp::reply::with_status(
                            warp::reply::json(&serde_json::json!({ "error": "unauthorized; db-admin-token required" })),
                            warp::http::StatusCode::UNAUTHORIZED,
                        ),
                    ));
                }

                // Require explicit confirmation.
                let confirm = body.get("confirm")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                if confirm != "kill" {
                    return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                        warp::reply::with_status(
                            warp::reply::json(&serde_json::json!({ "error": "missing confirmation; send { \"confirm\": \"kill\" }" })),
                            warp::http::StatusCode::BAD_REQUEST,
                        ),
                    ));
                }

                // Cooldown: 30 seconds between kills.
                let now = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs();
                let last = LAST_KILL.load(Ordering::Acquire);
                if now - last < 30 {
                    let remaining = 30 - (now - last);
                    return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                        warp::reply::with_status(
                            warp::reply::json(&serde_json::json!({
                                "error": "too soon; please wait",
                                "seconds_remaining": remaining
                            })),
                            warp::http::StatusCode::TOO_MANY_REQUESTS,
                        ),
                    ));
                }

                LAST_KILL.store(now, Ordering::Release);

                // Inline kill logic (platform-specific).
                #[cfg(target_os = "windows")]
                {
                    use std::process::Command;
                    match Command::new("taskkill")
                        .args(["/IM", "llama-server.exe", "/F"])
                        .output()
                    {
                        Ok(output) => {
                            if output.status.success() {
                                Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                                    warp::reply::json(&serde_json::json!({ "ok": true })),
                                ))
                            } else {
                                let err = String::from_utf8_lossy(&output.stderr);
                                Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                                    warp::reply::json(&serde_json::json!({ "ok": false, "error": err })),
                                ))
                            }
                        }
                        Err(e) => Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                            warp::reply::json(&serde_json::json!({ "ok": false, "error": e.to_string() })),
                        )),
                    }
                }
                #[cfg(target_os = "linux")]
                {
                    use std::process::Command;
                    match Command::new("pkill").args(["-f", "llama-server"]).output() {
                        Ok(output) => {
                            if output.status.success() {
                                Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                                    warp::reply::json(&serde_json::json!({ "ok": true })),
                                ))
                            } else {
                                let err = String::from_utf8_lossy(&output.stderr);
                                Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                                    warp::reply::json(&serde_json::json!({ "ok": false, "error": err })),
                                ))
                            }
                        }
                        Err(e) => Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                            warp::reply::json(&serde_json::json!({ "ok": false, "error": e.to_string() })),
                        )),
                    }
                }
                #[cfg(target_os = "macos")]
                {
                    use std::process::Command;
                    match Command::new("pkill").args(["-f", "llama-server"]).output() {
                        Ok(output) => {
                            if output.status.success() {
                                Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                                    warp::reply::json(&serde_json::json!({ "ok": true })),
                                ))
                            } else {
                                let err = String::from_utf8_lossy(&output.stderr);
                                Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                                    warp::reply::json(&serde_json::json!({ "ok": false, "error": err })),
                                ))
                            }
                        }
                        Err(e) => Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                            warp::reply::json(&serde_json::json!({ "ok": false, "error": e.to_string() })),
                        )),
                    }
                }
                #[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
                {
                    Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                        warp::reply::json(&serde_json::json!({ "ok": false, "error": "Unsupported platform" })),
                    ))
                }
            }
        })
}

/// GET /api/tls/config — returns current TLS configuration (non-sensitive).
fn api_get_tls_config(
    state: AppState,
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    let app_config = app_config.clone();

    warp::path!("api" / "tls" / "config")
        .and(warp::get())
        .and(warp::header::optional::<String>("authorization"))
        .and(with_app_config(app_config))
        .map(move |auth: Option<String>, cfg: Arc<AppConfig>| {
            let bearer = auth.and_then(|v| v.strip_prefix("Bearer ").map(str::to_string));
            let has_api_token =
                bearer_matches_api_token(bearer.as_deref(), &cfg);

            if !has_api_token {
                return Box::new(warp::reply::with_status(
                    warp::reply::json(&serde_json::json!({ "error": "unauthorized" })),
                    warp::http::StatusCode::UNAUTHORIZED,
                )) as Box<dyn warp::reply::Reply>;
            }

            let tls_cfg = state.get_tls_config();

            let mode_str = match tls_cfg.mode {
                TlsMode::None => "none",
                TlsMode::SelfSigned => "self-signed",
                TlsMode::Custom => "custom",
                TlsMode::Acme => "acme",
            };

            // Build a safe ACME summary (no secrets).
            let acme_summary: serde_json::Value = if matches!(tls_cfg.mode, TlsMode::Acme) {
                serde_json::json!({
                    "enabled": tls_cfg.acme.enabled,
                    "fqdn": tls_cfg.acme.fqdn,
                    "environment": tls_cfg.acme.environment,
                    "dnsProvider": tls_cfg.acme.dns_provider,
                    "validationDelay": tls_cfg.acme.validation_delay,
                    "lastRenewal": tls_cfg.acme.last_renewal,
                    "certPath": tls_cfg.acme.cert_path.as_ref().map(|p| p.to_string_lossy().to_string()),
                    "keyPath": tls_cfg.acme.key_path.as_ref().map(|p| p.to_string_lossy().to_string()),
                })
            } else {
                serde_json::json!({
                    "enabled": tls_cfg.acme.enabled,
                })
            };

            Box::new(warp::reply::json(&serde_json::json!({
                "mode": mode_str,
                "customCertPath": tls_cfg.custom_cert_path.as_ref().map(|p| p.to_string_lossy().to_string()),
                "customKeyPath": tls_cfg.custom_key_path.as_ref().map(|p| p.to_string_lossy().to_string()),
                "acme": acme_summary,
            })))
        })
}

/// PUT /api/tls/config — update TLS configuration (requires api-token).
/// In Phase 1, changes require a restart; we only persist to tls-config.json.
fn api_put_tls_config(
    state: AppState,
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "tls" / "config")
        .and(warp::put())
        .and(warp::header::optional::<String>("Authorization"))
        .and(warp::body::json())
        .and_then(
            move |auth_header: Option<String>, body: serde_json::Value| {
                let state = state.clone();
                let app_config = app_config.clone();
                async move {
                    if !check_api_token(&auth_header, &app_config) {
                        return Ok::<_, warp::Rejection>(warp::reply::with_status(
                            warp::reply::json(&serde_json::json!({"ok": false, "error": "unauthorized; api-token required"})),
                            warp::http::StatusCode::UNAUTHORIZED,
                        ));
                    }

                    // Extract mode
                    let mode_str = body.get("mode").and_then(|v| v.as_str()).unwrap_or("none");

                    let mode = match mode_str {
                        "none" => TlsMode::None,
                        "self-signed" => TlsMode::SelfSigned,
                        "custom" => TlsMode::Custom,
                        "acme" => TlsMode::Acme,
                        _ => {
                            return Ok::<_, warp::Rejection>(warp::reply::with_status(
                                warp::reply::json(&serde_json::json!({
                                    "ok": false,
                                    "error": format!("Invalid mode: {}", mode_str)
                                })),
                                warp::http::StatusCode::BAD_REQUEST,
                            ));
                        }
                    };

                    // For custom mode, validate cert/key paths
                    if mode == TlsMode::Custom {
                        let cert_path_str = body.get("customCertPath").and_then(|v| v.as_str());
                        let key_path_str = body.get("customKeyPath").and_then(|v| v.as_str());

                        if cert_path_str.is_none() || key_path_str.is_none() {
                            return Ok::<_, warp::Rejection>(warp::reply::with_status(
                                warp::reply::json(&serde_json::json!({
                                    "ok": false,
                                    "error": "custom mode requires customCertPath and customKeyPath"
                                })),
                                warp::http::StatusCode::BAD_REQUEST,
                            ));
                        }
                    }

                    // Build ACME config from request (or keep existing if not acme mode)
                    let existing = state.get_tls_config();
                    let acme_cfg = if mode == TlsMode::Acme {
                        // Read acme fields from body
                        let acme_obj = body.get("acme").and_then(|v| v.as_object());

                        let enabled = acme_obj
                            .and_then(|o| o.get("enabled").and_then(|v| v.as_bool()))
                            .unwrap_or(true);

                        let fqdn = acme_obj
                            .and_then(|o| o.get("fqdn").and_then(|v| v.as_str()))
                            .unwrap_or("")
                            .to_string();

                        let environment = acme_obj
                            .and_then(|o| o.get("environment").and_then(|v| v.as_str()))
                            .unwrap_or("staging")
                            .to_string();

                        let dns_provider = acme_obj
                            .and_then(|o| o.get("dnsProvider").and_then(|v| v.as_str()))
                            .unwrap_or("")
                            .to_string();

                        let validation_delay = acme_obj
                            .and_then(|o| o.get("validationDelay").and_then(|v| v.as_u64()))
                            .unwrap_or(300);

                        // Parse dnsConfig as a map
                        let dns_config: HashMap<String, String> = acme_obj
                            .and_then(|o| o.get("dnsConfig").and_then(|v| v.as_object()))
                            .map(|map| {
                                map.iter()
                                    .filter_map(|(k, v)| {
                                        v.as_str().map(|s| (k.clone(), s.to_string()))
                                    })
                                    .collect()
                            })
                            .unwrap_or_default();

                        // Validate ACME fields
                        if fqdn.is_empty() {
                            return Ok::<_, warp::Rejection>(warp::reply::with_status(
                                warp::reply::json(&serde_json::json!({
                                    "ok": false,
                                    "error": "acme mode requires acme.fqdn"
                                })),
                                warp::http::StatusCode::BAD_REQUEST,
                            ));
                        }

                        if environment != "staging" && environment != "production" {
                            return Ok::<_, warp::Rejection>(warp::reply::with_status(
                                warp::reply::json(&serde_json::json!({
                                    "ok": false,
                                    "error": "acme.environment must be 'staging' or 'production'"
                                })),
                                warp::http::StatusCode::BAD_REQUEST,
                            ));
                        }

                        if dns_provider.is_empty() {
                            return Ok::<_, warp::Rejection>(warp::reply::with_status(
                                warp::reply::json(&serde_json::json!({
                                    "ok": false,
                                    "error": "acme mode requires acme.dnsProvider"
                                })),
                                warp::http::StatusCode::BAD_REQUEST,
                            ));
                        }

                        if dns_config.is_empty() {
                            return Ok::<_, warp::Rejection>(warp::reply::with_status(
                                warp::reply::json(&serde_json::json!({
                                    "ok": false,
                                    "error": "acme mode requires acme.dnsConfig"
                                })),
                                warp::http::StatusCode::BAD_REQUEST,
                            ));
                        }

                        let email = acme_obj
                            .and_then(|o| o.get("email").and_then(|v| v.as_str()))
                            .unwrap_or("")
                            .to_string();

                        crate::config::AcmeConfig {
                            enabled,
                            fqdn,
                            email,
                            environment,
                            dns_provider,
                            dns_config,
                            validation_delay,
                            last_renewal: existing.acme.last_renewal.clone(),
                            cert_path: existing.acme.cert_path.clone(),
                            key_path: existing.acme.key_path.clone(),
                        }
                    } else {
                        // Non-acme mode: disable ACME fields but preserve existing cert paths
                        // (they may still be valid) until mode changes.
                        existing.acme
                    };

                    let new_cfg = crate::config::TLSConfig {
                        mode,
                        custom_cert_path: body
                            .get("customCertPath")
                            .and_then(|v| v.as_str())
                            .map(PathBuf::from),
                        custom_key_path: body
                            .get("customKeyPath")
                            .and_then(|v| v.as_str())
                            .map(PathBuf::from),
                        acme: acme_cfg,
                    };

                    // Update in-memory state
                    state.set_tls_config(new_cfg.clone());

                    // Persist to disk (restart required to apply)
                    if let Err(e) = crate::config::save_tls_config(&app_config.config_dir, &new_cfg)
                    {
                        eprintln!("[error] Failed to save tls-config.json: {}", e);
                        // Still return success; in-memory state updated.
                    }

                    Ok::<_, warp::Rejection>(warp::reply::with_status(
                        warp::reply::json(&serde_json::json!({
                            "ok": true,
                            "requires_restart": true
                        })),
                        warp::http::StatusCode::OK,
                    ))
                }
            },
        )
}

/// POST /api/tls/acme/request — trigger ACME certificate request (requires api-token).
fn api_tls_acme_request(
    state: AppState,
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    use std::sync::atomic::AtomicU64;
    use std::time::{SystemTime, UNIX_EPOCH};

    static LAST_TLS_ACME_REQUEST: AtomicU64 = AtomicU64::new(0);

    warp::path!("api" / "tls" / "acme" / "request")
        .and(warp::post())
        .and(warp::header::optional::<String>("Authorization"))
        .and_then(move |auth_header: Option<String>| {
            let state = state.clone();
            let app_config = app_config.clone();
            async move {
                if !check_api_token(&auth_header, &app_config) {
                    return Ok::<_, warp::Rejection>(warp::reply::with_status(
                        warp::reply::json(&serde_json::json!({"ok": false, "error": "unauthorized; api-token required"})),
                        warp::http::StatusCode::UNAUTHORIZED,
                    ));
                }

                let now = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs();
                let (ok, remaining) = try_cooldown(&LAST_TLS_ACME_REQUEST, now, 60);
                if !ok {
                    return Ok::<_, warp::Rejection>(warp::reply::with_status(
                        warp::reply::json(&serde_json::json!({
                            "ok": false,
                            "error": "too soon; please wait",
                            "seconds_remaining": remaining
                        })),
                        warp::http::StatusCode::TOO_MANY_REQUESTS,
                    ));
                }

                let cfg = state.get_tls_config();
                let config_dir = app_config.config_dir.clone();

                match crate::acme::acme_request_cert(&config_dir, &cfg) {
                    Ok(new_cfg) => {
                        eprintln!("[info] ACME certificate request succeeded");
                        state.set_tls_config(new_cfg.clone());
                        if let Err(e) = crate::config::save_tls_config(&config_dir, &new_cfg) {
                            eprintln!(
                                "[error] Failed to save tls-config.json after ACME request: {}",
                                e
                            );
                        }
                        Ok::<_, warp::Rejection>(warp::reply::with_status(
                            warp::reply::json(&serde_json::json!({
                                "ok": true,
                                "requires_restart": true
                            })),
                            warp::http::StatusCode::OK,
                        ))
                    }
                    Err(e) => {
                        eprintln!("[error] ACME certificate request failed: {}", e);
                        Ok::<_, warp::Rejection>(warp::reply::with_status(
                            warp::reply::json(&serde_json::json!({
                                "ok": false,
                                "error": e
                            })),
                            warp::http::StatusCode::BAD_REQUEST,
                        ))
                    }
                }
            }
        })
}

/// POST /api/tls/acme/renew — trigger ACME certificate renewal (requires api-token).
fn api_tls_acme_renew(
    state: AppState,
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    use std::sync::atomic::AtomicU64;
    use std::time::{SystemTime, UNIX_EPOCH};

    static LAST_TLS_ACME_RENEW: AtomicU64 = AtomicU64::new(0);

    warp::path!("api" / "tls" / "acme" / "renew")
        .and(warp::post())
        .and(warp::header::optional::<String>("Authorization"))
        .and_then(move |auth_header: Option<String>| {
            let state = state.clone();
            let app_config = app_config.clone();
            async move {
                if !check_api_token(&auth_header, &app_config) {
                    return Ok::<_, warp::Rejection>(warp::reply::with_status(
                        warp::reply::json(&serde_json::json!({"ok": false, "error": "unauthorized; api-token required"})),
                        warp::http::StatusCode::UNAUTHORIZED,
                    ));
                }

                let now = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs();
                let (ok, remaining) = try_cooldown(&LAST_TLS_ACME_RENEW, now, 60);
                if !ok {
                    return Ok::<_, warp::Rejection>(warp::reply::with_status(
                        warp::reply::json(&serde_json::json!({
                            "ok": false,
                            "error": "too soon; please wait",
                            "seconds_remaining": remaining
                        })),
                        warp::http::StatusCode::TOO_MANY_REQUESTS,
                    ));
                }

                let cfg = state.get_tls_config();
                let config_dir = app_config.config_dir.clone();

                match crate::acme::acme_renew_cert(&config_dir, &cfg) {
                    Ok(new_cfg) => {
                        eprintln!("[info] ACME renewal succeeded (manual)");
                        state.set_tls_config(new_cfg.clone());
                        if let Err(e) = crate::config::save_tls_config(&config_dir, &new_cfg) {
                            eprintln!(
                                "[error] Failed to save tls-config.json after ACME renewal: {}",
                                e
                            );
                        }
                        Ok::<_, warp::Rejection>(warp::reply::with_status(
                            warp::reply::json(&serde_json::json!({
                                "ok": true,
                                "requires_restart": true
                            })),
                            warp::http::StatusCode::OK,
                        ))
                    }
                    Err(e) => {
                        eprintln!("[error] ACME renewal failed: {}", e);
                        Ok::<_, warp::Rejection>(warp::reply::with_status(
                            warp::reply::json(&serde_json::json!({
                                "ok": false,
                                "error": e
                            })),
                            warp::http::StatusCode::BAD_REQUEST,
                        ))
                    }
                }
            }
        })
}

// ========================
// llama-server Binary Updater
// ========================

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

                let url = "https://api.github.com/repos/ggerganov/llama.cpp/releases/latest";
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

                match crate::llama::llama_cpp_downloader::list_releases(&client).await {
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

/// POST /api/llama-binary/update — downloads latest release and overwrites llama-server binary
fn api_llama_binary_update(
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (impl warp::reply::Reply,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "llama-binary" / "update")
        .and(warp::post())
        .and(warp::header::optional::<String>("authorization"))
        .and(super::safe_json_body::<serde_json::Value>())
        .and_then(move |auth: Option<String>, _body: serde_json::Value| {
            let cfg = app_config.clone();
            async move {
                if !check_api_token(&auth, &cfg) {
                    return Ok(unauthorized_api_token());
                }

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
                    match crate::llama::llama_cpp_downloader::list_releases(&client).await {
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
                    crate::llama::llama_cpp_downloader::select_assets(&release, backend, arch_str);

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

                if let Err(e) = crate::llama::llama_cpp_downloader::download_and_extract(
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

                // Ensure destination directory exists (e.g. ~/.config/llama-monitor/bin/)
                if let Err(e) = std::fs::create_dir_all(dest_dir) {
                    return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                        warp::reply::json(&serde_json::json!({
                            "ok": false,
                            "error": format!("Failed to create bin dir {}: {}", dest_dir.display(), e)
                        })),
                    ));
                }

                // Copy ALL files from the extracted archive into dest_dir so that
                // CUDA / Vulkan / SYCL builds have their shared libraries alongside
                // the binary (ggml-cuda.dll, cublas64_12.dll, etc.).
                fn copy_all_files(
                    src: &std::path::Path,
                    dest: &std::path::Path,
                ) -> std::io::Result<()> {
                    for entry in std::fs::read_dir(src)?.filter_map(|e| e.ok()) {
                        let path = entry.path();
                        if path.is_dir() {
                            copy_all_files(&path, dest)?;
                        } else if let Some(fname) = path.file_name() {
                            let _ = std::fs::copy(&path, dest.join(fname));
                        }
                    }
                    Ok(())
                }

                if let Err(e) = copy_all_files(tmp_dir.path(), dest_dir) {
                    return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                        warp::reply::json(&serde_json::json!({
                            "ok": false,
                            "error": format!("Failed to copy release files to {}: {}", dest_dir.display(), e)
                        })),
                    ));
                }

                // Locate the extracted binary first, then make sure the configured
                // llama_server_path itself is updated even when it uses a custom filename.
                let found_path = dest_dir.join(binary_name);
                if !found_path.exists() {
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

                if found_path != dest_path
                    && let Err(e) = std::fs::copy(&found_path, &dest_path)
                {
                    return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                        warp::reply::json(&serde_json::json!({
                            "ok": false,
                            "error": format!(
                                "Failed to install {} at {}: {}",
                                binary_name,
                                dest_path.display(),
                                e
                            )
                        })),
                    ));
                }

                // Set executable bit on all extracted files (unix)
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    if let Ok(entries) = std::fs::read_dir(dest_dir) {
                        for entry in entries.filter_map(|e| e.ok()) {
                            let _ = std::fs::set_permissions(
                                entry.path(),
                                std::fs::Permissions::from_mode(0o755),
                            );
                        }
                    }
                    let _ = std::fs::set_permissions(
                        &dest_path,
                        std::fs::Permissions::from_mode(0o755),
                    );
                }

                // Compute SHA256 of the llama-server binary so users can
                // verify integrity out-of-band (e.g. `sha256sum llama-server`).
                let installed_path = if dest_path.exists() {
                    &dest_path
                } else {
                    &found_path
                };
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
                    }),
                )))
            }
        })
}

fn api_self_update(
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    use std::sync::atomic::AtomicU64;
    use std::time::{SystemTime, UNIX_EPOCH};

    static LAST_UPDATE: AtomicU64 = AtomicU64::new(0);

    let app_config = app_config.clone();

    warp::path!("api" / "self-update")
        .and(warp::post())
        .and(warp::header::optional::<String>("authorization"))
        .and(warp::body::json::<serde_json::Value>())
        .and(with_app_config(app_config))
        .and_then(move |auth: Option<String>, body: serde_json::Value, cfg: Arc<AppConfig>| {
            async move {
                // Require db-admin-token (elevated operation).
                let bearer = auth.and_then(|v| v.strip_prefix("Bearer ").map(str::to_string));
                let has_admin_token =
                    bearer_matches_db_admin_token(bearer.as_deref(), &cfg);

                if !has_admin_token {
                    return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                        warp::reply::with_status(
                            warp::reply::json(&serde_json::json!({ "error": "unauthorized; db-admin-token required" })),
                            warp::http::StatusCode::UNAUTHORIZED,
                        ),
                    ));
                }

                // Require explicit confirmation.
                let confirm = body.get("confirm")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                if confirm != "update" {
                    return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                        warp::reply::with_status(
                            warp::reply::json(&serde_json::json!({ "error": "missing confirmation; send { \"confirm\": \"update\" }" })),
                            warp::http::StatusCode::BAD_REQUEST,
                        ),
                    ));
                }

                // Cooldown: 5 minutes between updates.
                let now = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs();
                let (ok, remaining) = try_cooldown(&LAST_UPDATE, now, 300);
                if !ok {
                    return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                        warp::reply::with_status(
                            warp::reply::json(&serde_json::json!({
                                "error": "too soon; please wait",
                                "seconds_remaining": remaining
                            })),
                            warp::http::StatusCode::TOO_MANY_REQUESTS,
                        ),
                    ));
                }

                match crate::agent::self_update_binary().await {
                    Ok(result) => {
                        // All platforms: schedule exit so the OS / user can relaunch with
                        // the freshly written binary. On Windows the batch helper also
                        // restarts automatically once this PID disappears.
                        tokio::spawn(async {
                            tokio::time::sleep(std::time::Duration::from_millis(600)).await;
                            std::process::exit(0);
                        });
                        Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                            warp::reply::json(&serde_json::json!({
                                "ok": true,
                                "tag_name": result.tag_name,
                                "restart_required": true
                            })),
                        ))
                    }
                    Err(e) => Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                        warp::reply::json(&serde_json::json!({
                            "ok": false,
                            "error": e.to_string()
                        })),
                    )),
                }
            }
        })
}

#[cfg(test)]
mod tests {
    use super::legacy_chat_types::*;
    use super::resolve_hf_target_dir;
    use super::token_bootstrap_allowed;

    use crate::chat_storage::ChatStorage;
    use crate::config::{self, AcmeConfig, TLSConfig, TlsMode};
    use crate::gpu::env::GpuEnv;
    use crate::state::{AppPaths, AppState};
    use crate::web::auth::AuthManager;
    use std::collections::HashMap;
    use std::path::PathBuf;
    use std::sync::Arc;
    use warp::Filter;

    fn make_test_app_state(tls_config: TLSConfig) -> (AppState, Arc<config::AppConfig>) {
        let paths = AppPaths {
            presets_path: PathBuf::new(),
            templates_path: PathBuf::new(),
            models_dir: None,
            gpu_env_path: PathBuf::new(),
            ui_settings_path: PathBuf::new(),
            sessions_path: PathBuf::new(),
            model_tags_path: PathBuf::new(),
        };
        let cs = Arc::new(
            ChatStorage::open(&PathBuf::from(":memory:")).expect("open in-memory chat storage"),
        );
        let state = AppState::new(
            vec![],
            paths,
            GpuEnv::default(),
            crate::state::UiSettings::default(),
            cs,
            tls_config,
        );
        let app_config = Arc::new(config::AppConfig::for_test(
            Some("test-token".to_string()),
            None,
        ));
        (state, app_config)
    }

    fn tls_routes_filter(
        state: AppState,
        app_config: Arc<config::AppConfig>,
    ) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
        let tls_get_config = super::api_get_tls_config(state.clone(), app_config.clone());
        let tls_put_config = super::api_put_tls_config(state.clone(), app_config.clone());
        let tls_acme_request = super::api_tls_acme_request(state.clone(), app_config.clone());
        let tls_acme_renew = super::api_tls_acme_renew(state.clone(), app_config.clone());
        tls_get_config
            .or(tls_put_config)
            .or(tls_acme_request)
            .or(tls_acme_renew)
    }

    fn auth_routes_filter(
        auth_manager: AuthManager,
    ) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
        super::auth_api_routes(auth_manager)
    }

    #[test]
    fn token_bootstrap_allows_loopback_without_basic_auth() {
        let auth = AuthManager::new(None, None, &TlsMode::None);
        assert!(token_bootstrap_allowed(&auth, "127.0.0.1"));
        assert!(token_bootstrap_allowed(&auth, "localhost"));
    }

    #[test]
    fn resolve_hf_target_dir_rejects_path_traversal() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let models_dir = tmp.path().join("models");
        let err = resolve_hf_target_dir(&models_dir, Some("../escape")).expect_err("rejects");
        assert!(err.contains("path traversal"));
    }

    #[test]
    fn resolve_hf_target_dir_creates_and_resolves_child_dir() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let models_dir = tmp.path().join("models");
        let target = resolve_hf_target_dir(&models_dir, Some("nested/model-dir")).expect("path");
        assert!(target.starts_with(models_dir.canonicalize().expect("canonical models_dir")));
        assert!(target.exists());
    }

    #[cfg(unix)]
    #[test]
    fn resolve_hf_target_dir_rechecks_symlink_escape_after_create() {
        use std::os::unix::fs::symlink;

        let tmp = tempfile::tempdir().expect("tempdir");
        let models_dir = tmp.path().join("models");
        let outside_dir = tmp.path().join("outside");

        std::fs::create_dir_all(&outside_dir).expect("outside dir");
        std::fs::create_dir_all(&models_dir).expect("models dir");
        symlink(&outside_dir, models_dir.join("linked")).expect("symlink");

        let err =
            resolve_hf_target_dir(&models_dir, Some("linked/new-download")).expect_err("rejects");
        assert!(err.contains("escapes models_dir"));
    }

    #[test]
    fn token_bootstrap_allows_all_when_no_auth_configured() {
        let auth = AuthManager::new(None, None, &TlsMode::None);
        // No Auth mode: fully open (local-first)
        assert!(token_bootstrap_allowed(&auth, "0.0.0.0"));
        assert!(token_bootstrap_allowed(&auth, "192.168.2.44"));
    }

    #[test]
    fn token_bootstrap_allows_non_loopback_host_when_no_auth() {
        let auth = AuthManager::new(None, None, &TlsMode::None);
        // No Auth mode: fully open regardless of bind address
        assert!(token_bootstrap_allowed(&auth, "0.0.0.0"));
        assert!(token_bootstrap_allowed(&auth, "192.168.2.44"));
    }

    #[test]
    fn token_bootstrap_rejects_spoofed_host_header_on_non_loopback_bind() {
        // Auth configured + non-loopback bind: Host header must NOT be trusted.
        let auth = AuthManager::new(
            AuthManager::parse_credentials("admin:secret"),
            None,
            &TlsMode::None,
        );
        assert!(!token_bootstrap_allowed(&auth, "0.0.0.0"));
    }

    #[test]
    fn token_bootstrap_allows_loopback_when_basic_auth_is_configured() {
        let auth = AuthManager::new(
            AuthManager::parse_credentials("admin:secret"),
            None,
            &TlsMode::None,
        );
        assert!(token_bootstrap_allowed(&auth, "127.0.0.1"));
        assert!(!token_bootstrap_allowed(&auth, "0.0.0.0"));
    }

    #[test]
    fn token_bootstrap_allows_loopback_when_form_auth_is_configured() {
        let auth = AuthManager::new(
            None,
            AuthManager::parse_credentials("admin:secret"),
            &TlsMode::None,
        );
        assert!(token_bootstrap_allowed(&auth, "127.0.0.1"));
        assert!(!token_bootstrap_allowed(&auth, "0.0.0.0"));
    }

    #[tokio::test]
    async fn form_auth_login_sets_session_cookie_and_status_reflects_it() {
        let auth = AuthManager::new(
            None,
            AuthManager::parse_credentials("admin:secret123"),
            &TlsMode::None,
        );
        let routes = auth_routes_filter(auth);

        let login_resp = warp::test::request()
            .method("POST")
            .path("/api/auth/login")
            .json(&serde_json::json!({
                "username": "admin",
                "password": "secret123",
            }))
            .reply(&routes)
            .await;

        assert_eq!(login_resp.status(), 200);
        let set_cookie = login_resp
            .headers()
            .get("set-cookie")
            .and_then(|value| value.to_str().ok())
            .expect("set-cookie header");
        assert!(set_cookie.contains("llama_monitor_session="));

        let status_resp = warp::test::request()
            .method("GET")
            .path("/api/auth/status")
            .header("cookie", set_cookie)
            .reply(&routes)
            .await;

        assert_eq!(status_resp.status(), 200);
        let body: serde_json::Value =
            serde_json::from_slice(status_resp.body()).expect("valid JSON");
        assert_eq!(body["enabled"], true);
        assert_eq!(body["methods"]["form"], true);
        assert_eq!(body["authenticated"], true);
        assert_eq!(body["method"], "form");
        assert_eq!(body["username"], "admin");
    }

    #[tokio::test]
    async fn form_auth_logout_clears_session_cookie() {
        let auth = AuthManager::new(
            None,
            AuthManager::parse_credentials("admin:secret123"),
            &TlsMode::None,
        );
        let routes = auth_routes_filter(auth);

        let login_resp = warp::test::request()
            .method("POST")
            .path("/api/auth/login")
            .json(&serde_json::json!({
                "username": "admin",
                "password": "secret123",
            }))
            .reply(&routes)
            .await;

        let set_cookie = login_resp
            .headers()
            .get("set-cookie")
            .and_then(|value| value.to_str().ok())
            .expect("set-cookie header");

        let logout_resp = warp::test::request()
            .method("POST")
            .path("/api/auth/logout")
            .header("cookie", set_cookie)
            .reply(&routes)
            .await;

        assert_eq!(logout_resp.status(), 200);
        let clear_cookie = logout_resp
            .headers()
            .get("set-cookie")
            .and_then(|value| value.to_str().ok())
            .expect("set-cookie clear header");
        assert!(clear_cookie.contains("Max-Age=0"));
    }

    #[tokio::test]
    async fn tls_config_get_requires_api_token() {
        let (state, app_config) = make_test_app_state(TLSConfig::default());
        let routes = tls_routes_filter(state, app_config);

        // Without token -> 401
        let resp = warp::test::request()
            .method("GET")
            .path("/api/tls/config")
            .reply(&routes)
            .await;
        assert_eq!(resp.status(), 401);

        // With correct token -> 200
        let resp = warp::test::request()
            .method("GET")
            .path("/api/tls/config")
            .header("Authorization", "Bearer test-token")
            .reply(&routes)
            .await;
        assert_eq!(resp.status(), 200);
        let body: serde_json::Value = serde_json::from_slice(resp.body()).expect("valid JSON");
        assert_eq!(body["mode"], "none");
    }

    #[tokio::test]
    async fn tls_config_get_returns_acme_fields() {
        let mut dns_config = HashMap::new();
        dns_config.insert("CF_API_TOKEN".to_string(), "redacted".to_string());

        let tls_config = TLSConfig {
            mode: TlsMode::Acme,
            custom_cert_path: None,
            custom_key_path: None,
            acme: AcmeConfig {
                enabled: true,
                fqdn: "llama-monitor.example.com".to_string(),
                email: String::new(),
                environment: "staging".to_string(),
                dns_provider: "cloudflare".to_string(),
                dns_config,
                validation_delay: 300,
                last_renewal: None,
                cert_path: None,
                key_path: None,
            },
        };

        let (state, app_config) = make_test_app_state(tls_config);
        let routes = tls_routes_filter(state, app_config);

        let resp = warp::test::request()
            .method("GET")
            .path("/api/tls/config")
            .header("Authorization", "Bearer test-token")
            .reply(&routes)
            .await;

        assert_eq!(resp.status(), 200);
        let body: serde_json::Value = serde_json::from_slice(resp.body()).expect("valid JSON");
        assert_eq!(body["mode"], "acme");
        assert_eq!(body["acme"]["fqdn"], "llama-monitor.example.com");
        assert_eq!(body["acme"]["environment"], "staging");
        assert_eq!(body["acme"]["dnsProvider"], "cloudflare");
    }

    #[tokio::test]
    async fn tls_config_put_accepts_valid_acme() {
        let (state, app_config) = make_test_app_state(TLSConfig::default());
        let routes = tls_routes_filter(state.clone(), app_config);

        let payload = serde_json::json!({
            "mode": "acme",
            "acme": {
                "enabled": true,
                "fqdn": "llama-monitor.example.com",
                "environment": "staging",
                "dnsProvider": "cloudflare",
                "validationDelay": 300,
                "dnsConfig": {
                    "CF_API_TOKEN": "test-token"
                }
            }
        });

        let resp = warp::test::request()
            .method("PUT")
            .path("/api/tls/config")
            .header("Authorization", "Bearer test-token")
            .json(&payload)
            .reply(&routes)
            .await;

        assert_eq!(resp.status(), 200);
        let body: serde_json::Value = serde_json::from_slice(resp.body()).expect("valid JSON");
        assert_eq!(body["ok"], true);

        // Verify TLSConfig was updated in state
        let cfg = state.get_tls_config();
        assert_eq!(cfg.mode, TlsMode::Acme);
        assert_eq!(cfg.acme.fqdn, "llama-monitor.example.com");
        assert_eq!(cfg.acme.dns_provider, "cloudflare");
    }

    #[tokio::test]
    async fn tls_config_put_rejects_invalid_acme_missing_provider() {
        let (state, app_config) = make_test_app_state(TLSConfig::default());
        let routes = tls_routes_filter(state, app_config);

        let payload = serde_json::json!({
            "mode": "acme",
            "acme": {
                "enabled": true,
                "fqdn": "llama-monitor.example.com",
                "environment": "staging",
                "dnsProvider": "",
                "dnsConfig": {
                    "CF_API_TOKEN": "test-token"
                }
            }
        });

        let resp = warp::test::request()
            .method("PUT")
            .path("/api/tls/config")
            .header("Authorization", "Bearer test-token")
            .json(&payload)
            .reply(&routes)
            .await;

        assert_eq!(resp.status(), 400);
        let body: serde_json::Value = serde_json::from_slice(resp.body()).expect("valid JSON");
        assert!(
            body["error"]
                .as_str()
                .map(|s| s.contains("dnsProvider"))
                .unwrap_or(false)
        );
    }

    fn make_minimal_chat_tab() -> ChatTab {
        ChatTab {
            id: "tab-1".to_string(),
            name: "Test Tab".to_string(),
            system_prompt: "You are helpful.".to_string(),
            ai_name: None,
            user_name: None,
            explicit_level: None,
            messages: vec![],
            total_input_tokens: None,
            total_output_tokens: None,
            model_params: ChatModelParams::default(),
            created_at: 0,
            updated_at: 0,
            auto_compact: None,
            auto_compact_summarize: None,
            compact_threshold: None,
            compact_mode: None,
            last_ctx_pct: None,
            active_template_id: None,
            context_notes: vec![],
            sidebar_width: 0,
            quick_guide_active: String::new(),
            armed_story_beats: vec![],
            role_boundary_custom: None,
            ai_gender: None,
        }
    }

    #[test]
    fn chat_tab_explicit_level_serialization() {
        let mut tab = make_minimal_chat_tab();
        tab.explicit_level = Some(1);

        let json = serde_json::to_string(&tab).expect("ChatTab should serialize");

        // Verify camelCase key in JSON
        assert!(
            json.contains("\"explicitLevel\""),
            "JSON should contain camelCase 'explicitLevel' field, got: {}",
            json
        );

        // Verify value is correct
        let parsed: serde_json::Value =
            serde_json::from_str(&json).expect("JSON should parse to Value");
        assert_eq!(
            parsed.get("explicitLevel").and_then(|v| v.as_u64()),
            Some(1),
            "explicitLevel should be 1"
        );

        // Deserialize back and verify
        let deserialized: ChatTab =
            serde_json::from_str(&json).expect("ChatTab should deserialize from own JSON");
        assert_eq!(
            deserialized.explicit_level,
            Some(1),
            "explicit_level should round-trip to Some(1)"
        );
    }

    #[test]
    fn chat_tab_explicit_level_default() {
        let json = r#"{
            "id": "tab-1",
            "name": "Test Tab",
            "system_prompt": "You are helpful.",
            "messages": [],
            "model_params": {
                "temperature": 0.7,
                "top_p": 0.9,
                "top_k": 40,
                "min_p": 0.01,
                "repeat_penalty": 1.0
            },
            "created_at": 0,
            "updated_at": 0
        }"#;

        let result = serde_json::from_str::<ChatTab>(json);
        assert!(
            result.is_ok(),
            "Should deserialize without explicitLevel field"
        );

        let tab = result.unwrap();
        assert!(
            tab.explicit_level.is_none(),
            "explicit_level should default to None when field is absent"
        );
    }

    #[test]
    fn chat_tab_explicit_mode_alias_migration() {
        // The serde alias "explicit_mode" allows deserialization of JSON that uses
        // the legacy field name (instead of camelCase "explicitLevel").
        // The value must still be a u8 to match the explicit_level type.
        let json = r#"{
            "id": "tab-1",
            "name": "Test Tab",
            "system_prompt": "You are helpful.",
            "explicit_mode": 2,
            "messages": [],
            "model_params": {
                "temperature": 0.7,
                "top_p": 0.9,
                "top_k": 40,
                "min_p": 0.01,
                "repeat_penalty": 1.0
            },
            "created_at": 0,
            "updated_at": 0
        }"#;

        let result = serde_json::from_str::<ChatTab>(json);
        assert!(
            result.is_ok(),
            "Should deserialize legacy 'explicit_mode' field via alias"
        );

        let tab = result.unwrap();
        assert_eq!(
            tab.explicit_level,
            Some(2),
            "explicit_mode alias should map to explicit_level"
        );
    }

    #[test]
    fn chat_tab_explicit_level_all_states() {
        for level in [0u8, 1, 2] {
            let mut tab = make_minimal_chat_tab();
            tab.explicit_level = Some(level);

            let json = serde_json::to_string(&tab)
                .unwrap_or_else(|e| panic!("ChatTab should serialize for level {}: {}", level, e));

            // Verify the camelCase key is present
            assert!(
                json.contains("\"explicitLevel\""),
                "JSON for level {} should contain 'explicitLevel'",
                level
            );

            // Deserialize back and verify value
            let deserialized: ChatTab = serde_json::from_str(&json).unwrap_or_else(|e| {
                panic!("ChatTab should deserialize for level {}: {}", level, e)
            });
            assert_eq!(
                deserialized.explicit_level,
                Some(level),
                "explicit_level should round-trip for state {}",
                level
            );
        }
    }

    #[test]
    fn chat_message_compaction_metadata_round_trips() {
        let msg = ChatMessage {
            role: "system".to_string(),
            content: "## Persistent Facts\n- Keeps rolling memory".to_string(),
            timestamp_ms: 123,
            input_tokens: None,
            output_tokens: None,
            cumulative_input_tokens: None,
            cumulative_output_tokens: None,
            compaction_marker: Some(true),
            summarized: Some(true),
            dropped_count: Some(42),
            dropped_preview: Some(vec![CompactionPreview {
                role: "user".to_string(),
                snippet: "example".to_string(),
            }]),
            tokens_freed_estimate: Some(999),
            ctx_pct_before: Some(87.5),
            memory_version: Some(2),
            memory_domain: Some("coding".to_string()),
            summary_kind: Some("rolling-memory".to_string()),
            compacted_at: Some(456),
            compacted_message_count_total: Some(84),
            recent_tail_kept: Some(8),
            thinking_content: None,
        };

        let json = serde_json::to_string(&msg).expect("ChatMessage should serialize");
        let decoded: ChatMessage =
            serde_json::from_str(&json).expect("ChatMessage should deserialize from own JSON");

        assert_eq!(decoded.compaction_marker, Some(true));
        assert_eq!(decoded.memory_version, Some(2));
        assert_eq!(decoded.memory_domain.as_deref(), Some("coding"));
        assert_eq!(decoded.summary_kind.as_deref(), Some("rolling-memory"));
        assert_eq!(decoded.compacted_message_count_total, Some(84));
        assert_eq!(decoded.recent_tail_kept, Some(8));
        assert_eq!(
            decoded
                .dropped_preview
                .as_ref()
                .and_then(|rows| rows.first())
                .map(|row| row.snippet.as_str()),
            Some("example")
        );
    }

    // ── Route smoke tests ──────────────────────────────────────────────────────
    // Each test sends a properly-formed request (correct method + Content-Type)
    // without an API token and asserts 401, not 404.
    //
    // A 404 means the route was accidentally deleted from api_routes().
    // A 401 means the route exists and auth is working correctly.
    //
    // These tests exist specifically to catch the regression from commit ac643ab
    // where a worktree-agent silently deleted 27 handler functions.

    fn make_all_routes()
    -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
        // Both api_token AND db_admin_token must be set so that handlers using
        // "check_db_admin_token OR check_api_token" still reject unauthenticated requests.
        let paths = crate::state::AppPaths {
            presets_path: PathBuf::new(),
            templates_path: PathBuf::new(),
            models_dir: None,
            gpu_env_path: PathBuf::new(),
            ui_settings_path: PathBuf::new(),
            sessions_path: PathBuf::new(),
            model_tags_path: PathBuf::new(),
        };
        let cs = Arc::new(
            crate::chat_storage::ChatStorage::open(&PathBuf::from(":memory:"))
                .expect("in-memory chat storage"),
        );
        let state = crate::state::AppState::new(
            vec![],
            paths,
            crate::gpu::env::GpuEnv::default(),
            crate::state::UiSettings::default(),
            cs,
            crate::config::TLSConfig::default(),
        );
        let app_config = Arc::new(crate::config::AppConfig::for_test(
            Some("test-token".to_string()),
            Some("db-admin-token".to_string()),
        ));
        let auth = AuthManager::new(None, None, &crate::config::TlsMode::None);
        super::api_routes(state, app_config, auth, "127.0.0.1".to_string())
    }

    macro_rules! route_smoke_tests {
        // $body: None for GET/DELETE (no body), Some("...json...") for POST/PUT
        ( $( ($test_name:ident, $method:expr, $path:expr, $body:expr) ),* $(,)? ) => {
            $(
                #[tokio::test]
                async fn $test_name() {
                    let routes = make_all_routes();
                    let req = warp::test::request()
                        .method($method)
                        .path($path);
                    let body_str: Option<&str> = $body;
                    let resp = if let Some(b) = body_str {
                        req.header("Content-Type", "application/json")
                           .body(b)
                           .reply(&routes)
                           .await
                    } else {
                        req.reply(&routes).await
                    };
                    assert_ne!(
                        resp.status(), 404,
                        "Route {} {} returned 404 — it may have been deleted from api_routes()",
                        $method, $path
                    );
                    assert_eq!(
                        resp.status(), 401,
                        "Route {} {} should require auth (expected 401, got {})",
                        $method, $path, resp.status()
                    );
                }
            )*
        };
    }

    route_smoke_tests![
        // Spawn wizard import
        (
            route_spawn_wizard_import,
            "POST",
            "/api/spawn-wizard/import-launch-file",
            Some("{}")
        ),
        // Chat template
        (
            route_chat_template_fetch,
            "POST",
            "/api/chat-template/fetch",
            Some("{}")
        ),
        (
            route_chat_template_upload,
            "POST",
            "/api/chat-template/upload",
            Some("{}")
        ),
        (
            route_chat_template_install_hf,
            "POST",
            "/api/chat-template/install-hf",
            Some(
                "{\"repo\":\"froggeric/Qwen-Fixed-Chat-Templates\",\"file\":\"chat_template.jinja\",\"name\":\"test\"}"
            )
        ),
        // VRAM estimation
        (
            route_vram_estimate,
            "POST",
            "/api/vram/estimate",
            Some("{}")
        ),
        (
            route_vram_estimate_breakdown,
            "POST",
            "/api/vram-estimate",
            Some("{}")
        ),
        (
            route_vram_quant_compare,
            "POST",
            "/api/vram/quant-compare",
            Some("{}")
        ),
        (
            route_vram_auto_size,
            "POST",
            "/api/vram/auto-size",
            Some("{}")
        ),
        // Model download
        (
            route_models_download_start,
            "POST",
            "/api/models/download/start",
            Some("{}")
        ),
        (
            route_models_download_status,
            "GET",
            "/api/models/download/test-id/status",
            None
        ),
        (
            route_models_download_cancel,
            "POST",
            "/api/models/download/test-id/cancel",
            Some("{}")
        ),
        // Benchmarking
        (route_benchmark, "POST", "/api/benchmark", Some("{}")),
        // Model metadata
        (
            route_model_defaults,
            "POST",
            "/api/model-defaults",
            Some("{}")
        ),
        (
            route_model_introspect,
            "POST",
            "/api/model/introspect",
            Some("{}")
        ),
        (
            route_third_party_models,
            "POST",
            "/api/third-party-models",
            Some("{}")
        ),
        // MoE tuning
        (route_moe_tune, "POST", "/api/moe-tune", Some("{}")),
        // HuggingFace
        (route_hf_search, "POST", "/api/hf/search", Some("{}")),
        (route_hf_files, "POST", "/api/hf/files", Some("{}")),
        (
            route_hf_community_picks,
            "GET",
            "/api/hf/community-picks",
            None
        ),
        (route_hf_quantizers_get, "GET", "/api/hf/quantizers", None),
        // hf_quantizers_put expects Vec<UserQuantizer> — send empty array, not {}
        (
            route_hf_quantizers_put,
            "PUT",
            "/api/hf/quantizers",
            Some("[]")
        ),
        (route_hf_download_dir, "GET", "/api/hf/download-dir", None),
        (route_hf_token_get, "GET", "/api/hf/token", None),
        (route_hf_token_put, "PUT", "/api/hf/token", Some("{}")),
        (route_hf_token_delete, "DELETE", "/api/hf/token", None),
        // hf/card requires ?repo= param — without it we expect 400, not 404
        (route_hf_card, "GET", "/api/hf/card?repo=test%2Fmodel", None),
        (
            route_hf_author_models,
            "POST",
            "/api/hf/author-models",
            Some("{}")
        ),
        (route_hf_download, "POST", "/api/hf/download", Some("{}")),
        // llama-server binary updater
        (
            route_llama_binary_version,
            "GET",
            "/api/llama-binary/version",
            None
        ),
        (
            route_llama_binary_latest,
            "GET",
            "/api/llama-binary/latest",
            None
        ),
        (
            route_llama_binary_update,
            "POST",
            "/api/llama-binary/update",
            Some("{}")
        ),
    ];
}
