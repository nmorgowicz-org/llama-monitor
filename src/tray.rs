use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::{Duration, Instant};

use tray_icon::{Icon, TrayIcon, TrayIconBuilder, TrayIconEvent};
use winit::application::ApplicationHandler;
use winit::dpi::{PhysicalPosition, PhysicalSize};
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::WindowId;
#[cfg(not(target_os = "linux"))]
use winit::window::{WindowAttributes, WindowLevel};

#[cfg(target_os = "macos")]
use winit::platform::macos::{ActivationPolicy, EventLoopBuilderExtMacOS};

#[cfg(target_os = "linux")]
use gtk::prelude::*;
#[cfg(target_os = "linux")]
use wry::WebViewBuilderExtUnix;

use crate::gpu::GpuMetrics;
use crate::llama::metrics::LlamaMetrics;
use crate::state::AppState;
use crate::system::SystemMetrics;

const POPOVER_WIDTH: f64 = 240.0;
const POPOVER_INITIAL_HEIGHT: f64 = 220.0;
const POPOVER_MIN_HEIGHT: f64 = 96.0;
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

    #[cfg(target_os = "linux")]
    let gtk_ready = match gtk::init() {
        Ok(()) => true,
        Err(e) => {
            eprintln!("[tray] GTK init failed; tray WebView popover disabled: {e}");
            false
        }
    };

    let (resize_tx, resize_rx) = mpsc::channel();
    let tray_start_failed = Arc::new(AtomicBool::new(false));

    let app: Box<dyn winit::application::ApplicationHandler + 'static> = Box::new(TrayApp {
        tray_state: TrayState {
            app_state: Arc::new(state),
        },
        tray: None,
        icon: create_tray_icon(),
        port,
        popover: None,
        resize_tx,
        resize_rx,
        tray_start_failed: Arc::clone(&tray_start_failed),
        #[cfg(target_os = "linux")]
        gtk_ready,
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
    port: u16,
    popover: Option<Popover>,
    resize_tx: Sender<PopoverResize>,
    resize_rx: Receiver<PopoverResize>,
    tray_start_failed: Arc<AtomicBool>,
    #[cfg(target_os = "linux")]
    gtk_ready: bool,
}

struct Popover {
    #[cfg(not(target_os = "linux"))]
    window: std::sync::Arc<dyn winit::window::Window>,
    #[cfg(target_os = "linux")]
    window: gtk::Window,
    webview: wry::WebView,
    width: f64,
    height: f64,
}

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
            let builder = TrayIconBuilder::new()
                .with_tooltip("Llama Monitor")
                .with_icon(std::mem::replace(
                    &mut self.icon,
                    Icon::from_rgba(vec![0, 0, 0, 255], 1, 1).unwrap(),
                ));

            #[cfg(target_os = "macos")]
            let builder = builder.with_icon_as_template(true);

            match builder.build() {
                Ok(tray) => {
                    self.tray = Some(tray);
                }
                Err(e) => {
                    eprintln!("[tray] Failed to create tray icon: {e}");
                    self.tray_start_failed.store(true, Ordering::Relaxed);
                    event_loop.exit();
                    return;
                }
            }

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
        event_loop.set_control_flow(ControlFlow::WaitUntil(
            Instant::now() + Duration::from_millis(500),
        ));
    }

    fn proxy_wake_up(&mut self, _event_loop: &dyn ActiveEventLoop) {
        let mut latest = None;
        while let Ok(resize) = self.resize_rx.try_recv() {
            latest = Some(resize);
        }

        if let Some(resize) = latest {
            self.resize_popover(resize.width, resize.height);
        }
    }
}

impl TrayApp {
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

        #[cfg(not(target_os = "linux"))]
        let attrs = WindowAttributes::default()
            .with_surface_size(winit::dpi::LogicalSize::new(width, height))
            .with_position(PhysicalPosition::new(x as i32, y as i32))
            .with_decorations(false)
            .with_resizable(false)
            .with_window_level(WindowLevel::AlwaysOnTop)
            .with_visible(true);

        #[cfg(not(target_os = "linux"))]
        let window: std::sync::Arc<dyn winit::window::Window> =
            match event_loop.create_window(attrs) {
                Ok(w) => std::sync::Arc::from(w),
                Err(_) => return,
            };

        #[cfg(not(target_os = "linux"))]
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

        #[cfg(not(target_os = "linux"))]
        {
            self.popover = Some(Popover {
                window,
                webview,
                width,
                height,
            });
        }
    }

    fn close_popover(&mut self) {
        #[cfg(target_os = "linux")]
        if let Some(popover) = self.popover.take() {
            popover.window.close();
        }

        #[cfg(not(target_os = "linux"))]
        {
            self.popover.take();
        }
    }

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

        #[cfg(not(target_os = "linux"))]
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

fn rect_has_position(rect: tray_icon::Rect) -> bool {
    rect.size.width > 0 || rect.size.height > 0 || rect.position.x > 0.0 || rect.position.y > 0.0
}

#[cfg(target_os = "linux")]
fn pump_gtk_events() {
    while gtk::events_pending() {
        gtk::main_iteration_do(false);
    }
}

#[cfg(not(target_os = "linux"))]
fn pump_gtk_events() {}

struct TrayState {
    app_state: Arc<AppState>,
}

impl TrayState {
    fn get_metrics(&self) -> TrayMetrics {
        let local_metrics_available = self.app_state.active_session_uses_local_metrics();
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
        }

        if llama.generation_tokens_per_sec > 0.0 {
            lines.push(format!("{:.0} tok/s", llama.generation_tokens_per_sec));
        }

        lines.join("\n")
    }
}
