# Security Audit — Llama Monitor

Date: 2026-05-15  
Scope: End-to-end review of backend (Rust), frontend (JS/HTML), and remote-agent/SSH flows from an attacker perspective.

## Executive Summary

Llama Monitor is a powerful local-first dashboard, but several areas currently allow:

- Arbitrary SQL execution on the chat database.
- Command injection on remote hosts via SSH.
- SSRF-style internal network probing.
- Potential XSS via chat messages.
- Unrestricted filesystem browsing.

Most issues are local/LAN-scoped, but they are real and exploitable. The audit below is prioritized by severity and includes concrete fixes.

## CRITICAL

### 1. Arbitrary SQL via /api/db/query

- Status: MITIGATED
- Endpoints:
  - POST /api/db/query
  - GET /api/db/admin-token
- Files:
  - src/web/api.rs:3230–3273
  - src/chat_storage.rs:659–760
  - src/config.rs (ensure_db_admin_token)
  - static/js/features/db-admin.js
- Issue:
  - The backend accepts a user-supplied SQL string and executes it if it starts with "SELECT" or "PRAGMA".
  - Without restrictions, this allows full read of all chat data and any sensitive fields.
- Impact:
  - Full read of all persisted secrets and chat data.
  - Potential DB corruption or exfiltration.
- Likelihood:
  - High: any authenticated user (or any user if UI is exposed) can call this.
- Mitigation Applied:
  - Kept the endpoint (needed for DB admin panel and troubleshooting).
  - Enforced a strict allowlist:
    - Allowed:
      - SELECT
      - PRAGMA (diagnostics)
      - VACUUM
      - ANALYZE
    - Blocked:
      - ATTACH/DETACH DATABASE
      - LOAD_EXTENSION
      - CREATE/DROP/ALTER TABLE
      - CREATE/DROP INDEX
      - CREATE/DROP TRIGGER
      - CREATE/DROP VIEW
      - INSERT/UPDATE/DELETE/REPLACE
      - BEGIN/COMMIT/ROLLBACK
      - Dangerous PRAGMAs (journal_mode, synchronous, foreign_keys, encoding, page_size, cache_size, temp_store, mmap_size, locking_mode, wal_checkpoint variants)
  - Any query not starting with SELECT/PRAGMA/VACUUM/ANALYZE is rejected.
  - Added DB admin token:
    - On first run, a random token is generated and persisted to ~/.config/llama-monitor/db-admin-token.
    - New endpoint: GET /api/db/admin-token returns this token to the DB Admin UI.
    - DB Admin UI:
      - Fetches the token on initialization.
      - Sends it as Authorization: Bearer <token> when calling /api/db/query.
  - Restricted mode (no token or wrong token):
    - PRAGMA/VACUUM/ANALYZE allowed as before.
    - SELECT is allowed only if it does not expose sensitive columns:
      - Blocked:
        - SELECT * FROM messages
        - SELECT ... FROM messages where messages.content is selected
        - SELECT system_prompt / context_notes / model_params from tabs
      - Allowed:
        - Metadata queries (counts, timestamps, IDs, token counts, etc.).
  - Full mode (valid admin token):
    - Current behavior: full SELECT/PRAGMA/VACUUM/ANALYZE (for power users/admin).
- Remaining Risk:
  - Low:
    - With admin token, arbitrary SELECT is still allowed (by design for DB admin).
    - No localhost-only restriction yet (warp 0.4.2 does not expose addr filter cleanly).
- Next Steps:
  - Optionally: add localhost-only guard when not behind basic auth.

### 2. Command Injection in Remote-Agent (SSH)

- Status: MITIGATED
- Endpoints:
  - POST /api/remote-agent/start
  - POST /api/settings (via remote_agent_ssh_command)
- Files:
  - src/agent.rs:55–128 (validate_remote_command)
  - src/agent.rs:874–895 (autostart path)
  - src/web/api.rs:1417–1452 (start fallback path)
- Issue:
  - Two exploitable vectors were confirmed:
    - Autostart (HIGH): The `remote_agent_ssh_command` field (from ui-settings.json or app config) was used verbatim as a shell command over SSH. Any attacker able to set this field (via XSS, CSRF, or direct API call) could run arbitrary commands on the remote host when autostart triggered.
    - Start fallback (MEDIUM): POST /api/remote-agent/start accepted a `start_command` from the client when no `ssh_connection` was provided, and executed it as-is over SSH.
- Impact:
  - Remote code execution on the SSH target machine.
- Likelihood:
  - Medium-high: reachable via authenticated API calls or XSS in the dashboard.
- Mitigation Applied:
  - Added `validate_remote_command()` (src/agent.rs) with a strict allowlist:
    - Command must start with a plausible llama-monitor binary path (e.g., `llama-monitor`, `llama-monitor.exe`).
    - Only known, safe flags are allowed: `--agent`, `--agent-host`, `--agent-port`, `--models-dir`, `--version`, `--help`.
    - Rejects all shell metacharacters: `; | & $ ` ( ) { } [ ] ! # < > ? * ~ \n \r \`.
    - Rejects unknown flags and positional arguments.
  - Autostart path (agent.rs:874–895):
    - If `remote_agent_ssh_command` fails validation, logs a warning and falls back to the safe default command.
  - Start fallback path (api.rs:1417–1452):
    - If client-supplied `start_command` fails validation, falls back to the safe default command for the target.
  - `install_path` (already safe):
    - Confirmed protected by `validate_install_path()` (rejects shell metacharacters) + `shell_quote_path()` (shlex-style quoting).
- Remaining Risk:
  - Low:
    - The validation is application-level; an attacker with full control of the llama-monitor host (filesystem, process) can still manipulate config or memory.
    - The SSH layer itself still uses `channel.exec(command)` (shell on the remote), so any future change that relaxes validation without re-checking this assumption could reintroduce injection.
- Next Steps:
  - Consider long-term migration to a non-shell execution model on the remote side (e.g., a small wrapper that uses arg-style invocation instead of exec), though current allowlist is strong for now.

### 3. SSRF via Attach Endpoint and Chat Proxy

- Status: MITIGATED
- Endpoints:
  - POST /api/attach
  - POST /api/chat
- Files:
  - src/web/api.rs:3822–3964 (attach)
  - src/web/api.rs:2040–2109 (chat proxy)
- Issue:
  - Attach endpoint:
    - Accepts endpoint from client.
    - Previously:
      - Must be http/https.
      - If IP is numeric, must be loopback or RFC1918 private.
      - Hostnames were allowed without strict validation.
    - Attacker could supply a hostname that resolves internally (e.g., via DNS rebinding or internal DNS) to reach internal services.
  - Chat endpoint:
    - Proxies POST to the active session’s /v1/chat/completions.
    - If an attacker can control the active session’s endpoint (via SSRF/attach), they can:
      - Probe internal services.
      - Use llama-monitor as a pivot.
- Impact:
  - Internal network reconnaissance.
  - Potential access to internal services (metadata endpoints, other APIs).
- Likelihood:
  - Medium: requires attacker to call attach with crafted endpoint.
- Mitigation Applied (2026-05-16):
  - Added validate_endpoint_for_ssrf() in src/web/api.rs:
    - Rejects IPv4:
      - 127.0.0.0/8, 10.0.0.0/8, 172.16.0.0/12, 192.168.0.0/16
      - 169.254.0.0/16 (link-local)
      - 100.64.0.0/16 (CGNAT)
      - 198.18.0.0/15 (documentation)
      - 224.0.0.0/4 multicast, 240.0.0.0/4 reserved
      - 0.0.0.0
    - Rejects IPv6:
      - ::1, fc00::/7 (ULA), 2001:db8::/32, fe80::/10
    - Rejects hostnames:
      - "localhost", "metadata"
      - containing "metadata.google.internal" or "instance-data"
    - For non-IP hosts:
      - Resolves via ToSocketAddrs.
      - If any resolved IP is internal/metadata → reject.
      - If DNS fails → allow with a warning log (no breakage).
  - Integrated into api_attach:
    - Runs after URL parsing and before health check.
    - On block: returns 400 with "Endpoint blocked by SSRF protection".
  - Chat proxy:
    - Still constrained to /v1/chat/completions on the active session endpoint; SSRF risk primarily via attach, now mitigated.
- Remaining Risk:
  - Low:
    - DNS resolution is best-effort; race conditions or DNS rebinding could theoretically bypass in advanced scenarios.
    - For typical local/LAN use, SSRF is already limited.

## HIGH

### 4. Overly Permissive CSP and unsafe-inline Script Usage

- Status: FIXED (scripts); PARTIALLY MITIGATED (styles)
- Files:
  - src/web/mod.rs (CSP headers, index route, compact route)
  - static/index.html
  - static/compact.html
  - static/js/compact.js
- Issue:
  - Original CSP included:
    - script-src 'self' 'unsafe-inline' https://cdn.jsdelivr.net
    - style-src 'self' 'unsafe-inline' https://fonts.googleapis.com https://cdn.jsdelivr.net
  - 'unsafe-inline' allows:
    - Inline scripts and event handlers.
    - Makes XSS more impactful.
- Impact:
  - If any XSS exists (see below), 'unsafe-inline' makes exploitation easier.
- Likelihood:
  - High: already in use.
- Mitigation Applied (2026-05-17):
  - script-src:
    - 'unsafe-inline' removed for scripts across all routes.
    - Index route uses per-request nonce:
      - script-src 'self' 'nonce-{nonce}' https://cdn.jsdelivr.net
    - index.html inline script now uses nonce:
      - <script nonce="{{ NONCE }}"> for APP_VERSION/APP_PLATFORM.
    - No inline event handlers remain in index.html.
    - compact.html:
      - Inline script externalized to static/js/compact.js.
      - Served with its own per-request nonce for the small inline script that sets __COMPACT_PORT__.
  - style-src:
    - 'unsafe-inline' removed from global CSP (non-index routes).
    - Kept in index.html and compact.html where inline styles (display:none, etc.) are heavily used.
- Remaining Risk:
  - Low-Medium:
    - 'unsafe-inline' for styles still allows inline style-based XSS vectors.
    - For index.html and compact.html, removing 'unsafe-inline' from style-src would require:
      - Moving all inline styles to external CSS, or
      - Using style-src hashes for each inline style block.
    - This is a significant refactor with limited security gain (styles are less exploitable than scripts).
- Next Steps:
  - Optional: remove 'unsafe-inline' from style-src by:
    - Moving inline styles to external CSS.
    - Using style-src hashes for necessary inline styles.

### 5. Potential XSS via Markdown Rendering and DOM Insertion

- Status: MITIGATED
- Files:
  - static/js/features/chat-render.js
  - static/index.html
- Issue:
  - You use:
    - marked for markdown.
    - DOMPurify is loaded.
  - But:
    - In some code paths, HTML is inserted via innerHTML or similar.
    - If DOMPurify is not consistently applied to all dynamic content (especially streaming content), an attacker-controlled model response or chat message can carry:
      - <script>, <img onerror=...>, <iframe>, etc.
- Impact:
  - Stored XSS via chat messages.
  - Attacker can:
    - Read user’s chat, tokens, settings.
    - Perform actions as the user.
- Likelihood:
  - Medium-high: depends on exact usage of DOMPurify across all render paths.
- Mitigation Applied:
  - Updated renderMd and renderMdStreaming to:
    - Call marked.parse() as before.
    - Then wrap the result in DOMPurify.sanitize() before returning.
  - This centralizes sanitization:
    - All chat messages (normal and streaming).
    - Compaction summaries.
    - Release notes.
    - Compaction preview.
    - Edit/save flows.
- Remaining Risk:
  - Low:
    - DOMPurify is now consistently used for all marked-rendered content.
    - Any future code that uses marked must continue to use renderMd/renderMdStreaming (or apply DOMPurify) to stay safe.

### 6. Basic Auth Weaknesses and Lack of TLS Enforcement

- Files:
  - src/web/mod.rs:56
  - src/cli.rs
- Issue:
  - Basic auth:
    - Uses simple string comparison (user == expected_user && pass == expected_pass).
    - Not timing-safe (minor).
    - Sent over HTTP in base64 (trivially decoded).
  - No TLS:
    - If exposed on LAN or via reverse proxy without TLS, credentials and all data are in cleartext.
- Impact:
  - Credential sniffing on the same network.
- Likelihood:
  - Medium: if user binds to 0.0.0.0 without TLS.
- Mitigation Applied (2026-05-16):
  - Built-in TLS support added:
    - Modes:
      - No TLS (default).
      - Self-signed.
      - Custom certificate.
      - ACME (Let’s Encrypt) with DNS-01 for automated certs.
    - ACME:
      - Provider-agnostic via lego.
      - Supports 100+ DNS providers (Cloudflare, Route 53, DigitalOcean, Namecheap, etc.).
    - Certificates managed via:
      - CLI flags (--tls, --tls-cert, --tls-key, --tls-self-signed).
      - Settings → Certificates tab.
  - Basic auth:
    - Still intended as lightweight protection for local/LAN use.
    - Now recommended to be used with TLS (built-in or reverse proxy).
  - Credential sniffing risk:
    - Reduced: when TLS is enabled, all traffic (including basic auth) is encrypted.
- Remaining Risk:
  - Low-Medium:
    - If user runs without TLS on 0.0.0.0, traffic remains in cleartext.
    - Basic auth comparison is not timing-safe (minor, local-only).

## MEDIUM

### 7. Filesystem Exposure via /api/browse

- Status: FIXED
- Endpoint:
  - GET /api/browse
- Files:
  - src/web/api.rs (api_browse, around 2180–2245)
  - static/js/features/file-browser.js
- Issue:
  - Accepts path query parameter.
  - Uses PathBuf::from(&requested) and canonicalize().
  - Originally:
    - No root restriction, no allowlist, no prefix check.
    - Any directory reachable by the process was browsable.
    - Symlinks could escape via canonicalize().
- Impact:
  - Medium:
    - Attacker could enumerate the full filesystem tree readable by the process.
    - Reveals usernames, project directories, config locations, installed software, model names.
- Likelihood:
  - Medium:
    - Endpoint is exposed on the same interface as the rest of the UI.
- Mitigation Applied (2026-05-17):
  - Implemented allowlist of allowed roots:
    - User’s home directory.
    - Parent directory of the configured models_dir.
    - Parent directories of configured TLS cert/key paths.
  - Flow:
    - Requested path is canonicalized.
    - If canonicalized path does not start_with any allowed root:
      - Return JSON: { "path": ..., "error": "Path not allowed" }.
  - Symlink protection:
    - Because canonicalization happens before the allowlist check, symlinks that resolve outside allowed roots are rejected.
  - Frontend:
    - file-browser.js continues to allow arbitrary path input.
    - It now explicitly handles the "Path not allowed" error with a user-visible message.
- Remaining Risk:
  - Low:
    - Browsing is confined to a small, predictable set of directories.
    - No exposure of /etc, /var, /proc, or arbitrary system paths.

### 8. Overly Broad Database Restore and Backup Access

- Endpoints:
  - POST /api/db/restore
   - POST /api/db/backup
- Files:
  - src/web/api.rs:3235
  - src/web/api.rs:3426
  - src/web/api.rs:3498
  - src/web/api.rs:3671
- Issue:
  - Any authenticated user can:
    - List backups.
    - Restore from any backup.
    - Delete backups.
  - No separation between admin and normal user.
  - backup_name used directly in path without validation (directory traversal risk).
- Impact:
  - Data loss or tampering.
- Likelihood:
  - Medium: if multiple users share the same instance.
- Mitigation Applied (2026-05-17):
  - Auth requirements:
    - POST /api/db/backup: requires api-token.
    - GET /api/db/backups: requires api-token.
    - POST /api/db/restore: requires db-admin-token (high-impact).
    - DELETE /api/db/backup: requires db-admin-token (high-impact).
  - Path validation:
    - backup_name validated: rejects "..", absolute paths, backslashes.
    - Canonical path enforced: resolved path must stay within backups/ directory.
  - Frontend:
    - db-admin.js updated to send correct tokens:
      - api-token for backup/list.
      - db-admin-token for restore/delete.
- Remaining Risk:
  - Low: defense-in-depth is solid; risk only if tokens are leaked.

### 9. SSH Credentials and Agent Tokens Stored in Plaintext

- Status: FULLY FIXED
- Files:
  - src/config.rs (encryption helpers, AES-256-GCM)
  - src/state.rs (UiSettings load/save with encryption)
  - src/web/api.rs (token/ACME handling)
  - static/js/features/settings.js
  - static/js/features/remote-agent.js
- Issue:
  - SSH passwords, private key paths, passphrases, and agent tokens (e.g., remote_agent_token) were stored in:
    - sessions.json
    - ui-settings.json
  - No encryption at rest.
- Impact:
  - If filesystem is compromised, all secrets are exposed.
- Likelihood:
  - Medium: depends on environment.
- Mitigation Applied (2026-05-17):
  - Encryption at rest:
    - AES-256-GCM via aes-gcm crate.
    - Key management:
      - Uses LLAMA_MONITOR_ENCRYPTION_KEY if set (≥16 chars), or
      - Auto-generated 32-byte key stored in config_dir/encryption-key (Unix 0600).
    - Encrypted values stored as: enc:<base64(nonce || ciphertext)>.
    - Backward-compatible: values without "enc:" treated as plaintext.
  - Encrypted fields:
    - remote_agent_token in ui-settings.json.
    - ACME dns_config in tls-config.json (DNS provider credentials).
    - api-token file.
    - db-admin-token file.
  - SSH credentials:
    - Not persisted to disk.
    - Collected in-memory in the frontend and sent per-request (e.g., for host-key, detect, start).
    - No long-term plaintext storage.
  - UI and logging:
    - remote_agent_token masked in GET /api/settings.
    - Real token only via GET /api/settings/full with api-token auth.
    - Tokens never logged in full.
- Remaining Risk:
  - Low:
    - Secrets are exposed in plaintext only:
      - In-memory during operation.
      - Over the wire if TLS is not enabled.
    - No OS keychain integration (optional future improvement).

### 10. Lack of Rate Limiting and DoS Resistance

- Status: FIXED (core mitigations applied)
- Scope:
  - All API endpoints.
- Issue:
  - Previously:
    - No rate limiting.
    - No per-client or global request throttling.
    - No concurrency limits.
    - Only partial timeout protection.
- Impact:
  - Denial of service.
- Likelihood:
  - Medium: if exposed on LAN or internet.
- Mitigations Applied (2026-05-17):
  1. Global rate limiter:
     - Added a lightweight in-memory limiter to all routes.
     - 200 req/s with 500 burst allowance.
     - Prevents trivial flooding and accidental loops.
  2. /api/self-update and /api/kill-llama:
     - Now require api-token.
     - Added cooldowns:
       - /api/self-update: 30 seconds between calls.
       - /api/kill-llama: 15 seconds between calls.
  3. /api/db/query:
     - Wrapped in tokio::time::timeout (10 seconds).
     - SQL length capped at 16KB.
     - Returns "query timed out" or "query too long" instead of hanging.
  4. WebSocket (/ws):
     - Max 50 concurrent connections.
     - New connections rejected with 429 when limit is reached.
  5. Body size limits:
     - /api/chat: 2 MB.
     - /api/db/query: 256KB.
- Remaining Risk:
  - Low:
    - No per-client or per-IP tracking (intentional for local-first use).
    - No fine-grained per-endpoint rate limits (global is sufficient for this app).
    - Advanced SSRF/DoS via crafted traffic is still possible but significantly reduced.

## LOW

### 11. Self-Update Endpoint Abuse

- Endpoint:
  - POST /api/self-update
- Files:
  - src/web/api.rs:3987
- Issue:
  - Any authenticated user can trigger a self-update and restart.
  - No additional confirmation or admin-only protection.
- Impact:
  - Potential for forced updates or disruption.
- Likelihood:
  - Low: requires authenticated access.
- Fix:
  - Add:
    - Confirmation step.
    - Or admin-only protection.

### 12. Kill-Llama Endpoint

- Endpoint:
  - POST /api/kill-llama
- Files:
  - src/web/api.rs:3906
- Issue:
  - Any authenticated user can kill llama-server.
  - No confirmation or admin-only protection.
- Impact:
  - Disruption of service.
- Likelihood:
  - Low: requires authenticated access.
- Fix:
  - Add:
    - Confirmation step.
    - Or admin-only protection.

## Recommended Next Steps (Prioritized)

1. Harden /api/db/query:
   - DONE: strict allowlist + restricted mode + admin token + DB Admin UI integration.
2. Harden remote-agent command execution:
   - DONE: strict allowlist validator added; both autostart and start fallback now enforce safe commands.
3. Mitigate SSRF:
   - DONE: SSRF guard added to attach endpoint with IP/host allowlists and DNS checks.
4. Improve XSS protection:
   - DONE: DOMPurify now applied to all marked-rendered content.
5. Restrict /api/browse:
   - DONE: limited to home directory, models_dir parent, and TLS cert/key parent directories; symlink-escape protected.
6. Fix CSP and compact.html:
   - DONE:
     - script-src: 'unsafe-inline' removed; nonces used for index.html and compact.html.
     - compact.html inline script externalized to static/js/compact.js.
   - PARTIAL:
     - style-src: 'unsafe-inline' still present for index.html and compact.html (inline styles heavily used).
7. Add rate limiting:
   - PENDING: protect against DoS and brute-force attacks.
8. Encrypt sensitive data:
   - DONE: AES-256-GCM encryption at rest for agent tokens, ACME credentials, api-token, db-admin-token; SSH secrets in-memory only.
