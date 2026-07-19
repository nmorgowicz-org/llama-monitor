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
use crate::inference::rapid_mlx::RapidMlxConfig;
use crate::inference::rapid_mlx::changelog;
use crate::inference::rapid_mlx::compatibility;
use crate::inference::rapid_mlx::discovery::Discovery;
use crate::inference::rapid_mlx::info_query;
use crate::inference::rapid_mlx::model_resolver::{
    AuthoritativeSafetensorsSource, RapidMlxModelSource,
};
use crate::inference::rapid_mlx::updater::{
    ManagedReleaseChannel, ManagedReleaseSelection, ManagedRuntimeStatus, RapidMlxRuntimeManager,
    RuntimeInventoryEntry, RuntimeMutationResult,
};
use crate::state::{DoctorFinding, DoctorFindingType, DoctorSeverity, FixAction};

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
    changelog_cache: Arc<changelog::ChangelogCacheManager>,
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
        changelog_cache: Arc::new(changelog::ChangelogCacheManager::new()),
        client,
    };

    status_route(ctx.clone(), state.clone())
        .or(releases_route(ctx.clone(), state.clone()))
        .unify()
        .or(changelog_route(ctx.clone(), state.clone()))
        .unify()
        .or(recommendation_route(ctx.clone(), state.clone()))
        .unify()
        .or(doctor_route(ctx.clone(), state.clone()))
        .unify()
        .or(flag_advisor_route(ctx.clone()))
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
        .or(job_route(ctx.clone(), state.clone()))
        .unify()
        .or(profile_route(ctx, state))
        .unify()
        .or(escape_hatch_route())
        .unify()
        .boxed()
}

fn escape_hatch_route() -> ApiRoute {
    warp::path!("api" / "rapid-mlx" / "escape-hatch-flags")
        .and(warp::get())
        .map(|| {
            Box::new(warp::reply::json(
                &crate::inference::rapid_mlx::escape_hatch::ALLOWED_ESCAPE_FLAGS,
            )) as ApiReply
        })
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

fn changelog_route(ctx: ApiCtx, state: RuntimeApiState) -> ApiRoute {
    let config = ctx.config;
    warp::path!("api" / "rapid-mlx" / "runtime" / "changelog")
        .and(warp::get())
        .and(warp::query::<ChangelogQuery>())
        .and(warp::header::optional::<String>("authorization"))
        .and_then(move |query: ChangelogQuery, auth: Option<String>| {
            let config = config.clone();
            let state = state.clone();
            async move {
                if !check_api_token(&auth, &config) {
                    return Ok(unauthorized_api_token());
                }
                let result = changelog::fetch_compare(
                    &state.client,
                    &state.changelog_cache,
                    &query.from,
                    &query.to,
                )
                .await;

                match result {
                    Ok(summary) => Ok::<ApiReply, warp::Rejection>(Box::new(warp::reply::json(
                        &serde_json::json!({ "ok": true, "changelog": summary }),
                    ))),
                    Err(err) => {
                        let status = match err.kind {
                            changelog::ChangelogErrorKind::RateLimited => {
                                StatusCode::TOO_MANY_REQUESTS
                            }
                            changelog::ChangelogErrorKind::InvalidTag => StatusCode::NOT_FOUND,
                            _ => StatusCode::BAD_GATEWAY,
                        };
                        Ok(Box::new(warp::reply::with_status(
                            warp::reply::json(&serde_json::json!({
                                "ok": false,
                                "error": err.message,
                                "kind": &err.kind,
                            })),
                            status,
                        )))
                    }
                }
            }
        })
        .boxed()
}

#[derive(Debug, Deserialize, Default)]
#[serde(default, deny_unknown_fields)]
struct ChangelogQuery {
    from: String,
    to: String,
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

fn profile_route(ctx: ApiCtx, state: RuntimeApiState) -> ApiRoute {
    let config = ctx.config;
    warp::path!("api" / "rapid-mlx" / "models" / String / "profile")
        .and(warp::get())
        .and(warp::header::optional::<String>("authorization"))
        .and_then(move |model_id: String, auth: Option<String>| {
            let config = config.clone();
            let state = state.clone();
            async move {
                if !check_api_token(&auth, &config) {
                    return Ok(unauthorized_api_token());
                }
                if crate::inference::rapid_mlx::ensure_local_platform_supported().is_err() {
                    return Ok(json_error(
                        StatusCode::BAD_REQUEST,
                        "Model profile queries require Apple Silicon macOS",
                    ));
                }
                let manager_result = manager(&state);
                let mut active_info: Option<(std::path::PathBuf, ManagedReleaseChannel)> = None;
                if let Ok(manager) = &manager_result {
                    let manager = manager.clone();
                    if let Ok(Ok(status)) =
                        tokio::task::spawn_blocking(move || manager.status()).await
                        && let Some(active) = status.active
                    {
                        active_info = Some((active.executable_path, active.release_channel));
                    }
                }
                let binary_path = active_info.as_ref().map(|(p, _)| p.as_path());
                let Ok((binary, source)) = Discovery::resolve_binary(None, binary_path).await
                else {
                    return Ok(json_error(
                        StatusCode::NOT_FOUND,
                        "Rapid-MLX binary not found. Run rapid-mlx doctor or install via Settings.",
                    ));
                };
                let allow_prerelease = active_info
                    .as_ref()
                    .is_some_and(|(p, c)| p == &binary && *c == ManagedReleaseChannel::Prerelease);
                if source == crate::inference::rapid_mlx::runtime::RuntimeSource::Managed {
                    if allow_prerelease {
                        if compatibility::probe_published_managed_release(&binary, allow_prerelease)
                            .await
                            .is_err()
                        {
                            return Ok(json_error(
                                StatusCode::SERVICE_UNAVAILABLE,
                                "Rapid-MLX runtime probe failed",
                            ));
                        }
                    } else if compatibility::probe(&binary, source).await.is_err() {
                        return Ok(json_error(
                            StatusCode::SERVICE_UNAVAILABLE,
                            "Rapid-MLX runtime probe failed",
                        ));
                    }
                } else if compatibility::probe(&binary, source).await.is_err() {
                    return Ok(json_error(
                        StatusCode::SERVICE_UNAVAILABLE,
                        "Rapid-MLX runtime probe failed",
                    ));
                }
                match info_query::fetch_model_profile(&binary, &model_id).await {
                    Ok(Some(profile)) => Ok::<ApiReply, warp::Rejection>(Box::new(
                        warp::reply::json(&serde_json::json!({ "ok": true, "profile": profile })),
                    )),
                    Ok(None) => Ok(json_error(
                        StatusCode::NOT_FOUND,
                        format!(
                            "Model '{}' not recognized by this Rapid-MLX installation",
                            model_id
                        ),
                    )),
                    Err(error) => {
                        let msg = error.to_string();
                        if msg.contains("timed out") {
                            Ok(json_error(
                                StatusCode::REQUEST_TIMEOUT,
                                "Rapid-MLX info query timed out",
                            ))
                        } else {
                            Ok(json_error(
                                StatusCode::INTERNAL_SERVER_ERROR,
                                "Rapid-MLX info query failed",
                            ))
                        }
                    }
                }
            }
        })
        .boxed()
}

fn doctor_route(ctx: ApiCtx, _state: RuntimeApiState) -> ApiRoute {
    let config = ctx.config;
    warp::path!("api" / "rapid-mlx" / "doctor")
        .and(warp::get())
        .and(warp::header::optional::<String>("authorization"))
        .and_then(move |auth: Option<String>| {
            let config = config.clone();
            async move {
                if !check_api_token(&auth, &config) {
                    return Ok(unauthorized_api_token());
                }
                if crate::inference::rapid_mlx::ensure_local_platform_supported().is_err() {
                    return Ok(json_error(
                        StatusCode::BAD_REQUEST,
                        "rapid-mlx doctor requires Apple Silicon macOS",
                    ));
                }
                let Ok((binary, _)) = Discovery::resolve_binary(None, None).await else {
                    return Ok(json_error(
                        StatusCode::SERVICE_UNAVAILABLE,
                        "rapid-mlx binary not found on PATH",
                    ));
                };

                let version = run_rapid_mlx_version(&binary).await;
                if version.is_err() {
                    return Ok(json_error(
                        StatusCode::SERVICE_UNAVAILABLE,
                        "rapid-mlx version check failed",
                    ));
                }
                let version_str = version.unwrap_or_default();

                // No `--json` output exists on any `rapid-mlx` subcommand, so
                // `parse_doctor_output` scrapes human box-drawing/glyph text whose
                // layout is only guaranteed on trusted versions. Below the trusted
                // minor, degrade to raw-output-only with no structured findings
                // rather than risk misparsing an unknown layout.
                let version_trusted = match info_query::cached_version(&binary).await {
                    Ok(Some((_, minor))) => minor >= info_query::MIN_TRUSTED_MINOR,
                    _ => false,
                };

                let doctor_output = run_rapid_mlx_doctor(&binary).await;
                match doctor_output {
                    Ok(output) => {
                        let findings = if version_trusted {
                            parse_doctor_output(&output)
                        } else {
                            Vec::new()
                        };
                        Ok::<ApiReply, warp::Rejection>(Box::new(warp::reply::json(
                            &serde_json::json!({
                                "ok": true,
                                "version": version_str,
                                "version_trusted": version_trusted,
                                "findings": findings,
                                "raw_output": output
                            }),
                        )))
                    }
                    Err(_) => Ok(json_error(
                        StatusCode::INTERNAL_SERVER_ERROR,
                        "rapid-mlx doctor command failed",
                    )),
                }
            }
        })
        .boxed()
}

/// Preset-flag advisor: diffs the active session's model `rapid-mlx info` profile
/// against the active preset's Rapid-MLX launch flags and emits `DoctorFinding`s
/// with `fix: Some(FixAction::…)` where the preset is missing a flag the model's
/// declared capabilities imply it needs. Findings from this route flow into the
/// same diagnostics panel as `doctor_route`'s findings (which always keep
/// `fix: None`) via the frontend's `loadDoctorFindings`.
fn flag_advisor_route(ctx: ApiCtx) -> ApiRoute {
    let config = ctx.config;
    let state = ctx.state;
    warp::path!("api" / "rapid-mlx" / "flag-advisor")
        .and(warp::get())
        .and(warp::header::optional::<String>("authorization"))
        .and_then(move |auth: Option<String>| {
            let config = config.clone();
            let state = state.clone();
            async move {
                if !check_api_token(&auth, &config) {
                    return Ok(unauthorized_api_token());
                }

                fn empty_findings() -> ApiReply {
                    Box::new(warp::reply::json(&serde_json::json!({
                        "ok": true,
                        "findings": Vec::<DoctorFinding>::new()
                    })))
                }

                let active_session_id = state.active_session_id.lock().unwrap().clone();
                if active_session_id.is_empty() {
                    return Ok::<ApiReply, warp::Rejection>(empty_findings());
                }

                let preset_id = {
                    let sessions = state.sessions.lock().unwrap();
                    sessions
                        .iter()
                        .find(|s| s.id == active_session_id)
                        .and_then(|s| {
                            if s.preset_id.is_empty() {
                                None
                            } else {
                                Some(s.preset_id.clone())
                            }
                        })
                };
                let Some(preset_id) = preset_id else {
                    return Ok(empty_findings());
                };

                let rapid_config = {
                    let presets = state.presets.lock().unwrap();
                    presets
                        .iter()
                        .find(|p| p.id == preset_id)
                        .and_then(|p| p.rapid_mlx.clone())
                };
                let Some(rapid_config) = rapid_config else {
                    return Ok(empty_findings());
                };

                if crate::inference::rapid_mlx::ensure_local_platform_supported().is_err() {
                    return Ok(empty_findings());
                }
                let Ok((binary, _)) = Discovery::resolve_binary(None, None).await else {
                    return Ok(empty_findings());
                };
                let Some(model_id) = model_id_for_info(&rapid_config) else {
                    return Ok(empty_findings());
                };

                let profile = match info_query::fetch_model_profile(&binary, &model_id).await {
                    Ok(Some(profile)) => profile,
                    _ => return Ok(empty_findings()),
                };

                let findings = build_flag_advisor_findings(&profile, &rapid_config);
                Ok::<ApiReply, warp::Rejection>(Box::new(warp::reply::json(&serde_json::json!({
                    "ok": true,
                    "findings": findings
                }))))
            }
        })
        .boxed()
}

/// Extract the identifier to pass to `rapid-mlx info <id>` from a preset's
/// Rapid-MLX model source. Only sources `rapid-mlx info` can resolve (aliases,
/// HuggingFace repos) are supported; local directory/GGUF-file sources have no
/// equivalent `info` lookup and are skipped (advisor degrades to no findings).
fn model_id_for_info(config: &RapidMlxConfig) -> Option<String> {
    match &config.model_source {
        Some(RapidMlxModelSource::Alias { value }) => Some(value.clone()),
        Some(RapidMlxModelSource::HuggingFaceRepo { repo_id, .. }) => Some(repo_id.clone()),
        Some(RapidMlxModelSource::AuthoritativeSafetensors { source, .. }) => match source {
            AuthoritativeSafetensorsSource::HuggingFaceRepo { repo_id, .. } => {
                Some(repo_id.clone())
            }
            AuthoritativeSafetensorsSource::LocalDirectory { .. } => None,
        },
        Some(RapidMlxModelSource::MlxDirectory { .. } | RapidMlxModelSource::GgufFile { .. }) => {
            None
        }
        None if !config.model_path.is_empty() => Some(config.model_path.clone()),
        None => None,
    }
}

/// Pure diff between a model's `rapid-mlx info` profile and the active preset's
/// Rapid-MLX diagnostic flags. Kept free of I/O so it is directly unit-testable.
fn build_flag_advisor_findings(
    profile: &info_query::ModelProfile,
    config: &RapidMlxConfig,
) -> Vec<DoctorFinding> {
    let mut findings = Vec::new();

    let tool_format = profile
        .tool_format
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());

    if let Some(tool_format) = tool_format {
        if config.tool_call_parser.is_none() {
            findings.push(DoctorFinding {
                finding_type: DoctorFindingType::Preset,
                severity: DoctorSeverity::Warning,
                message: format!(
                    "Model declares tool format '{tool_format}' but the active preset does not pass --tool-call-parser"
                ),
                section: "Preset Flags".to_string(),
                fix: Some(FixAction::AddToolCallParser),
            });
        }
        if !config.auto_tool_choice {
            findings.push(DoctorFinding {
                finding_type: DoctorFindingType::Preset,
                severity: DoctorSeverity::Warning,
                message: format!(
                    "Model declares tool format '{tool_format}' but the active preset does not pass --enable-auto-tool-choice"
                ),
                section: "Preset Flags".to_string(),
                fix: Some(FixAction::EnableAutoToolChoice),
            });
        }
    }

    let has_reasoning_parser = profile
        .reasoning_parser
        .as_deref()
        .map(str::trim)
        .is_some_and(|value| !value.is_empty());

    // The preset has explicitly opted out of thinking by default
    // (`enable_thinking: Some(false)`) but the launch args don't pass
    // `--no-thinking`, so a reasoning-capable model may still emit thinking
    // tokens despite the preset's stated intent.
    if has_reasoning_parser && config.enable_thinking == Some(false) && !config.no_thinking {
        findings.push(DoctorFinding {
            finding_type: DoctorFindingType::Preset,
            severity: DoctorSeverity::Warning,
            message:
                "Preset disables thinking by default but does not pass --no-thinking; the model may still emit reasoning tokens"
                    .to_string(),
            section: "Preset Flags".to_string(),
            fix: Some(FixAction::AddNoThinking),
        });
    }

    findings
}

async fn run_rapid_mlx_version(binary: &std::path::Path) -> Result<String, std::io::Error> {
    let output = tokio::time::timeout(
        Duration::from_secs(10),
        tokio::process::Command::new(binary)
            .arg("--version")
            .output(),
    )
    .await
    .map_err(|_| std::io::Error::new(std::io::ErrorKind::TimedOut, "version timed out"))??;

    let text = String::from_utf8_lossy(&output.stdout);
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("v") && trimmed.split('.').count() >= 3 {
            return Ok(trimmed.to_string());
        }
    }
    Ok(text.lines().next().unwrap_or("").trim().to_string())
}

async fn run_rapid_mlx_doctor(binary: &std::path::Path) -> Result<String, std::io::Error> {
    let output = tokio::time::timeout(
        Duration::from_secs(30),
        tokio::process::Command::new(binary).arg("doctor").output(),
    )
    .await
    .map_err(|_| std::io::Error::new(std::io::ErrorKind::TimedOut, "doctor timed out"))??;

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let combined = if stderr.trim().is_empty() {
        stdout
    } else {
        format!("{stdout}\n{stderr}")
    };
    Ok(combined)
}

fn parse_doctor_output(output: &str) -> Vec<DoctorFinding> {
    let mut findings = Vec::new();
    let mut current_section = String::from("general");

    for line in output.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        if trimmed.starts_with('◆') {
            // Capture the full multi-word header (e.g. "Optional Packages",
            // "Optional Tools", "Required Packages") — do not truncate to the
            // first word, which collides sections like "Optional Packages"
            // and "Optional Tools" into a single "Optional" bucket.
            current_section = trimmed.trim_start_matches('◆').trim().to_string();
            continue;
        }

        let first_char = trimmed.chars().next();
        let glyph = match first_char {
            Some(g @ ('✓' | '⚠' | '✗' | '×' | '!')) => g,
            _ => continue,
        };

        let severity = DoctorSeverity::from_glyph(glyph);
        let message = trimmed.trim_start_matches(glyph).trim().to_string();

        findings.push(DoctorFinding {
            finding_type: DoctorFindingType::Environment,
            severity,
            message,
            section: current_section.clone(),
            fix: None,
        });
    }

    findings
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_state() -> RuntimeApiState {
        RuntimeApiState {
            manager: Err("unused".into()),
            releases: Arc::new(tokio::sync::Mutex::new(None)),
            jobs: Arc::new(Mutex::new(RuntimeJobs::default())),
            changelog_cache: Arc::new(changelog::ChangelogCacheManager::new()),
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

    /// Real `rapid-mlx doctor` output captured on Apple Silicon (M5 Max,
    /// rapid-mlx 0.10.12) via `rapid-mlx doctor`. Not hand-mirrored — used
    /// verbatim (box-drawing header + `◆` section markers + glyphs) so the
    /// contract test exercises the real layout, per the project's fixture rule.
    const REAL_DOCTOR_OUTPUT: &str = "\
┌─────────────────────────────────────────────────────────┐
│                    🩺 Rapid-MLX Doctor                   │
└─────────────────────────────────────────────────────────┘

◆ System
  ✓ Apple Silicon (Apple M5 Max, 64 GB)
  ✓ macOS 26.5.1 (Darwin 25.5.0)
  ✓ Free disk: 514 GB
  ⚠ HF cache size: 123 GB (consider `rapid-mlx rm` for unused models)

◆ Python
  ✓ Python 3.11.15
  ✓ Install location: virtualenv (/Users/nick/.local/share/uv/python/cpython-3.11.15-macos-aarch64-none/bin/python3.11)

◆ Required Packages
  ✓ mlx 0.32.0
  ✓ mlx-lm 0.31.3
  ✓ transformers 5.12.1
  ✓ fastapi 0.139.2
  ✓ uvicorn 0.51.0
  ✓ rapid-mlx 0.10.12

◆ Optional Packages
  ⚠ mlx-vlm (vision extras) not installed (`pip install 'rapid-mlx[vision]'`)
  ⚠ mlx-audio (audio extras) not installed (`pip install 'rapid-mlx[audio]'`)
  ⚠ mlx-embeddings (embeddings extras) not installed (`pip install 'rapid-mlx[embeddings]'`)
  ⚠ mlx-vlm 0.5.0+ (dflash extras) not installed or too old (current: not installed, need: 0.5.0+)

◆ HuggingFace Cache
  ✓ /Users/nick/.cache/huggingface/hub exists, writable
  ✓ Free space: 514 GB

◆ Network
  ✓ huggingface.co reachable

◆ Shell Integration
  ✓ rapid-mlx in $PATH (/Users/nick/.local/bin/rapid-mlx)
  ⚠ argcomplete not activated — add `eval \"$(register-python-argcomplete rapid-mlx)\"` to your shell rc

◆ Optional Tools
  ✓ codex CLI (/opt/homebrew/bin/codex)

────────────────────────────────────────
Summary: 16 ok, 6 warnings, 0 issues
Run with `--verbose` for details on each check.
";

    #[test]
    fn parse_doctor_output_captures_full_multi_word_section_names() {
        let findings = parse_doctor_output(REAL_DOCTOR_OUTPUT);

        let sections: std::collections::BTreeSet<&str> =
            findings.iter().map(|f| f.section.as_str()).collect();

        // Real multi-word headers must survive intact — the historical bug
        // truncated everything after the first word, colliding "Optional
        // Packages" and "Optional Tools" into a single "Optional" bucket.
        assert!(sections.contains("System"));
        assert!(sections.contains("Python"));
        assert!(sections.contains("Required Packages"));
        assert!(sections.contains("Optional Packages"));
        assert!(sections.contains("HuggingFace Cache"));
        assert!(sections.contains("Network"));
        assert!(sections.contains("Shell Integration"));
        assert!(sections.contains("Optional Tools"));

        // No truncated collision bucket should exist.
        assert!(!sections.contains("Optional"));
        assert!(!sections.contains("Required"));
        assert!(!sections.contains("Shell"));

        // "Optional Packages" and "Optional Tools" findings must stay in their
        // own distinct sections rather than merging.
        let optional_packages_count = findings
            .iter()
            .filter(|f| f.section == "Optional Packages")
            .count();
        let optional_tools_count = findings
            .iter()
            .filter(|f| f.section == "Optional Tools")
            .count();
        assert_eq!(optional_packages_count, 4);
        assert_eq!(optional_tools_count, 1);
    }

    #[test]
    fn parse_doctor_output_maps_glyphs_to_severity_and_rollup_matches_summary_line() {
        let findings = parse_doctor_output(REAL_DOCTOR_OUTPUT);

        let ok_count = findings
            .iter()
            .filter(|f| f.severity == DoctorSeverity::Ok)
            .count();
        let warning_count = findings
            .iter()
            .filter(|f| f.severity == DoctorSeverity::Warning)
            .count();
        let issue_count = findings
            .iter()
            .filter(|f| f.severity == DoctorSeverity::Issue)
            .count();

        // Cross-check against the fixture's own "Summary: N ok, M warnings, K issues"
        // rollup line rather than a hand-picked number.
        let summary_line = REAL_DOCTOR_OUTPUT
            .lines()
            .find(|l| l.trim_start().starts_with("Summary:"))
            .expect("fixture must contain a Summary line");
        assert_eq!(summary_line.trim(), "Summary: 16 ok, 6 warnings, 0 issues");

        assert_eq!(ok_count, 16);
        assert_eq!(warning_count, 6);
        assert_eq!(issue_count, 0);

        // All doctor findings are informational only — never fixable.
        assert!(findings.iter().all(|f| f.fix.is_none()));
        assert!(
            findings
                .iter()
                .all(|f| f.finding_type == DoctorFindingType::Environment)
        );
    }

    fn sample_profile_with_tool_format(tool_format: &str) -> info_query::ModelProfile {
        info_query::ModelProfile {
            tool_format: Some(tool_format.to_string()),
            ..Default::default()
        }
    }

    #[test]
    fn flag_advisor_emits_fix_when_preset_missing_tool_call_flags() {
        let profile = sample_profile_with_tool_format("hermes");
        let config = RapidMlxConfig {
            model_path: String::new(),
            model_source: None,
            served_model_name: None,
            executable_path: None,
            managed_runtime_path: None,
            host: "127.0.0.1".into(),
            port: 8000,
            log_level: "INFO".into(),
            timeout: None,
            max_cache_blocks: None,
            api_key: None,
            enable_thinking: None,
            reasoning_effort: None,
            trust_remote_code_consent: None,
            tool_call_parser: None,
            auto_tool_choice: false,
            no_thinking: false,
            escape_hatch_flags: Vec::new(),
            model_source_view: None,
        };

        let findings = build_flag_advisor_findings(&profile, &config);

        assert_eq!(findings.len(), 2);
        assert!(
            findings
                .iter()
                .any(|f| f.fix == Some(FixAction::AddToolCallParser))
        );
        assert!(
            findings
                .iter()
                .any(|f| f.fix == Some(FixAction::EnableAutoToolChoice))
        );
        assert!(
            findings
                .iter()
                .all(|f| f.finding_type == DoctorFindingType::Preset)
        );
    }

    #[test]
    fn flag_advisor_is_silent_when_preset_already_matches_model_profile() {
        let profile = sample_profile_with_tool_format("hermes");
        let config = RapidMlxConfig {
            model_path: String::new(),
            model_source: None,
            served_model_name: None,
            executable_path: None,
            managed_runtime_path: None,
            host: "127.0.0.1".into(),
            port: 8000,
            log_level: "INFO".into(),
            timeout: None,
            max_cache_blocks: None,
            api_key: None,
            enable_thinking: None,
            reasoning_effort: None,
            trust_remote_code_consent: None,
            tool_call_parser: Some("openai".to_string()),
            auto_tool_choice: true,
            no_thinking: false,
            escape_hatch_flags: Vec::new(),
            model_source_view: None,
        };

        let findings = build_flag_advisor_findings(&profile, &config);

        assert!(findings.is_empty());
    }

    #[test]
    fn flag_advisor_recommends_no_thinking_when_preset_wants_thinking_disabled() {
        let profile = info_query::ModelProfile {
            reasoning_parser: Some("qwen3".to_string()),
            ..Default::default()
        };
        let config = RapidMlxConfig {
            model_path: String::new(),
            model_source: None,
            served_model_name: None,
            executable_path: None,
            managed_runtime_path: None,
            host: "127.0.0.1".into(),
            port: 8000,
            log_level: "INFO".into(),
            timeout: None,
            max_cache_blocks: None,
            api_key: None,
            enable_thinking: Some(false),
            reasoning_effort: None,
            trust_remote_code_consent: None,
            tool_call_parser: None,
            auto_tool_choice: false,
            no_thinking: false,
            escape_hatch_flags: Vec::new(),
            model_source_view: None,
        };

        let findings = build_flag_advisor_findings(&profile, &config);

        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].fix, Some(FixAction::AddNoThinking));
    }
}
