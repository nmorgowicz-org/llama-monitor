# App Update Capability — 2026-04-29

## Architecture Context

**llama-monitor** is a standalone Rust binary with embedded HTML/CSS/JS frontend (via `include_str!` in `src/web/static_assets.rs`). It is distributed as raw binaries on GitHub Releases for four platforms: Linux x86_64, Linux ARM64, Windows x86_64, macOS ARM64.

**There is no Tauri/Electron wrapper.** The app uses `tray-icon` + `wry` directly for native tray/webview.

**No `self_update` crate was added.** The implementation reuses the existing download/extract infrastructure in `pub mod install` inside `src/agent.rs`, which already handles GitHub asset fetching, tar.gz extraction, and temp file management for remote agent installs.

---

## What Was Built

### 1. Version Display

**Location:** Bottom-left of sidebar nav, below the collapse/expand button.

**Template injection** in `src/web/mod.rs`:
```rust
let html = static_assets::INDEX_HTML
    .replace("{{ VERSION }}", env!("CARGO_PKG_VERSION"))
    .replace("{{ PLATFORM }}", std::env::consts::OS);
```

**JS constants** injected in `static/index.html` `<head>`:
```html
<script>const APP_VERSION = '{{ VERSION }}'; const APP_PLATFORM = '{{ PLATFORM }}';</script>
```

`APP_PLATFORM` is one of `"linux"`, `"macos"`, `"windows"` — used by the frontend to choose the correct update CTA without any runtime API call.

`initAppVersion()` in `static/app.js` writes `v${APP_VERSION}` to `#app-version` (`.nav-version` span in the sidebar).

---

### 2. Update Check

**Trigger:** `DOMContentLoaded` via `checkForUpdate()` in `static/app.js`.

**Endpoint reused:** `GET /api/remote-agent/releases/latest` — the existing endpoint that proxies GitHub's releases API and returns `{ ok, release: { tag_name, html_url, body, published_at, assets } }`. No new Rust endpoint was needed.

```js
async function checkForUpdate() {
    const resp = await fetch('/api/remote-agent/releases/latest');
    const data = await resp.json();
    const latest = data.release || data;
    if (compareVersions(latest.tag_name, APP_VERSION) > 0) {
        showUpdatePill(latest);
    }
}
```

**`compareVersions(a, b)`** — parses semver strings (strips `v` prefix), returns `-1/0/1`. Variables in the comparison loop are named `av`/`bv`/`x`/`y` to avoid parameter shadowing.

**`_pendingRelease`** — module-level variable holds the release object. Earlier design stored it as `pill.dataset.release = JSON.stringify(...)` which is fragile with large release note bodies; replaced with a plain JS variable.

---

### 3. Update Pill

**Location:** Top nav bar, between Settings button and User menu.

```html
<button id="update-pill" class="top-nav-pill" style="display:none;" onclick="openReleaseNotes()">
    ...
    <span id="update-pill-text"></span>
</button>
```

**`showUpdatePill(release)`** checks `localStorage['update-dismissed']` — if the version was dismissed within the last 24 hours, the pill stays hidden. Otherwise sets `_pendingRelease`, populates `#update-pill-text` with `"v0.11.0 available"`, and shows the pill.

**CSS** in `static/css/layout.css`: `.top-nav-pill` — indigo-tinted pill, `height: 24px`, `font-size: 11px`.

---

### 4. Release Notes Panel

**Appearance:** Slide-out from right (`width: 420px`), `transform: translateX(100%)` → `.open` triggers `translateX(0)` with `cubic-bezier(0.16, 1, 0.3, 1)` transition.

**HTML structure** in `static/index.html`:
```html
<div id="release-notes-overlay" class="modal-overlay" onclick="closeReleaseNotes()"></div>
<div id="release-notes-panel" class="slide-panel">
    <div class="slide-panel-header">
        <div class="slide-panel-title-group">
            <h3 id="release-notes-title"></h3>
            <span id="release-notes-version-from" class="slide-panel-version-from"></span>
        </div>
        <button class="modal-close" onclick="closeReleaseNotes()">&times;</button>
    </div>
    <div class="slide-panel-body" id="release-notes-body"></div>
    <div class="slide-panel-footer">
        <a id="release-notes-link" href="#" target="_blank" rel="noopener">Open on GitHub ↗</a>
        <div class="slide-panel-footer-actions">
            <button id="release-notes-update-btn" class="btn-sm btn-update-action" onclick="triggerSelfUpdate()"></button>
            <button class="btn-sm btn-preset" onclick="dismissUpdate()">Later</button>
        </div>
    </div>
</div>
```

**`openReleaseNotes()`** populates from `_pendingRelease`:
- `#release-notes-title` ← `release.tag_name` (e.g. `v0.11.0`)
- `#release-notes-version-from` ← `"from v${APP_VERSION}"` (e.g. `from v0.10.2`)
- `#release-notes-body` ← `renderMd(release.body)` using the existing Markdown renderer
- Calls `_resetUpdateBtn()` to set the correct platform-specific label

**`_resetUpdateBtn(btn)`** — sets button label based on `APP_PLATFORM`:
- `"windows"` → "Download for Windows" (download icon)
- Everything else → "Update & Restart" (upload/cloud icon)

**`closeReleaseNotes()`** — removes `.open` class, hides panel/overlay after 300ms transition.

**`dismissUpdate()`** — writes `{ [tag_name]: Date.now() }` to `localStorage['update-dismissed']`, hides pill, closes panel.

---

### 5. Self-Update Backend

**Location:** `pub async fn self_update_binary()` inside `pub mod install` in `src/agent.rs`.

**No new crate dependency.** Uses `download_asset_locally()` and `extract_archive_with_timeout()` already present in the same module.

#### macOS / Linux flow

1. `latest_release_info()` fetches the release and calls `matching_asset(os, arch)` to find the right asset:
   - Linux x86_64: `llama-monitor-linux-x86_64` (bare binary, no archive)
   - Linux aarch64: `llama-monitor-linux-aarch64` (bare binary)
   - macOS aarch64: `llama-monitor-macos-aarch64.tar.gz` (tar.gz, extracted to `llama-monitor-macos-aarch64`)
2. Downloads to a temp file; extracts if `asset.archive == true`
3. Copies the binary into the **same directory** as the running executable — this keeps the rename on one filesystem, avoiding `EXDEV` cross-device errors
4. Sets `0o755` permissions via `std::os::unix::fs::PermissionsExt`
5. `std::fs::rename(staged_path, current_exe)` — atomic on Unix even while the process is running; the OS keeps the old inode mapped in memory

```rust
pub async fn self_update_binary() -> Result<SelfUpdateResult> {
    let os = std::env::consts::OS;
    let arch = std::env::consts::ARCH;
    let release = crate::agent::latest_release_info().await?;

    if os == "windows" {
        // In-place replacement of a running .exe is blocked by Windows.
        // Return the download URL so the frontend can open it directly.
        let download_url = release.matching_asset("windows", arch).map(|a| a.url.clone());
        return Ok(SelfUpdateResult { tag_name: release.tag_name, windows: true, download_url });
    }

    let asset = release.matching_asset(os, arch)
        .ok_or_else(|| anyhow::anyhow!("No release asset for {os}/{arch}"))?
        .clone();

    let local_path = download_asset_locally(&asset).await?;
    let binary_path = if asset.archive {
        extract_archive_with_timeout(&local_path, &asset).await?
    } else {
        local_path.clone()
    };

    let current_exe = std::env::current_exe()?;
    let parent = current_exe.parent().ok_or_else(|| anyhow!("no parent dir"))?;
    let staged = parent.join(format!(".llama-monitor-update-{}", std::process::id()));

    std::fs::copy(&binary_path, &staged)?;  // same filesystem as running binary
    std::fs::set_permissions(&staged, Permissions::from_mode(0o755))?;
    std::fs::rename(&staged, &current_exe)?;  // atomic

    Ok(SelfUpdateResult { tag_name: release.tag_name, windows: false, download_url: None })
}
```

#### Windows behavior

In-place replacement of a running `.exe` is blocked by Windows. The implementation uses a detached batch helper:

1. Downloads `llama-monitor-windows-x86_64.zip` and extracts via `tar -xf` (not `-xzf` — the `-z` flag forces gzip and fails on zip; Windows `tar.exe` auto-detects zip without it)
2. Writes a batch file to `%TEMP%\lm-update-<pid>.bat`:

```batch
@echo off
:check
tasklist /FI "PID eq <pid>" 2>NUL | find /I "exe" >NUL
if not errorlevel 1 (
    timeout /t 1 /nobreak >NUL
    goto check
)
copy /Y "<new_exe>" "<current_exe>"
start "" "<current_exe>"
(goto) 2>NUL & del "%~f0"
```

3. Spawns `cmd.exe /C lm-update-<pid>.bat` with `DETACHED_PROCESS` (Win32 flag `0x00000008`) so it outlives the parent
4. Returns `{ ok: true, restart_required: true }` — the API handler schedules `process::exit(0)` after 600ms, same as Unix
5. The batch file's `:check` loop detects PID exit, copies the new binary over, and relaunches it

The `(goto) 2>NUL & del "%~f0"` trick lets the batch script delete itself while still executing — the `goto` triggers an error (redirected to NUL), and `del` runs immediately after in the same command.

**Assumption:** the `.exe` is run from a user-writable location (e.g. home directory or Downloads). If run from `Program Files`, `copy /Y` will fail silently and the old binary remains. UAC elevation is not attempted.

---

### 6. Self-Update API Endpoint

**Route:** `POST /api/self-update` in `src/web/api.rs`.

On success for Unix/macOS:
- Returns `{ ok: true, restart_required: true, tag_name: "v0.11.0" }`
- Spawns a detached task: `sleep(600ms)` then `process::exit(0)` — gives the HTTP response time to flush before the process terminates

On Windows:
- Returns `{ ok: true, windows: true, restart_required: false, tag_name, download_url }`

On error:
- Returns `{ ok: false, error: "..." }` with a user-readable message (permission denied, no matching asset, etc.)

---

### 7. Frontend Update Flow

#### Mac / Linux

```
User clicks "Update & Restart"
  → triggerSelfUpdate()
  → btn: "Downloading…" (spinner)
  → POST /api/self-update
  ← { ok: true, restart_required: true }
  → btn: "Restarting…" (spinner)
  → _pollForReconnect() — HEAD / every 1s, up to 30s
  → process exits on backend (600ms delay)
  → HEAD / succeeds once process restarts
  → location.reload() — browser gets fresh app
```

If the process doesn't restart within 30s (e.g. not managed by a daemon), the button shows "Relaunch the app to finish".

#### Windows

```
User clicks "Update & Restart"
  → triggerSelfUpdate()
  → btn: "Downloading…" (spinner)
  → POST /api/self-update
    → downloads zip, extracts .exe to temp
    → writes lm-update-<pid>.bat to %TEMP%
    → spawns cmd.exe /C lm-update-<pid>.bat (DETACHED_PROCESS)
  ← { ok: true, restart_required: true }
  → btn: "Restarting…" (spinner)
  → process::exit(0) fires after 600ms
  → batch :check loop detects PID gone
  → copy /Y new_exe current_exe
  → start "" current_exe   ← app relaunches
  → _pollForReconnect() HEAD / every 1s
  → location.reload()
```

---

### 8. CSS

**`static/css/layout.css` additions:**

- `.slide-panel-title-group` — flex row aligning `h3` + version-from label
- `.slide-panel-version-from` — `11px`, muted, `opacity: 0.6`
- `.slide-panel-footer-actions` — flex row for the two footer buttons
- `.btn-update-action` — indigo-tinted primary button with hover, disabled, and `[data-state="error"]` states

---

## Files Modified

| File | Change |
|------|--------|
| `src/agent.rs` | Added `SelfUpdateResult` struct and `pub async fn self_update_binary()` inside `pub mod install`; re-exported `self_update_binary` |
| `src/web/api.rs` | Added `fn api_self_update()` (`POST /api/self-update`); registered in `api_routes()` |
| `src/web/mod.rs` | Added `{{ PLATFORM }}` template replacement alongside existing `{{ VERSION }}` |
| `static/index.html` | Added `APP_PLATFORM` to inline script; restructured release notes panel (title group, version-from span, footer-actions, `#release-notes-update-btn`) |
| `static/css/layout.css` | Added `.slide-panel-title-group`, `.slide-panel-version-from`, `.slide-panel-footer-actions`, `.btn-update-action` |
| `static/app.js` | Fixed `compareVersions` variable shadowing; replaced `pill.dataset.release` with `_pendingRelease`; added `_resetUpdateBtn()`, `triggerSelfUpdate()`, `_pollForReconnect()`; updated `openReleaseNotes()`, `closeReleaseNotes()`, `dismissUpdate()`, `showUpdatePill()` |

---

## What Was Not Implemented

- **Dedicated `/api/releases/latest` endpoint** — the spec proposed a new endpoint separate from the remote-agent one. The implementation reuses `GET /api/remote-agent/releases/latest` which returns the same GitHub data. No functional difference.
- **Progress percentage during download** — the download is buffered entirely before replacement. Streaming progress with `Content-Length` tracking would require a chunked download path; deferred.
- **UAC elevation on Windows** — if the `.exe` is in a protected location (e.g. `Program Files`), `copy /Y` in the batch script will fail silently. Assumed user runs from a user-writable directory.
