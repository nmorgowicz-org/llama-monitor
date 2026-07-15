use crate::inference::InferenceBackend;
use crate::inference::metrics::{HealthState, InferenceMetricsSnapshot};
use anyhow::{Context, Result, anyhow, bail};
use futures_util::StreamExt;
use serde::Deserialize;
use serde::de::DeserializeOwned;
use serde_json::{Map, Value, json};
use std::time::{Duration, SystemTime};

const GIB_BYTES: f64 = 1_073_741_824.0;
const CALL_SPACING: Duration = Duration::from_millis(200);
const STATUS_BODY_LIMIT: usize = 512 * 1024;
const CACHE_BODY_LIMIT: usize = 256 * 1024;
const ACTIVE_REQUEST_LIMIT: usize = 64;

#[derive(Deserialize)]
struct StatusResponse {
    #[serde(default)]
    status: Option<String>,
    model: Option<String>,
    uptime_s: Option<f64>,
    steps_executed: Option<u64>,
    num_running: Option<u64>,
    num_waiting: Option<u64>,
    total_requests_processed: Option<u64>,
    total_prompt_tokens: Option<u64>,
    total_completion_tokens: Option<u64>,
    generation_tps: Option<f64>,
    prompt_tps: Option<f64>,
    metal: Option<MetalMetrics>,
    cache: Option<Value>,
    requests: Option<Vec<Value>>,
    progress: Option<Value>,
}

#[derive(Deserialize)]
struct MetalMetrics {
    active_memory_gb: Option<f64>,
    peak_memory_gb: Option<f64>,
    cache_memory_gb: Option<f64>,
}

pub struct RapidMlxPoller {
    client: reqwest::Client,
    base_url: String,
    api_key: Option<String>,
}

impl RapidMlxPoller {
    pub fn new(host: &str, port: u16, api_key: Option<&str>) -> Self {
        let host = match host {
            "0.0.0.0" | "::" | "[::]" => "127.0.0.1",
            "::1" => "[::1]",
            host => host,
        };
        Self::from_base_url(format!("http://{host}:{port}"), api_key)
    }

    pub fn from_base_url(base_url: String, api_key: Option<&str>) -> Self {
        Self {
            client: reqwest::Client::builder()
                .pool_max_idle_per_host(0)
                .build()
                .unwrap_or_default(),
            base_url: base_url.trim_end_matches('/').to_string(),
            api_key: api_key.map(str::to_string),
        }
    }

    pub async fn poll(&self) -> Result<InferenceMetricsSnapshot> {
        let health_ok = self
            .authenticated_get(&format!("{}/health", self.base_url))
            .timeout(Duration::from_secs(2))
            .send()
            .await
            .is_ok_and(|response| response.status().is_success());

        tokio::time::sleep(CALL_SPACING).await;
        let status_response = self
            .authenticated_get(&format!("{}/v1/status", self.base_url))
            .timeout(Duration::from_secs(3))
            .send()
            .await?
            .error_for_status()
            .context("Rapid-MLX /v1/status returned an error status")?;
        let mut status: StatusResponse =
            parse_json_limited(status_response, STATUS_BODY_LIMIT, "Rapid-MLX /v1/status").await?;
        status.model = status
            .model
            .filter(|model| !model.is_empty() && model.len() <= 512);
        status.requests = status.requests.map(sanitize_requests);
        validate_status(&status)?;

        tokio::time::sleep(CALL_SPACING).await;
        let endpoint_cache = match self
            .authenticated_get(&format!("{}/v1/cache/stats", self.base_url))
            .timeout(Duration::from_secs(2))
            .send()
            .await
        {
            Ok(response) if response.status().is_success() => {
                parse_json_limited::<Value>(response, CACHE_BODY_LIMIT, "Rapid-MLX /v1/cache/stats")
                    .await
                    .ok()
                    .and_then(recognized_endpoint_cache)
            }
            Ok(_) | Err(_) => None,
        };

        let status_cache = status.cache.as_ref().and_then(normalized_status_cache);
        let global_cache_hit_rate = status_cache
            .as_ref()
            .and_then(|cache| cache.get("hit_rate"))
            .and_then(Value::as_f64);
        let global_cache_entries = status_cache
            .as_ref()
            .and_then(|cache| cache.get("entry_count"))
            .and_then(Value::as_u64);
        let cache_metrics = merge_cache_metrics(status_cache, endpoint_cache);

        let health = if !health_ok {
            HealthState::Degraded
        } else {
            match status.status.as_deref() {
                Some("generating" | "idle") => HealthState::Ok,
                Some("not_loaded") => HealthState::NotLoaded,
                _ => HealthState::Degraded,
            }
        };
        let ready = match status.status.as_deref() {
            Some("not_loaded") => Some(false),
            Some("generating" | "idle") => Some(true),
            _ => None,
        };
        let metal = status.metal.as_ref();

        Ok(InferenceMetricsSnapshot {
            sampled_at: SystemTime::now(),
            backend: InferenceBackend::RapidMlx,
            health: Some(health),
            ready,
            model: status.model,
            uptime_seconds: status.uptime_s,
            generation_tokens_per_second: status.generation_tps,
            prompt_tokens_per_second: status.prompt_tps,
            running_requests: status.num_running,
            waiting_requests: status.num_waiting,
            completed_requests_total: status.total_requests_processed,
            prompt_tokens_total: status.total_prompt_tokens,
            completion_tokens_total: status.total_completion_tokens,
            steps_executed: status.steps_executed,
            global_cache_hit_rate,
            global_cache_entries,
            ttft: None,
            speculative_acceptance_rate: None,
            active_memory_bytes: metal
                .and_then(|m| m.active_memory_gb)
                .map(gib_to_bytes)
                .transpose()?,
            peak_memory_bytes: metal
                .and_then(|m| m.peak_memory_gb)
                .map(gib_to_bytes)
                .transpose()?,
            cache_memory_bytes: metal
                .and_then(|m| m.cache_memory_gb)
                .map(gib_to_bytes)
                .transpose()?,
            cache_metrics,
            active_requests: status.requests,
            backend_details: Some(json!({
                "runtime_status": status.status,
                "progress": status.progress.and_then(recognized_progress),
            })),
        })
    }

    fn authenticated_get(&self, url: &str) -> reqwest::RequestBuilder {
        match &self.api_key {
            Some(key) => self.client.get(url).bearer_auth(key),
            None => self.client.get(url),
        }
    }
}

async fn parse_json_limited<T: DeserializeOwned>(
    response: reqwest::Response,
    limit: usize,
    endpoint: &str,
) -> Result<T> {
    let mut body = Vec::new();
    let mut stream = response.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.with_context(|| format!("Failed reading {endpoint} response"))?;
        if body.len().saturating_add(chunk.len()) > limit {
            bail!("{endpoint} response exceeded the {limit}-byte telemetry limit");
        }
        body.extend_from_slice(&chunk);
    }
    serde_json::from_slice(&body)
        .with_context(|| format!("Failed to parse required {endpoint} telemetry"))
}

fn validate_status(status: &StatusResponse) -> Result<()> {
    if status
        .status
        .as_ref()
        .is_some_and(|value| value.trim().is_empty() || value.len() > 64)
    {
        bail!("Rapid-MLX /v1/status contains an invalid status value");
    }
    for (name, value) in [
        ("uptime_s", status.uptime_s),
        ("generation_tps", status.generation_tps),
        ("prompt_tps", status.prompt_tps),
        (
            "metal.active_memory_gb",
            status.metal.as_ref().and_then(|m| m.active_memory_gb),
        ),
        (
            "metal.peak_memory_gb",
            status.metal.as_ref().and_then(|m| m.peak_memory_gb),
        ),
        (
            "metal.cache_memory_gb",
            status.metal.as_ref().and_then(|m| m.cache_memory_gb),
        ),
    ] {
        if value.is_some_and(|number| !number.is_finite() || number < 0.0) {
            bail!("Rapid-MLX /v1/status contains invalid numeric telemetry in {name}");
        }
    }
    Ok(())
}

fn gib_to_bytes(gib: f64) -> Result<u64> {
    let bytes = gib * GIB_BYTES;
    if !bytes.is_finite() || bytes < 0.0 || bytes > u64::MAX as f64 {
        return Err(anyhow!("Rapid-MLX Metal memory telemetry overflows bytes"));
    }
    Ok(bytes as u64)
}

fn normalized_status_cache(value: &Value) -> Option<Value> {
    let object = value.as_object()?;
    if object.get("enabled") == Some(&Value::Bool(false)) {
        return None;
    }
    let mut normalized = Map::new();
    for key in ["hits", "misses", "current_memory_bytes", "entry_count"] {
        if let Some(number) = object.get(key).and_then(Value::as_u64) {
            normalized.insert(key.to_string(), Value::from(number));
        }
    }
    if let Some(rate) = object
        .get("hit_rate")
        .and_then(Value::as_f64)
        .filter(|rate| rate.is_finite() && (0.0..=1.0).contains(rate))
    {
        normalized.insert("hit_rate".to_string(), Value::from(rate));
    }
    (!normalized.is_empty()).then_some(Value::Object(normalized))
}

fn recognized_endpoint_cache(value: Value) -> Option<Value> {
    let object = value.as_object()?;
    if object.contains_key("message") && object.contains_key("model_type") {
        return None;
    }
    let recognized = [
        "multimodal_kv_cache",
        "pixel_values_cache",
        "pil_image_cache",
    ];
    let kinds: Vec<Value> = recognized
        .iter()
        .filter(|key| object.get(**key).is_some_and(Value::is_object))
        .map(|key| Value::from(*key))
        .collect();
    (!kinds.is_empty()).then_some(json!({ "multimodal_cache_kinds": kinds }))
}

fn merge_cache_metrics(status: Option<Value>, endpoint: Option<Value>) -> Option<Value> {
    let mut merged = Map::new();
    for value in [status, endpoint].into_iter().flatten() {
        if let Some(object) = value.as_object() {
            merged.extend(object.clone());
        }
    }
    (!merged.is_empty()).then_some(Value::Object(merged))
}

fn sanitize_requests(requests: Vec<Value>) -> Vec<Value> {
    requests
        .into_iter()
        .take(ACTIVE_REQUEST_LIMIT)
        .filter_map(|request| {
            let object = request.as_object()?;
            let mut sanitized = Map::new();
            for (key, max_len) in [("id", 256), ("request_id", 256), ("status", 64)] {
                if let Some(value) = object
                    .get(key)
                    .and_then(Value::as_str)
                    .filter(|value| !value.is_empty() && value.len() <= max_len)
                {
                    sanitized.insert(key.to_string(), Value::from(value));
                }
            }
            (!sanitized.is_empty()).then_some(Value::Object(sanitized))
        })
        .collect()
}

fn recognized_progress(value: Value) -> Option<Value> {
    if value
        .as_f64()
        .is_some_and(|number| (0.0..=1.0).contains(&number))
    {
        return Some(value);
    }
    let object = value.as_object()?;
    let current = object.get("current").and_then(Value::as_f64)?;
    let total = object.get("total").and_then(Value::as_f64)?;
    (current.is_finite() && total.is_finite() && current >= 0.0 && total > 0.0 && current <= total)
        .then(|| json!({ "current": current, "total": total }))
}
