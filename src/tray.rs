use std::sync::{Arc, Mutex as StdMutex};
use std::time::Duration;

use tray_icon::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tray_icon::{Icon, TrayIconBuilder};

use crate::gpu::GpuMetrics;
use crate::llama::metrics::LlamaMetrics;
use crate::state::{AppState, SessionStatus};
use crate::system::SystemMetrics;

type TrayMetrics = (
    SystemMetrics,
    Option<Vec<(String, GpuMetrics)>>,
    LlamaMetrics,
    Option<String>,
);

pub fn run_tray(state: AppState, port: u16) {
    let tray_state = TrayState {
        app_state: Arc::new(state),
        last_status: StdMutex::new(0u8),
        notified_errors: StdMutex::new(0u32),
    };

    let tray_menu = Menu::new();

    let open_item = MenuItem::with_id(
        tray_icon::menu::MenuId::new("open"),
        "Open Web UI",
        true,
        None,
    );
    let _sep1 = PredefinedMenuItem::separator();
    let metrics_item = MenuItem::with_id(
        tray_icon::menu::MenuId::new("metrics"),
        "Show Metrics",
        true,
        None,
    );
    let _sep2 = PredefinedMenuItem::separator();
    let quit_item = MenuItem::with_id(tray_icon::menu::MenuId::new("quit"), "Quit", true, None);

    tray_menu.append(&open_item).unwrap();
    tray_menu.append(&_sep1).unwrap();
    tray_menu.append(&metrics_item).unwrap();
    tray_menu.append(&_sep2).unwrap();
    tray_menu.append(&quit_item).unwrap();

    let icon_data: &[u8] = include_bytes!("../static/icon.svg");
    let icon = Icon::from_rgba(icon_data.to_vec(), 64, 64).unwrap_or_else(|_| {
        let default: &[u8] = &[0xFF, 0xFF, 0xFF, 0xFF];
        Icon::from_rgba(default.to_vec(), 1, 1).unwrap()
    });

    let tray = TrayIconBuilder::new()
        .with_menu(Box::new(tray_menu))
        .with_tooltip("Llama Monitor")
        .with_icon(icon)
        .build()
        .expect("Failed to build tray icon");

    tray_state.show_notification("Llama Monitor", "Started");

    let initial_metrics = tray_state.get_metrics();
    if let Some(ref tooltip) = initial_metrics.3 {
        let _ = tray.set_tooltip(Some(tooltip));
    }

    MenuEvent::set_event_handler(Some(move |event: MenuEvent| {
        let id = event.id.0.clone();
        match id.as_str() {
            "open" => {
                let url = format!("http://127.0.0.1:{}", port);
                let _ = webbrowser::open(&url);
            }
            "metrics" => {
                // Show metrics - tooltip is updated in the poll loop
            }
            "quit" => {
                std::process::exit(0);
            }
            _ => {}
        }
    }));

    let mut last_poll = std::time::Instant::now();
    let poll_interval = Duration::from_secs(3);
    let mut last_status_check = std::time::Instant::now();
    let status_check_interval = Duration::from_secs(10);

    loop {
        std::thread::sleep(Duration::from_millis(500));

        if last_poll.elapsed() >= poll_interval {
            last_poll = std::time::Instant::now();
            let metrics = tray_state.get_metrics();
            if let Some(ref tooltip) = metrics.3 {
                let _ = tray.set_tooltip(Some(tooltip));
            }
            tray_state.check_gpu_thresholds(&metrics.1);
        }

        if last_status_check.elapsed() >= status_check_interval {
            last_status_check = std::time::Instant::now();
            tray_state.check_session_status();
        }
    }
}

struct TrayState {
    app_state: Arc<AppState>,
    last_status: StdMutex<u8>,
    notified_errors: StdMutex<u32>,
}

impl TrayState {
    fn get_metrics(&self) -> TrayMetrics {
        let sys = self.app_state.system_metrics.lock().unwrap().clone();
        let gpu = self.app_state.gpu_metrics.lock().unwrap().clone();
        let llama = self.app_state.llama_metrics.lock().unwrap().clone();
        let gpu_entries = if gpu.is_empty() {
            None
        } else {
            Some(gpu.into_iter().collect())
        };
        let tooltip = self.build_tooltip(&sys, &gpu_entries, &llama);
        (sys, gpu_entries, llama, Some(tooltip))
    }

    fn build_tooltip(
        &self,
        sys: &SystemMetrics,
        gpu: &Option<Vec<(String, GpuMetrics)>>,
        llama: &LlamaMetrics,
    ) -> String {
        let mut lines = Vec::new();

        lines.push(format!("CPU: {}%", sys.cpu_load as f32 / 10.0));

        if sys.cpu_temp_available {
            lines.push(format!("Temp: {:.0}C", sys.cpu_temp));
        }

        if let Some(g) = gpu {
            for (name, m) in g.iter() {
                let vram_pct = if m.vram_total > 0 {
                    (m.vram_used as f64 / m.vram_total as f64 * 100.0) as u32
                } else {
                    0
                };
                lines.push(format!("{}: {:.0}C / {}% VRAM", name, m.temp, vram_pct));
            }
        }

        if llama.generation_tokens_per_sec > 0.0 {
            lines.push(format!("{:.0} tok/s", llama.generation_tokens_per_sec));
        }

        lines.join("\n")
    }

    fn check_gpu_thresholds(&self, gpu: &Option<Vec<(String, GpuMetrics)>>) {
        if let Some(g) = gpu {
            let mut error_count = *self.notified_errors.lock().unwrap();

            for (name, m) in g.iter() {
                if m.temp > 85.0 {
                    let title = "GPU Thermal Warning";
                    let body = format!("{} is at {:.0}C", name, m.temp);
                    self.show_notification(title, &body);
                    error_count += 1;
                }
            }

            let mut ec = self.notified_errors.lock().unwrap();
            *ec = error_count;
        }
    }

    fn check_session_status(&self) {
        let sessions = self.app_state.sessions.lock().unwrap();
        let active_id = self.app_state.active_session_id.lock().unwrap();

        if let Some(active_session) = sessions.iter().find(|s| s.id == *active_id) {
            let current_status = active_session.status.clone();
            let mut prev_status = self.last_status.lock().unwrap();

            let status_changed = matches!(
                (*prev_status, &current_status),
                (0, SessionStatus::Running)
                    | (1, SessionStatus::Disconnected)
                    | (1, SessionStatus::Error(_))
            );

            if status_changed {
                match &current_status {
                    SessionStatus::Running => {
                        self.show_notification(
                            "Session Started",
                            &format!("{} is running", active_session.name),
                        );
                    }
                    SessionStatus::Disconnected => {
                        self.show_notification(
                            "Session Disconnected",
                            &format!("{} disconnected", active_session.name),
                        );
                    }
                    SessionStatus::Error(msg) => {
                        self.show_notification(
                            "Session Error",
                            &format!("{}: {}", active_session.name, msg),
                        );
                    }
                    _ => {}
                }

                *prev_status = match &current_status {
                    SessionStatus::Running => 1,
                    SessionStatus::Disconnected => 2,
                    SessionStatus::Error(_) => 3,
                    _ => 0,
                };
            }
        }
    }

    fn show_notification(&self, title: &str, body: &str) {
        let mut b = notify_rust::Notification::new();
        let _ = b.appname("llama-monitor").summary(title).body(body);

        #[cfg(target_os = "linux")]
        {
            let _ = b.icon("system-devices-panel");
        }

        let _ = b.show();
    }
}
