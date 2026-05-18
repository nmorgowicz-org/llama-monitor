use std::collections::HashMap;
use std::fmt;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use warp::Filter;
use warp::http::StatusCode;
use warp::reject::Reject;

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

const MONITOR_INFERENCE_QUEUE_TIMEOUT: Duration = Duration::from_secs(30);
const UPSTREAM_BUSY_WAIT_TIMEOUT: Duration = Duration::from_secs(20);
const UPSTREAM_BUSY_POLL_INTERVAL: Duration = Duration::from_millis(250);
const UPSTREAM_SEND_RETRIES: usize = 3;
const UPSTREAM_SEND_RETRY_BACKOFF_MS: u64 = 250;

fn active_chat_completions_url(state: &AppState) -> Result<String, warp::Rejection> {
    let session = state
        .get_active_session()
        .ok_or(warp::reject::not_found())?;
    Ok(match &session.mode {
        crate::state::SessionMode::Spawn { port } => {
            format!("http://127.0.0.1:{port}/v1/chat/completions")
        }
        crate::state::SessionMode::Attach { endpoint } => {
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
                "The active llama-server is busy with another request. Wait for the current request to finish and retry.",
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

use crate::chat_storage::ChatStorage;

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
    let create_session = api_create_session(state.clone(), app_config.clone());
    let delete_session = api_delete_session(state.clone(), app_config.clone());
    let get_active_session = api_get_active_session(state.clone(), app_config.clone());
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
    let model_routes = get_models.or(refresh_models);
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
        .or(chat_search);
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
        .or(create_session)
        .or(delete_session)
        .or(get_active_session)
        .or(set_active_session)
        .or(get_capabilities)
        .or(spawn_session_with_preset);
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

    let agent_routes = remote_agent_latest
        .or(remote_agent_detect)
        .or(remote_agent_host_key)
        .or(remote_agent_trust_host)
        .or(remote_agent_status)
        .or(api_remote_agent_install(app_config.clone()))
        .or(api_remote_agent_start(app_config.clone()))
        .or(api_remote_agent_update(app_config.clone()))
        .or(api_remote_agent_stop(app_config.clone()));

    server_routes
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
                let (ok, _remaining) = try_cooldown(&LOGIN_LAST_ATTEMPT, now, 2);
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
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    static LAST_REMOTE_AGENT_LATEST_RELEASE: AtomicU64 = AtomicU64::new(0);

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

                let now = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs();
                let last = LAST_REMOTE_AGENT_LATEST_RELEASE.load(Ordering::Acquire);
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
                LAST_REMOTE_AGENT_LATEST_RELEASE.store(now, Ordering::Release);

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
                    let install_path = match request.get("install_path") {
                        Some(v) => v.as_str().unwrap_or("").to_string(),
                        None => crate::agent::default_install_path_for_target(&ssh_target).await,
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
                    let command = if let Some(ref conn) = ssh_connection {
                        let remote_os = crate::agent::detect_remote_os_with(conn).await;
                        let resolved_install_path =
                            crate::agent::default_install_path_for_os(remote_os);
                        crate::agent::default_start_command_for_os_with(
                            conn,
                            remote_os,
                            &resolved_install_path,
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
            futures_util::future::ready(Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(
                Box::new(warp::reply::json(&models)),
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
                                    warp::http::StatusCode::OK,
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
                                warp::http::StatusCode::OK,
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
                                warp::http::StatusCode::OK,
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
                            if name.starts_with('.') {
                                continue;
                            }
                            let meta = entry.metadata().ok();
                            let is_dir = meta.as_ref().is_some_and(|m| m.is_dir());

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
        .and(with_chat_storage(storage))
        .and_then(move |auth: Option<String>, store: Arc<ChatStorage>| {
            let cfg = app_config.clone();
            async move {
                if !check_api_token(&auth, &cfg) {
                    return Ok(unauthorized_api_token());
                }
                match store.list_tabs() {
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
        })
}

// POST /api/chat/tabs — create new tab
fn api_create_tab(
    storage: Arc<ChatStorage>,
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "chat" / "tabs")
        .and(warp::post())
        .and(warp::header::optional::<String>("authorization"))
        .and(warp::body::json::<crate::chat_storage::ChatTabRow>())
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
        .and(warp::body::json::<crate::chat_storage::ChatTabRow>())
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
        .and(warp::body::json::<crate::chat_storage::ChatTabRow>())
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
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    static LAST_CHAT_SEARCH: AtomicU64 = AtomicU64::new(0);

    #[derive(serde::Deserialize)]
    struct SearchParams {
        q: String,
        #[serde(default = "default_limit")]
        limit: usize,
        #[serde(default)]
        offset: usize,
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
                    let now = SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs();
                    let last = LAST_CHAT_SEARCH.load(Ordering::Acquire);
                    if now - last < 1 {
                        return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                            warp::reply::with_status(
                                warp::reply::json(&serde_json::json!({
                                    "ok": false,
                                    "error": "too many searches; please wait",
                                    "seconds_remaining": 1
                                })),
                                warp::http::StatusCode::TOO_MANY_REQUESTS,
                            ),
                        ));
                    }
                    LAST_CHAT_SEARCH.store(now, Ordering::Release);

                    let limit = p.limit.clamp(1, 100);
                    match store.search(&p.q, limit, p.offset) {
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

fn host_header_is_loopback(host_header: Option<&str>) -> bool {
    let Some(host) = host_header.map(str::trim).filter(|s| !s.is_empty()) else {
        return false;
    };
    let host = host.trim_matches(['[', ']']);
    let host_only = host.rsplit_once(':').map(|(name, _)| name).unwrap_or(host);
    bind_host_is_loopback(host_only)
}

fn token_bootstrap_allowed(
    auth_manager: &AuthManager,
    bind_host: &str,
    host_header: Option<&str>,
) -> bool {
    // No Auth mode: fully open (local-first).
    if !auth_manager.has_any() {
        return true;
    }
    // Auth configured: restrict token bootstrap to loopback clients.
    bind_host_is_loopback(bind_host) || host_header_is_loopback(host_header)
}

// GET /api/internal/api-token - Return internal API token for UI use
fn api_internal_token(
    app_config: Arc<AppConfig>,
    auth_manager: AuthManager,
    bind_host: String,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "internal" / "api-token")
        .and(warp::get())
        .and(warp::header::optional::<String>("host"))
        .and(with_app_config(app_config))
        .map(move |host_header: Option<String>, cfg: Arc<AppConfig>| {
            if !token_bootstrap_allowed(&auth_manager, &bind_host, host_header.as_deref()) {
                return Box::new(warp::reply::with_status(
                    warp::reply::json(&serde_json::json!({ "error": "forbidden" })),
                    warp::http::StatusCode::FORBIDDEN,
                )) as Box<dyn warp::reply::Reply>;
            }
            let live = cfg.live_api_token();
            let token = live.as_deref().unwrap_or("");
            Box::new(warp::reply::json(&serde_json::json!({ "token": token })))
        })
}

// GET /api/db/admin-token - Return DB admin token for authenticated UI use
fn api_db_admin_token(
    app_config: Arc<AppConfig>,
    auth_manager: AuthManager,
    bind_host: String,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "db" / "admin-token")
        .and(warp::get())
        .and(warp::header::optional::<String>("host"))
        .and(with_app_config(app_config))
        .map(move |host_header: Option<String>, cfg: Arc<AppConfig>| {
            if !token_bootstrap_allowed(&auth_manager, &bind_host, host_header.as_deref()) {
                return Box::new(warp::reply::with_status(
                    warp::reply::json(&serde_json::json!({ "error": "forbidden" })),
                    warp::http::StatusCode::FORBIDDEN,
                )) as Box<dyn warp::reply::Reply>;
            }
            let live = cfg.live_db_admin_token();
            let token = live.as_deref().unwrap_or("");
            Box::new(warp::reply::json(&serde_json::json!({ "token": token })))
        })
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
                            crate::state::SessionMode::Spawn { port } => format!("Spawn:{}", port),
                            crate::state::SessionMode::Attach { endpoint } => {
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
                let preset_id: String = match payload.get("preset_id") {
                    Some(v) => {
                        if let Some(s) = v.as_str() {
                            s.to_string()
                        } else {
                            return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(warp::reply::json(
                                &serde_json::json!({"ok": false, "error": "Invalid preset_id"}),
                            )));
                        }
                    }
                    None => {
                        return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(warp::reply::json(
                            &serde_json::json!({"ok": false, "error": "Missing preset_id"}),
                        )));
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
                    draft_model: preset.draft_model.clone(),
                    draft_min: preset.draft_min,
                    draft_max: preset.draft_max,
                    spec_ngram_size: preset.spec_ngram_size,
                    seed: preset.seed,
                    system_prompt_file: preset.system_prompt_file.clone(),
                    extra_args: preset.extra_args.clone(),
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

                // Check if server is reachable
                eprintln!("[info] Health-checking llama-server at {}", endpoint);
                let server_up = match client.get(&endpoint).send().await {
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

                // Check if metrics endpoint is available
                let metrics_available = client
                    .get(format!("{}/health", endpoint.trim_end_matches('/')))
                    .send()
                    .await
                    .is_ok();

                // Check if there's already an attach session for this endpoint
                let existing_session_id = {
                    let sessions = state.sessions.lock().unwrap();
                    sessions.iter().find(|s| {
                        if let crate::state::SessionMode::Attach { endpoint: ep } = &s.mode {
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
                    );
                    if !state.add_session(session) {
                        return Ok::<_, warp::Rejection>(warp::reply::with_status(
                            warp::reply::json(&serde_json::json!({"ok": false, "error": "Maximum sessions reached"})),
                            warp::http::StatusCode::OK,
                        ));
                    }
                    session_id
                };

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

                drop(sessions);
                // Clear the active session only - server_running is managed by the poller
                state.set_active_session("");

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
        assert!(token_bootstrap_allowed(&auth, "127.0.0.1", None));
        assert!(token_bootstrap_allowed(&auth, "localhost", None));
    }

    #[test]
    fn token_bootstrap_allows_all_when_no_auth_configured() {
        let auth = AuthManager::new(None, None, &TlsMode::None);
        // No Auth mode: fully open (local-first)
        assert!(token_bootstrap_allowed(&auth, "0.0.0.0", None));
        assert!(token_bootstrap_allowed(&auth, "192.168.2.44", None));
    }

    #[test]
    fn token_bootstrap_allows_loopback_host_on_non_loopback_bind() {
        let auth = AuthManager::new(None, None, &TlsMode::None);
        assert!(token_bootstrap_allowed(
            &auth,
            "0.0.0.0",
            Some("localhost:8080"),
        ));
    }

    #[test]
    fn token_bootstrap_allows_non_loopback_host_when_no_auth() {
        let auth = AuthManager::new(None, None, &TlsMode::None);
        // No Auth mode: fully open
        assert!(token_bootstrap_allowed(
            &auth,
            "0.0.0.0",
            Some("192.168.2.44:8080"),
        ));
    }

    #[test]
    fn token_bootstrap_allows_loopback_when_basic_auth_is_configured() {
        let auth = AuthManager::new(
            AuthManager::parse_credentials("admin:secret"),
            None,
            &TlsMode::None,
        );
        assert!(token_bootstrap_allowed(&auth, "127.0.0.1", None));
        assert!(!token_bootstrap_allowed(&auth, "0.0.0.0", None));
    }

    #[test]
    fn token_bootstrap_allows_loopback_when_form_auth_is_configured() {
        let auth = AuthManager::new(
            None,
            AuthManager::parse_credentials("admin:secret"),
            &TlsMode::None,
        );
        assert!(token_bootstrap_allowed(&auth, "127.0.0.1", None));
        assert!(!token_bootstrap_allowed(&auth, "0.0.0.0", None));
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
}
