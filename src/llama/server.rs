use anyhow::Result;
use std::sync::atomic::Ordering;
use crate::config::AppConfig;
use crate::state::AppState;
use std::sync::Arc;
use crate::inference::llama_cpp::ServerConfig;

pub async fn start_server(
    state: Arc<AppState>,
    config: ServerConfig,
    app_config: &AppConfig,
) -> Result<()> {
    let adapter = Arc::new(crate::inference::llama_cpp::LlamaCppAdapter::new(app_config.clone(), config.clone()));
    adapter.validate().await?;

    let launch = adapter.build_launch().await?;
    let supervisor = Arc::new(crate::inference::supervisor::Supervisor::new(
        launch,
        adapter.clone(),
        state.clone(),
    ));
    supervisor.clone().start().await?;

    {
        let mut backend_lock = state.backend.lock().unwrap();
        *backend_lock = Some(crate::inference::backend::BackendAdapter::LlamaCpp(adapter));
    }
    let mut supervisor_lock = state.supervisor.lock().await;
    *supervisor_lock = Some(supervisor);

    state.llama_poll_notify.notify_waiters();
    Ok(())
}

pub async fn stop_server(state: &AppState) -> Result<()> {
    state.server_stopping.store(true, Ordering::Relaxed);

    if let Some(supervisor) = state.supervisor.lock().await.take() {
        supervisor.stop().await?;
    }

    {
        let mut r = state.server_running.lock().unwrap();
        *r = false;
    }
    {
        let mut local_running = state.local_server_running.lock().unwrap();
        *local_running = false;
    }
    {
        let mut cfg = state.server_config.lock().unwrap();
        *cfg = None;
    }
    {
        let mut m = state.llama_metrics.lock().unwrap();
        *m = crate::llama::metrics::LlamaMetrics::default();
    }
    state.push_log("[monitor] Server stopped.".into());
    state.server_stopping.store(false, Ordering::Relaxed);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;



    #[allow(dead_code)]
    fn command_args(_config: &ServerConfig) -> Vec<String> {
        vec![]
    }

    #[allow(dead_code)]
    fn kv_command_args(_config: &ServerConfig) -> Vec<String> {
        vec![]
    }

    #[test]
    fn kv_unified_supports_default_on_and_off() {
        // tests...
    }
}
