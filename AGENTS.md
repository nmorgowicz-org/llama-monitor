# Llama Monitor Project Rules

## Conventional Commits

All commits MUST follow the [Conventional Commits](https://www.conventionalcommits.org/) format:

```
<type>(<scope>): <description>
```

### Commit Types

| Type | Purpose | Version Bump |
|------|---------|--------------|
| `feat` | New feature | MINOR |
| `fix` | Bug fix | PATCH |
| `perf` | Performance improvement | PATCH |
| `refactor` | Code refactoring (no behavior change) | No bump |
| `test` | Adding tests | No bump |
| `chore` | Maintenance tasks | No bump |
| `docs` | Documentation changes | No bump |
| `ci` | CI/CD changes | No bump |
| `revert` | Revert previous commit | Depends on reverted commit |

### Format Examples

✅ **Valid:**
- `feat: add new API endpoint`
- `feat(api): add user authentication`
- `fix: resolve memory leak in session manager`
- `fix(gpu): correct temperature reading for AMD GPUs`
- `perf: optimize GPU metrics polling`
- `refactor: simplify session state management`
- `test: add unit tests for GPU monitoring`
- `chore: update dependencies`
- `docs: add API reference documentation`
- `ci: add conventional commit validation`
- `revert: revert breaking change from v0.2.0`

### Scope Convention

Use the area being changed as scope: `api`, `ui`, `chat`, `gpu`, `nav`, `settings`, `models`, `sessions`, `wizard`, `vram`, `hf`, `binary`, `spawn`, `docs`, `ci`. When in doubt, pick the closest match — don't invent new scopes.

### PR Titles and Release Notes

- PR title MUST be `feat:` or `fix:` if the PR contains any feat/fix commits. release-please inspects PR titles, not individual commits.
- **NEVER** put `BEGIN_COMMIT_OVERRIDE` / `END_COMMIT_OVERRIDE` in git commit messages — only in PR bodies.
- When a PR ships multiple distinct user-facing `feat`, `fix`, or `perf` items, add an override block to the PR body so each appears separately in the changelog:

```text
BEGIN_COMMIT_OVERRIDE
feat(chat): add send-to-stop generation toggle
feat(chat): add assistant variant navigation and resend after user edits
fix(chat): default auto-compaction on restored tabs
END_COMMIT_OVERRIDE
```

- Use overrides more than you think you need to. Prefer several precise bullets over one vague summary line.
- Pure refactors, internal cleanup, docs-only, and CI-only changes should not appear in override blocks.
- Update the override block before merge if the PR scope changes during review.

## Multi-Platform Compatibility (MANDATORY)

This project targets **macOS, Linux, and Windows**. Development frequently happens on macOS, which means Windows-specific code paths are the most likely to silently break. Agents MUST treat all three platforms as first-class targets on every change.

### Platform Compatibility Rules

1. **Never add platform-specific code without a Windows equivalent** — If a feature works on macOS/Linux, it must either work on Windows or have an explicit `#[cfg(target_os = "windows")]` stub with a documented reason for the gap.

2. **Run a cross-compile check before every PR** — After any change to `src/tray.rs`, `Cargo.toml`, or any file with `#[cfg]` guards, run:
   ```bash
   rustup target add x86_64-pc-windows-gnu
   cargo check --target x86_64-pc-windows-gnu
   ```
   CI uses the `x86_64-pc-windows-gnu` (MinGW) target. If the check fails, fix it before pushing.

3. **Removing a `not(target_os = "windows")` guard is a common task** — The tray popover, GPU monitoring, and other subsystems use these guards to exclude Windows from macOS/Linux-only paths. When extending a feature, audit every `not(target_os = "windows")` in the affected files and determine if Windows should now be included.

4. **New `#[cfg]` guards must be justified** — When adding platform-specific code, add an inline comment explaining why it's platform-specific and what Windows gets instead (or why it can't be supported yet).

5. **Test tooltip and fallback paths on all platforms** — If code falls back gracefully when a platform feature is unavailable, the fallback must not silently swallow errors on Windows. Log a warning.

6. **`winit` features** — The `winit` dependency uses `default-features = false` with `x11` and `wayland` features. The Win32 backend is selected automatically when the build target is Windows and requires no explicit feature flag in winit 0.31. Do not add platform-specific winit feature flags unless a future winit version requires it.

7. **`wry` is universal** — After the Windows tray WebView implementation (branch `feature/windows-tray-webview-and-compat`), `wry` is a universal dependency and must not be re-scoped to `not(target_os = "windows")`.

8. **File permission hardening** — `harden_file_permissions()` is a no-op on Windows. This is a known gap (documented in `docs/plans/20260519-windows_compatibility_fixes.md` issue W-04). Do not remove the function; do not pretend it works on Windows. If adding new secret files, add a call to `harden_file_permissions()` for Unix coverage, and note the Windows gap in a comment.

9. **GPU metrics on Windows** — GPU monitoring works via `nvidia-smi` (NVIDIA) and `rocm-smi` (AMD) on Windows when those tools are in PATH. Intel GPU monitoring is not yet implemented on Windows. Document Windows requirements in any GPU-related user-visible feature work.

### Windows-Specific Architecture

| Feature | Windows Implementation | Status |
|---------|----------------------|--------|
| Tray popover | WebView2 via `wry` | In progress (this branch) |
| CPU temperature | `sensor_bridge.exe` HTTP sidecar | Working — requires elevated privileges |
| GPU metrics | `nvidia-smi` / `rocm-smi` in PATH | Working for NVIDIA/AMD |
| File permissions | No-op (security gap documented) | Known gap — see W-04 |
| Signal handling | `tokio::signal::ctrl_c()` | Sufficient for tray app |
| Dashboard open | `cmd.exe /C start <url>` | Working (used by old menu; may be reused) |

### Related Documents

- `docs/plans/20260505-windows_tray_webview.md` — Primary Windows tray implementation plan
- `docs/plans/20260519-windows_compatibility_fixes.md` — Full catalog of Windows compatibility gaps
- `docs/reference/windows-sensor-bridge-implementation.md` — Sensor bridge architecture

---

## Development Workflow

1. Create feature branch: `git checkout -b feature/my-feature`
2. Make changes and commit with conventional format
3. Push branch and create PR
4. Merge to `main` → release-please creates release PR
5. Merge release PR → tag created → release artifacts built

## Project Structure

- `src/` - Rust backend code
- `static/` - Web frontend (HTML/CSS/JS, embedded at compile time)
- `docs/` - Documentation files

## Build & Test Commands

```bash
# Build
cargo build --release

# Run tests
cargo test

# Lint
cargo clippy -- -D warnings

# Format
cargo fmt

# Validate JavaScript syntax (catches errors before browser load)
npm run validate-js

# Lint static/js with ESLint (catches XSS, no-undef, import-assign)
npm run lint

# Check for whitespace/indentation issues (CI enforces this)
git diff --check
```

### Mandatory pre-PR checks

Before pushing or marking a PR ready, agents MUST run these steps in this exact order.
Any step that modifies files must be committed before continuing.
Never rely on CI to catch issues you can catch locally.

1. `cargo clippy -- -D warnings`
   - Fix all warnings. Do not push with known clippy failures.

2. `cargo test` (or a focused subset if the full suite is too slow; CI will run it)
   - Do not push with known test failures.

3. `npm run validate-js`
4. `npm run lint`
   - Fix issues; re-run after changes.

5. `git diff --check`
   - Run AFTER all previous steps, on the final committed state.
   - If it reports issues, fix them and re-run.

6. `cargo build --release`
   - Ensures the release binary builds cleanly. Required before any screenshot or e2e run.

7. Final formatting (ABSOLUTE LAST CHECK):
   - Run: `cargo fmt`
   - If it changes files, commit them (e.g., `chore: apply cargo fmt`).
   - Do not push until formatting is committed.
   - Always run this even if you already ran it earlier.

8. Final consistency check (CRITICAL):
   - Run: `git status`
   - Ensure:
     - No uncommitted changes from fmt/clippy/lint.
     - No generated files (e.g., `src/gen/*`) left uncommitted after toolchain changes.
   - If anything is uncommitted, commit it before pushing.

9. **JS module baseline** (only if a new `.js` file was added under `static/js/` and imported from `bootstrap.js`):
   - Run: `cd tests/ui && npm run update-baseline`
   - Commit the updated `tests/ui/core/js-module-baseline.json` together with the new module.
   - Do not skip this — the baseline test will fail in CI if you do.

Hard rules:
- Never push "to see if CI passes."
- Never ignore, comment out, or trust a failing check.
- Include auto-generated files (e.g., `src/gen/*`) in every check and commit.
- If a check fails and you cannot immediately fix it, STOP and ask for clarification instead of pushing.

If any of these fail, fix the issues before pushing.

## Documentation Maintenance

Docs are updated in the same PR as the code — not as follow-up. Primary doc areas:

| Area | File |
|------|------|
| Chat, personas, compaction, guided generation | `docs/reference/chat.md` |
| REST API endpoints and data shapes | `docs/reference/api.md` |
| Dashboard, monitoring, hardware | `docs/reference/dashboard.md` |
| Remote agent and SSH | `docs/reference/remote-agent.md` |
| CLI flags | `docs/reference/cli-flags.md` |
| Spawn wizard, HF integration, binary download | `docs/reference/spawn-wizard.md` |
| VRAM estimator, ModelArch heuristics | `docs/reference/vram-estimator.md` |

**When to update:**
- New user-visible feature → add to relevant `docs/reference/<area>.md`; README if high-impact
- Changed UI label or element ID → update any doc prose + `capture.mjs` selectors in the same PR
- New or changed API field → update `docs/reference/api.md` schema; keep ChatTab/ChatMessage in sync with Rust structs
- Removed feature → remove all references and stale screenshots
- New screenshot scenario → update `tests/ui/README.md` and the `printUsage()` block in `capture.mjs`

Write reference docs as if the feature always existed. Don't say "we added X in this PR."

## Screenshot Harness

All repo-managed screenshots and animated UI captures use the single harness:

```bash
node tests/ui/capture.mjs --scenario <name>
```

**NEVER run multiple scenarios in parallel** (no `&`, no `&&`, no parallel tool calls). Each scenario launches its own monitor instance; parallel runs cause port conflicts and race conditions.

```bash
# List available scenarios
node tests/ui/capture.mjs --list-scenarios

# Run a scenario (always rebuild release binary first if static/ changed)
cargo build --release
node tests/ui/capture.mjs --scenario <name>
```

Raw output lands in `docs/screenshots/artifacts/` (gitignored). Promote files to `docs/screenshots/` manually when referencing them in README or docs.

**Key rules:**
- Always `cargo build --release` before captures when `static/` changed — the harness uses the release binary.
- When an element ID changes, update `capture.mjs` selectors in the same PR.
- Add new scenarios to `capture.mjs` (not one-off scripts); document them in `printUsage()` and `tests/ui/README.md`.
- `--no-attach` only for the `welcome` scenario. All others require a reachable `REMOTE_SERVER`.

## Playwright UI E2E Tests

Playwright UI tests are separate from the Puppeteer screenshot harness. They:

- Run anywhere (local and CI).
- Validate behavior and flows, not pixel-perfect visuals.
- Are typically run near the end of a feature branch before marking a PR ready.

> **LIVE INSTANCE PROTECTION — READ FIRST**
>
> Default Playwright behavior (`npm test` with no flags) spawns a fresh `llama-monitor`
> on port **7778**, and **kills any existing process on that port** before starting.
>
> An AI coding session, spawned model, or your active chat history may be running on 7778.
> Killing it silently terminates the model and may corrupt ongoing work.
>
> **ALWAYS specify a test port when running locally. Never run bare `npm test`.**
> Use the canonical local command below.

### How isolation works

- **Isolated (always use this)**: `run-server.mjs` starts a fresh `llama-monitor` on the
  specified port with a brand-new temp config directory. No user chat history, settings,
  or presets are used. This is correct for all agents and PR validation.
- **Attached (advanced only)**: If `LLAMA_MONITOR_UI_URL` is set, Playwright connects
  to an existing instance and uses its real config and data. Avoid for PR validation.

### Canonical local run (CI-equivalent, isolated) — USE THIS

```bash
cd tests/ui
CI=1 LLAMA_MONITOR_USE_RELEASE=1 LLAMA_MONITOR_TEST_PORT=17778 npm test
```

What each flag does:
- `CI=1` — Playwright: workers=1 (sequential), retries=2, forbidOnly=true.
- `LLAMA_MONITOR_USE_RELEASE=1` — uses `target/release/llama-monitor` instead of `cargo run` (faster; matches CI).
- `LLAMA_MONITOR_TEST_PORT=17778` — spawns the test server on 17778, leaving port 7778 untouched.

If you haven’t built a release binary yet, build it first:
```bash
cargo build --release
```

Or omit `LLAMA_MONITOR_USE_RELEASE=1` to let run-server.mjs use `cargo run` (slower):
```bash
cd tests/ui && CI=1 LLAMA_MONITOR_TEST_PORT=17778 npm test
```

### Attaching to a live instance (advanced, not default)

Only use this if you specifically want to validate against an existing running instance
and know its state (its settings, chat tabs, etc.).

```bash
# Start an instance on a dedicated port
target/release/llama-monitor --headless --port 17778

# Run tests against it (NOT isolated — uses real config)
cd tests/ui && LLAMA_MONITOR_UI_URL=http://127.0.0.1:17778 npm test
```

Do not use this for PR validation or regression checks.

### Running Locally (Useful Variants)

From repo root:

- **CI-equivalent, isolated (required for PRs):**
  `cd tests/ui && CI=1 LLAMA_MONITOR_USE_RELEASE=1 LLAMA_MONITOR_TEST_PORT=17778 npm test`
- **Debug/interactive (headed browser):**
  `cd tests/ui && LLAMA_MONITOR_TEST_PORT=17778 npm run test:headed`
- **Step-through debugger:**
  `cd tests/ui && LLAMA_MONITOR_TEST_PORT=17778 npm run test:debug`
- **SSH integration tests:**
  `cd tests/ui && npm run test:ssh` (requires `LLAMA_MONITOR_SSH_TARGET`)
- **AI-dependent tests:**
  `cd tests/ui && LLAMA_MONITOR_HAS_AI=1 npm run test:ai`

Notes:

- Tests use a fresh temporary config dir via `run-server.mjs`; they do not depend on your local `~/.config/llama-monitor/` when run in the default (isolated) mode.
- When using `LLAMA_MONITOR_UI_URL`, you are attaching to an existing instance (not isolated).

### When to Run

- Before adding the `ready-to-test` label to a PR.
- After any change that affects:
  - Chat behavior (tabs, messages, compaction, guided generation).
  - Navigation, modals, panels, or settings flows.
  - Remote-agent or SSH-related flows.
- Near the end of a feature branch, as part of final validation.

If a change is small and clearly isolated (e.g., internal refactor, docs-only), you may skip e2e, but you must still run the Rust checks and JS linting.

### CI Behavior

In CI:

- Triggered by the `ui` job when:
  - The PR has `ready-to-test` label or is from `dependabot`.
  - One or more files under `static/**` or `tests/ui/**` changed.
- Runs:
  - `npm run validate-js` (syntax).
  - `npm ci` in `tests/ui`.
  - `npx playwright install chromium`.
  - `npm test` against a freshly built release binary.
- Skips:
  - SSH integration tests unless `LLAMA_MONITOR_SSH_TARGET` is configured.
  - AI-dependent tests: excluded from CI by default because runners have no live endpoint.
    - If a test is valuable but requires AI, keep it in the suite as disabled/manual-only:
      - Gate it with `LLAMA_MONITOR_HAS_AI=1`.
      - Use `test.skip(!hasAi, 'Set LLAMA_MONITOR_HAS_AI=1 to run AI-dependent tests.')`.
      - Never make AI-dependent tests mandatory for CI.

### Known Flakiness

Some tests exhibit timing issues when run with multiple workers in parallel. CI mitigates this via:
- `workers: 1` — single worker (sequential execution)
- `fullyParallel: false` — no parallel test execution
- `retries: 2` — failed tests retry up to 2 times

**Debugging flaky failures:**
1. Run with CI-equivalent flags first: `cd tests/ui && CI=1 LLAMA_MONITOR_TEST_PORT=17778 npm test`
2. If that passes, the local failure was a parallel timing race — not a real bug
3. Run the test in isolation to confirm: `cd tests/ui && LLAMA_MONITOR_TEST_PORT=17778 npx playwright test --workers=1 --grep "test name"`
4. Do not mark PR as ready-to-test if a CI-equivalent run fails — that is a real bug

**Cargo test flakiness from concurrent processes:**
Rust unit tests that write to shared temp paths (e.g., `std::env::temp_dir()`) can fail
with "No such file or directory" when two separate `cargo test` processes run at the same
time (e.g., you have a background build running while launching another test run). Always:
- Ensure no other `cargo test` or `cargo build` processes are running before `cargo test`
- Use PID-unique temp paths in tests: `temp_dir().join(format!("name-{}", std::process::id()))`
- Never use a shared, collision-prone path like `temp_dir().join("llama-monitor-test")`

### Maintenance Rules

UI e2e tests are first-class and must be kept in sync with the application. Agents MUST treat them similarly to reference documentation: update them as part of the same PR that changes the feature.

IMPORTANT: When running Playwright tests locally, ALWAYS:
- Use LLAMA_MONITOR_TEST_PORT=17778 (or similar) to avoid killing the live llama-monitor
- Rebuild release first when UI code changed, as tests run against the current binary.

#### Must Update

Update e2e tests when:

- A UI element’s ID, label, or structure changes that tests rely on.
- A flow is modified (e.g., new step, removed step, changed default behavior).
- A feature is renamed or moved (e.g., tab name, settings section, menu item).
- A new non-trivial feature or flow is added (see “When to Add” below).

#### Must Prune

Remove or adjust tests when:

- A feature is removed or deprecated.
- A test no longer reflects the intended behavior of the app.
- A test is flaky and depends on transient or external behavior that is not stable or testable.

Never leave dead tests that pass for the wrong reasons or test removed functionality.

#### When to Add

Add or extend e2e tests when:

- The feature is:
  - Visually or functionally significant (new modal, panel, sidebar, chat mode).
  - Easy to regress (multi-step flows, configuration toggles, persistence).
- The test can be:
  - Deterministic and fast.
  - Independent of AI responses (preferred), or:
    - If it requires AI, keep it disabled and manual-only:
      - Gate with `LLAMA_MONITOR_HAS_AI=1`.
      - Use `test.skip(!hasAi, 'Set LLAMA_MONITOR_HAS_AI=1 to run AI-dependent tests.')`.
      - Never make it mandatory for CI.

Prefer:

- Short, focused tests that validate key behaviors.
- Tests that assert meaningful outcomes (e.g., “chat tab created”, “settings persisted”, “search filters results”) rather than internal implementation details.

Avoid:

- Tests that:
  - Rely on exact visual layout or pixel positions (use screenshot harness for that).
  - Depend on external AI responses without being gated and idempotent.
  - Are so tightly coupled to internal structure that every refactor breaks them.
  - Require AI and are not disabled via `LLAMA_MONITOR_HAS_AI=1` (CI must never fail because a runner lacks a live endpoint).

If a change is minor (e.g., small UX tweak, internal refactor, non-user-facing fix) and existing tests already cover the broader area, you can skip adding a new test.

### Performance Baseline

`tests/ui/core/performance.spec.js` tracks the number of JS files loaded on cold page load. The ceiling is stored in `tests/ui/core/js-module-baseline.json`.

**Mandatory:** Whenever you add a new `.js` file under `static/js/` that gets imported (directly or transitively) from `static/js/bootstrap.js`, you MUST update the baseline. The test will fail with a message telling you to run:

```
cd tests/ui && npm run update-baseline
```

The script does a headless cold load against the running test server, counts the actual JS files, and writes the new ceiling to `js-module-baseline.json`. You must commit both the new module AND the updated baseline file together.

**Do not skip this step.** The baseline count must match the actual number of JS modules loaded. If CI fails on the performance test, update the baseline and amend the commit.

### Relationship to Screenshot Harness

- Playwright tests:
  - Purpose: validate functional behavior and flows.
  - Environment: local and CI.
- Puppeteer screenshot harness (`capture.mjs`):
  - Purpose: capture reference visuals for docs and design.
  - Environment: local only, requires a release binary and (often) a remote llama.cpp server.

Do not conflate them:

- When a feature’s behavior changes:
  - Update Playwright tests if it affects automated flows.
  - Update screenshot scenarios only if visuals/docs are impacted.

## Static Asset Registration (AUTO-GENERATED)

All static files (JS, CSS, HTML, etc.) are embedded at compile time via `include_str!` macros. **Registration is automatic** — `build.rs` scans `static/` and generates:

- `src/gen/static_assets.rs` — `include_str!` constants for each file
- `src/gen/routes.rs` — warp route filters for each file

### Adding a New Static File

1. Add the file to `static/` (e.g. `static/js/features/new-file.js`)
2. Run `cargo build` — `build.rs` regenerates the constants and routes automatically
3. Commit both your new file AND the updated `src/gen/*.rs` files

**That's it.** No manual registration needed. The build system handles everything.

### Constant Naming Convention

The generator follows this convention (match it when referencing constants):

| File Path | Generated Constant |
|-----------|-------------------|
| `css/tokens.css` | `CSS_TOKENS` |
| `css/cards-inference.css` | `CSS_CARDS_INFERENCE` |
| `js/bootstrap.js` | `BOOTSTRAP_JS` |
| `js/compat/globals.js` | `COMPAT_GLOBALS_JS` |
| `js/features/nav.js` | `FEATURES_NAV_JS` |
| `js/features/chat-render.js` | `FEATURES_CHAT_RENDER_JS` |
| `index.html` | `INDEX_HTML` |
| `manifest.json` | `MANIFEST_JSON` |
| `icon.svg` | `ICON_SVG` |

Rules:
- **CSS**: `CSS_` + filename stem, hyphens → underscores, uppercase
- **JS**: skip `js/` prefix, join remaining path parts with `_`, replace `.` and `-` with `_`, uppercase
- **Root files**: filename with `.` and `-` replaced by `_`, uppercase

### Special Cases

- **index.html**: Constant generated, but route handled specially in `mod.rs` (version/platform injection)
- **Generated files**: Committed to git for code review and incremental builds. Marked with `// AUTO-GENERATED` header.

### JavaScript Linting

Always run `npm run lint` after modifying any `.js` files under `static/js/`. This runs ESLint with three rules:
- `no-import-assign` — catches assignment to ES module namespace bindings (the `TypeError: Assignment to constant variable` class of error)
- `no-undef` — catches bare references to functions no longer on `window` after ES module extraction
- `no-unsanitized/property` and `/method` — catches `innerHTML`/`insertAdjacentHTML` with unescaped user data (XSS); `escapeHtml()` is the approved sanitizer

The lint job runs automatically in CI on every PR push that touches `static/**` or `tests/ui/**`, without requiring the `ready-to-test` label. Also run `npm run validate-js` for syntax-only validation on all JS files.

## Pre-PR Validation

For any PR touching multiple files or adding features, run a sub-agent cross-cutting check for: CSS selector duplication / specificity conflicts, missing `prefers-reduced-motion` overrides on new animations, missing `[data-theme="light"]` overrides on new styled elements, broken JS→HTML→CSS cross-module references, backend-frontend API contract mismatches, and stale code left from refactoring. Critical issues (broken functionality, silent overrides) must be fixed before the PR is marked ready.

## CI/CD Workflow

- **CI triggers**: only when a PR has the `ready-to-test` label
- **PR title**: must be conventional commit format (`feat:`, `fix:`, etc.)
- **CI checks**: `cargo fmt -- --check`, `cargo clippy -- -D warnings`, `cargo test`, `cargo build --release`
- **Releases**: release-please auto-creates release PRs when `feat:`/`fix:` merges to `main`. `feat:` → MINOR bump, `fix:` → PATCH. Preserve the release-please body in GitHub Releases; don't replace it with auto-generated notes.

## Security Requirements (MANDATORY)

All changes MUST follow these rules. Treat them as hard constraints, not suggestions.

### Canonical Security References

Use the current code and `docs/reference/` as the primary sources of truth for active behavior.

- `docs/reference/tls-architecture.md` describes the current TLS / ACME / mTLS design.
- `docs/archive/security/20260515-security-audit.md` is an audit record and remediation history.
- `docs/archive/security/20260516-tls_acme_implementation.md` is implementation history for the TLS/ACME rollout.

Archive docs are useful context, but they are not the canonical definition of current behavior unless the code and reference docs still match them.

### 1) Threat model (always assume)

- Local-first, but:
  - Likely exposed to LAN.
  - Possibly exposed via reverse proxy or port forwarding.
- Assume:
  - Any HTTP client on the network can send crafted requests.
  - Chat/model responses are untrusted.
  - Multiple llama-monitor instances may connect to the same agent or llama-server.

### 2) Authentication and authorization

- Any endpoint that **writes, deletes, or modifies** user data requires at minimum `api-token`.
- Any endpoint that **reads user-owned data** (settings, presets, templates, chat content, configuration) also requires `api-token`. “Read-only” does not mean “unauthenticated.”
- Any endpoint that:
  - Starts/stops/kills processes,
  - Restores/deletes DB backups,
  - Changes security-sensitive config (TLS, tokens, SSH),
  - Exposes secrets or tokens
  must:
  - Require an appropriate token:
    - `api-token` for general operations.
    - `db-admin-token` for high-impact/irreversible operations.
  - Never rely on “same machine” or “same browser” as security.
- New destructive or elevated endpoints:
  - Must use `db-admin-token` or a dedicated elevated token.
  - Must include a confirmation field (e.g., `{ “confirm”: “action” }`) to prevent accidental use.
- **Token rotation**: rotating a token MUST update both the on-disk file AND the live in-memory `AppConfig` atomically. Writing to disk only is a silent security failure — the old token keeps working indefinitely. See `api_rotate_agent_token` as the canonical implementation pattern.
- **Multi-level auth endpoints**: if an endpoint accepts either `api-token` or `db-admin-token` (e.g. to grant elevated access), the gate must accept either value — never require `api-token` first and then check `db-admin-token` separately, because a bearer cannot equal both simultaneously.
- When in doubt:
  - Prefer stricter auth (`db-admin-token`) over weaker (`api-token`).

### 3) Input validation

- Treat all user input as untrusted:
  - HTTP params, JSON fields, WebSocket messages, chat content, model outputs.
- For file paths:
  - Never trust raw user input.
  - Rules:
    - Reject "..", leading "/", leading "\\", and embedded path separators when expecting a filename.
    - Canonicalize and confirm the final path is within an allowed root (e.g., backups/, models_dir, TLS paths).
- For SQL:
  - Only allow expected statement types (e.g., SELECT/PRAGMA).
  - Enforce:
    - Max SQL length (e.g., 16KB).
    - Execution timeout (e.g., 10s).

### 4) XSS and DOM insertion (frontend)

- Never:
  - Use innerHTML or insertAdjacentHTML with:
    - Model responses,
    - Chat messages,
    - User-provided strings,
    - Arbitrary JSON fields.
- Must:
  - Use textContent when inserting plain text from dynamic data.
  - Use DOMPurify on HTML rendered from markdown or untrusted content.
- For metrics/numbers:
  - Use textContent instead of innerHTML unless you explicitly need HTML and have sanitized.

### 5) Rate limiting and DoS resistance

- For any endpoint that:
  - Is expensive,
  - Affects the running system (kill, restart),
  - Touches the DB or agent
  you must:
  - Add:
    - A reasonable global or per-endpoint rate limit.
    - A cooldown between calls (e.g., 10–60 seconds).
    - Body size limits (e.g., 256KB–2MB) where appropriate.
- For long-running operations:
  - Use timeouts (e.g., 10–30 seconds).
  - Fail fast with a clear error instead of hanging.

### 6) Protocol compatibility (remote agent and multi-instance)

- When changing agent/dashboard protocol, metrics, or data formats:
  - Use #[serde(default)] on all deserialized fields from the agent.
  - Never allow a single missing field to fully disconnect the agent.
  - Implement:
    - A protocol_version in agent /info.
    - A minimum enforced version in the dashboard.
    - Degraded mode:
      - Keep the agent connected.
      - Log a warning.
      - Disable only affected features instead of silently breaking.
- Assume:
  - Agents update slower than the dashboard.
  - Multiple instances may coexist.

### 7) Secrets handling

- Never:
  - Log full tokens, passwords, or keys.
  - Store secrets in plaintext on disk when encryption is available.
- Must:
  - Use existing encryption helpers for sensitive config.
  - Mask secrets in APIs meant for general UI consumers.
  - Provide a “full” or “admin” endpoint for real tokens when needed, protected by auth.

### 8) Security implementation patterns

These are concrete implementation rules. Violations in these areas have caused real bugs. Follow them exactly.

#### Randomness
- **Primary: use `getrandom::getrandom()`** for any security-sensitive random bytes: tokens, nonces, session IDs, CSP nonces, key material.
- **Use `rand_core::OsRng` only when a concrete RNG trait object is required** (e.g. argon2 `SaltString::generate`), never as a general-purpose random source.
- **Never derive security values from** timestamps, PIDs, thread IDs, or any other predictable system state — even as a "fallback." A predictable nonce or token defeats its entire purpose.
- Both `getrandom` and `rand_core::OsRng` work cross-platform including Windows. There is no valid reason for a non-system RNG path in security code.

#### Token comparison
- **Always use `subtle::ConstantTimeEq`** (already a dependency via `subtle` crate) when comparing tokens, passwords, or any secret string.
- Plain `==` on `&str` or `String` is not constant-time and leaks timing information that can be used to brute-force short secrets over a network.
- The `check_api_token` helper uses this pattern — call it instead of doing inline `==` comparisons.

#### Key derivation
- Use **HKDF-SHA-256** (via the `hkdf` crate) for any new key derivation. SHA-1 with a static salt is not acceptable for new code.

#### SQLite file operations
- **Never use `std::fs::copy` or any direct file operation on `chat.db`** while a `ChatStorage` instance is alive. The live `Connection` holds the file open; overwriting it on disk corrupts the connection's page cache and diverges the database state.
- **Always use the `ChatStorage::backup()` method** (SQLite online backup API) when copying the database. It is safe to run against an open connection.
- **WAL sidecar files**: when restoring a database, the `-wal` and `-shm` files from the running instance must be deleted or the restored database will be silently corrupted when SQLite replays the stale WAL on next open. Any restore path must handle these files explicitly.
- **`ChatStorage` is a single long-lived `Mutex<Connection>`**: operations like `VACUUM` and `ANALYZE` hold this mutex for their full duration, blocking all other DB calls. Do not call these from a hot path. Run them in background maintenance tasks only.

#### CSRF origin checks
- String-matching an Origin header against a bind address requires exact prefix/suffix logic. Off-by-one slicing (e.g. slicing `”0.0.0.0:”` as `”0.0.”`) creates bypass opportunities. Test CSRF guards with adversarial origin values before considering them correct.

### 9) Security checklist (before marking PR ready)

Before marking a PR ready-to-test, the agent MUST verify every item below. This checklist exists because these exact categories have produced shipped bugs.

**Mandatory review pass**: Run `/security-review` on the branch before filing the PR. If the branch has broad changes across multiple subsystems, also run `/review`. These catch logic-level issues the checklist alone cannot.

- [ ] **Auth on all user-data endpoints**: every new GET endpoint that returns user-owned data (settings, presets, templates, chat, config) requires `api-token`. "Read-only" does not mean unauthenticated.
- [ ] **Auth on all mutating endpoints**: every new POST/PUT/PATCH/DELETE requires at minimum `api-token`; destructive or irreversible operations require `db-admin-token`.
- [ ] **Token rotation updates in-memory state**: if rotating a token, the live `AppConfig` is updated atomically alongside the on-disk file.
- [ ] **No `==` on secrets**: all token/credential comparisons use `subtle::ConstantTimeEq`, not `==`.
- [ ] **No predictable randomness**: all new nonces, tokens, session IDs use `getrandom::getrandom()` (or `rand_core::OsRng` when a trait RNG is required). No timestamp/PID fallbacks.
- [ ] **No direct file ops on live SQLite**: use `ChatStorage::backup()` API; handle WAL sidecars on restore.
- [ ] **All new file paths from user input** are validated (reject `..`, leading `/`, `\`) and canonicalized within an allowed root.
- [ ] **No new innerHTML/insertAdjacentHTML** with untrusted data; use `textContent` or `DOMPurify`.
- [ ] **Long-running or expensive operations** have a timeout, size limit, and rate limit or cooldown.
- [ ] **Agent/protocol fields** use `#[serde(default)]` and degraded-mode behavior on missing fields.
- [ ] **Secrets are not logged** or returned in error messages.
- [ ] **Reference docs** are updated to match new auth requirements, token types, and constraints.

If any item is unclear or not satisfied, the agent MUST either add the missing protection or explicitly document the gap and rationale in the PR description.

## Security & Auth Patterns (MANDATORY)

These patterns supplement the rules in [Security Requirements](#security-requirements-mandatory) with
concrete implementation helpers and code-level conventions. Violations have caused real shipped bugs.

### 1) Auth layers (do not mix)

- UI auth (Form Login / Basic Auth):
  - Enforced by auth_guard in mod.rs.
  - Protects the browser UI and general routes.
- API auth (api-token / db-admin-token):
  - Enforced per-endpoint in api.rs.
  - Protects programmatic access and internal API calls.
- Rules:
  - Any endpoint that reads or modifies user-owned data (settings, presets, templates, chat, config) MUST require api-token.
  - Destructive or high-impact operations MUST require db-admin-token.
  - “Read-only” does NOT mean unauthenticated.
  - Never rely solely on the UI auth_guard to protect API endpoints.

### 2) auth_guard behavior

- auth_guard allows:
  - No auth configured (local-first).
  - Valid session cookie (Form Login).
  - Valid Basic Auth header.
  - Valid api-token (Bearer).
- When adding new API endpoints:
  - Assume callers may use api-token only (no browser session).
  - Ensure your endpoint:
    - Is reachable when auth_guard is satisfied via api-token.
    - Performs its own check_api_token / bearer_matches_api_token.

### 3) Token comparison

- Always use constant-time comparison for tokens:
  - bearer_matches_api_token(bearer, cfg)
  - bearer_matches_db_admin_token(bearer, cfg)
- Never use == or != directly on token strings.

### 4) Rate limiting and brute-force resistance

- For:
  - Login endpoints.
  - Expensive or destructive operations.
  - Any endpoint callable without strong auth.
- You MUST add:
  - A reasonable cooldown (e.g., 2–10 seconds) or rate limit.
- Use existing patterns (try_cooldown / AtomicU64) instead of inventing new ones.

### 5) Tests are mandatory for auth changes

- When you:
  - Add a new endpoint.
  - Change auth behavior.
  - Introduce a new auth mode or token.
- You MUST:
  - Add or update tests in tests/auth_routing.rs covering:
    - No auth mode.
    - api-token-only caller.
    - Form Login / Basic Auth enabled.
- If a change affects how auth_guard or origin_guard interacts with a route, add a test that explicitly validates that behavior.

If any item is unclear or not satisfied, the agent MUST either add the missing protection or explicitly document the gap and rationale in the PR description.

## API, Serialization, and Data Safety Rules (MANDATORY)

These rules prevent the class of bug where a JSON mismatch or auth guard silently turns into 404 and causes data loss (e.g., losing a chat tab).

### 1) Serde defaults on API/DB structs

- All structs that are:
  - Sent/received via HTTP, or
  - Stored in DB and serialized
- MUST use #[serde(default)] on fields that:
  - Have a sensible default (0, 0.0, false, empty string, empty array, etc.).
- Rationale:
  - A single missing field must not cause a hard deserialization failure.
  - This prevents 404/500 from minor frontend-backend mismatches.
- When changing such structs:
  - Assume older clients and older DB rows coexist.
  - Prefer adding fields with default over breaking existing payloads.

### 2) JSON parse errors must be 400, never 404

- A malformed or mismatched request body must:
  - Return 400 (Bad Request) with a short error.
- A missing resource (tab, backup, etc.) must:
  - Return 404.
- Never let warp’s default rejection mapping turn a JSON parse error into 404.
- When adding a new endpoint with warp::body::json:
  - Ensure:
    - Parse failures → 400
    - “Not found” → 404
  - If unsure, add a small test that sends invalid JSON and asserts 400.

### 3) No silent data loss on HTTP errors (frontend)

When handling errors from the backend, never silently delete user data based on a single ambiguous error (especially 404).

Rules:
- For operations like:
  - PUT /api/chat/tabs/:id
  - PATCH /api/chat/tabs/:id/meta
  - Any request that updates user-owned data:
- On failure:
  - If 404:
    - Log it.
    - Retry 2–3 times with short backoff.
    - Only consider removal if:
      - Every retry is 404, and
      - The response explicitly indicates “not_found” (e.g. via error field).
  - On other errors:
    - Keep data in local state.
    - Continue retrying silently.
- For destructive operations (delete tab, delete backup, etc.):
  - Require an explicit confirmation step in the UI.
  - Never infer “delete” from unexpected errors.

### 4) Contract and auth tests are mandatory for API changes

When you:
- Change an API struct (add/remove/renamed field).
- Change auth, guards, or route wiring.
- Introduce a new endpoint.

You MUST:
- Run:
  - cargo test
  - tests/auth_routing.rs
- Add or update:
  - A test for the new/changed endpoint’s auth behavior.
  - (Recommended) A quick test that sends partial/legacy JSON to ensure robustness.

If any item is unclear or not satisfied, the agent MUST either add the missing protection or explicitly document the gap and rationale in the PR description.

## VRAM Estimator: Adding New Model Architectures

When a new model family is released, update `src/llama/vram_estimator.rs`. Full field-by-field guidance and the data-gathering workflow is in `docs/reference/vram-estimator.md`. Key pitfalls that have caused real bugs:

- **"A3B" / "A4B" / "A10B" suffixes** are active **parameter** counts, not expert counts. Get the actual expert count from "routed + shared" in the model card.
- **Hybrid DeltaNet models** (Qwen3.5, Qwen3.6): `n_attn_layers` must be set or KV cache inflates to total layers (4× too many), causing max-context to be severely underestimated.
- **Gemma4 vs Gemma3**: Gemma4 uses `global_head_dim = 512` for global layers and 1024-token sliding window; Gemma3 is different. Use separate heuristics.
- **Sliding window vs DeltaNet**: Different mechanisms. Never set `local_attn_window` on DeltaNet models — use `n_attn_layers` + `linear_attn_state_bytes`.
- Every new `_arch()` or `_heuristic()` function requires a `#[test]` citing the source URL and asserting every relevant field. More specific detection patterns must come before general ones in `from_name_and_params()`.
