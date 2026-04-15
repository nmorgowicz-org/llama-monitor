use std::time::Duration;

use crate::state::AppState;

use super::metrics::parse_prometheus_metrics;

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

    loop {
        // Get active session ID
        let active_id = { state.active_session_id.lock().unwrap().clone() };

        // Skip polling if no active session
        if active_id.is_empty() {
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
                // No active session found, skip polling
                tokio::time::sleep(Duration::from_secs(poll_interval)).await;
                continue;
            }
        };

        let base = endpoint;

        let server_reachable = match client.get(format!("{base}/health")).send().await {
            Ok(resp) => match resp.text().await {
                Ok(body) => {
                    if let Ok(json) = serde_json::from_str::<serde_json::Value>(&body) {
                        let mut m = state.llama_metrics.lock().unwrap();
                        m.status = json
                            .get("status")
                            .and_then(|v| v.as_str())
                            .unwrap_or("unknown")
                            .to_string();
                        true
                    } else {
                        eprintln!("[poller] /health returned invalid JSON");
                        false
                    }
                }
                Err(_) => {
                    eprintln!("[poller] Failed to read /health response");
                    false
                }
            },
            Err(_) => {
                eprintln!("[poller] Failed to reach /health at {}", base);
                false
            }
        };

        // Update server_running state with hysteresis (only change on state transition)
        {
            let mut running = state.server_running.lock().unwrap();
            // Only update if state actually changed to prevent flickering
            if server_reachable != *running {
                *running = server_reachable;
            }
        }

        if !server_reachable {
            // Don't reset metrics when server is temporarily unavailable
            // Just continue with the last known metrics
            tokio::time::sleep(Duration::from_secs(poll_interval)).await;
            continue;
        }

        // Poll /metrics
        if let Ok(resp) = client.get(format!("{base}/metrics")).send().await
            && let Ok(body) = resp.text().await
        {
            let prom = parse_prometheus_metrics(&body);

            let prompt_tps = if prom.prompt_tokens_per_sec > 0.0 {
                prom.prompt_tokens_per_sec
            } else if prom.prompt_seconds_total > 0.0 {
                prom.prompt_tokens_total / prom.prompt_seconds_total
            } else {
                0.0
            };

            let gen_tps = if prom.predicted_tokens_per_sec > 0.0 {
                prom.predicted_tokens_per_sec
            } else if prom.predicted_seconds_total > 0.0 {
                prom.predicted_tokens_total / prom.predicted_seconds_total
            } else {
                0.0
            };

            let mut m = state.llama_metrics.lock().unwrap();
            m.prompt_tokens_per_sec = prompt_tps;
            m.generation_tokens_per_sec = gen_tps;
            m.prompt_tokens_total = prom.prompt_tokens_total as u64;
            m.predicted_tokens_total = prom.predicted_tokens_total as u64;
            m.kv_cache_tokens = prom.n_tokens_max;
            m.requests_processing = prom.requests_processing;
        }

        // Poll /slots — get per-slot processing state + total context
        if let Ok(resp) = client.get(format!("{base}/slots")).send().await
            && let Ok(body) = resp.text().await
            && let Ok(slots) = serde_json::from_str::<Vec<serde_json::Value>>(&body)
        {
            let mut idle = 0u32;
            let mut processing = 0u32;
            let num_slots = slots.len() as u64;
            let mut per_slot_ctx = 0u64;
            for slot in &slots {
                if slot
                    .get("is_processing")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false)
                {
                    processing += 1;
                } else {
                    idle += 1;
                }
                if let Some(n) = slot.get("n_ctx").and_then(|v| v.as_u64()) {
                    per_slot_ctx = n;
                }
            }
            let mut m = state.llama_metrics.lock().unwrap();
            m.slots_idle = idle;
            m.slots_processing = processing;
            if per_slot_ctx > 0 {
                m.kv_cache_max = per_slot_ctx * num_slots;
            }
        }

        tokio::time::sleep(Duration::from_secs(poll_interval)).await;
    }
}
