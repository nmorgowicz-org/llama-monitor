# Llama Monitor Project Rules

## Serena (IDE-level symbolic tools via MCP)

Serena is connected via MCP and provides IDE-level, symbol-aware tools for navigating and editing this codebase.

> **Current status**: Serena is configured for local use via OpenCode only (not in Codex or Claude Code right now). The `.serena/` directory is in the repo so Serena can operate locally on this project.

### When to use Serena

- **Use Serena tools for any symbol-level operation** — finding symbols, finding references, renaming symbols, replacing symbol bodies, inserting before/after symbols, safe-delete. This is faster and more reliable than naive search-and-replace.
- **Use Serena for refactoring** — cross-file renames, moves, inlining, propagating deletions.
- **Use Serena for exploring structure** — symbol overview (file outline), type hierarchy, diagnostics.
- **Use OpenCode built-in tools for** — small text edits, non-code work, quick reads, shell commands, git.

### How Serena is configured for this project

- Context: `ide` (single-project, focused on symbolic tools; basic file/terminal tools are left to OpenCode).
- Project is pre-activated on startup via `--project ~/SCRIPTS/CLAUDE/llama-monitor`.
- On first use, Serena will run an onboarding pass to build memories about the project. Let it complete — this is a one-time cost.
- Once onboarding is done, Serena will draw on its memories for better context about conventions, structure, and patterns.

### Key Serena tools (names as the agent sees them)

- `find_symbol` — locate a symbol by name across the project.
- `symbol_overview` — file outline for a given file.
- `find_references` — find all references to a symbol.
- `replace_symbol_body` — replace the body of a function/method/struct implementation.
- `insert_after_symbol` / `insert_before_symbol` — insert code relative to a symbol.
- `safe_delete` — delete a symbol, propagating the change.
- `rename_symbol` — rename a symbol across all references.

### Example workflow

To refactor the `AppConfig` struct:
1. `find_symbol("AppConfig")` → get the struct's location.
2. `find_references("AppConfig")` → see where it's used.
3. `replace_symbol_body("AppConfig", <new body>)` → make the change.
4. Run `cargo clippy` via shell to verify.

## UI/UX Collaboration

For all UI/UX work (bars, cards, modals, layout changes, visual polish), use the screenshot harness to iterate with the user, not just code descriptions.

- Always run:
  - `cargo build --release`
  - `node tests/ui/capture/index.mjs --scenario <scenario>`
- Use this to:
  - Confirm proposed designs in real UI.
  - Validate text, spacing, colors, and behavior.
- Use whichever scenario matches the area being changed (e.g., welcome, chat, spawn-wizard, dashboard, settings, sidebar, panels, models-v2, etc.).
- If new capabilities require new capture scenarios, add them under `tests/ui/capture/scenarios/<group>/` (group = functional area, e.g. `wizard-llamacpp`, `presets`) and register them in `tests/ui/capture/index.mjs`, then update usage docs. Groups also drive `cli-group.mjs` — see Screenshots Workflow below.
- Never rely on screenshots from other environments or imagined renders.
- Treat screenshots as the single source of truth for "what this will look like."

## Conventional Commits

All commits MUST follow [Conventional Commits](https://www.conventionalcommits.org/):

```
<type>(<scope>): <description>
```

| Type | Purpose | Version |
|------|---------|---------|
| `feat` | New feature | MINOR |
| `fix` | Bug fix | PATCH |
| `perf` | Performance improvement | PATCH |
| `refactor` | Code refactoring | — |
| `test` | Tests | — |
| `chore` | Maintenance | — |
| `docs` | Documentation | — |
| `ci` | CI/CD | — |
| `revert` | Revert | Depends |

Scope: `api`, `ui`, `chat`, `gpu`, `nav`, `settings`, `models`, `sessions`, `wizard`, `vram`, `hf`, `binary`, `spawn`, `docs`, `ci`. Pick closest match.

PR title MUST be `feat:` or `fix:` if it contains those commits (release-please requirement).
NEVER put `BEGIN_COMMIT_OVERRIDE`/`END_COMMIT_OVERRIDE` in git commit messages — only in PR bodies.

We use squash-merge; release-please evaluates each PR individually (not all inner commits):
- `feat:` in PR title → minor
- `fix:` in PR title → patch
- `feat!:` in PR title OR `BREAKING CHANGE:` in PR body → major
  - Example: `feat(wizard)!: redesign spawn flow`
  - Or include at bottom of PR body:
    - `BREAKING CHANGE: spawn wizard now requires explicit profile selection`

For PRs with multiple user-facing items, add override block to PR body:
```text
BEGIN_COMMIT_OVERRIDE
feat(chat): add send-to-stop generation toggle
fix(gpu): correct AMD temperature reading
END_COMMIT_OVERRIDE
```

## Feature Implementation Loop

For complex features (e.g., Rapid-MLX integration), use a Pipelined Implementation Loop to maximize context efficiency and stability.

- **The Builder (General Agent)**: Responsible for the "How". Implements a specific phase of the roadmap. Focuses on code correctness, trait implementation, and meeting the phase's "Hard Gates".
- **The Verifier (General Agent)**: Responsible for the "What". Validates the Builder's work against the original specification. Writes integration tests, performs security reviews, and provides final sign-off for the phase.
- **The Coordinator (Lead Agent)**: Orchestrates the loop. Manages the roadmap, handles git commits/pushes, ensures a clean handoff between Builder and Verifier, and triggers the next phase only after sign-off.

This separation prevents "looping" where an agent tries to fix a bug it introduced while simultaneously implementing a feature, ensuring a higher quality bar for every merge.

## Build & Test Commands

```bash
cargo build --release
cargo test
cargo clippy -- -D warnings
cargo fmt
npm run validate-js
npm run lint
git diff --check
```

## Mandatory Pre-PR Checks

Run in this exact order; commit any auto-changes before continuing. Never push "to see if CI passes."

1. `cargo clippy -- -D warnings` — fix all warnings
2. `cargo test` — no known test failures
3. `npm run validate-js`
4. `npm run lint`
5. `git diff --check` — fix whitespace issues
6. `cargo build --release`
7. `cargo fmt` — commit changes if any
8. `git status` — ensure nothing uncommitted
9. If new `.js` under `static/js/` imported from `bootstrap.js`:
   - `cd tests/ui && npm run update-baseline`
   - Commit `tests/ui/core/js-module-baseline.json` with the new module.

## Static Asset Registration

`build.rs` scans `static/` and auto-generates:
- `src/gen/static_assets.rs` — `include_str!` constants
- `src/gen/routes.rs` — warp route filters

To add a file: place it in `static/`, run `cargo build`, commit both your file and updated `src/gen/*.rs`.

Constant naming:
- CSS: `CSS_` + filename stem, hyphens→underscores, uppercase
- JS: skip `js/` prefix, join path parts with `_`, replace `.` and `-` with `_`, uppercase
- Root: filename with `.` and `-` replaced by `_`, uppercase

## Chat Template UI — Dual-Surface Change

Chat-template selection UI is hand-duplicated in two places: the Spawn Wizard
(`spawn-wizard-chat-template.js`) and the Preset Editor (`presets.js` +
`static/index.html`'s `modal-chat-template-file` row). Any change to install/history/
rollback/status behavior must be made in **both** — see `docs/reference/spawn-wizard.md`
("Chat templates — two frontend surfaces, one backend") for the full breakdown, including
the current llama.cpp-only runtime coverage gap (Rapid-MLX does not consume
`chat_template_file` yet).

## JavaScript Linting

After modifying `.js` under `static/js/`:
- Run `npm run lint` (catches import-assign, no-undef, XSS via innerHTML).
- Run `npm run validate-js` (syntax only).

## Multi-Platform Compatibility (MANDATORY)

Targets: macOS, Linux, Windows. Never add platform-specific code without a Windows equivalent or explicit `#[cfg]` stub.

- After changes to `src/tray.rs`, `Cargo.toml`, or files with `#[cfg]`, run:
  - `rustup target add x86_64-pc-windows-gnu`
  - `cargo check --target x86_64-pc-windows-gnu`
- `wry` is universal (not re-scoped away from Windows).
- `winit` uses `default-features = false` with `x11`/`wayland`; Win32 backend is automatic.
- `harden_file_permissions()` is a no-op on Windows (known gap).

Full reference: `docs/agents/platform-details.md`

## Playwright UI E2E Tests

CRITICAL: Default `npm test` (no flags) kills process on port 7778, which may be your active model.

Canonical local run (CI-equivalent, isolated) — always use this:
```bash
cd tests/ui
CI=1 LLAMA_MONITOR_USE_RELEASE=1 LLAMA_MONITOR_TEST_PORT=17778 npm test
```

Run before adding `ready-to-test` label or after significant UI/chat/flow changes.

TIMEOUT: Use at least 600 seconds (10 minutes). Full suite is ~193 tests and will exceed 5 min.

Full reference: `docs/agents/playwright.md`

## Screenshot Harness

```bash
node tests/ui/capture/index.mjs --scenario <name>
```
NEVER run multiple scenarios in parallel (port conflicts). Always `cargo build --release` first if `static/` changed.

## Documentation

Docs updated in same PR as code — not as follow-up. Primary areas:
- Chat: `docs/reference/chat.md`
- API: `docs/reference/api.md`
- Dashboard/monitoring: `docs/reference/dashboard.md`
- Remote agent/SSH: `docs/reference/remote-agent.md`
- CLI: `docs/reference/cli-flags.md`
- Spawn wizard/HF: `docs/reference/spawn-wizard.md`
- VRAM estimator: `docs/reference/vram-estimator.md`
- Windows runtime behavior: `docs/reference/windows-support.md`

Write as if feature always existed.

## CI/CD

- CI triggers: PR has `ready-to-test` label, dependabot, or `static/**`/`tests/ui/**` changes.
- PR title: must be conventional commit format.
- Releases: release-please on `feat:`/`fix:` merged to `main`.
- NEVER add the `ready-to-test` label to a PR. This label is only set by the human.

## Security (MANDATORY — Summary)

Full reference: `docs/agents/security-details.md`

- All data-reading endpoints require `api-token`. "Read-only" is not unauthenticated.
- All write/delete endpoints require `api-token` minimum.
- High-impact/irreversible operations require `db-admin-token` + confirmation field.
- Token rotation: MUST update both on-disk file AND in-memory `AppConfig` atomically.
- No `==` on secrets: use `subtle::ConstantTimeEq` (via `check_api_token`).
- Randomness: use `getrandom::getrandom()` (or `rand_core::OsRng` when trait needed). No timestamp/PID fallbacks.
- No direct file ops on live SQLite: use `ChatStorage::backup()`; handle WAL sidecars on restore.
- No innerHTML/insertAdjacentHTML with untrusted data: use `textContent` or DOMPurify.
- All user input is untrusted; validate/canonicalize file paths (reject `..`, leading `/`\`\\\``).
- Protocol fields use `#[serde(default)]` with degraded mode.
- Rate limit/timeout expensive or system-affecting endpoints.
- Run `/security-review` before PR.

Before marking PR ready, verify:
- [ ] Auth on all new endpoints
- [ ] No `==` on secrets
- [ ] No predictable randomness
- [ ] No direct SQLite file ops
- [ ] File paths validated
- [ ] No new XSS via innerHTML
- [ ] Expensive ops have timeout/limit
- [ ] Agent/protocol fields use #[serde(default)]
- [ ] Secrets not logged
- [ ] Docs updated

## API and Serialization Safety

- All HTTP/DB structs: use `#[serde(default)]` on fields with sensible defaults.
- JSON parse errors: must return 400, never 404.
- Never silently delete user data on HTTP errors.
- On 404 for update: retry 2-3 times with backoff; only consider removal if response explicitly indicates "not_found".
- When changing API struct, auth, or adding endpoint:
  - Run `cargo test` and `tests/auth_routing.rs`.
  - Add/update auth tests.

## VRAM Estimator (Key Pitfalls)

When updating `src/llama/vram_estimator/` (a module dir, not a flat file):
- **GGUF is the source of truth.** `gguf_meta.rs` reads real arch (layer counts, `full_attention_interval`, `ssm.*`, sliding-window pattern, expert counts, MTP) and `to_arch` overrides the name heuristic. Name parsing (`from_name_and_params`) is fallback-only. Don't "fix" a model by editing the heuristic if a GGUF exists — verify against the file.
- Pre-download estimates introspect too: `/api/vram-estimate` accepts `hf_repo_id`+`hf_file_path`+`model_size_bytes` and range-fetches the GGUF header (`crate::hf::fetch_gguf_header_metadata`). All UI VRAM bars use this endpoint; there is no client-side VRAM formula.
- Discrete-GPU overhead (`discrete_overhead_*`) is **calibrated to real RTX 5090 measurements** — do NOT revert to a context-independent `n_layers × n_embd` formula. To re-measure, follow "Recalibrating the discrete overhead" in the reference doc (Windows WDDM has no per-process VRAM → use total `nvidia-smi` delta; pass `--parallel 1 -fit off`).
- "A3B"/"A4B"/"A10B" suffixes are active parameter counts, NOT expert counts (heuristic fallback only).
- Hybrid DeltaNet (Qwen3.5/3.6): `n_attn_layers` (= `block_count / full_attention_interval`) drives KV; wrong value inflates KV ~4×.
- Gemma4: `global_head_dim = 512`, 1024-token sliding window; never set `local_attn_window` on DeltaNet.
- Every new `_arch()`/`_heuristic()` requires a `#[test]` with source URL.
Full reference: `docs/reference/vram-estimator.md`

## Pre-PR Validation (Cross-Cutting)

For PRs touching multiple files or adding features, run a sub-agent check for:
- CSS selector duplication / specificity conflicts
- Missing `prefers-reduced-motion` on new animations
- Missing `[data-theme="light"]` overrides on new styled elements
- Broken JS→HTML→CSS cross-module references
- Backend-frontend API contract mismatches
- Stale code from refactoring

## Screenshots Workflow

- **Capture** (for debugging, UI review): use artifacts/
  - Single scenario: `node tests/ui/capture/index.mjs --scenario <name>`
  - Whole group (every scenario registered under `tests/ui/capture/scenarios/<group>/`, run sequentially):
    `SCREENSHOT_PORT=<port> node tests/ui/capture/cli-group.mjs <group> --no-attach`
    Groups: `config`, `core`, `features`, `models`, `presets`, `validation`, `wizard-llamacpp`, `wizard-rapidmlx`.
  - Files go to: `docs/screenshots/artifacts/<group>/<scenario-filename>.png` (subfolder per group, not flat).
  - This folder is gitignored: keep it for UX reference, debugging, comparisons.
  - NEVER run scenarios/groups in parallel (port conflicts) — one `SCREENSHOT_PORT` at a time.

- **Promote** only when actually used in docs:
  - 1) Add image reference in README.md or docs/reference/*.md.
  - 2) Copy from artifacts/<group>/ to docs/screenshots/ (flat, no subfolder):
       `cp docs/screenshots/artifacts/<group>/<name>.png docs/screenshots/<name>.png`
  - 3) Commit both: your doc changes + the promoted screenshot.

- **Check for unused screenshots** (before or after PR):
  - Run: `bash scripts/check-unused-screenshots.sh`
  - If it lists files, either:
      - Add them to docs, or
      - Delete them.

- **Rules**:
  - Never commit a screenshot to docs/screenshots/ unless it is referenced in documentation.
  - Prefer promoting existing artifacts over capturing fresh if the scene hasn't changed.
