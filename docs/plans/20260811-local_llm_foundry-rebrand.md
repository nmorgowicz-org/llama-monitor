# Local LLM Foundry 2.0 Rebrand — Authoritative Execution Plan

- Status: execution authority after the decisions in this document are approved
- Plan owner: Sol
- Execution owner: Luna, one gated phase at a time
- Target branch: `feat/rapid-mlx-integration`
- Target release: 2.0.0
- Source snapshot: 2026-08-11

## Execution status (verified 2026-08-12)

| Phase | Status | Evidence / remaining gate |
|---|---|---|
| 0 — discovery | Complete | Baseline, redacted resource inventory, raw manifests, API/release/screenshot receipts, fail-closed classifier, and SHA-256 manifest verified. |
| 1 — identity + Token Ingot | Complete | Deterministic master, complete web/PWA/native/package/social derivatives, binary-safe registration, proofs, similarity review, and approval receipt verified. |
| 2 — identity/path authority | Complete | Central identity, selected-root routing, encryption ordering, migration queue, reviewed literal policy, environment matrix, Windows cross-check, and receipts verified. |
| 3 — migration protocol + fixtures | Complete | Versioned schemas, pure planner, deterministic/stale/idempotent checks, full pure fixture matrix, sanitized error contract, API auth protocol, and compatibility receipts verified. |
| 4 — application-home migration core | Complete | Copy-first queued maintenance migration, lock/journal/resume, free-space preflight, rollback/cleanup queues, sanitized receipts, and fixture gates verified; native Windows execution remains a Phase 12 qualification marker. |
| 5 — path/config/resource default-root switch | Complete | Fresh canonical defaults, selected-root consumer routing, helper/docs updates, Windows early logging, and persisted-path policy receipt verified. |
| 6 — model-library relocation path integration | Complete | Receipt-backed keep/move selection, resumable verified copy, persistence rewrites, and source-retention policy passed; native Windows qualification remains a Phase 12 marker. |
| 7 — package, crate, binary, CLI, and backend identity | Complete | Canonical/legacy entrypoints, parity tests, release build, JS gates, and Windows GNU cross-check passed; native Windows execution remains a Phase 12 marker. |
| 8 — frontend identity, migration UX, accessibility, and theme parity | Complete with CI regression follow-up | Source-level gates and focused/full local release-built browser validation pass; the two CI timing races from `31603543480` are fixed locally and await a fresh remote run. |
| 9 — runtime, agent, updater, tray, and platform identities | Complete | Runtime compatibility, updater, remote-path/task migration, CA continuity, entropy hardening, and Token Ingot tray integration are validated; native Windows proofs remain explicit return markers. |
| 10 — documentation, API, CLI, migration, and historical policy | Complete | Current docs/templates, upgrade guide, brand usage policy, compatibility notes, historical allowlist, and 42-file relative-link validation pass. Fresh screenshot promotion remains Phase 13. |
| 11 — CI, release-please, packaging, dual assets, and repository rename | In progress | Source and GitHub cutover gates pass: repository renamed, old URL redirects, git continuity verified, and PR #314 carries `feat!:` with a compact 20-entry override. Windows-target clippy/build and release-smoke jobs pass. The two UI races from CI run `31603543480` are fixed locally; a fresh remote run, generated 2.0.0 release, and real artifact/update probes remain open. |
| 12–14 — cross-platform, security, final qualification, and launch | Not started; Windows handoff prepared | Native Windows work is intentionally deferred to the Windows machine. Use `docs/plans/20260812-local_llm_foundry-windows-validation-handoff.md`; do not close these phases from macOS cross-compilation alone. |

### Current CI qualification incident and local resolution (2026-08-12)

PR #314 run `31603543480` completed with **268 passed, 5 skipped, 2 failed** in
the UI job. All other CI jobs in that run passed, including Windows-target
clippy, Windows GNU release smoke, Linux/macOS release smoke, lint, and CodeQL.

The downloaded Playwright report is retained at
`/tmp/local-llm-foundry-ci-31603543480/playwright-report/` and contains traces,
screenshots, and error contexts for each retry. The two failing tests are:

1. `core/app-shell.spec.js:176` — the Gemma recommendation click completed but
   `#modal-chat-template-file` remained empty on all retries.
2. `core/rapid-preset-visibility.spec.js:99` — the control-reachability loop
   timed out while repeatedly navigating sections/opening Rapid-MLX details.

Local diagnosis found that the Gemma failure was a real layout race: the async
VRAM estimate changed the strip from `display:none` to visible while the click
was in flight, moving the button under the pointer. The strip now reserves a
stable footprint and toggles visibility/accessibility state without reflow.
Rapid-MLX reachability now uses the same stable layout under repeated section
and details navigation.

Verified after the fix with the release build:

- Gemma recommendation: 20/20 focused repetitions.
- Rapid-MLX control reachability: 10/10 focused repetitions.
- Full Playwright suite: 267 passed, 5 skipped, and 3 flaky-but-passed-on-retry;
  no hard failures.
- Fresh `preset-editor` screenshot harness completed and was visually inspected.

The three remaining flakes are guided-generation startup/toast timing and are
not the two CI failures above. A fresh GitHub `ready-to-test` run is still
required before Phase 11 or the release candidate is marked green.

### Native Windows qualification update (2026-08-12)

Windows commit `3069a1b` is clean and now passes Rust 1.97.1 native MSVC
clippy, the full Rust suite (5 intentional ignores), release build, GNU
cross-target check and clippy, `cargo fmt -- --check`, JavaScript validation,
lint, rebrand validation, and release-contract validation. The release server
also starts successfully on Windows. Native Playwright now passes after
allowing Chromium loopback traffic in Portmaster: 270 passed, 5 intentional
skips, 0 failures. The remaining Windows return markers are the disposable
application-home, tray/WebView2, sensor bridge, updater, remote-agent,
package, and fresh-CI receipts. The receipt is tracked in
`docs/plans/evidence/20260811-local-llm-foundry/phase-12/windows/`.

### Screenshot parity scope update (2026-08-12)

Windows core/config capture groups are valid cross-platform UI evidence at the
shared `1440x900` viewport. Rapid-MLX local execution, live telemetry, and
runtime-manager evidence remain Apple-Silicon/macOS-only. The current Rapid
wizard/preset scenarios also consult the real platform-info gate, so the
Windows `wizard-rapidmlx` and `rapid-preset` probes fell back or timed out and
are explicitly non-evidence. The Windows handoff records this distinction and
the remaining macOS Rapid capture/parity work; it is the operational companion
to this plan, not a competing status ledger.

The capture harness now skips every scenario marked as Rapid-dependent before
starting the app on non-Apple-Silicon hosts. Playwright's mocked Rapid UI tests
remain cross-platform; only local executable/runtime screenshot evidence is
gated.

The initial Windows `config`/`core` artifact batch is excluded from visual
parity because it inherited the real Windows application-home state. The
capture harness now supplies an explicit temporary config root and isolated
Windows profile variables; those groups must be recaptured before visual
acceptance.

The detailed pair manifest and review order live in
`docs/plans/evidence/20260811-local-llm-foundry/phase-13/windows-macos-visual-parity-ledger.md`.

### Phase closure blockers

These are the specific reasons an earlier phase remains open; they are
acceptance gates, not implementation uncertainty.

| Phase | Required before closure |
|---|---|
| 0 | Closed 2026-08-11: all required receipts and the fail-closed classification check pass; synthetic Windows cases are recorded for the Phase 3 fixture owner. |
| 1 | Closed 2026-08-11: derivative matrix, small-size proofs, local similarity review, and production-master approval receipt pass. External legal clearance remains a launch-owner responsibility. |
| 2 | Closed 2026-08-11: reviewed allowlist, environment-alias conflict tests, consumer audit, selected-root routing, and Windows target check pass. |
| 3 | Closed 2026-08-11: pure fixture matrix, sanitized public error codes, schema compatibility, auth protocol, and receipt manifest pass. Mutation/free-space/native permission behavior is owned by Phases 4/12. |
| 4 | Closed 2026-08-11: mutation core, lock/journal/resume, free-space/permission fixtures, rollback/cleanup queues, receipts, and CLI/API gates pass. Native Windows execution remains Phase 12. |
| 5 | Closed 2026-08-11: fresh canonical default, selected-root routing, helper/docs updates, Windows early logging, and persisted-path policy pass. Model relocation is isolated to Phase 6. |
| 6 | Closed 2026-08-11 for source-level macOS/Linux and cross-target contract: model-root preview/keep-or-copy API, receipt-backed selection, hash/free-space/collision/symlink protections, resumable copy, persistence rewrites, Migration settings controls, and focused gates pass. Native Windows model-root qualification is an explicit Phase 12 return marker. |
| 7 | Closed 2026-08-11: canonical package and binary, thin legacy alias, shared runner, CLI parity receipt, full Rust/JS/release gates, and Windows GNU target check pass. Native Windows launch and installed-binary discovery remain an explicit Phase 12 return marker. |
| 8 | Source-level closure remains valid; the two CI timing races from run `31603543480` are fixed and verified locally (focused Gemma 20/20, Rapid reachability 10/10; full suite 267 passed, 5 skipped, 3 flaky retries). External CI closure remains open until a fresh remote run is green. |
| 9 | Complete 2026-08-11: runtime identity centralization, deterministic canonical-first assets, checksum URL retention, exact process/task compatibility, legacy install-path preservation, CA continuity, entropy hardening, Token Ingot tray integration, and current-exe update paths pass source-level gates. Native Windows task/tray/mixed-version proofs remain explicit return markers. |
| 10 | Complete 2026-08-12: current docs/templates, upgrade guide, brand usage policy, CLI/API/runtime compatibility notes, historical allowlist, and 42-file relative-link validation pass. Fresh screenshot promotion remains Phase 13. |

## 1. Purpose and execution contract

This plan coordinates the complete public rebrand from Llama Monitor /
`llama-monitor` to **Local LLM Foundry**. It covers product identity, visual
identity, application roots, migration of existing user resources, package and
binary identity, frontend and backend surfaces, remote agents, updater behavior,
CI and releases, repository rename, documentation, compatibility, rollback, and
the 2.0 cutover.

Luna should execute this plan consecutively. Each phase is a fresh-context work
unit with its own prerequisites, owned files, tasks, tests, receipts, gate, stop
conditions, and rollback. A phase is not complete because code exists; it is
complete only when its evidence is captured and its pass gate is satisfied.

Rules:

- Preserve unrelated dirty-worktree changes. Re-read `AGENTS.md` at the start
  of every execution phase.
- Audit current source before editing. Paths and line numbers below are the
  2026-08-11 baseline, not permission to assume the checkout stayed unchanged.
- Use RTK-prefixed shell commands.
- Do not combine phases whose rollback boundaries differ.
- Never push to discover whether CI passes. Run the mandatory checks locally.
- Never add the `ready-to-test` label.
- Never treat old screenshots, status ledgers, or prior plan text as current
  proof.
- Commit only the files owned by the current phase.
- Stop on a genuine decision fork, data-integrity uncertainty, signing/package
  ownership issue, or failed gate. Do not improvise around it.

## 2. Frozen product decisions

These decisions are authoritative unless Nick explicitly changes them:

| Surface | Decision |
|---|---|
| Product name | **Local LLM Foundry** |
| Product short name | **Foundry** where space is constrained |
| Canonical slug | `local-llm-foundry` |
| New public Rust identifier form | `local_llm_foundry` when a new public type/field requires one; not a crate rename |
| Selected visual direction | **Token Ingot** |
| Concept source | `docs/brand/concepts/02-token-ingot.png` |
| Token Ingot validation proof | Accepted; proceed to deterministic SVG redraw and derivative proofs |
| macOS/Linux application home | `~/.config/local-llm-foundry/` |
| Windows application home | `%APPDATA%\\local-llm-foundry\\` |
| Default managed model root | `<application-home>/models/` |
| New binary/CLI | `local-llm-foundry` / `local-llm-foundry.exe` |
| New Cargo package | `local-llm-foundry` |
| Rust library crate | Keep `llama_monitor` as an explicit internal compatibility target in 2.0 |
| Default legacy model trees | Keep in place and effective until the user explicitly previews and executes migration |
| Existing API route policy | Keep routes stable throughout 2.x |
| Existing serialized protocol fields | Keep stable unless a field is explicitly branded |
| Backend terminology | Keep `llama.cpp`, `llama-server`, `llama` modules, GGUF, MLX, and Rapid-MLX where they describe technology rather than the old product |
| Historical records | Do not rewrite old release notes, benchmark receipts, or historical plans as if they used the new name; label them historical when needed |
| Runtime compatibility window | Accept supported legacy runtime names for all of 2.x; removal is no earlier than 3.0 |
| 2.0 release/share cutover | One coordinated big-bang release; share the update with the small friend/tester group after rebrand, migration, updater, repository, and release gates pass |
| Cleanup policy | No automatic deletion of the old application root or external models |
| Logo production policy | The generated PNG is direction artwork, not a shippable master; redraw it as deterministic SVG |

Recommended brand palette for the production redraw:

- Foundry teal: `#2dd4bf` on dark surfaces and `#0f766e` on light surfaces.
- Ingot copper: `#f97316`, used sparingly for the forged core.
- Charcoal: `#1f232a` or the current dark surface token.
- One-color variants must work in pure black and pure white.
- Do not add a custom webfont solely for the rebrand. Use the established UI
  type system unless a separately licensed typography decision is approved.

## 3. Decisions requiring Nick before their owning phase

Only these questions should stop execution:

1. **Canonical repository destination.** Recommended:
   `nmorgowicz-org/local-llm-foundry`. Confirm organization ownership and
   availability before Phase 11.
2. **Production Token Ingot redraw.** The direction and validation proof are
   accepted; Phase 1 may proceed with the hand-authored SVG. Final SVG/native
   proof remains an implementation gate, not a planning decision blocker.
3. **Code-signing, notarization, package registries, domains, and external
   distribution channels.** Inventory first; stop if credentials, ownership, or
   exact names are not locally discoverable.
4. **Legacy CLI alias duration if shorter than all of 2.x is desired.** Default
   is the entire 2.x line.

Resolved during planning: legacy default model trees remain in place and
effective until the user explicitly previews and executes relocation; the UI
may offer a toast/card, but never moves them during first launch.

Everything else in this plan has a recommended default and should proceed
without interruption.

## 4. Current source baseline and complete source-backed inventory contract

The executor must regenerate a complete inventory, but these findings shape the
phase order:

- An exact case-insensitive scan for `Llama Monitor`, `llama-monitor`, or
  `llama_monitor` currently reaches roughly 281 files across source, static
  UI, tests, documentation, scripts, workflows, and repository metadata.
- `Cargo.toml:2` names the package `llama-monitor`.
- `src/config.rs:468-544` resolves the default root and derives most resource
  paths.
- `src/main.rs:264` constructs `AppConfig` before the existing Windows
  migration at `src/main.rs:295-297`.
- `AppConfig::from_args` calls `ensure_api_token` and
  `ensure_db_admin_token` at `src/config.rs:530-536`; token creation creates
  the destination directory. The current Windows migration then returns when
  the destination exists. The new migration must run before all destination
  writes.
- `src/main.rs:163-229` contains a Windows-only best-effort copy/rename
  migration with no journal, conflict resolver, verification receipt, or safe
  recovery from a partial destination.
- `src/config.rs:85-118` embeds the HKDF info string
  `llama-monitor-encryption-key`, accepts
  `LLAMA_MONITOR_ENCRYPTION_KEY`, and stores `encryption-key` under the
  application root. This is cryptographic compatibility state, not cosmetic
  copy.
- `src/tray.rs:754-760` independently derives a log directory with
  `dirs::config_dir()`, bypassing `AppConfig` and producing a macOS path
  policy different from the frozen `~/.config` contract.
- `src/web/auth.rs:12` names the form cookie
  `llama_monitor_session`.
- `src/agent.rs:1911-1929` recognizes only legacy release asset names;
  existing 1.x clients therefore require a dual-name bridge release.
- `src/agent.rs:57-140` validates remote commands against only
  `llama-monitor[.exe]`.
- `src/agent.rs:2303-2316` hard-codes legacy remote install paths.
- `src/agent.rs:3937-4114` embeds updater staging, restart log, executable,
  and launcher identities.
- `src/models/library.rs:900-1589` already supplies a plan-id, journal,
  collision validation, relative-path rewrite, atomic JSON, and hashing pattern
  for model-library migration. Reuse its concepts; do not claim it migrates the
  application root.
- `src/chat_storage.rs:953-969` supplies `ChatStorage::backup()`. Direct file
  copies of a live `chat.db` are forbidden.
- `static/icon.svg` is a llama emoji tile and must be replaced by the approved
  Token Ingot asset family.
- `build.rs:155-320` generates static asset constants and routes from
  `static/`; a release build is required after asset changes.
- The current reference documentation itself contains stale claims about the
  existing Windows migration being effective. Documentation is inventory, not
  proof.

## 5. Phase 0 documentation discovery and allowed implementation patterns

Before implementation, Luna must create:

`docs/plans/evidence/20260811-local-llm-foundry/phase-00/`

Required receipts:

- `source-inventory.txt`: raw `rg` matches grouped by source class.
- `path-consumers.tsv`: literal, file, symbol/line, platform, ownership, new
  value, compatibility behavior, owning phase.
- `identity-contract.json`: machine-readable old/new mapping.
- `resource-inventory.json`: every file/directory below the legacy root,
  classification, size, symlink status, migration policy.
- `api-auth-matrix.tsv`: route, method, auth level, compatibility.
- `browser-storage-inventory.tsv`: cookie/storage key, producer, consumer,
  migration.
- `release-surface-inventory.tsv`: artifact, updater parser, workflow,
  checksum, consumer, bridge behavior.
- `screenshot-scenario-manifest.json`: scenario, group, viewport, theme,
  target surface.
- raw stdout/stderr for every inventory command and an exact produced-file
  manifest with SHA-256 hashes.

Allowed APIs and patterns:

- Copy the preview/execute/plan-id/journal model from
  `src/models/library.rs:900-1589`.
- Copy API auth separation from
  `src/web/api/models.rs:889-1020`: preview/read with `api-token`;
  execution with `db-admin-token` and explicit confirmation.
- Use `ChatStorage::backup()`, `checkpoint()`, and
  `restore_from_path()` from `src/chat_storage.rs`; never raw-copy a live
  SQLite database.
- Use HKDF-SHA-256 and constant-time secret comparison as specified in
  `docs/agents/security-details.md:72-120`.
- Use the canonical model-library behavior in
  `docs/reference/model-library.md`: explicit migration, no silent external
  deletion, collision and symlink refusal, restartable journals.
- Use the platform contract in
  `docs/agents/platform-details.md` and
  `docs/reference/windows-support.md`.
- Use release-built, isolated Playwright exactly as documented in
  `docs/agents/playwright.md:17-28`.
- Use `static/css/tokens.css` and the light-theme/reduced-motion rules in
  `docs/reference/ui-design-patterns.md:685-727`.
- Use the screenshot harness and groups in `tests/ui/capture/`; never run
  scenarios concurrently.

Anti-pattern guards:

- No global blind search-and-replace of `llama`; backend names are valid.
- No filename-derived model identity or architecture inference.
- No destination writes during path resolution.
- No migration after tokens, settings, databases, logs, or runtime state have
  initialized under the new root.
- No direct copy of a live SQLite database or omission of WAL handling.
- No following symlinks during inventory, size calculation, copy, cleanup, or
  rollback.
- No merge when both roots contain unrecognized user data.
- No automatic deletion of old roots, external models, or external HF caches.
- No changing the HKDF info string as a branding edit.
- No API route rename solely for branding.
- No accepting only new artifact names in the first 2.0 release.
- No broad `pkill -f` or task deletion that could target unrelated processes.
- No untrusted `innerHTML`/HTML interpolation.
- No generated raster embedded in the final SVG.
- No screenshots from debug builds, stale manifests, or imagined layouts.
- No renaming self-hosted runner labels until replacement infrastructure exists.

## 6. Naming and compatibility contract

| Surface | Legacy | 2.0 canonical | 2.x compatibility |
|---|---|---|---|
| Product | Llama Monitor | Local LLM Foundry | Old name appears only in migration/deprecation/history |
| Short UI name | Llama Monitor / monitor | Foundry | Preserve technical “monitor” verbs where natural |
| Slug | `llama-monitor` | `local-llm-foundry` | Accept old aliases through 2.x |
| Binary | `llama-monitor[.exe]` | `local-llm-foundry[.exe]` | Ship a thin legacy launcher/alias with one warning |
| Cargo package | `llama-monitor` | `local-llm-foundry` | No dual package; lockfile updates atomically |
| Rust crate | `llama_monitor` | Explicitly keep `llama_monitor` in 2.0 | Internal namespace is not public product branding |
| Backend modules | `llama::*` | unchanged where llama.cpp-specific | Never rebrand technology names |
| Repository | `nmorgowicz-org/llama-monitor` | recommended `nmorgowicz-org/local-llm-foundry` | Rely on GitHub redirect but update every canonical URL |
| Unix/macOS root | `~/.config/llama-monitor` | `~/.config/local-llm-foundry` | Legacy-root mode and explicit migration through 2.x |
| Windows root | `%APPDATA%\\llama-monitor` plus older `%USERPROFILE%\\.config\\llama-monitor` | `%APPDATA%\\local-llm-foundry` | Detect both legacy roots; never merge silently |
| Environment prefix | `LLAMA_MONITOR_*` plus standalone legacy variables | `LOCAL_LLM_FOUNDRY_*` | New wins; unequal dual values fail with an actionable error |
| Encryption env | `LLAMA_MONITOR_ENCRYPTION_KEY` | `LOCAL_LLM_FOUNDRY_ENCRYPTION_KEY` | Accept both; keep legacy HKDF info string or explicitly re-encrypt |
| Form cookie | `llama_monitor_session` | `local_llm_foundry_session` | Accept both; login sets new; logout clears both |
| Browser storage | Existing branded keys | Freeze as internal compatibility identifiers in 2.x | Centralize/allowlist; do not lose preferences for cosmetic cleanup |
| API routes | Existing `/api/*` | unchanged | No client break for a product rename |
| JSON fields | Existing fields | unchanged unless human-facing brand metadata | New protocol fields use `#[serde(default)]` |
| Remote agent task | legacy task names | `local-llm-foundry-agent` | Detect, stop, update, and remove both exact names |
| Install path | legacy slug/binary | new slug/binary | Preview exact transition; no shell-string replacement |
| Process names | legacy executable | new executable | Exact-match support for both during 2.x |
| Release assets | `llama-monitor-<platform>` | `local-llm-foundry-<platform>` | 2.0 bridge publishes both names with identical checksummed payloads |
| Models | legacy default tree | new default tree | External/custom paths remain external; legacy default may remain selected |
| Service worker/cache | inventory current name/version | new versioned identity | One-time cache eviction only for owned caches |
| Screenshot paths | existing scenario/artifact names | rename only current product-facing assets | Historical evidence remains immutable |

Environment alias precedence:

1. If only the new variable exists, use it.
2. If only the legacy variable exists, use it and emit one deprecation warning
   without logging its value.
3. If both exist and are byte-identical, use the new variable and emit no
   warning for secrets.
4. If both exist and differ, fail closed with the variable names only.
5. Test empty, non-Unicode, malformed, and platform-specific values.

## 7. Application-root migration architecture

### 7.1 Core rule

Path resolution must be pure. Migration selection and execution happen before
`AppConfig`, token creation, encryption initialization, logging under the new
root, model-tree creation, `ChatStorage::open`, tray startup, agents, update
checks, or background tasks.

Split the current startup into:

1. Parse CLI.
2. Resolve `AppIdentity` and candidate paths without writes.
3. Inspect roots and any migration request/journal.
4. Select fresh, legacy-compatible, new, conflict, maintenance-migration, or
   recovery mode.
5. Complete/resume/rollback maintenance work if requested.
6. Only then initialize `AppConfig`, secrets, databases, paths, runtime, web
   server, and tray.

### 7.2 Root states

Define a versioned, serializable state enum:

- `Fresh`: neither root contains app state; initialize the new root.
- `LegacyActive`: only a recognized legacy root exists; run from it without
  writes to the new root and show migration pending.
- `NewActive`: a valid new root exists; use it.
- `BothIdentical`: both roots match a completed receipt; use new and offer
  explicit cleanup.
- `Conflict`: both roots contain divergent or unrecognized state; do not
  merge, write, or choose silently.
- `MigrationQueued`: an authenticated preview was confirmed; require a
  controlled restart into maintenance mode.
- `Migrating`: a journal exists; resume exactly that operation.
- `MigrationFailed`: preserve both roots/staging and expose recovery.
- `RollbackAvailable`: new is active and the pre-migration legacy snapshot
  remains.
- `CustomConfig`: `--config-dir` is authoritative; do not auto-migrate or
  reinterpret it.

### 7.3 Two-stage migration policy

Stage A migrates application state. Stage B handles models explicitly.

Stage A critical resources:

- `encryption-key`, `api-token`, `db-admin-token`;
- `auth-config.json`, `tls-config.json`, TLS server keys/certificates,
  per-device CA material, agent tokens and remote-agent config;
- presets, sessions, UI settings, templates, GPU environment, known hosts,
  model tags/collections, community source/catalog state, and feature state;
- `chat.db` through `ChatStorage::backup()`, with integrity verification;
- migration and updater state whose semantics remain valid;
- provenance, journals, sidecar metadata, and resumable-download metadata that
  refer to managed resources.

Recreatable resources default to fresh state:

- release/runtime downloads that can be verified and reinstalled;
- updater staging files and temporary extraction directories;
- transient caches;
- logs, unless the user selects “copy logs” in preview.

Stage B model policy:

- A custom `--models-dir` or persisted external model path is recorded as
  external and never moved.
- A legacy default `<old-root>/models` remains the effective model root after
  Stage A until the user explicitly chooses either “keep here” or “move into
  Foundry.”
- “Keep here” persists the old model root as an explicit external path and is a
  supported steady state throughout 2.x.
- “Move” uses a new root-relocation plan plus the existing model-library
  planner. It must preview bytes, filesystem relationship, collisions,
  symlinks, partials, HF cache selection, path rewrites, and rollback.
- First relocate the model root; then organize legacy flat contents. Compose
  replacements so presets/sessions/drafts/mmproj/tag keys are rewritten once.
- External user-wide HF cache remains external unless repositories are
  explicitly selected through the existing import contract.

This order avoids an automatic multi-hundred-gigabyte copy during first launch
and avoids rewriting model paths twice.

### 7.4 Migration plan, journal, and receipt

Add versioned structures modeled on `LibraryMigrationPlan`:

- `AppHomeMigrationPlan`: schema version, plan ID, source/destination,
  root-state fingerprint, resource entries, model policy, total bytes,
  filesystem relationship, conflicts, warnings, required free space, and
  restart requirement.
- `AppHomeMigrationEntry`: relative source/destination only, resource class,
  kind, size, identity metadata, symlink state, operation, and verification
  policy.
- `AppHomeMigrationJournal`: operation ID, plan hash, current state,
  per-entry state, database backup state, staging root, timestamps, last safe
  checkpoint, and sanitized error.
- `AppHomeMigrationReceipt`: completed plan hash, source/destination
  fingerprints, exact resource manifest, exclusions, effective models root,
  backup location, verification results, application version, and completion
  time.

Plan IDs must cover all mutation-relevant inputs. Execution rejects a stale
plan, changed source, changed destination, new collision, changed free space,
or mismatched confirmation.

Write journals and receipts atomically. Store the active journal in a
validated sibling maintenance location so it exists before the destination is
committed. Never place a lock only inside a destination that does not yet
exist.

### 7.5 Copy and verification rules

- Inventory with `symlink_metadata`; reject symlinked roots and symlinked
  path components for managed moves.
- Accept only canonical descendants of recognized roots.
- Use same-filesystem rename only for staged, verified resources whose rollback
  is defined. Do not rename the live legacy root wholesale.
- Cross-volume copies are chunked, bounded, resumable, and written to a staging
  filename before atomic promotion.
- Refuse overwrite. Identical destination content may be recorded as already
  complete only after verification.
- Hash secrets, JSON, manifests, scripts, and small binaries. For very large
  model files, use size/mtime plus the model migration’s existing identity
  policy unless a full hash already exists; never pretend a partial hash is a
  full-content hash.
- Reapply restrictive permissions to secrets and private keys.
- Verify JSON parseability and expected schema after copy.
- Verify TLS key/certificate pair usability without exposing key material.
- Verify `chat.db` with SQLite integrity checks after online backup.
- Never log secret contents, auth headers, tokens, private paths returned to
  unauthenticated callers, or unbounded journal errors.

### 7.6 UI, CLI, and API contract

Proposed stable 2.x routes:

- `GET /api/app-home-migration/status`: `api-token`.
- `POST /api/app-home-migration/preview`: `api-token`, bounded and
  rate-limited.
- `POST /api/app-home-migration/queue`: `db-admin-token` plus exact
  confirmation `MIGRATE TO LOCAL LLM FOUNDRY`.
- `POST /api/app-home-migration/rollback/preview`: `api-token`.
- `POST /api/app-home-migration/rollback/queue`: `db-admin-token` plus
  exact confirmation.
- `POST /api/app-home-migration/cleanup`: `db-admin-token` plus an exact
  path-independent confirmation; no recursive deletion outside a receipt.

All request fields use `#[serde(default)]` where a safe default exists.
Malformed JSON returns 400. Missing plan IDs return 404 only when the resource
is genuinely absent. Expensive inventory/copy operations have concurrency
guards, timeouts where safe, and bounded retained status.

UI behavior:

- In legacy mode, show an authenticated persistent migration card/banner, not a
  blocking unauthenticated data view.
- Preview lists copied, recreated, retained-external, skipped, and conflicting
  resources with byte totals.
- Confirmation queues maintenance work for the next controlled restart. Do not
  switch the global encryption key, config root, or database in the live
  process.
- On the next start, maintenance mode completes or resumes before normal
  initialization, then starts from the new root.
- Conflict mode offers “Use new,” “Continue legacy,” and “Export diagnostics”;
  it never offers automatic merge.
- Cleanup remains a separate later action and shows the receipt’s exact target.

CLI behavior:

- `local-llm-foundry --app-home-migration-status`
- `local-llm-foundry --app-home-migration-preview`
- `local-llm-foundry --app-home-migrate --confirm ...`
- `local-llm-foundry --app-home-rollback --confirm ...`

Exact clap spelling may be refined in Phase 3, but the maintenance commands
must be available in headless mode and through the legacy binary alias.

### 7.7 Case matrix

| Case | Required behavior |
|---|---|
| Fresh install | Create only the new root |
| Old-only | Run legacy root, offer explicit Stage A migration |
| New-only | Use new root |
| Both valid and identical | Use new; offer receipt-scoped cleanup |
| Both divergent | Conflict mode; no merge or writes before choice |
| Empty new root created accidentally | Treat as empty only if it contains no unrecognized entry; remove only through journaled staging |
| Partial new root | Resume only with a matching journal; otherwise conflict |
| Explicit `--config-dir` | Use exactly that path; no automatic brand-root migration |
| Custom models path | Preserve and label external |
| Default old models | Keep effective old path until explicit model decision |
| Root-level legacy GGUF/partials | Include in Stage B model preview |
| Canonical GGUF/MLX/Transformers/Rapid-MLX | Preserve relative structure before optional organization |
| Incomplete `.part` and resume JSON | Preserve pair and validate reference |
| External HF cache | Audit only; never move without selection |
| Symlinked root/component | Refuse and explain |
| Cross-volume | Preview bytes/free space; resumable copy; retain source |
| Collision | Fail preview or execution; never overwrite |
| Interrupted copy | Resume from matching journal and verified checkpoints |
| Verification failure | Keep legacy active; preserve staging and diagnostics |
| Old service/script | Binary/env aliases and exact deprecation message |
| Running legacy service | Refuse destructive maintenance until the process is stopped |

### 7.8 Rollback and retention

- Stage A is copy-first. The legacy root stays intact and is the rollback point.
- Migration does not dual-write after cutover. Rollback preview must disclose
  new-root changes that would not exist in the legacy snapshot.
- Rollback is a root-selection operation performed before normal startup; it
  does not merge databases or JSON.
- Model rollback is governed by the separate model-migration journal and
  receipt.
- No timed deletion. Cleanup is explicit, receipt-scoped, symlink-safe, and
  requires db-admin authorization plus confirmation.
- A 3.0 cleanup phase may remove compatibility code, but never user data.

## 8. Execution phases

Every phase closure packet must explicitly record:

- objective and completed scope;
- prerequisites and serialized dependencies;
- exact files, symbols, APIs, and fixtures owned;
- compatibility behavior and anti-pattern guards;
- focused tests plus documentation/screenshot impact;
- raw commands, stdout/stderr, exit codes, produced-file manifest, and hashes;
- pass/fail gate result;
- stop conditions, rollback path, commit, and next phase.

If a field is not applicable, the receipt says why rather than omitting it.

### Phase 0 — Documentation discovery and baseline receipts

**Objective**

Freeze a current, reproducible source and documentation baseline before any
rename or migration edit.

**Prerequisites and serialization**

- First implementation phase; no code changes may precede it.
- Preserve the current untracked plan, concept assets, and all unrelated work.

**Tasks**

- [x] Read `AGENTS.md`, this plan, security/platform/Playwright references,
  model-library, binary-lifecycle, remote-agent, Windows, CLI, API, UI patterns,
  screenshot workflow, release workflows, and release-please configuration.
- [x] Run exact old-name scans separately for Rust, frontend, tests, docs,
  workflows, scripts, assets, filenames, environment variables, URLs, cookies,
  browser keys, task/service/process names, artifacts, and generated files.
- [x] Classify every match as public brand, compatibility identifier,
  historical record, infrastructure-only name, or legitimate llama.cpp/LHM
  terminology.
- [x] Inventory all `AppConfig` fields and every direct
  `dirs::*`/`HOME`/`APPDATA`/literal path consumer.
- [x] Inventory files below representative legacy roots on macOS/Linux and
  Windows fixtures, including permissions, sizes, symlinks, WAL sidecars,
  partials, caches, journals, and unknown files.
- [x] Inventory all browser storage keys before module initialization.
- [x] Inventory current and generated brand assets, including the three
  unrelated current marks.
- [x] Write every Phase 0 receipt listed in Section 5 and a SHA-256 manifest.

**Owned outputs**

- `docs/plans/evidence/20260811-local-llm-foundry/phase-00/**`
- `scripts/validate-rebrand-phase0.mjs` (audit-only fail-closed receipt
  validator; not product/runtime implementation)

**Verification**

- Raw commands, stdout, stderr, exit codes, timestamps, checkout SHA, and exact
  manifests must be retained.
- A fail-closed checker must reject an unclassified old-name match.
- [x] Current stale claims, including the existing Windows migration description,
  must be labeled as documentation rather than proof.

**Pass gate**

Every discovered identity/path/resource match has an owner phase and policy;
the produced-file manifest matches disk; and the baseline receipt proves no
product/runtime implementation file changed before Phase 0 capture. Later
implementation edits are owned by Phases 1–3 and are not attributed to this
discovery phase.

**Stop/rollback**

Stop if the inventory cannot distinguish user data from app-owned data. Rollback
is deletion of Phase 0 evidence only if Nick requests it; otherwise evidence is
append-only.

### Phase 1 — Identity contract and Token Ingot production assets

**Objective**

Turn the selected Token Ingot direction into an approved, deterministic visual
system and a machine-readable naming contract.

**Prerequisites**

- Phase 0 passed.
- Token Ingot direction is frozen.

**Tasks**

- [x] Commit `identity-contract.json` and a human-readable contract under
  `docs/reference/`.
- [x] Hand-reconstruct the mark; do not auto-trace
  `docs/brand/concepts/02-token-ingot.png`.
- [x] Store the canonical editable SVG source under `assets/brand/` with a
  square viewBox, simple geometry, no raster, filter, font, script, or external
  reference.
- [x] Simplify the three layers and widen negative spaces for 16–24 px.
- [x] Produce full-color, dark, light, one-color dark, one-color light, and
  macOS template variants.
- [x] Produce deterministic web/PWA derivatives: favicon SVG/ICO, Apple touch
  180 PNG, regular 192/512 PNG, separately padded maskable 192/512 PNG.
- [x] Produce tray/app/package derivatives for macOS, Windows, and Linux plus
  proof renders at 16, 20, 22, 24, 32, 64, 128, 256, 512, and 1024 px.
- [x] Produce repository/social artwork at 1200×630 without inventing a slogan.
- [x] Add named brand tokens for teal, copper, and charcoal without
  repurposing semantic warning/success tokens.
- [x] Fix `build.rs` before serving binary PNG/ICO from `static/`:
  binary extensions must use `include_bytes!` and byte replies; keep
  `include_str!` for text.
- [x] Add generator tests and HTTP tests for MIME, byte integrity, SVG safety,
  and manifest icon purpose.
- [x] Replace duplicated inline marks only after the master is approved.
- [x] Perform a basic visual-similarity/trademark search and record results.

**Owned files**

- `assets/brand/**`
- `static/brand/**`, `static/icon.svg`, `static/manifest.json`
- packaging brand asset directory introduced by this phase
- `static/css/tokens.css`
- `build.rs`, `src/gen/static_assets.rs`, `src/gen/routes.rs`
- focused static-asset tests and brand reference documentation

**Compatibility**

- Keep old asset URLs only if an installed manifest/browser can request them;
  serve the new image at those URLs during 2.x rather than leaving the llama
  mark.
- The concept PNG remains design history and is never the production master.

**Verification and evidence**

- SVG lint/safety check; deterministic export hashes; transparent-background
  verification; maskable safe-zone proof; small-size contact sheet.
- `cargo build --release` regenerates static files.
- HTTP tests prove binary assets are not UTF-8 `include_str!`.
- Visual proofs on dark/light backgrounds and actual native tray lanes.
- Nick approves the final SVG before integration continues.

**Pass gate**

One approved canonical master, complete derivative matrix, reproducible export
receipt, static generator support, and no current UI integration divergence.

**Stop/rollback**

Stop on similarity concern, unreadable small sizes, or unapproved vector
geometry. Rollback keeps concept assets and removes only unapproved production
derivatives.

### Phase 2 — Central identity and pure path authority

**Objective**

Create one source of truth for public identity, compatibility aliases, and all
application-derived paths before changing defaults.

**Prerequisites**

- Phases 0–1 passed.

**Tasks**

- [x] Add `src/identity.rs` for product/binary/repository/artifact constants
  and explicit legacy aliases.
- [x] Add `src/paths.rs` for pure candidate-root resolution and derived
  `AppPaths`.
- [x] Add injectable platform inputs so Windows/macOS/Linux path tests run on
  one host without mutating the real environment.
- [x] Split `AppConfig::from_args` into pure resolution and explicit
  initialization. Token creation, directory creation, config loading, and
  permission changes are not allowed in resolution.
- [x] Initialize the selected encryption key before loading/decrypting token or
  protected config files. Add a restart regression proving an encrypted token
  never enters the live store as a literal `enc:` envelope.
- [x] Make recovery/status commands such as `--clear-auth-config` resolve
  paths without creating replacement tokens or unrelated resources.
- [x] Represent root selection as the state enum in Section 7.2.
- [x] Pass resolved paths into tray logging, Windows redirection, certs, agent,
  updater, HF, model library, runtime sidecars, scripts, logs, and tests.
- [x] Remove direct product-root derivation from `src/tray.rs`,
  `src/main.rs`, `src/agent.rs`, and other consumers.
- [x] Add environment alias parsing with the precedence and secret-safe errors
  in Section 6.
- [ ] Preserve `llama` modules and `llama-server` names that refer to the
  backend.
- [x] Add a literal-policy test with a reviewed allowlist; new unowned legacy
  literals fail.

**Owned files/symbols**

- New `src/identity.rs`, `src/paths.rs`
- `src/main.rs`, `src/config.rs::AppConfig::from_args`
- `src/tray.rs::open_logs_folder` and Windows log redirection
- only the minimum consumer edits needed to compile against `AppPaths`
- focused unit tests and identity/path reference docs

**Compatibility**

- Defaults do not switch yet. This phase creates authority while current
  behavior remains selected behind the compatibility state.
- `--config-dir` remains exact and authoritative.

**Verification**

- Table-driven path tests for fresh/legacy/new/both/custom on all platforms.
- Tests prove path resolution creates no file or directory.
- Tests prove conflicting env aliases fail without printing values.
- Old-literal checker passes only its explicit allowlist.
- Windows cross-check if touched by cfg changes.

**Pass gate**

Every path consumer receives a resolved path; pure resolution has zero
filesystem side effects; no visible rename has landed.

**Stop/rollback**

Stop if a consumer cannot be routed through the authority without changing
runtime semantics. Roll back this phase as one refactor commit.

### Phase 3 — Migration specification, protocol, and fixtures

**Objective**

Make migration behavior executable as pure planning against comprehensive
fixtures before implementing mutations.

**Prerequisites**

- Phase 2 passed.

**Tasks**

- [x] Add versioned plan/journal/receipt schemas from Section 7.4.
- [x] Add fixture builders for fresh, old-only, both, empty, partial,
  cross-volume-simulated, symlink, collision, permission-denied, unknown-file,
  custom config, custom models, old default models, partial downloads, HF cache,
  TLS/certs/tokens, chat DB/WAL, agents, and updater state.
- [x] Implement read-only root inventory and root-state classification.
- [x] Implement deterministic plan IDs and stale-plan rejection.
- [x] Classify resources as critical-copy, recreate, models-deferred,
  external, unknown, or conflict.
- [x] Define sanitized public error codes separately from local diagnostics.
- [ ] Specify exact confirmation strings, auth level, rate limit, timeout,
  concurrency, restart, and cleanup behavior.
- [x] Add schema compatibility tests with missing fields and
  `#[serde(default)]`.
- [x] Document that legacy mode remains functional without creating the new
  root.

**Owned files**

- New `src/migration/mod.rs`, `src/migration/app_home.rs` or equivalent
- fixture modules under `tests/`
- migration API contract in `docs/reference/api.md`
- new application-home migration reference/upgrade guide skeleton

**Copy-ready patterns**

- `LibraryMigrationPlan`, `MigrationJournal`, `atomic_json`, path
  validation, and tests in `src/models/library.rs:900-1589`.
- Auth/concurrency patterns in `src/web/api/models.rs:21-53,889-1020`.

**Verification**

- Pure preview tests for every case matrix row.
- Same inputs produce identical plan IDs.
- Any changed file/collision/free-space input invalidates the plan.
- No preview test leaves a filesystem mutation.

**Pass gate**

The full case matrix has deterministic expected plans and errors, schemas are
documented, and zero mutation code is reachable.

**Stop/rollback**

Stop on an unknown resource that might contain user data or a platform behavior
without a fixture. Rollback removes only the new pure planner/fixtures.

### Phase 4 — Application-home migration core

**Objective**

Implement restartable, idempotent, verified Stage A migration without changing
the active root in a live initialized process.

**Prerequisites and serialization**

- Phase 3 passed.
- Must precede path-default switch, package rename, and frontend rebrand.
- No parallel changes to `main.rs`, `config.rs`, auth/token initialization,
  or migration modules.

**Tasks**

- [x] Run root classification before all destination writes.
- [x] Implement queue-on-live / execute-on-maintenance-restart behavior.
- [x] Implement sibling lock/journal with bounded stale-lock recovery.
- [x] Implement staging, atomic promotion, resume, and verified idempotency.
- [x] Copy `encryption-key` unchanged and preserve the legacy HKDF info
  string. Add new env alias support without re-deriving existing ciphertext.
- [x] Use `ChatStorage::checkpoint()` and `backup()`; verify the destination
  database with integrity checks. Do not copy live WAL files.
- [x] Copy critical JSON/secrets/certs with schema and permission verification.
- [x] Recreate or explicitly skip transient/runtime/cache resources.
- [x] Record the legacy model root as the effective external root; do not move
  models in Stage A.
- [x] Refuse unrecognized destination data, symlinks, traversal, collisions,
  insufficient space, changed plans, and active legacy sibling processes.
- [x] Implement status, preview, queue, rollback preview/queue, and
  receipt-scoped cleanup endpoints with the auth contract in Section 7.6.
- [x] Implement headless maintenance CLI equivalents.
- [x] Write success/failure receipts and sanitized diagnostics.

**Owned files**

- migration modules and fixtures
- startup bootstrap portions of `src/main.rs`
- initialization split in `src/config.rs`
- migration routes/module registration, auth-routing tests, CLI flags
- application-home migration reference docs

**Compatibility**

- Old-only installations continue in legacy-root mode until confirmed.
- Explicit custom config roots do not migrate.
- Old root remains intact after Stage A.
- New application-root default is enabled only after all migration gates pass.

**Verification**

- Unit/integration tests for every case matrix row.
- Kill/restart injection after every journal checkpoint; resume must converge.
- Database round-trip with WAL activity and integrity verification.
- Secret decryption before and after migration with both env aliases.
- Permission tests; symlink/traversal/collision tests.
- `tests/auth_routing.rs` covers no auth, API token, db-admin token, form/basic
  modes, malformed JSON, and confirmation.

**Pass gate**

Every migration fixture either completes with a matching receipt or fails
without changing the active legacy root. Re-running success is a no-op.

**Stop/rollback**

Stop on any database integrity failure, key mismatch, partial destination
without matching journal, permission weakening, or data classification gap.
Rollback chooses the intact legacy root before startup; it never merges.

### Phase 5 — Path/config/resource consumers and default-root switch

**Objective**

Move every application-owned consumer to the new path authority and make the
new root canonical for fresh installations.

**Prerequisites**

- Phase 4 passed with receipts.

**Tasks**

- [x] Switch fresh defaults to the new root on all platforms.
- [x] Route presets, sessions, UI settings, templates, GPU env, tags,
  collections, community state, chat DB/backups, tokens, encryption key,
  TLS/certs/CA, SSH known hosts, agent state, updater state, logs, binaries,
  scripts, runtime metadata, MTP pins/sidecars, journals, provenance, staging,
  HF app cache, and tray state through `AppPaths`.
- [x] Remove the old Windows best-effort migration and replace its documentation
  with the new state machine.
- [x] Ensure Windows early logging uses the selected active root.
- [x] Ensure macOS uses the frozen `~/.config` policy, not
  `dirs::config_dir()`.
- [x] Update helper scripts only when they consume the app default; preserve
  explicit user paths.
- [x] Audit absolute paths persisted in JSON/SQLite; Stage A leaves model and
  external paths byte-identical, and any managed-model rewrites are receipt-scoped
  to Phase 6 relocation only.
- [x] Add effective-path diagnostics that expose no secrets.

**Owned files**

- all path consumers listed by Phase 0, disjoint by file assignment
- config/path docs, CLI persisted-location docs, Windows support docs

**Compatibility**

- Legacy-active mode still routes every consumer to the old root.
- Custom paths remain exact.
- No consumer may independently concatenate a product slug.

**Verification**

- Literal-policy scan.
- Fresh and migrated application integration tests on all three platform
  policies.
- Tests prove tray/log/runtime/model/agent paths agree with `AppPaths`.
- Windows cross-check and release build.

**Pass gate**

No unowned direct application-root derivation remains and all fixture roots
resolve to the expected resources.

**Stop/rollback**

Stop if a consumer opens old and new resources in one run. Roll back the
default switch while retaining the path authority and migration core.

### Phase 6 — Model-library relocation and path integration

**Objective**

Support safe, explicit migration of default legacy model resources without
moving custom/external data or losing incomplete work.

**Prerequisites and serialization**

- Phase 5 passed.
- Serialize with all changes to `src/models/library.rs`, model download paths,
  Rapid-MLX sidecars, HF cache, presets, and sessions.

**Tasks**

- [x] Add the “keep legacy location” and “move into Foundry” decisions to the
  migration center.
- [x] Persist “keep” as an explicit external models root with relocation-plan and
  receipt provenance.
- [x] Build a pure model-root relocation preview with filesystem, byte, free-space,
  symlink, collision, partial/resume, cache, sidecar, and rewrite inventory.
- [x] On any filesystem, copy-first with a resumable journal, verified entries,
  and retained source; same-volume optimization remains optional and never
  weakens rollback safety.
- [x] Preserve/inventory `gguf/`, `mlx/native/`, `mlx/converted/`,
  `transformers/`, `rapid-mlx/`, `cache/huggingface/`, `.staging/`,
  provenance, completion markers, journals, and sidecars.
- [x] Preserve/inventory root-level legacy GGUF and partials for the existing organization
  planner.
- [x] Compose relocation replacements with the existing model-library plan so
  presets, sessions, draft/mmproj paths, path-keyed tags, and in-memory state
  rewrite once.
- [x] Never move unselected external HF repositories.
- [x] Revalidate all managed cache manifests and incomplete-download metadata.
- [x] Add cleanup/rollback as receipt-scoped actions separate from execution.

**Owned files**

- `src/models/library.rs` extensions or a new relocation module
- `src/web/api/models.rs` migration endpoints
- model state/preset/session rewrite integration
- migration center model-root controls introduced here; broader migration UX polish remains Phase 8
- model-library/HF/VRAM/spawn-wizard docs as applicable

**Verification**

- Existing migration tests remain green.
- New fixtures cover large-file simulation, cross-volume resume, structured
  trees, flat GGUF, partial pairs, selected/unselected HF cache, sidecars,
  custom roots, collisions, symlinks, interruption, and rollback.
- Model inventory before/after has the same logical ready/incomplete entries.
- No external file mutation in “keep” or unselected-cache cases.

**Pass gate**

Every supported model resource is preserved or explicitly external, path
rewrites are exact, and the logical inventory is equivalent after relocation.

**Stop/rollback**

Stop on unknown cache/provenance format, missing partial companion, ambiguous
path rewrite, or insufficient destination verification. Rollback follows the
model journal only; never infer moves from filenames.

### Phase 7 — Package, crate, binary, CLI, and backend public identity

**Objective**

Make Local LLM Foundry the executable/package identity while preserving a safe
2.x legacy launch path.

**Prerequisites**

- Phases 2–6 passed.
- Migration works when invoked through both canonical and legacy CLI names.

**Tasks**

- [x] Rename Cargo package to `local-llm-foundry`, add an explicit
  `[lib] name = "llama_monitor"`, and document why the internal crate remains
  stable in 2.0.
- [x] Move application entry logic into a shared runner and create thin
  canonical and legacy binary entrypoints.
- [x] The legacy `llama-monitor` entrypoint emits one actionable deprecation
  warning, preserves argv/exit codes, and invokes the identical runner.
- [x] Keep Rust/test imports on `llama_monitor`; do not rename backend
  `llama` modules.
- [x] Update clap name, help, and version output. Current-doc examples,
  recovery copy, and any generated shell completions remain owned by Phase 10.
- [x] Carry forward the new environment variables and alias resolver from
  Phase 2.
- [x] Update process/executable discovery to exact-match both names.
- [x] Update default installed binary paths without changing
  `llama-server`.
- [x] Add tests comparing canonical and legacy CLI behavior, paths, migration
  plan IDs, error text, and exit codes.

**Owned files**

- `Cargo.toml`, `Cargo.lock`, binary entrypoints, `src/lib.rs`,
  `src/main.rs`/shared runner, `src/cli.rs`
- Rust integration test imports and CLI reference

**Compatibility**

- Ship both executable names in all 2.x release bundles by default; release
  artifact filenames have a separate 2.1 cutover described in Phase 11.
- Internal technology names remain stable.
- No API route or protocol field rename.

**Verification**

- Canonical and alias `--help`, `--version`, migration maintenance, agent,
  headless, and invalid-argument tests.
- Cargo build/check/test and Windows cross-check.
- Literal-policy scan distinguishes the intentional legacy entrypoint.

**Pass gate**

New binary is canonical; legacy binary runs the same application with only the
documented warning; backend behavior and APIs are unchanged.

**Stop/rollback**

Stop if the dual entrypoints diverge, release packaging cannot carry both, or a
crate rename leaks into serialized/API identity. Roll back package/binary rename
as one commit.

### Phase 8 — Frontend, auth cookie, browser storage, and migration UX

**Objective**

Replace all current product-facing UI identity, integrate Token Ingot, and
provide non-destructive browser/auth compatibility and migration controls.

**Prerequisites**

- Production brand assets approved.
- Backend migration/API and canonical CLI contracts passed.

**Tasks**

- [x] Replace browser title, manifest name/short name, favicon/touch/PWA icons,
  auth shell, navigation mark/title, welcome/setup mark/copy, compact title and
  heading, restart dialog, settings/help placeholders, recovery commands,
  update copy, remote-agent copy, toasts, dialogs, and notifications.
- [x] Replace duplicated inline marks with the shared production asset or safe
  deterministic inline SVG; decorative marks are `aria-hidden`.
- [x] Add a small frontend identity module for dynamic product/CLI/repository
  copy. Static HTML remains covered by the literal scan.
- [x] Preserve `llama.cpp`, `llama-server`, llama backend controls, and all
  LibreHardwareMonitor/`LHM` terminology.
- [x] Implement the authenticated legacy-mode migration card, preview,
  confirmation, queued-restart, progress/recovery, conflict, rollback, and
  cleanup UI.
- [x] When managed legacy model trees are detected, show a non-blocking toast
  linking to the migration card (using the established model-migration toast
  pattern). The toast must explain that models remain where they are until the
  user explicitly previews and executes relocation; dismissing it must not alter
  the effective model root.
- [x] Centralize and document existing `llama-monitor-*` and
  `llama_monitor_*` browser keys as frozen 2.x compatibility identifiers.
  Add them to the literal-policy allowlist and test that the rebrand preserves
  every preference.
- [x] If a future phase renames browser keys, it must use a pre-bootstrap helper
  loaded before `bootstrap.js` and compact initialization; that migration is
  explicitly out of scope for 2.0 unless Nick overrides this decision.
- [x] Rename the cookie to `local_llm_foundry_session`; backend accepts both,
  login sets new, logout expires both. JavaScript never touches HttpOnly
  cookies.
- [x] Keep `sw.js` passthrough unless offline support is separately scoped;
  do not invent Cache Storage migration.
- [x] Add light-theme styles, narrow behavior, focus/ARIA, forced-colors smoke,
  and reduced-motion overrides for every new animation/transform.
- [x] Update/add focused Playwright tests and capture scenarios; do not capture
  final evidence until Phase 13.

**Owned files**

- `static/index.html`, `static/compact.html`, `static/manifest.json`,
  `static/sw.js` only if text requires it
- brand/layout/setup/auth CSS and tokens
- browser compatibility registry and dynamic identity JS
- affected feature modules, auth backend/tests, migration API UI
- focused Playwright/capture scenario definitions

**Copy-ready patterns**

- Capture theme/narrow/reduced motion from
  `tests/ui/capture/scenarios/presets/evidence-drawer.mjs`.
- Appearance preference setup from
  `tests/ui/capture/scenarios/config/appearance-palette.mjs`.
- App dialogs from `static/js/features/toast.js:259-705`.
- Safe SVG DOM construction from
  `static/js/features/chat-notes.js:setSvgIcon`.

**Verification**

- DOM and manifest tests for canonical product identity.
- Asset URLs return correct MIME and bytes.
- Browser preference round-trip for every inventoried key in main and compact
  views, including corrupt-value tolerance and storage events.
- Cookie matrix in Rust auth tests.
- Old product absent from visible current UI while legitimate backend/LHM terms
  remain.
- `npm run validate-js`, `npm run lint`, focused and full isolated UI tests.

**Pass gate**

All current user-facing surfaces use the new identity, preferences/auth survive,
migration UX is complete, and accessibility/theme/motion gates pass.

**Stop/rollback**

Stop on preference loss, auth lockout, inaccessible migration actions, or icon
failure at small sizes. Rollback UI branding and storage canonical writes while
retaining read compatibility.

### Phase 9 — Runtime, agent, updater, tray, and platform identities

**Objective**

Rebrand and migrate operational identities without leaving duplicate services,
breaking old agents, or stranding 1.x updaters.

**Prerequisites and serialization**

- Phases 4–8 passed.
- Serialize all edits to `src/agent.rs`, `src/lhm.rs`, updater routes,
  release asset parsing, remote paths/tasks, tray, and platform bundles.

**Tasks**

- [x] Centralize canonical and legacy release repository URLs, asset contracts,
  install paths, task names, process names, temp files, restart logs, user-agent
  strings, and executable names under `identity.rs`.
- [x] Fix the checksum asset data flow: `latest_release_info()` currently
  filters through `asset_info_from_github_asset()`, which discards
  `checksums.json`, while `fetch_checksums_json()` later searches the
  filtered list. Add a regression test before relying on updater evidence.
- [x] Make asset matching deterministic: canonical family first, legacy
  fallback; never depend on GitHub response order.
- [x] Use `current_exe()` for replacement/relaunch so old-name and new-name
  installations both update in place.
- [x] Accept both exact executable names in remote command validation.
- [x] Preview and migrate remote install paths; preserve explicit custom paths and keep discovered legacy paths until explicit migration.
- [x] Introduce canonical Windows tasks
  `LocalLLMFoundryAgent` and `LocalLLMFoundrySensorBridge`.
- [x] Detect old task families, install/verify the new authenticated task, then
  retire the exact old task. Never leave two active SYSTEM tasks.
- [x] Pass an explicit resolved `--config-dir` to Windows scheduled tasks so
  SSH-user and SYSTEM-profile `%APPDATA%` cannot diverge.
- [x] Migrate SYSTEM-profile agent data separately from the SSH user’s install
  path via explicit resolved `--config-dir`, retaining legacy discoverability.
- [x] Preserve old CA/key identity; valid legacy CA pairs are reused without
  sentinel-dependent rotation; `llama-monitor CA` remains unchanged.
- [x] Replace agent timestamp/PID randomness fallback with system entropy.
- [x] Harden temporary token transport with system entropy, exclusive creation,
  restrictive permissions, canonical filenames, and a legacy read fallback.
- [x] Update exact stop/status/remove operations for both name families; remove
  broad product-name `pkill -f` usage where it can hit unrelated processes.
- [x] Replace the programmatic tray monitor glyph with the approved Token Ingot
  asset; retain macOS template behavior.
- [x] Update tray tooltip/menu/restart/log actions and compact popover identity.
- [ ] Record that no first-class systemd/launchd installer currently exists;
  do not invent one solely for parity. If external installer repos are found,
  assign them an explicit owner and gate.

**Owned files**

- `src/agent.rs`, `src/lhm.rs`, `src/certs.rs`, updater/auth routes/tests
- `src/tray.rs`, compact popover resources, platform asset integration
- remote-agent, binary-lifecycle, TLS, Windows, and platform docs

**Compatibility**

- Old agents/tasks/paths remain discoverable and manageable through 2.x.
- New controller tolerates old protocol identity; API routes/protocol version
  change only if behavior requires it, with `#[serde(default)]`.
- Existing CA trust remains valid.

**Verification**

- Exact tests for both binary names, paths, task families, process families,
  archive names, current-exe relaunch, checksums, certificate continuity, and
  agent auth.
- Mixed controller/agent matrix: old controller→bridge asset, new
  controller→old agent, new controller→new agent.
- Native Windows scheduled-task tests and native tray proofs; cross-compile is
  not a substitute.
- Security tests for randomness, temp permissions, shell validation, and secret
  redaction.

**Pass gate**

Canonical operational identities work, every legacy identity has explicit
2.x handling, update checksums are actually reachable, and no duplicate task or
trust break remains.

**Stop/rollback**

Stop on lost remote connectivity, certificate reenrollment, checksum bypass,
duplicate SYSTEM tasks, ambiguous process targeting, or native tray failure.
Rollback re-enables the verified old task/path and leaves compatibility readers.

### Phase 10 — Documentation, API, CLI, migration, and historical policy — **Complete**

**Objective**

Make all current documentation read as if Local LLM Foundry 2.0 always existed,
while keeping historical evidence truthful.

**Prerequisites**

- Product/API/CLI/migration/runtime behavior is stable through Phase 9.

**Tasks**

- [x] Rewrite README product introduction, features, install, run, update,
  remote agent, screenshots, repository links, and migration notice.
- [x] Update `docs/README.md`, quick start, CLI flags, config/filesystem,
  security, TLS, model library, HF browser, spawn/setup wizard, llama.cpp and
  Rapid-MLX runtime, binary lifecycle, remote agent/SSH, Windows, dashboard,
  chat, API/auth routing, external clients, contribution/issue guidance, and
  UI design patterns.
- [x] Add a dedicated 1.x→2.0 upgrade guide with all root states, model choices,
  compatibility identifiers, receipts, rollback, cleanup, troubleshooting,
  and headless commands.
- [x] Document legacy names accepted through 2.x and removal no earlier than
  3.0.
- [x] Correct stale updater checksum and Windows migration claims.
- [x] Document that browser storage keys and the internal Rust crate remain
  stable compatibility identifiers.
- [x] Document Token Ingot usage, prohibited modifications, palette, clear
  space, small-size variants, and asset-source workflow.
- [x] Update issue forms and current templates.
- [x] Classify historical plans, changelog links, benchmark receipts, release
  notes, calibration fixtures, and third-party names; do not bulk rewrite them.
- [x] Update screenshot references only after Phase 13 promotes fresh images;
  fresh promotion remains a Phase 13 return marker.

**Owned files**

- README, SECURITY/current policy docs, `docs/reference/**`,
  `docs/agents/**`, issue/contribution templates
- no historical receipt mutation

**Verification**

- Documentation link checker and exact current-doc brand scan.
- Commands and paths copied from current `--help` and runtime behavior.
- Historical allowlist reviewed line by line.
- Unused screenshot checker after promotion.

**Receipt**

Current docs were checked with a 42-file relative-link validator and the
canonical `local-llm-foundry --help` command. Historical plans, changelog,
benchmark receipts, calibration fixtures, and screenshots were not bulk
rewritten. Legacy names remain supported through 2.x; earliest planned removal
is 3.0.0.

**Pass gate**

Every current behavior has accurate new-name documentation; every legacy
compatibility behavior and removal boundary is explicit; historical material
remains truthful.

**Stop/rollback**

Stop if documentation claims behavior not yet proven. Roll back only inaccurate
current-doc edits; never rewrite receipts to make a gate pass.

### Phase 11 — CI, release-please, packaging, dual assets, and repository rename

**Current execution position (2026-08-12):** source-controlled implementation
and authorized GitHub cutover gates are complete; release launch gates remain
open. The latest completed CI run (`31627911658`) passed CodeQL, lint, and
Windows clippy but failed the Rust `check` job before UI qualification: one
Rapid-MLX compatibility probe hit a transient Linux `ETXTBSY` (“Text file
busy”) spawn error. The probe now has a bounded retry for that specific error;
the exact test and full Rust suite pass locally. The earlier UI fixes remain
verified by focused release-built repeats and a complete release-built UI run;
a fresh remote run is still required before this phase can close. The release
workflow now publishes the four canonical and four legacy 2.0.x bridge assets,
checksums them fail-closed, preserves archive layouts, and validates the source
identity contract. See `evidence/20260811-local-llm-foundry/phase-11/README.md`.
The downloaded Playwright report is retained at
`/tmp/local-llm-foundry-ci-31615465442/playwright-report/`.

Completed in this phase: release-please package identity, canonical CI paths and
cache keys, release path filters, dual-asset packaging, exact checksum coverage,
2.1 canonical-only policy encoding, archive contract validators, and frozen
bridge fixtures. The repository now redirects from
`nmorgowicz-org/llama-monitor` to `nmorgowicz-org/local-llm-foundry`; old and new
git URLs resolve to the same `main` HEAD. PR #314 uses the proven major-release
title `feat!: launch Local LLM Foundry 2.0 with backend-neutral Rapid-MLX` and a
compact 20-entry override block. The pending 1.8.2 release-please PR will be
superseded by this merged feature delta. Remaining before Phase 11 closure:
push the UI qualification fixes and obtain a fresh passing CI run, generate 2.0.0 release artifacts,
complete real updater probes, and validate the self-hosted runner after the
repository rename.

**Objective**

Prepare reproducible 2.0 packaging and rename the GitHub repository without
breaking runners, releases, redirects, or last-supported 1.x updates.

**Prerequisites and serialization**

- Phases 0–10 passed.
- Nick confirmed the repository destination and external ownership.
- Freeze concurrent merges during the actual repository rename.

**Tasks**

- [ ] Update release-please package identity and ensure the rebrand squash is
  breaking: `feat!:` and a `BREAKING CHANGE:` body.
- [ ] Hard-gate the generated release PR at exactly 2.0.0; reject 1.9.0.
- [ ] Update CI binary paths, logs, test identity aliases, cache keys, package
  names, issue forms, and current repository URLs.
- [ ] Retain `arc-llama-monitor`/`arc-llama-monitor-fast` runner labels until
  replacement infrastructure is proven. They are external operational IDs.
- [ ] Extend release-smoke path filters to release workflows/config and build
  scripts so packaging changes cannot skip CI.
- [ ] Build four canonical and four legacy-named assets on every 2.0.x release;
  this is the one-time 1.x-to-2.0 bridge and must be complete before 2.1.0:
  - `local-llm-foundry-linux-x86_64`
  - `local-llm-foundry-linux-aarch64`
  - `local-llm-foundry-windows-x86_64.zip`
  - `local-llm-foundry-macos-aarch64.tar.gz`
  - the equivalent four `llama-monitor-*` aliases. These are four target builds
    exposed under two compatible filenames, not eight independent compilations.
- [ ] Generate `checksums.json` covering all eight exact filenames.
- [ ] Make 2.1.0 the artifact cutover: publish only the four canonical filenames
  and prove a fully updated 2.0 client discovers them without legacy names.
- [ ] Preserve legacy archive layout:
  old macOS archive contains the old payload filename; old Windows ZIP contains
  `llama-monitor.exe`, `sensor_bridge.exe`, and
  `WebView2Loader.dll`.
- [ ] Canonical Windows ZIP contains the canonical executable and required
  bridge/support files defined by the release contract.
- [ ] Add `scripts/validate-rebrand-contract.mjs` and
  `scripts/validate-release-contract.mjs` with explicit allowlists and
  archive/checksum inspection.
- [ ] Capture a frozen v1.8.1 asset-parser/update fixture plus positive and
  negative checksum/archive fixtures.
- [ ] Dry-run packaging on a non-production tag/workflow.
- [ ] Inventory branch protection, Actions permissions, environments,
  secrets/variables, GitHub App, webhooks, runners, open PRs/issues, Pages,
  packages, and external integrations.
- [ ] Nick renames the repository; update local origin.
- [ ] Verify old and new web, API, git, release, and issue/PR continuity.
- [ ] Re-run all validation after the rename and before merging the breaking
  implementation PR.

**Owned files**

- `Cargo.toml`, lock/package manifests as assigned from Phase 7
- `.github/release-please/**`, workflows, labels, issue forms
- release/preflight/build scripts, validators, release fixtures
- canonical current repository URLs

**Compatibility**

- Legacy public asset aliases remain through every 2.0.x release because an
  old 1.x client queries `releases/latest`; do not publish 2.1.0 until the
  1.x-to-2.0 bridge has been exercised on every supported platform.
- 2.1.0 is the intentional public artifact-name cutover. Legacy runtime
  readers/CLI/env/cookie/root compatibility may remain, but release assets are
  canonical-only from 2.1 onward.
- GitHub redirect is useful but never substitutes for runtime update evidence.
- Historical changelog URLs remain as generated history.

**Verification**

- Release contract proves eight unique assets plus checksums, correct archive
  layouts, exact hashes, canonical preference, and frozen old-parser success.
- Test new updater against both old-name and new-name `current_exe()`.
- Verify old/new GitHub web/API/git URLs and release permissions.
- Cross-platform build/preflight on all targets.

**Pass gate**

Dry-run artifacts and repository rename checks pass; release-please is ready to
produce exactly 2.0.0; old 1.x selection/extraction contract consumes the bridge
assets.

**Stop/rollback**

Stop on unavailable repository/package name, runner loss, GitHub App failure,
redirect failure, missing secret, wrong version, archive mismatch, or any old
parser failure. Repository rollback uses GitHub rename reversal only before
public release and only with Nick’s authorization.

### Phase 12 — Cross-platform, security, migration, and regression qualification

**Objective**

Qualify the entire rebrand as one release candidate across data, auth, UI,
runtime, packaging, and platforms.

**Prerequisites**

- Phases 0–11 passed.
- Release candidate worktree contains only intended changes.

**Task matrix**

- [ ] Fresh install on macOS, Linux, Windows.
- [ ] Old-only root on each platform.
- [ ] Windows pre-`%APPDATA%` legacy root.
- [ ] Both roots identical, conflicting, partial, empty, permission-denied.
- [ ] Interrupted migration after every journal checkpoint and resume.
- [ ] Encryption known-answer, raw file key, old env, new env, equal aliases,
  conflicting aliases, corrupt ciphertext, restart token reload.
- [ ] SQLite active-WAL backup, integrity, restart, rollback, and old-process
  refusal.
- [ ] Custom config and external/custom models unchanged byte-for-byte.
- [ ] Default legacy model keep/move, all canonical subtrees, root GGUF,
  incomplete pairs, HF cache, sidecars, provenance, cross-volume resume.
- [ ] Auth cookie old/new/both/conflict/logout/TLS.
- [ ] API auth-routing and malformed JSON.
- [ ] Remote agent old/new controller/agent/task/path/cert/update matrices.
- [ ] Updater old/new assets/checksums/current-exe/archive matrices.
- [ ] Browser preference preservation, current UI copy, accessibility/themes.
- [ ] Repository/release contract and clean version identity.
- [ ] Security review: secrets, randomness, permissions, traversal/symlinks,
  reparse points, XSS, rate limits, timeouts, protocol defaults.

Windows-machine follow-up markers (intentionally open until native validation):

- [ ] Build and execute the Windows release binary on the Windows development
  machine; record raw stdout/stderr and exit codes.
- [ ] Exercise fresh, legacy-only, both-root/conflict, queued migration, and
  `--clear-auth-config` using disposable `%APPDATA%` roots; verify no unrelated
  token/key creation.
- [ ] Verify `%APPDATA%` logs/certs/bin paths, scheduled-task arguments, legacy
  task cleanup, remote-agent install/update/uninstall, and tray icon/popover.
- [ ] Run Windows ZIP/installer/package checks and compare canonical/legacy
  filenames, checksums, icons, and upgrade behavior.
- [ ] Capture native Windows screenshots and attach the machine manifest to the
  Phase 12 evidence receipt.

**Mandatory checks in exact project order**

1. `rtk cargo clippy -- -D warnings`
2. `rtk cargo test`
3. `rtk npm run validate-js`
4. `rtk npm run lint`
5. `rtk git diff --check`
6. `rtk cargo build --release`
7. `rtk cargo fmt`
8. Commit any formatter/generated changes.
9. `rtk git status` and require a clean intended worktree.

Additional gates:

- `rtk cargo test --test auth_routing`
- `rtk rustup target add x86_64-pc-windows-gnu`
- `rtk cargo check --target x86_64-pc-windows-gnu`
- `rtk bash scripts/release-preflight.sh`
- all four release target builds
- isolated release-built Playwright with at least 600 seconds available,
  working directory `tests/ui`:
  `CI=1 LLAMA_MONITOR_USE_RELEASE=1 LLAMA_MONITOR_TEST_PORT=17778 rtk npm test`
  until the harness variables are canonically renamed with old aliases.

**Evidence**

- Store raw stdout/stderr, exit codes, target/platform, manifests, fixture
  hashes, and receipts under Phase 12 evidence.
- A filtered `PASS (0) FAIL (0)` is failure, not proof.

**Pass gate**

Every matrix row is green on its required native platform and the mandatory
sequence passes from a clean intended worktree.

**Stop/rollback**

Any data, auth, update, native platform, or release-contract failure blocks
cutover. Roll back to the last phase boundary; do not waive failures.

### Phase 13 — Screenshot refresh and visual acceptance

**Objective**

Prove the Token Ingot identity in the release-built product at realistic sizes
and refresh only documentation-used screenshots.

**Prerequisites**

- Phase 12 passed.
- Final brand assets and frontend code are unchanged during capture.

**Tasks**

- [ ] Add/register a `brand-identity` capture scenario under `core` or a
  documented `brand` group.
- [ ] Capture welcome, auth, navigation, main shell, compact popover,
  migration pending/preview/conflict/success, settings, models, and update
  surfaces.
- [ ] Capture dark/light, 1440×900 and 1280×900 desktop, 430×900 narrow, and
  reduced motion where relevant.
- [ ] Produce deterministic favicon/PWA/small-size proof sheets.
- [ ] Capture native tray/app icon proof on macOS, Windows, and Linux outside
  the browser harness.
- [ ] Run scenarios sequentially with manifest-backed declared outputs.
- [ ] Review spacing, clear space, crop, contrast, theme, focus, legibility,
  maskable safe zone, and no visible old product name.
- [ ] Promote only images actually referenced by README/current reference docs.
- [ ] Run unused-screenshot checker.

**Owned files**

- capture scenarios/index/docs, current promoted screenshots, documentation
  references, Phase 13 evidence

**Verification**

- Release build first.
- Strict capture manifest check.
- Exact file/receipt manifest and realistic viewport proof.
- Human acceptance by Nick remains mandatory for final brand presentation.

**Pass gate**

Fresh receipts and native proofs demonstrate the approved identity across all
required surfaces; every promoted screenshot is referenced.

**Stop/rollback**

Stop on stale assets, wrong build, missing manifest, unexpected output, old
brand, poor small-size rendering, light-theme mismatch, or native icon failure.
Fix and recapture; never edit screenshots.

### Phase 14 — 2.0 cutover, bridge-release proof, and release handoff

**Objective**

Publish 2.0 safely, prove real 1.x upgrade and migration behavior, and prepare
the update handoff for the project owner’s friends/testers. There is no formal
public announcement, launch campaign, or announcement-approval gate.

**Prerequisites and serialization**

- Phases 0–13 passed.
- Repository renamed and verified.
- Merge freeze active.

**Cutover sequence**

1. Record the final commit, clean worktree, version, identity contract,
   migration schema, release contract, screenshots, and rollback owner.
2. Merge the implementation PR with a breaking conventional squash title/body.
3. Inspect the release-please PR; stop unless it is exactly 2.0.0 with the
   expected changelog.
4. Merge the release PR.
5. Wait for all target builds, eight assets, checksums, and uploads.
6. Validate every downloaded asset/hash/archive, canonical metadata, and
   `--version`.
7. Run real shipped v1.8.1→2.0 self-update probes on Linux x86_64, Linux
   aarch64, macOS aarch64, and Windows x86_64 in disposable roots.
8. Each probe preserves sentinel config, token, preset, model reference, and
   migration state; proves restarted 2.0 health and receipt.
9. Run fresh 2.0 install and old-root migration probes on native platforms.
10. Verify old/new repository, release, API, and git URLs once more.
11. Keep the friend/tester update handoff pending until all probes pass. The
    workflow’s publish-before-assets window is not acceptable proof of readiness.
12. Share the 2.0 update with the intended friends/testers, publish the upgrade
    guide, and update any external integrations that are actually in use.
13. Monitor issues/update failures/migration failures and retain rollback
    authority through the acceptance window.

**Evidence**

- `release-contract.json`, downloaded asset manifest, hashes, archive listings,
  v1.8.1 probe logs, fresh/migration receipts, URLs, workflow runs, final
  screenshot manifest, and release-handoff record.

**Pass gate**

2.0 is available with complete dual assets, real old-client update and old-root
migration proof, correct repository identity, and no unresolved release-blocking
defect; the owner can safely share the update with the intended testers.

**Stop/rollback**

- Before the friend/tester handoff: mark release prerelease/draft if safely
  supported, unpublish only with Nick authorization, fix forward, and preserve
  artifacts.
- After the handoff: prefer a fast 2.0.1 fix with both asset families.
- Repository reversal, tag deletion, or release deletion are destructive and
  require Nick’s explicit direction.
- Data migration rollback follows receipts; never instruct users to copy live
  databases or delete roots manually.

### Phase 15 — Compatibility observation and future 3.0 cleanup

**Objective**

Keep 2.x compatibility measurable and define, but do not prematurely execute,
legacy removal.

**Prerequisites**

- Phase 14 completed and 2.0 is released.

**Tasks**

- [ ] Keep old binary/env/cookie/root/task/path/provenance readers through all
  2.x; keep release filename aliases through 2.0.x, then remove only those
  aliases at the planned 2.1.0 artifact cutover.
- [ ] Track support reports and explicit update/migration failures without
  logging secrets or user data.
- [ ] Document deprecation warnings and remediation.
- [ ] Before 3.0, inventory remaining legitimate 1.x use and publish a separate
  breaking cleanup plan.
- [ ] 3.0 cleanup may remove code aliases; it must not delete legacy user data.

**Owned files/APIs**

- 2.x compatibility constants, warnings, upgrade documentation, release
  contract, and support evidence only.

**Compatibility and verification**

- Run the literal-policy and release-contract validators on every 2.x release.
- Prove legacy latest-release assets through 2.0.x, then prove canonical-only
  2.1.0 discovery plus old binary/env/cookie/root/task/path readers and current
  migration receipts.
- Capture deprecation/support evidence without secrets or user content.

**Pass gate**

No 2.0.x release drops the bridge contract; 2.1.0 is the planned release-name
cutover. A separately approved 3.0 plan owns runtime compatibility removals.

**Stop/rollback**

Any accidental compatibility removal blocks that 2.x release and is restored
before publication. No user-data cleanup is authorized by this phase.

## 9. Phase ownership and serialization map

| Phase | Primary owner | Exclusive files/surfaces | Must wait for |
|---|---|---|---|
| 0 | Inventory | Evidence only | — |
| 1 | Brand/static assets | Brand source, static generator, icon assets | 0 |
| 2 | Identity/path refactor | `identity.rs`, `paths.rs`, config resolution | 0–1 |
| 3 | Migration specification | Schemas, pure planner, fixtures | 2 |
| 4 | Migration core | Startup/bootstrap, mutation/journal/API/CLI | 3 |
| 5 | Resource consumers | Path consumers and fresh default | 4 |
| 6 | Model relocation | Model library, downloads, path rewrites | 5 |
| 7 | Package/CLI | Cargo/package/binaries/CLI | 4–6 |
| 8 | Frontend/auth | HTML/CSS/JS, cookie, migration UX | 1, 4–7 |
| 9 | Runtime/platform | Agent, updater, LHM, certs, tray | 4–8 |
| 10 | Documentation | Current docs/templates | 4–9 |
| 11 | Release/repository | Workflows, artifacts, repo rename | 0–10 |
| 12 | Qualification | Tests/evidence only, fixes return to owner | 0–11 |
| 13 | Visual QA | Capture assets/docs screenshots | 1, 8, 12 |
| 14 | Cutover | PR/release/repository operations | all prior |
| 15 | 2.x maintenance | Compatibility observation | 14 |

Rules:

- Migration core, path resolution, model relocation, package entrypoints,
  updater/release parsing, repository rename, and cutover remain serialized.
- Frontend copy work may be divided by module only after the identity contract
  and production assets are frozen.
- A verifier may add tests/evidence but must return behavioral fixes to the
  owning phase rather than creating a second implementation.
- Each phase receives one focused verification review before it closes.

## 10. Source-backed surface appendix

This appendix is a minimum inventory, not a substitute for Phase 0.

### 10.1 Frontend and native identity

| Surface | Current source |
|---|---|
| Browser title/favicon/manifest link | `static/index.html:5-13` |
| Compact title | `static/compact.html:5` |
| Manifest name/short name/icon | `static/manifest.json:2-9` |
| Auth brand/title | `static/index.html:49-52` |
| Navigation inline logo/title | `static/index.html:152-158` |
| Welcome inline logo/tagline | `static/index.html:308-321` |
| Restart dialog | `static/index.html:2016-2019` |
| Managed binary/path/help/recovery copy | `static/index.html:2110-2113,2308,5730,6149,6293` |
| Compact heading | `static/compact.html:161-164` |
| Existing emoji icon | `static/icon.svg` |
| Programmatic tray glyph | `src/tray.rs:create_tray_icon` |
| Tray menu/tooltip | `src/tray.rs:291-313` |
| Dynamic auth/recovery copy | `static/js/features/auth.js` |
| Update/restart copy | `static/js/features/updates.js`, `settings.js` |
| Remote-agent paths/copy | `static/js/features/remote-agent.js` |
| Terminal/system copy | `static/js/features/dashboard-ws.js` |

Preserve as technology rather than brand:

- `llama.cpp`, `llama-server`, GGUF/backend controls and modules.
- `LHM`, LibreHardwareMonitor, `/api/lhm/*`, and LHM selectors.
- Historical source/benchmark/release identities.

### 10.2 Browser storage compatibility identifiers

Freeze these production keys throughout 2.x:

- `llama-monitor-preferences`
- `llama-monitor-last-endpoint`
- `llama-monitor-last-session`
- `llama-monitor-previous-position`
- `llama-monitor-group-by-family`
- `llama-monitor-preset-sort`
- `llama-monitor-chat-font`
- `llama-monitor-enter-to-send`
- `llama-monitor-chat-telemetry-pinned`
- `llama-monitor-chat-style`
- `llama-monitor-models-prefs`
- `llama-monitor-chat-focus-mode`
- `llama-monitor-notifications`
- `llama-monitor-gpu-viz`
- `llama-monitor-system-viz`
- `llama-monitor-date-format`
- `llama-monitor-log-font-size`
- `llama-monitor-log-tail-enabled`
- `llama-monitor-log-tail-lines`
- `llama_monitor_sidebar_width`
- `llama_monitor_sidebar_expanded`
- `llama_monitor_context_notes_intro_hidden`

Do not mechanically brand unrelated generic keys such as
`wizard_view_mode`, `update-dismissed`, `sidebarCollapsed`, or
`appNavWidth`. Treat `llama_monitor_settings` as a likely stale test
fixture until Phase 0 proves a production consumer.

### 10.3 Static/PWA constraints

- `static/sw.js` is fetch passthrough and no registration was found. The
  rebrand must not silently become an offline-PWA project.
- `build.rs` currently generates `include_str!` constants for all static
  assets while listing PNG/ICO MIME types. Binary asset support must be fixed
  and tested before adding those files under `static/`.
- Use separate regular and maskable PWA icons; “any maskable” on one full-bleed
  asset is not sufficient.
- A browser mockup is not native tray/app-icon proof.

### 10.4 Application resource inventory

At minimum classify and test:

- `api-token`, `db-admin-token`, `encryption-key`
- `auth-config.json`, `tls-config.json`
- `presets.json`, `sessions.json`, `templates.json`,
  `ui-settings.json`, `gpu-env.json`, `model-tags.json`
- community source/catalog and HF token state
- `chat.db`, `chat.db-wal`, `chat.db-shm`, `backups/`
- `ssh-known-hosts.json`, agent token/config files
- `certs/` including CA, client/server keys, trusted remote CAs
- `bin/`, `binaries/`, `scripts/`, `logs/`
- spec-decode reports/pins, Rapid-MLX templates/overlays/sidecar inventory
- chat-template releases/files/history
- GGUF metadata cache
- model library, HF app cache, staging, partial/resume files, provenance,
  journals, manifests, completion markers
- unknown root entries, which are user-owned until proven otherwise.

Known independent path consumers to eliminate or classify include
`src/main.rs`, `src/certs.rs`, `src/tray.rs`, `src/agent.rs`,
`src/hf/mod.rs`, `src/llama/spawn_wizard.rs`,
`src/web/api/spawn_wizard.rs`, and Rapid-MLX store/resolver modules.

### 10.5 Provenance and cryptographic compatibility identifiers

Do not blind-replace:

- HKDF info `llama-monitor-encryption-key`
- existing `enc:` ciphertext envelope
- `.llama-monitor-provenance.json`
- `.llama-monitor-source.json`
- `llama-monitor-conversion.json`
- existing CA subject/trust identity

Add dual readers before introducing any new provenance filename. Existing
metadata must remain discoverable indefinitely or until an explicit
schema-versioned migration proves conversion.

### 10.6 Current release and operational identities

Legacy public assets:

- `llama-monitor-linux-x86_64`
- `llama-monitor-linux-aarch64`
- `llama-monitor-windows-x86_64.zip`
- `llama-monitor-macos-aarch64.tar.gz`

Current managed Windows task families:

- `LlamaMonitorAgent`
- `llama-monitor-agent`
- `LlamaMonitorSensorBridge`

No checked-in systemd unit, LaunchAgent plist, Homebrew formula, MSI, DMG, or
separate native installer was found. External distribution surfaces must be
discovered rather than fabricated.

## 11. Global evidence and command contract

### 11.1 Literal-policy validation

Add a repository validator that scans current source and fails every
unclassified old identity. Allowlist entries require:

- exact file/glob and literal;
- classification: compatibility, crypto/provenance, historical,
  infrastructure, or backend technology;
- owner and earliest removal release;
- explanation.

The validator must catch filenames as well as file contents. It must not allow a
broad directory such as all of `src/` or `docs/`.

### 11.2 Release contract

`release-contract.json` must prove:

- package, binary, version, repository, manifest, and product identity agree;
- eight platform assets plus `checksums.json` exist exactly once;
- every checksum matches;
- legacy and canonical archive layouts match their consumers;
- canonical updater prefers canonical names;
- frozen v1.8.1 parser selects legacy names;
- old and new `current_exe()` update/relaunch;
- no unexpected executable payload exists.

### 11.3 Repository rename checks

After rename and before release:

- old web URL redirects to the canonical repository;
- old API endpoint resolves with canonical `full_name`;
- old and new git URLs support `ls-remote`;
- GitHub app, Actions, runners, branch protection, environments, issues/PRs,
  release permissions, and default branch remain intact.

### 11.4 Screenshot commands

Run after `rtk cargo build --release`; never parallelize scenarios:

- strict manifest validation;
- brand-identity, welcome, navbar, appearance-palette, compact, migration, and
  affected feature scenarios;
- group captures only sequentially on one `SCREENSHOT_PORT`;
- `rtk bash scripts/check-unused-screenshots.sh`.

Artifacts remain under `docs/screenshots/artifacts/<group>/`. Promote only
documentation-referenced images to flat `docs/screenshots/`.

### 11.5 Fail-closed gate matrix

| Gate | Required proof |
|---|---|
| Source inventory completeness | Classified match inventory, exact command receipts, zero unowned old literals |
| Naming/compatibility approval | Approved human contract plus machine-readable identity JSON |
| Migration unit tests | Pure resolver/planner/state/journal/path/hash tests |
| Migration integration tests | Real staged resources, secrets, DB, receipts, restart and rollback |
| Fresh install | Only canonical root/resources created |
| Old install | Legacy works before migration; verified Stage A cutover |
| Both roots/conflict | Identical/empty/partial/divergent cases fail or select exactly as contracted |
| Interrupted/resume | Kill injection after every journal checkpoint converges |
| Custom config/models | Explicit paths remain authoritative and byte-identical where external |
| Incomplete downloads/library | Part/resume/provenance/cache/sidecar/model inventory equivalence |
| Auth/API routing | No-auth/API/db-admin/form/basic/malformed/confirmation/cookie matrix |
| Security/path review | Secrets, randomness, permissions, XSS, rate limits, symlink/reparse/traversal |
| macOS | Native root, migration, app/tray assets, update, release archive |
| Linux | Native root, migration, tray/headless, update, both architectures |
| Windows | Native APPDATA/SYSTEM/tasks/reparse/tray/update/bundle plus cross-check |
| Rust formatting/lint/tests | Mandatory project sequence with raw nonzero test receipts |
| JavaScript syntax/lint | `npm run validate-js` and `npm run lint` |
| Whitespace/clean tree | `git diff --check`, formatter commit, clean intended status |
| Release build | Generated static files committed; canonical and alias binaries healthy |
| Release-built UI | Full isolated Playwright, nonzero tests, release binary |
| Screenshot harness | Sequential strict manifests and exact fresh files |
| Visual modes | Dark/light/narrow/reduced-motion plus native tray/app proof |
| Documentation | Links, commands, current-name scan, upgrade guide, historical allowlist |
| Screenshots in docs | Every promoted image referenced; unused checker clean |
| Release identity | 2.0.0, repo/package/binary/manifest/artifacts/checksums agree |
| Bridge release | Frozen parser plus real shipped v1.8.1→2.0 native probes |
| Final worktree | Intended commits only, receipts/manifests complete, no blocker |

## 12. Documentation update matrix

| Area | Required result |
|---|---|
| README | New product, Token Ingot, install/run/update, migration, repo URLs |
| Quick start | New binary/root plus legacy alias note |
| CLI flags | Canonical commands, maintenance commands, aliases/deprecation |
| Config/filesystem | Platform roots, root-state selection, every managed path |
| Security/TLS | Encryption identifier freeze, key/cert/CA migration, cookie behavior |
| Model library/HF | Deferred explicit relocation, external/custom policy, partials/cache |
| Spawn/setup wizard | New brand/path copy without renaming llama.cpp backend |
| Rapid-MLX/llama.cpp | Foundry as host product; technology names unchanged |
| Binary lifecycle | Canonical/legacy binaries, checksum fix, dual artifacts |
| Remote agent/SSH | New paths/tasks/processes, old detection, SYSTEM root behavior |
| Windows | New APPDATA root, scheduled tasks, self-update/archive layout |
| API/auth | Migration routes/auth/confirmation, stable existing routes |
| Dashboard/chat/UI patterns | New shell/mark/tokens/accessibility |
| Upgrade guide | All cases, preview, receipts, restart, rollback, cleanup |
| Contribution/issues | Canonical repository/product and version requests |
| Release notes | 2.0 breaking identity plus supported compatibility |
| Screenshots | Fresh release-built current imagery only |

## 13. Risk register

| Risk | Prevention | Detection | Recovery | Owner/evidence |
|---|---|---|---|---|
| New root created before classification | Pure resolver and startup split | No-side-effect tests, filesystem snapshots | Stay legacy; delete only receipt-scoped empty staging | Phases 2–4 |
| Data loss | Copy-first Stage A; no auto-delete | Source/destination manifests and hashes | Select intact legacy root | Phase 4 receipt |
| Duplicate huge models | Defer model move; explicit keep/move | Byte/free-space preview | Keep legacy path; cleanup only after receipt | Phase 6 |
| Partial migration | Journal every checkpoint | Restart injection | Resume matching journal or legacy rollback | Phase 4 |
| Both-root conflict | No silent merge | Root fingerprints/conflict state | Explicit root selection/export diagnostics | Phases 3–4 |
| Symlink/reparse escape | `symlink_metadata`, canonical containment | Unix/native Windows fixtures | Refuse without writes | Phases 3, 12 |
| Cross-volume exhaustion | Preflight bytes/free space, resumable staging | Forced-copy tests | Retain source, resume or abandon staging | Phases 4, 6 |
| Encryption break | Freeze HKDF info/envelope, copy key first | Known-answer/restart tests | Legacy root/key rollback | Phases 2, 4 |
| Existing encrypted token loaded as ciphertext | Initialize key before token load | Token restart test | Correct startup order; preserve raw file | Phase 2 |
| SQLite/WAL corruption | Online backup/restore APIs only | Integrity/WAL fixture | Restore verified backup | Phase 4 |
| Auth lockout | Dual cookie, preserve tokens/key | Auth matrix and migrated login | Legacy cookie/token/root rollback | Phases 4, 8 |
| TLS/agent trust break | Preserve CA/key identity | Mixed trust/enrollment tests | Re-enable old task/certs | Phase 9 |
| Custom paths moved | Explicit classification, no prefix guessing | Byte-identical external fixture | Keep explicit external path | Phases 3, 6 |
| Partials/provenance stranded | Pair/manifest inventory, dual readers | Inventory equivalence test | Legacy models root | Phase 6 |
| Remote task duplication | Install/health-check new then retire old | Native task enumeration | Disable new, re-enable verified old | Phase 9 |
| Process over-kill | Exact executable/task identity | Adversarial process fixture | Stop only owned process | Phase 9 |
| Old clients cannot update | Legacy assets through 2.0.x latest releases and a gated 2.1.0 cutover | Frozen parser and real v1.8.1 probes | Fix-forward release with both families before 2.1.0 | Phases 11, 14 |
| Checksum unavailable | Preserve checksum asset in release model | Focused parser/retrieval test | Block self-update/release | Phase 9 |
| Wrong major version | Breaking PR contract and release-PR gate | Inspect generated 2.0.0 PR | Do not merge; use supported override | Phases 11, 14 |
| Release visible before assets | Do not share the update until artifact probes | GitHub asset/checksum manifest | Mark prerelease/draft if authorized; fix forward | Phase 14 |
| Repository integration loss | Pre-rename external inventory/freeze | Post-rename live checks | Authorized rename reversal before release | Phase 11 |
| Runner label outage | Keep old labels until replacements exist | Queue/runs-on verification | Restore old labels | Phase 11 |
| Browser preference loss | Freeze storage keys in 2.x | Full preference round-trip | Revert UI write changes | Phase 8 |
| Visual/logo confusion | Manual SVG, similarity and small-size review | Proof sheet/native screenshots | Return to approved concept/fallback | Phases 1, 13 |
| Binary static embedding failure | `include_bytes!` path and tests | Release build/HTTP byte tests | Serve SVG only until fixed | Phase 1 |
| XSS/accessibility regression | Safe DOM APIs, ARIA/focus/motion rules | Lint/Playwright/manual audit | Revert owning UI change | Phase 8 |
| Historical evidence drift | Explicit historical classification | Diff/review/validator | Restore immutable history | Phase 10 |
| Platform divergence | Native fixtures plus Windows cross-check | Platform matrix | Block release; fix owning phase | Phase 12 |

## 14. Proposed commit sequence

Use phase-sized conventional commits; do not combine rollback boundaries:

1. `docs(docs): freeze Local LLM Foundry identity contract`
2. `feat(ui): add Token Ingot production brand assets`
3. `refactor(binary): centralize product identity and application paths`
4. `test(binary): add application-home migration fixtures`
5. `feat(binary): add restartable application-home migration`
6. `refactor(binary): route resources through canonical app paths`
7. `feat(models): add explicit legacy model-root relocation`
8. `feat(binary): launch canonical CLI with legacy compatibility`
9. `feat(ui): apply Local LLM Foundry identity and migration UX`
10. `fix(api): preserve auth and cookie compatibility during rebrand`
11. `feat(spawn): migrate managed agent and platform identities`
12. `fix(binary): add dual-name updater and checksum compatibility`
13. `build(ci): publish canonical and legacy 2.x artifacts`
14. `test(binary): freeze the 1.x to 2.0 bridge contract`
15. `docs(docs): document Local LLM Foundry 2.0 and upgrades`
16. `test(ui): add rebrand and migration visual coverage`
17. `chore(ci): prepare repository and release-please rename`

If a phase requires formatter/generated-file follow-up, commit it before the
phase gate closes. Do not create one giant “rebrand” commit.

## 15. PR and release sequence

- Implement on the existing feature branch with phase commits and receipts.
- Use one focused verifier after each phase; resolve failures before the next
  serialized phase.
- Before the final PR/update, prepare a complete PR body with:
  - identity and migration contract;
  - compatibility timeline;
  - security/data review;
  - tests/evidence;
  - screenshots;
  - release/repository steps;
  - `BEGIN_COMMIT_OVERRIDE` entries for all user-facing changes.
- The squash title must be a breaking conventional title, recommended:
  `feat!: launch Local LLM Foundry 2.0`.
- Include the required `BREAKING CHANGE:` paragraph.
- Nick performs the repository rename at the Phase 11 stop point.
- Merge implementation only after post-rename validation.
- Merge release-please only if the generated release is exactly 2.0.0.
- Do not share the update with the intended testers until Phase 14’s real
  artifact/update/migration probes pass.

## 16. Compatibility and deprecation timeline

| Release | Policy |
|---|---|
| 2.0.0 | New product/root/binary/repo canonical; legacy binary/env/cookie/root/tasks/paths/artifacts/provenance accepted |
| 2.0.x | Fix migration/update defects forward; keep both canonical and legacy release asset names for the 1.x bridge |
| 2.1.0 | Cut over release artifacts to canonical names; retain runtime compatibility readers unless a separate decision removes them |
| 2.1–2.x | Canonical release assets; runtime aliases/warnings remain actionable and secret-safe |
| Final 2.x | Publish explicit 3.0 removal notice only after evidence review |
| 3.0 earliest | Separately planned removal of code aliases; never automatic deletion of legacy user data |

## 17. Final 2.0 acceptance checklist

### Identity and visual system

- [ ] Product/short name/slug contract approved.
- [ ] Token Ingot SVG approved by Nick.
- [ ] Complete deterministic derivative matrix exists.
- [ ] Small-size, maskable, light/dark, and native tray proofs pass.
- [ ] Current visible UI contains no unapproved old product identity.
- [ ] Backend llama.cpp and LHM terminology remains correct.

### Paths and migration

- [ ] Pure resolution performs no writes.
- [ ] Fresh installs create only the new root.
- [ ] Old-only installs remain functional before migration.
- [ ] Both-root cases fail closed or select only by explicit policy.
- [ ] Critical state, secrets, certs, and DB migrate with verification.
- [ ] HKDF info/envelope/key continuity proven.
- [ ] Interrupted migration resumes from every checkpoint.
- [ ] Rollback selects intact legacy state without merge.
- [ ] Cleanup is explicit, receipt-scoped, and non-automatic.

### Models and resources

- [ ] Custom/external models remain byte-identical and external.
- [ ] Legacy default model keep/move choices work.
- [ ] All GGUF/MLX/Transformers/Rapid-MLX/HF/staging/partial/provenance resources are preserved.
- [ ] Path rewrites are schema-aware and exact.
- [ ] Inventory equivalence and model launches pass.

### Backend/API/auth

- [ ] Canonical binary/package works; legacy binary runs the same core.
- [ ] Internal `llama_monitor` crate and technology namespaces are documented.
- [ ] API routes and existing serialized fields remain compatible.
- [ ] Migration auth/confirmation/rate/concurrency/error contracts pass.
- [ ] Cookie dual-name behavior and token rotation pass.
- [ ] No secret logging, weak randomness, permission regression, or direct live SQLite copy.

### Runtime/platform

- [ ] Agent/controller old/new matrix passes.
- [ ] Old/new tasks and paths transition without duplicates.
- [ ] Existing CA trust survives.
- [ ] Updater checksum discovery is fixed and tested.
- [ ] Both current-exe names update/relaunch.
- [ ] Native macOS/Linux/Windows tray/app assets pass.

### Release/repository/docs

- [ ] Canonical repository rename and old redirects are live.
- [ ] Runners, GitHub App, secrets, branch protection, issues/PRs, and Actions survive.
- [ ] Release-please generates exactly 2.0.0.
- [ ] Eight assets plus checksums pass the release contract.
- [ ] Real shipped v1.8.1→2.0 updates pass on every supported platform.
- [ ] Fresh and old-root release probes pass.
- [ ] Current docs, upgrade guide, issue forms, and screenshots are complete.
- [ ] Historical records remain truthful.

### Final quality

- [ ] Mandatory local command sequence passes.
- [ ] Windows cross-check and native platform matrix pass.
- [ ] Full isolated release-built Playwright passes with nonzero tests.
- [ ] Fresh manifest-backed screenshots and native proofs are approved.
- [ ] Literal-policy, docs links, unused screenshots, release contract, and clean
  intended worktree gates pass.
- [ ] Rollback owner and procedure are recorded before release.
- [ ] No unresolved P0/P1 defect or unapproved decision remains.

## 18. Luna execution handoff

For each new Luna context:

1. Read `AGENTS.md`, this plan, the prior phase receipt, and the current
   source owned by the phase.
2. Confirm prerequisites and dirty-worktree boundaries.
3. Re-run the phase’s focused discovery; do not trust line numbers blindly.
4. Implement only the checked tasks owned by the phase, copying established
   project patterns cited here.
5. Run focused tests first, then the phase gate.
6. Capture raw receipts and exact produced-file manifests.
7. Ask one focused verifier to compare source, tests, docs, and receipts against
   the phase objective.
8. Fix all actionable findings within the owning phase.
9. Commit conventionally and close the phase only when its pass gate is true.
10. Hand the next context: commit, files changed, decisions, tests, receipts,
    remaining risks, and explicit next phase.

Never advance because the work is large or the context is ending. Advance only
on evidence.
