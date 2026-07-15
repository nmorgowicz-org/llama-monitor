use crate::inference::InferenceBackend;
use crate::inference::metrics::{HealthState, InferenceMetricsSnapshot};
use anyhow::{Context, Result};
use serde::Deserialize;
use std::time::SystemTime;

#[derive(Deserialize)]
struct StatusResponse {
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
    #[allow(dead_code)]
    cache: Option<serde_json::Value>,
    requests: Option<Vec<serde_json::Value>>,
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
}

impl RapidMlxPoller {
    pub fn new(host: &str, port: u16) -> Self {
        Self {
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(2))
                .build()
                .unwrap_or_default(),
            base_url: format!("http://{}:{}", host, port),
        }
    }

    pub async fn poll(&self) -> Result<InferenceMetricsSnapshot> {
        let status_url = format!("{}/v1/status", self.base_url);
        let status_resp: StatusResponse = self
            .client
            .get(&status_url)
            .send()
            .await?
            .json()
            .await
            .context("Failed to parse /v1/status JSON")?;

        let cache_url = format!("{}/v1/cache/stats", self.base_url);
        let cache_metrics = match self.client.get(&cache_url).send().await {
            Ok(resp) => resp.json::<serde_json::Value>().await.ok(),
            Err(_) => None,
        };

        let gb_to_bytes = 1_073_741_824u64;

        let health = match status_resp.status.as_deref() {
            Some("generating") | Some("idle") => HealthState::Ok,
            Some("not_loaded") => HealthState::NotLoaded,
            _ => HealthState::Unreachable,
        };

        Ok(InferenceMetricsSnapshot {
            sampled_at: SystemTime::now(),
            backend: InferenceBackend::RapidMlx,
            health: Some(health),
            ready: Some(status_resp.status.as_deref() != Some("not_loaded")),
            model: status_resp.model,
            uptime_seconds: status_resp.uptime_s,
            generation_tokens_per_second: status_resp.generation_tps,
            prompt_tokens_per_second: status_resp.prompt_tps,
            running_requests: status_resp.num_running,
            waiting_requests: status_resp.num_waiting,
            completed_requests_total: status_resp.total_requests_processed,
            prompt_tokens_total: status_resp.total_prompt_tokens,
            completion_tokens_total: status_resp.total_completion_tokens,
            steps_executed: status_resp.steps_executed,
            global_cache_hit_rate: None,
            global_cache_entries: None,
            ttft: None,
            speculative_acceptance_rate: None,
            active_memory_bytes: status_resp
                .metal
                .as_ref()
                .and_then(|m| m.active_memory_gb)
                .map(|gb| (gb * gb_to_bytes as f64) as u64),
            peak_memory_bytes: status_resp
                .metal
                .as_ref()
                .and_then(|m| m.peak_memory_gb)
                .map(|gb| (gb * gb_to_bytes as f64) as u64),
            cache_memory_bytes: status_resp
                .metal
                .as_ref()
                .and_then(|m| m.cache_memory_gb)
                .map(|gb| (gb * gb_to_bytes as f64) as u64),
            cache_metrics,
            active_requests: status_resp.requests,
            backend_details: None,
        })
    }
}
