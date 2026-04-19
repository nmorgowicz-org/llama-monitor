use std::sync::{Arc, Mutex as StdMutex};
use std::time::{Duration, Instant};

use tray_icon::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tray_icon::{Icon, TrayIcon, TrayIconBuilder};
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::platform::macos::{ActivationPolicy, EventLoopBuilderExtMacOS};
use winit::window::WindowId;

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

fn create_tray_icon() -> Icon {
    let size = 64u32;
    let mut rgba = vec![0u8; (size * size * 4) as usize];

    for y in 0..size {
        for x in 0..size {
            let idx = ((y * size + x) * 4) as usize;
            let cx = size / 2;
            let cy = size / 2;
            let dx = (x as i32 - cx as i32) as f32;
            let dy = (y as i32 - cy as i32) as f32;
            let dist = (dx * dx + dy * dy).sqrt();

            if dist < size as f32 / 2.0 - 2.0 {
                let t = (dist / (size as f32 / 2.0)) * 0.3;
                rgba[idx] = (46.0 * (1.0 - t) + 136.0 * t) as u8;
                rgba[idx + 1] = (52.0 * (1.0 - t) + 192.0 * t) as u8;
                rgba[idx + 2] = (64.0 * (1.0 - t) + 209.0 * t) as u8;
                rgba[idx + 3] = 255;
            } else {
                rgba[idx] = 0;
                rgba[idx + 1] = 0;
                rgba[idx + 2] = 0;
                rgba[idx + 3] = 0;
            }
        }
    }

    Icon::from_rgba(rgba, size, size).unwrap_or_else(|_| {
        let default: &[u8] = &[0, 128, 255, 255];
        Icon::from_rgba(default.to_vec(), 1, 1).unwrap()
    })
}

pub fn run_tray(state: AppState, port: u16) {
    #[cfg(target_os = "macos")]
    {
        let _ = mac_notification_sys::set_application("com.apple.Finder");
    }

    let event_loop = EventLoop::builder()
        .with_activation_policy(ActivationPolicy::Accessory)
        .build()
        .expect("Failed to create event loop");

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
    let sep1 = PredefinedMenuItem::separator();
    let metrics_item = MenuItem::with_id(
        tray_icon::menu::MenuId::new("metrics"),
        "Show Metrics",
        true,
        None,
    );
    let sep2 = PredefinedMenuItem::separator();
    let quit_item = MenuItem::with_id(tray_icon::menu::MenuId::new("quit"), "Quit", true, None);

    tray_menu.append(&open_item).unwrap();
    tray_menu.append(&sep1).unwrap();
    tray_menu.append(&metrics_item).unwrap();
    tray_menu.append(&sep2).unwrap();
    tray_menu.append(&quit_item).unwrap();

    let icon = create_tray_icon();

    let mut app = TrayApp {
        tray_state,
        tray: None,
        tray_menu: Box::new(tray_menu),
        icon,
        port,
        last_poll: Instant::now(),
        poll_interval: Duration::from_secs(3),
        last_status_check: Instant::now(),
        status_check_interval: Duration::from_secs(10),
    };

    event_loop.run_app(&mut app).expect("Event loop error");
}

struct TrayApp {
    tray_state: TrayState,
    tray: Option<TrayIcon>,
    tray_menu: Box<dyn tray_icon::menu::ContextMenu>,
    icon: Icon,
    port: u16,
    last_poll: Instant,
    poll_interval: Duration,
    last_status_check: Instant,
    status_check_interval: Duration,
}

impl ApplicationHandler for TrayApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let tray = TrayIconBuilder::new()
            .with_menu(std::mem::replace(
                &mut self.tray_menu,
                Box::new(Menu::new()),
            ))
            .with_tooltip("Llama Monitor")
            .with_icon(std::mem::replace(
                &mut self.icon,
                Icon::from_rgba(vec![0xFF, 0xFF, 0xFF, 0xFF], 1, 1).unwrap(),
            ))
            .build();
        self.tray = tray.ok();
        if self.tray.is_some() {
            eprintln!("[tray] tray icon created successfully");
        } else {
            eprintln!("[tray] FAILED to create tray icon");
        }

        self.tray_state
            .show_notification("Llama Monitor", "Started");

        let initial_metrics = self.tray_state.get_metrics();
        if let Some(ref tooltip) = initial_metrics.3
            && let Some(ref tray) = self.tray
        {
            let _ = tray.set_tooltip(Some(tooltip));
        }

        event_loop.set_control_flow(ControlFlow::WaitUntil(
            Instant::now() + Duration::from_millis(500),
        ));
    }

    fn window_event(&mut self, _: &ActiveEventLoop, _: WindowId, _: WindowEvent) {}

    fn new_events(&mut self, _event_loop: &ActiveEventLoop, _cause: winit::event::StartCause) {
        while let Ok(event) = MenuEvent::receiver().try_recv() {
            match event.id.0.as_str() {
                "open" => {
                    let url = format!("http://127.0.0.1:{}", self.port);
                    let _ = webbrowser::open(&url);
                }
                "quit" => {
                    std::process::exit(0);
                }
                _ => {}
            }
        }

        if let Some(ref tray) = self.tray {
            if self.last_poll.elapsed() >= self.poll_interval {
                self.last_poll = Instant::now();
                let metrics = self.tray_state.get_metrics();
                if let Some(ref tooltip) = metrics.3 {
                    let _ = tray.set_tooltip(Some(tooltip));
                }
                self.tray_state.check_gpu_thresholds(&metrics.1);
            }

            if self.last_status_check.elapsed() >= self.status_check_interval {
                self.last_status_check = Instant::now();
                self.tray_state.check_session_status();
            }
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        event_loop.set_control_flow(ControlFlow::WaitUntil(
            Instant::now() + Duration::from_millis(500),
        ));
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
        #[cfg(target_os = "macos")]
        {
            let _ = mac_notification_sys::set_application("com.apple.Finder");
        }
        let mut b = notify_rust::Notification::new();
        let _ = b.appname("llama-monitor").summary(title).body(body);

        #[cfg(target_os = "linux")]
        {
            let _ = b.icon("system-devices-panel");
        }

        let _ = b.show();
    }
}
