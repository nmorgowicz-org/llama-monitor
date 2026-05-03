use std::time::Duration;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::state::AppState;

use super::metrics::{PrometheusValues, parse_prometheus_metrics, parse_slot_metrics};

#[derive(Debug, Clone, Default)]
struct CounterSnapshot {
    prompt_tokens_total: f64,
    prompt_seconds_total: f64,
    predicted_tokens_total: f64,
    predicted_seconds_total: f64,
}

// NOTE: Blocked state detection is disabled because we cannot reliably distinguish
// "blocked on tool call" from "processing big context" using only n_decoded stagnation.
// Both show is_processing=true, output_active=false, and stagnant n_decoded.
// Re-enable when llama-server exposes a tool-calling state in /slots.

impl CounterSnapshot {
    fn from_prometheus(values: &PrometheusValues) -> Self {
        Self {
            prompt_tokens_total: values.prompt_tokens_total,
            prompt_seconds_total: values.prompt_seconds_total,
            predicted_tokens_total: values.predicted_tokens_total,
            predicted_seconds_total: values.predicted_seconds_total,
        }
    }
}

fn counter_rate(
    current_tokens: f64,
    previous_tokens: f64,
    current_seconds: f64,
    previous_seconds: f64,
) -> f64 {
    let token_delta = current_tokens - previous_tokens;
    let second_delta = current_seconds - previous_seconds;

    if token_delta > 0.0 && second_delta > 0.0 {
        token_delta / second_delta
    } else {
        0.0
    }
}

fn unix_time_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

pub async fn llama_metrics_poller(state: AppState, poll_interval: u64) {
    let client = match reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .pool_max_idle_per_host(0)
        .pool_idle_timeout(Duration::from_secs(0))
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            eprintln!("[error] Failed to build HTTP client: {:?}", e);
            return;
        }
    };

    let mut enabled = false;
    let mut previous_counters: Option<CounterSnapshot> = None;
    let mut previous_counter_session: Option<String> = None;

    loop {
        if !enabled {
            state.llama_poll_notify.notified().await;
            enabled = true;
        }

        let active_id = { state.active_session_id.lock().unwrap().clone() };
        if active_id.is_empty() {
            enabled = false;
            previous_counters = None;
            previous_counter_session = None;
            tokio::time::sleep(Duration::from_secs(poll_interval)).await;
            continue;
        }

        // Determine endpoint from active session
        let endpoint = {
            let session = {
                let sessions = state.sessions.lock().unwrap();
                sessions.iter().find(|s| s.id == active_id).cloned()
            };

            if let Some(sess) = session {
                match sess.mode {
                    crate::state::SessionMode::Spawn { port } => {
                        format!("http://127.0.0.1:{}", port)
                    }
                    crate::state::SessionMode::Attach { endpoint } => endpoint,
                }
            } else {
                enabled = false;
                previous_counters = None;
                previous_counter_session = None;
                tokio::time::sleep(Duration::from_secs(poll_interval)).await;
                continue;
            }
        };

        let base = endpoint;

        // First check if server is up at all
        let server_up = client.get(&base).send().await.is_ok();

        let server_reachable = if server_up {
            // Try /health for detailed status
            match client.get(format!("{base}/health")).send().await {
                Ok(resp) => match resp.text().await {
                    Ok(body) => {
                        if let Ok(json) = serde_json::from_str::<serde_json::Value>(&body) {
                            let mut m = state.llama_metrics.lock().unwrap();
                            m.status = json
                                .get("status")
                                .and_then(|v| v.as_str())
                                .unwrap_or("running")
                                .to_string();
                        }
                        true
                    }
                    Err(_) => true, // Server up but /health returned non-JSON
                },
                Err(_) => true, // Server up but /health not available (metrics disabled)
            }
        } else {
            false
        };

        // Update server_running state with hysteresis (only change on state transition)
        {
            let mut running = state.server_running.lock().unwrap();
            if server_reachable != *running {
                *running = server_reachable;
            }
        }

        if !server_reachable {
            // Don't reset metrics when server is temporarily unavailable
            // Just continue with the last known metrics
            previous_counters = None;
            previous_counter_session = None;
            tokio::time::sleep(Duration::from_secs(poll_interval)).await;
            continue;
        }

        if let Ok(resp) = client.get(format!("{base}/metrics")).send().await
            && let Ok(body) = resp.text().await
        {
            let prom = parse_prometheus_metrics(&body);
            let current_counters = CounterSnapshot::from_prometheus(&prom);

            let (prompt_tps, gen_tps) = if previous_counter_session.as_deref()
                == Some(active_id.as_str())
                && let Some(previous) = &previous_counters
            {
                (
                    counter_rate(
                        current_counters.prompt_tokens_total,
                        previous.prompt_tokens_total,
                        current_counters.prompt_seconds_total,
                        previous.prompt_seconds_total,
                    ),
                    counter_rate(
                        current_counters.predicted_tokens_total,
                        previous.predicted_tokens_total,
                        current_counters.predicted_seconds_total,
                        previous.predicted_seconds_total,
                    ),
                )
            } else {
                (0.0, 0.0)
            };

            previous_counters = Some(current_counters);
            previous_counter_session = Some(active_id.clone());

            let mut m = state.llama_metrics.lock().unwrap();
            m.prompt_tokens_per_sec = prompt_tps;
            m.generation_tokens_per_sec = gen_tps;
            m.throughput_source = "interval_delta".to_string();
            m.prompt_throughput_active = prompt_tps > 0.0;
            m.generation_throughput_active = gen_tps > 0.0;
            let now_ms = unix_time_ms();
            if prompt_tps > 0.0 {
                m.last_prompt_tokens_per_sec = prompt_tps;
                m.last_prompt_throughput_unix_ms = now_ms;
            }
            if gen_tps > 0.0 {
                m.last_generation_tokens_per_sec = gen_tps;
                m.last_generation_throughput_unix_ms = now_ms;
            }
            m.prompt_tokens_total = prom.prompt_tokens_total as u64;
            m.predicted_tokens_total = prom.predicted_tokens_total as u64;
            m.generation_tokens_total = prom.predicted_tokens_total as u64;
            m.kv_cache_high_water = prom.n_tokens_max;
            m.context_high_water_tokens = prom.n_tokens_max;
            m.requests_processing = prom.requests_processing;
        }

        // Poll /slots — get per-slot processing state + total context
        if let Ok(resp) = client.get(format!("{base}/slots")).send().await
            && let Ok(body) = resp.text().await
            && let Some(slots) = parse_slot_metrics(&body)
        {
            let mut m = state.llama_metrics.lock().unwrap();
            m.slots_idle = slots.slots_idle;
            m.slots_processing = slots.slots_processing;
            m.kv_cache_max = slots.kv_cache_max;
            m.kv_cache_tokens = slots.kv_cache_tokens;
            m.kv_cache_tokens_available = slots.kv_cache_tokens_available;
            m.kv_cache_tokens_source = slots.kv_cache_tokens_source;
            m.context_capacity_tokens = slots.kv_cache_max;
            m.context_live_tokens = slots.kv_cache_tokens;
            m.context_live_tokens_available = slots.kv_cache_tokens_available;
            m.context_live_tokens_source = m.kv_cache_tokens_source.clone();
            m.active_task_id = slots.active_task_id;
            if slots.last_task_id.is_some() {
                m.last_task_id = slots.last_task_id;
            }
            m.slot_generation_tokens = slots.slot_generation_tokens;
            m.slot_generation_remaining = slots.slot_generation_remaining;
            m.slot_generation_limit = slots.slot_generation_limit;
            m.slot_generation_active = slots.slot_generation_active;
            m.slot_generation_available = slots.slot_generation_available;
            m.slots = slots.slots;
            if !m.kv_cache_tokens_available && m.requests_processing == 0 {
                m.kv_cache_tokens = 0;
                m.context_live_tokens = 0;
            }
        }

        // Poll /v1/models — get model name and metadata
        if let Ok(resp) = client.get(format!("{base}/v1/models")).send().await
            && let Ok(body) = resp.text().await
            && let Ok(json) = serde_json::from_str::<serde_json::Value>(&body)
            && let Some(model_name) = json["data"][0]["id"].as_str()
        {
            let mut m = state.llama_metrics.lock().unwrap();
            m.model_name = model_name.to_string();
            if let Some(n_params) = json["data"][0]["meta"]["n_params"].as_u64() {
                m.model_params = Some(n_params);
            }
            if let Some(n_ctx_train) = json["data"][0]["meta"]["n_ctx_train"].as_u64() {
                m.model_ctx_train = Some(n_ctx_train);
            }
        }

        tokio::time::sleep(Duration::from_secs(poll_interval)).await;
    }
}
