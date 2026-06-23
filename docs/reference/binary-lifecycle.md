# Binary Lifecycle — llama.cpp Runtime Management

This document describes how llama-monitor discovers, installs, updates, and version-tracks
the `llama-server` binary. It is the reference implementation for backend runtime management.
The Rapid-MLX integration plan (`docs/plans/20260611-rapid_mlx_integration.md`) explicitly
names this system as the quality baseline that all future backends must match.

## Source files

| File | Responsibility |
|---|---|
| `src/llama/llama_cpp_downloader.rs` | Release fetching, asset selection, download/extract, artifact cleanup |
| `src/web/api/llama_binary.rs` | HTTP endpoints: version, latest, releases, release, platform-info, update, restart |
| `src/model_download.rs` | Model-file download manager: dedup guard, existing-file check, concurrency limit |

## Binary path

The path to `llama-server` is resolved at startup from `AppConfig::llama_server_path`.
The install workflow writes to `dest_path.parent()` (the directory containing that binary),
so all per-backend shared libraries land next to the executable.

## API endpoints

All endpoints require the configured API token (`Authorization: Bearer <token>`).

| Method | Route | Purpose |
|---|---|---|
| `GET` | `/api/llama-binary/version` | Currently installed build number |
| `GET` | `/api/llama-binary/latest` | Latest GitHub release metadata (30-min cache) |
| `GET` | `/api/llama-binary/releases` | Last 8 releases for the version picker |
| `GET` | `/api/llama-binary/release?build=N` | Metadata for one specific build |
| `GET` | `/api/llama-binary/platform-info` | Detected OS, arch, available backends |
| `POST` | `/api/llama-binary/update` | Download and install a release |
| `POST` | `/api/llama/restart` | Restart the running llama-server with its current config |

### `GET /api/llama-binary/version`

Runs `llama-server --version` synchronously (blocking task), combines stdout + stderr,
and extracts the build number with the pattern `(?:version|build)[:\s]+(\d+)`.

Returns `{ build: N, version: "bN", path: "..." }`.
Returns `{ build: null, version: null, path: "..." }` if the binary is missing or
the output does not match.

### `GET /api/llama-binary/latest`

Calls `list_releases` and returns the first (most recent) result. Cached for 30 minutes
in a process-static `Mutex<Option<(Instant, Value)>>`. Cache is bypassed only on expiry,
not on update install.

### `GET /api/llama-binary/releases`

Returns the most recent 8 releases. Each entry includes tag name, publication date, and release
body (notes). The endpoint returns all assets for each release without filtering by the current
platform; the caller (UI) applies its own `select_assets` filter to present only relevant
downloads for the host.

### `GET /api/llama-binary/release?build=N`

Fetches a single release by build number via `get_release_by_tag("bN")`, which hits
`https://api.github.com/repos/ggml-org/llama.cpp/releases/tags/bN`.
Used by the version picker rollback flow.

### `GET /api/llama-binary/platform-info`

Returns:
- `os`: `"macos"` / `"linux"` / `"windows"`
- `arch`: `"aarch64"` / `"x86_64"`
- `arch_label`: human-readable architecture label
- `auto_backend`: backend chosen automatically for this platform
- `label`: short human-readable platform label (e.g. "Apple Silicon Metal")
- `backends`: array of available backend choices, each with:
  - `id`: backend identifier (e.g. "cuda12", "vulkan", "rocm")
  - `label`: display label
  - `note`: short description of requirements
  - `recommended`: whether it is the recommended option
- `multi_backend`: true when the platform offers multiple selectable backends (Linux and Windows)

The UI uses this to pre-select the correct backend and to render backend-choice UI on
multi-backend platforms.

### `POST /api/llama-binary/update`

Body (all fields optional):

```json
{
  "backend": "metal",
  "tag": "b9479"
}
```

`backend` defaults to the platform default (`metal` / `cpu` / `avx2`).
`tag` defaults to the latest release when omitted.

Response:

```json
{
  "ok": true,
  "version": "b9512",
  "backend": "metal",
  "arch": "arm64",
  "sha256": "abc123...",
  "server_restarted": true
}
```

- `sha256`: SHA256 of the installed llama-server binary (for integrity verification).
- `server_restarted`: true when llama-monitor already restarted the server with its previous
  config; the frontend must skip its own restart call when this is true.

### `POST /api/llama/restart`

Restarts the running local llama-server using its saved configuration and the current
llama-server binary. Commonly used after an in-place update to pick up the new binary.

- If no local server is running, returns an error.
- Brief pause is inserted between stop and start to ensure the previous process has fully exited.

## Update pipeline

The install handler follows a strict ordering: it downloads and prepares a candidate
binary in a temporary directory, then stops the running server (if any), runs a health
check on the new binary, and only then installs it. If health checks fail, the original
binary is left in place.

```
1. Capture current ServerConfig if a local server is running
2. Fetch release list from GitHub
3. Select matching assets via select_assets(release, backend, arch)
4. Download + extract all assets into a temp dir (tempfile::tempdir())
5. Locate llama-server / llama-server.exe inside the temp tree
6. Set executable bit (unix: chmod 0o755)
7. Strip macOS Gatekeeper quarantine xattr (macOS only)
8. Stop running llama-server (if it was running)
9. Health check: run `llama-server --help` from the temp binary with a 10-second timeout
10. Install:
    - macOS: copy into a sibling staging dir, health-check again, rename old bin/ to backup, promote staging dir
    - other platforms: copy into the configured `bin/` path, normalize binary name, health-check the installed binary
11. Restart server with preserved config (if it was running before)
```

If any step from 1–7 fails, the live binary is untouched and the server (if running)
continues to run. The server is stopped in step 8; if the subsequent health check (step 9)
fails, the old binary is not overwritten and the caller receives an error indicating the
new binary failed validation.

### Step 3 — Asset selection (`select_assets`)

`select_assets` is a filename heuristic in `llama_cpp_downloader.rs`. It filters release
assets by backend keyword and architecture substring:

**Backend keywords** (matched against lowercased asset name):

| Backend | Matches |
|---|---|
| `cpu` | `cpu`, `base`, `avx2` |
| `avx2` | `avx2`, `cpu`, `base` |
| `cuda` | `cuda` |
| `cuda12` | `cuda12`, `cuda-12`, `cu12`, `cuda_12` |
| `cuda13` | `cuda13`, `cuda-13`, `cu13`, `cuda_13` |
| `sycl` | `sycl`, `oneapi` |
| `vulkan` | `vulkan` |
| `rocm` / `hip` | `rocm`, `hip` |
| `metal` | `metal`, `mac` |

**Architecture keywords** (matched against lowercased asset name):

| arch | Matches |
|---|---|
| `x86_64` / `x64` | `x64`, `x86_64`, `amd64` |
| `arm64` / `aarch64` | `arm64`, `aarch64` |

Files ending in `.json`, `.md`, or `.txt` are always skipped.

Multiple assets may be selected (e.g. the main archive plus a shared-library bundle).
All selected assets are downloaded; GPU builds rely on this to co-locate CUDA/Vulkan
`.dll` / `.so` files next to the binary.

### Step 4 — Download and extract (`download_and_extract`)

Each asset is streamed to `<tmpdir>/<asset.name>.part`, renamed to `<asset.name>` on
completion, then extracted if it is a `.zip`, `.tar.gz`, or `.tgz`. Extraction failures
are logged as warnings but do not abort the install.

The HTTP client uses a 300-second timeout on the outer request.

After extraction, `cleanup_old_binaries` runs against the temp dir:
- Tarballs matching `llama-b<N>-bin-*.tar.gz`: only the highest build number is kept.
- Versioned dylibs matching `<prefix>.<N>.dylib`: only the highest build in each family
  is kept.
This prevents the binary directory accumulating stale archives across repeated updates.

### Step 5 — Binary location (`find_binary`)

A small recursive search (`find_binary`) walks the temp dir looking for `llama-server`
(or `llama-server.exe` on Windows). Releases may place the binary at the archive root or
inside a subdirectory named after the build (e.g. `llama-b9479-bin-macos-arm64/`).

### Step 7 — Gatekeeper quarantine strip (macOS)

**Why this matters.** The entire extracted archive — the executable, co-located dylibs
(`libllama.dylib`, etc.), and Metal shaders (`ggml-metal.metal`) — can carry the
`com.apple.quarantine` extended attribute. Gatekeeper enforces this at `dlopen()` time,
not just at initial exec, so stripping only the executable is insufficient: the process
launches, tries to load a quarantined dylib, and Gatekeeper SIGKILLs it. The exit status
reports no exit code (Unix signal termination shows up as `code = "unknown"`).

The strip is applied **recursively to the entire temp dir** so that `copy_all_files`
copies clean files into `dest_dir`:

```rust
#[cfg(target_os = "macos")]
{
    let _ = std::process::Command::new("xattr")
        .args(["-rd", "com.apple.quarantine"])
        .arg(tmp_dir.path())
        .output();
}
```

`start_server` applies the same strip to the **binary's parent directory** before its own
`--help` check. This covers manually placed binaries and the restart-after-update path
where the binary has already been written to `dest_dir`:

```rust
#[cfg(target_os = "macos")]
if let Some(bin_dir) = bin_path.parent() {
    let _ = std::process::Command::new("xattr")
        .args(["-rd", "com.apple.quarantine"])
        .arg(bin_dir)
        .output();
}
```

Both calls are best-effort (errors discarded). A missing `xattr` binary on an unusual
macOS installation does not abort the flow; the health check that follows will still
catch a genuinely broken binary.

### Step 8 — Health check

```rust
tokio::time::timeout(Duration::from_secs(10), async {
    Command::new(&tmp_binary)
        .arg("--help")
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .output()
        .await
})
```

A successful exit code from `--help` is the gate. Stderr is captured for the failure
log message (Gatekeeper errors, missing dylibs, wrong architecture, etc.). A timeout
produces the message `"health check timed out"`.

If the check fails, the error response includes the human-readable message shown in the
UI toast:

> "New llama-server binary failed basic health check. The downloaded file may be
> corrupted or incompatible. Try updating again or install manually."

### Step 9 — Copy to destination (`copy_all_files`)

On macOS, the updater does not mutate the live binary directory in place. It creates
a hidden sibling staging directory, copies all extracted release files there,
normalizes the configured binary name, runs `llama-server --help` from the staged
location, renames the current `bin/` directory to a timestamped backup, and
promotes the staging directory to `bin/`. This avoids stale Gatekeeper/XProtect
provenance state from a previous failed install or in-place overwrite poisoning the
live path.

On Linux and Windows, the updater copies into the configured binary directory,
normalizes the configured binary name, cleans old artifacts, and health-checks
the installed binary before reporting success.

## Model-file download stability

Model-file downloads (Hugging Face, etc.) are managed in `src/model_download.rs` with three
safety guards to reduce conflicts, wasted bandwidth, and runaway concurrency.

### Duplicate guard

A single in-memory `HashMap` keyed by `(repo_id, file_path)` tracks active download tasks.
If the API receives a second request for the same file while an existing task is running,
it returns a 409 error with the message:

> Already downloading: {name}. Please wait until it completes.

### Existing-file guard

Before starting a download, the handler checks whether the target file already exists at
its final path with a reasonable size. If so, it returns a 409 error:

> File already exists at: {path}. It may be available in your library.

This prevents re-downloading large models that are already on disk.

### Concurrency limit

A hard limit of 2 simultaneous running downloads is enforced. A third request is rejected
immediately with:

> Too many downloads in progress. Please wait for one to finish.

The frontend (hf-browse.js, models.js) listens for these responses and shows toast messages
instead of silently queuing extra tasks.

## Server stop/restart around updates

When the local server is running at update time:

1. The current `ServerConfig` is captured before download and validation begins.
2. Download, extraction, and temp/staged binary health checks run while the existing server keeps running.
3. `stop_server` is called only after the replacement candidate has passed pre-install validation.
4. After a successful install and final installed-path health check, the server is restarted with the preserved config.
5. If promotion fails after the old directory is moved aside, the updater attempts to restore the previous directory before returning an error.

## Windows self-update

On Windows, a running executable cannot overwrite itself. When `llama-monitor.exe` installs an update to itself (the `install` module in `src/agent.rs`), it writes a small `.bat` helper and launches it with the `DETACHED_PROCESS` creation flag. The batch file waits for the parent process PID to exit, then copies the new binary over the old path and starts the updated executable. This is distinct from the llama-server binary update described above, which uses the rename-based `copy_all_files` pipeline and runs entirely within the Rust process.

---

## Expected quality bar for future backends

The Rapid-MLX integration must provide the same set of capabilities through its own
routes under `/api/inference/backends/rapid_mlx/`:

- Installed version display (equivalent of `/api/llama-binary/version`)
- Latest available version with release notes (equivalent of `/api/llama-binary/latest`)
- Historical release list with rollback support (equivalent of `/api/llama-binary/releases`)
- Platform availability and recommended configuration (equivalent of `/api/llama-binary/platform-info`)
- Atomic install/upgrade with a pre-activation health check gate (equivalent of the update pipeline above)
- Cleanup of superseded artifacts after install

The health check gate in particular is non-negotiable: no backend should ever overwrite
a working runtime with an unverified one. The macOS quarantine strip is a llama.cpp
specific detail (binary download); Rapid-MLX installs through pip/Homebrew and will
have its own equivalent first-run probe concerns.
