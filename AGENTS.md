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
```

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

### Scenarios

| Scenario | What It Captures | Saves To |
|----------|-----------------|----------|
| `welcome` | Welcome/setup screen without attach | `docs/screenshots/artifacts/` |
| `chat` | Chat, telemetry, logs | `docs/screenshots/artifacts/` |
| `guided-gen` | Context notes, suggestions, quick guide, director mode, surprise, explicit mode, categories | `docs/screenshots/artifacts/` |
| `sidebar` | Chat sidebar, FTS search, context menu, name filter | `docs/screenshots/artifacts/` |
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
- Never push “to see if CI passes.”
- Never ignore, comment out, or “trust” a failing check.
- Include auto-generated files (e.g., src/gen/*) in every check and commit.
- If you change code, configs, or static assets, re-run the full checklist before pushing.
- If you are unsure whether something might affect CI, assume it does and run the checks.
- If a check fails and you cannot immediately fix it, STOP and ask for clarification instead of pushing.

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
| `backups/` | Manual `chat_*.db`, automatic hourly `chat_auto_*.db`, and pre-restore `pre_restore_*.db` database backups |
| `sessions.json` | Persisted session list (`Session` objects with spawn/attach mode, status, preset ID, timestamps) |
| `presets.json` | Model presets with llama.cpp launch parameters |
| `templates.json` | User-created or user-modified chat persona templates and explicit policy overrides |
| `ui-settings.json` | Persisted `UiSettings` values such as paths, ports, remote-agent settings, explicit policy, guided-generation defaults, and chat input height |
| `gpu-env.json` | GPU environment overrides (`arch`, `devices`, `rocm_path`, `extra_env`) |
| `ssh-known-hosts.json` | Trusted SSH host keys for remote-agent workflows |
| `lhm-disabled.json` | Persisted Windows LibreHardwareMonitor disabled/enabled state |

Data is persisted:
- Sessions: autosaved every 30 seconds
- Presets: saved immediately by the preset CRUD API
- Templates: saved immediately by the template CRUD API
- UI settings: saved immediately by `PUT /api/settings`
- GPU environment: saved immediately by `PUT /api/gpu-env`
- Chat database: updated live by chat tab/message APIs; WAL checkpointed hourly
- Chat backups: automatic hourly backup plus manual `/api/db/backup`

Frontend-only browser persistence also exists in `localStorage` and is not mirrored into `~/.config/llama-monitor/`. This includes UI state such as chat style, chat font, enter-to-send, telemetry pinning, nav/sidebar collapse state, visualization preferences, last attached endpoint, last session/setup positioning, date format, update dismissals, and some guided-generation UI toggles/categories.

## Git Branch Strategy

- **`main`**: Stable, release-ready code
- **`feature/*`**: Feature development branches
- **`release/*`**: Release preparation (created by release-please)
- **`hotfix/*`**: Critical bug fixes for production

All branches should be deleted after merge.
