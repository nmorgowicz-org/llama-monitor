use futures_util::{SinkExt, StreamExt};
use std::time::Duration;
use warp::Filter;
use warp::ws::{Message, Ws};

use crate::state::AppState;

const WS_PUSH_INTERVAL: Duration = Duration::from_millis(500);

pub fn ws_route(
    state: AppState,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    let ws_state = state;
    warp::path("ws").and(warp::ws()).map(move |ws: Ws| {
        let state = ws_state.clone();
        ws.on_upgrade(move |socket| {
            let state = state.clone();
            async move {
                let (mut ws_tx, mut ws_rx) = socket.split();

                let update_task = tokio::spawn(async move {
                    let mut interval = tokio::time::interval(WS_PUSH_INTERVAL);
                    loop {
                        if state.active_session_id.lock().unwrap().is_empty() {
                            state.llama_poll_notify.notified().await;
                            continue;
                        }

                        interval.tick().await;

                        let running = *state.server_running.lock().unwrap();
                        let local_running = *state.local_server_running.lock().unwrap();

                        let json = {
                            let local_metrics_available = state.active_session_uses_local_metrics();
                            let host_metrics_available = state.host_metrics_available();
                            let remote_agent_connected = state.remote_agent_connected();
                            let remote_agent_url = state.remote_agent_url.lock().unwrap().clone();
                            let gpu = if host_metrics_available {
                                state.gpu_metrics.lock().unwrap().clone()
                            } else {
                                Default::default()
                            };
                            let llama = state.llama_metrics.lock().unwrap().clone();
                            let system = if host_metrics_available {
                                Some(state.system_metrics.lock().unwrap().clone())
                            } else {
                                None
                            };
                            let logs: Vec<String> =
                                state.server_logs.lock().unwrap().iter().cloned().collect();
                            let active_session_id = state.active_session_id.lock().unwrap().clone();
                            let sessions = state.sessions.lock().unwrap();
                            let session_mode = sessions
                                .iter()
                                .find(|s| s.id == active_session_id)
                                .map(|s| match &s.mode {
                                    crate::state::SessionMode::Spawn { .. } => "spawn",
                                    crate::state::SessionMode::Attach { .. } => "attach",
                                })
                                .unwrap_or("");
                            let active_session_endpoint = sessions
                                .iter()
                                .find(|s| s.id == active_session_id)
                                .map(|s| match &s.mode {
                                    crate::state::SessionMode::Spawn { port } => {
                                        format!("http://127.0.0.1:{port}")
                                    }
                                    crate::state::SessionMode::Attach { endpoint } => {
                                        endpoint.clone()
                                    }
                                })
                                .unwrap_or_default();
                            drop(sessions);
                            let capabilities = state.calculate_capabilities();
                            let endpoint_kind = state.current_endpoint_kind();
                            let session_kind = state.current_session_kind();
                            let (system_reason, gpu_reason, cpu_temp_reason) =
                                state.calculate_availability_reasons();
                            serde_json::json!({
                                "gpu": gpu,
                                "llama": llama,
                                "system": system,
                                "logs": logs,
                                "server_running": running,
                                "local_server_running": local_running,
                                "session_mode": session_mode,
                                "active_session_id": active_session_id,
                                "active_session_endpoint": active_session_endpoint,
                                "local_metrics_available": local_metrics_available,
                                "host_metrics_available": host_metrics_available,
                                "remote_agent_connected": remote_agent_connected,
                                "remote_agent_health_reachable": remote_agent_connected,
                                "remote_agent_url": remote_agent_url,
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
                        if ws_tx.send(Message::text(json.to_string())).await.is_err() {
                            break;
                        }
                    }
                });

                while let Some(_msg) = ws_rx.next().await {}
                update_task.abort();
            }
        })
    })
}
