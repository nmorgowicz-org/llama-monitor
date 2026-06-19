use std::time::Duration;

use warp::http::StatusCode;

use super::ApiError;
use crate::state::{AppState, SessionMode};

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
        SessionMode::Spawn { port, .. } => {
            format!("http://127.0.0.1:{port}/v1/chat/completions")
        }
        SessionMode::Attach { endpoint, .. } => {
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
            "Cannot reach active llama-server.",
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

pub(super) async fn prepare_inference_request(
    state: &AppState,
) -> Result<(String, tokio::sync::OwnedSemaphorePermit), warp::Rejection> {
    let url = active_chat_completions_url(state)?;
    let permit = acquire_inference_permit(state).await?;
    wait_for_upstream_capacity(state).await?;
    Ok((url, permit))
}

pub(super) fn build_upstream_client(timeout: Duration) -> Result<reqwest::Client, warp::Rejection> {
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

pub(super) async fn send_upstream_request_with_retry<F>(
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
