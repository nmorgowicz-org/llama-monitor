# API Module Refactor Plan

**Date:** 2026-06-16
**Status:** Proposed
**Priority:** High
**Scope:** `src/web/api.rs`, route organization, HTTP handler boundaries

## Goal

Reduce `src/web/api.rs` from a 14,000+ line catch-all file into a navigable module tree that is easier for humans and AI agents to inspect without changing API behavior.

This is a structure-first refactor. The first implementation pass should move existing code into domain modules, preserve route paths, preserve request/response shapes, and preserve auth behavior. Service extraction can follow once the HTTP surface is easier to reason about.

## Why This Comes First

`src/web/api.rs` has become a central bottleneck:

- It mixes route assembly, authentication helpers, upstream inference plumbing, DTOs, HTTP handlers, business logic, and tests.
- Agents must load a very large file to answer narrow questions about one API area.
- Related endpoints are not consistently grouped by ownership boundary.
- The full warp route chain is already large enough that some route smoke tests are ignored because of stack overflow risk from deep filter recursion.
- New endpoint work has a high chance of creating duplicate helpers, inconsistent auth checks, or stale route wiring.

This refactor should happen before larger API or backend feature plans, because those plans will otherwise add more weight to the same overloaded file.

## Current State

As of 2026-06-16:

- `src/web/api.rs` is approximately 14,108 lines.
- Public entry points are:
  - `api_routes(...)`
  - `auth_api_routes(...)`
  - `public_tokens_routes(...)`
  - `check_api_token(...)`
- `src/web/mod.rs` imports `api::ApiError` and calls the route entry points.
- `src/web/mod.rs` owns top-level concerns such as static routes, auth guard, origin guard, helmet/CSP, and rejection recovery.
- `src/web/api.rs` owns almost every API endpoint below `/api/...`, plus some `/metrics...` routes that are nested under the API route tree by `src/web/mod.rs`.

Major handler groups currently embedded in `api.rs`:

- Auth status/login/logout and auth config
- Token bootstrap and token rotation
- Settings, GPU env, model tags, presets, templates
- Chat streaming, guided chat, suggestions, keyword generation, context-note analysis
- Chat tab persistence and chat search/archive/hide/restore
- Database admin and SQL query routes
- Sessions, attach/detach/spawn/kill, active-session readiness
- LHM and sensor bridge management
- Remote agent install/update/start/stop/remove/TLS status
- Spawn wizard import, chat template install/fetch/upload, VRAM estimation
- Benchmarking, tuning, model defaults, MoE advisor
- Hugging Face search/files/card/meta/download/token routes
- TLS config and ACME routes
- llama.cpp binary updater and restart routes
- Debug and self-update endpoints

## Non-Goals

- Do not change route paths.
- Do not change JSON field names, status codes, or auth requirements except to fix an explicitly discovered bug.
- Do not migrate from warp as part of this plan.
- Do not rewrite business logic just because it is moved.
- Do not combine this with frontend behavior changes.
- Do not use this as an opportunity to relax security checks.

## Design Principles

### Organize by Feature Domain

Use domain modules instead of one giant `api` file. A future agent looking for `/api/hf/search` should open an HF module, not scan all API code.

### Keep HTTP Adapters Thin Over Time

The first pass can be mechanical. Later passes should move heavy business logic out of route closures into service functions where that reduces complexity.

Preferred endpoint shape after the mechanical split:

```rust
pub(crate) fn routes(ctx: ApiCtx) -> ApiRoute {
    get_presets(ctx.clone())
        .or(create_preset(ctx.clone()))
        .or(update_preset(ctx.clone()))
        .or(delete_preset(ctx.clone()))
        .unify()
        .boxed()
}
```

Longer-term shape for complex endpoints:

```rust
async fn create_preset_handler(ctx: ApiCtx, auth: Option<String>, body: ModelPreset) -> ApiResult {
    require_api_token(&auth, &ctx.config)?;
    let preset = presets_service::create(&ctx.state, body)?;
    json_ok(&preset)
}
```

### Box at Module Boundaries

Warp's nested filter types can become enormous. Each domain module should return a boxed route group, and `api/mod.rs` should combine boxed groups.

Use one canonical route type:

```rust
pub(crate) type ApiReply = Box<dyn warp::reply::Reply>;
pub(crate) type ApiRoute = warp::filters::BoxedFilter<(ApiReply,)>;
```

Rules:

- Endpoint functions may return concrete `impl Filter` internally.
- Module-level `routes(ctx)` functions should return `ApiRoute`.
- `api_routes(...)`, `auth_api_routes(...)`, and `public_tokens_routes(...)` should return boxed routes or a small generic wrapper around boxed routes.
- Use `.unify()` when combining filters that all return `ApiReply`.
- Use `.boxed()` after each domain group.

### Prefer Shared Context Over Long Parameter Lists

Introduce an `ApiCtx` to carry shared state:

```rust
#[derive(Clone)]
pub(crate) struct ApiCtx {
    pub(crate) state: AppState,
    pub(crate) config: Arc<AppConfig>,
    pub(crate) auth: AuthManager,
    pub(crate) bind_host: String,
}
```

Not every module needs every field, but a shared context avoids inconsistent route constructors like `(state, app_config)`, `(app_config, auth_manager)`, `(chat_storage, app_config)`, and `(state, app_config, bind_host)`.

Keep `AppState` and `Arc<AppConfig>` names consistent across modules. Avoid introducing a second global app container outside `ApiCtx`.

## Recommended File Layout

Use a subdirectory under `src/web/api/`, not a flat pile of files next to `src/web/api.rs`.

Target layout:

```text
src/web/api/
  mod.rs
  common.rs
  auth.rs
  tokens.rs
  inference.rs
  metrics.rs
  config.rs
  presets.rs
  templates.rs
  models.rs
  hf.rs
  spawn_wizard.rs
  vram.rs
  benchmark.rs
  chat/
    mod.rs
    stream.rs
    guided.rs
    suggestions.rs
    notes.rs
    tabs.rs
  db.rs
  sessions.rs
  remote_agent.rs
  lhm.rs
  sensor_bridge.rs
  tls.rs
  llama_binary.rs
  debug.rs
  self_update.rs
```

Rationale:

- `chat/` deserves a subdirectory because chat streaming, guided generation, suggestions, notes, and tab storage are separate change surfaces.
- `common.rs` holds generic API infrastructure only.
- `inference.rs` holds upstream llama-server request helpers, not user-facing route handlers.
- `config.rs` is for settings/GPU env/model tags/token rotation config routes. It should not replace `crate::config`.
- `models.rs`, `hf.rs`, `vram.rs`, `spawn_wizard.rs`, and `benchmark.rs` are separate because spawn/model acquisition work has grown into multiple ownership boundaries.

If a module exceeds roughly 1,500 lines after the split, create a subdirectory for it immediately instead of letting a new mega-file form.

## Module Responsibilities

### `api/mod.rs`

Owns public API entry points and route composition only.

Contains:

- `pub fn api_routes(...)`
- `pub fn auth_api_routes(...)`
- `pub fn public_tokens_routes(...)`
- `ApiCtx` construction
- Domain route group composition
- Minimal re-exports required by `src/web/mod.rs`

Must not contain endpoint business logic.

### `common.rs`

Contains API-wide helpers:

- `ApiError`
- `ApiReply`
- `ApiRoute`
- `ApiResult`
- `json_reply`, `json_status`, `error_status` helpers if added
- `with_app_config`, `with_state`, `with_chat_storage` style filters if still needed
- `try_cooldown`
- bearer extraction
- token comparison helpers used by route modules

Security-sensitive token checks must continue to use constant-time comparison through the existing token-check pattern.

### `auth.rs`

Contains form/basic auth API endpoints:

- `/api/auth/status`
- `/api/auth/login`
- `/api/auth/logout`
- auth config get/put

Do not confuse this with `src/web/auth.rs`, which owns `AuthManager`.

### `tokens.rs`

Contains:

- `/api/internal-token`
- `/api/db-admin-token`
- token bootstrap allow checks
- token rotation endpoints

Keep bootstrap routes separate from normal protected routes because `src/web/mod.rs` intentionally exposes them before the top-level auth guard.

### `inference.rs`

Contains upstream llama-server helpers:

- active chat completions URL
- upstream capacity checks
- inference semaphore acquisition
- reqwest client construction
- upstream send retry behavior

This module should not define user-facing routes. It is used by `chat`, `chat/suggestions`, and possibly future inference-dependent endpoints.

### `chat/`

Recommended split:

- `chat/stream.rs`: `/api/chat`, `/api/chat/abort`
- `chat/guided.rs`: `/api/chat/guided`, thinking-stripping/sanitization helpers
- `chat/suggestions.rs`: `/api/chat/suggestions`, keyword generation, suggestion parsing
- `chat/notes.rs`: context-note analysis
- `chat/tabs.rs`: tab CRUD, append messages, reorder, search, archive/hide/restore
- `chat/mod.rs`: combines chat route groups

Keep parser tests next to the parser module. For example, suggestion parsing tests belong in `chat/suggestions.rs` or `chat/suggestions/tests.rs`.

### `sessions.rs`

Contains:

- session list/recent/create/delete
- active session get/set/readiness/capabilities
- endpoint health check
- spawn with preset
- attach/detach
- kill llama
- restore hint if it remains session-oriented

The older preset-based spawn path must remain visible in this module because it is easy to miss when changing spawn wizard behavior.

### `remote_agent.rs`

Contains remote agent routes:

- latest release
- detect
- SSH host key/trust
- install/start/update/stop/remove/status
- TLS status

Keep SSH request hydration helpers here unless they become broadly reusable.

### `db.rs`

Contains database admin routes:

- stats
- integrity
- maintenance
- backup/list/restore/delete/repair
- indexes
- SQL query

Keep high-impact operations visibly guarded by `db-admin-token` where required. Restore paths must preserve existing WAL sidecar handling behavior.

### `tls.rs`

Contains:

- TLS config get/put
- ACME request/renew
- TLS cooldowns

Keep TLS path validation in this module unless extracted into a shared TLS service.

### `hf.rs`

Contains Hugging Face and model discovery routes:

- search
- files
- community picks
- quantizers get/put
- download dir
- card/meta
- resolve origin
- token get/put/delete
- download

The constrained URL install flow for chat templates should remain in `spawn_wizard.rs` or a chat-template module, not move to `hf.rs`, because it is not a general remote-fetching endpoint.

### `llama_binary.rs`

Contains llama.cpp binary management:

- version
- latest
- releases
- release details
- platform info
- update
- restart

Keep restart auth and cooldown behavior unchanged.

## Migration Plan

### Phase 0: Baseline and Safety Checks

- [ ] Record current line count and route groups.
- [ ] Run `cargo fmt -- --check` before starting to catch pre-existing formatting drift.
- [ ] Run a focused compile check before large moves if the branch is already dirty.
- [ ] Confirm no unrelated user changes are mixed into `src/web/api.rs`.

### Phase 1: Create API Module Skeleton

- [ ] Convert `src/web/api.rs` into `src/web/api/mod.rs`.
- [ ] Add empty or minimal domain modules.
- [ ] Move `ApiError`, reply types, auth helpers, and shared filters into `common.rs`.
- [ ] Re-export only what `src/web/mod.rs` needs:
  - `ApiError`
  - `check_api_token`
  - route entry points
- [ ] Keep `src/web/mod.rs` call sites unchanged except module paths if required.

Expected validation:

```bash
cargo check
```

### Phase 2: Move Low-Risk Route Groups

Start with route groups that have limited coupling:

- [ ] `presets.rs`
- [ ] `templates.rs`
- [ ] `lhm.rs`
- [ ] `sensor_bridge.rs`
- [ ] `metrics.rs`
- [ ] `debug.rs`

For each module:

- [ ] Move handlers and directly related helpers.
- [ ] Add `pub(crate) fn routes(ctx: ApiCtx) -> ApiRoute`.
- [ ] Box the module route group.
- [ ] Wire the group from `api/mod.rs`.
- [ ] Run `cargo check`.

### Phase 3: Move Config, Models, HF, VRAM, Benchmark

Move medium-coupling groups:

- [ ] `config.rs`
- [ ] `models.rs`
- [ ] `hf.rs`
- [ ] `spawn_wizard.rs`
- [ ] `vram.rs`
- [ ] `benchmark.rs`

Guardrails:

- Preserve `safe_json_body` usage where request bodies are size-limited today.
- Keep HF repo/path validation behavior unchanged.
- Keep model download concurrency/cooldown behavior unchanged.
- Keep Windows/macOS/Linux cfg guards exactly equivalent unless there is an explicit bug fix.

Expected validation after this phase:

```bash
cargo check
npm run validate-js
```

`npm run validate-js` should not be necessary for pure Rust moves, but run it if any static API call paths or fixtures are touched.

### Phase 4: Move Chat

Move chat in smaller pieces:

- [ ] `chat/stream.rs`
- [ ] `chat/guided.rs`
- [ ] `chat/suggestions.rs`
- [ ] `chat/notes.rs`
- [ ] `chat/tabs.rs`
- [ ] `chat/mod.rs`

Guardrails:

- Do not change SSE formatting.
- Do not change inference queue timeout behavior.
- Do not change guided-generation thinking suppression behavior.
- Do not change tab persistence semantics around `thinking_content`.
- Keep `/api/chat/search` app-wide by default unless a separate product change explicitly scopes it.

Expected validation:

```bash
cargo test --lib
```

Run targeted UI tests only if frontend behavior or chat API semantics changed. For a mechanical move, Rust tests should be sufficient.

### Phase 5: Move Sessions, DB, TLS, Remote Agent, llama Binary

Move the highest-risk groups last:

- [ ] `sessions.rs`
- [ ] `db.rs`
- [ ] `tls.rs`
- [ ] `remote_agent.rs`
- [ ] `llama_binary.rs`
- [ ] `self_update.rs`

Guardrails:

- Confirm every read/write endpoint still requires the correct token.
- Confirm destructive endpoints still require `db-admin-token` or their existing elevated token.
- Preserve confirmation-field requirements.
- Preserve cooldowns.
- Preserve DB restore WAL sidecar cleanup.
- Preserve token rotation live-memory updates, not just disk writes.
- Preserve remote-agent SSH connection hydration behavior.

Expected validation:

```bash
cargo clippy -- -D warnings
cargo test --lib
```

### Phase 6: Re-enable or Replace Route Smoke Coverage

The current ignored route smoke tests exist because full `api_routes()` construction can overflow from deep warp filter recursion.

After boxing module route groups:

- [ ] Try re-enabling route smoke tests that build the full route tree.
- [ ] If full-tree tests still overflow, add per-module smoke tests instead.
- [ ] Prefer route existence/auth tests per domain module.
- [ ] Do not leave tests that pass for the wrong reason.

Recommended test shape:

```rust
#[tokio::test]
async fn presets_routes_require_auth() {
    let routes = presets::routes(test_ctx());
    let resp = warp::test::request()
        .method("GET")
        .path("/api/presets")
        .reply(&routes)
        .await;
    assert_eq!(resp.status(), 401);
}
```

### Phase 7: Final Cleanup

- [ ] Ensure `api/mod.rs` is mostly route composition.
- [ ] Ensure no domain module exceeds roughly 1,500 lines without justification.
- [ ] Ensure shared helpers are not duplicated across modules.
- [ ] Ensure docs/reference files do not need updates because behavior did not change.
- [ ] Run final validation required by AGENTS.md before any PR is marked ready.

For a pure Rust refactor touching `src/web/api/*`, minimum final validation before push:

```bash
cargo clippy -- -D warnings
cargo test
npm run validate-js
npm run lint
git diff --check
cargo build --release
cargo fmt
git status
```

If `cargo fmt` changes files, commit those changes before pushing.

## Detailed Route Group Map

This is the intended destination for current handlers.

| Current Handler/Area | Destination |
| --- | --- |
| `api_routes`, route group composition | `api/mod.rs` |
| `auth_api_routes`, auth status/login/logout | `api/auth.rs` |
| `public_tokens_routes`, internal/db-admin token bootstrap | `api/tokens.rs` |
| `ApiError`, token checks, bearer parsing, unauthorized replies | `api/common.rs` |
| upstream llama-server capacity/client/retry helpers | `api/inference.rs` |
| `/api/presets...` | `api/presets.rs` |
| `/api/templates...` | `api/templates.rs` |
| `/api/models...`, model tags, GGUF meta | `api/models.rs` |
| `/api/hf...` | `api/hf.rs` |
| `/api/vram...` | `api/vram.rs` |
| `/api/benchmark`, `/api/bench...`, tuning advisor | `api/benchmark.rs` |
| `/api/spawn-wizard...`, chat-template install/fetch/upload | `api/spawn_wizard.rs` |
| `/api/chat`, `/api/chat/guided`, abort | `api/chat/stream.rs`, `api/chat/guided.rs` |
| chat suggestions, keywords, director card parsing | `api/chat/suggestions.rs` |
| context-note analysis | `api/chat/notes.rs` |
| chat tabs/search/archive/hide/restore | `api/chat/tabs.rs` |
| `/api/db...` | `api/db.rs` |
| sessions, attach/detach/spawn/kill/readiness/capabilities | `api/sessions.rs` |
| LHM routes | `api/lhm.rs` |
| sensor bridge routes | `api/sensor_bridge.rs` |
| remote agent routes | `api/remote_agent.rs` |
| TLS/ACME routes | `api/tls.rs` |
| llama binary updater/restart | `api/llama_binary.rs` |
| debug spawn/log routes | `api/debug.rs` |
| self update route | `api/self_update.rs` |

## Security Checklist for This Refactor

For each moved endpoint, verify:

- [ ] The same token type is required as before.
- [ ] Read endpoints for user-owned data still require `api-token`.
- [ ] Mutating endpoints still require `api-token` at minimum.
- [ ] Destructive/high-impact endpoints still require `db-admin-token` where applicable.
- [ ] Confirmation fields are preserved.
- [ ] Body size limits are preserved.
- [ ] Path traversal checks are preserved.
- [ ] SQL query length/type/timeout behavior is preserved.
- [ ] Token rotation updates both disk and live `AppConfig`.
- [ ] No token comparisons are changed from constant-time helpers to `==`.
- [ ] No secret values are newly logged.
- [ ] Windows cfg behavior is unchanged unless explicitly justified.

This checklist is mandatory because mechanical refactors can accidentally move an endpoint behind the wrong helper or remove a size/auth guard.

## Testing Strategy

### Compile and Unit Tests

Run frequently during the split:

```bash
cargo check
cargo test --lib
```

Run full validation before pushing:

```bash
cargo clippy -- -D warnings
cargo test
```

### Route Smoke Tests

Route smoke tests should verify:

- Route exists.
- Missing auth returns 401 for protected endpoints.
- Wrong token returns 401/403 as before.
- Expected body parser failures return 400 rather than 404 where `safe_json_body` is used.

Per-module route smoke tests are acceptable and may be better than one full tree test if warp recursion remains expensive.

### UI Tests

For a mechanical Rust-only move, Playwright is not required unless route behavior changes. If behavior changes in chat, sessions, settings, remote-agent, or spawn wizard flows, run the isolated UI command from AGENTS.md after rebuilding release:

```bash
cargo build --release
cd tests/ui && CI=1 LLAMA_MONITOR_USE_RELEASE=1 LLAMA_MONITOR_TEST_PORT=17778 npm test
```

Never run bare `npm test` in `tests/ui`.

### Cross-Platform Check

If any moved code touches `#[cfg]` guards, `src/tray.rs`, `Cargo.toml`, platform-specific process launching, or Windows-specific behavior, run:

```bash
rustup target add x86_64-pc-windows-gnu
cargo check --target x86_64-pc-windows-gnu
```

Pure file moves with unchanged cfg blocks may not need the Windows cross-check, but it is required if cfg logic changes.

## Implementation Notes for Future Agents

- Use `git mv` or equivalent file moves where practical so history remains readable.
- Keep each commit conventional, for example:
  - `refactor(api): introduce api module skeleton`
  - `refactor(api): split preset and template routes`
  - `refactor(chat): move chat routes into api chat modules`
- Do not mix feature work into these commits.
- Prefer small commits by domain group.
- If a route move reveals a real bug, fix it in the same branch but call it out separately in the PR body. Use a `fix(...)` commit if the behavior changes.
- Avoid broad `pub` exports. Use `pub(crate)` by default.
- Keep test helpers local to modules unless at least two modules need them.
- When a module needs a helper from another module, consider whether that helper belongs in `common.rs` or whether the dependency direction is wrong.
- Run `cargo fmt` after each large move, but still run final `cargo fmt` as the last check before push.

## Success Criteria

This plan is complete when:

- `src/web/api.rs` no longer exists as a 14,000 line file.
- `src/web/api/mod.rs` is primarily route composition and public API entry points.
- Domain modules are small enough for targeted review.
- All existing routes still compile and behave the same unless an intentional fix is documented.
- Route groups are boxed at module boundaries.
- Ignored route smoke tests are either re-enabled or replaced with reliable per-module tests.
- Full required validation passes before PR readiness.
