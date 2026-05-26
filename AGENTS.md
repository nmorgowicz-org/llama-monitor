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

Scopes should describe the **component/area** being changed, not the type of change:

| Scope | Purpose |
|-------|---------|
| `api` | Backend API endpoints, routes |
| `ui` | Frontend layout, styling, components |
| `chat` | Chat features, message rendering |
| `gpu` | GPU monitoring, metrics |
| `nav` | Navigation, sidebar, tab switching |
| `settings` | Settings modal, preferences |
| `models` | Model presets, configuration |
| `sessions` | Session management, persistence |
| `docs` | Documentation files |
| `ci` | CI/CD, workflows, build scripts |

**Good scope usage:**
- `fix(nav): modal navigation state broken after export`
- `docs(ui): add screenshots for chat features`
- `feat(chat): add message edit/regenerate actions`

**Avoid:**
- `fix(modal): navigation broken` - too vague
- `fix: fix: modal navigation` - redundant

❌ **Invalid:**
- `Update README` (missing type)
- `feat add new feature` (missing colon)
- `fix: bug fix` (no description after colon)
- `Fix bug in GPU monitoring` (wrong format)

### Branch Naming

When creating branches for releases, use the format:

```
release/v<version>
```

Examples:
- `release/v0.2.0`
- `release/v1.0.0`

### AI Agent Guidelines

1. **Always use conventional commit format** - Never commit without a type prefix
2. **Use lowercase** - `ci:` not `CI:`, `feat:` not `Feat:`
3. **Use parentheses for scope** - `feat(api):`, not `feat api:`
4. **Describe what changed** - Be specific about the change
5. **Chores don't bump version** - `chore:`, `refactor:`, `ci:`, `docs:` don't trigger version bumps
6. **feat and fix bump versions** - Only these types trigger semantic version increments
7. **PR titles must reflect the most significant change** - If a PR contains any `fix:` or `feat:` commits, the PR title MUST also be `fix:` or `feat:`. release-please only inspects PR titles, not individual commits. Never use `refactor:`, `chore:`, or `docs:` as a PR title if the PR includes bug fixes or features.
8. **When one PR contains multiple releasable user-facing changes, provide release-please overrides** - If a PR includes more than one distinct `feat`, `fix`, or `perf` item that should appear separately in release notes, the agent MUST add a `BEGIN_COMMIT_OVERRIDE` / `END_COMMIT_OVERRIDE` block to the PR body before merge. Inside that block, include one conventional-commit line per release note entry, for example:

```text
BEGIN_COMMIT_OVERRIDE
feat(chat): add context compaction controls
fix(chat): preserve previous compaction tombstones across retries
perf(ui): reduce chat render work during compaction
END_COMMIT_OVERRIDE
```

9. **Use overrides more often than you think** - If the branch contains several meaningful user-visible commits, do not collapse them into one generic `feat:` or `fix:` line just because the PR is being squash-merged. Prefer a richer override block whenever the work naturally breaks into multiple release-note bullets.
10. **Derive override entries from the branch's releasable commits** - Before merge, review the PR commits and identify the distinct user-facing `feat`, `fix`, and `perf` items. The override block should summarize those outcomes, not internal refactor steps, and should usually track the major releasable commits on the branch.
11. **Prefer several precise bullets over one vague bullet** - If a PR ships multiple UI improvements, chat behavior changes, or fixes, list them separately in the override block. Example:

```text
BEGIN_COMMIT_OVERRIDE
feat(chat): add send-to-stop generation toggle
feat(chat): add assistant variant navigation and resend after user edits
fix(chat): default auto-compaction on restored tabs
fix(ui): restore endpoint status interaction after module extraction
END_COMMIT_OVERRIDE
```

12. **Do not include non-user-facing maintenance unless it matters to release notes** - Pure refactors, internal cleanup, docs-only changes, test-only changes, and CI-only changes should usually stay out of the override block unless they have a direct user-visible effect worth calling out.
13. **Do not rely on intermediate commit messages for release notes** - Because PRs are typically squash-merged, release-please usually sees the merged PR title/body, not every branch commit. If multiple entries are needed in the changelog, use the PR body override block above.
14. **PR body overrides should be updated before merge, not after** - If the scope of the PR changes during review, the agent should revise the override block so the final merged PR body matches what actually shipped.

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

1. `cargo fmt`
   - If it changes files, commit them (e.g., `chore: apply cargo fmt`).
   - Do not push until formatting is committed.

2. `cargo clippy -- -D warnings`
   - Fix all warnings.
   - If fixes introduce new formatting changes, run `cargo fmt` again and commit.

3. `cargo test` (or a focused subset if the full suite is too slow; CI will run it)
   - Do not push with known test failures.

4. `npm run validate-js`
5. `npm run lint`
   - Fix issues; re-run after changes.

6. `git diff --check`
   - Run this AFTER all previous steps, on the final committed state.
   - If it reports issues, fix them and re-run.

7. Final consistency check (CRITICAL):
   - Run: `git status`
   - Ensure:
     - No uncommitted changes from fmt/clippy/lint.
     - No generated files (e.g., src/gen/*) left uncommitted after toolchain changes.
   - If anything is uncommitted, commit it before pushing.

If any of these fail, fix the issues before pushing.

## Documentation Maintenance

Keeping docs accurate is a first-class concern. Agents MUST update documentation when features change — not as a follow-up, but as part of the same PR.

### When Docs Must Be Updated

Update documentation whenever a change falls into any of these categories:

| Change Type | Required Doc Updates |
|-------------|---------------------|
| New user-visible feature | `docs/reference/<area>.md`, README feature list if high-impact |
| Changed UI label, button name, or element ID | Any doc that references the old name; update capture.mjs selectors |
| New API endpoint | `docs/reference/api.md` with request/response schema |
| Changed API field name or type | `docs/reference/api.md` ChatTab/Message object, any affected schemas |
| New backend struct field | `docs/reference/api.md` if client-visible |
| Renamed function or exported symbol | Check docs for any prose references |
| New config file or persistence key | `docs/reference/` area doc + `AGENTS.md` File Persistence table |
| Removed feature | Remove all doc references; remove screenshots if no longer accurate |
| New screenshot scenario or capture script change | `tests/ui/README.md`, scenario usage block in `capture.mjs` |

### Which Docs to Update

| Area | Primary File | Notes |
|------|-------------|-------|
| Chat features, personas, compaction, guided generation | `docs/reference/chat.md` | Most frequently changed |
| REST API endpoints and data shapes | `docs/reference/api.md` | Keep ChatTab/ChatMessage objects in sync with Rust structs |
| Dashboard, monitoring, hardware | `docs/reference/dashboard.md` | |
| Remote agent and SSH | `docs/reference/remote-agent.md` | |
| CLI flags | `docs/reference/cli-flags.md` | |
| High-impact features visible in the README | `README.md` | See README guidelines below |

### README.md Guidelines

The README is a product overview — not a feature changelog. Apply a high bar for what appears there.

**Include in the README:**
- Features that are visually striking or immediately useful to a first-time visitor
- Features that differentiate llama-monitor from a plain llama.cpp dashboard
- Screenshots that show off the app in an impressive or useful state

**Do not include in the README:**
- Debugging-only features (internal logs, raw metrics dumps)
- Minor UX improvements (default value changes, layout tweaks)
- Features that are only relevant after deep use (export formats, fine-grained settings)
- More than 7–8 feature screenshots total — prefer fewer, higher-quality shots

**Feature section structure:**

```markdown
### Feature Name

One or two sentences explaining what the feature does and why it's useful. Write for
a developer who has never seen the app — lead with the value, not the mechanism.

![Alt text](docs/screenshots/filename.png)
```

Keep descriptions under 3 sentences. If a feature needs more explanation, it belongs in `docs/reference/`, not the README. Link to the reference doc at the bottom of the section if relevant.

**Updating the README:**
- Prefer updating existing sections to replacing them
- When a feature is substantially reworked, update both the prose and the screenshot
- When removing a feature, remove its README section and the screenshot file if it is no longer accurate

### docs/reference/ Guidelines

Reference docs are comprehensive. Include:
- All parameters with their defaults and valid ranges
- All API fields (type, default, nullability)
- All UI controls and what they do
- Screenshots for any non-trivial modal, panel, or workflow
- Cross-references to related sections with relative links

Avoid:
- Repeating prose from the README verbatim
- Writing from the perspective of a commit ("we added X") — write as if it always existed
- Leaving stale information — if a field is renamed, update the doc in the same PR

---

## Screenshot Harness

All repo-managed screenshots and animated UI captures use the single harness:

```bash
node tests/ui/capture.mjs --scenario <name>
```

The harness spawns a fresh `target/release/llama-monitor` binary on a temporary port with a clean config dir, then attaches to `REMOTE_SERVER` (default `http://192.168.2.16:8001`). The remote llama.cpp server must be reachable for any scenario that sends chat messages.

### Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `SCREENSHOT_PORT` | `8892` | Base port for the spawned monitor instance |
| `REMOTE_SERVER` | `http://192.168.2.16:8001` | llama.cpp server to attach to |
| `SCREENSHOT_FORM_AUTH` | `admin:secret123` | Credentials used for the auth-shell still captured by the `welcome` scenario |

### Scenarios

| Scenario | What It Captures | Saves To |
|----------|-----------------|----------|
| `welcome` | Welcome/setup screen plus form-auth shell without attach | `docs/screenshots/artifacts/` |
| `chat` | Chat, telemetry, logs | `docs/screenshots/artifacts/` |
| `guided-gen` | Context notes, suggestions, quick guide, director mode, surprise, explicit mode, categories | `docs/screenshots/artifacts/` |
| `sidebar` | Chat sidebar, FTS search, context menu, title filter | `docs/screenshots/artifacts/` |
| `settings` | Settings modal, preferences, persona, models, shortcuts | `docs/screenshots/artifacts/` |
| `panels` | Chat config panels, style, prompt debug | `docs/screenshots/artifacts/` |
| `dashboard` | Server tab and GPU section | `docs/screenshots/artifacts/` |
| `sparkline` | Throughput sparkline validation stills | `docs/screenshots/artifacts/` |
| `gifs` | Inference and GPU/system animated GIFs | `docs/screenshots/artifacts/` |
| `smoke` | Startup smoke validation | no screenshots unless the scenario is extended |

**Output directory convention:**
- `docs/screenshots/` — promoted hero shots used directly in `README.md`
- `docs/screenshots/artifacts/` — raw harness output for stills, GIFs, reference-doc images, and debugging

### Running Captures

```bash
# Rebuild release binary first if UI code changed
cargo build --release

# Welcome screen only
node tests/ui/capture.mjs --scenario welcome

# Core chat surfaces
SCREENSHOT_PORT=8892 node tests/ui/capture.mjs --scenario chat

# Guided-generation features
SCREENSHOT_PORT=9001 node tests/ui/capture.mjs --scenario guided-gen

# Sidebar and search surfaces
SCREENSHOT_PORT=8893 node tests/ui/capture.mjs --scenario sidebar

# Settings, panels, modals
SCREENSHOT_PORT=8894 node tests/ui/capture.mjs --scenario settings
SCREENSHOT_PORT=8896 node tests/ui/capture.mjs --scenario panels

# Dashboard/server surfaces
SCREENSHOT_PORT=8897 node tests/ui/capture.mjs --scenario dashboard

# Validation stills
SCREENSHOT_PORT=8898 node tests/ui/capture.mjs --scenario sparkline

# Animated GIFs
SCREENSHOT_PORT=8895 node tests/ui/capture.mjs --scenario gifs
SCREENSHOT_PORT=8895 node tests/ui/capture.mjs --scenario gifs --gpu-only
SCREENSHOT_PORT=8895 node tests/ui/capture.mjs --scenario gifs --inference-only
```

### When to Regenerate Screenshots

| Situation | Action |
|-----------|--------|
| New UI surface added (modal, panel, section) | Extend the appropriate scenario; run it; commit new screenshots |
| Existing UI surface visually changed | Re-run the scenario that covers it; commit updated screenshots |
| Element ID or selector changed | Update the selector in `capture.mjs` first, then re-run |
| Screenshot referenced in docs but no longer accurate | Regenerate or remove; never leave stale screenshots in docs |
| README hero shot is outdated | Re-run the relevant scenario, then explicitly promote the best artifact into `docs/screenshots/` |

### Rules for Agents

1. **Always rebuild release before running captures** when any `static/` file has changed. The harness runs `target/release/llama-monitor`, not the dev build.
2. **Do not create new one-off screenshot scripts.** Add scenarios to `tests/ui/capture.mjs` instead.
3. **Fix broken selectors in the same PR as the rename.** When an element ID changes (e.g., `#btn-system-prompt` → `#btn-behavior`), update `capture.mjs` at the same time to keep the harness green.
4. **Promote screenshots from `artifacts/` to `docs/screenshots/` explicitly** when linking them in the README. Copy the file; do not change the artifacts path in the scenario.
5. **Prefer updating existing scenario functions** over adding new ones for incremental changes to an existing feature area.
6. **Log geometry/state for invisible surfaces.** If a popup, hovercard, or panel isn't appearing, add a `console.log` with element geometry before calling `captureShot`. Do not skip the capture silently.
7. **Document new scenarios** in both the `printUsage()` block and `tests/ui/README.md`.
8. **Use `--no-attach` only for the welcome-screen shot.** All chat and feature screenshots require an attached server.

### Naming Convention

Screenshots use a numeric prefix for rough ordering and a descriptive slug:

```
NN-description.png         # hero shots in docs/screenshots/
NNb-description.png        # variant or sub-shot of the same numbered slot
```

When adding a new screenshot:
- Pick the next available number in the relevant range
- Use a slug that describes the UI surface, not the commit or feature branch
- Never reuse a number that already refers to a different surface (readers bookmark images by URL)

## Playwright UI E2E Tests

Playwright UI tests are separate from the Puppeteer screenshot harness. They:

- Run anywhere (local and CI).
- Validate behavior and flows, not pixel-perfect visuals.
- Are typically run near the end of a feature branch before marking a PR ready.

### Running Locally

From the repository root:

```bash
cd tests/ui
npm install
npx playwright install chromium
npm test
```

**IMPORTANT: Always run with CI-equivalent flags to catch real failures and match CI behavior exactly:**

```bash
# CI-equivalent run (sequential, 2 retries) — use this before marking a PR ready
npx playwright test --workers=1 --retries=2
```

Running without `--workers=1` uses multiple workers in parallel, which can cause false
failures from timing races. A test that passes with `--workers=1` but fails with many
workers in parallel is a timing issue, not a functional bug. A test that fails with
`--workers=1 --retries=2` is a real failure — fix it before pushing.

Useful variants:

- `npm test` — run all tests headless (default workers, no retries)
- `npx playwright test --workers=1 --retries=2` — CI-equivalent sequential run (**preferred**)
- `npm run test:headed` — run with browser visible
- `npm run test:debug` — run in Playwright inspector
- `npm run test:ssh` — run SSH integration tests (requires LLAMA_MONITOR_SSH_TARGET)
- `LLAMA_MONITOR_HAS_AI=1 npm run test:ai` — include AI-dependent tests

Notes:

- Tests use a fresh temporary config dir via `run-server.mjs`; they do not depend on your local `~/.config/llama-monitor/`.
- The UI URL defaults to `http://127.0.0.1:7778`. You can override with `LLAMA_MONITOR_UI_URL`.

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
1. Run with CI-equivalent flags first: `npx playwright test --workers=1 --retries=2`
2. If that passes, the local failure was a parallel timing race — not a real bug
3. Run the test in isolation to confirm: `npx playwright test --workers=1 --grep "test name"`
4. Do not mark PR as ready-to-test if `--workers=1 --retries=2` fails — that is a real bug

**Cargo test flakiness from concurrent processes:**
Rust unit tests that write to shared temp paths (e.g., `std::env::temp_dir()`) can fail
with "No such file or directory" when two separate `cargo test` processes run at the same
time (e.g., you have a background build running while launching another test run). Always:
- Ensure no other `cargo test` or `cargo build` processes are running before `cargo test`
- Use PID-unique temp paths in tests: `temp_dir().join(format!("name-{}", std::process::id()))`
- Never use a shared, collision-prone path like `temp_dir().join("llama-monitor-test")`

### Maintenance Rules

UI e2e tests are first-class and must be kept in sync with the application. Agents MUST treat them similarly to reference documentation: update them as part of the same PR that changes the feature.

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

Before creating a PR, the agent **must** run a cross-cutting validation pass using a sub-agent (Task tool) to catch issues that automated checks miss. This is mandatory for any PR that touches multiple files or introduces new features.

### Validation Checklist

The sub-agent must verify:

1. **CSS integrity**: No duplicate selectors, duplicate `@keyframes`, or specificity conflicts where new styles are silently overridden
2. **Cross-module wiring**: All new JS functions have callers, all new HTML elements have CSS rules, all new CSS classes are used in HTML
3. **Accessibility**: All new animations have `@media (prefers-reduced-motion: reduce)` overrides
4. **Theme coverage**: All new styled elements have `[data-theme="light"]` overrides
5. **Backend-frontend contract**: New API fields are serialized, deserialized, and consumed on the frontend; new WebSocket messages are handled
6. **No stale code**: No leftover blocks from refactoring (e.g., duplicate rules, commented-out sections that should be deleted)

### Validation Command

Use the Task tool with a prompt like:

```
Use a sub-agent to validate all changes in the current branch. Check for:
- CSS selector duplication and specificity conflicts
- Missing prefers-reduced-motion overrides for new animations
- Missing light theme overrides for new styled elements
- Broken cross-module references (JS → HTML → CSS)
- Backend-frontend contract mismatches
- Stale code from refactoring
Return a detailed report of any issues found with file:line references.
```

### Action on Findings

- **Critical issues** (broken functionality, silent CSS overrides): Must fix before PR
- **Medium issues** (missing accessibility, incomplete theme coverage): Should fix before PR
- **Low issues** (cosmetic, minor cleanup): Can note in PR description as follow-up

## Pre-Push / Pre-Tag Checks (MANDATORY)

Before pushing or tagging a PR as ready, the agent MUST run the same checks as CI locally.
If any check fails, the agent MUST NOT push until all are fixed.
This is not optional.

Required checks (every time):

- `git diff --check`
- `cargo fmt -- --check`
- `cargo clippy -- -D warnings`
- `cargo test`
- `cargo build --release`
- `npm run validate-js`
- `npm run lint` (if `static/**` or `tests/ui/**` changed)

Hard rules:

- Never rely on CI to catch issues you can avoid.
- Never push "to see if CI passes."
- Never ignore, comment out, or "trust" a failing check.
- Include auto-generated files (e.g., src/gen/*) in every check and commit.
- If you change code, configs, or static assets, re-run the full checklist before pushing.
- If you are unsure whether something might affect CI, assume it does and run the checks.
- If a check fails and you cannot immediately fix it, STOP and ask for clarification instead of pushing.
- **JS module baseline:** If you added a new `.js` file under `static/js/` imported from `bootstrap.js`, you MUST update `tests/ui/core/js-module-baseline.json` (run `cd tests/ui && npm run update-baseline`). Commit the updated baseline with the new module.

## CI/CD Workflow

### Pull Requests

1. **CI triggers**: CI only runs when PR has `ready-to-test` label
2. **Title format**: Use conventional commit format (`feat:`, `fix:`, etc.)
3. **Auto-labeling**: GitHub labels PRs based on file paths and commit titles:
    - **File paths**: `.github/workflows/**` → `ci`, `github-actions`; `**/*.rs` → `rust`; `**/*.cs` → `csharp`; `**/*.js` → `javascript`; `static/**` → `ui`; `**/*.sh` → `shell`; `**/*.md` → `docs`; `Cargo.toml`/`Cargo.lock` → `dependencies`; `package.json` → `node-dependencies`; `tests/**` → `test`
    - **Commit titles**: `fix(` → `fix`; `feat(` → `feat`; `refactor(` → `refactor`; `chore(` → `chore`; `perf(` → `perf`; `docs(` → `docs`; `test(` → `test`; `ci(` → `ci`
4. **CI checks**: All PRs must pass:
   - `cargo fmt -- --check`
   - `cargo clippy -- -D warnings`
   - `cargo test`
   - `cargo build --release`

### Releases

- **Automated**: release-please creates release PRs when `feat:` or `fix:` commits merge to `main`
- **Release type**: Rust (semantic versioning based on commit types)
- **Version bump**: `feat:` → MINOR, `fix:` → PATCH, others → no bump
- **Published release notes source**: GitHub Releases should preserve the `release-please` body generated from `CHANGELOG.md`; do not replace it with GitHub auto-generated notes for normal tagged releases.
- **Multi-entry release notes**: If one PR contains several distinct releasable changes that should appear as separate bullets, update the PR body with a `BEGIN_COMMIT_OVERRIDE` / `END_COMMIT_OVERRIDE` block before merge.
- **Override quality bar**: Override blocks should usually contain one line per meaningful user-facing `feat`, `fix`, or `perf` item that shipped in the PR, based on the branch's releasable commits. Prefer several specific bullets over one catch-all summary.

## File Persistence

All user data persists to `~/.config/llama-monitor/`:

| File | Purpose |
|------|---------|
| `chat.db` | SQLite chat storage for tabs, messages, full-text search index, and chat metadata |
| `backups/auto/` | Automatic hourly rolling backups (`chat_auto_<ts>.db`); last 24 kept |
| `backups/daily/` | Automatic daily backups (`chat_daily_<day>.db`); last 7 kept |
| `backups/manual/` | Manual API-triggered backups (`chat_<ts>.db`) and pre-restore safety copies (`pre_restore_<ts>.db`); last 7 kept |
| `sessions.json` | Persisted session list (`Session` objects with spawn/attach mode, status, preset ID, timestamps) |
| `presets.json` | Model presets with llama.cpp launch parameters |
| `templates.json` | User-created or user-modified chat persona templates and explicit policy overrides |
| `ui-settings.json` | Persisted `UiSettings` values such as paths, ports, remote-agent settings, explicit policy, guided-generation defaults, and chat input height |
| `auth-config.json` | Persisted dashboard auth mode (`basic`, `form`, or both), username, and hashed password for the Security tab |
| `gpu-env.json` | GPU environment overrides (`arch`, `devices`, `rocm_path`, `extra_env`) |
| `ssh-known-hosts.json` | Trusted SSH host keys for remote-agent workflows |
| `lhm-disabled.json` | Persisted Windows LibreHardwareMonitor disabled/enabled state |

Data is persisted:
- Sessions: autosaved every 30 seconds
- Presets: saved immediately by the preset CRUD API
- Templates: saved immediately by the template CRUD API
- UI settings: saved immediately by `PUT /api/settings`
- GPU environment: saved immediately by `PUT /api/gpu-env`
- Chat database: updated live by chat tab/message APIs; WAL checkpointed and ANALYZE run hourly
- Chat backups: automatic hourly (last 24) to `backups/auto/`, automatic daily (last 7) to `backups/daily/`, manual on-demand to `backups/manual/` via `POST /api/db/backup`

Frontend-only browser persistence also exists in `localStorage` and is not mirrored into `~/.config/llama-monitor/`. This includes UI state such as chat style, chat font, enter-to-send, telemetry pinning, nav/sidebar collapse state, visualization preferences, last attached endpoint, last session/setup positioning, date format, update dismissals, and some guided-generation UI toggles/categories.

## Git Branch Strategy

- **`main`**: Stable, release-ready code
- **`feature/*`**: Feature development branches
- **`release/*`**: Release preparation (created by release-please)
- **`hotfix/*`**: Critical bug fixes for production

All branches should be deleted after merge.

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

These patterns must be followed when adding or modifying endpoints. Violations have caused real shipped bugs.

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
