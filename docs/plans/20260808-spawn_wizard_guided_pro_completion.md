# Spawn Wizard Guided + Pro Completion Plan

| Field | Value |
|---|---|
| Created | 2026-08-08 |
| Status | Phase 9 Rapid-MLX Pro parity complete; awaiting human acceptance stop |
| Scope | Complete the three-step Spawn Wizard for llama.cpp/GGUF and Rapid-MLX with Guided and Pro views, trustworthy capture evidence, introspection-backed recommendations, and documentation closure |
| Primary design source | [`../archive/rapid-mlx/20260806-spawn_wizard_uiux_redesign.md`](../archive/rapid-mlx/20260806-spawn_wizard_uiux_redesign.md) |
| Phase-router context | [`20260718-final_rapidmlx_followups_execution.md`](20260718-final_rapidmlx_followups_execution.md), Phase 10 |
| Intended executor | Coordinator -> bounded Builder -> fresh Verifier -> focused remediation |

### Execution progress ledger

Updated as milestones pass; this ledger records verified repository state, not intended work.

| Milestone | Status | Evidence |
|---|---|---|
| Phase 0 evidence freeze and contract inventory | Complete | Commit `39634a71`; release identity, route/auth inventory, control contract, fixture freeze, and raw G0 receipts recorded under `docs/plans/evidence/spawn-wizard-guided-pro/`. |
| Phase 1 capture contract and baseline ownership | Complete | Commits `eeb8bf8`, `1147adc`, `35b04da`; strict receipts, realistic viewport enforcement, capture tests, llama/Pro/Rapid baselines, all six registered wizard scenario contracts, GIF receipts, and runtime/dashboard group migration are implemented. Fresh outside-sandbox runs verified llama baseline, HF download, tier matrix, Pro baseline, Rapid baseline, and both wizard GIF groups. |
| Phase 2 CSS split | Complete | Commit `1a91f91`; `spawn-wizard-base.css` + `spawn-wizard-theme.css` are linked in original order, concatenation is byte-identical to the pre-split stylesheet, generated routes/assets build, and release-built llama/Pro baseline captures pass. |
| Phase 3 introspection/effective/provenance truth | In progress | Phase 3A inventory recorded at `docs/plans/evidence/spawn-wizard-guided-pro/phase-3-inference-inventory.md`; filename-based family/MTP/parameter-count/architecture/sampling fallbacks are removed from the active wizard path, local and streamed-HF introspection now merge native context, hybrid/sliding-attention, multimodal, and metadata status/provenance fields, Rapid profile success/failure updates shared metadata status, degraded GGUF/HF/MLX estimator paths are size-only rather than name-derived, and the Guided option-copy path uses DOM cloning instead of `innerHTML`. Resolved/degraded evidence is now rendered in the selected-model architecture card. `cargo clippy -- -D warnings`, `cargo test` (2272 passed, 26 ignored), JS validation/lint, strict capture-manifest/receipt tests, and release builds pass. Fresh release-built tier, mmproj selector, Rapid baseline, and llama baseline captures were inspected at original resolution; `spawn-wizard-mmproj-selection--receipt.json` / `spawn-wizard-mmproj-selection--llamacpp-local--vision.png` verify the hardware vision selector and typed family-backed F16 recommendation. Remaining work is MLX source/config fixture coverage plus retiring the isolated legacy `model_defaults.rs` compatibility branches. |
| Phase 4 shared view descriptor relocation | In progress | The Guided drawer now owns one canonical llama.cpp or Rapid-MLX advanced wrapper at a time, derives its total from `controlsForView()`, reports a live changed count, preserves labels/listeners/payload controls by moving wrappers rather than cloning inputs, and hides the inactive backend. Rapid Guided layout keeps the backend-native panel and fit rail visible. Pro remains non-destructive and explicitly unavailable (`Pro (coming in a later phase)`). Fresh release-built G4 receipt `docs/screenshots/artifacts/wizard-llamacpp/spawn-wizard-guided-drawer--receipt.json` and five original-resolution shots verify llama closed/open, Rapid closed/open, engine switch-back, changed count, and Pro availability. JS validation/lint, diff check, release build, strict capture-manifest, and the G4 scenario pass; full Phase 4 descriptor/state/projection contracts remain. |

> [!IMPORTANT]
> **Coordinator start here:** jump to the [Terra Coordinator handoff prompt](#terra-coordinator-handoff), copy
> that bootstrap prompt into a fresh Coordinator context, and substitute `0` for the phase number.
> The Coordinator reads this plan completely once; Builders and Verifiers read only the routed
> sections for their assigned packet. Always begin with Phase 0, and do not advance without its gate.

### Responsive modal sizing milestone (2026-08-09)

Added a shared `modal-sizing.css` contract loaded after the modal surfaces. It replaces inline pixel widths and independent viewport caps with safe-area gutters, dynamic viewport dimensions, responsive shell widths, and a tall-screen height expansion for content-rich modals. Compact dialogs retain intrinsic sizing while all overlays can scroll on short viewports. Release-built evidence: `spawn-wizard-tier-matrix--llamacpp-local--guided.png` (1280x1400), `spawn-wizard-rapid-guided-baseline--rapidmlx-local--dark.png`, and settings/preset captures show centered shells without the prior tall-viewport backdrop bands. Validation: release build, clippy, 2272 Rust tests (26 ignored), JS validation/lint, diff check, and cargo fmt.

### Guided MTP capability preflight (2026-08-09)

The Guided speed decision now waits for resolved model metadata before enabling built-in MTP. Resolved models with `mtp_depth` advertise the detected head count; resolved models without heads, degraded metadata, and unknown metadata remain unavailable instead of claiming a platform-only default. The stale UI test was updated with enabled and unavailable assertions; focused MTP Playwright coverage passes against the release build. This is a Phase 5 hard-gate repair recorded before the next planned Phase 7 Pro work.

## 1. Why this plan exists

### Phase 4 milestone update (2026-08-09)

The shared Guided drawer now uses a canonical `SettingStateRegistry` for resolved defaults, dirty/provenance, effective/pending state, reset, snapshots, and async resolved-default updates. A single overlay-level input/change pipeline syncs canonical controls and refreshes VRAM, context, guardrails, and drawer counts. Guided now owns the disclosure axis: the legacy Quick/Balanced/Advanced cards and persistence path are retired, while the backward-compatible payload profile remains fixed to `balanced`; five workload intents map to typed backend scenarios, including tool/research and deterministic batch/eval. G4 asserts the state shape and reset semantics in addition to descriptor validity, backend relocation, duplicate-ID protection, payload/preset parity, workload mapping, duplicate IDs, and honest Pro unavailability. Fresh release-built G4 evidence is recorded in `docs/screenshots/artifacts/wizard-llamacpp/spawn-wizard-guided-drawer--receipt.json` with five original-resolution screenshots, and the tier capture now proves the legacy axis is absent. Validation passed: release build, clippy, 2272 Rust tests (26 ignored), JS validation/lint, descriptor validation, diff check, and the strict G4/tier captures. Phase 4 implementation gates are complete; retain the ledger as in progress until the coordinator records the final commit boundary.

The 2026-08-06 redesign describes the right destination: one three-step wizard with a default
Guided view and a power-user Pro view over the same controls and state. The current repository does
not implement that destination, despite several documents and commit-status notes saying that it
does.

This plan is a source-and-runtime reconciliation, not a continuation of those status claims. It
was prepared from the current worktree, fresh release-build captures, the archived redesign in
full, the Phase 10 route map, current frontend/backend contracts, current tests, and current
documentation. Memory, commit messages, stale artifact files, and plan-ledger labels were used only
as discovery pointers. They are not accepted as proof.

The concrete current-state verdict is:

- The wizard really is three steps: Model, Hardware & memory, Start server.
- Guided contains four llama.cpp-looking proxy cards, but their wiring is incomplete and their
  "All settings" drawer is empty.
- Rapid-MLX does not have a backend-correct Guided projection. Fresh captures show the main column
  almost entirely blank while the memory rail renders.
- The Guided/Pro selector persists a value, but selecting Pro only logs
  `Pro mode selected (not yet implemented)`.
- Pro CSS exists without a renderer, navigation, search, modified-state model, or tests.
- The screenshot harness contains a Pro TODO, generates no Pro screenshots, leaves stale Pro files
  in the artifact directory, sometimes skips a requested shot without failing the scenario, and
  stores Rapid wizard stills under the llama.cpp group.
- Current automated tests protect much of the payload/schema/runtime behavior but do not test the
  Guided/Pro presentation contract or view switching.
- Spawn Wizard reference documentation repeats completion claims and older tier/step semantics that
  current source and pixels disprove.

No old Phase 10 sub-packet is considered complete for the purposes of this plan unless its hard gate
is re-run against the current tree.

## 2. Authority and supersession

For Spawn Wizard information architecture, this plan records the current authority order:

1. Current user instructions and `AGENTS.md`.
2. This plan's verified three-step Guided/Pro contract.
3. The archived redesign's full Guided and Pro behavior, wireframes, and control inventory.
4. Current backend schemas, capability/effective-value APIs, and runtime behavior.
5. The comprehensive Rapid-MLX plan's backend-neutral setting and seven-category principles.
6. Older six-step or "Phase 10 complete" statements only as historical context.

This resolves an inherited documentation conflict. The comprehensive plan's older D8/A50 six-step
direction is superseded for this slice by the later approved and currently shipped three-step shell.
The target is not to restore six steps. It is to make the current three steps coherent:

1. **Model** — workload intent, guidance level, engine, source, artifact, and model qualification.
2. **Tune** — Guided or Pro presentation of all launch-affecting settings and live memory/effective
   feedback.
3. **Launch** — canonical requested/effective review, optional preset save, exact backend launch
   evidence where available, and start/status behavior.

## 3. Frozen product decisions

These are implementation inputs, not questions for a Builder to reopen.

### 3.1 One state model and one live control per setting

Guided and Pro are presentations over the same `wizardState`, canonical DOM controls, typed payload
builders, and backend validation. A setting may be moved between containers, but there must not be
two independently live inputs synchronized by ad-hoc event handlers.

The existing Guided proxy pattern is therefore transitional and must be removed. The current custom
context proxy already demonstrates why: its change handler can update a tile without updating the
canonical context input.

### 3.2 Retire the legacy expertise profile; Guided/Pro owns disclosure

The later archived redesign supersedes the older Quick/Balanced/Advanced layout axis: "profile
becomes a view mode." Do not leave two competing expertise controls on Model. Migrate the useful
behavior rather than the labels:

- Quick's safe-default/auto-tune behavior becomes Guided defaults plus the explicit Auto-size action.
- Balanced's ability to inspect more settings becomes Guided's reachable All settings drawer.
- Advanced's full visibility becomes Pro.
- Remove or migrate the stored legacy profile preference deliberately; do not let it continue to
  disable or overwrite controls after Guided/Pro owns disclosure.

Guided/Pro is the presentation preference:

- `Guided` is the default.
- `Pro` is stored in `sessionStorage`, not in a model or preset.
- Switching views preserves engine, model, workload, every canonical setting, validation state,
  current Tune section, and effective-value evidence.
- View mode never changes payload semantics by itself.

Workload intent remains a separate transparent recommendation input. Converge the current three
use-case cards to the accepted five-workload taxonomy: Interactive coding agent (default),
Tool/research agent, Roleplay/storytelling, General chat, and Deterministic batch/eval API
(advanced). Show the assumptions each workload applies and preserve explicit user overrides. Do not
use `Guided` as both a view name and a second guidance-level label.

### 3.3 Seven stable Pro categories

Pro uses the accepted cross-backend category order already specified for the Preset Editor:

1. Model & compatibility
2. Memory & context
3. Performance
4. Generation & reasoning
5. Tools & conversation formatting
6. Network & observability
7. Advanced

Backend-specific subsections may be hidden when truly empty. The rail order must not reshuffle when
the engine changes. `Memory & context` is the Tune landing pane for both engines.

### 3.4 Guided is backend-adaptive, not visually forced parity

For llama.cpp, the four primary decisions remain those in the archived redesign:

1. Context size
2. K/V cache precision
3. Vision/mmproj
4. Speculative decoding/speed boost

For Rapid-MLX, Guided presents the decisions that are actually free variables in the qualified
runtime:

1. Context planning target (estimator/recommendation input, not a Rapid launch argument unless a
   future qualified runtime contract adds one)
2. Active/retained memory policy, with active KV shown as requested versus effective when pinned
3. Request capacity/scheduler policy
4. Generation/reasoning and eligible acceleration/companion status

Vision is shown as a truthful capability state for Rapid, not as an mmproj picker. While Rapid vision
remains unqualified, it is unavailable with an explanation and no live-looking control. Suspended or
unqualified MTP behavior is warning-first and cannot be described as free speed.

### 3.5 Every setting remains reachable

Guided shows primary decisions inline and every other applicable control in one in-place
`All settings (N)` drawer. The count and `N changed` state are registry-derived and backend-specific.
Pro shows every applicable control through the rail and panes. Backend-only controls are labeled;
they are not silently dropped and are not mirrored with invented equivalents.

### 3.6 Backend truth stays server-owned

The frontend may own layout, category placement, search copy, focus behavior, and teaching text. It
must not invent:

- runtime support;
- stable defaults or omission semantics;
- requested-versus-effective values;
- memory formulas;
- launch argv;
- validation and mutual exclusions;
- model family, architecture, MoE, MTP, or vision capability.

Use the existing typed payloads, `/api/vram-estimate`, Rapid model profiles, and Rapid command
preview. If llama.cpp exact argv is later required, add a shared server-backed preview seam; do not
assemble an "exact" argv in JavaScript.

The frontend presentation registry may reference stable backend semantic IDs, capability/evidence
states, and resolved-default/effective providers. It may not independently declare semantic IDs,
availability, risk, defaults, omission rules, validation, mutual exclusions, or effective runtime
behavior. Phase 4 must either consume the backend-owned setting catalog established by D6 or add a
contract comparison that fails when the temporary JavaScript presentation metadata disagrees with
the typed Rust authority; it must not create a second semantic catalog.

## 4. Current evidence baseline

| Claim | Current evidence | Classification |
|---|---|---|
| Three-step shell exists | `static/index.html` step badges/containers and `spawn-wizard.js::showStep` | Implemented; retain |
| Guided context/KV/vision/speed cards exist | `static/index.html` and `spawn-wizard-guided.js` | Partial llama.cpp facade |
| Guided uses canonical controls | Cards proxy hidden inputs through bespoke listeners | False; must replace |
| Guided All settings is complete | `#hw-all-settings-panel` is empty; a second legacy drawer also exists | False |
| Guided works for Rapid | Fresh `rapidmlx-local--spawn-wizard-rapid-mlx-*` captures show blank main content | False |
| Pro toggle exists | `#view-mode-select` + `sessionStorage` write | Scaffolding only |
| Pro works | `_initViewMode()` contains an explicit TODO/log; capture scenario contains a TODO and no Pro shot | False |
| Pro CSS proves layout | Orphan `.pro-*` rules have no renderer/consumer | False |
| Capture artifacts prove current behavior | Three stale Pro PNGs have no current capture call site; skipped shots do not fail | False |
| Payload/runtime contracts exist | `buildSpawnPayload`, `buildRapidMlxConfig`, Rust launch validation, preset hydration | Implemented; protect |
| Preset Editor is independent of wizard | It duplicates backend UI but shares stored schema/launch contracts | Regression surface; protect |
| Model-derived recommendations are introspection-only | Current source still contains name-based family/parameter/MTP/speculation fallbacks | False |

## 5. Execution protocol

Each phase is a separate Builder context followed by a fresh Verifier context. The Coordinator owns
scope, decisions, status, commits, and user-facing screenshot review.

Every Builder return must include:

- exact files changed;
- behavior implemented;
- tests and capture scenarios run;
- artifact filenames and dimensions;
- known gaps and unchanged backend-only behavior;
- `rtk git diff --check` result;
- confirmation that unrelated worktree changes were preserved.

Every Verifier must inspect the diff, current source, behavior, and fresh pixels. A commit hash,
passing build, DOM existence, or previous phase ledger is never sufficient sign-off.

For every phase that changes `static/`:

1. Run `rtk cargo build --release` before captures.
2. Run only one capture scenario/group at a time, on one explicit `SCREENSHOT_PORT`.
3. Capture ordinary viewport-sized UI; never flatten a fixed modal into a 5000px image.
4. Inspect every named gate image visually.
5. Run `rtk npm run validate-js`, `rtk npm run lint`, and the focused Playwright/Node tests.
6. Update `src/gen/static_assets.rs` and `src/gen/routes.rs` through the normal build when new static
   assets are added.

Run `rtk cargo build --release` before **every** screenshot gate, including evidence/harness phases
that did not edit `static/`, so captures always exercise the current embedded assets.

## 6. Phase map

| Phase | Bounded output | Context target | Gate |
|---:|---|---:|---|
| 0 | Evidence freeze, authority repair, control/route/capture inventories | 30k | G0 |
| 1 | Trustworthy wizard capture harness and baseline receipts | 55k | G1 |
| 2 | Split `spawn-wizard.css` with zero visual change | 55k | G2 |
| 3 | Introspection/effective/provenance truth prerequisites | 75k | G3 |
| 4 | Shared view descriptor, relocation, defaults, and state engine | 75k | G4 |
| 5 | Complete llama.cpp Guided | 65k | G5 |
| 6 | Complete Rapid-MLX Guided | 65k | G6 |
| 7 | Pro shell: rail, panes, switching, search, modified/reset | 80k | G7 |
| 8 | Complete llama.cpp Pro | 60k | G8 |
| 9 | Complete Rapid-MLX Pro and capability parity audit | 65k | G9 |
| 10 | Launch/review/preset/API contract convergence | 70k | G10 |
| 11 | Responsive, accessibility, theme, and interaction hardening | 60k | G11 |
| 12 | README/reference/API documentation and screenshot promotion | 55k | G12 |
| 13 | Final independent release validation and ledger reconciliation | 55k | G13 |

Do not combine phases. If a phase exceeds its target or touches more than roughly eight substantive
files, split it into A/B Builder packets and verify A before starting B.

### 6.1 Authoritative capture scenario and output contract

Phase 1 creates these scenario keys and makes this table executable. Shot names listed later in a
gate are semantic shot IDs. Their final artifact name is always
`<scenario>--<runtime-tag>--<shot-id>.png`, and the receipt records the absolute group-relative path.

| Gates | Group | Scenario key / exact command | Fixture and required output contract | Dimensions |
|---|---|---|---|---|
| G0 diagnostic | existing group | `rtk env SCREENSHOT_PORT=17800 node tests/ui/capture/index.mjs --scenario spawn-wizard`, then `rtk env SCREENSHOT_PORT=17814 node tests/ui/capture/index.mjs --scenario spawn-wizard-engines` | Current untrusted llama/Rapid states; console/file inventory only | Existing viewport; diagnostic |
| G1/G2 | `wizard-llamacpp` | `rtk env SCREENSHOT_PORT=17801 node tests/ui/capture/index.mjs --scenario spawn-wizard-baseline-llama` | llama general fixture; Tune top, drawer target, launch, Pro-toggle result | 1440x900 dark; 430x900 narrow |
| G1/G2 | `wizard-rapidmlx` | `rtk env SCREENSHOT_PORT=17802 node tests/ui/capture/index.mjs --scenario spawn-wizard-baseline-rapid` | Rapid text fixture; Tune top, launch, effective rail | 1440x900 dark/light |
| G5 | `wizard-llamacpp` | `rtk env SCREENSHOT_PORT=17805 node tests/ui/capture/index.mjs --scenario spawn-wizard-llama-guided` | Five workloads; q8/q4, vision, MTP, drawer, long/degraded | 1440x900 dark/light; 430x900 narrow |
| G6 | `wizard-rapidmlx` | `rtk env SCREENSHOT_PORT=17806 node tests/ui/capture/index.mjs --scenario spawn-wizard-rapid-guided` | Five workloads; planning context, active/retained, capacity, unavailable capability, drawer | 1440x900 dark/light; 430x900 narrow |
| G7 | `wizard-llamacpp` | `rtk env SCREENSHOT_PORT=17807 node tests/ui/capture/index.mjs --scenario spawn-wizard-pro-shell` | same canonical llama fixture before/after switch; rail/search/modified/reset | 1440x900 dark/light; 430x900 narrow |
| G8 | `wizard-llamacpp` | `rtk env SCREENSHOT_PORT=17808 node tests/ui/capture/index.mjs --scenario spawn-wizard-llama-pro` | every populated category and archived llama walkthroughs | 1440x900 dark/light; 430x900 narrow |
| G9 | `wizard-rapidmlx` | `rtk env SCREENSHOT_PORT=17809 node tests/ui/capture/index.mjs --scenario spawn-wizard-rapid-pro` | every populated category, active-memory/effective/search/modified/trust states | 1440x900 dark/light; 430x900 narrow |
| G10 | matching backend group | `rtk env SCREENSHOT_PORT=17810 node tests/ui/capture/index.mjs --scenario spawn-wizard-launch-contracts` | both views/backends, Full config evidence labels, validation, preset restore | 1280x900 and deliberate 1280x1400 review |
| G11 | matching backend group | `rtk env SCREENSHOT_PORT=17811 node tests/ui/capture/index.mjs --scenario spawn-wizard-accessibility` | pairwise matrix below; required outputs enumerated in its manifest | 1440x900, 430x900 |
| G12/G13 | both wizard groups | `rtk env SCREENSHOT_PORT=17812 node tests/ui/capture/cli-group.mjs wizard-llamacpp --no-attach`, then `wizard-rapidmlx` on port 17813 | all registered wizard scenarios, sequential per group | scenario manifests |

`rtk cargo build --release` precedes every row. Commands in the same row/table are still run
sequentially; never reuse a live port. Phase 1 may adjust unused port numbers, but it must update this
table and scenario receipts in the same change.

### 6.2 Pre-split Terra packet boundaries

These boundaries are mandatory so a lower-tier Builder never chooses an architectural split ad hoc:

| Phase | Builder packets (each followed by a fresh Verifier) |
|---:|---|
| 1 | 1A strict manifest/receipt runner + tests; 1B llama scenario split/migration; 1C Rapid group migration + docs/baselines |
| 2 | 2A shell + model/discovery extraction; 2B shared Tune + Guided/Pro extraction; 2C launch extraction + final at-rule-aware CSS validator |
| 3 | 3A repository-wide inference inventory/evidence classification; 3B typed local/HF GGUF + MLX default/provenance responses; 3C remove every unsafe inference fallback and close degraded/auth tests |
| 4 | 4A descriptor kinds/mount inventory/backend contract validator; 4B retire old IA owners + atomic mount/restore engine; 4C async default/dirty/provenance pipeline + single drawer + payload/preset regression |
| 5 | 5A sticky/context/KV; 5B vision/speculation state machines; 5C drawer/actions/workload matrix + regressions |
| 6 | 6A engine isolation/context planning/active-retained memory; 6B capacity/generation/capability/unavailable-state semantics; 6C drawer/actions/workloads + stale-preset regressions |
| 7 | 7A toggle/rail/panes/focus; 7B search/keyboard/scroll-spy; 7C dirty/modified/reset + responsive shell |
| 8 | 8A Model/Memory/Performance; 8B Generation/Tools/Network/Advanced + walkthroughs |
| 9 | 9A Memory/Performance; 9B Generation/Tools/Network/Advanced; 9C parity ledger + unavailable/trust/preset regressions |
| 10 | 10A canonical Full config/Launch evidence; 10B credential-safe preset hydration + API/client-validation navigation |
| 11 | 11A accessibility/keyboard/axe; 11B responsive/theme/reduced-motion pairwise matrix |
| 12 | 12A canonical Spawn Wizard/API/README docs; 12B peripheral reference audit + screenshot promotion/unused check |

### 6.3 Primary source worklists

Use these as starting worklists, then narrow per packet from the Phase 0 inventory:

- UI/state: `static/index.html`, `static/js/features/spawn-wizard.js`,
  `static/js/features/spawn-wizard-guided.js`, `static/js/features/spawn-wizard-ia.js`,
  `static/js/features/spawn-wizard-llama-ia.js`, `static/js/features/spawn-wizard-mlx-ia.js`,
  `static/js/features/spawn-wizard-groups.js`.
- Truth/payload/review: `static/js/features/spawn-wizard-vram-display.js`,
  `static/js/features/vram-estimate.js`, `static/js/features/spawn-wizard-rapid-mlx.js`,
  `static/js/features/spawn-wizard-review-step.js`, `static/js/features/spawn-wizard-spawn.js`.
- Preset dual surface: `static/js/features/presets.js`,
  `static/js/features/preset-editor-mlx.js` where present, Preset Editor rows in
  `static/index.html`, `src/presets/mod.rs`, `src/web/api/presets.rs`,
  `src/inference/launch.rs`, `src/web/api/sessions.rs`.
- Sampling/introspection: `src/web/api/benchmark.rs`, `src/llama/sampling_catalog.rs`,
  `src/llama/model_defaults.rs`, `src/hf/mod.rs`, `src/web/api/models.rs`,
  `src/web/api/vram.rs`.
- Capture: `tests/ui/capture/index.mjs`, `cli-group.mjs`, `cli-manifest.mjs`,
  `harness/shot.mjs`, relevant scenarios, and `tests/ui/README.md`.
- Generated assets: `src/gen/static_assets.rs` and `src/gen/routes.rs` after each build that adds or
  removes a static file.

### Documentation routing by phase

Builders read only the routed sources for their phase plus this plan's authority/frozen-decision
sections; they do not need the entire 2,700-line comprehensive plan.

| Phase | Required source sections/patterns |
|---:|---|
| 0 | Archived redesign §§2–8; execution companion Phase 10; comprehensive D6–D10/D16, Phase 10, UI matrix; `docs/reference/spawn-wizard.md` |
| 1 | `AGENTS.md` Screenshot Harness/Screenshots Workflow; `tests/ui/README.md`; current `captureShot`, group runner, engine fixture, and capture manifest |
| 2 | `AGENTS.md` Static Asset Registration and UI/UX Collaboration; current CSS major-section ledger; `static/index.html` stylesheet order |
| 3 | Execution companion Phase 10e; comprehensive D2/D13/D27; GGUF/MLX introspection and estimator patterns named in Phase 3 below |
| 4 | Comprehensive D6/D7; archived redesign §§2, 6, 7; current `createWizardIA` anchor/restore pattern and loader registries |
| 5 | Archived redesign §3 llama walkthroughs and §6 llama inventory; `docs/reference/inference-tuning.md`; `docs/reference/vram-estimator.md` |
| 6 | Archived redesign §3 S3 and §6 Rapid inventory; current `docs/reference/rapid-mlx-runtime.md`; Phase 6.5 suspension evidence |
| 7 | Archived redesign §4 anti-scroll mechanisms and §7 recommendation; comprehensive D7 and UI matrix |
| 8 | Archived redesign §4 llama walkthroughs and §6 llama inventory; current llama payload/preset contracts |
| 9 | Archived redesign §4 Rapid walkthrough and §6 Rapid inventory; current Rapid profile, estimator, command-preview, payload, and preset contracts |
| 10 | Archived redesign §6 “Migrated from step 3/4/5”; `docs/reference/spawn-wizard.md`; Rust `ModelPreset` and launch-request paths |
| 11 | Comprehensive §12.8 UI/accessibility matrix; `AGENTS.md` UI/UX and JavaScript linting rules |
| 12 | `AGENTS.md` Documentation and Screenshots Workflow; Phase 0 route/doc inventory; current README/reference files |
| 13 | `AGENTS.md` Mandatory Pre-PR Checks, Playwright command, Security checklist, and this plan's final acceptance matrix |

## 7. Phase 0 — Evidence freeze and contract inventory

### Objective

Create a durable, current-source baseline so later agents cannot inherit false completion claims.

### Read first

- This plan in full.
- Archived redesign §§2–8.
- Execution companion Phase 10, but treat status lines as unverified.
- Comprehensive plan D6–D10, D16, Phase 10, and UI/accessibility matrix.
- `docs/reference/spawn-wizard.md`.
- `AGENTS.md` screenshot, static-asset, security, serialization, and documentation rules.

### Builder work

1. Record `HEAD`, branch, dirty files, release binary identity, and capture-harness command in a
   Phase 0 receipt under `docs/plans/evidence/spawn-wizard-guided-pro/`.
2. Generate a machine-readable control inventory from current `CONTROLS`, llama groups, MLX groups,
   canonical DOM IDs, payload keys, and preset keys. Classify each control as:
   `guided-card`, `guided-drawer`, `pro-pane`, `launch-only`, `read-only-effective`, or
   `backend-unavailable`. Store the checked-in contract as
   `tests/ui/core/fixtures/spawn-wizard-control-contract.json`, generate/validate frontend portions
   with `scripts/validate-wizard-groups.mjs`, and add a Rust contract fixture/test for typed
   serializer/preset fields. Fields that cannot be mechanically joined are marked `human-audited`
   with a source path rather than reported as automated proof.
3. Generate a route inventory for every API called by the wizard, including auth level and source
   handler.
4. Generate a current capture manifest from registered scenario call sites. Mark artifact files
   with no producer as stale evidence.
5. Record the known contradictions in the Phase 10 execution companion and Spawn Wizard reference.
   Do not mark old phases complete or rewrite history yet.
6. Freeze the representative fixtures used throughout this plan:
   - llama.cpp General chat with q8_0 and Roleplay with q4_0;
   - llama.cpp Interactive coding and Tool/research with q8_0;
   - Deterministic batch/eval with explicit sampling ownership;
   - llama.cpp MTP eligible and ineligible;
   - Rapid text model with fixed/effective active KV and retained cache;
   - Rapid speculative unavailable/suspended;
   - long repo/model identity;
   - introspection unavailable/degraded.

### Verification

- Inventory IDs are unique and every applicable DOM ID exists once.
- Every visible control maps to a payload/preset/effective read path or is labeled read-only.
- Every current artifact is either produced by a registered scenario or explicitly listed stale.
- The receipt contains source paths and commands, not prose-only claims.

### Diagnostic evidence G0 (not a visual acceptance gate)

After a fresh release build, run the current registered `spawn-wizard` and
`spawn-wizard-engines` scenarios exactly as they exist and attach their console output plus produced
file list to the Phase 0 receipt. Do not invent new filenames or trust unproduced files. The receipt
must identify skipped, stale, mis-grouped, and broken output. Phase 1 creates the first trustworthy
visual baseline and is the first screenshot acceptance gate.

### Anti-pattern guards

- Do not copy status from memory or the old ledger.
- Do not use old PNG timestamps as evidence.
- Do not modify product UI in this phase.
- Do not infer missing control behavior from labels or CSS class names.

## 8. Phase 1 — Make screenshot evidence trustworthy

### Objective

Make a successful scenario mean that every requested shot was freshly produced with the intended
state, correct backend/group ownership, and realistic dimensions.

### Copy/adapt from

- Deterministic API/state fixtures in
  `tests/ui/capture/scenarios/wizard-llamacpp/spawn-wizard-engines.mjs`.
- Sequential group execution in `tests/ui/capture/cli-group.mjs`.
- Viewport capture behavior in `tests/ui/capture/harness/shot.mjs::captureShot`.

### Builder work

1. Make group/category ownership exact: llama Spawn Wizard still/GIF scenarios live under
   `wizard-llamacpp`; Rapid Spawn Wizard still/GIF scenarios live under `wizard-rapidmlx`;
   `rapid-mlx-runtime`, `rapid-mlx-live`, and `dashboard-rapid-mlx` move to or remain in their own
   runtime/dashboard functional groups rather than being treated as wizard coverage. The registry
   category and filesystem scenario folder must agree.
2. Split monolithic wizard scenarios by user-visible contract rather than appending more captures
   to one long script:
   - llama Guided;
   - llama Pro;
   - Rapid Guided;
   - Rapid Pro;
   - engine/model selection;
   - launch/preset review;
   - accessibility/responsive matrix.
3. Give each scenario an explicit expected-output manifest. A missing or skipped required shot fails
   the scenario. Optional shots must be declared optional with a reason.
4. At scenario start, remove only that scenario's exact expected filenames from its artifact group.
   Never recursively clear the artifact tree. At scenario end, write a receipt containing produced
   filenames, dimensions, theme, viewport, backend, view, and fixture identity.
5. Add a required `INTENT` string for every wizard capture describing the visible state and the
   assertion a reviewer should make from it.
6. Remove current Pro TODO scaffolding from the general llama scenario. Until Phase 7, the dedicated
   Pro baseline scenario must intentionally assert `not implemented` and capture the broken result;
   it must not create a golden-looking filename.
7. Ensure captures reset the correct scroll container to a deliberate top/anchor position. Do not
   use `fullPage: true` as a substitute for internal modal scrolling.
8. Add harness tests for stale-file detection, missing expected outputs, wrong output group, and
   realistic maximum dimensions.
9. Update `tests/ui/README.md` and scenario help text with the new groups and receipt contract.
10. Make `cli-manifest.mjs --strict` exit nonzero for a missing wizard `INTENT`, duplicate final
    filename, unregistered scenario, group/category mismatch, or expected/produced manifest mismatch.

### Verification

- Running a scenario after deleting one capture call fails on its missing manifest output.
- Stale Pro PNGs are not reported as produced by a fresh run.
- `wizard-rapidmlx` produces the Rapid wizard stills.
- No output exceeds the agreed realistic viewport envelope (default desktop 1440x900 or 1280x900;
  narrow 430x900; a deliberate 1280x1400 review viewport is allowed only when named).
- `rtk node tests/ui/capture/cli-manifest.mjs --strict` passes for all wizard call sites and its
  negative-fixture tests prove each violation exits nonzero.

### Screenshot gate G1

Produce the five baseline states defined in the authoritative capture table in §6.1 plus dark/light
and desktop/narrow engine-selection states. Verify that the receipts name exactly the files created
in this run. G1 becomes the before-set for the CSS split.

### Anti-pattern guards

- No whole-artifact-directory deletion.
- No giant flattened modal screenshots.
- No scenario success after `Skipped non-full-page` for a required image.
- No fixture mutation that bypasses the user-visible control under test.

## 9. Phase 2 — Split `spawn-wizard.css` without changing pixels

### Objective

Replace the 7,851-line mixed-concern stylesheet with ordered modules before adding more Guided/Pro
layout work. This phase is structural only.

### Target modules and order

Load these as separate `<link rel="stylesheet">` entries in `static/index.html`, in this order:

1. `css/spawn-wizard-shell.css` — modal, header, three-step body, footer, focus, shared responsive
   shell, reduced motion.
2. `css/spawn-wizard-model.css` — guidance/use-case cards, engine/source selection, HF discovery,
   quant advisor, community picks, model-card/origin/tag UI.
3. `css/spawn-wizard-tune.css` — shared hardware rows, model header, memory/VRAM rail, context-fit,
   mmproj, MTP, Rapid hardware/effective rows, download state.
4. `css/spawn-wizard-guided.css` — Guided decision/card/drawer presentation only.
5. `css/spawn-wizard-pro.css` — Pro toggle, rail, pane, search, modified/reset presentation only.
6. `css/spawn-wizard-launch.css` — sampling/reasoning/shaping/network review, canonical config,
   preset/save, command preview, spawn/progress.

Keep `spawn-wizard.css` only as a temporary compatibility shim if current external consumers require
the URL. If retained, it must contain no product selectors and clearly point to the new linked
modules; do not use chained `@import` as the primary loader.

### Builder work

1. Produce a line/selector migration ledger before moving rules.
2. Execute this phase as three mandatory bounded packets, each with its own G2 comparison:
   - **2A:** shell and model/discovery modules;
   - **2B:** shared Tune, Guided, and Pro modules;
   - **2C:** Launch module plus final cross-module validator.
   Move rules in existing cascade order. Do not rename selectors, change specificity, consolidate
   declarations, or "clean up" values in the same patch.
3. Identify rules that belong to the Models modal or another non-wizard consumer. Move them only
   after confirming their owning markup and ensuring that surface loads the new owner stylesheet.
4. Add a focused validator that flags duplicate selectors across wizard modules, missing light-theme
   partners for component blocks, and animation/transition blocks without reduced-motion review.
   It must be CSS-parser/at-rule aware: an intentional selector repeated inside a media, theme, or
   reduced-motion scope is not treated as a global duplicate.
5. Rebuild so generated asset constants/routes include every new file.
6. Add `pixelmatch` and `pngjs` as test-only dependencies and a deterministic
   `tests/ui/capture/compare-sets.mjs` comparator. Await `document.fonts.ready`, suppress cursor/time
   noise, require identical dimensions, use threshold `0.1`, and require `maxDiffPixels=0` for this
   CSS-only phase. Emit diff images and JSON receipts on failure. Any delta blocks the packet unless
   a fresh same-source control pair proves capture nondeterminism.

### Verification

- `spawn-wizard.css` no longer contains the migrated product rules.
- New files are served successfully and appear in generated assets/routes.
- `rtk npm run lint`, `rtk npm run validate-js`, CSS validator,
  `rtk cargo build --release`, and `rtk git diff --check` pass.
- Engine cards, both broken baseline Tune states, launch, dark/light, narrow, and reduced-motion
  captures are pixel-equivalent to G1.
- 2A is independently verified before 2B, and 2B before 2C; if any packet cannot preserve exact pixels,
  revert only that packet's intended CSS moves and stop for Coordinator review rather than carrying
  a half-split stylesheet into feature work.

### Screenshot gate G2

Repeat G1 exactly and store before/after comparison metadata. This is a no-visual-change gate.

### Anti-pattern guards

- No new Pro or Guided behavior.
- No selector renames mixed into file moves.
- No `@import` waterfall as the final architecture.
- No moving shared Models-modal rules without proving its stylesheet loading.
- No deletion of apparently dead `.pro-*` rules until Phase 4 inventories their intended owner.

## 10. Phase 3 — Introspection, effective values, and provenance truth

### Objective

Ensure the values that drive recommendations, modified-state baselines, locked rows, and provenance
come from real GGUF/MLX metadata and runtime evidence before the views present them as authoritative.

### Copy/adapt from

- Local GGUF metadata: `src/llama/gguf_meta.rs`.
- Remote GGUF progressive header fetch: `src/hf/mod.rs::fetch_gguf_header_metadata`.
- MLX config fetch: `fetch_mlx_config` / `fetch_mlx_config_revision_aware`.
- Existing HF estimator branch in `src/web/api/vram.rs`.
- Current Rapid profile and command-preview requested/effective contracts.

### Builder work

1. Retain Phase 10e's whole-codebase mandate: audit every model-property inference, not only direct
   wizard call sites. Include family, architecture, parameter counts, active parameters, MoE, MTP,
   native context, vision, and sampling catalog selection. The minimum enumerated closure set is
   `src/llama/sampling_catalog.rs`, `/api/model-defaults` in `src/web/api/benchmark.rs`,
   `static/js/features/spawn-wizard.js`, streamed-selection paths in
   `static/js/features/spawn-wizard-hf-browse.js`, fallback behavior in `src/web/api/vram.rs`,
   `src/llama/model_defaults.rs`, and every additional site found by the audit.
2. Replace local-file name/repo/tag fallbacks with GGUF metadata. Replace streamed-HF fallbacks with
   progressive GGUF header or revision-aware MLX config reads.
3. Extend `/api/model-defaults` or its typed successor so a not-yet-downloaded HF selection carries
   source kind, repo, revision/file, size, and engine, then returns metadata-backed defaults with
   provenance and degraded reasons.
4. On introspection failure, return `unknown`/`degraded` and keep safe server defaults. Never guess a
   family or MTP capability from a name.
5. Separate artifact identification from model-property inference. If quant/artifact filename parsing
   remains necessary to select a file, document that boundary and do not reuse it to infer model
   architecture/capabilities.
6. Remove the current name-based automatic `draft-mtp` choice in `showStep`; enable or recommend it
   only from qualified introspection/capability evidence.
7. Define provenance values used by both views: `model-native`, `qualified-profile`,
   `workload-derived`, `runtime-effective`, `user`, `unknown/degraded`. Include evidence source and
   explanatory copy, not only a colored chip.
8. Add auth/routing tests for changed endpoints and focused local/remote GGUF + MLX fixtures.

### Verification

- A streamed model receives family-/mode-specific sampling defaults without download only when
  authoritative GGUF/MLX metadata encodes enough evidence. An arbitrary finetune whose persona or
  tuning intent is not encoded degrades to explicit universal defaults; it is never guessed from
  the repo or filename.
- Renaming a local GGUF does not change architecture/family/MoE/MTP/context results.
- A failed range/config fetch produces an explicit degraded result and no guessed recommendation.
- Repository-wide structural/search audit finds no filename/name/repo/tag substring fallback for a
  model property. Any deliberately retained artifact-name parsing is listed with a proof that it
  selects an artifact only and cannot feed architecture/capability/default inference.
- Preset restore uses stored explicit values; it does not re-infer them from the preset name.

### Screenshot gate G3

- llama streamed-HF metadata loading -> qualified result;
- llama metadata unavailable -> degraded/unknown copy;
- Rapid profile loading -> effective values pending, then resolved;
- long identity/revision provenance presentation.

These may use the existing baseline layout; the purpose is truth-state reproducibility.

### Anti-pattern guards

- No filename/repo/tag substring inference for model properties anywhere in the codebase.
- No frontend recreation of GGUF/MLX parsing.
- No silent fallback from failed introspection to a confident recommendation.
- No unauthenticated data-reading endpoint.

## 11. Phase 4 — Shared view descriptor and relocation engine

### Objective

Build the presentation foundation once so Guided and Pro render the same canonical controls without
proxy inputs or duplicated bindings.

### Copy/adapt from

- Comment-anchor preservation/restoration in `spawn-wizard-ia.js::createWizardIA`.
- Current loader registries in `spawn-wizard-groups.js`, `spawn-wizard-llama-ia.js`, and
  `spawn-wizard-mlx-ia.js`.
- Requested/effective rendering in `applyEffectiveLocks` and Rapid command preview.

### Descriptor contract

Every applicable mount needs presentation metadata sufficient for both views:

- reference to a backend-owned stable semantic ID plus a stable `mountId`/owner wrapper;
- mount kind: `setting-control`, `composite-group`, `action`, `read-only-status`,
  `conditional-wrapper`, or `launch-only`;
- loaders/backends as a presentation filter validated against backend capability data;
- one of the seven category IDs and optional subsection;
- Guided placement (`decision`, `drawer`, `capability-state`, `launch-only`);
- Pro placement and search keywords;
- presentation emphasis/help references; semantic risk and backend availability remain backend-owned;
- references to backend-owned default/effective/provenance providers;
- serializer/preset trace reference for validation, not a second serializer;
- aliases/help text for search and accessibility.

Counts deduplicate by semantic editable setting/mount rather than raw DOM ID; wrappers, actions, and
read-only status rows are reported separately and never inflate `N settings` or `N changed`. Search
indexes may include all kinds.

### Builder work

1. Replace `controlsForView()`'s unused/simple filtering with a validated frontend presentation
   registry that references, and is contract-tested against, backend-owned semantics.
2. Before adding a relocation owner, build a mount compatibility table for every entry: atomic
   wrapper, label/help IDs, conditional parent, `.closest()`/descendant query dependencies, CSS
   ancestor dependencies, cached DOM references, listener ownership, and prebuilt composite groups.
   Move an atomic row/group, never a naked input away from its label/help.
3. Disable both existing llama/MLX IA owners, restore their anchors, verify original DOM order, and
   only then instantiate the one new relocation owner. Two owners must never remember/move the same
   node. The new owner mounts a canonical row/group into Guided drawer, Guided decision shell, or Pro
   pane and restores it safely on view/backend changes.
4. Delete the two competing drawer implementations and create one Guided drawer populated from the
   registry.
5. Establish a per-setting state machine before modified-only work:
   `{resolvedDefault, value, dirty, provenance, effective, pending}`. A newly resolved async default
   updates `value` only when `dirty=false`; reset clears dirty and applies the latest resolved
   default. HTML `defaultValue` alone is never the modified baseline.
6. Create one event/update pipeline so a canonical control change refreshes state, estimator,
   provenance, validation, sticky digest, changed counts, and both presentation modes as applicable.
7. Replace MutationObserver/value-attribute assumptions with explicit state updates or property-aware
   events.
8. Add Node/Playwright contract tests:
   - unique IDs;
   - one canonical live control per setting;
   - every applicable control reachable in Guided or its drawer and in Pro;
   - no control mounted beneath a hidden parent;
   - descriptor/backend/payload/preset coverage;
   - view switching preserves values.
9. Add semantic launch projection and hydrated preset round-trip tests for both backends now, then
   rerun them at every Guided/Pro gate. Actions/status entries trace to handler/auth/API behavior,
   not invented serializer keys.
10. Retire the Quick/Balanced/Advanced UI and disclosure engine reads. Migrate any stored legacy
    preference to Guided/Pro without overwriting explicit settings, preserve safe-default/Auto-size
    behavior, and render the five transparent workload intents on Model. Validate their assumptions
    and reset/default effects independently from view mode.
11. Keep the view selector disabled or visibly `Coming in a later phase` for Pro until G7; do not let
   users enter an empty surface.

### Verification

- Guided drawer count is computed, nonzero, and matches reachable applicable controls.
- Changing a canonical control through either presentation yields the same `buildSpawnPayload()`.
- Switching engine/view repeatedly produces no duplicate IDs, listeners, or detached controls.
- Existing payload tests plus both hydrated preset round trips remain green.
- Model has one Guided/Pro disclosure axis, five workload intents, and no legacy profile control that
  still disables or overwrites settings.

### Screenshot gate G4

- llama Guided drawer closed/open with live total/changed count;
- Rapid Guided drawer closed/open with backend-native rows;
- switch engine twice and show no mixed-backend controls;
- Pro unavailable state is honest and non-destructive.

### Anti-pattern guards

- No proxy inputs synchronized to hidden originals.
- No hard-coded `23` count.
- No second payload builder.
- No frontend-owned runtime defaults.
- No `#wizard-step-N` selector when a semantic class/data attribute can express ownership.

## 12. Phase 5 — Complete llama.cpp Guided

### Objective

Implement the archived Guided experience for llama.cpp using canonical controls and live estimator
feedback.

### Builder work

1. Build the sticky context/budget strip from canonical model, quant, loader, use case, context, K/V,
   flash-attention, parallel, memory breakdown, and fit state.
   On desktop, Guided is a two-pane layout with the decision column and a sticky budget inspector;
   the primary decision set must be visible within roughly 1.5 ordinary viewports at 1440x900.
2. Context decision:
   - workload-aware quick choices bounded by introspected native context;
   - custom input updates canonical state;
   - per-choice KV cost from debounced/cached `/api/vram-estimate` payloads;
   - training/fit/warning copy rendered inside the card.
3. KV decision:
   - adjacent/matched K and V semantics;
   - workload-derived q4_0/q8_0 guidance;
   - q4_0 remains selectable for agentic use with persistent non-color warning;
   - provenance flips to `user` on edit and can reset to resolved recommendation.
4. Vision decision:
   - canonical mmproj selector and browse action;
   - image min/max token controls when a projector is selected;
   - explicit text-only and detected-but-unavailable states.
5. Speed decision:
   - one truthful state machine for MTP, n-gram, draft model, and off;
   - capability-based availability/copy;
   - inline tuning expander for the advanced speculation controls;
   - no unconditional `On — MTP heads detected`.
6. Populate All settings with placement, batching/threads, prompt cache, auto-fit, sweeps, sampling,
   thinking, shaping, tools/format, network/observability, extra args, and download utilities.
7. Move advisor and download warnings to the decision/rail region that causes them.
8. Render primary choices as task tiles/segmented decisions from the archived wireframe, not raw
   dropdowns placed inside cards. View-specific buttons may issue typed mutations to canonical state;
   they are not second form controls and do not own independent values.

### Verification

- Roleplay starts at q4_0 with appropriate provenance and no agentic warning. General chat remains
  q8_0 unless a separate evidence-backed policy decision changes it.
- Roleplay verification also shows conversation-format ownership, sampler precedence, reserved
  context/headroom, and cache guidance without overwhelming the primary decision path.
- Agentic starts at q8_0; selecting q4_0 warns but is not blocked.
- Custom context, K/V, mmproj, speed mode, and drawer controls serialize correctly.
- MTP-ineligible model never displays detected/enabled language.
- All settings count/reachability contract passes.
- The Guided primary column meets the bounded-scroll criterion and the inspector remains available
  while decisions scroll.

### Screenshot gate G5

Fresh, manifest-backed captures:

- `llama-guided-agentic-q8-dark.png`
- `llama-guided-roleplay-q4-light.png`
- `llama-guided-agentic-q4-warning.png`
- `llama-guided-vision-selected.png`
- `llama-guided-mtp-eligible.png`
- `llama-guided-mtp-unavailable.png`
- `llama-guided-drawer-changed.png`
- `llama-guided-narrow.png`

### Anti-pattern guards

- No duplicated speculative controls with conflicting state.
- No warning at the bottom of the page when it belongs to a card.
- No estimator formula in JavaScript.
- No tooltip-only safety guidance.
- No raw `<select>` as the primary context/KV/speed decision interaction.

## 13. Phase 6 — Complete Rapid-MLX Guided

### Objective

Replace the blank/mixed Rapid hardware page with backend-native Guided decisions and truthful
requested/effective states.

### Builder work

1. Ensure selecting Rapid hides/removes all llama-only Tune decisions and mounts Rapid descriptors
   into the same Guided shell and rail.
2. Context planning decision uses MLX config/profile native context and Rapid estimator semantics.
   Current Rapid launch config has no context/max-model-length field, so label this as a planning
   target and exclude it from requested launch config. Adding a real launch control requires a
   separately qualified Rapid CLI/API field first.
3. Active/retained memory decision:
   - active KV is a locked requested/effective row when the runtime pins it;
   - retained prefix cache remains an editable, separately labeled budget;
   - TurboQuant and PFlash display requested/effective evidence honestly;
   - fit scenarios vary actual Rapid free variables, never llama K/V quant choices.
4. Capacity/scheduler decision explains and controls supported `max-num-seqs`, request limits, and
   batch sizing without claiming a cache-entry recommendation from concurrency alone.
5. Generation/reasoning decision exposes qualified reasoning/parser/sampling behavior and shows
   speculative companions as suspended/unavailable unless the current runtime qualification permits
   them.
   For a preset-restored request that is now unavailable, preserve the requested value for review,
   mark it declined/omitted in command preview, and require an explicit supported choice before it
   can become effective. Do not silently clear it and do not emit it merely because hidden state is
   truthy. A capability that validation classifies as forbidden must fail with its semantic field ID.
6. Vision capability is unavailable/read-only while unqualified; no mmproj vocabulary is shown.
7. Populate the Guided drawer with protocol, scheduler, retained cache details, tools, companions,
   network/access, advanced escape hatches, and other applicable Rapid controls. Do not add a Web UI
   group: current Rapid has no llama `--ui`/`--path` equivalent, and stable category order does not
   require invented fields.
8. Keep command-preview requested/effective reasons available from Tune and Launch.

### Verification

- Rapid hardware page has a populated main column and no llama context/KV/mmproj/MTP controls.
- Fixed active KV cannot look editable.
- Retained cache changes alter Rapid payload and estimator while active KV remains distinct.
- Unsupported speculation/vision cannot be enabled through hidden or stale controls.
- Context planning targets affect recommendations/estimates but do not appear as a Rapid launch arg.
- Rapid Roleplay shows its actual format-owner/parser, sampling precedence, context-planning reserve,
  and retained-cache guidance rather than copying llama q4 behavior.

### Screenshot gate G6

- `rapid-guided-context-memory-dark.png`
- `rapid-guided-effective-locks-light.png`
- `rapid-guided-retained-cache-changed.png`
- `rapid-guided-capacity.png`
- `rapid-guided-speculation-unavailable.png`
- `rapid-guided-drawer-changed.png`
- `rapid-guided-narrow.png`
- `rapid-guided-launch-preview.png`

### Anti-pattern guards

- No forced visual 1:1 mapping to llama.cpp.
- No editable-looking pinned setting.
- No claim that Rapid uses llama cache-type flags, mmproj, or llama speculation.
- No product re-enablement of suspended MTP/vision from UI work alone.

### Human acceptance stop after G6

Present the complete G5 and G6 Guided sets to the user. Record accepted/rejected filenames and any
requested remediation in the phase receipt. Do not begin Pro implementation until the four-decision
card set, backend adaptation, density, copy, and narrow behavior are explicitly accepted.

## 14. Phase 7 — Build the shared Pro shell

### Objective

Implement Pro navigation and interaction infrastructure once, over the same descriptors and controls
proven in Guided.

### Builder work

1. Make the Guided/Pro toggle rerender the active Tune presentation immediately and restore the
   session preference on reopen.
2. Build the persistent seven-category left rail with active-section state, keyboard navigation,
   semantic headings, and scroll-spy within the Tune pane. Each category may contain stable
   subsection anchors so the archived 9–11-anchor random-access behavior is retained without
   abandoning the seven cross-surface categories.
3. Build dense responsive panes from the shared descriptor registry. Related scalar fields may share
   a row, but canonical control order and label relationships remain accessible.
4. Implement Cmd/Ctrl+K search:
   - active only while the wizard is open;
   - does not steal shortcuts while typing in an input;
   - searches labels, IDs/flags, help text, aliases, and category breadcrumbs;
   - shows results with category/subsection context;
   - Enter navigates/focuses; Escape clears search before closing the wizard.
5. Implement `Modified only (N)` from resolved-default snapshots. Unknown/effective-pending values are
   not falsely classified as modified.
6. Implement `Reset tune settings` to resolved defaults. It must not reset model, engine, workload,
   credentials, or launch identity. Recompute estimator/effective state after reset.
7. Preserve the sticky memory strip and one-line effective config digest in Pro.
8. Add focus restoration when switching view and when search/rail navigation changes panes.

### Verification

- Guided -> Pro -> Guided preserves all canonical values and identical payload.
- Reopen in the same browser session restores Pro; a fresh session defaults to Guided.
- Search finds a control in every populated category. A truly empty stable category is non-selectable
  or shows an intentional backend-unavailable state; no control is invented to fill it.
- Modified-only count responds to user edits, workload-derived defaults, async effective values, and
  reset correctly.
- Cmd/Ctrl+K and Escape behavior passes keyboard tests.
- No `.pro-*` rule remains orphaned; stale Pro rules are reused deliberately or deleted.

### Screenshot gate G7

- Pro shell landing pane, dark/light;
- rail active/scroll-spy state;
- search results with breadcrumbs;
- modified-only state;
- reset result;
- narrow rail/pane adaptation;
- Guided/Pro switch before/after with the same visible values.

### Anti-pattern guards

- No second Pro-only state object or payload builder.
- No one-column dump masquerading as Pro.
- No hidden accordion maze in Pro.
- No global shortcut interception when the wizard is closed.
- No reset of model/source/credentials.
- No Pro category pane exceeds roughly 1.5 viewports at 1440x900 without subsection random access;
  the Performance pane must demonstrate dense related rows rather than recreating the legacy wall.

## 15. Phase 8 — Complete llama.cpp Pro panes

### Objective

Populate all llama.cpp settings into the Pro shell and satisfy the archived Pro walkthroughs.

### Builder work

1. `Model & compatibility`: selected artifact/quant, tags, mmproj/image tokens, chat-template
   ownership/status, capability evidence.
2. `Memory & context`: context, adjacent K/V, unified KV, prompt cache, auto-fit/target, effective
   memory deltas and fit scenarios.
3. `Performance`: GPU layers, MoE offload/autotune, tensor split, batch/ubatch/parallel/threads/prio,
   flash attention, speculation/draft settings, benchmarks.
4. `Generation & reasoning`: sampling catalog/defaults, thinking/reasoning, max tokens and related
   shaping inputs.
5. `Tools & conversation formatting`: grammar/JSON schema/tool-call format and chat-template status.
6. `Network & observability`: bind/port/alias/API key and supported Web UI/observability controls.
7. `Advanced`: mlock, extra args, escape hatches, and rare controls with explicit risk/help.
8. Keep K/V matching and agentic q8 guidance inline and non-blocking.

### Verification

- Every applicable llama descriptor appears in exactly one Pro category and remains searchable.
- Archived llama Pro roleplay/q4 and agentic/q8 walkthroughs pass.
- K/V mismatch guidance is visible and serialization remains explicit.
- Modified-only and reset cover every pane.

### Screenshot gate G8

One fresh capture per Pro category plus:

- agentic q8 baseline;
- agentic q4 warning;
- roleplay q4 baseline;
- `kv` search;
- modified-only multi-category state;
- narrow Memory & context and Performance panes.

### Anti-pattern guards

- No field omitted because it is rare.
- No duplicate K/V or speculation controls.
- No unsafe raw extra-arg rendering.
- No implied exact argv without a server-backed preview.

## 16. Phase 9 — Complete Rapid-MLX Pro and parity audit

### Objective

Populate backend-native Rapid panes, then prove meaningful parity and document intentional
differences.

### Builder work

1. Map every Rapid descriptor into the same seven category shell.
2. `Memory & context` lands on locked active memory, retained cache, the non-launch context planning
   target, prefill sizing, and requested/effective TurboQuant/PFlash evidence.
3. `Performance` contains scheduler/batching/concurrency and qualified acceleration settings.
4. `Generation & reasoning` contains reasoning mode/parser and sampling mode/defaults.
5. `Tools & conversation formatting` contains tool parser/auto-tool choice and template/protocol
   ownership.
6. `Network & observability` contains access, Web UI, and applicable diagnostics.
7. `Advanced` contains hybrid override, companions/trust, disable-auto-K, and escape hatches with
   capability and security gating.
8. Produce a parity ledger with three classifications for every cross-backend concept:
   `equivalent`, `backend-native difference`, or `unsupported/unqualified`.
9. Close real missing equivalents; preserve and label intentional differences.

### Verification

- Every Rapid descriptor is reachable/searchable in Pro.
- Archived Rapid Pro active-memory scenario passes using current runtime truth.
- Mixed llama/Rapid payload guards still pass.
- Trust consent remains revision-scoped; secrets never enter capture receipts or command preview.
- The parity ledger matches payload/schema/adapter reality.

### Screenshot gate G9

One fresh capture per populated Rapid Pro category plus:

- active-memory requested/effective block;
- retained-cache edit;
- parser/profile evidence;
- speculative companion unavailable/trust-gated states;
- `cache` search;
- modified-only;
- narrow state.

### Anti-pattern guards

- No llama vocabulary imposed on Rapid.
- No invented equivalent solely to fill a category.
- No secret values in screenshots or receipts.
- No remote-code consent bypass.

### Human acceptance stop after G9

Present the G7–G9 Pro sets and the parity ledger to the user. Record acceptance of rail/category
navigation, search, modified/reset behavior, density, backend adaptation, and narrow layout. Do not
start Launch/documentation convergence until this acceptance or focused remediation is complete.

## 17. Phase 10 — Launch, review, preset, and API convergence

### Objective

Finish the three-step collapse by replacing duplicate summaries with one canonical requested/effective
review and proving save/edit/launch parity.

### Builder work

1. Replace the current duplicate/empty summary surfaces with one `Full config` drawer available from
   Tune and Launch. Label every row as `requested`, `estimator-effective`, `runtime-effective`,
   `unknown`, or `unavailable`. Rapid command preview may supply runtime-effective evidence; llama
   rows must not imply exact effective argv where only requested config/estimator evidence exists.
2. Launch shows model, engine, endpoint/access, memory fit, changed settings, warnings/degraded
   evidence, and exact Rapid command preview. It does not reintroduce a long editable settings page.
3. Keep preset name/save optional in the Launch bar. Launch and save derive backend setting values
   from the same canonical projection, then use intentionally different transport shapes: preset
   projection adds its name, strips/promotes protected Rapid credentials, and preserves fields needed
   for `ModelPreset`; launch uses the session-spawn request.
4. Prove wizard save -> Rust `ModelPreset` -> Preset Editor edit/save -> launch restore for both
   engines, including fields moved between Guided drawer/Pro panes.
5. Update the Preset Editor only where schema, category, effective/provenance, or duplicated
   chat-template behavior requires parity. Any install/history/rollback/status change must update
   `spawn-wizard-chat-template.js`, `presets.js`, the `modal-chat-template-file` row in
   `static/index.html`, backend routes/tests, and `docs/reference/spawn-wizard.md`. Do not redesign
   unrelated preset UI in this phase.
6. Preserve protected API-key handling and secret scrubbing in preview/receipts.
7. Add/update auth/routing tests for every wizard-used endpoint changed since the current API docs.
8. Client guardrail errors must focus/navigate to the owning semantic control in either view. If
   server validation must do the same, first add a typed `{code, field_path, message}` error contract
   and mapping tests; do not parse free-form session-spawn strings into field IDs.

### Verification

- Guided and Pro produce deep-equal `buildSpawnPayload()` output for the same values.
- Preset projection/hydration yields a semantically equivalent `LocalLaunchRequest` while preserving
  the intentional credential-safe transport differences.
- llama and Rapid presets round-trip without stale fields crossing backend switches.
- Rapid preview reflects the same adapter argv used for launch.
- Client guardrail errors return to and focus the owning Tune control in either view; server errors
  do so only if the typed field-path contract was implemented.

### Screenshot gate G10

- llama Guided Launch and Full config;
- llama Pro Launch and Full config;
- Rapid Guided Launch with command preview/effective reasons;
- Rapid Pro Launch;
- validation error return-to-control;
- preset saved/restored state for both engines.

### Anti-pattern guards

- No third summary implementation.
- No client-side exact Rapid argv reconstruction.
- No schema field added only to wizard or only to preset editor.
- No secret display/log/capture.

## 18. Phase 11 — Responsive, accessibility, theme, and interaction hardening

### Objective

Make both views usable beyond the primary dark desktop screenshots.

### Builder work

1. Verify desktop, narrow, dark, light, and reduced-motion for both engines/views.
2. Define narrow behavior deliberately:
   - Guided rail becomes an in-flow budget section or accessible drawer;
   - Pro rail becomes a category selector/drawer without hiding settings;
   - dense rows collapse to one column without changing logical order.
3. Verify keyboard traversal, visible focus, labels/descriptions, dialog semantics, screen-reader step
   announcements, search results, drawer/rail expanded state, and focus restoration.
   Add `@axe-core/playwright` as a test-only dependency and scan Model, Guided Tune, Pro Tune/search,
   and Launch for both backends. Serious/critical violations block; lower-severity findings require
   an explicit disposition in the receipt.
4. Ensure warnings use icon/text in addition to color and all `Why?` content is keyboard/touch
   reachable.
5. Add long-name, long-help, unavailable, provisional, warning, and validation-error fixtures.
6. Run a CSS audit for duplicate selectors/specificity conflicts, missing light overrides, and missing
   reduced-motion handling.
7. Run JS->HTML->CSS reference and backend/frontend contract checks.

### Verification

- No horizontal clipping at supported widths.
- No setting becomes unreachable on narrow screens.
- Automated axe/focus tests pass for representative states.
- Reduced motion removes new animated scrolling/transitions where appropriate.
- Light theme retains contrast and visible warning/effective states.

### Screenshot gate G11

Use a bounded pairwise matrix rather than a 32-image Cartesian product:

- all four engine/view combinations: dark desktop smoke;
- llama Guided + Rapid Pro: light desktop and narrow-open state;
- Rapid Guided + llama Pro: reduced motion and narrow closed state;
- one long identity/help state per engine;
- one unavailable/provisional state per engine;
- one validation-error state per view.

The scenario manifest enumerates each required shot and dimensions. Run sequentially. Automated
layout/accessibility tests cover the remaining combinations without creating an unreviewable image
pile.

### Anti-pattern guards

- No desktop-only acceptance.
- No sidebar simply hidden on narrow when it owns required controls.
- No hover-only explanation.
- No warning conveyed only by color.

## 19. Phase 12 — Documentation and screenshot closure

### Objective

Make repository documentation describe the behavior that was actually verified, as though the
finished feature always existed, while correcting false historical status claims.

### Required documentation audit

Search `README.md` and every `docs/reference/*.md` file for `spawn wizard`, `setup wizard`, wizard
step numbers, Guided/Pro, Quick/Balanced/Advanced, and every API route in the Phase 0 route inventory.
At minimum, inspect and update as applicable:

- `README.md`
- `docs/reference/spawn-wizard.md`
- `docs/reference/setup-wizard.md`
- `docs/reference/quick-start.md`
- `docs/reference/api.md`
- `docs/reference/hf-model-library.md`
- `docs/reference/inference-tuning.md`
- `docs/reference/rapid-mlx-runtime.md`
- `docs/reference/vram-estimator.md`
- `docs/reference/model-library.md`
- `docs/reference/memory-management.md`
- `docs/reference/navigation.md`
- `docs/reference/backend-inference.md`
- `docs/reference/dashboard.md`
- `docs/reference/tune-panel.md`

### Builder work

1. Make `spawn-wizard.md` the canonical feature reference. Reduce duplicated architecture text in
   `setup-wizard.md` to a concise task flow plus links where practical.
2. Document:
   - the three steps;
   - the independent guidance-level and Guided/Pro axes;
   - Guided decisions and All settings behavior for each backend;
   - Pro categories, search, modified-only, reset, and persistence;
   - requested/effective/provenance meanings;
   - backend differences and unavailable/suspended capabilities;
   - preset/save/launch behavior;
   - narrow/keyboard behavior.
3. Update API docs for all wizard-used routes. Current known omissions include Rapid command preview,
   MTP preflight/recheck, and Rapid model profile. Correct current auth/method inaccuracies such as
   the community-picks route while verifying against source.
4. Correct stale step-number captions and the nonexistent redesign-doc link.
5. Remove "implemented and verified" claims that predate G13 or replace them with evidence-linked
   current status.
6. Promote only final G11/G12 screenshots that are actually referenced. Replace blank/stale promoted
   Rapid images and obsolete wizard-flow media. Do not promote every matrix image.
7. Run `rtk bash scripts/check-unused-screenshots.sh` and resolve every result.
8. Add a small maintainer section documenting the descriptor validator, capture manifests, and how
   to add a setting to both views.

### Verification

- Repository-wide search finds no stale six-step/current-tier-layout descriptions.
- README and reference docs agree on names, steps, defaults, and backend differences.
- Every documented route/method/auth contract matches source and auth tests.
- Every promoted screenshot is referenced; every reference resolves.
- Documentation never claims unqualified Rapid vision/MTP functionality.

### Screenshot gate G12

Run the final docs-selected scenarios from a clean artifact manifest. Promote only the approved
images after visual review.

### Anti-pattern guards

- No documentation-only follow-up after code merge.
- No copied old screenshots presented as current.
- No endpoint documented as public without source/auth proof.
- No blanket claim of backend parity.

## 20. Phase 13 — Final independent validation and ledger reconciliation

### Objective

Prove the golden state end to end, then and only then update Phase 10 status.

### Required validation order

Run repository mandatory checks in the exact `AGENTS.md` order:

1. `rtk cargo clippy -- -D warnings`
2. `rtk cargo test`
3. `rtk npm run validate-js`
4. `rtk npm run lint`
5. `rtk git diff --check`
6. `rtk cargo build --release`
7. `rtk cargo fmt`
8. Commit any formatter/generated auto-changes through the Coordinator, then `rtk git status` and
   require a clean worktree
9. JS module baseline update if a new imported module requires it

Then run the canonical isolated Playwright suite:

From working directory `tests/ui`:

```bash
rtk env CI=1 LLAMA_MONITOR_USE_RELEASE=1 LLAMA_MONITOR_TEST_PORT=17778 npm test
```

Run the full wizard screenshot groups sequentially after the final release build.
Run `rtk cargo test --test auth_routing`, the repository `/security-review` workflow/checklist, and
a documented platform-impact determination. If changed Rust/Cargo files contain platform gates or
touch tray/Cargo platform configuration, run the required Windows target check from `AGENTS.md`;
otherwise record why the change is platform-neutral HTML/CSS/JS/API behavior.

### Final acceptance matrix

For both llama.cpp and Rapid-MLX:

- Model -> Tune -> Launch in Guided.
- Model -> Tune -> Launch in Pro.
- Guided -> Pro -> Guided without state or payload drift.
- Fresh session defaults to Guided; current session restores Pro.
- Workload/model change recomputes defaults and provenance without overwriting user changes.
- Introspection loading, exact, degraded, unavailable, warning, and error states.
- Drawer reachability and count.
- Every Pro category, search, modified-only, reset.
- Save new preset, update preset, reopen in Preset Editor, launch from preset.
- Dark/light, desktop/narrow, reduced motion, keyboard-only, long identity/help.
- Auth failures, invalid payloads, backend/artifact mismatch, secret scrubbing.

### Final hard gate G13

All of the following are required:

- No required screenshot missing, skipped, stale, duplicated, mislabeled, or in the wrong group.
- Guided satisfies the archived llama walkthroughs and the backend-adaptive Rapid contract.
- Pro satisfies the archived llama and Rapid walkthroughs with seven stable categories.
- One canonical state/control/payload path remains authoritative.
- Every visible setting is reachable and traces through schema, validation, save/restore, launch or
  read-only effective status, review, tests, and docs.
- No model-property filename/repo/tag inference remains anywhere in the repository-wide Phase 10e
  audit scope.
- No false Rapid vision/MTP/KV claims.
- CSS modules have no unresolved duplicate/specificity/light/reduced-motion issues.
- README/reference/API docs match the verified behavior.
- Worktree contains only intended changes.
- Security review, auth routing, secret/XSS/path validation, and platform-impact receipts are closed.

Only after G13 may the Coordinator:

1. mark this plan complete;
2. reconcile Phase 10a–10e status in the execution companion;
3. correct the archived redesign's misleading `Done` header while preserving it as design history;
4. call Guided and Pro implemented and verified.

## 21. Required test additions

The implementation phases must leave these durable tests, not only screenshots:

- descriptor schema, unique IDs, backend applicability, category validity, and reachability;
- one canonical live control per setting;
- Guided card/drawer -> canonical state -> payload for both engines;
- Pro pane -> canonical state -> payload for both engines;
- Guided/Pro switching and session persistence;
- search, breadcrumb navigation, modified-only, and reset semantics;
- async effective/default/provenance updates;
- no stale listeners/duplicate DOM after repeated backend/view switching;
- narrow/category-drawer accessibility and keyboard navigation;
- llama and Rapid preset round trips through the Preset Editor;
- wizard-used endpoint auth/routing/contracts;
- streamed-HF GGUF and MLX introspection plus degraded failure;
- capture manifest/receipt/stale-output enforcement.

Screenshots validate pixels and layout. They do not replace these behavior and contract tests.

### Focused command contract by phase

Commands run from the repository root unless a row names `tests/ui` as the working directory. Test
files/scripts named here are required outputs of the owning phase when they do not exist yet.

| Phase | Minimum focused commands before its screenshot gate |
|---:|---|
| 1 | `rtk node tests/ui/capture/cli-manifest.mjs --strict`; `rtk node --test tests/ui/capture/capture-manifest.test.mjs` |
| 2 | `rtk node scripts/validate-spawn-wizard-css.mjs`; `rtk node tests/ui/capture/compare-sets.mjs --before <G1 receipt> --after <G2 receipt> --max-diff-pixels 0` |
| 3 | `rtk cargo test sampling_catalog`; `rtk cargo test model_defaults`; `rtk cargo test --test auth_routing`; focused HF/GGUF/MLX metadata route tests added by 3B/3C |
| 4 | `rtk node scripts/validate-wizard-groups.mjs`; from `tests/ui`: `rtk npx playwright test core/spawn-wizard-view-contract.spec.js core/preset-flow.spec.js` |
| 5 | from `tests/ui`: `rtk npx playwright test core/spawn-wizard-guided.spec.js core/spawn-wizard.spec.js`; `rtk node scripts/validate-wizard-groups.mjs` |
| 6 | from `tests/ui`: `rtk npx playwright test core/spawn-wizard-rapid-guided.spec.js core/phase7-presets.spec.js core/rapid-phase7-fields.spec.js`; focused Rapid config/preview Rust tests |
| 7 | from `tests/ui`: `rtk npx playwright test core/spawn-wizard-pro-shell.spec.js core/spawn-wizard-view-contract.spec.js` |
| 8 | from `tests/ui`: `rtk npx playwright test core/spawn-wizard-llama-pro.spec.js core/spawn-wizard.spec.js` |
| 9 | from `tests/ui`: `rtk npx playwright test core/spawn-wizard-rapid-pro.spec.js core/command-preview-ui.spec.js core/rapid-preset-throughput.spec.js`; focused Rapid config/preview Rust tests |
| 10 | `rtk cargo test --test auth_routing`; focused preset/launch/session Rust tests; from `tests/ui`: `rtk npx playwright test core/spawn-wizard-launch-contracts.spec.js core/preset-flow.spec.js core/phase7-command-preview.spec.js` |
| 11 | from `tests/ui`: `rtk npx playwright test core/spawn-wizard-accessibility.spec.js core/spawn-wizard-responsive.spec.js` |
| 12 | `rtk bash scripts/check-unused-screenshots.sh`; documentation link/route validator added by the phase |

Every static-JS phase also runs `rtk npm run validate-js`, `rtk npm run lint`, and
`rtk git diff --check`. Every API struct/auth/route phase also runs
`rtk cargo test --test auth_routing`.

## 22. Explicit non-goals

- Reopening the three-step versus six-step architecture.
- Redesigning unrelated app surfaces.
- Making the Spawn Wizard a fully server-generated form.
- Forcing identical controls across llama.cpp and Rapid-MLX.
- Re-enabling unqualified Rapid vision or speculative decoding.
- Adding decorative animation.
- Adding client-side estimator or argv formulas.
- Reworking every Preset Editor visual solely to resemble the wizard.
- Completing unrelated Phase 11–14 Rapid roadmap work.

### Phase 4 descriptor milestone (2026-08-09)

`controlsForView()` now returns validated semantic/mount descriptors for all 46 canonical controls (28 llama.cpp, 18 Rapid-MLX), including category, Guided placement, aliases, and search text. Schema-2 control-contract evidence and `validate-wizard-groups.mjs` enforce descriptor uniqueness/completeness. The G4 Playwright capture now also asserts canonical llama payload deep-equality after Rapid-MLX switch-away/switch-back and verifies the drawer edit survives `buildPresetPayload()`. The remaining Phase 4 gate is the full cross-view state/update contract.

<a id="terra-coordinator-handoff"></a>

## 23. Terra handoff prompt

Use this prompt for the Coordinator, replacing `<N>` with the next open phase:

> Act as Coordinator for Phase `<N>` of
> `docs/plans/20260808-spawn_wizard_guided_pro_completion.md`. Read the plan's authority,
> frozen decisions, execution protocol, and exact Phase `<N>` section. Do not trust prior completion
> claims, stale screenshots, memory, CSS scaffolding, or commit hashes as proof. Inspect the current
> worktree and preserve unrelated changes. Brief one bounded Builder for only this phase, then a fresh
> Verifier. For any static change, build release before running the named screenshot gate, run capture
> scenarios sequentially on an explicit port, and visually inspect every required artifact. Do not
> proceed to Phase `<N+1>` until the phase hard gate is evidenced in source, behavior, tests, and fresh
> capture receipts. Do not commit, push, or update old Phase 10 completion status without Coordinator
> authority.

### Phase 4 final boundary (2026-08-09)

Phase 4 is complete in the current tree: canonical setting state and update pipeline, backend-safe Guided relocation, five workload intents, and legacy profile retirement are implemented and release-built. G4 and tier captures pass at original resolution; focused Playwright hardware-control checks pass. The next work is Phase 7 Pro, not additional Phase 4 scope.

### Phase 7A Pro shell milestone (2026-08-09)

Implemented and release-built the shared Pro presentation over canonical controls: seven-category rail, active-section navigation, search filter, modified-only filter, resolved-default reset, Guided/Pro relocation without duplicate inputs, and responsive dark/light/narrow styling. Added focused Playwright coverage for switching, canonical ownership, filtering, modified state, reset, and return to Guided. Replaced the stale Pro-not-implemented capture with a strict manifest-backed scenario producing shell, rail, search, modified-only, light/reset, and narrow evidence. Fresh outside-sandbox capture completed at allowed realistic viewports; visual inspection found the Pro surface reachable and the category pane usable. Phase 7A remains implementation-complete pending the broader Rapid-MLX Pro parity work in Phase 9 and final verification gates.

## 24. Completion definition

The work is complete when a novice can choose a model and make only the few backend-relevant Guided
decisions, a power user can find and edit every applicable setting quickly in Pro, switching views
never changes the configuration, both backends tell the truth about what is requested and effective,
the launch/preset/API paths agree, and a fresh harness run produces an auditable set of realistic
screenshots that matches the documentation.
### Phase 8 milestone update (2026-08-09)

Phase 8 llama.cpp Pro panes are complete. The Pro surface now relocates sampling/reasoning, structured-output, network, extra-argument, MTP-depth, decision-card, and compatibility controls into one canonical seven-category surface. Decision cards carry category/search/dirty metadata, and reset dispatches canonical controls. Fresh release-built G8 evidence is recorded in `docs/screenshots/artifacts/wizard-llamacpp/spawn-wizard-pro-baseline--receipt.json` with model, memory, performance, generation, tools, network, advanced, agentic q4 warning, roleplay q4, modified/reset, search, and narrow captures. Focused Pro Playwright passes; clippy, 2272 Rust tests, JS validation/lint, diff check, and release build pass.

### Phase 9 milestone update (2026-08-09)

Rapid-MLX Pro parity is complete in the current tree. The seven-category Pro surface now exposes backend-native Model, Memory & context, Performance, Generation & reasoning, Tools & conversation formatting, Network & observability, and Advanced panes; shared access controls remain available to both loaders, while compatibility copy explicitly identifies MLX-native artifacts and avoids llama.cpp-only quant/mmproj claims. Effective-state and effective-KV evidence, retained-cache editing, parser/profile evidence, companion trust/unavailable gating, cache search, modified-only filtering, and narrow layout are covered by the fresh receipt `docs/screenshots/artifacts/wizard-rapidmlx/spawn-wizard-rapid-pro-baseline--receipt.json` and its 13 original-resolution captures. Focused release-built Rapid-MLX Pro Playwright verification passes; JS validation/lint and diff checks pass. This phase is ready for the human acceptance stop.
