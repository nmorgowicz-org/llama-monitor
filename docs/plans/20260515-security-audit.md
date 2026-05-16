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

- Status: PARTIALLY MITIGATED
- Endpoints:
  - POST /api/db/query
- Files:
  - src/web/api.rs:3218
  - src/chat_storage.rs:659
- Issue:
  - The backend accepts a user-supplied SQL string and executes it if it starts with "SELECT" or "PRAGMA".
  - This is not a safe sandbox:
    - SELECT can read all chat data, tokens, SSH targets, agent tokens, etc.
    - PRAGMA can alter DB behavior.
    - Extensions or functions may be callable depending on build.
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
- Remaining Risk:
  - Still allows broad SELECT/PRAGMA access (e.g., reading all chat data, tokens, SSH targets).
  - No localhost-only restriction yet (warp 0.4.2 does not expose addr filter cleanly).
- Next Steps:
  - Add localhost-only guard when not behind basic auth.
  - Consider adding a separate admin-only token for sensitive queries.

### 2. Command Injection in Remote-Agent (SSH)

- Endpoints:
  - POST /api/remote-agent/install
  - POST /api/remote-agent/start
  - POST /api/remote-agent/update
  - POST /api/remote-agent/stop
  - POST /api/remote-agent/remove
- Files:
  - src/web/api.rs:1293–1583
  - src/agent.rs (install/start/update/stop/remove logic)
- Issue:
  - User-controlled fields (ssh_target, install_path, start_command, etc.) are used to build shell commands executed over SSH.
  - While there is:
    - validate_install_path()
    - shell_quote_path()
  - The start_command is largely trusted from the frontend or derived from user input.
  - If an attacker can influence these fields (via XSS, CSRF, or direct API calls), they can inject arbitrary commands on the remote host.
- Impact:
  - Remote code execution on the SSH target machine.
- Likelihood:
  - Medium-high: depends on whether an attacker can influence these fields.
- Fix:
  - Enforce:
    - A strict allowlist/structure for start_command.
    - No raw concatenation of user input into shell commands.
    - Use a controlled wrapper script on the remote side (Command::new(...).arg(...) style), not shell interpolation.

### 3. SSRF via Attach Endpoint and Chat Proxy

- Endpoints:
  - POST /api/attach
  - POST /api/chat
- Files:
  - src/web/api.rs:3732
  - src/web/api.rs:2026
- Issue:
  - Attach endpoint:
    - Accepts endpoint from client.
    - Validates:
      - Must be http/https.
      - If IP is numeric, must be loopback or RFC1918 private.
    - But:
      - Hostnames are allowed without strict validation.
      - Attacker can supply a hostname that resolves internally (e.g., via DNS rebinding or internal DNS) to reach internal services.
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
- Fix:
  - Add:
    - Blocklist for internal ranges (10/8, 172.16/12, 192.168/16, 127/8, 169.254/16, etc.).
    - Optional DNS resolution check before attach.
    - Optional allowlist of allowed domains.

## HIGH

### 4. Overly Permissive CSP and unsafe-inline Script Usage

- Files:
  - src/web/mod.rs:34
- Issue:
  - CSP includes:
    - script-src 'self' 'unsafe-inline' https://cdn.jsdelivr.net
    - style-src 'self' 'unsafe-inline' https://fonts.googleapis.com https://cdn.jsdelivr.net
  - 'unsafe-inline' allows:
    - Inline scripts and event handlers.
    - Makes XSS more impactful.
- Impact:
  - If any XSS exists (see below), 'unsafe-inline' makes exploitation easier.
- Likelihood:
  - High: already in use.
- Fix:
  - Move to:
    - Non-inline scripts.
    - Use nonces or hashes.
    - Remove 'unsafe-inline' where possible.

### 5. Potential XSS via Markdown Rendering and DOM Insertion

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
- Fix:
  - Ensure:
    - All markdown-rendered HTML is sanitized with DOMPurify.sanitize().
    - No raw innerHTML from untrusted content.
    - Streaming content is sanitized before insertion.

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
- Fix:
  - Recommend:
    - Always use TLS (via reverse proxy or built-in).
    - Use a constant-time comparison for credentials.
    - Consider a more robust auth mechanism for LAN exposure.

## MEDIUM

### 7. Filesystem Exposure via /api/browse

- Endpoint:
  - GET /api/browse
- Files:
  - src/web/api.rs:1913
- Issue:
  - Accepts path query parameter.
  - Uses PathBuf::from(&requested) and canonicalize().
  - Allows browsing the entire filesystem.
  - Only skips dotfiles.
- Impact:
  - Any authenticated user can:
    - Read arbitrary files (config, SSH keys, tokens, etc.).
- Likelihood:
  - High: directly callable via API.
- Fix:
  - Either:
    - Restrict to a specific directory (e.g., models directory).
    - Or:
      - Add a strict allowlist of allowed root paths.
      - Reject paths outside allowed roots.

### 8. Overly Broad Database Restore and Backup Access

- Endpoints:
  - POST /api/db/restore
  - POST /api/db/backup
- Files:
  - src/web/api.rs:3292
  - src/web/api.rs:3135
- Issue:
  - Any authenticated user can:
    - List backups.
    - Restore from any backup.
    - Delete backups.
  - No separation between admin and normal user.
- Impact:
  - Data loss or tampering.
- Likelihood:
  - Medium: if multiple users share the same instance.
- Fix:
  - Introduce:
    - Admin-only endpoints or tokens.
    - Or at least confirmations and rate limits.

### 9. SSH Credentials and Agent Tokens Stored in Plaintext

- Files:
  - src/remote_ssh.rs:15
  - src/state.rs:80
- Issue:
  - SSH passwords, private key paths, passphrases, and agent tokens are stored in:
    - sessions.json
    - ui-settings.json
  - No encryption at rest.
- Impact:
  - If filesystem is compromised, all secrets are exposed.
- Likelihood:
  - Medium: depends on environment.
- Fix:
  - Consider:
    - Encrypting sensitive fields.
    - Or at least documenting and warning users.

### 10. Lack of Rate Limiting and DoS Resistance

- Scope:
  - All API endpoints.
- Issue:
  - No rate limiting.
  - Endpoints like:
    - /api/chat
    - /api/db/query
    - /api/self-update
  - Can be abused to:
    - Exhaust resources.
    - Trigger long-running operations.
- Impact:
  - Denial of service.
- Likelihood:
  - Medium: if exposed on LAN or internet.
- Fix:
  - Add:
    - Per-endpoint rate limiting.
    - Timeouts and concurrency limits.

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
   - Remove arbitrary SQL or restrict to a fixed allowlist.
2. Harden remote-agent command execution:
   - Enforce strict allowlists and avoid raw shell interpolation.
3. Mitigate SSRF:
   - Add stricter checks to attach endpoint and chat proxy.
4. Improve XSS protection:
   - Ensure all dynamic content is sanitized with DOMPurify.
5. Restrict /api/browse:
   - Limit to a specific directory or allowlist.
6. Add rate limiting:
   - Protect against DoS and brute-force attacks.
7. Encrypt sensitive data:
   - At least SSH credentials and agent tokens.
