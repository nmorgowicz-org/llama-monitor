use std::collections::{BTreeMap, VecDeque};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use futures_util::StreamExt;
use rand::TryRng;
use rand::rngs::SysRng;
use serde::{Deserialize, Serialize};
use warp::Filter;
use warp::http::StatusCode;

use super::common::{check_api_token, check_db_admin_token};
use super::{ApiCtx, ApiReply, ApiRoute, unauthorized_api_token, unauthorized_db_admin_token};
use crate::inference::backend::{
    BackendRecommendationInput, RecommendationArtifactKind, recommend_backend,
};
use crate::inference::rapid_mlx::compatibility;
use crate::inference::rapid_mlx::discovery::Discovery;
use crate::inference::rapid_mlx::updater::{
    ManagedReleaseChannel, ManagedReleaseSelection, ManagedRuntimeStatus, RapidMlxRuntimeManager,
    RuntimeInventoryEntry, RuntimeMutationResult,
};

const RELEASES_URL: &str =
    "https://api.github.com/repos/raullenchai/Rapid-MLX/releases?per_page=30";
const RELEASE_BY_TAG_URL: &str = "https://api.github.com/repos/raullenchai/Rapid-MLX/releases/tags";
const MAX_RELEASE_RESPONSE_BYTES: usize = 512 * 1024;
const RELEASE_CACHE_TTL: Duration = Duration::from_secs(300);
const MAX_RETAINED_JOBS: usize = 16;

type ReleaseCache = Option<(Instant, Vec<PublishedRelease>)>;

#[derive(Clone)]
struct RuntimeApiState {
    manager: Result<Arc<RapidMlxRuntimeManager>, String>,
    releases: Arc<tokio::sync::Mutex<ReleaseCache>>,
    jobs: Arc<Mutex<RuntimeJobs>>,
    client: reqwest::Client,
}

#[derive(Default)]
struct RuntimeJobs {
    entries: BTreeMap<String, RuntimeJobSnapshot>,
    order: VecDeque<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum RuntimeOperation {
    Install,
    Upgrade,
    Repair,
    Rollback,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum RuntimeJobState {
    Queued,
    Running,
    Complete,
    Failed,
}

#[derive(Debug, Clone, Serialize)]
struct RuntimeJobSnapshot {
    id: String,
    operation: RuntimeOperation,
    state: RuntimeJobState,
    message: String,
    version: Option<String>,
    result: Option<PublicRuntimeMutationResult>,
}

impl Default for RuntimeJobSnapshot {
    fn default() -> Self {
        Self {
            id: String::new(),
            operation: RuntimeOperation::Install,
            state: RuntimeJobState::Queued,
            message: String::new(),
            version: None,
            result: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
struct PublicRuntimeInventoryEntry {
    environment_id: String,
    version: String,
    release_channel: ManagedReleaseChannel,
    active: bool,
    rollback_candidate: bool,
    complete: bool,
}

impl From<RuntimeInventoryEntry> for PublicRuntimeInventoryEntry {
    fn from(entry: RuntimeInventoryEntry) -> Self {
        Self {
            environment_id: entry.environment_id,
            version: entry.version,
            release_channel: entry.release_channel,
            active: entry.active,
            rollback_candidate: entry.rollback_candidate,
            complete: entry.complete,
        }
    }
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
struct PublicRuntimeMutationResult {
    active: PublicRuntimeInventoryEntry,
    previous_environment_id: Option<String>,
}

impl From<RuntimeMutationResult> for PublicRuntimeMutationResult {
    fn from(result: RuntimeMutationResult) -> Self {
        Self {
            active: result.active.into(),
            previous_environment_id: result.previous_environment_id,
        }
    }
}

#[derive(Debug, Serialize)]
struct PublicManagedRuntimeStatus {
    supported: bool,
    installer_available: bool,
    mutation_in_progress: bool,
    rollback_available: bool,
    active: Option<PublicRuntimeInventoryEntry>,
    inventory: Vec<PublicRuntimeInventoryEntry>,
}

impl From<ManagedRuntimeStatus> for PublicManagedRuntimeStatus {
    fn from(status: ManagedRuntimeStatus) -> Self {
        Self {
            supported: status.supported,
            installer_available: status.installer_available,
            mutation_in_progress: status.mutation_in_progress,
            rollback_available: status.rollback_available,
            active: status.active.map(Into::into),
            inventory: status.inventory.into_iter().map(Into::into).collect(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
struct PublishedRelease {
    version: String,
    tag: String,
    channel: ManagedReleaseChannel,
    published_at: String,
}

impl Default for PublishedRelease {
    fn default() -> Self {
        Self {
            version: String::new(),
            tag: String::new(),
            channel: ManagedReleaseChannel::Stable,
            published_at: String::new(),
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct RuntimeMutationRequest {
    version: String,
    channel: ManagedReleaseChannel,
    confirm: String,
}

impl Default for RuntimeMutationRequest {
    fn default() -> Self {
        Self {
            version: String::new(),
            channel: ManagedReleaseChannel::Stable,
            confirm: String::new(),
        }
    }
}

#[derive(Debug, Deserialize, Default)]
#[serde(default, deny_unknown_fields)]
struct RuntimeConfirmationRequest {
    confirm: String,
}

#[derive(Debug, Deserialize, Default)]
#[serde(default, deny_unknown_fields)]
struct RecommendationRequest {
    artifact_kind: RecommendationArtifactKind,
}

#[derive(Debug, Deserialize)]
struct GithubRelease {
    #[serde(default)]
    tag_name: String,
    #[serde(default)]
    draft: bool,
    #[serde(default)]
    prerelease: bool,
    #[serde(default)]
    published_at: String,
}

pub(crate) fn routes(ctx: ApiCtx) -> ApiRoute {
    let manager = RapidMlxRuntimeManager::new(&ctx.config.config_dir)
        .map(Arc::new)
        .map_err(|_| "Managed Rapid-MLX storage is unavailable".to_string());
    let client = reqwest::Client::builder()
        .user_agent("llama-monitor/rapid-mlx-runtime-manager")
        .timeout(Duration::from_secs(20))
        .build()
        .expect("static Rapid-MLX release client configuration must be valid");
    let state = RuntimeApiState {
        manager,
        releases: Arc::new(tokio::sync::Mutex::new(None)),
        jobs: Arc::new(Mutex::new(RuntimeJobs::default())),
        client,
    };

    status_route(ctx.clone(), state.clone())
        .or(releases_route(ctx.clone(), state.clone()))
        .unify()
        .or(recommendation_route(ctx.clone(), state.clone()))
        .unify()
        .or(mutation_route(
            ctx.clone(),
            state.clone(),
            RuntimeOperation::Install,
        ))
        .unify()
        .or(mutation_route(
            ctx.clone(),
            state.clone(),
            RuntimeOperation::Upgrade,
        ))
        .unify()
        .or(simple_mutation_route(
            ctx.clone(),
            state.clone(),
            RuntimeOperation::Repair,
        ))
        .unify()
        .or(simple_mutation_route(
            ctx.clone(),
            state.clone(),
            RuntimeOperation::Rollback,
        ))
        .unify()
        .or(job_route(ctx, state))
        .unify()
        .boxed()
}

fn recommendation_route(ctx: ApiCtx, state: RuntimeApiState) -> ApiRoute {
    let config = ctx.config;
    warp::path!("api" / "rapid-mlx" / "recommend")
        .and(warp::post())
        .and(warp::header::optional::<String>("authorization"))
        .and(super::super::safe_json_body::<RecommendationRequest>())
        .and_then(
            move |auth: Option<String>, request: RecommendationRequest| {
                let config = config.clone();
                let state = state.clone();
                async move {
                    if !check_api_token(&auth, &config) {
                        return Ok(unauthorized_api_token());
                    }
                    let local_available =
                        crate::inference::rapid_mlx::ensure_local_platform_supported().is_ok();
                    let runtime_compatible = if local_available {
                        compatible_runtime_available(&state).await
                    } else {
                        false
                    };
                    let recommendation = recommend_backend(&BackendRecommendationInput {
                        artifact_kind: request.artifact_kind,
                        rapid_mlx_local_available: local_available,
                        rapid_mlx_runtime_compatible: runtime_compatible,
                    });
                    Ok::<ApiReply, warp::Rejection>(Box::new(warp::reply::json(&recommendation)))
                }
            },
        )
        .boxed()
}

async fn compatible_runtime_available(state: &RuntimeApiState) -> bool {
    let managed_active = match manager(state) {
        Ok(manager) => tokio::task::spawn_blocking(move || manager.status())
            .await
            .ok()
            .and_then(Result::ok)
            .and_then(|status| {
                status
                    .active
                    .map(|active| (active.executable_path, active.release_channel))
            }),
        Err(_) => None,
    };
    let managed_path = managed_active.as_ref().map(|(path, _)| path.as_path());
    let Ok((binary, source)) = Discovery::resolve_binary(None, managed_path).await else {
        return false;
    };
    if source == crate::inference::rapid_mlx::runtime::RuntimeSource::Managed {
        let allow_prerelease = managed_prerelease_allowed(managed_active.as_ref(), &binary);
        compatibility::probe_published_managed_release(&binary, allow_prerelease)
            .await
            .is_ok()
    } else {
        compatibility::probe(&binary, source).await.is_ok()
    }
}

fn managed_prerelease_allowed(
    managed_active: Option<&(std::path::PathBuf, ManagedReleaseChannel)>,
    resolved_binary: &std::path::Path,
) -> bool {
    managed_active.is_some_and(|(path, channel)| {
        path == resolved_binary && *channel == ManagedReleaseChannel::Prerelease
    })
}

fn status_route(ctx: ApiCtx, state: RuntimeApiState) -> ApiRoute {
    let config = ctx.config;
    warp::path!("api" / "rapid-mlx" / "runtime" / "status")
        .and(warp::get())
        .and(warp::header::optional::<String>("authorization"))
        .and_then(move |auth: Option<String>| {
            let config = config.clone();
            let state = state.clone();
            async move {
                if !check_api_token(&auth, &config) {
                    return Ok(unauthorized_api_token());
                }
                let manager = match manager(&state) {
                    Ok(manager) => manager,
                    Err(message) => {
                        return Ok(json_error(StatusCode::INTERNAL_SERVER_ERROR, message));
                    }
                };
                let status = tokio::task::spawn_blocking(move || manager.status()).await;
                match status {
                    Ok(Ok(status)) => Ok::<ApiReply, warp::Rejection>(Box::new(warp::reply::json(
                        &serde_json::json!({
                            "runtime": PublicManagedRuntimeStatus::from(status),
                            "jobs": job_list(&state),
                        }),
                    ))),
                    _ => Ok(json_error(
                        StatusCode::INTERNAL_SERVER_ERROR,
                        "Managed Rapid-MLX status is unavailable",
                    )),
                }
            }
        })
        .boxed()
}

fn releases_route(ctx: ApiCtx, state: RuntimeApiState) -> ApiRoute {
    let config = ctx.config;
    warp::path!("api" / "rapid-mlx" / "runtime" / "releases")
        .and(warp::get())
        .and(warp::header::optional::<String>("authorization"))
        .and_then(move |auth: Option<String>| {
            let config = config.clone();
            let state = state.clone();
            async move {
                if !check_api_token(&auth, &config) {
                    return Ok(unauthorized_api_token());
                }
                match published_releases(&state).await {
                    Ok(releases) => Ok::<ApiReply, warp::Rejection>(Box::new(warp::reply::json(
                        &serde_json::json!({ "releases": releases }),
                    ))),
                    Err(_) => Ok(json_error(
                        StatusCode::BAD_GATEWAY,
                        "Rapid-MLX release discovery is temporarily unavailable",
                    )),
                }
            }
        })
        .boxed()
}

fn mutation_route(ctx: ApiCtx, state: RuntimeApiState, operation: RuntimeOperation) -> ApiRoute {
    let config = ctx.config;
    let action = match operation {
        RuntimeOperation::Install => "install",
        RuntimeOperation::Upgrade => "upgrade",
        _ => unreachable!("release mutation route requires install or upgrade"),
    };
    warp::path("api")
        .and(warp::path("rapid-mlx"))
        .and(warp::path("runtime"))
        .and(warp::path(action))
        .and(warp::path::end())
        .and(warp::post())
        .and(warp::header::optional::<String>("authorization"))
        .and(super::super::safe_json_body::<RuntimeMutationRequest>())
        .and_then(
            move |auth: Option<String>, request: RuntimeMutationRequest| {
                let config = config.clone();
                let state = state.clone();
                async move {
                    if !check_db_admin_token(&auth, &config) {
                        return Ok(unauthorized_db_admin_token());
                    }
                    let expected = format!("{}_RAPID_MLX_RUNTIME", action.to_ascii_uppercase());
                    if request.confirm != expected || request.version.is_empty() {
                        return Ok(json_error(
                            StatusCode::BAD_REQUEST,
                            format!("Confirmation must be {expected} with an exact version"),
                        ));
                    }
                    if crate::inference::rapid_mlx::ensure_local_platform_supported().is_err() {
                        return Ok(json_error(
                            StatusCode::BAD_REQUEST,
                            "Managed Rapid-MLX runtime changes require Apple Silicon macOS",
                        ));
                    }
                    let release = match select_published_release(
                        &state,
                        &request.version,
                        request.channel,
                    )
                    .await
                    {
                        Ok(release) => release,
                        Err(_) => {
                            return Ok(json_error(
                                StatusCode::BAD_REQUEST,
                                "The selected Rapid-MLX release was not found in published release metadata",
                            ));
                        }
                    };
                    start_job(&state, operation, Some(release)).await
                }
            },
        )
        .boxed()
}

fn simple_mutation_route(
    ctx: ApiCtx,
    state: RuntimeApiState,
    operation: RuntimeOperation,
) -> ApiRoute {
    let config = ctx.config;
    let action = match operation {
        RuntimeOperation::Repair => "repair",
        RuntimeOperation::Rollback => "rollback",
        _ => unreachable!("simple mutation route requires repair or rollback"),
    };
    warp::path("api")
        .and(warp::path("rapid-mlx"))
        .and(warp::path("runtime"))
        .and(warp::path(action))
        .and(warp::path::end())
        .and(warp::post())
        .and(warp::header::optional::<String>("authorization"))
        .and(super::super::safe_json_body::<RuntimeConfirmationRequest>())
        .and_then(
            move |auth: Option<String>, request: RuntimeConfirmationRequest| {
                let config = config.clone();
                let state = state.clone();
                async move {
                    if !check_db_admin_token(&auth, &config) {
                        return Ok(unauthorized_db_admin_token());
                    }
                    let expected = format!("{}_RAPID_MLX_RUNTIME", action.to_ascii_uppercase());
                    if request.confirm != expected {
                        return Ok(json_error(
                            StatusCode::BAD_REQUEST,
                            format!("Confirmation must be {expected}"),
                        ));
                    }
                    if crate::inference::rapid_mlx::ensure_local_platform_supported().is_err() {
                        return Ok(json_error(
                            StatusCode::BAD_REQUEST,
                            "Managed Rapid-MLX runtime changes require Apple Silicon macOS",
                        ));
                    }
                    let release = if operation == RuntimeOperation::Repair {
                        match published_selection_for_active_runtime(&state).await {
                            Ok(release) => Some(release),
                            Err(_) => {
                                return Ok(json_error(
                                    StatusCode::BAD_REQUEST,
                                    "The active managed Rapid-MLX runtime could not be verified against published release metadata",
                                ));
                            }
                        }
                    } else {
                        None
                    };
                    start_job(&state, operation, release).await
                }
            },
        )
        .boxed()
}

async fn published_selection_for_active_runtime(
    state: &RuntimeApiState,
) -> anyhow::Result<ManagedReleaseSelection> {
    let manager = manager(state).map_err(|message| anyhow::anyhow!(message))?;
    let status = tokio::task::spawn_blocking(move || manager.status()).await??;
    let active = status
        .active
        .ok_or_else(|| anyhow::anyhow!("No active managed runtime"))?;
    select_published_release(state, &active.version, active.release_channel).await
}

fn job_route(ctx: ApiCtx, state: RuntimeApiState) -> ApiRoute {
    let config = ctx.config;
    warp::path!("api" / "rapid-mlx" / "runtime" / "jobs" / String)
        .and(warp::get())
        .and(warp::header::optional::<String>("authorization"))
        .and_then(move |id: String, auth: Option<String>| {
            let config = config.clone();
            let state = state.clone();
            async move {
                if !check_api_token(&auth, &config) {
                    return Ok(unauthorized_api_token());
                }
                match state
                    .jobs
                    .lock()
                    .ok()
                    .and_then(|jobs| jobs.entries.get(&id).cloned())
                {
                    Some(job) => Ok::<ApiReply, warp::Rejection>(Box::new(warp::reply::json(&job))),
                    None => Ok(json_error(
                        StatusCode::NOT_FOUND,
                        "Runtime operation was not found",
                    )),
                }
            }
        })
        .boxed()
}

async fn start_job(
    state: &RuntimeApiState,
    operation: RuntimeOperation,
    release: Option<ManagedReleaseSelection>,
) -> Result<ApiReply, warp::Rejection> {
    let manager = match manager(state) {
        Ok(manager) => manager,
        Err(message) => return Ok(json_error(StatusCode::INTERNAL_SERVER_ERROR, message)),
    };
    let status_manager = manager.clone();
    if tokio::task::spawn_blocking(move || status_manager.status())
        .await
        .is_ok_and(|status| status.is_ok_and(|status| status.mutation_in_progress))
    {
        return Ok(json_error(
            StatusCode::TOO_MANY_REQUESTS,
            "Another managed Rapid-MLX runtime operation is already in progress",
        ));
    }
    let id = match random_job_id() {
        Ok(id) => id,
        Err(_) => {
            return Ok(json_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Secure runtime operation ID generation is unavailable",
            ));
        }
    };
    let version = release.as_ref().map(|item| item.version().to_string());
    if !try_insert_job(
        state,
        RuntimeJobSnapshot {
            id: id.clone(),
            operation,
            state: RuntimeJobState::Queued,
            message: "Runtime operation queued".into(),
            version,
            result: None,
        },
    ) {
        return Ok(json_error(
            StatusCode::TOO_MANY_REQUESTS,
            "Another managed Rapid-MLX runtime operation is already in progress",
        ));
    }
    let job_state = state.clone();
    let job_id = id.clone();
    tokio::spawn(async move {
        update_job(
            &job_state,
            &job_id,
            RuntimeJobState::Running,
            "Installing and validating an isolated runtime",
            None,
        );
        let result = match (operation, release) {
            (RuntimeOperation::Install, Some(release)) => manager.install_release(release).await,
            (RuntimeOperation::Upgrade, Some(release)) => manager.upgrade_release(release).await,
            (RuntimeOperation::Repair, Some(release)) => manager.repair_release(release).await,
            (RuntimeOperation::Rollback, None) => manager.rollback().await,
            _ => Err(anyhow::anyhow!("Invalid runtime operation state")),
        };
        match result {
            Ok(result) => update_job(
                &job_state,
                &job_id,
                RuntimeJobState::Complete,
                "Runtime validated and activated",
                Some(result),
            ),
            Err(error) => update_job(
                &job_state,
                &job_id,
                RuntimeJobState::Failed,
                public_runtime_error(&error),
                None,
            ),
        }
    });

    Ok(Box::new(warp::reply::with_status(
        warp::reply::json(&serde_json::json!({ "job_id": id, "state": "queued" })),
        StatusCode::ACCEPTED,
    )))
}

fn manager(state: &RuntimeApiState) -> Result<Arc<RapidMlxRuntimeManager>, &'static str> {
    state
        .manager
        .as_ref()
        .cloned()
        .map_err(|_| "Managed Rapid-MLX storage is unavailable")
}

async fn published_releases(state: &RuntimeApiState) -> anyhow::Result<Vec<PublishedRelease>> {
    {
        let cache = state.releases.lock().await;
        if let Some((updated, releases)) = cache.as_ref()
            && updated.elapsed() < RELEASE_CACHE_TTL
        {
            return Ok(releases.clone());
        }
    }

    let body = fetch_bounded_release_body(&state.client, RELEASES_URL).await?;
    let releases = decode_published_releases(&body)?;
    *state.releases.lock().await = Some((Instant::now(), releases.clone()));
    Ok(releases)
}

async fn fetch_bounded_release_body(
    client: &reqwest::Client,
    url: &str,
) -> anyhow::Result<Vec<u8>> {
    let response = client.get(url).send().await?;
    if !response.status().is_success() {
        anyhow::bail!("GitHub release metadata returned a non-success status");
    }
    let mut stream = response.bytes_stream();
    let mut body = Vec::new();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        if body.len().saturating_add(chunk.len()) > MAX_RELEASE_RESPONSE_BYTES {
            anyhow::bail!("GitHub release metadata exceeded its response bound");
        }
        body.extend_from_slice(&chunk);
    }
    Ok(body)
}

fn decode_published_releases(body: &[u8]) -> anyhow::Result<Vec<PublishedRelease>> {
    let raw: Vec<GithubRelease> = serde_json::from_slice(body)?;
    Ok(decode_github_releases(raw))
}

fn decode_github_releases(raw: Vec<GithubRelease>) -> Vec<PublishedRelease> {
    raw.into_iter()
        .filter(|item| !item.draft)
        .filter_map(|item| {
            let version = item.tag_name.strip_prefix('v')?.to_string();
            let channel = if item.prerelease {
                ManagedReleaseChannel::Prerelease
            } else {
                ManagedReleaseChannel::Stable
            };
            RapidMlxRuntimeManager::validate_published_version(&version, channel).ok()?;
            Some(PublishedRelease {
                version,
                tag: item.tag_name,
                channel,
                published_at: item.published_at,
            })
        })
        .collect()
}

async fn select_published_release(
    state: &RuntimeApiState,
    version: &str,
    channel: ManagedReleaseChannel,
) -> anyhow::Result<ManagedReleaseSelection> {
    RapidMlxRuntimeManager::validate_published_version(version, channel)?;
    if let Ok(selection) =
        select_release_from_metadata(published_releases(state).await?, version, channel)
    {
        return Ok(selection);
    }

    let url = format!("{RELEASE_BY_TAG_URL}/v{version}");
    let body = fetch_bounded_release_body(&state.client, &url).await?;
    let release: GithubRelease = serde_json::from_slice(&body)?;
    select_release_from_metadata(decode_github_releases(vec![release]), version, channel)
}

fn select_release_from_metadata(
    releases: Vec<PublishedRelease>,
    version: &str,
    channel: ManagedReleaseChannel,
) -> anyhow::Result<ManagedReleaseSelection> {
    let release = releases
        .into_iter()
        .find(|release| release.version == version && release.channel == channel)
        .ok_or_else(|| anyhow::anyhow!("Release was not found"))?;
    ManagedReleaseSelection::from_published_release(release.version, release.channel)
}

fn random_job_id() -> anyhow::Result<String> {
    let mut bytes = [0_u8; 16];
    SysRng.try_fill_bytes(&mut bytes)?;
    Ok(bytes.iter().map(|byte| format!("{byte:02x}")).collect())
}

fn try_insert_job(state: &RuntimeApiState, snapshot: RuntimeJobSnapshot) -> bool {
    if let Ok(mut jobs) = state.jobs.lock() {
        if jobs.entries.values().any(|job| {
            matches!(
                job.state,
                RuntimeJobState::Queued | RuntimeJobState::Running
            )
        }) {
            return false;
        }
        while jobs.order.len() >= MAX_RETAINED_JOBS {
            if let Some(oldest) = jobs.order.pop_front() {
                jobs.entries.remove(&oldest);
            }
        }
        jobs.order.push_back(snapshot.id.clone());
        jobs.entries.insert(snapshot.id.clone(), snapshot);
        true
    } else {
        false
    }
}

fn update_job(
    state: &RuntimeApiState,
    id: &str,
    job_state: RuntimeJobState,
    message: impl Into<String>,
    result: Option<RuntimeMutationResult>,
) {
    if let Ok(mut jobs) = state.jobs.lock()
        && let Some(job) = jobs.entries.get_mut(id)
    {
        job.state = job_state;
        job.message = message.into();
        job.result = result.map(Into::into);
    }
}

fn job_list(state: &RuntimeApiState) -> Vec<RuntimeJobSnapshot> {
    state
        .jobs
        .lock()
        .map(|jobs| {
            jobs.order
                .iter()
                .rev()
                .filter_map(|id| jobs.entries.get(id).cloned())
                .collect()
        })
        .unwrap_or_default()
}

fn public_runtime_error(error: &anyhow::Error) -> &'static str {
    let text = error.to_string();
    if text.contains("already in progress") {
        "Another managed Rapid-MLX runtime operation is already in progress"
    } else if text.contains("require macOS on Apple Silicon") {
        "Managed Rapid-MLX runtime changes require Apple Silicon macOS"
    } else if text.contains("No previous known-good") {
        "No previous known-good Rapid-MLX runtime is available"
    } else if text.contains("No active managed") {
        "No active managed Rapid-MLX runtime is available"
    } else {
        "Managed Rapid-MLX validation failed safely; the active runtime was not changed"
    }
}

fn json_error(status: StatusCode, message: impl Into<String>) -> ApiReply {
    Box::new(warp::reply::with_status(
        warp::reply::json(&serde_json::json!({ "ok": false, "error": message.into() })),
        status,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_state() -> RuntimeApiState {
        RuntimeApiState {
            manager: Err("unused".into()),
            releases: Arc::new(tokio::sync::Mutex::new(None)),
            jobs: Arc::new(Mutex::new(RuntimeJobs::default())),
            client: reqwest::Client::new(),
        }
    }

    fn queued_job(id: &str) -> RuntimeJobSnapshot {
        RuntimeJobSnapshot {
            id: id.into(),
            operation: RuntimeOperation::Install,
            state: RuntimeJobState::Queued,
            message: String::new(),
            version: Some("0.10.10".into()),
            result: None,
        }
    }

    #[test]
    fn runtime_errors_never_disclose_worker_paths() {
        let error = anyhow::anyhow!(
            "uv failed at /Users/person/.config/llama-monitor/runtimes/rapid-mlx/secret"
        );
        assert!(!public_runtime_error(&error).contains("/Users/"));
    }

    #[test]
    fn release_request_rejects_unknown_fields() {
        assert!(
            serde_json::from_str::<RuntimeMutationRequest>(
                r#"{"version":"0.10.10","channel":"stable","confirm":"x","extra":true}"#,
            )
            .is_err()
        );
    }

    #[test]
    fn release_selection_requires_exact_published_version_and_channel() {
        let releases = vec![PublishedRelease {
            version: "0.10.10".into(),
            tag: "v0.10.10".into(),
            channel: ManagedReleaseChannel::Stable,
            published_at: "2026-07-16T00:00:00Z".into(),
        }];
        assert!(
            select_release_from_metadata(
                releases.clone(),
                "0.10.10",
                ManagedReleaseChannel::Stable,
            )
            .is_ok()
        );
        assert!(
            select_release_from_metadata(
                releases.clone(),
                "0.10.11",
                ManagedReleaseChannel::Stable,
            )
            .is_err()
        );
        assert!(
            select_release_from_metadata(releases, "0.10.10", ManagedReleaseChannel::Prerelease,)
                .is_err()
        );
    }

    #[test]
    fn release_discovery_filters_drafts_and_versions_below_floor() {
        let releases = decode_published_releases(
            br#"[
                {"tag_name":"v0.10.10","draft":false,"prerelease":false},
                {"tag_name":"v0.10.8","draft":false,"prerelease":false},
                {"tag_name":"v0.10.11rc1","draft":false,"prerelease":true},
                {"tag_name":"v0.10.12","draft":true,"prerelease":false}
            ]"#,
        )
        .unwrap();
        assert_eq!(releases.len(), 2);
        assert_eq!(releases[0].version, "0.10.10");
        assert_eq!(releases[1].version, "0.10.11rc1");
    }

    #[test]
    fn api_job_admission_is_atomic() {
        let state = test_state();
        assert!(try_insert_job(&state, queued_job("first")));
        assert!(!try_insert_job(&state, queued_job("second")));
        update_job(&state, "first", RuntimeJobState::Complete, "done", None);
        assert!(try_insert_job(&state, queued_job("second")));
    }

    #[test]
    fn public_runtime_payloads_never_disclose_executable_paths() {
        let internal = RuntimeInventoryEntry {
            environment_id: "rapid-mlx-0.10.10-test".into(),
            version: "0.10.10".into(),
            release_channel: ManagedReleaseChannel::Stable,
            executable_path: "/Users/person/.config/llama-monitor/private/rapid-mlx".into(),
            active: true,
            rollback_candidate: false,
            complete: true,
        };
        let public = PublicRuntimeInventoryEntry::from(internal);
        let json = serde_json::to_string(&public).unwrap();
        assert!(!json.contains("/Users/person"));
        assert!(!json.contains("executable"));
    }

    #[test]
    fn managed_prerelease_probe_policy_follows_active_manifest_channel() {
        let binary = std::path::PathBuf::from("/managed/rapid-mlx");
        assert!(managed_prerelease_allowed(
            Some(&(binary.clone(), ManagedReleaseChannel::Prerelease)),
            &binary,
        ));
        assert!(!managed_prerelease_allowed(
            Some(&(binary.clone(), ManagedReleaseChannel::Stable)),
            &binary,
        ));
        assert!(!managed_prerelease_allowed(
            Some(&(
                std::path::PathBuf::from("/other/rapid-mlx"),
                ManagedReleaseChannel::Prerelease,
            )),
            &binary,
        ));
    }
}
