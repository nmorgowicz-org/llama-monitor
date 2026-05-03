# RUSTSEC-2024-0429: glib VariantStrIter Unsoundness

**Date:** 2026-04-23  
**Advisory:** [RUSTSEC-2024-0429](https://rustsec.org/advisories/RUSTSEC-2024-0429.html) / GHSA-wrw7-89jp-8q8g  
**Severity:** Medium  
**Affected versions:** `glib >= 0.15.0, < 0.20.0`  
**Fixed in:** `glib 0.20.0`

---

## What the vulnerability is

The `VariantStrIter::impl_get` function (called internally by `Iterator` and `DoubleEndedIterator` for `glib::VariantStrIter`) was unsound. An immutable reference `&p` to a `*mut c_char` was passed as an out-argument to a C function that mutates it in place. After changes in recent Rust compiler versions, these unsound writes are silently ignored under optimization, causing all calls to `VariantStrIter::impl_get` to pass a NULL pointer to `CStr::from_ptr` — resulting in NULL dereference crashes.

**Practical exposure in this project:** We do not use `VariantStrIter` anywhere in the codebase. The vulnerable code is present in the binary (via the `gtk` → `glib` dependency on Linux) but is never called. Risk is low, but the advisory cannot be resolved without an upstream fix.

---

## Why dependabot can't fix it automatically

Dependabot's security update run on 2026-04-23 (Actions run [#24857032590](https://github.com/nmorgowicz-org/llama-monitor/actions/runs/24857032590)) failed with:

```
security_update_not_possible
  "dependency-name": "glib"
  "latest-resolvable-version": "0.18.5"
  "lowest-non-vulnerable-version": "0.20.0"
  "conflicting-dependencies": []
```

The dependency chain on Linux is:

```
llama-monitor
├── gtk = "0.18"  (direct, Linux only)  ──┐ both require glib ^0.18
└── wry = "0.55"  (non-Windows)           │
    └── gtk "^0.18"  (Linux)  ────────────┘
                └── glib 0.18.5   ← vulnerable, max resolvable version
```

The `gtk` crate (GTK3 Rust bindings) is **officially unmaintained** as of its 0.18.2 release. There is no gtk 0.19 or 0.20 in the GTK3 ecosystem — the gtk-rs project moved on to `gtk4`. Since `glib 0.20` is only available through the GTK4 ecosystem, and both our direct `gtk = "0.18"` dependency and `wry 0.55`'s Linux backend hard-require `glib ^0.18`, there is no valid resolution to `glib >= 0.20.0` within the current dependency graph.

---

## Current mitigation

The advisory is suppressed in `.cargo/audit.toml` with a note that it is blocked on upstream:

```toml
[advisories]
ignore = ["RUSTSEC-2024-0429"]
# Blocked on wry gtk4 / webkit2gtk-6.0 migration. We do not use VariantStrIter.
# Track: https://github.com/tauri-apps/wry/issues (gtk4/webkit2gtk4 support)
```

---

## What a real fix requires

Resolving this vulnerability requires migrating the Linux webview/window backend from GTK3 to GTK4. This means replacing:

| Current | Replacement |
|---|---|
| `gtk = "0.18"` (GTK3, unmaintained) | `gtk4 = "0.9+"` |
| `webkit2gtk = "2.0"` (GTK3) | `webkit6 = "0.4+"` (GTK4) |
| `wry = "0.55"` (uses GTK3 on Linux) | see options below |

The system library change on the user's machine is: `libwebkit2gtk-4.1-dev` → `libwebkit2gtk-6.0-dev`.

---

## Migration options

### Option 1: Wait for wry to support GTK4 (no code changes)

**Status:** Not yet available. `wry 0.55` (latest as of 2026-04-23) still uses `gtk ^0.18` on Linux.

The Tauri team has a known blocker: `libappindicator-rs` only supports GTK3. There is a tracked effort to replace it with the `ksni` crate (KStatusNotifierItem via D-Bus), which would unblock GTK4 adoption. This is expected to land in a future Tauri v3 release — no published timeline.

**Action:** Watch [tauri-apps/wry](https://github.com/tauri-apps/wry) and [tauri-apps/tauri#11293](https://github.com/tauri-apps/tauri/issues/11293) for progress.

When wry ships GTK4 support, the fix will be: bump `wry` version, remove direct `gtk = "0.18"` dep, update the `src/tray.rs` gtk4 API usage.

---

### Option 2: Replace wry with `webview_app`

[`webview_app`](https://crates.io/crates/webview_app) is a cross-platform webview crate that already uses **gtk4 + webkit6** on Linux and **WebView2** on Windows.

```toml
# Cargo.toml
[target.'cfg(not(target_os = "windows"))'.dependencies]
webview_app = "1"   # uses gtk4 + webkit6 on Linux, webkit on macOS
```

**Pros:**
- Already on GTK4 — resolves the vulnerability today
- Single crate handles Linux and Windows webview
- Simpler API than wry for basic embedded webview use cases

**Cons:**
- Less ecosystem adoption than wry; smaller community
- API is different from wry — requires rewriting `src/tray.rs` webview integration
- May lack some wry features (custom URI schemes, IPC, devtools flags)
- macOS backend is less mature than wry's

**Effort estimate:** Medium. The GTK window creation and webview embedding in `src/tray.rs` (lines ~369–560) would need to be rewritten against the `webview_app` builder API. The popover positioning logic (attaching to tray icon coordinates) may need extra work since `webview_app` is designed for standalone windows, not popover-style panels.

---

### Option 3: Use `webkit6` + `gtk4` directly (no wry)

Use GNOME's official Rust bindings directly:

```toml
[target.'cfg(target_os = "linux")'.dependencies]
gtk4 = "0.9"
webkit6 = "0.4"   # webkit2gtk-6.0 bindings, GTK4-native
```

System library requirement: `libwebkit2gtk-6.0-dev`

**Pros:**
- Full control over the GTK4 window and webview
- Official GNOME-maintained crates with stable APIs
- Resolves the vulnerability with glib 0.20+
- Can integrate tightly with the existing GTK4 window management

**Cons:**
- Highest implementation effort — no abstraction layer, raw GTK4 + WebKit API
- GTK4's windowing model differs significantly from GTK3 (no `WindowType::Popup`, different overlay/popover patterns)
- Would need a separate macOS/Windows webview path (currently handled by wry)
- The popover-over-tray-icon UX pattern requires `GtkPopover` or a custom `GtkWindow` with `set_decorated(false)` + manual positioning, which is more involved in GTK4

**Effort estimate:** High. This is essentially writing a custom wry replacement for the Linux backend. Only recommended if deep GTK4 customization is needed (e.g., Libadwaita styling).

---

## Recommendation

1. **Now:** Keep the `RUSTSEC-2024-0429` suppression in place. We don't call `VariantStrIter`, so there is no practical exploit path.

2. **Short-term:** Monitor [tauri-apps/wry](https://github.com/tauri-apps/wry) for GTK4 support. If wry ships it, upgrade wry and drop the direct `gtk` dep — this will be the lowest-effort path.

3. **If wry GTK4 support stalls past mid-2026:** Evaluate `webview_app` (Option 2) as the migration target. It is the most practical alternative that already resolves the vulnerability.

4. **Do not pursue Option 3** unless there is a specific need for deep GTK4/Libadwaita integration, as the effort-to-benefit ratio is poor relative to the low actual risk.
