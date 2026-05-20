# Windows Tray WebView Implementation Plan

**Date:** 2026-05-05
**Status:** Implemented (2026-05-19 on branch feature/windows-tray-webview-and-compat)
**Priority:** Medium

## Goal

Replace the Windows tray context menu with a WebView popover identical to the macOS/Linux experience, showing the compact dashboard UI with live metrics.

## Current State

### macOS/Linux (working)
- Left-click tray icon toggles a borderless winit window containing a wry WebView
- WebView loads `compact.html` from the local server
- Resize communication: JS `window.ipc.postMessage()` -> Rust `with_ipc_handler` -> `mpsc` channel -> `proxy_wake_up`
- Popover positioned below tray icon using `tray_icon::Rect`

### Windows (current)
- Left/right-click tray icon shows a native context menu with static text items
- Menu items updated every 500ms via `MenuItem::set_text()`
- No WebView, no rich UI, no GPU visual bars

## Why WebView2 Works on Windows

wry supports Windows via Microsoft WebView2 (Edge Chromium runtime, pre-installed on Windows 10+).

**Verified APIs (wry 0.55.1 docs.rs, tray-icon 0.23.1 docs.rs):**

- `build_as_child` -- confirmed cross-platform. Docs: "Windows: This will create the webview as a child window of the `parent` window."
- `with_ipc_handler` -- docs list platform-specific notes for Linux/Android only, **no Windows exclusion**. May work out of the box on Windows via wry's internal WebView2 message bridging.
- `with_initialization_script` -- available for injecting JS before `window.onload`, useful for IPC polyfill if needed.
- `TrayIconEvent::Click` -- documented as cross-platform by tray-icon crate.
- `TrayIconEvent::set_event_handler` -- recommended pattern for winit integration (forwards events to `EventLoopProxy`).

| Platform | IPC Mechanism | wry API | Status |
|----------|--------------|---------|--------|
| macOS | `window.ipc.postMessage()` | `with_ipc_handler()` | Confirmed working |
| Linux (GTK) | `window.ipc.postMessage()` | `with_ipc_handler()` | Confirmed working |
| Windows (WebView2) | `window.ipc` (if wry bridges) or `window.chrome.webview` | `with_ipc_handler()` + fallback | **Verify at implementation time** |

## Implementation Checklist

### Phase 1: Cargo.toml

- [ ] **1.1.** Add `wry` to Windows dependencies (currently excluded by `not(target_os = "windows")`):

  ```toml
  # Current:
  [target.'cfg(not(target_os = "windows"))'.dependencies]
      wry = { version = "0.55", default-features = false, features = ["os-webview"], optional = true }

  # Change to:
  [dependencies]
      wry = { version = "0.55", default-features = false, features = ["os-webview"], optional = true }
  ```

  Remove the platform-specific target block for wry. The `os-webview` feature enables WebView2 on Windows automatically.

- [ ] **1.2.** Verify `winit` features include Windows support. Current config:
  ```toml
  winit = { version = "0.31.0-beta.2", default-features = false, features = ["x11", "wayland"], optional = true }
  ```
  Windows uses default winit (no X11/Wayland), so no feature changes needed -- winit's default features include Windows support.

### Phase 2: Remove `not(target_os = "windows")` guards from tray.rs

The following `#[cfg(...)]` attributes currently exclude Windows from the webview popover path. All of these need to be updated to include Windows:

- [ ] **2.1.** Imports (lines 3-25):

  Change `#[cfg(all(feature = "webview-popover", not(target_os = "windows")))]` to `#[cfg(feature = "webview-popover")]` for:
  - `std::sync::mpsc::{self, Receiver, Sender}`
  - `tray_icon::TrayIconEvent`
  - `winit::dpi::PhysicalPosition`
  - `winit::dpi::PhysicalSize`

  Change `#[cfg(all(not(target_os = "linux"), not(target_os = "windows"), feature = "webview-popover"))]` to `#[cfg(all(not(target_os = "linux"), feature = "webview-popover"))]` for:
  - `winit::window::{WindowAttributes, WindowLevel}`

- [ ] **2.2.** Constants (lines 40-47):

  Change `#[cfg(all(feature = "webview-popover", not(target_os = "windows")))]` to `#[cfg(feature = "webview-popover")]` for:
  - `POPOVER_WIDTH`, `POPOVER_INITIAL_HEIGHT`, `POPOVER_MIN_HEIGHT`, `POPOVER_MAX_HEIGHT`

- [ ] **2.3.** `TrayApp` struct fields (lines 157-177):

  Change all `#[cfg(all(feature = "webview-popover", not(target_os = "windows")))]` to `#[cfg(feature = "webview-popover")]` for:
  - `popover: Option<Popover>`
  - `resize_tx: Sender<PopoverResize>`
  - `resize_rx: Receiver<PopoverResize>`

  Remove `#[cfg(target_os = "windows")]` from `windows_menu: Option<WindowsTrayMenu>` (will be deleted in Phase 4).

- [ ] **2.4.** `Popover` struct (lines 179-188):

  Change `#[cfg(all(feature = "webview-popover", not(target_os = "windows")))]` to `#[cfg(feature = "webview-popover")]`.

  Inside the struct, the macOS window field currently has:
  ```rust
  #[cfg(all(not(target_os = "linux"), not(target_os = "windows")))]
  window: std::sync::Arc<dyn winit::window::Window>,
  ```
  Change to:
  ```rust
  #[cfg(all(not(target_os = "linux")))]
  window: std::sync::Arc<dyn winit::window::Window>,
  ```

- [ ] **2.5.** `PopoverResize` struct (lines 190-195):

  Change guard to `#[cfg(feature = "webview-popover")]`.

### Phase 3: Tray builder -- remove Windows context menu

- [ ] **3.1.** In `new_events` tray builder (lines 225-235), remove the Windows-specific block:

  ```rust
  // DELETE THIS ENTIRE BLOCK:
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
  ```

- [ ] **3.2.** Replace with a unified approach for all platforms. After the macOS `with_icon_as_template` block, add:

  ```rust
  // All platforms: enable tooltip, no menu needed (popover on click)
  ```

  The builder should just be `builder` (already has tooltip + icon) on all platforms.

- [ ] **3.3.** Remove `self.windows_menu = windows_menu;` from the `builder.build()` success path (line 241-243).

- [ ] **3.4.** Remove the Windows menu event loop from `new_events` (lines 264-267):

  ```rust
  // DELETE:
  #[cfg(target_os = "windows")]
  while let Ok(menu_event) = MenuEvent::receiver().try_recv() {
      self.handle_windows_menu(menu_event, event_loop);
  }
  ```

- [ ] **3.5.** Update the `TrayIconEvent` click handler (lines 269-290) to include Windows:

  Change `#[cfg(all(feature = "webview-popover", not(target_os = "windows")))]` to `#[cfg(feature = "webview-popover")]`.

  NOTE: tray-icon 0.23.1 docs confirm `TrayIconEvent` is cross-platform. The docs also recommend using `TrayIconEvent::set_event_handler` with `EventLoopProxy` for winit integration to ensure the event loop wakes on tray events. The current code uses `TrayIconEvent::receiver().try_recv()` which should work, but if events are missed, consider migrating to the `set_event_handler` pattern shown in the tray-icon docs.

### Phase 4: Remove old Windows context menu code

- [ ] **4.1.** Delete `WindowsTrayMenu` struct and impl (lines 565-656)
- [ ] **4.2.** Delete `format_rate` function (lines 558-564)
- [ ] **4.3.** Delete `open_dashboard` function (lines 566-575)
- [ ] **4.4.** Delete `handle_windows_menu` method from `TrayApp` (lines 331-342)
- [ ] **4.5.** Remove `#[cfg(target_os = "windows")]` imports for `Menu, MenuEvent, MenuItem, PredefinedMenuItem` (line 9-10)
- [ ] **4.6.** Remove `windows_menu: Option<WindowsTrayMenu>` from `TrayApp` struct (line 175-176)
- [ ] **4.7.** Remove `menu.update(&metrics)` from `refresh_tray_status` (lines 325-328)

### Phase 5: Add Windows WebView popover creation

The macOS popover creation code at lines 411-454 creates a winit window + wry WebView using `build_as_child`. Windows needs the same pattern.

- [ ] **5.1.** The existing `#[cfg(all(not(target_os = "linux"), not(target_os = "windows")))]` blocks in `open_popover` (lines 411-454) already contain the correct logic. Change the guard to `#[cfg(all(not(target_os = "linux")))]` to include Windows:

  ```rust
  // Before:
  #[cfg(all(not(target_os = "linux"), not(target_os = "windows")))]
  let attrs = WindowAttributes::default() ...

  // After:
  #[cfg(all(not(target_os = "linux")))]
  let attrs = WindowAttributes::default() ...
  ```

  This applies to all three blocks in `open_popover`: window attributes creation, window creation, and webview creation + popover assignment.

- [ ] **5.2.** Similarly update `close_popover` (lines 464-467):

  Change `#[cfg(all(not(target_os = "linux"), not(target_os = "windows")))]` to `#[cfg(all(not(target_os = "linux")))]`.

- [ ] **5.3.** Update `resize_popover` (lines 488-491):

  Change `#[cfg(all(not(target_os = "linux"), not(target_os = "windows")))]` to `#[cfg(all(not(target_os = "linux")))]`.

### Phase 6: Windows IPC bridge (verify first, polyfill if needed)

**Start here.** On macOS/Linux, wry exposes `window.ipc.postMessage()` automatically. The wry 0.55.1 docs for `with_ipc_handler` do NOT list Windows as a platform-specific limitation (only Linux/Android are mentioned). This suggests wry may already bridge `window.ipc` on Windows via WebView2's message API.

- [ ] **6.1.** **First, test if `with_ipc_handler` works on Windows as-is.** The existing macOS webview builder code uses:

   ```rust
   .with_ipc_handler(move |request| {
       if let Ok(resize) = serde_json::from_str::<PopoverResize>(request.body()) {
           let _ = resize_tx.send(resize);
           proxy.wake_up();
       }
   })
   ```

   If `request.body()` receives the resize messages from `window.ipc.postMessage()` on Windows, **no additional work is needed** for this phase.

- [ ] **6.2.** **If `with_ipc_handler` does NOT work on Windows**, add an IPC polyfill using `with_initialization_script` (confirmed available in wry 0.55.1):

   ```rust
   #[cfg(target_os = "windows")]
   let builder = builder.with_initialization_script(
       "
       if (!window.ipc) {
         window.ipc = {
           postMessage: function(msg) {
             if (window.chrome && window.chrome.webview) {
               window.chrome.webview.postMessage(msg);
             }
           }
         };
       }
       "
   );
   ```

   And add a complementary message handler. The exact wry API for receiving WebView2 `postMessage` may be `with_ipc_handler` (if it picks up chrome.webview messages) or a Windows-specific handler. Check wry 0.55 source if needed.

- [ ] **6.3.** **Alternative approach if polyfill is messy:** Instead of bridging `window.ipc`, use `with_initialization_script` to inject a `window.ipc` polyfill that calls a Rust-registered handler, and use wry's Windows message API to receive those messages. The agent should experiment on a Windows build environment to determine the cleanest approach.

### Phase 7: Update `refresh_tray_status` and `proxy_wake_up`

- [ ] **7.1.** The `refresh_tray_status` method (lines 317-329) currently has a Windows-specific `menu.update()` call that will be deleted. No additional changes needed -- the tooltip update path is already cross-platform.

- [ ] **7.2.** The `proxy_wake_up` method (lines 301-313) currently has:
  ```rust
  #[cfg(all(feature = "webview-popover", not(target_os = "windows")))]
  ```
  Change to `#[cfg(feature = "webview-popover")]` to include Windows.

### Phase 8: Update helper functions

- [ ] **8.1.** `resolve_popover_anchor` (lines 501-547):

  Change `#[cfg(all(feature = "webview-popover", not(target_os = "windows")))]` to `#[cfg(feature = "webview-popover")]`.

- [ ] **8.2.** `rect_has_position` (lines 550-553):

  Change `#[cfg(all(feature = "webview-popover", not(target_os = "windows")))]` to `#[cfg(feature = "webview-popover")]`.

### Phase 9: Update `run_tray` function

- [ ] **9.1.** The `#[cfg(not(any(...)))]` dead-code guard for `port` (lines 92-96) -- update to remove Windows from the exclusion since Windows now uses `port` for the WebView URL.

- [ ] **9.2.** The `mpsc` channel creation (lines 120-121):

  Change `#[cfg(all(feature = "webview-popover", not(target_os = "windows")))]` to `#[cfg(feature = "webview-popover")]`.

- [ ] **9.3.** `TrayApp` initialization (lines 124-146): Update all `#[cfg(...)]` guards for `popover`, `resize_tx`, `resize_rx` to include Windows (same as Phase 2.3).

- [ ] **9.4.** Remove `#[cfg(target_os = "windows")] windows_menu: None,` from initialization.

### Phase 10: Update `should_start_tray` (main.rs)

No changes needed -- the `should_start_tray` function already returns `true` for non-Linux platforms when not headless.

## Files Modified

| File | Changes |
|------|---------|
| `Cargo.toml` | Move `wry` from Linux/macOS-only to universal dependency |
| `src/tray.rs` | ~20 `#[cfg]` guard updates, delete WindowsTrayMenu, unify popover path |
| `src/main.rs` | No changes needed |
| `static/compact.html` | No changes needed (IPC polyfill injected at runtime) |

## Testing Strategy

1. **Cross-compile check:** `cargo check --target x86_64-pc-windows-msvc` (can be done on macOS with `rustup target add x86_64-pc-windows-msvc`)
2. **Full build:** `cargo build --release` on a Windows machine
3. **IPC test (critical):** Before implementing any polyfill, test whether `with_ipc_handler` receives messages from `window.ipc.postMessage()` on Windows. Add a debug log in the handler to verify.
4. **Runtime test:** Verify tray icon click toggles WebView popover
5. **Resize test:** Verify popover resizes when content changes (GPU sections appearing/disappearing)
6. **Close test:** Verify clicking tray icon again closes the popover
7. **Multi-monitor:** Verify popover positions correctly on non-primary monitors
8. **WebView2 availability test:** Verify behavior on a system without WebView2 (should fail gracefully)

## Risks and Unknowns

1. **wry `build_as_child` on Windows:** Verified in wry 0.55.1 docs as supported: "Windows: This will create the webview as a child window of the `parent` window." Low risk.

2. **WebView2 runtime availability:** Windows 10+ includes WebView2 by default. Older systems may need the runtime installed. Consider adding a graceful fallback (revert to context menu) if WebView2 is unavailable.

3. **`TrayIconEvent::Click` on Windows:** tray-icon 0.23.1 docs confirm cross-platform support. Low risk, but consider using `TrayIconEvent::set_event_handler` pattern if events are dropped.

4. **wry IPC on Windows:** `with_ipc_handler` docs don't exclude Windows, suggesting it may work. **This is the primary unknown.** Test first before implementing polyfill. If `with_ipc_handler` doesn't deliver messages on Windows, the fallback is `with_initialization_script` + WebView2 `window.chrome.webview.postMessage` bridge.

5. **Window z-order on Windows:** The `WindowLevel::AlwaysOnTop` setting may behave differently on Windows. The popover should appear above other windows but below the active window's menus.

6. **winit 0.31.0-beta.2:** This is a beta version. Verify that window creation with `WindowAttributes` works correctly on Windows, particularly `with_decorations(false)` and `with_window_level(WindowLevel::AlwaysOnTop)`.

## Rollback Plan

If implementation hits blockers, the original context menu code can be restored from git history. The `#[cfg(target_os = "windows")]` blocks are self-contained and can be re-added without affecting macOS/Linux.
