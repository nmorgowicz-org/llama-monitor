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
./scripts/validate-js.sh

# Lint static/js with ESLint (catches XSS, no-undef, import-assign)
npm run lint
```

**Important:** Always run `npm run lint` after modifying any `.js` files under `static/js/`. This runs ESLint with three rules:
- `no-import-assign` — catches assignment to ES module namespace bindings (the `TypeError: Assignment to constant variable` class of error)
- `no-undef` — catches bare references to functions no longer on `window` after ES module extraction
- `no-unsanitized/property` and `/method` — catches `innerHTML`/`insertAdjacentHTML` with unescaped user data (XSS); `escapeHtml()` is the approved sanitizer

The lint job runs automatically in CI on every PR push that touches `static/**` or `tests/ui/**`, without requiring the `ready-to-test` label. Also run `./scripts/validate-js.sh` for syntax-only validation on all JS files.

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
| `sessions.json` | Session definitions (spawn/attach mode, ports, server configs) |
| `presets.json` | Model presets with all llama.cpp parameters |
| `ui-settings.json` | Web UI preferences (paths, ports, presets) |
| `gpu-env.json` | GPU environment config (architecture, device indices) |

Data is persisted:
- Sessions: every 30 seconds + on explicit save
- Presets/Settings: on explicit save via API

## Git Branch Strategy

- **`main`**: Stable, release-ready code
- **`feature/*`**: Feature development branches
- **`release/*`**: Release preparation (created by release-please)
- **`hotfix/*`**: Critical bug fixes for production

All branches should be deleted after merge.
