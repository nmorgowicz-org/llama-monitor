# Security Requirements (Full Reference)

All changes MUST follow these rules.

## Canonical References
- `docs/reference/tls-architecture.md` — TLS / ACME / mTLS design
- `docs/archive/security/20260515-security-audit.md` — Audit record
- `docs/archive/security/20260516-tls_acme_implementation.md` — Implementation history

## 1) Threat Model
- Local-first, but exposed to LAN and possibly via reverse proxy.
- Assume:
  - Any HTTP client on the network can send crafted requests.
  - Chat/model responses are untrusted.
  - Multiple llama-monitor instances may connect to the same agent or llama-server.

## 2) Authentication and Authorization
- All data-reading endpoints (settings, presets, templates, chat, config) require `api-token`.
- All write/delete endpoints require `api-token` minimum.
- High-impact/irreversible operations (kill, restore, TLS, tokens, SSH) require `db-admin-token`.
- New destructive/elevated endpoints:
  - Must use `db-admin-token` or dedicated elevated token.
  - Must include a confirmation field (e.g., `{ "confirm": "action" }`).
- Token rotation:
  - MUST update both on-disk file AND live in-memory `AppConfig` atomically.
  - See `api_rotate_agent_token` as the canonical pattern.
- Multi-level auth endpoints:
  - If accepting either `api-token` or `db-admin-token`, the gate must accept either value.
  - A bearer cannot equal both simultaneously.
- When in doubt: prefer stricter auth.

## 3) Input Validation
- All user input is untrusted.
- File paths:
  - Reject "..", leading "/", leading "\\", embedded path separators for filenames.
  - Canonicalize and confirm within allowed root.
- SQL:
  - Only allow expected statement types (SELECT/PRAGMA).
  - Enforce max length (16KB) and execution timeout (10s).

## 4) XSS and DOM Insertion
- Never use innerHTML/insertAdjacentHTML with:
  - Model responses, chat messages, user strings, arbitrary JSON.
- Must:
  - Use textContent for plain text from dynamic data.
  - Use DOMPurify for HTML rendered from markdown or untrusted content.

## 5) Rate Limiting and DoS Resistance
- For expensive, system-affecting, or DB/agent-touching endpoints:
  - Add per-endpoint rate limit or cooldown (10-60s).
  - Add body size limits (256KB-2MB).
  - For long-running ops: 10-30s timeout; fail fast with clear error.

## 6) Protocol Compatibility
- When changing agent/dashboard protocol, metrics, or data formats:
  - Use `#[serde(default)]` on all deserialized fields from the agent.
  - Implement:
    - A `protocol_version` in agent /info.
    - A minimum enforced version in the dashboard.
    - Degraded mode: keep agent connected, log warning, disable affected features only.
- Assume agents update slower than the dashboard; multiple instances may coexist.

## 7) Secrets Handling
- Never:
  - Log full tokens, passwords, or keys.
  - Store secrets in plaintext on disk when encryption is available.
- Must:
  - Use existing encryption helpers.
  - Mask secrets in APIs for general UI consumers.
  - Provide a "full"/"admin" endpoint for real tokens when needed, protected by auth.

## 8) Security Implementation Patterns

### Randomness
- Use `getrandom::getrandom()` for security-sensitive bytes: tokens, nonces, session IDs, CSP nonces, key material.
- Use `rand_core::OsRng` only when a concrete RNG trait is required (e.g., argon2).
- Never derive security values from timestamps, PIDs, or thread IDs — even as "fallback."

### Token Comparison
- Always use `subtle::ConstantTimeEq` for tokens, passwords, or any secret string.
- Plain `==` leaks timing information.
- Call `check_api_token` helper instead of inline `==`.

### Key Derivation
- Use HKDF-SHA-256 (via `hkdf` crate). SHA-1 with static salt is not acceptable.

### SQLite File Operations
- Never use `std::fs::copy` or direct file ops on `chat.db` while `ChatStorage` is alive.
- Always use `ChatStorage::backup()` API.
- WAL sidecar files:
  - On restore, delete `-wal` and `-shm` files or database is corrupted.
- `ChatStorage` is a single long-lived `Mutex<Connection>`:
  - VACUUM/ANALYZE block all DB calls — run in background maintenance only.

### CSRF Origin Checks
- Off-by-one slicing creates bypass opportunities.
- Test CSRF guards with adversarial origin values.

## 9) Auth Layers (Do Not Mix)

- UI auth (Form Login / Basic Auth):
  - Enforced by `auth_guard` in mod.rs.
- API auth (`api-token` / `db-admin-token`):
  - Enforced per-endpoint in api.rs.
- Rules:
  - "Read-only" does NOT mean unauthenticated.
  - Never rely solely on `auth_guard` to protect API endpoints.
  - Assume API callers may use api-token only (no browser session).

## 10) Rate Limiting and Brute-Force Resistance
- For login endpoints, expensive/destructive ops, or any endpoint callable without strong auth:
  - Add cooldown (2-10s) or rate limit.
  - Use existing patterns (try_cooldown / AtomicU64).

## 11) Tests Mandatory for Auth Changes
- When adding/changing endpoints or auth:
  - Add/update tests in `tests/auth_routing.rs` covering:
    - No auth mode.
    - api-token-only caller.
    - Form Login / Basic Auth enabled.

## 12) Security Checklist (Before Marking PR Ready)

Run `/security-review` on the branch before filing the PR. For broad changes, also run `/review`.

- [ ] Auth on all user-data endpoints (GETs require api-token)
- [ ] Auth on all mutating endpoints (POST/PUT/PATCH/DELETE require api-token or db-admin-token)
- [ ] Token rotation updates in-memory state atomically
- [ ] No `==` on secrets (use ConstantTimeEq)
- [ ] No predictable randomness (use getrandom)
- [ ] No direct file ops on live SQLite (use ChatStorage::backup())
- [ ] New file paths validated and canonicalized
- [ ] No new innerHTML/insertAdjacentHTML with untrusted data
- [ ] Long-running/expensive ops have timeout, size limit, rate limit
- [ ] Agent/protocol fields use #[serde(default)] with degraded mode
- [ ] Secrets not logged or returned in error messages
- [ ] Reference docs updated
