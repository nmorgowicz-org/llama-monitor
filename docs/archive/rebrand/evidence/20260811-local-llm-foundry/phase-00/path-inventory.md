# Path and resource consumer inventory

## Application root and startup ordering

- `src/config.rs:428-542` defines `AppConfig`, derives `config_dir`, all
  resource paths, `default_models_dir`, and creates API/admin tokens during
  `AppConfig::from_args`.
- `src/main.rs:157-203` contains the existing Windows-only legacy move;
  `src/main.rs:264-320` constructs `AppConfig`, initializes stores/keys, and
  only then calls the migration helper. This ordering is ineffective for a
  restartable 2.0 migration and must be replaced by Phase 4.
- `src/main.rs:379-417` opens `chat.db`, initializes model state, and loads
  settings after root selection.
- `src/config.rs:99-135` owns encryption-key initialization. The HKDF info
  identifier remains an internal compatibility constant.

## Direct consumers that bypass one central root

| Consumer | Evidence | Phase |
|---|---|---|
| certificates | `src/certs.rs:15-39` derives `~/.config/llama-monitor/certs` and has a separate macOS migration | 5,9 |
| tray logs | `src/tray.rs:755-767` derives `dirs::config_dir()/llama-monitor/logs` | 5,9 |
| remote agent state/install | `src/agent.rs:377-496`, `2306-2314`, `3986-4051`, `4151-4227` | 5,9 |
| remote Windows tasks | `src/agent.rs:1743-1769`, `3762-3766` | 9 |
| updater assets | `src/agent.rs:1911-1917`, `3986-4051` | 9,11 |
| model library | `src/models/library.rs:26-40`, `148-194`, `942-1098` | 6 |
| model recovery/import | `src/models/gguf_recovery.rs:632-723`, `src/models/gguf_import.rs:245-281` | 6 |
| HF/MLX caches | `src/llama/spawn_wizard.rs:689-701`, Rapid-MLX sidecar/store modules | 5,6 |
| launcher | `src/inference/launch.rs:427-458` resolves effective models path | 5,6 |

## Resource classes requiring migration policy

Stage A: encryption key, API/admin tokens, auth/TLS config, cert/key/CA
material, agent tokens/config, presets/sessions/settings/templates/GPU state,
`chat.db` via `ChatStorage::backup()`, provenance/journals/sidecars, and
resumable-download metadata.

Stage B: managed model trees (`gguf`, `mlx/native`, `mlx/converted`,
`transformers`, `rapid-mlx`, managed HF cache, staging, partials, provenance).
External/custom `--models-dir` and external HF repositories remain in place
unless explicitly selected by the user.

## Platform roots

- Legacy Unix/macOS: `~/.config/llama-monitor/`.
- Canonical Unix/macOS: `~/.config/local-llm-foundry/`.
- Legacy Windows: `%APPDATA%\\llama-monitor\\` (plus documented historical
  `%USERPROFILE%\\.config\\llama-monitor` cases).
- Canonical Windows: `%APPDATA%\\local-llm-foundry\\`.

No root is deleted automatically. Symlink/reparse escapes, collisions,
cross-volume copies, changed free-space preconditions, and interrupted
journals must fail closed.

## Live macOS read-only inventory (2026-08-11)

The existing legacy Unix/macOS root is populated. This was a metadata/path
listing only; secret and database contents were not read.

- `/Users/nick/.config/llama-monitor`: mode `drwxr-xr-x`, 1120 bytes.
- `chat.db`: 4,890,624 bytes, with `chat.db-wal` (24,752 bytes) and
  `chat.db-shm` (32,768 bytes) present.
- `models/`: mode `drwxr-xr-x`, with `gguf`, `rapid-mlx`, `cache`,
  `.staging`, `experimental`, and a root provenance sidecar.
- `.staging/downloads/` contains incomplete `.part` model downloads.
- `rapid-mlx/mtp-sidecars/` contains provenance metadata.
- `bin/` and multiple `bin-previous-*` directories contain runtime binaries
  and historical rollback payloads.
- `/Users/nick/Library/Application Support/llama-monitor`: mode `drwxr-xr-x`,
  384 bytes, with settings/session/preset/chat backup files and `certs/`.

No Windows application root or checked-in Windows fixture exists in this
checkout; Phase 12 must provide synthetic `%APPDATA%` fixtures and native
cross-checks rather than inferring Windows behavior from the Unix inventory.

## Phase 2 correction

The old best-effort Windows move in `src/main.rs` was removed before any
default switch. Startup now performs metadata-only root inspection first and
selects legacy-only installs in place; both-root cases fail closed until the
explicit journaled migration flow exists. This correction is intentionally
appended rather than rewriting the baseline observation above.
