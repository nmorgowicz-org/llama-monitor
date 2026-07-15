use std::time::Duration;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::state::AppState;

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

    loop {
        if !enabled {
            state.llama_poll_notify.notified().await;
            enabled = true;
        }

        let active_id = { state.active_session_id.lock().unwrap().clone() };
        if active_id.is_empty() {
            enabled = false;
            tokio::time::sleep(Duration::from_secs(poll_interval)).await;
            continue;
        }

        // Determine endpoint and optional API key from active session
        let (base, api_key) = {
            let session = {
                let sessions = state.sessions.lock().unwrap();
                sessions.iter().find(|s| s.id == active_id).cloned()
            };

            if let Some(sess) = session {
                match sess.mode {
                    crate::state::SessionMode::Spawn { port, api_key, .. } => {
                        (format!("http://127.0.0.1:{}", port), api_key)
                    }
                    crate::state::SessionMode::Attach { endpoint, api_key } => (endpoint, api_key),
                }
            } else {
                enabled = false;
                tokio::time::sleep(Duration::from_secs(poll_interval)).await;
                continue;
            }
        };

        // Helper to add auth header if API key is set
        fn with_auth(
            mut req: reqwest::RequestBuilder,
            api_key: &Option<String>,
        ) -> reqwest::RequestBuilder {
            if let Some(key) = api_key {
                req = req.header("Authorization", format!("Bearer {}", key));
            }
            req
        }

        // First check if server is up at all
        let server_up = with_auth(client.get(&base), &api_key).send().await.is_ok();

        let server_reachable = if server_up {
            // Try /health for detailed status
            match with_auth(client.get(format!("{base}/health")), &api_key)
                .send()
                .await
            {
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
            tokio::time::sleep(Duration::from_secs(poll_interval)).await;
            continue;
        }

        // Use the backend adapter to poll normalized metrics
        let backend = state.backend.lock().unwrap().clone();
        if let Some(backend) = backend {
            let port = if let Some(sess) = {
                let sessions = state.sessions.lock().unwrap();
                sessions.iter().find(|s| s.id == active_id).cloned()
            } {
                match sess.mode {
                    crate::state::SessionMode::Spawn { port, .. } => port,
                    crate::state::SessionMode::Attach { endpoint, .. } => endpoint
                        .split(':')
                        .next_back()
                        .and_then(|p| p.parse().ok())
                        .unwrap_or(0),
                }
            } else {
                0
            };

            if port != 0
                && let Ok(snapshot) = backend.poll_metrics(port, &active_id).await
            {
                let now_ms = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis() as u64;

                let mut m = state.llama_metrics.lock().unwrap();

                if let Some(prompt_tps) = snapshot.prompt_tokens_per_second {
                    m.prompt_tokens_per_sec = prompt_tps;
                    m.prompt_throughput_active = prompt_tps > 0.0;
                    if prompt_tps > 0.0 {
                        m.last_prompt_tokens_per_sec = prompt_tps;
                        m.last_prompt_throughput_unix_ms = now_ms;
                    }
                }

                if let Some(gen_tps) = snapshot.generation_tokens_per_second {
                    m.generation_tokens_per_sec = gen_tps;
                    m.generation_throughput_active = gen_tps > 0.0;
                    if gen_tps > 0.0 {
                        m.last_generation_tokens_per_sec = gen_tps;
                        m.last_generation_throughput_unix_ms = now_ms;
                    }
                }

                m.throughput_source = "backend_poll".to_string();

                if let Some(prompt_total) = snapshot.prompt_tokens_total {
                    m.prompt_tokens_total = prompt_total;
                }
                if let Some(completion_total) = snapshot.completion_tokens_total {
                    m.predicted_tokens_total = completion_total;
                    m.generation_tokens_total = completion_total;
                }
                if let Some(running) = snapshot.running_requests {
                    m.requests_processing = running as u32;
                }
                if let Some(details) = snapshot.backend_details {
                    if let Some(idle) = details.get("slots_idle").and_then(|v| v.as_u64()) {
                        m.slots_idle = idle as u32;
                    }
                    if let Some(processing) =
                        details.get("slots_processing").and_then(|v| v.as_u64())
                    {
                        m.slots_processing = processing as u32;
                    }
                    if let Some(max) = details.get("kv_cache_max").and_then(|v| v.as_u64()) {
                        m.kv_cache_max = max;
                        m.context_capacity_tokens = max;
                    }
                    if let Some(tokens) = details.get("kv_cache_tokens").and_then(|v| v.as_u64()) {
                        m.kv_cache_tokens = tokens;
                        m.context_live_tokens = tokens;
                    }
                    if let Some(avail) = details
                        .get("kv_cache_tokens_available")
                        .and_then(|v| v.as_bool())
                    {
                        m.kv_cache_tokens_available = avail;
                        m.context_live_tokens_available = avail;
                    }
                    if let Some(source) = details
                        .get("kv_cache_tokens_source")
                        .and_then(|v| v.as_str())
                    {
                        m.kv_cache_tokens_source = source.to_string();
                        m.context_live_tokens_source = source.to_string();
                    }
                    if let Some(active) = details.get("active_task_id").and_then(|v| v.as_u64()) {
                        m.active_task_id = Some(active);
                    }
                    if let Some(last) = details.get("last_task_id").and_then(|v| v.as_u64()) {
                        m.last_task_id = Some(last);
                    }
                }
                if let Some(model) = snapshot.model {
                    m.model_name = model;
                }
            }
        }

        // Poll /v1/models — get model name and metadata
        if let Ok(resp) = with_auth(client.get(format!("{base}/v1/models")), &api_key)
            .send()
            .await
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

        // T-047: slow poll interval when in low-power mode (session is active here)
        let mode = state.sleep_mode.load(std::sync::atomic::Ordering::Relaxed);
        let interval_secs = if mode >= 1 {
            if let Ok(cfg) = state.sleep_mode_config.lock() {
                cfg.sleep_llama_interval_secs.max(1)
            } else {
                poll_interval
            }
        } else {
            poll_interval
        };
        tokio::time::sleep(Duration::from_secs(interval_secs)).await;
    }
}
