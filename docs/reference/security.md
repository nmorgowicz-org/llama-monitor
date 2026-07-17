# Security Reference

This document covers dashboard-side authentication, token handling, local recovery, and where
security-related configuration is stored on disk.

## Architecture Note

All API security behavior is implemented in the modular API layer under `src/web/api/`, primarily:

- `src/web/api/common.rs` — token checks, auth helpers, shared response utilities.
- `src/web/api/auth.rs` — dashboard auth routes (`/api/auth/status`, `/api/auth/login`, etc.).
- `src/web/api/tokens.rs` — token rotation and internal bootstrap.
- Domain modules (e.g., `db.rs`, `remote_agent.rs`, `self_update.rs`, `sleep.rs`) — per-endpoint auth enforcement.

No public HTTP endpoints have changed as a result of the modular refactor; only internal file
locations have.

## Dashboard Access Modes

Llama Monitor supports four dashboard access states:

- `No Auth`
  - Default behavior.
  - Intended for local-only use on `127.0.0.1` or trusted LAN.
  - When "No Auth" is configured:
    - No endpoints are gated behind a login.
    - The api-token is still auto-generated and used internally for protected endpoints.
    - Token bootstrap (`/api/internal/api-token`) is fully allowed, even for non-loopback llama-server endpoints.
- `Form Login`
  - Shows the in-app sign-in shell before protected routes are available.
  - Uses an HttpOnly session cookie after a successful login.
- `Basic Auth`
  - Uses the browser's native username/password challenge.
- `Both`
  - Accepts either HTTP Basic credentials or a valid in-app form session.

Users can manage config-backed dashboard auth from:

- `Settings → Security → Dashboard Access`

CLI flags remain available for startup-managed auth:

- `--basic-auth user:password`
- `--form-auth user:password`

If startup flags are present, the Security tab shows that the instance is CLI-managed and treats
the in-app dashboard-access controls as read-only.

## Stored Files

Security-relevant local files include:

- `auth-config.json`
  - Stores dashboard auth mode, username, and hashed password.
- `api-token`
  - General admin bearer token for protected UI/API operations.
- `db-admin-token`
  - Elevated bearer token for destructive database operations.
- `tls-config.json`
  - Persisted TLS mode and ACME configuration.
- `encryption-key`
  - Local key used for at-rest encryption of other sensitive values.

On Unix, llama-monitor hardens these files to owner-only permissions (`0600`) at startup.

## Password Storage

Dashboard passwords stored in `auth-config.json` are:

- hashed, not stored in plaintext
- not returned by the UI API
- used for both Basic Auth and form-login verification when config-managed auth is enabled

The auth config is separate from `ui-settings.json` so security settings do not get mixed into the
general dashboard preferences payload.

## Password Change Flow

When a user is already signed in:

1. Open `Settings → Security`.
2. Go to `Dashboard Access`.
3. Choose the desired auth mode.
4. Enter a username.
5. Enter a new password.
6. If replacing an existing password, provide the current password too.
7. Click `Save Dashboard Access`.

Behavior:

- Changing the password invalidates existing form-auth sessions.
- Changing the mode without changing the password keeps the existing stored hash.
- Setting both modes to off disables dashboard auth and clears `auth-config.json`.

## Locked-Out Recovery

There is intentionally no unauthenticated "forgot password" web endpoint.

If a user is locked out of config-managed dashboard auth:

1. Run:

   ```bash
   llama-monitor --clear-auth-config
   ```

2. Restart llama-monitor.
3. Open the local dashboard.
4. Reconfigure `Dashboard Access` from the Security tab.

This keeps recovery local to the machine instead of exposing a network-visible reset action.

## Migration From Older Builds

Older builds only supported dashboard auth through startup flags.

On newer builds:

- If `--basic-auth` and/or `--form-auth` are present and compatible, llama-monitor seeds
  `auth-config.json` automatically on first run.
- Runtime behavior still honors the live startup flags first.
- Once users remove the flags, the stored config-backed auth can take over without a manual reset.

If `--basic-auth` and `--form-auth` use different credentials, the app does not auto-migrate them
into `auth-config.json` because the persisted dashboard-access UI uses a single shared account.

## Token Rotation

Three endpoints rotate tokens at runtime. Implemented in `src/web/api/config.rs`.
Auth requirements:

- `POST /api/rotate-agent-token`
  - Requires: `api-token` (Bearer).
  - Rotates the remote-agent token.
  - Updates `ui-settings.json` and notifies the agent poll loop.
- `POST /api/rotate-api-token`
  - Requires: `api-token` (Bearer).
  - Rotates the general API bearer token.
  - Writes new token to `api-token` file and updates in-memory state via `update_live_api_token`.
- `POST /api/rotate-db-admin-token`
  - Requires: `api-token` (Bearer).
  - Rotates the elevated DB admin bearer token.
  - Writes new token to `db-admin-token` file and updates in-memory state via `update_live_db_admin_token`.

Important:
- All three update both the on-disk file and the live in-memory `AppConfig` atomically, so the old token stops working immediately without a restart.
- Tokens and nonces are generated with `getrandom::getrandom()`; argon2 salt uses `rand_core::OsRng` where a trait RNG is required. All are stored encrypted when encryption is configured.

### API Token vs DB-Admin Token Boundary

The project uses two tokens:

- `api-token` — general bearer token for routine operations:
  - Reading sessions, presets, templates, settings, models.
  - Chat persistence, search, and streaming.
  - GPU env, browse, HF, VRAM, benchmark, metrics, TLS config.
  - DB stats, integrity, maintenance, backup creation, index listing, backups listing.
  - DB queries via `POST /api/db/query` with a relaxed column filter on `SELECT`:
    - Sensitive columns such as `content` in messages, `system_prompt`, `context_notes`,
      and `model_params` are blocked unless you use `db-admin-token`.
    - Non-SELECT commands (`PRAGMA`, `VACUUM`, `ANALYZE`) are allowed as-is.

- `db-admin-token` — elevated token for destructive/high-impact operations:
  - DB restore, repair, and backup deletion.
  - Session deletion.
  - Session spawn (starting any managed inference backend).
  - Kill-server (best-effort backend-agnostic stop).
  - Self-update.
  - Metal GPU limit tuning (on macOS).
  - DB queries via `POST /api/db/query` with no column restrictions on `SELECT`.

Operations protected by `db-admin-token`:
- `DELETE /api/db/backup`
- `POST /api/db/restore`
- `POST /api/db/repair`
- `DELETE /api/sessions/:id`
- `POST /api/sessions/spawn`
- `POST /api/kill-server`
- `POST /api/self-update`
- `POST /api/system/set-metal-gpu-limit`

DB operations:
- The `POST /api/db/query` endpoint allows more than `SELECT`:
  - `VACUUM` and `ANALYZE` are allowed.
  - A restricted PRAGMA allowlist is permitted (see below).
- This is not limited to "read-only" queries.

### PRAGMA Allowlist (chat_storage.rs)

The `execute_query` in `chat_storage.rs` enforces:

- Single-statement only: presence of `;` anywhere is rejected ("Multi-statement queries are not allowed").
  - This is a simple substring scan, not a parser; it is fragile and can be bypassed indirectly.
- Dangerous keywords blocked (e.g., INSERT, UPDATE, DELETE, DDL, ATTACH, LOAD_EXTENSION, etc.).
- Only `SELECT`, `VACUUM`, `ANALYZE`, and a restricted PRAGMA subset are allowed.
- `writable_schema` is NOT in the allowlist, so it is blocked.
- The allowlist includes several write-affecting PRAGMAs:
  - `INCREMENTAL_VACUUM`, `AUTOVACUUM`, `SECURE_DELETE`, `WAL_AUTOCHECKPOINT`,
    `TEMP_STORE`, `MMAP_SIZE`, `QUERY_ONLY`, etc.

Example allowed PRAGMAs:
- `INTEGRITY_CHECK`, `QUICK_CHECK`, `PAGE_COUNT`, `FREELIST_COUNT`, `SCHEMA_VERSION`,
  `USER_VERSION`, `INDEX_LIST`, `INDEX_INFO`, `TABLE_INFO`, `TABLE_XINFO`,
  `FOREIGN_KEY_LIST`, `LOCK_LIST`, `DATABASE_LIST`, `JOURNAL_MODE`, `SYNCRONOUS`,
  `CACHE_SIZE`, `CACHE_SPILL`, `EPOCHMS`.

This design intentionally restricts arbitrary PRAGMA while still permitting some configuration
queries that may alter internal behavior.

## Security Headers and CSP

The HTTP server uses `warp_helmet` plus explicit CSPs for HTML pages.

### Global (non-index routes)

Applied via `Helmet` with a custom `ContentSecurityPolicy`:

- `default-src`: `'self' data:`
- `connect-src`: `'self' https: wss:`
- `script-src`: `'self'` (no CDN) for non-index routes.
- `style-src`: `'self' https://fonts.googleapis.com`
- `font-src`: `'self' https://fonts.gstatic.com`
- `img-src`: `'self' data: https:`
- `frame-src`: `'self'`

`warp_helmet` also sets standard hardening headers (e.g., `X-Content-Type-Options`, `X-Frame-Options`, `X-XSS-Protection`) on these routes.

### index.html

Served with a dedicated CSP that:

- Uses a per-request cryptographic nonce (16-byte, `getrandom::getrandom()`) for inline scripts.
- Allows:
  - `connect-src`: `'self' https: wss:`
  - `script-src`: `'self'` + nonce + `https://cdn.jsdelivr.net` (CDN kept only for
    backward compatibility with SRI-pinned scripts; most vendor scripts are now self-hosted).
  - `style-src`: `'self' 'unsafe-inline'` + Google Fonts.
  - `font-src`, `img-src`, `frame-src` similar to global.
- Note: `strict-dynamic` is not used; it disabled host allowlists and blocked both local
  and CDN scripts.

### /compact

Served with its own CSP:

- Uses a per-request nonce for inline scripts.
- No external script sources; `script-src` is `'self'` + nonce.
- `style-src` includes `'unsafe-inline'` for inline styles.

## Origin / CSRF Guard

For `/api/*` routes, there is an Origin validation filter:

- Applies only to mutating methods: POST, PUT, PATCH, DELETE.
- Behavior:
  - If the `Origin` header is present and does not match the server's origin, the request is rejected with 403.
  - If `Origin` is absent, the request is allowed (to support curl and non-browser clients).
  - GET requests are always allowed.
- When bound to `0.0.0.0`:
  - Only the port is validated (via rsplit on `:`), any host is accepted if the port matches.

This is a best-effort defense against cross-origin CSRF from third-party pages; it is not a full CSRF token system.

## Rapid-MLX Runtime Mutation Auth

Rapid-MLX runtime changes (install, upgrade, repair, rollback) are protected with
`db-admin-token` plus an explicit confirmation string. Each mutation runs in a managed,
isolated environment under `config_dir/runtimes/rapid-mlx/` and never touches the host
system outside that tree.

Endpoints:

- `POST /api/rapid-mlx/runtime/install`
  - Requires: `db-admin-token` (Bearer).
  - Body: `{ "version": "<version>", "channel": "stable"|"prerelease", "confirm": "INSTALL_RAPID_MLX_RUNTIME" }`.
  - Platform gate: 400 on non-Apple-Silicon; no mutation occurs.

- `POST /api/rapid-mlx/runtime/upgrade`
  - Requires: `db-admin-token` (Bearer).
  - Body: `{ "version": "<version>", "channel": "stable"|"prerelease", "confirm": "UPGRADE_RAPID_MLX_RUNTIME" }`.

- `POST /api/rapid-mlx/runtime/repair`
  - Requires: `db-admin-token` (Bearer).
  - Body: `{ "confirm": "REPAIR_RAPID_MLX_RUNTIME" }`.
  - Reinstalls the active managed runtime and verifies it against published metadata.

- `POST /api/rapid-mlx/runtime/rollback`
  - Requires: `db-admin-token` (Bearer).
  - Body: `{ "confirm": "ROLLBACK_RAPID_MLX_RUNTIME" }`.

Notes:

- All four use `check_db_admin_token()` with a strict confirmation string, not `api-token`.
- Only one mutation may run at a time; concurrent requests receive 429.
- The active runtime is only updated after full validation; if validation fails, the
  previous environment is preserved and no public pointer is changed.
- The public status endpoints (`GET /api/rapid-mlx/runtime/status`, `/releases`,
  `/jobs/:id`) require `api-token` (not `db-admin-token`) and never expose internal
  filesystem paths, environment IDs with sensitive details, or raw error traces.

### Runtime Sandboxing

When installing a Rapid-MLX runtime, the process is heavily constrained:

- `uv` runs with `env_clear()` plus a small allowlist:
  - `PATH`, `SSL_CERT_FILE`, `SSL_CERT_DIR` forwarded for connectivity.
  - All other env vars (including `HF_TOKEN`, `LLAMA_MONITOR_*`, `PATH` extras) are dropped.
- The managed root is created under `config_dir/runtimes/rapid-mlx/`:
  - `environments/<id>/` — isolated per-version Python and tool env.
  - `uv-cache/`, `uv-python/` — private caches.
- Path traversal defenses:
  - No symlink components allowed anywhere in the managed root.
  - All child paths are canonicalized and validated to stay inside the root.
- HF_TOKEN:
  - Not set in the uv install environment.
  - For runtime HF operations (model fetches, config introspection), the token is passed
    only through the child process environment and never stored in manifests, source
    metadata, or command arguments.

### Rapid-MLX Diagnostics and Token Redaction

Rapid-MLX runtime error responses are sanitized:

- `public_runtime_error()` normalizes internal error messages into a small set of stable
  strings, discarding filesystem paths, Python tracebacks, and uv output.
- Public inventory responses (`PublicRuntimeInventoryEntry`) include:
  - `environment_id`, `version`, `release_channel`, `active`, `rollback_candidate`, `complete`.
  - The absolute `executable_path` is excluded and never exposed.
- Job detail responses (`GET /api/rapid-mlx/runtime/jobs/:id`) never include raw stderr
  from uv or the runtime probe.

## Per-Endpoint Cooldowns

Some endpoints enforce short cooldowns to reduce accidental or abusive use.

- `POST /api/kill-server`
  - Requires `db-admin-token` and `{ "confirm": "kill" }`.
  - 30-second cooldown between calls.
  - Backend-agnostic: stops any actively managed inference process (llama.cpp or Rapid-MLX)
    via the shared `stop_server()` path (generation-based invalidation, supervisor cleanup).
  - Fallback: if no supervised process is found, it attempts a best-effort OS-level kill
    of `llama-server`; this does not affect Rapid-MLX, which is only ever killed through
    the managed stop_server() path.
- `POST /api/self-update`
  - Requires db-admin-token and `{ "confirm": "update" }`. 5-minute cooldown between calls.
  - Update safety: self-update downloads the release asset for the running platform in-place. No cryptographic signature or integrity check is currently enforced beyond token-based auth.
- `POST /api/sessions/spawn`
  - Requires `db-admin-token`.
  - 15-second cooldown between spawns.

Each uses an in-memory atomic timestamp; these cooldowns do not survive restarts.

## Rate Limiting

Global and per-surface limits:

- HTTP requests:
  - Base: 200 req/s.
  - Burst allowance: up to 700 in a 1-second window (200 + 500 burst).
  - Implemented as a per-second atomic counter; excess requests get 429.
- WebSocket:
  - Max 50 concurrent connections.
  - Additional connections are rejected with 429.
- `/api/db/query`:
  - Max body size: 256 KB.
  - Max SQL length: 16 KB.
  - Execution timeout: 10 seconds.

These are lightweight, single-instance, in-memory limits — not distributed or per-client.

## Related Docs

- [CLI Flags](cli-flags.md)
- [API Reference](api.md)
- [TLS Architecture](tls-architecture.md)