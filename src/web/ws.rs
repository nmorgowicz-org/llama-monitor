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
                        interval.tick().await;

                        let running = *state.server_running.lock().unwrap();

                        let json = {
                            let local_metrics_available = state.active_session_uses_local_metrics();
                            let gpu = if local_metrics_available {
                                state.gpu_metrics.lock().unwrap().clone()
                            } else {
                                Default::default()
                            };
                            let llama = state.llama_metrics.lock().unwrap().clone();
                            let system = if local_metrics_available {
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
                            drop(sessions);
                            serde_json::json!({
                                "gpu": gpu,
                                "llama": llama,
                                "system": system,
                                "logs": logs,
                                "server_running": running,
                                "session_mode": session_mode,
                                "active_session_id": active_session_id,
                                "local_metrics_available": local_metrics_available
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
