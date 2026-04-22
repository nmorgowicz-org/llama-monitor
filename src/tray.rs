use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
#[cfg(all(feature = "webview-popover", not(target_os = "windows")))]
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::{Duration, Instant};

#[cfg(all(feature = "webview-popover", not(target_os = "windows")))]
use tray_icon::TrayIconEvent;
#[cfg(target_os = "windows")]
use tray_icon::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tray_icon::{Icon, TrayIcon, TrayIconBuilder};
use winit::application::ApplicationHandler;
#[cfg(all(feature = "webview-popover", not(target_os = "windows")))]
use winit::dpi::PhysicalPosition;
#[cfg(all(feature = "webview-popover", not(target_os = "windows")))]
use winit::dpi::PhysicalSize;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::WindowId;
#[cfg(all(
    not(target_os = "linux"),
    not(target_os = "windows"),
    feature = "webview-popover"
))]
use winit::window::{WindowAttributes, WindowLevel};

#[cfg(target_os = "macos")]
use winit::platform::macos::{ActivationPolicy, EventLoopBuilderExtMacOS};

#[cfg(all(target_os = "linux", feature = "webview-popover"))]
use gtk::prelude::*;
#[cfg(all(target_os = "linux", feature = "webview-popover"))]
use wry::WebViewBuilderExtUnix;

use crate::gpu::GpuMetrics;
use crate::llama::metrics::LlamaMetrics;
use crate::state::AppState;
use crate::system::SystemMetrics;

#[cfg(all(feature = "webview-popover", not(target_os = "windows")))]
const POPOVER_WIDTH: f64 = 240.0;
#[cfg(all(feature = "webview-popover", not(target_os = "windows")))]
const POPOVER_INITIAL_HEIGHT: f64 = 220.0;
#[cfg(all(feature = "webview-popover", not(target_os = "windows")))]
const POPOVER_MIN_HEIGHT: f64 = 96.0;
#[cfg(all(feature = "webview-popover", not(target_os = "windows")))]
const POPOVER_MAX_HEIGHT: f64 = 520.0;

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

    Icon::from_rgba(rgba, size, size)
        .unwrap_or_else(|_| Icon::from_rgba(vec![0, 0, 0, 255], 1, 1).unwrap())
}

pub fn run_tray(state: AppState, port: u16) -> anyhow::Result<()> {
    #[cfg(not(any(
        all(feature = "webview-popover", not(target_os = "windows")),
        target_os = "windows"
    )))]
    let _ = port;

    #[cfg(target_os = "macos")]
    {
        let _ = mac_notification_sys::set_application("com.apple.Finder");
    }

    #[cfg(target_os = "macos")]
    let event_loop = EventLoop::builder()
        .with_activation_policy(ActivationPolicy::Accessory)
        .build()?;

    #[cfg(not(target_os = "macos"))]
    let event_loop = EventLoop::builder().build()?;

    #[cfg(all(target_os = "linux", feature = "webview-popover"))]
    let gtk_ready = match gtk::init() {
        Ok(()) => true,
        Err(e) => {
            eprintln!("[tray] GTK init failed; tray WebView popover disabled: {e}");
            false
        }
    };

    #[cfg(all(feature = "webview-popover", not(target_os = "windows")))]
    let (resize_tx, resize_rx) = mpsc::channel();
    let tray_start_failed = Arc::new(AtomicBool::new(false));

    let app: Box<dyn winit::application::ApplicationHandler + 'static> = Box::new(TrayApp {
        tray_state: TrayState {
            app_state: Arc::new(state),
        },
        tray: None,
        icon: create_tray_icon(),
        #[cfg(any(
            all(feature = "webview-popover", not(target_os = "windows")),
            target_os = "windows"
        ))]
        port,
        #[cfg(all(feature = "webview-popover", not(target_os = "windows")))]
        popover: None,
        #[cfg(all(feature = "webview-popover", not(target_os = "windows")))]
        resize_tx,
        #[cfg(all(feature = "webview-popover", not(target_os = "windows")))]
        resize_rx,
        tray_start_failed: Arc::clone(&tray_start_failed),
        #[cfg(all(target_os = "linux", feature = "webview-popover"))]
        gtk_ready,
        #[cfg(target_os = "windows")]
        windows_menu: None,
    });

    event_loop.run_app(app)?;

    if tray_start_failed.load(Ordering::Relaxed) {
        anyhow::bail!("failed to create tray icon");
    }

    Ok(())
}

struct TrayApp {
    tray_state: TrayState,
    tray: Option<TrayIcon>,
    icon: Icon,
    #[cfg(any(
        all(feature = "webview-popover", not(target_os = "windows")),
        target_os = "windows"
    ))]
    port: u16,
    #[cfg(all(feature = "webview-popover", not(target_os = "windows")))]
    popover: Option<Popover>,
    #[cfg(all(feature = "webview-popover", not(target_os = "windows")))]
    resize_tx: Sender<PopoverResize>,
    #[cfg(all(feature = "webview-popover", not(target_os = "windows")))]
    resize_rx: Receiver<PopoverResize>,
    tray_start_failed: Arc<AtomicBool>,
    #[cfg(all(target_os = "linux", feature = "webview-popover"))]
    gtk_ready: bool,
    #[cfg(target_os = "windows")]
    windows_menu: Option<WindowsTrayMenu>,
}

#[cfg(all(feature = "webview-popover", not(target_os = "windows")))]
struct Popover {
    #[cfg(not(target_os = "linux"))]
    window: std::sync::Arc<dyn winit::window::Window>,
    #[cfg(target_os = "linux")]
    window: gtk::Window,
    webview: wry::WebView,
    width: f64,
    height: f64,
}

#[cfg(all(feature = "webview-popover", not(target_os = "windows")))]
#[derive(serde::Deserialize)]
struct PopoverResize {
    width: f64,
    height: f64,
}

impl ApplicationHandler for TrayApp {
    fn can_create_surfaces(&mut self, _event_loop: &dyn ActiveEventLoop) {}

    fn resumed(&mut self, _event_loop: &dyn ActiveEventLoop) {}

    fn window_event(
        &mut self,
        _event_loop: &dyn ActiveEventLoop,
        _id: WindowId,
        _event: WindowEvent,
    ) {
    }

    fn new_events(&mut self, event_loop: &dyn ActiveEventLoop, _cause: winit::event::StartCause) {
        pump_gtk_events();

        if self.tray.is_none() {
            let initial_metrics = self.tray_state.get_metrics();
            let builder = TrayIconBuilder::new()
                .with_tooltip("Llama Monitor")
                .with_icon(std::mem::replace(
                    &mut self.icon,
                    Icon::from_rgba(vec![0, 0, 0, 255], 1, 1).unwrap(),
                ));

            #[cfg(target_os = "macos")]
            let builder = builder.with_icon_as_template(true);

            #[cfg(target_os = "windows")]
            let (builder, windows_menu) = {
                let windows_menu = WindowsTrayMenu::new(&initial_metrics);
                (
                    builder
                        .with_menu(Box::new(windows_menu.menu.clone()))
                        .with_menu_on_left_click(true)
                        .with_menu_on_right_click(true),
                    Some(windows_menu),
                )
            };

            match builder.build() {
                Ok(tray) => {
                    self.tray = Some(tray);
                    #[cfg(target_os = "windows")]
                    {
                        self.windows_menu = windows_menu;
                    }
                }
                Err(e) => {
                    eprintln!("[tray] Failed to create tray icon: {e}");
                    self.tray_start_failed.store(true, Ordering::Relaxed);
                    event_loop.exit();
                    return;
                }
            }

            if let Some(ref tooltip) = initial_metrics.3
                && let Some(ref tray) = self.tray
            {
                let _ = tray.set_tooltip(Some(tooltip));
            }

            event_loop.set_control_flow(ControlFlow::WaitUntil(
                Instant::now() + Duration::from_millis(500),
            ));
        }

        #[cfg(target_os = "windows")]
        while let Ok(menu_event) = MenuEvent::receiver().try_recv() {
            self.handle_windows_menu(menu_event, event_loop);
        }

        #[cfg(all(feature = "webview-popover", not(target_os = "windows")))]
        while let Ok(tray_event) = TrayIconEvent::receiver().try_recv() {
            match &tray_event {
                TrayIconEvent::Click {
                    button,
                    button_state,
                    position,
                    rect,
                    ..
                } if *button == tray_icon::MouseButton::Left
                    && *button_state == tray_icon::MouseButtonState::Down =>
                {
                    if self.popover.is_some() {
                        self.close_popover();
                    } else {
                        let rect = self.resolve_popover_anchor(event_loop, *rect, *position);
                        self.open_popover(event_loop, rect);
                    }
                }
                _ => {}
            }
        }
    }

    fn about_to_wait(&mut self, event_loop: &dyn ActiveEventLoop) {
        pump_gtk_events();
        self.refresh_tray_status();
        event_loop.set_control_flow(ControlFlow::WaitUntil(
            Instant::now() + Duration::from_millis(500),
        ));
    }

    fn proxy_wake_up(&mut self, _event_loop: &dyn ActiveEventLoop) {
        #[cfg(all(feature = "webview-popover", not(target_os = "windows")))]
        {
            let mut latest = None;
            while let Ok(resize) = self.resize_rx.try_recv() {
                latest = Some(resize);
            }

            if let Some(resize) = latest {
                self.resize_popover(resize.width, resize.height);
            }
        }
    }
}

impl TrayApp {
    fn refresh_tray_status(&mut self) {
        let metrics = self.tray_state.get_metrics();
        if let Some(ref tooltip) = metrics.3
            && let Some(ref tray) = self.tray
        {
            let _ = tray.set_tooltip(Some(tooltip));
        }

        #[cfg(target_os = "windows")]
        if let Some(menu) = self.windows_menu.as_ref() {
            menu.update(&metrics);
        }
    }

    #[cfg(target_os = "windows")]
    fn handle_windows_menu(&mut self, event: MenuEvent, event_loop: &dyn ActiveEventLoop) {
        let Some(menu) = self.windows_menu.as_ref() else {
            return;
        };

        if event.id() == menu.open_dashboard.id() {
            open_dashboard(self.port);
        } else if event.id() == menu.quit.id() {
            event_loop.exit();
        }
    }

    #[cfg(all(feature = "webview-popover", not(target_os = "windows")))]
    fn open_popover(&mut self, event_loop: &dyn ActiveEventLoop, icon_rect: tray_icon::Rect) {
        let pos = icon_rect.position;
        let width = POPOVER_WIDTH;
        let height = POPOVER_INITIAL_HEIGHT;
        let x = pos.x + (icon_rect.size.width as f64 / 2.0) - (width / 2.0);
        let y = pos.y + icon_rect.size.height as f64 + 4.0;

        let url = format!(
            "http://127.0.0.1:{}/compact?t={}",
            self.port,
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs()
        );
        let proxy = event_loop.create_proxy();
        let resize_tx = self.resize_tx.clone();

        #[cfg(target_os = "linux")]
        {
            if !self.gtk_ready {
                return;
            }

            let window = gtk::Window::new(gtk::WindowType::Popup);
            window.set_decorated(false);
            window.set_resizable(false);
            window.set_keep_above(true);
            window.set_default_size(width as i32, height as i32);
            window.move_(x as i32, y as i32);

            let fixed = gtk::Fixed::new();
            fixed.set_size_request(width as i32, height as i32);
            window.add(&fixed);

            let webview = match wry::WebViewBuilder::new()
                .with_ipc_handler(move |request| {
                    if let Ok(resize) = serde_json::from_str::<PopoverResize>(request.body()) {
                        let _ = resize_tx.send(resize);
                        proxy.wake_up();
                    }
                })
                .with_url(url)
                .with_bounds(wry::Rect {
                    position: wry::dpi::LogicalPosition::new(0.0, 0.0).into(),
                    size: wry::dpi::LogicalSize::new(width, height).into(),
                })
                .build_gtk(&fixed)
            {
                Ok(wv) => wv,
                Err(e) => {
                    eprintln!("[tray] Failed to create Linux tray WebView: {e}");
                    window.close();
                    return;
                }
            };

            window.show_all();
            self.popover = Some(Popover {
                window,
                webview,
                width,
                height,
            });
        }

        #[cfg(all(not(target_os = "linux"), not(target_os = "windows")))]
        let attrs = WindowAttributes::default()
            .with_surface_size(winit::dpi::LogicalSize::new(width, height))
            .with_position(PhysicalPosition::new(x as i32, y as i32))
            .with_decorations(false)
            .with_resizable(false)
            .with_window_level(WindowLevel::AlwaysOnTop)
            .with_visible(true);

        #[cfg(all(not(target_os = "linux"), not(target_os = "windows")))]
        let window: std::sync::Arc<dyn winit::window::Window> =
            match event_loop.create_window(attrs) {
                Ok(w) => std::sync::Arc::from(w),
                Err(_) => return,
            };

        #[cfg(all(not(target_os = "linux"), not(target_os = "windows")))]
        let webview = match wry::WebViewBuilder::new()
            .with_ipc_handler(move |request| {
                if let Ok(resize) = serde_json::from_str::<PopoverResize>(request.body()) {
                    let _ = resize_tx.send(resize);
                    proxy.wake_up();
                }
            })
            .with_url(url)
            .with_bounds(wry::Rect {
                position: wry::dpi::LogicalPosition::new(0.0, 0.0).into(),
                size: wry::dpi::LogicalSize::new(width, height).into(),
            })
            .build_as_child(&window)
        {
            Ok(wv) => wv,
            Err(_) => return,
        };

        #[cfg(all(not(target_os = "linux"), not(target_os = "windows")))]
        {
            self.popover = Some(Popover {
                window,
                webview,
                width,
                height,
            });
        }
    }

    #[cfg(all(feature = "webview-popover", not(target_os = "windows")))]
    fn close_popover(&mut self) {
        #[cfg(target_os = "linux")]
        if let Some(popover) = self.popover.take() {
            popover.window.close();
        }

        #[cfg(all(not(target_os = "linux"), not(target_os = "windows")))]
        {
            self.popover.take();
        }
    }

    #[cfg(all(feature = "webview-popover", not(target_os = "windows")))]
    fn resize_popover(&mut self, _reported_width: f64, height: f64) {
        let Some(popover) = self.popover.as_mut() else {
            return;
        };

        let width = POPOVER_WIDTH;
        let height = height.clamp(POPOVER_MIN_HEIGHT, POPOVER_MAX_HEIGHT);
        if (popover.width - width).abs() < 1.0 && (popover.height - height).abs() < 1.0 {
            return;
        }

        #[cfg(target_os = "linux")]
        {
            popover.window.resize(width as i32, height as i32);
            popover.window.set_default_size(width as i32, height as i32);
        }

        #[cfg(all(not(target_os = "linux"), not(target_os = "windows")))]
        let _ = popover
            .window
            .request_surface_size(winit::dpi::LogicalSize::new(width, height).into());

        let _ = popover.webview.set_bounds(wry::Rect {
            position: wry::dpi::LogicalPosition::new(0.0, 0.0).into(),
            size: wry::dpi::LogicalSize::new(width, height).into(),
        });
        popover.width = width;
        popover.height = height;
    }

    #[cfg(all(feature = "webview-popover", not(target_os = "windows")))]
    fn resolve_popover_anchor(
        &self,
        event_loop: &dyn ActiveEventLoop,
        event_rect: tray_icon::Rect,
        click_position: PhysicalPosition<f64>,
    ) -> tray_icon::Rect {
        if rect_has_position(event_rect) {
            return event_rect;
        }

        if let Some(ref tray) = self.tray
            && let Some(rect) = tray.rect()
            && rect_has_position(rect)
        {
            return rect;
        }

        if click_position.x > 0.0 || click_position.y > 0.0 {
            return tray_icon::Rect {
                position: PhysicalPosition::new(click_position.x - 11.0, click_position.y - 11.0),
                size: PhysicalSize::new(22, 22),
            };
        }

        if let Some(monitor) = event_loop
            .primary_monitor()
            .or_else(|| event_loop.available_monitors().next())
        {
            let monitor_pos = monitor
                .position()
                .unwrap_or_else(|| PhysicalPosition::new(0, 0));
            let monitor_size = monitor
                .current_video_mode()
                .map(|mode| mode.size())
                .unwrap_or_else(|| PhysicalSize::new(1024, 768));
            return tray_icon::Rect {
                position: PhysicalPosition::new(
                    monitor_pos.x as f64 + monitor_size.width as f64 - POPOVER_WIDTH - 16.0,
                    monitor_pos.y as f64 + 32.0,
                ),
                size: PhysicalSize::new(22, 22),
            };
        }

        event_rect
    }
}

#[cfg(all(feature = "webview-popover", not(target_os = "windows")))]
fn rect_has_position(rect: tray_icon::Rect) -> bool {
    rect.size.width > 0 || rect.size.height > 0 || rect.position.x > 0.0 || rect.position.y > 0.0
}

#[cfg(all(target_os = "linux", feature = "webview-popover"))]
fn pump_gtk_events() {
    while gtk::events_pending() {
        gtk::main_iteration_do(false);
    }
}

#[cfg(not(all(target_os = "linux", feature = "webview-popover")))]
fn pump_gtk_events() {}

#[cfg(target_os = "windows")]
struct WindowsTrayMenu {
    menu: Menu,
    endpoint: MenuItem,
    prompt: MenuItem,
    generation: MenuItem,
    requests: MenuItem,
    host: MenuItem,
    open_dashboard: MenuItem,
    quit: MenuItem,
}

#[cfg(target_os = "windows")]
impl WindowsTrayMenu {
    fn new(metrics: &TrayMetrics) -> Self {
        let menu = Menu::new();
        let endpoint = MenuItem::with_id("endpoint", "Endpoint: starting", false, None);
        let prompt = MenuItem::with_id("prompt", "Prompt: -- tok/s", false, None);
        let generation = MenuItem::with_id("generation", "Generation: -- tok/s", false, None);
        let requests = MenuItem::with_id("requests", "Inference: idle", false, None);
        let host = MenuItem::with_id("host", "Host: unavailable", false, None);
        let open_dashboard = MenuItem::with_id("open-dashboard", "Open Dashboard", true, None);
        let quit = MenuItem::with_id("quit", "Quit", true, None);

        let _ = menu.append_items(&[
            &endpoint,
            &prompt,
            &generation,
            &requests,
            &host,
            &PredefinedMenuItem::separator(),
            &open_dashboard,
            &PredefinedMenuItem::separator(),
            &quit,
        ]);

        let windows_menu = Self {
            menu,
            endpoint,
            prompt,
            generation,
            requests,
            host,
            open_dashboard,
            quit,
        };
        windows_menu.update(metrics);
        windows_menu
    }

    fn update(&self, metrics: &TrayMetrics) {
        let (sys, _gpu, llama, tooltip) = metrics;
        self.endpoint.set_text(
            tooltip
                .as_deref()
                .and_then(|text| text.lines().next())
                .unwrap_or("Endpoint: unknown"),
        );
        self.prompt.set_text(format!(
            "Prompt: {} tok/s",
            format_rate(llama.prompt_tokens_per_sec)
        ));
        self.generation.set_text(format!(
            "Generation: {} tok/s",
            format_rate(llama.generation_tokens_per_sec)
        ));

        let slot_text = if llama.slots_processing > 0 || llama.slots_idle > 0 {
            format!(
                "Inference: {} processing / {} idle",
                llama.slots_processing, llama.slots_idle
            )
        } else if llama.requests_processing > 0 {
            format!("Inference: {} request(s)", llama.requests_processing)
        } else {
            "Inference: idle".to_string()
        };
        self.requests.set_text(slot_text);

        if sys.cpu_load > 0 || sys.cpu_temp_available {
            let cpu = sys.cpu_load as f32 / 10.0;
            if sys.cpu_temp_available {
                self.host
                    .set_text(format!("Host: {cpu:.1}% CPU / {:.0}C", sys.cpu_temp));
            } else {
                self.host.set_text(format!("Host: {cpu:.1}% CPU"));
            }
        } else {
            self.host.set_text("Host: unavailable");
        }
    }
}

#[cfg(target_os = "windows")]
fn format_rate(value: f64) -> String {
    if value > 0.0 {
        format!("{value:.1}")
    } else {
        "--".to_string()
    }
}

#[cfg(target_os = "windows")]
fn open_dashboard(port: u16) {
    let url = format!("http://127.0.0.1:{port}");
    if let Err(e) = std::process::Command::new("cmd.exe")
        .args(["/C", "start", "", &url])
        .spawn()
    {
        eprintln!("[tray] Failed to open dashboard: {e}");
    }
}

struct TrayState {
    app_state: Arc<AppState>,
}

impl TrayState {
    fn get_metrics(&self) -> TrayMetrics {
        let local_metrics_available = self.app_state.host_metrics_available();
        let sys = self.app_state.system_metrics.lock().unwrap().clone();
        let gpu = if local_metrics_available {
            self.app_state.gpu_metrics.lock().unwrap().clone()
        } else {
            Default::default()
        };
        let llama = self.app_state.llama_metrics.lock().unwrap().clone();
        let gpu_entries = if gpu.is_empty() {
            None
        } else {
            Some(gpu.into_iter().collect())
        };
        let tooltip = self.build_tooltip(&sys, &gpu_entries, &llama, local_metrics_available);
        (sys, gpu_entries, llama, Some(tooltip))
    }

    fn build_tooltip(
        &self,
        sys: &SystemMetrics,
        gpu: &Option<Vec<(String, GpuMetrics)>>,
        llama: &LlamaMetrics,
        local_metrics_available: bool,
    ) -> String {
        let mut lines = Vec::new();

        let endpoint_kind = self.app_state.current_endpoint_kind();
        let session_mode = match self.app_state.current_session_kind() {
            crate::state::SessionKind::Spawn => "Spawn",
            crate::state::SessionKind::Attach => "Attach",
            crate::state::SessionKind::None => "",
        };

        let local_label = if endpoint_kind == crate::state::EndpointKind::Local {
            "Local"
        } else {
            "Remote"
        };

        lines.push(format!("{} - {}", local_label, session_mode));

        if local_metrics_available {
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
        } else {
            lines.push("Host metrics unavailable".to_string());
        }

        if llama.generation_tokens_per_sec > 0.0 {
            lines.push(format!("{:.0} tok/s", llama.generation_tokens_per_sec));
        }

        lines.join("\n")
    }
}
