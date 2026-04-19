use std::sync::{Arc, Mutex as StdMutex};
use std::time::{Duration, Instant};

use tray_icon::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tray_icon::{Icon, TrayIcon, TrayIconBuilder};
use tray_icon::menu::MenuId;
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::WindowId;

#[cfg(target_os = "macos")]
use winit::platform::macos::{ActivationPolicy, EventLoopBuilderExtMacOS};

#[cfg(feature = "tray-popover")]
use tray_icon::TrayIconEvent;
#[cfg(feature = "tray-popover")]
use winit::window::{WindowAttributes, WindowLevel};
#[cfg(feature = "tray-popover")]
use winit::dpi::PhysicalPosition;

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
    let size = 22u32;
    let mut rgba = vec![0u8; (size * size * 4) as usize];

    let mut set = |x: u32, y: u32| {
        if x < size && y < size {
            let idx = ((y * size + x) * 4) as usize;
            rgba[idx + 3] = 255;
        }
    };

    for x in 2..=19 {
        set(x, 3);
        set(x, 4);
        set(x, 13);
    }
    for y in 3..=13 {
        set(2, y);
        set(3, y);
        set(18, y);
        set(19, y);
    }

    for y in 14..=16 {
        set(10, y);
        set(11, y);
    }

    for x in 7..=14 {
        set(x, 17);
        set(x, 18);
    }

    Icon::from_rgba(rgba, size, size).unwrap_or_else(|_| {
        Icon::from_rgba(vec![0, 0, 0, 255], 1, 1).unwrap()
    })
}

pub fn run_tray(state: AppState, port: u16) {
    #[cfg(target_os = "macos")]
    {
        let _ = mac_notification_sys::set_application("com.apple.Finder");
    }

    #[cfg(target_os = "macos")]
    let event_loop = EventLoop::builder()
        .with_activation_policy(ActivationPolicy::Accessory)
        .build()
        .expect("Failed to create event loop");

    #[cfg(not(target_os = "macos"))]
    let event_loop = EventLoop::builder()
        .build()
        .expect("Failed to create event loop");

    let tray_state = TrayState {
        app_state: Arc::new(state),
        last_status: StdMutex::new(0u8),
        notified_errors: StdMutex::new(0u32),
    };

    let tray_menu = Menu::new();

    let cpu_item = MenuItem::with_id(MenuId::new("stat_cpu"), "CPU: —", true, None);
    let gpu_item = MenuItem::with_id(MenuId::new("stat_gpu"), "GPU: —", true, None);
    let sep1 = PredefinedMenuItem::separator();
    let open_item = MenuItem::with_id(MenuId::new("open"), "Open Web UI", true, None);
    let sep2 = PredefinedMenuItem::separator();
    let quit_item = MenuItem::with_id(MenuId::new("quit"), "Quit", true, None);

    tray_menu.append(&cpu_item).unwrap();
    tray_menu.append(&gpu_item).unwrap();
    tray_menu.append(&sep1).unwrap();
    tray_menu.append(&open_item).unwrap();
    tray_menu.append(&sep2).unwrap();
    tray_menu.append(&quit_item).unwrap();

    let icon = create_tray_icon();

    let mut app = TrayApp {
        tray_state,
        tray: None,
        tray_menu: Box::new(tray_menu),
        icon,
        cpu_item,
        gpu_item,
        port,
        last_poll: Instant::now(),
        poll_interval: Duration::from_secs(3),
        last_status_check: Instant::now(),
        status_check_interval: Duration::from_secs(10),
        #[cfg(feature = "tray-popover")]
        popover: None,
    };

    event_loop.run_app(&mut app).expect("Event loop error");
}

struct TrayApp {
    tray_state: TrayState,
    tray: Option<TrayIcon>,
    tray_menu: Box<dyn tray_icon::menu::ContextMenu>,
    icon: Icon,
    cpu_item: MenuItem,
    gpu_item: MenuItem,
    port: u16,
    last_poll: Instant,
    poll_interval: Duration,
    last_status_check: Instant,
    status_check_interval: Duration,
    #[cfg(feature = "tray-popover")]
    popover: Option<(winit::window::Window, wry::WebView)>,
}

impl ApplicationHandler for TrayApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let builder = TrayIconBuilder::new()
            .with_menu(std::mem::replace(
                &mut self.tray_menu,
                Box::new(Menu::new()),
            ))
            .with_tooltip("Llama Monitor")
            .with_icon(std::mem::replace(
                &mut self.icon,
                Icon::from_rgba(vec![0, 0, 0, 255], 1, 1).unwrap(),
            ));

        #[cfg(target_os = "macos")]
        let builder = builder.with_icon_as_template(true);

        let tray = builder.build();
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

    fn window_event(&mut self, _event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        #[cfg(feature = "tray-popover")]
        if let WindowEvent::Focused(false) = event {
            if self.popover.is_some() {
                self.close_popover();
            }
        }
    }

    fn new_events(&mut self, event_loop: &ActiveEventLoop, _cause: winit::event::StartCause) {
        while let Ok(event) = MenuEvent::receiver().try_recv() {
            match event.id.0.as_str() {
                "open" | "stat_cpu" | "stat_gpu" => {
                    let url = format!("http://127.0.0.1:{}", self.port);
                    let _ = webbrowser::open(&url);
                }
                "quit" => {
                    std::process::exit(0);
                }
                _ => {}
            }
        }

        #[cfg(feature = "tray-popover")]
        {
            while let Ok(tray_event) = TrayIconEvent::receiver().try_recv() {
                match &tray_event {
                    TrayIconEvent::Click { button, rect, .. } if *button == tray_icon::MouseButton::Left => {
                        if self.popover.is_some() {
                            self.close_popover();
                        } else if let Some(ref tray) = self.tray {
                            if let Some(rect) = tray.rect() {
                                self.open_popover(event_loop, rect);
                            }
                        }
                    }
                    _ => {}
                }
            }
        }

        if let Some(ref tray) = self.tray {
            if self.last_poll.elapsed() >= self.poll_interval {
                self.last_poll = Instant::now();
                let metrics = self.tray_state.get_metrics();

                self.cpu_item.set_text(self.tray_state.build_cpu_line(&metrics.0));
                self.gpu_item.set_text(self.tray_state.build_gpu_line(&metrics.1));

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

#[cfg(feature = "tray-popover")]
impl TrayApp {
    fn open_popover(&mut self, event_loop: &ActiveEventLoop, icon_rect: tray_icon::Rect) {
        let width = 320u32;
        let height = 400u32;

        let pos = icon_rect.position;
        let x = pos.x + (icon_rect.size.width as f64 / 2.0) - (width as f64 / 2.0);
        let y = pos.y + icon_rect.size.height as f64 + 4.0;

        let attrs = WindowAttributes::default()
            .with_inner_size(winit::dpi::PhysicalSize::new(width, height))
            .with_position(PhysicalPosition::new(x as i32, y as i32))
            .with_decorations(false)
            .with_resizable(false)
            .with_window_level(WindowLevel::AlwaysOnTop)
            .with_visible(true);

        let window = match event_loop.create_window(attrs) {
            Ok(w) => w,
            Err(e) => {
                eprintln!("[tray] Failed to create popover window: {}", e);
                return;
            }
        };

        let webview = match wry::WebViewBuilder::new()
            .with_url(format!("http://127.0.0.1:{}/compact", self.port))
            .build(&window)
        {
            Ok(wv) => wv,
            Err(e) => {
                eprintln!("[tray] Failed to create webview: {}", e);
                return;
            }
        };

        #[cfg(target_os = "macos")]
        {
            use winit::platform::macos::WindowExtMacOS;
            window.set_accepts_mouse_moved_events(true);
        }

        self.popover = Some((window, webview));
    }

    fn close_popover(&mut self) {
        self.popover.take();
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

    fn build_cpu_line(&self, sys: &SystemMetrics) -> String {
        let load = sys.cpu_load as f32 / 10.0;
        if sys.cpu_temp_available {
            format!("CPU: {:.0}% · {:.0}°C", load, sys.cpu_temp)
        } else {
            format!("CPU: {:.0}%", load)
        }
    }

    fn build_gpu_line(&self, gpu: &Option<Vec<(String, GpuMetrics)>>) -> String {
        match gpu {
            Some(entries) if !entries.is_empty() => {
                let (_, m) = &entries[0];
                let vram_pct = if m.vram_total > 0 {
                    m.vram_used * 100 / m.vram_total
                } else {
                    0
                };
                format!("GPU: {:.0}°C · {}% VRAM", m.temp, vram_pct)
            }
            _ => "GPU: —".to_string(),
        }
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
