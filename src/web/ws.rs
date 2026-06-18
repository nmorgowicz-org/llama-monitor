use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use warp::Filter;
use warp::ws::Message;

use crate::state::AppState;
#[cfg(test)]
use crate::state::MetricsCapabilities;

static WS_CONNECTIONS: AtomicUsize = AtomicUsize::new(0);

const WS_PUSH_INTERVAL_DEFAULT_MS: u64 = 500;
const WS_PUSH_INTERVAL_MIN_MS: u64 = 200;
const WS_PUSH_INTERVAL_MAX_MS: u64 = 10_000;
const WS_PUSH_INTERVAL_HIDDEN_MS: u64 = 5_000;
const WS_PUSH_INTERVAL_SLEEP_MS: u64 = 10_000; // T-049 / T-053: max interval when asleep
const MAX_WS_CONNECTIONS: usize = 50;

fn clamped_push_interval_ms(settings: &crate::state::UiSettings, asleep: bool) -> u64 {
    let val = settings.ws_push_interval_ms;
    let base = val.clamp(WS_PUSH_INTERVAL_MIN_MS, WS_PUSH_INTERVAL_MAX_MS);

    // T-049 / T-053: when asleep, enforce slow interval from config
    if asleep {
        let slow_ms = settings.sleep_mode.sleep_ws_interval_ms;
        base.max(slow_ms)
    } else {
        base
    }
}

pub fn ws_route(
    ws_state: AppState,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    warp::path("ws")
        .and(warp::ws())
        .map(move |ws: warp::ws::Ws| {
            // T-051: wake-on-activity: new WS connection counts as activity
            let current = WS_CONNECTIONS.fetch_add(1, Ordering::Relaxed);
            if current >= MAX_WS_CONNECTIONS {
                WS_CONNECTIONS.fetch_sub(1, Ordering::Relaxed);
                let reply: Box<dyn warp::reply::Reply> = Box::new(warp::reply::with_status(
                    warp::reply::json(&serde_json::json!({ "error": "too many connections" })),
                    warp::http::StatusCode::TOO_MANY_REQUESTS,
                ));
                return reply;
            }

            // Record activity timestamp (T-051)
            {
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs();
                ws_state.last_activity_at.store(now, Ordering::Relaxed);
            }

            let state = ws_state.clone();
            let upgrade = ws.on_upgrade(move |socket| {
                let state = state.clone();
                async move {
                    let (mut ws_tx, mut ws_rx) = socket.split();
                    let client_visible = Arc::new(AtomicBool::new(true));

                    // T-051: On open: wake auto-sleep only. Manual sleep persists across reconnects.
                    let asleep_on_open = state.sleep_mode.load(Ordering::Relaxed);
                    let manual_on_open = state.sleep_mode_manual.load(Ordering::Relaxed);
                    eprintln!(
                        "[sleep] WS open: asleep={} manual={}{}",
                        asleep_on_open,
                        manual_on_open,
                        if asleep_on_open && !manual_on_open { " → waking (auto-sleep)" } else { "" }
                    );
                    if asleep_on_open && !manual_on_open {
                        state.sleep_mode.store(false, Ordering::Relaxed);
                        state.sleep_notify.notify_waiters();
                    }

                    let update_visible = Arc::clone(&client_visible);

                    // Spawn broadcast task using its own clone (avoids move/borrow conflicts)
                    let update_task = {
                        let s = state.clone();
                        tokio::spawn(async move {
                            let mut last_interval_ms = WS_PUSH_INTERVAL_DEFAULT_MS;
                            loop {
                                if s.active_session_id.lock().unwrap().is_empty() {
                                    s.llama_poll_notify.notified().await;
                                    continue;
                                }

                                // Check if the push interval has changed in settings
                                let current_ms = {
                                    let settings = s.ui_settings.lock().unwrap();
                                    let asleep = s.sleep_mode.load(Ordering::Relaxed);
                                    clamped_push_interval_ms(&settings, asleep)
                                };
                                if current_ms != last_interval_ms {
                                    last_interval_ms = current_ms;
                                }

                                // T-049 / T-053: effective interval depends on visibility + sleep_mode
                                let asleep = s.sleep_mode.load(Ordering::Relaxed);
                                let client_vis = update_visible.load(Ordering::Relaxed);
                                let effective_interval_ms = if asleep {
                                    last_interval_ms.max(WS_PUSH_INTERVAL_SLEEP_MS)
                                } else if client_vis {
                                    last_interval_ms
                                } else {
                                    last_interval_ms.max(WS_PUSH_INTERVAL_HIDDEN_MS)
                                };

                                // T-056: if chat streaming is active, don't sleep interval even if asleep
                                let streaming_active = {
                                    let llama = s.llama_metrics.lock().unwrap();
                                    llama.generation_tokens_per_sec > 0.0
                                };
                                let final_interval_ms = if streaming_active && asleep {
                                    last_interval_ms
                                } else {
                                    effective_interval_ms
                                };

                                tokio::time::sleep(Duration::from_millis(final_interval_ms)).await;

                                let running = *s.server_running.lock().unwrap();
                                let local_running = *s.local_server_running.lock().unwrap();
                                let asleep = s.sleep_mode.load(Ordering::Relaxed);

                                // T-049: when asleep, send minimal payload (heartbeat + critical flags)
                                let json = if asleep {
                                    let active_session_id =
                                        s.active_session_id.lock().unwrap().clone();
                                    let active_status = {
                                        let sessions = s.sessions.lock().unwrap();
                                        sessions
                                            .iter()
                                            .find(|ss| ss.id == active_session_id)
                                            .map(|ss| match &ss.status {
                                                crate::state::SessionStatus::Stopped => "stopped",
                                                crate::state::SessionStatus::Running => "running",
                                                crate::state::SessionStatus::Disconnected => "disconnected",
                                                crate::state::SessionStatus::Error(_) => "error",
                                            })
                                            .unwrap_or("stopped")
                                    };

                                    let is_manual = s.sleep_mode_manual.load(Ordering::Relaxed);
                                    serde_json::json!({
                                        "sleep_mode": true,
                                        "sleep_mode_manual": is_manual,
                                        "server_running": running,
                                        "local_server_running": local_running,
                                        "active_session_id": active_session_id,
                                        "active_session_status": active_status
                                    })
                                } else {
                                    // Full payload (normal mode)
                                    let local_metrics_available =
                                        s.active_session_uses_local_metrics();
                                    let host_metrics_available = s.host_metrics_available();
                                    let remote_agent_connected = s.remote_agent_connected();
                                    let remote_agent_health_reachable =
                                        s.remote_agent_health_reachable();
                                    let remote_agent_url =
                                        s.remote_agent_url.lock().unwrap().clone();
                                    let remote_agent_version =
                                        s.remote_agent_version.lock().unwrap().clone();
                                    let remote_agent_update_available =
                                        *s.remote_agent_update_available.lock().unwrap();

                                    let gpu = if host_metrics_available {
                                        s.gpu_metrics.lock().unwrap().clone()
                                    } else {
                                        Default::default()
                                    };
                                    let llama = s.llama_metrics.lock().unwrap().clone();
                                    let system = if host_metrics_available {
                                        Some(s.system_metrics.lock().unwrap().clone())
                                    } else {
                                        None
                                    };
                                    let logs: Vec<String> = s
                                        .server_logs
                                        .lock()
                                        .unwrap()
                                        .iter()
                                        .cloned()
                                        .collect();
                                    let active_session_id =
                                        s.active_session_id.lock().unwrap().clone();

                                    let sessions = s.sessions.lock().unwrap();
                                    let session_mode = sessions
                                        .iter()
                                        .find(|ss| ss.id == active_session_id)
                                        .map(|ss| match &ss.mode {
                                            crate::state::SessionMode::Spawn { .. } => "spawn",
                                            crate::state::SessionMode::Attach { .. } => "attach",
                                        })
                                        .unwrap_or("");
                                    let active_session_endpoint = sessions
                                        .iter()
                                        .find(|ss| ss.id == active_session_id)
                                        .map(|ss| match &ss.mode {
                                            crate::state::SessionMode::Spawn { port, .. } => {
                                                format!("http://127.0.0.1:{port}")
                                            }
                                            crate::state::SessionMode::Attach {
                                                endpoint, ..
                                            } => {
                                                endpoint.clone()
                                            }
                                        })
                                        .unwrap_or_default();
                                    drop(sessions);

                                    let capabilities = s.calculate_capabilities();
                                    let endpoint_kind = s.current_endpoint_kind();
                                    let session_kind = s.current_session_kind();
                                    let (system_reason, gpu_reason, cpu_temp_reason) =
                                        s.calculate_availability_reasons();
                                    let last_spawn_cmd =
                                        s.last_spawn_cmd.lock().unwrap().clone();

                                    let active_session_status = {
                                        let sessions = s.sessions.lock().unwrap();
                                        sessions
                                            .iter()
                                            .find(|ss| ss.id == active_session_id)
                                            .map(|ss| match &ss.status {
                                                crate::state::SessionStatus::Stopped => "stopped",
                                                crate::state::SessionStatus::Running => "running",
                                                crate::state::SessionStatus::Disconnected => "disconnected",
                                                crate::state::SessionStatus::Error(_) => "error",
                                            })
                                            .unwrap_or("stopped")
                                    };

                                    let active_session_error = {
                                        let sessions = s.sessions.lock().unwrap();
                                        sessions
                                            .iter()
                                            .find(|ss| ss.id == active_session_id)
                                            .and_then(|ss| {
                                                if let crate::state::SessionStatus::Error(msg) =
                                                    &ss.status
                                                {
                                                    Some(msg.clone())
                                                } else {
                                                    None
                                                }
                                            })
                                    };

                                    let active_session_preset_id = {
                                        let sessions = s.sessions.lock().unwrap();
                                        sessions
                                            .iter()
                                            .find(|ss| ss.id == active_session_id)
                                            .and_then(|ss| {
                                                if ss.preset_id.is_empty() {
                                                    None
                                                } else {
                                                    Some(ss.preset_id.clone())
                                                }
                                            })
                                    };

                                    let is_manual = s.sleep_mode_manual.load(Ordering::Relaxed);
                                    serde_json::json!({
                                        "sleep_mode": asleep,
                                        "sleep_mode_manual": is_manual,
                                        "gpu": gpu,
                                        "llama": llama,
                                        "system": system,
                                        "logs": logs,
                                        "last_spawn_cmd": last_spawn_cmd,
                                        "server_running": running,
                                        "local_server_running": local_running,
                                        "session_mode": session_mode,
                                        "active_session_status": active_session_status,
                                        "active_session_error": active_session_error,
                                        "active_session_id": active_session_id,
                                        "active_session_endpoint": active_session_endpoint,
                                        "active_session_preset_id": active_session_preset_id,
                                        "local_metrics_available": local_metrics_available,
                                        "host_metrics_available": host_metrics_available,
                                        "remote_agent_connected": remote_agent_connected,
                                        "remote_agent_health_reachable": remote_agent_health_reachable,
                                        "remote_agent_url": remote_agent_url,
                                        "remote_agent_version": remote_agent_version,
                                        "remote_agent_protocol_version": *s.remote_agent_protocol_version.lock().unwrap(),
                                        "remote_agent_update_available": remote_agent_update_available,
                                        "remote_agent_protocol_too_old": *s.remote_agent_protocol_too_old.lock().unwrap(),
                                        "capabilities": capabilities,
                                        "endpoint_kind": endpoint_kind,
                                        "session_kind": session_kind,
                                        "availability": {
                                            "system": system_reason,
                                            "gpu": gpu_reason,
                                            "cpu_temp": cpu_temp_reason
                                        }
                                    })
                                };

                                if ws_tx
                                    .send(Message::text(json.to_string()))
                                    .await
                                    .is_err()
                                {
                                    break;
                                }
                            }
                        })
                    };

                    // Message receive loop (uses outer state)
                    while let Some(msg) = ws_rx.next().await {
                        let Ok(msg) = msg else { break };
                        if !msg.is_text() {
                            continue;
                        }
                        let Ok(text) = msg.to_str() else { continue };
                        let Ok(value) = serde_json::from_str::<serde_json::Value>(text) else {
                            continue
                        };

                        // T-051: record activity on all messages
                        {
                            let now = std::time::SystemTime::now()
                                .duration_since(std::time::UNIX_EPOCH)
                                .unwrap_or_default()
                                .as_secs();
                            state.last_activity_at.store(now, Ordering::Relaxed);
                        }

                        let msg_type = value.get("type").and_then(|v| v.as_str());

                        // T-053: client-visibility with mode (active/idle/sleep)
                        if msg_type == Some("client-visibility") {
                            let visible =
                                value.get("visible").and_then(|v| v.as_bool());
                            let mode = value.get("mode").and_then(|v| v.as_str());

                            if let Some(vis) = visible {
                                client_visible.store(vis, Ordering::Relaxed);
                            }

                            // T-051: wake auto-sleep on active visibility; manual sleep is exempt
                            if mode == Some("active") || visible == Some(true) {
                                let asleep = state.sleep_mode.load(Ordering::Relaxed);
                                let manual = state.sleep_mode_manual.load(Ordering::Relaxed);
                                if asleep && !manual {
                                    state.sleep_mode.store(false, Ordering::Relaxed);
                                    state.sleep_notify.notify_waiters();
                                }
                            }
                        }

                        // T-051: explicit wake command from client — clears manual flag too
                        if msg_type == Some("wake") {
                            state.sleep_mode_manual.store(false, Ordering::Relaxed);
                            let asleep = state.sleep_mode.load(Ordering::Relaxed);
                            if asleep {
                                state.sleep_mode.store(false, Ordering::Relaxed);
                                state.sleep_notify.notify_waiters();
                            }
                        }
                    }

                    update_task.abort();
                    WS_CONNECTIONS.fetch_sub(1, Ordering::Relaxed);
                }
            });

            Box::new(upgrade)
        })
}

#[cfg(test)]
fn is_full_capabilities(caps: &MetricsCapabilities, _sleep_mode: bool) -> bool {
    caps.inference
        && caps.system
        && caps.gpu
        && caps.cpu_temperature
        && caps.memory
        && caps.host_metrics
        && caps.tray
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_full_capabilities_returns_true_when_full() {
        let caps = MetricsCapabilities {
            inference: true,
            system: true,
            gpu: true,
            cpu_temperature: true,
            memory: true,
            host_metrics: true,
            tray: true,
            sensor_bridge_setup_available: true,
        };
        assert!(is_full_capabilities(&caps, false));
    }

    #[test]
    fn is_full_capabilities_returns_false_when_missing_system() {
        let caps = MetricsCapabilities {
            inference: true,
            system: false,
            gpu: true,
            cpu_temperature: true,
            memory: true,
            host_metrics: true,
            tray: true,
            sensor_bridge_setup_available: true,
        };
        assert!(!is_full_capabilities(&caps, false));
    }

    #[test]
    fn ws_connections_increment_decrement() {
        let before = WS_CONNECTIONS.load(Ordering::Relaxed);
        WS_CONNECTIONS.fetch_add(1, Ordering::Relaxed);
        assert_eq!(WS_CONNECTIONS.load(Ordering::Relaxed), before + 1);
        WS_CONNECTIONS.fetch_sub(1, Ordering::Relaxed);
        assert_eq!(WS_CONNECTIONS.load(Ordering::Relaxed), before);
    }
}
