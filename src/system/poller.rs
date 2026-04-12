use std::time::Duration;

use crate::state::AppState;
use crate::system::get_system_metrics;

const SYSTEM_POLL_INTERVAL: Duration = Duration::from_secs(5);

pub async fn system_metrics_poller(state: AppState) {
    loop {
        let metrics = get_system_metrics();
        *state.system_metrics.lock().unwrap() = metrics;
        tokio::time::sleep(SYSTEM_POLL_INTERVAL).await;
    }
}
