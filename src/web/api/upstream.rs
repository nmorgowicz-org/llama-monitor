use std::time::Duration;

use warp::http::StatusCode;

use super::ApiError;
use crate::inference::InferenceBackend;
use crate::inference::backend::BackendAdapter;
use crate::state::{AppState, SessionMode};

const MONITOR_INFERENCE_QUEUE_TIMEOUT: Duration = Duration::from_secs(330);
const UPSTREAM_BUSY_WAIT_TIMEOUT: Duration = Duration::from_secs(300);
const UPSTREAM_BUSY_POLL_INTERVAL: Duration = Duration::from_millis(500);
const UPSTREAM_SEND_RETRIES: usize = 3;
const UPSTREAM_SEND_RETRY_BACKOFF_MS: u64 = 250;
const MAX_UPSTREAM_ERROR_BYTES: usize = 64 * 1024;

pub(crate) struct PreparedInferenceRequest {
    pub url: String,
    pub permit: tokio::sync::OwnedSemaphorePermit,
    pub backend: InferenceBackend,
    model_identity: Option<String>,
    api_key: Option<String>,
    adapter: Option<BackendAdapter>,
}

struct ActiveInferenceTarget {
    url: String,
    backend: InferenceBackend,
    model_identity: Option<String>,
    api_key: Option<String>,
    local_spawn: bool,
}

pub(crate) fn adapter_matches_backend(backend: InferenceBackend, adapter: &BackendAdapter) -> bool {
    matches!(
        (backend, adapter),
        (InferenceBackend::LlamaCpp, BackendAdapter::LlamaCpp(_))
            | (InferenceBackend::RapidMlx, BackendAdapter::RapidMlx(_))
    )
}

pub(crate) fn local_connect_host(bind_host: Option<&str>) -> &str {
    match bind_host {
        None | Some("") | Some("0.0.0.0") | Some("::") | Some("[::]") => "127.0.0.1",
        Some("::1") => "[::1]",
        Some(host) => host,
    }
}

impl PreparedInferenceRequest {
    pub fn authenticate(&self, request: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        match &self.api_key {
            Some(key) if !key.is_empty() => request.bearer_auth(key),
            _ => request,
        }
    }

    pub fn map_chat_body(&self, body: &[u8]) -> Result<Vec<u8>, warp::Rejection> {
        let mapped = match self.backend {
            InferenceBackend::LlamaCpp => Ok(body.to_vec()),
            InferenceBackend::RapidMlx => match &self.adapter {
                Some(BackendAdapter::RapidMlx(adapter)) => adapter.map_chat_request(body),
                None => crate::inference::rapid_mlx::map_provisional_chat_request(body),
                Some(BackendAdapter::LlamaCpp(_)) => {
                    crate::inference::rapid_mlx::map_provisional_chat_request(body)
                }
            },
        };
        let mut mapped = mapped.map_err(|error| {
            warp::reject::custom(ApiError::new(StatusCode::BAD_REQUEST, error.to_string()))
        })?;
        if self.backend == InferenceBackend::RapidMlx
            && let Some(model) = self
                .model_identity
                .as_ref()
                .filter(|model| !model.is_empty())
        {
            let mut value: serde_json::Value = serde_json::from_slice(&mapped)
                .map_err(|error| warp::reject::custom(ApiError::internal(error.to_string())))?;
            if let Some(object) = value.as_object_mut()
                && !object.contains_key("model")
            {
                object.insert(
                    "model".to_string(),
                    serde_json::Value::String(model.clone()),
                );
                mapped = serde_json::to_vec(&value)
                    .map_err(|error| warp::reject::custom(ApiError::internal(error.to_string())))?;
            }
        }
        Ok(mapped)
    }
}

fn active_inference_target(state: &AppState) -> Result<ActiveInferenceTarget, warp::Rejection> {
    let session = state
        .get_active_session()
        .ok_or(warp::reject::not_found())?;
    let (url, api_key, local_spawn) = match &session.mode {
        SessionMode::Spawn {
            port,
            bind_host,
            api_key,
        } => {
            let host = local_connect_host(bind_host.as_deref());
            (
                format!("http://{host}:{port}/v1/chat/completions"),
                api_key.clone(),
                true,
            )
        }
        SessionMode::Attach { endpoint, api_key } => (
            format!("{endpoint}/v1/chat/completions"),
            api_key.clone(),
            false,
        ),
    };
    Ok(ActiveInferenceTarget {
        url,
        backend: session.backend,
        model_identity: session.model_identity,
        api_key,
        local_spawn,
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
            "Cannot reach the active inference runtime.",
        )));
    }

    let metrics = state.llama_metrics.lock().map_err(|e| {
        warp::reject::custom(ApiError::internal(format!(
            "Failed to read llama metrics: {e}"
        )))
    })?;
    let total_slots = metrics.slots_idle.saturating_add(metrics.slots_processing);
    if total_slots == 0 {
        return Ok(true);
    }
    Ok(metrics.slots_processing < total_slots)
}

async fn wait_for_upstream_capacity(state: &AppState) -> Result<(), warp::Rejection> {
    let deadline = tokio::time::Instant::now() + UPSTREAM_BUSY_WAIT_TIMEOUT;

    loop {
        if upstream_has_capacity(state)? {
            return Ok(());
        }

        if tokio::time::Instant::now() >= deadline {
            return Err(warp::reject::custom(ApiError::busy(
                "The active inference runtime has been busy for too long. Wait for it to finish and retry.",
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

pub(crate) async fn prepare_inference_request(
    state: &AppState,
) -> Result<PreparedInferenceRequest, warp::Rejection> {
    let permit = acquire_inference_permit(state).await?;
    wait_for_upstream_capacity(state).await?;
    // Snapshot routing only after queueing. A request that waited behind another
    // generation must not retain the session/backend that was active before it
    // acquired the inference permit.
    let target = active_inference_target(state)?;
    let adapter = state
        .backend
        .lock()
        .map_err(|error| {
            warp::reject::custom(ApiError::internal(format!(
                "Failed to read active backend: {error}"
            )))
        })?
        .clone()
        .filter(|adapter| target.local_spawn && adapter_matches_backend(target.backend, adapter));
    Ok(PreparedInferenceRequest {
        url: target.url,
        permit,
        backend: target.backend,
        model_identity: target.model_identity,
        api_key: target.api_key,
        adapter,
    })
}

pub(crate) fn build_upstream_client(timeout: Duration) -> Result<reqwest::Client, warp::Rejection> {
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

pub(crate) async fn send_upstream_request_with_retry<F>(
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
                let err_body = read_error_body_bounded(resp).await;
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

async fn read_error_body_bounded(response: reqwest::Response) -> String {
    use futures_util::StreamExt;

    let mut stream = response.bytes_stream();
    let mut body = Vec::new();
    while let Some(chunk) = stream.next().await {
        let Ok(chunk) = chunk else {
            break;
        };
        let remaining = MAX_UPSTREAM_ERROR_BYTES.saturating_sub(body.len());
        if chunk.len() > remaining {
            body.extend_from_slice(&chunk[..remaining]);
            break;
        }
        body.extend_from_slice(&chunk);
        if body.len() == MAX_UPSTREAM_ERROR_BYTES {
            break;
        }
    }
    String::from_utf8_lossy(&body).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inference::rapid_mlx::RapidMlxAdapter;
    use crate::inference::rapid_mlx::runtime::{RuntimeMetadata, RuntimeSource};
    use crate::state::Session;

    async fn prepared(
        backend: InferenceBackend,
        model_identity: Option<&str>,
        api_key: Option<&str>,
    ) -> PreparedInferenceRequest {
        let gate = std::sync::Arc::new(tokio::sync::Semaphore::new(1));
        PreparedInferenceRequest {
            url: "http://127.0.0.1:1/v1/chat/completions".into(),
            permit: gate.acquire_owned().await.unwrap(),
            backend,
            model_identity: model_identity.map(str::to_string),
            api_key: api_key.map(str::to_string),
            adapter: None,
        }
    }

    #[tokio::test]
    async fn llama_mapping_remains_byte_exact() {
        let prepared = prepared(InferenceBackend::LlamaCpp, Some("ignored"), None).await;
        let body = br#"{ "messages": [], "seed": 7, "cache_prompt": true }"#;
        assert_eq!(prepared.map_chat_body(body).unwrap(), body);
    }

    #[tokio::test]
    async fn rapid_mapping_injects_persisted_model_identity_only_when_absent() {
        let prepared = prepared(InferenceBackend::RapidMlx, Some("served-model"), None).await;
        let mapped = prepared
            .map_chat_body(br#"{"messages":[],"stream":true}"#)
            .unwrap();
        let value: serde_json::Value = serde_json::from_slice(&mapped).unwrap();
        assert_eq!(value["model"], "served-model");

        let mapped = prepared
            .map_chat_body(br#"{"messages":[],"model":"explicit"}"#)
            .unwrap();
        let value: serde_json::Value = serde_json::from_slice(&mapped).unwrap();
        assert_eq!(value["model"], "explicit");
    }

    #[tokio::test]
    async fn upstream_auth_uses_transient_bearer_header() {
        let with_key = prepared(InferenceBackend::RapidMlx, None, Some("transient-secret")).await;
        let request = with_key
            .authenticate(reqwest::Client::new().get("http://127.0.0.1:1"))
            .build()
            .unwrap();
        assert_eq!(
            request.headers()[reqwest::header::AUTHORIZATION],
            "Bearer transient-secret"
        );

        let no_key = prepared(InferenceBackend::RapidMlx, None, None).await;
        let request = no_key
            .authenticate(reqwest::Client::new().get("http://127.0.0.1:1"))
            .build()
            .unwrap();
        assert!(
            !request
                .headers()
                .contains_key(reqwest::header::AUTHORIZATION)
        );
    }

    #[tokio::test]
    async fn upstream_non_success_status_is_propagated() {
        let mut server = mockito::Server::new_async().await;
        let _failure = server
            .mock("POST", "/v1/chat/completions")
            .with_status(401)
            .with_body("invalid runtime key")
            .create_async()
            .await;
        let client = reqwest::Client::new();
        let url = format!("{}/v1/chat/completions", server.url());
        let rejection = send_upstream_request_with_retry(|| client.post(&url))
            .await
            .unwrap_err();
        let error = rejection.find::<ApiError>().unwrap();
        assert_eq!(error.status, StatusCode::UNAUTHORIZED);
        assert!(error.message.contains("invalid runtime key"));
    }

    #[tokio::test]
    async fn upstream_error_body_is_bounded() {
        let mut server = mockito::Server::new_async().await;
        let _failure = server
            .mock("POST", "/v1/chat/completions")
            .with_status(500)
            .with_body("x".repeat(MAX_UPSTREAM_ERROR_BYTES + 4096))
            .create_async()
            .await;
        let client = reqwest::Client::new();
        let url = format!("{}/v1/chat/completions", server.url());
        let rejection = send_upstream_request_with_retry(|| client.post(&url))
            .await
            .unwrap_err();
        let error = rejection.find::<ApiError>().unwrap();
        assert_eq!(error.status, StatusCode::INTERNAL_SERVER_ERROR);
        assert!(error.message.len() <= MAX_UPSTREAM_ERROR_BYTES + 64);
    }

    #[test]
    fn spawned_ipv6_loopback_routes_to_a_valid_local_url() {
        let state = AppState::default();
        let session = Session::new_spawn_with_backend(
            "rapid-ipv6".into(),
            "Rapid IPv6".into(),
            8123,
            String::new(),
            Some("::1".into()),
            None,
            InferenceBackend::RapidMlx,
            Some("model".into()),
        );
        assert!(state.add_session(session));
        state.set_active_session("rapid-ipv6");
        let target = active_inference_target(&state).unwrap();
        assert_eq!(target.url, "http://[::1]:8123/v1/chat/completions");
        assert_eq!(target.backend, InferenceBackend::RapidMlx);
        assert!(target.local_spawn);
    }

    #[tokio::test]
    async fn attached_session_never_inherits_a_stale_spawn_adapter() {
        let state = AppState::default();
        let mut session = Session::new_attach(
            "attached-rapid".into(),
            "Attached Rapid".into(),
            "http://127.0.0.1:9000".into(),
            None,
        );
        session.backend = InferenceBackend::RapidMlx;
        session.model_identity = Some("attached-model".into());
        assert!(state.add_session(session));
        state.set_active_session("attached-rapid");
        *state.server_running.lock().unwrap() = true;
        *state.backend.lock().unwrap() = Some(BackendAdapter::RapidMlx(std::sync::Arc::new(
            RapidMlxAdapter::new(
                RuntimeMetadata {
                    executable_path: "rapid-mlx".into(),
                    source: RuntimeSource::Managed,
                    version: "0.10.9".into(),
                },
                "stale-model".into(),
            ),
        )));

        let prepared = prepare_inference_request(&state).await.unwrap();
        assert!(prepared.adapter.is_none());
        let body = prepared
            .map_chat_body(br#"{"messages":[],"stream":true,"tools":[]}"#)
            .unwrap();
        let value: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert!(value.get("tools").is_none());
    }
}
