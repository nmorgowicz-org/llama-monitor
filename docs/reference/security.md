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

Three endpoints rotate tokens at runtime. All require a valid `api-token` (Bearer).
Implemented in `src/web/api/tokens.rs`.

- `POST /api/rotate-agent-token`
  - Rotates the remote-agent token.
  - Updates `ui-settings.json` and notifies the agent poll loop.
- `POST /api/rotate-api-token`
  - Rotates the general API bearer token.
  - Writes new token to `api-token` file and updates in-memory state via `update_live_api_token`.
- `POST /api/rotate-db-admin-token`
  - Rotates the elevated DB admin bearer token.
  - Writes new token to `db-admin-token` file and updates in-memory state via `update_live_db_admin_token`.

Important:
- All three update both the on-disk file and the live in-memory `AppConfig` atomically, so the old token stops working immediately without a restart.
- Tokens and nonces are generated with `getrandom::getrandom()`; argon2 salt uses `rand_core::OsRng` where a trait RNG is required. All are stored encrypted when encryption is configured.

## Security Headers and CSP

The HTTP server uses `warp_helmet` plus explicit CSPs for HTML pages.

### Global (non-index routes)

Applied via `Helmet` with a custom `ContentSecurityPolicy`:

- `default-src`: `'self' data:`
- `connect-src`: `'self' https: wss:`
- `script-src`: `'self' https://cdn.jsdelivr.net`
- `style-src`: `'self' https://fonts.googleapis.com https://cdn.jsdelivr.net`
- `font-src`: `'self' https://fonts.gstatic.com`
- `img-src`: `'self' data: https:`
- `frame-src`: `'self'`

`warp_helmet` also sets standard hardening headers (e.g., `X-Content-Type-Options`, `X-Frame-Options`, `X-XSS-Protection`) on these routes.

### index.html

Served with a dedicated CSP that:

- Uses a per-request cryptographic nonce (16-byte, `getrandom::getrandom()`) for inline scripts.
- Allows:
  - `connect-src`: `'self' https: wss:`
  - `script-src`: `'self'` + nonce + `https://cdn.jsdelivr.net`
  - `style-src`: `'self' 'unsafe-inline'` + Google Fonts + jsDelivr
  - `font-src`, `img-src`, `frame-src` similar to global.

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

## Per-Endpoint Cooldowns

Some endpoints enforce short cooldowns to reduce accidental or abusive use.

- `POST /api/kill-llama`
  - Requires `db-admin-token` and `{ "confirm": "kill" }`.
  - 30-second cooldown between calls.
- `POST /api/self-update`
  - Requires `db-admin-token` and `{ "confirm": "update" }`.
  - 5-minute cooldown between calls.
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