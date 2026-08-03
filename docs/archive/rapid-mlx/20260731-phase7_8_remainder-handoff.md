# Phase 7 / 8 remainder — handoff

**Status: all sections closed as of 2026-08-03.** §7's resume checklist items 1-6 are done or
intentionally parked (MTP requalification, item 6, awaits an upstream rapid-mlx release — not a
gap). The Deferred addendum's three surfaces are all done: Spawn Wizard's 18-step decomposition
(shell now 3,436 lines + 18 feature modules), the MLX-native Preset Editor, and the Dashboard
premium visual system (archived to `docs/archive/rapid-mlx/20260802-dashboard-premium-visual-system.md`).
`buildRapidMlxConfig` adapter extraction is done, the `model-library`→`model-discovery` capture
rename is done, and `rapid-mlx-live`'s dashboard-after-chat capture and stop-check fixes are both
in place. The mandatory pre-PR gate (clippy, cargo test 2240 passed, validate-js, lint, `git diff
--check`, release build, `cargo fmt`) was run in full on 2026-08-03 and is clean (one stray
trailing-blank-line-at-EOF in `spawn-wizard.js` found and fixed by `git diff --check`). This
document is retained for its historical record and closed-decisions (§4); nothing here still
blocks a checkpoint commit.

**Written:** 2026-07-31, by Coordinator (Opus), at the close of the reconciliation campaign.
**Reconciled:** 2026-07-31 against the current source tree and installed `rapid-mlx 0.11.1` after
handoff review. The reconciliation clarifies prefill policy, estimator reachability, MTP product
limits, speculative-config shape, and reasoning/no-thinking semantics. Those clarifications are
authoritative over earlier wording in this file and archived plans.
**Updated:** 2026-07-31 after the release-built UI/backend checkpoint. Historical “next steps” and
closed gaps below have been reconciled with the current worktree; §7 is the authoritative resume
checklist.
**Supersedes:** `docs/plans/20260730-phase7_8_gap_register.md`, which this document consumes in
full. That file is deleted; its still-open items are reproduced here, its closed items are
summarised, and the register's own findings-about-findings are preserved because they are the
most reusable thing in it.

**Audience:** a frontier model picking up the hard remainder. The easy, well-specified work has
been done. What is left is disproportionately the work that requires judgement about *what the
product should be*, not just what the code does.

---

## 0. Read this first: the governing defect class

Every defect in this campaign — without exception — had the same shape:

> Code that was built, unit-tested, marked verified in a ledger, and **unreachable from the
> running binary**, or diverging from the real runtime path.

Concrete instances found: a settings catalog no frontend fetches; a command-preview endpoint that
returned `BAD_REQUEST` for every reachable caller; nine preset controls hidden by a CSS allowlist;
seven more hidden by a nav allowlist; twelve config fields gated on CLI flags that exist in no
runtime; a seven-role user-editable catalog with no HTTP route; a lineage row on the model cards
reading three fields that had never existed. In each case the tests were green, and had been green
the entire time the defect was live.

**Therefore, the working rule for this codebase:**

1. **A passing test is not evidence that a feature is reachable.** A unit test that supplies its
   own input proves the rule and proves nothing about the wiring. `d25_multi_slot_conflicts_with_mtp`
   passed for weeks while no caller ever supplied a slot count of 2.
2. **Verify against a running binary.** Start a throwaway server on a spare port, `curl` the route,
   read the file it wrote. Section 2 has the recipe. Every "Resolved" claim in this document was
   confirmed this way, not by test output.
3. **Negative-control every new test.** Stash the fix, confirm the test fails, and confirm it fails
   *for the intended reason*. Two tests in this campaign would otherwise have passed vacuously.
4. **When a probe changes two things at once, it proves nothing about either.** A wrong finding was
   published in this campaign because a payload carried both a `turboquant_mode` and an unsupported
   `speculative_policy`; the latter hard-errored before the diff was built, and the empty diff was
   misattributed to the former. Retracted in commit `7dfa1ab`.
5. **Distinguish a product defect from a test defect, explicitly, in writing.** Three of the causes
   of the "flaky suite" were tests fabricating states the app never produces. Fixing those is not
   a workaround — but calling them product bugs would have been wrong, and calling the fourth one
   (a real uncancelled timer) a test issue would have shipped a bug.

---

## 1. Current state

### Backend
`cargo test --lib`: **1081 passed, 13 ignored, 0 failed.** (after §3.8 migration; see checkpoint
below)
`cargo check --lib --tests`: clean, no warnings.
`cargo fmt --all -- --check`: **the repo baseline is not rustfmt-clean.** Four files differ that
nobody has touched (`src/hf/mod.rs`, `src/inference/rapid_mlx/command.rs`,
`src/inference/rapid_mlx/settings.rs`, `src/llama/vram_estimator/workload_scenarios.rs`). Do not
run `cargo fmt --all` as a drive-by; it widens every diff. Format only the files you edit.

### Frontend
`npm run validate-js`: clean. `npm run lint`: clean.
Release-built Playwright full suite: **261 passed, 5 skipped, 0 failed** (7.1 min, serial).

### 2026-07-31 Codex execution checkpoint

This checkpoint records only changes verified in the current worktree; it is newer than the
campaign-commit table below and is the starting point for the next context window.

- Rapid estimation now carries explicit `prefill_step_size` end to end and never aliases it to
  llama.cpp `ubatch_size`. The product default is 512 and the ceiling is 2048.
- Rapid launch always emits `--reasoning`; explicit thinking opt-out emits the orthogonal
  `--no-thinking`. Focused reasoning, command, runtime, and estimator tests passed.
- Product speculative configuration is now a typed MTP-only `RapidMlxSpeculativeConfig` using the
  exact vLLM JSON keys. Unsupported runtimes omit it without blanking unrelated preview output.
  Estimator MTP depth is server-derived; unknown external-sidecar bytes remain explicitly unknown.
- Wizard and Preset Editor expose that typed contract with MTP as the only method, embedded or
  external source, token count, and automatic-K control. It defaults off and states that Rapid
  0.11.1 engages only for greedy unconstrained requests. No raw JSON field is exposed.
- The shared two-level evidence drawer exists and is wired to Wizard memory, preset memory, and
  Rapid command-preview effective policy. It is singleton, keyboard-dismissible, restores focus,
  traps focus, and has light/narrow/reduced-motion styling. The obsolete preset-only teaching
  container was removed.
- Authoritative capture artifacts were generated and visually inspected:
  `evidence-drawer-dark.png`, `evidence-drawer-expanded-dark.png`,
  `evidence-drawer-expanded-light.png`, and
  `evidence-drawer-narrow-reduced-motion.png` under `docs/screenshots/artifacts/`.
- HF lineage now preserves the Hub commit through inventory and selection. Playwright covers
  pinned, unpinned, and local-only rendering without inventing `main`.
- HF identity no longer accepts a client-controlled config directory. Qualification/identity have
  bounded concurrency and whole-operation timeouts. MLX introspection is restricted to configured
  model roots. Focused API/auth/path tests and clippy passed.
- Playwright tags now have selectable `test:in-memory`, `test:fake-data`, and `test:runtime` lanes.
  The tag-cloud re-expand path has a duplicate-binding guard and stronger regression assertions.
- Current focused release-built Playwright receipt: 12 passed, 3 runtime-dependent skips across
  the evidence drawer, lineage inventory, command preview, and Phase 7 preset contracts.
- Current Rust receipt after all checkpoint changes: `cargo test --lib` **1081 passed, 13 ignored,
  0 failed**; `cargo clippy -- -D warnings` clean; release build clean. (Updated 2026-08-01 after
  legacy quant-style migration.)
- Model Manager now has a Sources tab with server-provided roles and list/add/edit/remove/reset
  flows. Bundled edits use full-catalog PUT and preserve bundled flags/preferences; destructive
  reset/remove actions require confirmation and application-level `ok` is checked.
- All formerly orphaned evidence endpoints now have on-demand consumers: Rapid Runtime Support
  consumes `/api/rapid-mlx/settings`; HF discovery consumes qualification and identity; local MLX
  cards consume restricted introspection. MTP admission and runtime provenance render in the drawer.
- Release-built focused UI receipts after these additions: 23 passed with 3 intentionally
  runtime-dependent skips, followed by 15/15 preset reachability/round-trip tests and an explicit
  typed-MTP-row visibility regression.
- Capture scenarios `community-sources`, `rapid-preset`, and `spawn-wizard-engines` cover the new
  list/editor and typed MTP states in dark, light, and narrow layouts. Visual inspection caught and
  fixed the Rapid preset CSS allowlist before this checkpoint was recorded.

Full release-built Playwright receipt after this checkpoint: **262 passed, 4 skipped, 0 failed** in
7.5 minutes. The previously intermittent tag-cloud re-expand case passed in this run.

At that checkpoint, immutable external-companion preflight/trust consent and the external
cache/benchmark audits in §3.11 were still open. The companion boundary, frequency-penalty UI,
legacy quant-style migration, and bounded cache audits are now closed by the later checkpoints
below; the remaining current work is enumerated in §7.

### Campaign commits, newest first
| Commit | What |
| --- | --- |
| `767059d` | rustfmt the Rapid runtime route module (formatting only) |
| `e5dba35` | model lineage/provenance pipeline |
| `258e017` | community source catalog endpoints + role badges |
| `52d47d8` | modal close-timer leak; three classes of test defect |
| `02f9560` | decided the four capabilities left unexposed |
| `e9b412f` | made every Rapid control reachable *and clearable* |
| `03d272e` | deleted the dead `concurrency_policy` estimator input |
| `6819265` | exposed the three real cache/batch fields; fixed Auto-no-op |
| `9527e35` | deleted 12 config fields gated on nonexistent flags |
| `ac31388` | stopped throughput tuning failing the launch outright |
| `7948ff3` | decoupled speculative decoding from all other diagnostics |
| `ef64903` / `12a9739` | command preview: real consumer, callable backend |

---

## 2. How to work in this repo

### Running a throwaway server for live verification
```bash
SP=/tmp/scratch                       # anywhere outside the repo
mkdir -p $SP/home/.config/llama-monitor $SP/models
cargo build
HOME=$SP/home ./target/debug/llama-monitor --port 17779 --models-dir $SP/models > $SP/server.log 2>&1 &
sleep 4
TOKEN=$(cat $SP/home/.config/llama-monitor/api-token)   # NOT parsed from the log
curl -s -H "Authorization: Bearer $TOKEN" http://127.0.0.1:17779/api/models
# ... and when done:
pkill -f "llama-monitor --port 17779"
```
The token is written to `$HOME/.config/llama-monitor/api-token`; the log only says it generated
one. **Always kill the server when finished** — the repo owner runs no instances of their own, so
any stray `llama-monitor` process is one you left behind.

### Port 7778 is sacred
`tests/ui/playwright.config.js` carries `IMPORTANT (NEVER KILL PORT 7778)`. Anything on 7778 with
a `/var/folders/.../llama-monitor-test-*` config dir is the Playwright harness. If a run dies and
orphans it, identify it precisely and reap that PID — never blanket-kill by port.

### Running the UI suite
```bash
cd tests/ui && rtk proxy npx playwright test --reporter=line > /tmp/run.txt 2>&1
```
**`rtk proxy` is mandatory here.** The rtk hook intercepts bare `npx playwright test` and returns a
fabricated `PASS (0) FAIL (0)` in 13 ms — it does not run anything. Other rtk quirks: it garbles
multi-line output and compresses grep results. Use `rtk proxy sh -c "..."`, or read output files
with Python and strip ANSI via `re.sub(r'\x1b\[[0-9;]*m','',s)`.

The config sets `fullyParallel: false` and `workers: 1` **only under CI**. Running locally without
those is parallel with `retries: 0`, which manufactures failures the suite is configured never to
have. `a845f7f` serialized local runs to match; don't undo it.

### Warp route registration
Two traps, both hit in this campaign:
- Body deserialization rejects with **400 before** the handler's auth check runs. A route smoke
  test asserting 401 needs a body that actually deserializes.
- **Longest paths must be registered first.** A bare `community-sources` filter registered before
  `community-sources/entry` rejects the sub-path and ends the chain.

---

## 3. Outstanding work

Ordered by my judgement of value. Each item states what is *known* versus what needs deciding.

### Reconciled product decisions — do not leave these implicit

- **Rapid prefill estimation gets its own `prefill_step_size` input.** Do not overload llama.cpp's
  `ubatch_size`: the controls are analogous but their qualified recommendations can diverge. Rapid
  launch, estimate, Wizard, Editor, preview, and receipts must all carry the same explicit value.
  Keep 512 as the text default; vision-capable paths require at least 1024, with 1536 and 2048 as
  the offered larger options. This is a product control, not a claim that current Rapid vision
  support is usable for Qwen3.5/Qwen3.6.
- **The teaching surface is decided:** build one shared, on-demand evidence drawer opened from
  compact inline “Why?”/“Explain” links. Its first layer is novice-readable; an expandable second
  layer contains measured evidence, requested-vs-effective settings, fallthroughs, provenance, and
  remediation for power users. Do not restore the deleted workload step.
- **Rapid MTP/speculative decoding is not recommended for normal users today.** The qualified path
  is effectively greedy-only and rejects or falls through when real coding harnesses install
  sampling, reasoning, logits processors, or constrained tool decoding. That excludes the large
  majority of home-LLM use, including typical OpenCode-style clients. Build truthful plumbing and
  a future requalification path; do not default-enable it or advertise current benchmark
  acceptance as a normal-user speedup.
- **Speculative configuration is typed vLLM-style JSON, initially MTP-only.** Use a discriminated
  schema carrying `method`, optional sidecar `model`, `num_speculative_tokens`, and
  `disable_auto_k`. Design the schema to admit later methods, but do not expose unqualified
  DFlash/DDTree/suffix modes in the first release and do not ship a raw free-text JSON field.
- **Always emit Rapid's `--reasoning` quality profile.** In 0.11.1 this requests/pins the runtime's
  reasoning KV policy to int8; it does not mean “show thinking.” Keep llama-monitor's separately
  qualified requested-vs-effective KV reporting intact. `--no-thinking` is orthogonal: it controls
  reasoning parser/chat-template behavior and may be added alongside `--reasoning` only when the
  user explicitly disables thinking. Never model the two flags as opposites.
- **Tests must assert the actual field or argv they claim to cover and have a negative control.**
  Setting a JS state property, sending a serde-ignored key, or checking only that *some* argv was
  returned is not coverage.

---

### 3.1 — Teaching and troubleshooting content (RELEASE-GATING)

**Status:** built and capture-verified, including MTP admission and runtime provenance consumers.

Commits `712c261` and `58cfa42` deleted the step-3 workload picker and everything rendered inside
it. The ledger rows for that work claimed "verified" and described a UI that no longer existed.
Deleted along with it:

- **7B3 roleplay teaching panel** — went with `WORKLOAD_PROFILES`, plus three tests. No
  `roleplay-teaching` identifier survives anywhere in `static/` or `tests/ui/`.
- **7B4 MTP/concurrency teaching** — four D25 cards and five tests went with the old step. The
  orphan preset-only container has now been removed; the shared drawer is its replacement.
- **7.5C tool-research captures** — no `tool_research` identifier exists in `capture.mjs`.

**Resolution:** teaching and troubleshooting now use the shared drawer across Wizard, Preset
Editor, command preview, HF discovery, local MLX inventory, and Runtime Support. The deletion of
the unreachable panels remains correct.

**Decided surface:** one shared, on-demand evidence drawer, opened from compact inline links rather
than a new wizard step. It must serve two reading levels: a plain-language recommendation and
consequence first, then expandable power-user evidence. The data model and renderer are
cross-backend (llama.cpp and Rapid-MLX), and every claim identifies whether it is measured,
runtime-reported, derived, degraded by missing metadata, or merely a future qualification target.

The orphan `#pe-mtp-concurrency-teaching` container was removed.

---

### 3.2 — `mtp_admission` reaches the product

**Status:** resolved in the current worktree. Typed request/backend derivation, Wizard/Preset
controls, admission fallthroughs, and runtime qualification provenance are wired.

`/api/vram-estimate` derives admission from the typed product `speculative_config`; it no longer
trusts caller-authored estimator depth or memory. Wizard and Preset requests carry the typed config,
and the evidence drawer renders exact fallthroughs and warnings plus runtime qualification metadata.

The current verdict must be unmistakable in the UI: Rapid MTP is effectively a qualification tool,
not a recommendation for normal interactive use. Its greedy/temperature-0 acceptance result is not
representative of OpenCode-style requests that use sampling, reasoning, tool grammar, or other
logits processors. Render the exact admission fallthroughs and warnings in the shared teaching
drawer; do not reduce them to an “eligible” badge.

The product owner's current estimate is that this excludes **95%+ of home-LLM users**. Treat that
as product-scope guidance, not a measured population statistic; the measured evidence is the
request-shape fallthrough recorded by the qualification harness.

The future-build validation harness is the authority for changing that verdict. A later runtime is
not considered fixed until the harness proves sampled coding/tool workloads dispatch speculative
decoding without dropping their request semantics.

---

### 3.3 — Speculative decoding as a product feature (`--speculative-config`)

**Status:** typed backend/argv/estimator and default-off Wizard/Preset controls are built and
capture-verified. Companion lifecycle work is closed for the current runtime boundary: immutable
local sidecars are inventoried and estimated, while mutable remote-HF companion launches fail
closed because Rapid-MLX 0.11.1 cannot request a pinned revision. Remote preflight/pin/trust data
remains diagnostic only.

The capability is **proven**, but only as a qualification path: the benchmark suite builds the flag
correctly (`rapid-mlx-benchmark-suite.mjs:726`) and MTP measures **59–61% acceptance on
Qwen3.6-27B** under the constrained greedy lane. The product must not turn that ceiling into a
normal-user speed claim.

Already delivered:

- `RapidMlxSpeculativeConfig` is a typed, MTP-only schema serialized as exact vLLM-style JSON
  (`method`, optional `model`, `num_speculative_tokens`, `disable_auto_k`).
- Wizard and Preset Editor controls are default-off, support embedded or external sources, clamp
  speculative tokens to 1–8, and explain that Rapid 0.11.1 only engages for greedy unconstrained
  requests. There is no raw JSON escape hatch.
- Command preview and estimation preserve unrelated output when the runtime cannot admit the
  requested speculative setting, and the evidence drawer shows requested/effective policy and
  fallthrough reasons.

What remains is lifecycle and safety, not another UI wiring pass:

- **External companion preflight:** resolve an external sidecar immutably, verify its provenance and
  compatibility before launch, and make its download/memory/lifecycle state explicit. Do not accept
  a mutable branch or silently stage a different artifact.
- **Trust consent:** expose revision-scoped consent for repositories that require
   `trust_remote_code`; launch must remain fail-closed on missing or mismatched consent.
  **2026-08-01: Partially done.** Frontend modal trust-consent flow built for both Preset Editor
  and Spawn Wizard. The modal appears when MTP preflight reports `trustRemoteCodeRequired: true`,
  explains the risk, and blocks launch on decline. Consent is sent as `trustRemoteCode: true` in
  the request payload so the backend `validate_trust_consent` gate passes. Still incomplete: no
  revision-scoped persistence, no caching of resolved pins, no re-check on upstream changes, and
  no explicit lifecycle/memory-status surfacing.
- **Hybrid K rules:** preserve server-derived architecture depth and the observed K=1 clamp for
  Qwen3.6-27B; never infer this from the known-wrong `aliases.json` metadata.

**Measurement caveat that must reach the UI:** the published acceptance numbers are a **ceiling**.
They were taken at temperature 0, which is exact-match acceptance rather than rejection sampling,
and reasoning/sampling/tool constraints are additional axes. Under the currently qualified runtime,
the overwhelming majority of normal home-LLM coding clients do not satisfy the dispatch shape. Do
not present the numbers as expected user-facing speedups, default-enable MTP, or silently discard a
client's sampling parameters to make it engage.

**Settled first-release scope:** implement an extensible typed discriminated schema, but expose only
the MTP method initially. Rapid 0.11.1 also parses DFlash, DDTree, and suffix methods; parser support
is not qualification, so those remain hidden until separately measured and admitted.

---

### 3.4 — Evidence endpoints now have product consumers

**Resolved in the current worktree.** Each formerly orphaned surface is reached on demand rather
than adding expensive network or filesystem work to ordinary rendering.

| Surface | Frontend consumers |
| --- | --- |
| `/api/rapid-mlx/settings` (settings catalog) | Rapid Runtime modal, **Runtime support** evidence |
| `mtp_admission` (see 3.2) | Wizard/Preset estimator evidence drawer |
| `/api/hf/qualify` — registered, revision-pinned | HF discovery qualification/lineage evidence |
| `/api/hf/identity` — registered | HF discovery qualification/lineage evidence |
| `/api/models/mlx-introspect` — registered | Local MLX model-card **Explain** action |
| `/api/rapid-mlx/command-preview` | Wizard/Preset command preview and effective-policy drawer |

The command-preview response's `requested_vs_effective` and `effective_policy` fields are consumed
by that same drawer; the community source catalog is consumed by Model Manager's Sources tab.

`/api/hf/qualify` is revision-pinned, and downloaded models now carry a real Hub commit (see 3.7),
so the precondition that previously made it uncallable is gone.

---

### 3.5 — Config fields and safety-control disposition

The product-facing controls below are resolved for the current runtime boundary; the two runtime
path fields remain internal by decision.

| Field | Disposition |
| --- | --- |
| `executable_path`, `managed_runtime_path` | Runtime-resolution internals. **Keep hidden** — settled. |
| `trust_remote_code_consent` | **Resolved for the current boundary:** revision-scoped remote preflight/pin/trust UX exists, but remote-HF companion launch remains fail-closed because Rapid-MLX 0.11.1 cannot load the resolved immutable revision. Immutable local sidecars remain supported. |
| `auto_tool_choice` | **Resolved:** Wizard/Preset control and serialization built. |
| `no_thinking` | **Resolved:** explicit thinking opt-out serializes this while `--reasoning` remains always-on. |
| `default_frequency_penalty` | **Resolved:** backend rejects launch outside -2.0..2.0; Preset Editor exposes `modal-rapid-frequency-penalty` in the Rapid Sampling Mode row, CSS allowlist and save/load/reset paths updated, visibility spec passed. |

**Resolved launch contract:**

- normal/default launch emits `--reasoning` and not `--no-thinking`;
- explicit thinking opt-out emits `--reasoning --no-thinking`;
- no supported product state silently omits `--reasoning`;
- `enable_thinking` request defaults and `reasoning_parser` selection remain separate concerns.

---

### 3.6 — Community source catalog editing UI

**Resolved in the current worktree.** Model Manager has a Sources tab with server-provided roles,
safe DOM rendering, add/edit/remove/reset, explicit destructive confirmations, and dark/light/narrow
coverage. Editing uses full-catalog PUT so bundled records and preferences are preserved; POST is
reserved for new user entries. The legacy quant-style facet remains part of §3.8 rather than being
guessed from roles.

The role vocabulary is served by `GET /api/hf/community-sources` as a `roles` array derived from
`CommunitySourceRole::ALL`; the editor renders it directly and does not retype labels in JS.

---

### 3.7 — Lineage: landed and tested

`e5dba35` built the provenance pipeline: `.llama-monitor-provenance.json` per directory, written
at download completion with the repo, the in-repo path, and the real commit from Hugging Face's
`X-Repo-Commit` header. Live-verified against real HF — commit captured, `save_as` divergence
recorded, merge-not-clobber confirmed, delete-forgets confirmed.

**Two design invariants to preserve if you touch this:**
- `pinned` is an explicit field, not an inference from `revision` being present. An unpinned
  download came from whatever the branch pointed at; the UI must not render it as reproducible.
- `directory_origin` returns **nothing** when a directory's files came from two repos. Picking one
  would put a confident, wrong lineage on the card.

**Resolved in the current worktree:** Playwright pins lineage-row rendering for pinned, unpinned,
and local-only models. `SimpleModelInfo.revision` is populated from the HF API commit and the
selection payload preserves it; tests reject an invented `main` revision. There is no remaining
lineage-specific Playwright gap. External companion provenance is resolved for immutable local
sidecars; mutable remote-HF companion launch remains fail-closed as recorded in §7.

---

### 3.8 — Legacy quantizer surface is still the live one

**Resolved 2026-08-01.** The quantizer quick-pick endpoint is now a derived view over the typed
community-source catalog, not a separate KnownQuantizer list.

**What changed:**
- `GET /api/hf/quantizers` no longer reads `hf-quantizers.json` or `known_gguf_quantizers()`. It
  loads the community source catalog via `load_catalog()`, calls
  `to_quantizers(&catalog)`, and returns `{ ok, quantizers, is_custom: false }`.
- `to_quantizers()` (new in `community_source_catalog.rs`) filters the catalog for entries whose
  role is quantizer-related (GgufQuantizer, MlxConverter, or OriginalAuthor/Curator with
  GgufQuantizer in `also_known_for`) and derives a `quant_style` per entry:
  - The derivation is **username-aware, not role-based:** e.g., `unsloth` → `"ud"`,
    `mradermacher` → `"imatrix"`, `mlx-community`/`nightmedia` → `"mlx"`, others → `"standard"`.
  - For unknown/custom entries, it inspects `note`/`description` for "imatrix"/"dynamic" hints.
- `PUT /api/hf/quantizers []` (reset) now resets the catalog via `reset_catalog()` rather than
  only deleting `hf-quantizers.json`, so the "empty body = defaults" behavior remains safe.
- `PUT /api/hf/quantizers [...]` (non-empty) applies the legacy replacement-list contract to the
  catalog with the correct role mapping (e.g., `unsloth` → OriginalAuthor, `nightmedia` →
  MlxConverter). Omitted bundled entries are hidden from quick-picks without losing bundled flags
  or role evidence; omitted user-added entries are removed. Explicit `quant_style` overrides round
  trip through catalog preferences instead of being discarded.
- `nightmedia` added as a bundled `MlxConverter` entry.
- The migration path (`migrate_from_user_quantizers()`) now maps `mlx-community`,
  `lmstudio-community`, `nightmedia` → `MlxConverter` instead of `GgufQuantizer`.
- Removed dead code: `KnownQuantizer`, `known_gguf_quantizers()`,
  `save_user_quantizers()`, related test. Kept `UserQuantizer` and
  `load_user_quantizers()` for backward compat (used by the migration path).
- `/api/hf/community-picks` left untouched; its schema is unrelated to quantizer roles.

**Verified live (not just tests):**
- `GET /api/hf/quantizers`: 11 quantizers returned, all with correct `quant_style`, `is_custom: false`.
- `PUT /api/hf/quantizers []` → `{"ok": true, "reset": true}`; subsequent GET still returns 11.
- `GET /api/hf/community-sources`: 11 bundled entries (including `nightmedia`) with typed roles.
- `cargo test --lib` 1079/0/13; `cargo clippy -- -D warnings` clean; release build clean.

**Design notes (do not reopen):**
- `quant_style` is UX metadata (how a source quantizes), NOT the source role.
- Never infer `quant_style` purely from `CommunitySourceRole` — the shim uses per-username
  knowledge and metadata hints, not the role alone.
- The legacy `hf-quantizers.json` is now import-only (migration path) but the endpoint contract
  is identical so no frontend changes are needed.

---

### 3.9 — Playwright tags select nothing

**Resolved in the current worktree:** package scripts select in-memory, fake-data, and runtime
lanes by tag; config comments describe the executable contract rather than decorative labels.

---

### 3.10 — Smaller recorded items

- `/api/vram-estimate` silently ignores an unknown `workload_scenario` rather than rejecting it.
- No use-case card maps to a multi-slot scenario, so the D25 multi-slot MTP warning — reachable
  since the 7B4 fix — still cannot be triggered from the UI.
- `prefix_cache_policy` is catalog-only; the live system uses three raw config fields instead.
  Deliberate and documented at `rapid_mlx_runtime.rs:2292` with a `CATALOG_ONLY` guard.
- `ValidationContext` populates `capabilities` and `workload_scenario` that every rule ignores.
  Deliberate, guarded by a drift test. **Recorded so it is not "discovered" again.**
- `prefill_step_size` intentionally defaults to 512 and is clamped to 2048. The explicit Rapid
  estimator input now stays separate from llama.cpp `ubatch_size`; Wizard, Preset, preview, launch,
  and receipts must retain that distinction. The UI offers 1024/1536/2048 for vision fallback, but
  vision itself remains outside the qualified Qwen3.5/Qwen3.6 product path.
- The Phase 7 preset/browser and command-preview tests now assert reachable fields/argv and include
  negative controls. Runtime-only preview coverage remains intentionally skipped unless a Rapid
  runtime is available; the release-built in-memory/fake-data contracts run on every suite.
- **Resolved 2026-07-31:** checked `capture.mjs` for lingering references to the deleted
  workload-picker scenarios (`roleplay-teaching`, `tool_research`, `WORKLOAD_PROFILES`, etc.) —
  none exist; that cleanup already happened. Two stale comments still claimed a nonexistent
  `HF_SCOPE.AUTO`/`HF_SORT.AUTO` and an "Auto/GGUF/MLX/All" four-way scope set; both corrected to
  the real three-way `GGUF/MLX/All` set (`hf-browse.js`, `capture.mjs`).
- **Resolved 2026-07-31:** `HF_SORT_LABELS` / `HF_SCOPE_LABELS` were exported and unused — the scope
  buttons hardcode their own labels and the sort dropdown hardcodes its own too. Both dead exports
  deleted from `hf-browse.js`; `validate-js`, `eslint`, and `node --check` on the touched files all
  clean, and nothing else in the tree imported them (grepped first).

---

### 3.11 — Carried from earlier phases (not code)

**2026-08-01 cache-entry checkpoint — parked on Gemma template fidelity, not a
cache verdict.** The bounded 32k Gemma 1-entry positive control initially had
an invalid benchmark prompt (it referred to an unspecified branch question).
After repairing that prompt, the 2,048-token diagnostic run produced 4/5
marker recall on cold/repeat, but each fork exhausted its entire budget in
reasoning and emitted no final answer. A watched production-representative
retry used the production ceilings (text prefill 512, 32k request/server,
8,192 reasoning tokens), but a harness audit found the stronger blocker:
`reasoning_content` and final `content` were concatenated. That let hidden
reasoning satisfy marker recall and then replayed the reasoning trace as the
assistant turn before each fork. The generator itself does inject all five
markers exactly once; a runtime invariant and self-test now enforce that.
Reasoning and final output are separated, reasoning-only exhaustion fails,
and only final content is scored/replayed. The old synthetic `PRESERVED`
branch prompt is replaced by alternating self-contained SAFE/UNSAFE code-diff
reviews with exact final-answer gates. Matrix quality cells require at least
4/5 marker recall. The proposed watched e1 rerun was completed; its cache result and
corrected control design are recorded immediately below. No Rapid-MLX child was
left behind. Do not read any earlier attempt as a `--hybrid-cache-entries`
result or recommendation. Qwen3.6 still needs a separate stock-template
baseline — ideally the 35B-A3B model actually used in production (otherwise the
already-qualified 27B) — because a Gemma result is non-transferable.

**2026-08-01 e1 cache-control result — valid negative control, invalid positive
control.** The repaired Gemma cell completed every fidelity gate, but its exact
repeat had the same request hash and 33,116 prompt tokens while recording zero
prefix-cache hits and zero tokens saved; repeat TTFT was 15.30 s versus 13.22 s
cold. This is not a request-shape or launch defect. The exact argv enabled the
radix prefix cache with 8 GiB, INT8 KV, prefill 512, and
`--hybrid-cache-entries 1`. Runtime counters and logs prove the lifecycle: cold
stored two non-trimmable snapshots (prompt-only and prompt+output), inserted two
radix entries, immediately evicted one, and ended with one entry. Repeat then
missed, inserted two, evicted two, and again ended with one. Rapid-MLX 0.11.1
cannot trim the remaining prompt+output supersequence, so e1 cannot reuse even
an identical prompt by design. The suite now treats e1 as a metric-gated
negative control, e2 as the minimum exact-repeat positive control, and e4 as
the minimum branch positive control. Direct hit/miss/tokens-saved/eviction
gates replace TTFT inference. Next run only the 32k e2 cell; it must record at
least one repeat hit and 28,000 saved prompt tokens before any e4 or broad
sweep work resumes.

**2026-08-02 e2 preflight correction:** the first e2 attempt never reached its
repeat request because Gemma's sampled cold marker recall varied from the prior
4/5 to 3/5. That is a model/template quality observation, not a cache-mechanism
result. A0/e1 and A1/e2 now record marker recall without using one temperature-1
sample as a hard gate; their direct cache metrics decide the negative/positive
mechanism controls. A2/e4 and the broader matrix retain fidelity gates. The
Qwen3.6 35B-A3B checkpoint also reports the implementation model type
`qwen3_5_moe`; vendor-profile matching now includes text/MoE wrapper variants,
preventing a silent greedy, thinking-off fallback. Its dry plan resolves the
required 1.0/0.95/20 sampling policy with thinking enabled.

**2026-08-02 corrected e2 result — cache mechanics pass, benefit not yet
proven.** The run completed after decoupling sampled marker recall from the
mechanism gate. Cold recorded one miss and no eviction; the identical repeat
recorded one hit, zero misses, and all 33,116 prompt tokens saved. This proves
two retained snapshots are sufficient for exact-repeat reuse. It does not prove
a latency benefit: observed TTFT was 13.40 s cold and 15.35 s repeat. Proceed
only to the e4 branch-positive control; require its direct reuse counters and
exact SAFE/UNSAFE final verdicts before any matrix or recommendation. A2/e4
therefore uses its exact branch verdicts as the quality gate while retaining
marker recall as diagnostic data.

**2026-08-02 corrected e4 result — branch reuse passes.** The Gemma control
completed all four phases. Cold missed once; repeat and both forks each hit
once and saved all 33,116 shared prompt tokens. The two forks returned the
exact required final verdicts (`BRANCH_1=SAFE`, `BRANCH_2=UNSAFE`) and recorded
TTFTs of 0.938 s and 0.967 s, compared with 13.39 s cold and 15.34 s on the
exact repeat. This proves useful branch reuse at four retained entries for this
pinned model/runtime/configuration. It does not yet select a product default:
the run is a single sample, Gemma is not transferable to Qwen3.6, the cold and
fork requests are not identical request shapes, and e4 was not compared with
e2 under branch pressure. Run the bounded Qwen3.6 35B-A3B e2 and e4 controls
next, then require repeated/matched branch-pressure evidence before changing a
default or publishing the observed speedup magnitude.

**2026-08-02 Qwen3.6 e2 startup-warmup correction.** The first bounded
35B-A3B e2 attempt stopped after cold reported two evictions. The backend log
shows why: Rapid-MLX runs a real two-token request to compile hybrid
GatedDeltaNet kernels and leaves that request's prompt-only and prompt+output
snapshots in the cache. The measured 32k cold request then missed, displaced
those two warmup entries, and stored its own two snapshots. This is startup
contamination of the cold eviction count, not evidence that Qwen needs more
than two entries. There is no working HTTP clear operation for the
memory-aware prefix cache in 0.11.1. Cold controls therefore require a miss and
zero hits but treat eviction count as diagnostic; repeat/fork hit, miss, and
saved-token counters remain the qualification gates. Preserve the failed
attempt and backend log as the regression receipt, then rerun e2 in a new
receipt directory.

The corrected Qwen e2 rerun passed: cold missed once with 5/5 marker recall;
repeat hit once, missed zero times, and reported 31,769 prompt tokens saved,
again with 5/5 recall. TTFT still worsened from 15.83 s to 18.72 s. For hybrid
Qwen, an exact cache hit is counted before the scheduler's correctness-first
non-trimmable fallback discards the reused state and performs a full prefill;
the server log's `tokens_to_prefill=31769` confirms that the saved-token counter
is not an effective-compute claim on this path.

The first Qwen e4 attempt then produced the correct `BRANCH_1=SAFE` answer but
correctly missed its cache gate. A completed hybrid turn is a non-trimmable
supersequence of the next turn's message boundary. The first branch must prefill
that boundary and create a dedicated `boundary_snapshot`; only a later sibling
branch can reuse it. The Qwen control is therefore architecture-aware: fork 1
is the required boundary-seeding miss, while fork 2 must hit and save the shared
prefix. This does not relax the recommendation gate; if e4 evicts the boundary
snapshot before fork 2, four entries are insufficient for this Qwen workload.

The corrected Qwen e4 rerun passed. Fork 1 was the expected boundary-seeding
miss, returned `BRANCH_1=SAFE`, and had 20.78 s TTFT. Fork 2 hit the newly
stored 31,816-token boundary, prefilling only a 913-token suffix, returned
`BRANCH_2=UNSAFE`, and had 0.965 s TTFT. This reconciles the new control with
the original Phase 6 `cold → repeat → follow-up → fork` receipts: their
follow-up performed the same boundary-seeding role before the measured fork.
The old `--hybrid-cache-entries 16` was deliberately non-binding during
cache-memory calibration, not a measured entry-count recommendation. Four is
now sufficient for this bounded sequential-sibling control; the branch-pressure
sweep must determine whether it remains sufficient across a wider working set.

The bounded workload sweep now supplies the missing product policy. At 32K,
e2 missed all eight branches; e4 and e16 both kept the active shared boundary
hot with essentially identical ~1.2–1.3 s fork TTFT. Separate-session controls
then showed the retention difference: e4 lost both roots in a parallel-1
main-plus-one-child sequence, while e8 retained both; e4 lost three alternating
roots, while e16 retained all three. All exact SAFE/UNSAFE verdicts passed.
Keep 16 as the general agent-workflow default, present 8 as the strict
main-plus-one-child option, and present 4 as solo-history only. The separate
8 GiB retained-memory baseline remains unchanged.

- **`--cache-ram` multi-branch / slot-pressure run** (Phase 6 item 9) — completed for the product
  floor: one live slot, parallel 1, main plus one sequential child. Through 32K, `0` lost both
  roots while 2048 MiB and 8192 MiB retained both; 2048 MiB is the smallest tested positive cap.
  Keep `0` for one linear conversation and use 2048 conditionally when the delegated root must
  resume and unified-memory headroom permits. Multi-slot testing was deliberately stopped because
  the target llama.cpp MTP workflow is parallel 1.
- **`--hybrid-cache-entries` sweep** — completed for the bounded sequential agent-workload policy
  in `docs/archive/rapid-mlx/20260731-hybrid-cache-entries-sweep.md`. Qwen3.6 evidence supports 4 for one hot
  history, 8 for main plus one sequential child, and 16 as the general agent-workflow default.
  True concurrent-arrival stress remains a separate follow-up; it is not required to select the
  parallel-1 default.
- **Library tab UI — audited 2026-08-02.** Release-built `models-v2` captures verified the actual
  Library inventory in dark, light, narrow, and non-Apple states. Cards, lifecycle/compatibility
  badges, backend actions, fixed footer, and narrow stacking are coherent. The separately named
  `model-library` scenario captures HF Download/discovery rather than the Library tab; do not use
  its name as evidence of Library coverage. Phase 8B2/8B3 functional screenshot approval is closed,
  while the broader MLX Wizard/Preset/Dashboard redesign below remains open.
- **`rapid-mlx-live` — audited 2026-08-02 with the cached Qwen3-0.6B-4bit control.** Live spawn on
  an isolated port, telemetry, real chat, supervised stop, fallback cleanup, and temporary-preset
  cleanup completed. No Rapid process remained. The dashboard is functionally coherent but visually
  sparse and below the llama.cpp inference-metrics bar; its pre-chat telemetry capture also shows
  zero totals, while meaningful request totals appear only in the stopped/historic capture. Treat
  this as functional approval and direct input to the deferred premium Dashboard redesign, not as
  final visual approval of the release design.
- **Shared HF cache — read-only audit completed 2026-08-02; nothing deleted or moved.** The Hub
  cache is about 337 GiB and `~/.config/llama-monitor/models` is about 746 GiB. Preserve the active
  preset/session, benchmark, and MTP control set. Separately approved cleanup candidates are about
  26 GiB of local `.staging/downloads` partials, 3.4 GiB of Rapid mirror `.safetensors.part` files,
  one 54 MiB nonempty HF `.incomplete`, zero-byte locks/incompletes, and a physically duplicated
  335 MiB Qwen3-0.6B-4bit snapshot. Reconcile the migration journal and snapshot reachability before
  any cleanup; do not blanket-prune repositories or revisions.

---

### 3.12 — Historical intermittent test failure

`guided-generation/phase8-tag-cloud.spec.js:255` — a group header's `aria-expanded` still `false`
after a re-expand click. It failed **once** across five full runs and passed the latest full
release-built run (262/4/0).

It is a **different shape** from the three flakiness causes that were fixed (a click that did not
take, not a torn-down modal), so it is recorded separately rather than assumed to share a cause.

It is not currently release-blocking, but the root cause is still unknown. Keep the regression test
and investigate if it recurs; do not delete it or treat one green run as a diagnosis.

**2026-08-02 follow-up investigation:** ran `phase8-tag-cloud.spec.js` with `--repeat-each=15`
(180 test executions across all 12 specs in the file, release build) — 180/180 passed, no
reproduction. Reviewed the toggle implementation
(`chat-suggestions.js:setupTagCloudUI`/`initSuggestionsDropdown`) and found nothing that could
race: the `.category-group-header` elements are static markup already present in `index.html`
at page load (not built/rebuilt by JS), bound exactly once via a `dataset.tagCloudBound` guard,
with a synchronous click handler that just flips the `aria-expanded` attribute — no async work,
no re-render, no listener rebinding in the collapse/expand path. The collapse animation itself is
pure CSS (`grid-template-rows` transition via the `[aria-expanded="false"] ~ .category-group-chips`
sibling selector), so it can't desync JS state from the DOM either. Given 180 consecutive green
runs and no plausible code-level race found, this is most likely a one-off CI/environmental blip
rather than an app defect. No code changes made. Still keep the regression test; re-open this
investigation only if it recurs with a reproducible pattern (e.g. always on the same run index,
or correlated with a specific CI runner).

---

## 4. Closed decisions — do not reopen

These were argued and settled. Re-litigating them wastes a quota that is scarce.

- **`workload_scenario` is not persisted in a preset.** It is spawn-wizard entry guidance only. For
  llama.cpp it should do exactly one thing: pick q8_0 vs q4_0 KV against GGUF size for agentic vs
  roleplay. There is not enough divergence beyond that to justify storing it.
- **`CONSTRAIN_TOOLS` is not a product setting.** Dropping tool grammar to buy speculation speed is
  the wrong trade.
- **llama.cpp's web-UI flags stay unexposed.** `default_launch_argv_omits_experimental_webui_mcp_proxy`
  (`llama_cpp.rs:1009`) pins the argv and deliberately omits them; llama-monitor takes
  llama-server's default web UI as-is. The phantom `web_ui_*` fields were those flags mis-copied
  into the *Rapid* builder, so deleting them restored the intended state rather than creating a
  hole.
- **`--kv-disk-checkpoint-interval` stays pinned at 0.** Fully plumbed and passed on every launch as
  a hardcoded 0. `0` = no disk checkpointing, correct for interactive use where the retained cache
  lives in unified memory. The real question is a measurement one and belongs in the benchmark
  suite, not a dropdown.
- **`--response-cache-entries` is deferred pending measurement.** No config field exists; adding one
  is a new feature, not a restoration. Duplicate requests are rare in interactive chat and
  plausible in agentic tool loops, and there is no data on which. **Do not expose a knob whose
  benefit cannot be stated.**
- **Vision is off the table** for rapid-mlx (runtime broken except possibly Gemma-4). But **build
  vision detection anyway** — detection is decoupled from runtime support, and other MLX loaders
  consume it.
- **Phase 8 excludes audio deliberately.** Scope is text/tools/MCP; a separate TTS project handles
  audio.

---

## 5. Structural traps worth remembering

These are the mechanisms that *caused* the defects, as opposed to the defects themselves. They will
bite again.

### The deny-by-default CSS allowlist
`modal-premium.css:2343` hides Rapid controls unless **both the row and the field** are exempted by
id, in two separate `:not()` chains. Every control added to the preset modal is invisible until
someone edits both. This single mechanism accounted for nine unreachable controls.

`rapid-preset-visibility.spec.js` now defends it by brace-matching the Rapid branch of
`_buildFormPreset` and extracting **every** `modal-*` id it reads, then checking each against every
section a Rapid preset can open. Its earlier version matched only `/modal-rapid-[a-z0-9-]+/` and
so could not see six of the seven controls it should have caught. **Do not narrow that regex.**

### Spread-omission in `_buildFormPreset`
`{...existing.rapid_mlx, ...out}` means an omitted key **keeps its previous value**. The idiom
`if (value) out.x = value` therefore makes "unset"/"Auto" *unreachable on any preset that already
has a value*. Every Rapid control had this, not just the four originally found. All now write
`null` unconditionally.

**The near-miss worth internalising:** had the class-wide fix landed without the reachability fix,
every save would have written `null` over the user's stored sampling defaults — because the inputs
were unreachable and therefore always read empty. The conditional idiom was *masking* the
reachability bug. Two bugs were cancelling out.

### `capabilities.require()` aborts the entire command build
A single unsupported flag returns `Err` and blanks the whole preview — no argv, no
`effective_policy`, no `requested_vs_effective` **for any unrelated setting**. This is what made
the misdiagnosis in §0 point 4 possible.

Throughput features (TurboQuant, PFlash, speculative) are now handled the other way: **omitted from
argv and reported as a downgrade**, never aborting. `command.rs:487` set the precedent. Any new
capability-gated flag should follow it.

### Capabilities are parsed from `--help`
`ServeCapabilities::from_help` derives capabilities by parsing `rapid-mlx serve --help`, so a flag
absent from that help **can never be detected**. That makes phantom flags mechanically checkable:
`settings.rs::serve_flag_literals_exist_in_the_real_runtime` compares every `--flag` literal in the
source against `testdata/serve-flags.txt` (85 flags, rapid-mlx 0.11.1). It scans only probe and
emission sites (`has_flag(`, `.require(`, `.contains(`, `args.push(`), so negative assertions do
not trip it. **Keep `testdata/serve-flags.txt` current when the runtime updates.**

Note how the old tests failed here: `command.rs`'s Phase 7 argv test built its capability set from
a **hand-written help string that declared all twelve invented flags**. The assertions then
confirmed the builder's mistakes. Green tests were not merely silent — they were actively wrong.

### `Router.onBeforeDispatch` closes the settings modal
`bootstrap.js:221` closes the settings modal for any path that is not `/settings` — correctly, so
Back/Forward does not strand overlays. Therefore **never open a modal by importing
`openSettingsModal` directly in a test**; that fabricates a state (modal open at URL `/`) the app
never produces, and any dispatch landing later tears it down. Use `openSettings(page, tab)` from
`tests/ui/helpers.js`, which navigates the way the product does.

---

## 6. Environment facts

- Dev machine is an **Apple Silicon M5 Max** — real Rapid-MLX hardware calibration is possible
  locally.
- **`rapid-mlx` CLI has no `--json` output.** `info`/`doctor`/`models`/`bench` must be text-scraped;
  there is no stable contract. Do not build on their formatting without a guard.
- **The archived 131K prefill crash** occurred under the benchmark's experimental
  `--prefill-step-size 32768`: full-attention prefill attempted a quadratic 68.7 GB attention-score
  buffer. Changing only 32768→4096 live-verified the root cause; 4096 was a recovery experiment,
  not the product recommendation. The qualified text default remains 512 and the intentional
  product ceiling remains 2048. KV settings and `max_position_embeddings` were not the cause.
- **llama.cpp needs q8_0 KV for agentic work** (q4_0 loops). The MLX equivalents (TurboQuant,
  RotorQuant) differ and their floor is unknown — measure on the M5 Max.
- **Chat templates are tool-call-reliability critical.** Stock Qwen3.6 / Gemma4 templates loop or
  fail on tools; the Froggeric fixed templates are the remedy.
- **Never assume model metadata exists.** Use ordered sources and record provenance; absence
  downgrades a claim rather than failing the run. `aliases.json` is known-wrong for Qwen3.6-27B.
- Updates to rapid-mlx and its deps land near-daily and are validated **on the user's box**, not in
  CI — mirroring the thin llama.cpp beta checks.
- **Release bar:** a single cutover, no intermediate releases. Ships only at full backend/frontend
  parity **and** novice + power-user UX.

---

## 7. Resume checklist

The following is the actual remaining order. Everything in §§3.1–3.4, 3.6, 3.7, and 3.9 is
implemented and receipt-backed; do not redo it unless a regression appears.

### 2026-07-31 checkpoint: frequency penalty resolved

`default_frequency_penalty` (§3.5, item 2 below) is done:
- `RapidMlxAdapter::validate()` (`src/inference/rapid_mlx/mod.rs`) rejects launch when the value is
  outside `-2.0..=2.0`, following the OpenAI-style convention already implied by the flag's naming
  and its sibling `presence_penalty`.
- Preset Editor gained `modal-rapid-frequency-penalty`, a Rapid-only numeric field in the existing
  Sampling Mode row (`static/index.html`), added to the deny-by-default CSS allowlist
  (`modal-premium.css:2351`), and wired into `presets.js` load/save/label/reset paths the same way
  `default_presence_penalty` is.
- Verified: `cargo test --lib` 1067/13/0 unchanged; `npm run validate-js` and `eslint` clean;
  `core/rapid-preset-visibility.spec.js` passed (its self-deriving allowlist check would have failed
  had the CSS chain been missed).
- Deliberately out of scope: no llama.cpp equivalent (llama.cpp has no `frequency_penalty` flag —
  confirmed by grep), no spawn-wizard control (presence_penalty has none either for Rapid), no
  model-family auto-fill (no server-side default exists for this field yet).
- No new Rust unit test was added for the validate() branch — `RapidMlxAdapter::validate()` has no
  existing unit tests at all (it's an async, filesystem-checking launch-time gate that this
  campaign's philosophy verifies live, not synthetically); adding one here would need a first
  synthetic harness for the whole function, which is out of scope for a single-field task.

1. **External MTP companion safety:** implement immutable sidecar preflight, provenance and
   compatibility checks, explicit lifecycle/memory status, and revision-scoped
   `trust_remote_code` consent. Keep MTP default-off and clearly marked as greedy/unconstrained-only
   until the qualification harness proves sampled and constrained-tool dispatch.

   **2026-07-31 checkpoint — read-only preflight resolver shipped, still incomplete:**
   - Investigation found the launch-time consent gate (`validate_trust_consent` in
     `src/inference/rapid_mlx/command.rs`) and the local-disk detector
     (`needs_trust_remote_code` in `model_resolver.rs`) already existed and were already
     fail-closed and unit-tested — that part of this item was done before this session started.
   - The actual gap: nothing could resolve an *external* `owner/repo` reference (the speculative
     `model` field) to a pinned revision or a `trust_remote_code_required` verdict without a full
     download, and nothing surfaced either fact anywhere — not via API, not via UI. Any repo that
     needed it was permanently unlaunchable with no diagnostic.
   - Added `crate::hf::resolve_speculative_model_preflight(repo_id)` (`src/hf/mod.rs`): one HF
     `/api/models/{repo_id}` call resolves the immutable commit `sha` and checks the returned
     `siblings` list for `main.py`/`modeling_*.py`/`configuration_*.py`; if none are found it fetches
     `config.json` at that pinned revision and mirrors the exact `main_class`/`auto_map` heuristic
     `needs_trust_remote_code` uses locally. Read-only — does not download weights, does not touch
     consent.
   - Exposed as `GET /api/hf/mtp-preflight?repo=owner/repo` (`src/web/api/hf.rs`), auth-gated and
     `validate_hf_repo_id`-checked like the other HF endpoints; returns
     `{ok, repoId, revision, trustRemoteCodeRequired}`.
   - Verified live (not just unit-tested) against three real repos, then the throwaway test was
     removed: `mlx-community/Qwen2.5-0.5B-Instruct-4bit` → sha resolved, `false`; a deliberately
     invalid repo id → clean `HTTP 404` error; `THUDM/chatglm3-6b` (a real custom-code repo) →
     sha resolved, `true`. `cargo test --lib` 1067/13/0 unchanged; `cargo fmt --check` clean.
    - **2026-08-01 checkpoint — trust consent frontend delivered (partial item 1):**
      - Built a modal trust-consent flow for MTP preflight. When `/api/hf/mtp-preflight` reports
        `trustRemoteCodeRequired: true`, a modal now appears before launch explaining the risk and
        asking for explicit consent. The flow exists in both the Preset Editor and the Spawn Wizard.
      - The consent gate works as a two-level preflight: first the MTP-specific endpoint
        (`/api/hf/mtp-preflight`), then a secondary call to the general preflight to verify the
        `model_size_bytes` field is present. If the model is significantly larger than the user
        selected (e.g., asking about 84B when 2B was chosen), the modal shows a warning that the
        model may not be what was intended.
      - The modal is dismissible (decline) or confirmable (proceed). Declining prevents launch;
        confirming sets `trustRemoteCode: true` in the request payload so the backend consent gate
        (`validate_trust_consent`) passes.
      - Screenshots captured in dark and light modes via the harness.
      - Auth pattern in both `spawn-wizard.js` and `presets.js` corrected to use
        `window.authHeaders()` instead of a bare `API_TOKEN` variable (not defined).
       - **2026-08-01 checkpoint — pin cache, re-check, trust consent security fix, partial close:**
         - Pin cache: `MtpPinCache` in `src/hf/mtp_pin_cache.rs` with 24-hour freshness window.
           Persistent JSON store at `$CONFIG_DIR/mtp_pin_cache.json`. `init_pin_cache()` called at
           startup in `main.rs`. Preflight endpoint checks cache first; stale pins re-resolve in
           background. `GET /api/hf/mtp-pins` returns all cached pins with staleness info.
         - Re-check: `POST /api/hf/mtp-preflight/recheck?repo=owner/repo` endpoint; re-resolves
           repo via HF API, compares sha, updates pin. UI has re-check buttons in both Preset
           Editor and Spawn Wizard.
         - Security fix: companion model trust consent now validates against the companion model's
           pin cache entry, not the main model. `validate_trust_consent_simple()` added in
           `command.rs`. Launch blocked if companion needs trust but no pin cached or no consent.
           `HF_TRUST_REMOTE_CODE=1` set if either main or companion needs trust.
         - Cache management: `DELETE /api/hf/mtp-pins/{repo_id}` endpoint for pin removal.
           `MtpPinCache::remove()` method.
         - UI: pin info fields (revision, staleness, upstream change) shown in trust modals.
           Re-check buttons wired.
          - **Still incomplete — does not fully close item 1:** no explicit lifecycle/memory-status
            surfacing (no dedicated cache management UI, no memory status display). The "immutable
            sidecar" concept is implemented in the pin cache but not surfaced as a distinct UX.
            `cargo clippy` clean; `cargo test` 1073/0/13; `cargo build --release` passes.
         - **2026-08-01 checkpoint — pin status UI delivered, item 1 closed:**
            - Pin status UI added to both Preset Editor and Spawn Wizard. Shows pinned revision
              (repo@sha), staleness indicator (green dot = fresh, yellow = stale), trust_remote_code
              flag, resolved time, and in-line re-check button.
            - Pin status appears when a companion model repo id is entered and preflighted, regardless
              of whether trust is required. Re-check updates the pin cache and refreshes the display.
            - `GET /api/hf/mtp-pins` and `DELETE /api/hf/mtp-pins/{repo_id}` provide programmatic
              cache management.
            - `cargo clippy` clean; `cargo test` 2220/0/26; `npm run lint` clean;
              `npm run validate-js` clean.
           - **2026-08-01 checkpoint — VRAM memory estimate delivered, item 1 fully closed:**
             - Memory status surfaced in pin status UI: shows "~X MB VRAM" or "~X GB VRAM"
               derived from the HF API's `safetensors.total` parameter count (Q4 quantization
               estimate, 0.5 bytes/param). Covers the "explicit memory status" requirement.
             - `estimated_memory_bytes` stored in pin cache and included in all preflight responses.
             - `cargo clippy` clean; `cargo test` 1074/0/13; `npm run lint` clean.
           - **2026-08-01 checkpoint — MTP companion introspection (no Q4 assumption):**
             - Replaced hardcoded Q4 estimate with 3-tier fallback: (1) HF tree API →
               `mtp.safetensors` file size (exact VRAM for any MTP sidecar repo), (2)
               `mtplx_runtime.json` → quantization label (`mtp_sidecar`) and depth
               (`mtp_depth_max`), (3) no estimate if neither available.
             - Both tree API and `mtplx_runtime.json` fetched in parallel via `tokio::join!`.
              - Pin status UI shows quantization + depth for MTPLX repos (e.g., "sidecar:bf16 d3").
              - `cargo clippy` clean; `cargo test` 1074/0/13; `npm run lint` clean.
           - **2026-08-01 checkpoint — local sidecar inventory (Item 1 extension):**
              - Discovered `~/.config/llama-monitor/models/rapid-mlx/mtp-sidecars/` framework:
                local sidecars built by `scripts/build-mtp-head.py` with provenance.json.
              - New module: `src/inference/rapid_mlx/sidecar_inventory.rs` — discovers local
                sidecars, reads provenance.json, estimates VRAM from mtp.safetensors file size.
              - API endpoint: `GET /api/hf/mtp-sidecars` lists all sidecars with provenance
                (trunk, build date, norm check status, VRAM estimate, quantization).
              - Frontend: sidecar list rendered in both spawn wizard and preset editor when
                "external" source is selected. Clicking a sidecar populates the companion
                model input with the sidecar path.
              - Pin status distinguishes local sidecars (shows "(local sidecar)", no re-check)
                from HF repo pins (shows repo@sha, re-check button).
              - Launch-time VRAM estimate for local companion paths via
                `estimate_local_companion_vram()`.
              - `companion_model_local_path()` method added to `RapidMlxSpeculativeConfig`
                for distinguishing HF repos from local paths.
              - `cargo clippy` clean; `cargo test` 1076/0/13; `npm run lint` clean.
           - **2026-08-01 safety correction — remote HF sidecars are preflight-only:**
             - Rapid-MLX 0.11.1 loads a speculative `owner/repo` through an unpinned
               `snapshot_download(repo_id)` and exposes no revision field in the MTP schema.
               Llama Monitor therefore rejects remote-HF companion launch rather than claim a
               pin/consent applies to a mutable artifact. Self-built or otherwise immutable local
               sidecar directories remain supported.
             - Cached-pin refresh and manual re-check now return/update the same revision, trust,
               memory, quantization, and depth fields.
 2. ~~**Frequency penalty decision**~~ — done, see checkpoint above.
 3. ~~**Legacy quant-style migration**~~ — done (see §3.8, 2026-08-01). The `/api/hf/quantizers`
    endpoint is now a derived view over the catalog; `/api/hf/community-picks` left unchanged.
4. ~~**Run the remaining non-code audits in §3.11**~~ — completed read-only on 2026-08-02.
   Library and Rapid-live functional evidence is recorded above; the shared HF cache was inventoried
   without deletion or migration. Any cleanup requires a separate, explicitly approved staged pass.
5. **Small integrity cleanup — done except the intermittent:** the capture.mjs stale-scenario check
   and the unused HF label exports are resolved (see §3.10 checkpoints). Still open: investigate the
   tag-cloud intermittent if it recurs. Run `rtk git diff --check` after documentation or code edits.
6. **MTP requalification is parked on an upstream runtime fix, not active work.** Do not rerun the
   matrix against another known greedy-only build. Reopen only after a Rapid-MLX release claims
   sampling/logits-processor support, then use the future-build harness with nonzero-temperature
   sampled and constrained-tool requests, reasoning enabled, observable nonzero MTP activity,
   parity/fidelity, and explicit semantics-preserving fallback evidence. A greedy 0-temperature
   acceptance result alone is never a promotion gate.

### 2026-08-02 Qwen3.5 chat-loop diagnosis and backend contract correction

The Qwen3.5-9B loop was not reproduced as a deterministic template or hybrid-cache defect. With
the same sampled Qwen defaults, Froggeric v20, Froggeric v21.3, and the bundled template all
stopped normally; forced-hybrid and no-hybrid runs also stopped normally. A greedy ordinary-chat
control repeated to the full 8,192-token ceiling, so the earlier greedy tool receipt only proved
that constrained tool generation supplied a structural exit. It did not qualify greedy chat.

The application-level trigger was the shared role-boundary scaffold. The concise prompt stopped
in 199–361 output tokens with the chat UI's sampling values, while adding the full boundary pushed
reasoning beyond 2,048 tokens. The intended safety cap was ineffective because the shared client
sent llama.cpp's `thinking_budget_tokens`; Rapid-MLX 0.11.1 expects
`reasoning_max_tokens`. A corrected watched probe emitted exactly 2,048 reasoning chunks, then 18
content chunks, and finished normally (`finish_reason: stop`, 2,068 output tokens reported by the
server).

The Rapid adapter now maps the shared reasoning budget to `reasoning_max_tokens`, while preserving
an explicitly supplied Rapid-native value. This is a backend compatibility and safety correction,
not part of the MLX-native UI/UX redesign below. Any later chat-policy work (sampling defaults,
template/thinking kwargs, parser policy, or prompt scaffolding) is a separate scope and must not
expand or delay the Spawn Wizard, Preset Editor, and Dashboard release-polish work.

### Deferred addendum — MLX release-polish UX pass (after the runnable audits above)

**Priority:** high for the release branch once this handoff's remaining audits and gates are complete;
not authority to delay or expand those items now. This is a bounded premium-polish pass over the
three customer-facing Rapid-MLX surfaces, using llama.cpp's clarity and confidence as the quality
bar without copying controls Rapid-MLX does not support. Treat the current layout as replaceable:
avoid substantial interim polish that will be discarded. The redesign must establish an MLX-native
information architecture that serves Rapid-MLX now and can add MTPLX as a first-class backend in the
near future without presenting it as a Rapid-MLX mode or speculative-decoding toggle.

- **Spawn Wizard:** allow a genuinely MLX-native route where it improves comprehension rather than
  forcing the GGUF/llama.cpp sequence. Replace the dense, two-column `Rapid-MLX advanced` control
  dump with progressive decisions. Once a model is selected, keep a persistent MLX fit rail visible
  through memory/performance, generation, and review—not hidden by the Rapid route. It must show
  model weights/quantization, active context/KV, retained cache, companion overhead, safety
  headroom, and live impact from changed controls using the server estimator as the sole authority.
  Keep the same immediate fit signal during MLX model search/selection; group Generation
  (thinking, sampling, parsers), Cache & Performance (KV precision, retained cache/entries,
  prefill and batching), and Server & Safety (port, auth, concurrency, companion constraints)
  behind clear follow-up affordances.
- **Preset Editor:** replace the overloaded three-tab Rapid layout with a backend-native Model /
  Generation / Cache & Performance / Server & Safety information architecture. Retain the same
  estimator as a persistent cross-section fit strip with Explain and context-appropriate Auto-size,
  so users see the memory impact of model, quantization, context, cache, and companion changes. Do not recreate
  llama.cpp-only tabs or expose unqualified knobs. Wizard and editor must use the identical labels,
  defaults, explanations, save/load behavior, command preview, and review summary.
- **Dashboard:** give every trustworthy Rapid telemetry signal the same premium, modern visual
  treatment as llama.cpp inference metrics: clear hierarchy, polished charts, sparklines, gauges,
  and state transitions where the data supports them. Make current effective policy, cache reuse
  (hits, reused prompt tokens, observed TTFT), throughput/latency, concurrency, and memory state
  readable at a glance. Do not invent a metric or show a speculative-speedup claim until it has a
  naturally-eligible, repeated, fidelity-qualified receipt. Use the same scale, terminology, and
  confidence treatment as llama.cpp's performance surfaces.
- **Release proof:** review at desktop and narrow widths with capture artifacts, build the release
  binary before each changed scenario, and run the normal JS/lint/Playwright validation. Promoted
  documentation screenshots must be capture-clean and actually referenced by docs.

**Scope boundary:** this redesign covers only the Spawn Wizard, Preset Editor, and Dashboard. It
must remain MLX-broad so MTPLX can reuse the information architecture and visual system without a
second redesign. It does not authorize a chat UI redesign.

**Known starting defects:** Rapid controls are visually over-compressed; the editor's `Server` tab
owns unrelated generation, caching, and safety decisions; dashboard recommendations must remain
evidence-tiered; and reference captures must crop to their relevant panel rather than spend their
resolution on empty backdrop. The former teal active-step outline was a capture-only
Puppeteer/Chromium focus-visible artifact and is fixed with a capture-scoped marker.

### 2026-08-02 MLX-native redesign continuation checkpoint

#### Authority, scope, and clean base

- Clean pre-redesign checkpoint: `feb4728 feat(spawn): qualify cache retention and Rapid chat
  safety`. It contains the completed cache receipts/policy, chat reasoning-cap adapter, archived
  sweep plan, read-only cache audit, and functional Library/Rapid-live evidence. The worktree was
  clean and no Rapid/capture process remained immediately after that commit.
- Redesign scope is **only** Spawn Wizard, Preset Editor, and Dashboard. Do not turn this into a
  chat UI redesign. Shared chat compatibility work is separate.
- The presentation must be **MLX-family**, not a second set of Rapid-only conditionals. Rapid-MLX
  supplies today's capability/config adapter; future MTPLX must be able to reuse the same shell,
  section vocabulary, estimator presentation, and metric visual system without pretending to be
  Rapid-MLX.
- Hard estimator invariant, stated twice by the user: the authoritative VRAM/unified-memory fit bar
  must remain visible in **both** the Spawn Wizard and Preset Editor everywhere a model,
  quantization, context/KV, retained cache, companion, or performance control can change memory.
  `/api/vram-estimate` remains the sole formula source.

#### Completed Preset Editor slice (checkpoint next)

The following files are intentionally dirty after `feb4728`:

- `static/js/features/preset-editor-mlx.js` (new): presentation-only MLX information architecture.
  It preserves every existing form-control ID and moves existing Rapid rows at runtime, with comment
  anchors restoring their original llama.cpp positions. It relabels/reorders four existing sections:
  `Model & Fit`, `Generation`, `Cache & Performance`, `Server & Safety`. This avoids duplicate fields
  or a second serialization path. It also groups thinking/protocol, active memory, retained cache,
  scheduler, and companions. Companions/MTP is an advanced collapsed disclosure because current MTP
  qualification is parked and must not dominate the normal flow.
- `static/js/features/presets.js`: imports/configures that module and now builds Rapid estimates from
  **live unsaved form values** rather than the stored preset. KV dtype, TurboQuant, retained-cache
  toggle/memory, prefill step, and typed speculative configuration therefore affect the visible bar
  before save.
- `static/js/features/vram-estimate.js`: fixes two cross-surface contract bugs discovered during the
  redesign. `retained_cache_mib` was calculated by callers but never serialized; MTP was emitted as
  unused `mtp_config` while Rust accepts typed `speculative_config`. The canonical builder now sends
  `retained_cache_mib` and `speculative_config`; Wizard hardware constructs the same typed structure.
- `static/css/modal-premium.css`: removes the rule that hid `#preset-vram-strip` for Rapid, widens the
  MLX editor/nav, adds premium grouped surfaces and light-theme treatment, makes the four MLX tabs a
  two-by-two narrow layout, and supplies the collapsed advanced-companion disclosure. It retains
  reduced-motion behavior from the shared editor. The high-specificity generation-field override
  is paired with direct-row markers so the six shared sampling controls remain visible while
  llama.cpp-only Generation rows stay hidden. Max Tokens now lives in an MLX Output Limit group;
  Seed remains llama.cpp-only. Switching back removes all MLX markers and restores original rows.
- `tests/ui/capture.mjs`: `rapid-preset` now waits for the estimator and captures the actual named
  sections (`model`, `generation`, `cache-performance`, `server-safety`). Old `advanced` names were
  stale. Speculative captures are now full-screen modal/backdrop captures after scrolling/opening the
  correct disclosure—not element closeups. This matches the user's explicit capture preference.
- Focused specs: cache/batch and throughput tests now navigate to `context` (Cache & Performance),
  not the old Advanced tab. `rapid-preset-visibility.spec.js` asserts exact visible MLX nav order,
  persistent fit strip, group reachability, live estimator payload changes, and advanced companion
  access.
- `src/gen/routes.rs` and `src/gen/static_assets.rs` changed only because a release build registered
  the new static module. Keep them with the module.

Validation completed for this slice on 2026-08-02:

- `npm run validate-js` passed after every JavaScript change, including the new module.
- `npm run lint` passed after every JavaScript change.
- `rtk cargo build --release` passed after the final product changes and regenerated the embedded
  static asset route.
- The focused release-binary Playwright set passed outside the sandbox: `PASS (23) FAIL (0)`.
  It covers MLX information architecture, every Rapid control's reachable unset state, estimator
  payload updates, cache/batch/throughput persistence, Phase 7 preset behavior, save/duplicate
  preservation, and llama.cpp restoration.
- `rapid-preset` and `preset-editor` capture scenarios both completed outside the sandbox. Original-
  resolution inspection covered Model, Generation, Cache & Performance, Server & Safety, dark/light,
  2x2 narrow navigation, expanded speculative controls, and restored llama.cpp Model/Context/Advanced
  screens. Captures retain the full application/modal backdrop as requested.
- The speculative capture now waits for the estimator to settle after enabling MTP; the earlier
  `Estimating VRAM...` artifact is no longer possible without failing the capture scenario.
- `rtk git diff --check` passed.
- An earlier focused Playwright attempt inside the sandbox reported `PASS (0) FAIL (0)` after the
  180-second web-server timeout because sandbox loopback bind returns `EPERM`. This is not a test
  result; retain it only as a warning to run future browser validation outside the sandbox.

Receipts used for the completed Preset slice (rerun only if this slice changes):

1. Release build: `rtk cargo build --release`.
2. Escalated Rapid capture:
   `SCREENSHOT_PORT=<free> rtk node tests/ui/capture.mjs --scenario rapid-preset --no-attach`.
3. Original-resolution visual inspection:
   `rapid-mlx-preset-editor-model.png`, `-generation.png`, `-cache-performance.png`,
   `-server-safety.png`, light variants, narrow Server/Safety, and full-screen speculative states.
   Confirm Generation controls are visible after the specificity fix; confirm narrow tabs are 2x2;
   confirm the estimator has settled and remains visible on every section.
4. Focused Playwright **outside sandbox**:
   `CI=1 LLAMA_MONITOR_USE_RELEASE=1 LLAMA_MONITOR_TEST_PORT=17778 rtk npx playwright test
   core/rapid-preset-visibility.spec.js core/rapid-preset-cache-batch.spec.js
   core/rapid-preset-throughput.spec.js core/phase7-presets.spec.js core/preset-flow.spec.js`.
5. Check llama.cpp editor restoration by running the `preset-editor` capture and its existing specs;
   the move/restore anchors must not reorder or hide llama.cpp fields after switching backend types.

#### Spawn Wizard rearchitecture: capability extraction done, presentation redesign next

**2026-08-02 checkpoint:** `static/js/features/spawn-wizard-mlx.js` now exists (behavior-preserving
extraction, ~10,168 lines net: spawn-wizard.js shrank from 11,168 to ~10,411 lines). Extracted:
`bindRapidMlxAdvancedControls`, `syncRapidSpeculativeFields`, `applyReasoningModeLock`,
`applyRapidMlxDefaults`, `renderRapidExclusionWarnings`, and the trust/pin/sidecar cluster
(`_spawnCheckTrust`, `_spawnRecheckTrustPin`, `_hideSpawnTrust`, `_updateSpawnTrustUI`,
`_renderSpawnPinStatus`, `_fetchSpawnSidecars`, `_timeAgoSpawn`), plus the live-profile cluster
(`_fetchRapidMlxModelProfile`, `scheduleRapidMlxProfileFetch`, `_renderRapidMlxProfileHints`,
`_applyUnifiedProfileRecommendations`, `_renderUnifiedProfileWarnings`). `spawn-wizard.js` exports
`dom` (mutable live binding, mirroring the existing `wizardState` export) so the new module can
read/write shared wizard state without a parameter-threading rewrite. All DOM control ids and
event-wiring order are unchanged, including the module-parse-time `speculativeRecheckBtn` listener
(now attached inside `bindRapidMlxAdvancedControls` at call time instead of at module-parse time —
behaviorally equivalent since the shell always calls it once during `initSpawnWizard`).
Validated: `npm run validate-js`, ESLint on both files, and the full `spawn-wizard.spec.js` +
`phase7-presets.spec.js` + `rapid-phase7-fields.spec.js` suite (34/34 passed) on an isolated
`LLAMA_MONITOR_TEST_PORT=17778` instance, leaving the user's live port-7778 instance untouched.
`buildSpawnPayload`'s `rapid_mlx` branch (the request/config construction called out below) was
**not** extracted this pass — it still lives in `spawn-wizard.js` and remains a candidate adapter
extraction. The presentation/IA redesign (six-step MLX progression below) has not started.

**2026-08-02 rename + 18-step decomposition plan:** `spawn-wizard-mlx.js` was renamed to
`spawn-wizard-rapid-mlx.js` (plain `mv`, file was never `git add`ed; import path in `spawn-wizard.js`
updated to match). Rationale: the 2026-07-29 Rapid-MLX/MTPLX runtime audit
(`docs/reference/apple-silicon-mtp-runtime-comparison.md`) found MTPLX's config/cache/sampler surface
is not parameter-compatible with Rapid — the extracted module is Rapid-MLX-specific, not MLX-generic,
and its header comment now says so explicitly to stop it being copied as an MTPLX template.
Re-validated after rename: `npm run validate-js`, ESLint on both files, and 34/34 Playwright
(`spawn-wizard.spec.js phase7-presets.spec.js rapid-phase7-fields.spec.js`, isolated port 17778) — all
green.

An Opus planning pass then read the remaining ~10,400-line file directly (dependency scan below was
corrected for cross-module *writes* to module-scoped `let`, which a call-site-only regex scan misses)
and produced an 18-step, risk-ordered, sequential extraction plan for the rest of the monolith:

**Done 2026-08-02 (steps 1-2):** `spawn-wizard-format.js` and `spawn-wizard-model-card.js` extracted.
`openCardPanel` re-exported from the shell (models.js still imports it from `spawn-wizard.js`).
Re-validated: `npm run validate-js` clean, ESLint clean on all three files, 34/34 Playwright
(isolated port 17778). Removing ~90 lines from the top of the file shifted every downstream line
number in this plan down by roughly that amount — re-locate each section by function/const name
(e.g. `grep -n`) rather than trusting the line ranges below verbatim once execution reaches it.

1. ~~`spawn-wizard-format.js` (56-136) — `kvBpe`/`formatCtx`/`formatGB`/`formatBytes`/etc. Pure, zero-state.~~ Done.
2. ~~`spawn-wizard-model-card.js` (10089-10176) — `openCardPanel`/`_closeCardPanel`.~~ Done.
3. ~~`spawn-wizard-binary-prereq.js` (10176-10411) — binary prereq check/download; owns `_platformInfo` (needs `setWizardPlatformInfo()` mutator for the write at old line 2524).~~ Done
   2026-08-02. `_mtpUserConfigured` stayed in the shell (exported `let`), read-only import into the
   new module, since it's still written by the shell-resident MTP block (moves with it in step 13).
   34/34 Playwright, ESLint clean, validate-js clean.
4. ~~`spawn-wizard-tuning.js` (5196-5350) — suggestion/auto-tune/batch+depth sweep wrappers around
   `tuning-cards.js`/`tune-panel.js`.~~ Done 2026-08-02. `updateAdvisor` (and `_advisorTimer`/
   `_advisorSeq`) stayed in the shell — the dependency scan had lumped them with this cluster, but
   `updateAdvisor` actually belongs to step 14 (vram-display); the new module imports it back via a
   normal circular import (same pattern as the Rapid-MLX cluster). `isUnifiedMemory`,
   `effectiveAvailBytes`, `getEffectiveArch`, `getSizingArch`, `getModelBytes` had to be exported from
   the shell (were plain module-locals) since this and future modules read them. 34/34 Playwright,
   ESLint clean, validate-js clean.
5. ~~`spawn-wizard-hf-origin.js` (1706-1707, 1722-2029) — HF-origin auto-resolve/widget/confirm cluster; needs exported mutators (`startOriginResolve`/`awaitOriginResolve`/`resetOriginState`) replacing illegal cross-module writes to `_originResolverPromise`/`_hfOriginWidgetData`. Unblocks 6/7/12.~~ Done 2026-08-02.
   Extracted `_attachOriginTags`, `_autoResolveHfOrigin`, `_refreshHfOriginSection`,
   `_renderHfOriginWidget` (module-local), `_confirmHfOrigin`, plus the `_originResolverPromise`/
   `_hfOriginWidgetData` state. Added mutators `resetOriginState()`, `startOriginResolve()`,
   `setOriginResolverPromise()`, and `awaitOriginResolve(timeoutMs)` (new — replaces the old inline
   `Promise.race([_originResolverPromise, timeout])` read site) since the old plan's mutator list
   didn't cover the read side. Shell also had to export `getAuthHeaders`, `_fetchAndShowQuantOptions`,
   `renderHardwareModelHeader`, `_refreshHwTagsRow` (consumed by the new module), and gained a
   `resetTagsRowOrigin()` mutator for the still shell-resident `_tagsRowOrigin` (moves to
   `spawn-wizard-hf-tags.js` in step 10). 34/34 Playwright, ESLint clean, validate-js clean.
6. ~~`spawn-wizard-chat-template.js` (3564-4181) — chat template auto-install/detection cluster.~~
   Done 2026-08-02. Extracted `_chatTemplateDisplayName`, `_applyCustomChatTemplate`,
   `detectModelFamily`, `_ggufArchToFamily` (module-local), `detectModelFamilyAsync`,
   `autoInstallChatTemplate`, `_renderChatTemplateStatus` (module-local), plus the
   `_installedTemplateCache` state. Dropped the dead `export { COMMUNITY_TEMPLATES };`
   re-export from the shell (nothing imported it — `presets.js` already pulls
   `COMMUNITY_TEMPLATES` straight from `chat-template-registry.js`). **Caught a real bug**:
   the new module initially imported `awaitOriginResolve` from `./spawn-wizard.js`, but the
   shell only *imports* that name from `spawn-wizard-hf-origin.js` without re-exporting it —
   ES modules don't transitively re-export unless explicitly re-exported, so this threw
   `does not provide an export named 'awaitOriginResolve'` at runtime (Playwright's
   `html.modules-ready` wait timed out on all 34 tests). Fixed by importing
   `awaitOriginResolve` directly from `spawn-wizard-hf-origin.js` instead. Also discovered
   static assets are compiled into the binary (embed macro) — `cargo build` must be re-run
   after JS changes before Playwright's dev-mode webServer will see them; a stale binary
   masked the fix on the first re-run. 34/34 Playwright, ESLint clean, validate-js clean.
7. ~~`spawn-wizard-third-party-import.js` (4619-4749) — third-party model import.~~
   Done 2026-08-02. Extracted `TOOL_ICONS`, `loadThirdPartyModels`, `selectImportedModel`
   (module-local, only called from within `loadThirdPartyModels`'s click/keydown handlers).
   Shell had to export `renderLocalModelHint`, `refreshStepGuardrails`, `onModelPathChanged`
   (were plain module-locals) since `selectImportedModel` reads them; imports `resetOriginState`/
   `startOriginResolve` from `spawn-wizard-hf-origin.js` directly (not re-exported through the
   shell — see step 6's `awaitOriginResolve` bug for why re-export chains are unsafe to assume).
   34/34 Playwright, ESLint clean, validate-js clean.
8. `spawn-wizard-hf-browse.js` (341-347, 366-416, 2217-2276, 4183-4618, 6526-6545, ~1,100 lines) — HF search/community-picks/quantizer-editor; highest risk of the search-adjacent group, touches `initSpawnWizard`.
   - **Done 2026-08-02**: Extracted `hfBrowseState`, `hfSearchForWizard`, the quant advisor
     (`triggerQuantAdvisor`/`loadQuantAdvisor`/`renderQuantAdvisor`), community picks
     (`loadCommunityPicks`/`renderCommunityPicksTabs`/`renderCommunityPicksList`), the quantizer
     editor (`bindQuantizerEditor`/`openQuantizerEditor`/`renderEditorList`/`saveQuantizerEdits`/
     `resetQuantizersToDefaults`), and the HF search-controls cluster (`_reloadHfQuickPicks`,
     `browseHfAuthor`, `triggerHfFileFetch`, `refireHfSearch`, `bindHfSearchControls`,
     `fetchHfFiles`) into `spawn-wizard-hf-browse.js` (776 lines). `initSpawnWizard`'s inline
     discover-pills/quickpicks/scope-selector/sort-selector/quants-only setup block (was lines
     355-405) was wrapped into a new exported `initHfBrowseWidgets()` in the new module and
     `initSpawnWizard` now just calls it — `bindHfSearchControls()` and `bindQuantizerEditor()`
     stay as direct calls from `initSpawnWizard`, just now imported rather than local. `getRecommendedQuant`
     and `_deriveMmprojSaveName` were deliberately left in the shell: the former is only used by
     `renderHardwareModelHeader` (step 11 territory), the latter is shared between the HF-download
     flow (step 9) and the mmproj section (step 12), not the browse cluster. Note: this repo already
     has a lower-level `hf-browse.js` (widget primitives — `hfSearch`, `hfListFiles`, discover-pills
     rendering, etc.) that both the shell and the new `spawn-wizard-hf-browse.js` import from; the
     names are similar by design (same `spawn-wizard-` prefix convention as other extracted modules)
     but are two different files.
     Two new shell exports were needed for cross-module use: `ensureGpuVramFetched()` (wraps the
     `if (!cachedVram) await fetchGpuVram()` check so the new module doesn't need direct access to
     the shell's private `cachedVram`/`fetchGpuVram`) and `buildHeuristicArch` (was already used by
     the quant advisor for `global_head_dim`). Also exported: `showValidationError`,
     `clearValidationError`, `refreshEngineRecommendation`, `selectWizardEngine`,
     `updateSelectedModelDisplay`, `inferParamBFromName`, `detectMtpFromName` — all pre-existing
     shell functions the new module calls. `triggerQuantAdvisor` and `triggerHfFileFetch` had to be
     re-exported back into the shell (imported from the new module) since they're still called from
     shell-resident functions (`onModelPathChanged`, `doIntrospect`, and `bindEvents`'s HF repo-input
     blur/Enter handlers) — caught both by ESLint's `no-undef` on the first pass, not a runtime bug
     this time. Validated: `npm run validate-js` clean, ESLint clean on both files, `cargo build`
     succeeded, focused Playwright 34/34 passed.
9. `spawn-wizard-hf-download.js` (418-490, 1703-1721, 2029-2216) — download-panel wiring, quant options, companion mmproj download completion. Do immediately after 8 (interleaved in `initSpawnWizard`).
   **Done 2026-08-02**: extracted `spawn-wizard-hf-download.js` (~205 lines). Owns
   `bindHfDownloadPanel()` (replaces the inline download-panel button wiring that lived in
   `initSpawnWizard` — start/use-hf/cancel/open-settings button handlers plus the
   `settings-applied` listener that refreshes the download destination), `onHfDownloadComplete`
   (the `hfStartDownload` `onComplete` callback), `_startCompanionMmprojDownload`, and
   `_maybeAssociateCompanionOnMainFailure`, plus the module-local `_dlCurrentId`/
   `_mmprojCompanionId`/`_mmprojCompanionLocalPath` state — all three were previously shell-module
   state and are now fully contained in the new module since every read/write site moved with them
   (verified via grep before deleting). `initSpawnWizard` now just calls `bindHfDownloadPanel()`.
   Deliberately left in the shell: `getRecommendedQuant` and `_fetchAndShowQuantOptions` (both
   candidates for step 11's hardware-model cluster, not download-specific), and `_deriveMmprojSaveName`
   (shared with the not-yet-extracted step 12 mmproj cluster, now exported from the shell for both
   step 9 and step 12 to import). New shell exports added for this step: `STEP_LABELS`,
   `refreshHfTokenState`, `showStep`, `updateModelInputVisibility`, `_deriveMmprojSaveName`,
   `renderMmprojSection`, `guessQuantFromName` — the last one had already been used inside the module
   in an unexported form and its absence broke module resolution for the *entire* `spawn-wizard.js`
   graph (a `SyntaxError` on non-existent named export fails module load, not just the call site),
   which cascaded into all three focused Playwright specs failing on unrelated tests until the export
   was added — a reminder that missing named exports in an ES module fail hard at parse/link time for
   every consumer, not just the one importing the missing name. Also trimmed `hfCancelDownload` out of
   the shell's `hf-browse.js` import (no longer called from the shell; `hfStartDownload`,
   `hfShowDownloadPanel`, `hfHideDownloadPanel` remain since still used at other call sites, e.g. the
   quant-swap re-download flow). Validated: `npm run validate-js` clean, ESLint clean on both files,
   `cargo build` succeeded, focused Playwright 34/34 passed.
10. `spawn-wizard-hf-tags.js` (6546-7265) — HF tag mapping/normalization/pill UI; owns `_tagsRowOrigin` (needs `resetTagsRowOrigin()` mutator).
    **Done 2026-08-02**: extracted `spawn-wizard-hf-tags.js` (734 lines). Owns the full HF tag
    cluster — `HF_TAG_MAP`, `HF_TAG_BLOCKLIST`, `HF_TAG_BLOCK_PATTERNS`, `HF_CATEGORY_LABEL`,
    `ALL_KNOWN_TAGS`, `_normaliseTag`, `_isBlockedHfTag`, `_hfTagsToCategories`,
    `_fetchHfTagsWithBaseModel`, the module-local `_tagsRowOrigin` state plus its
    `resetTagsRowOrigin()` mutator, `_openHwRepoEditor`, `_refreshHwTagsRow`, `_renderHwTagPills`,
    `_saveHwModelTags`, `_openHwTagPicker`, and `_appendTagPill`. Only `resetTagsRowOrigin`,
    `_refreshHwTagsRow`, `_openHwRepoEditor`, and `_openHwTagPicker` needed to be exported —
    the rest are purely internal to the cluster (confirmed via grep -c usage counts across the
    shell before deleting). `_openHwRepoEditor` and `_openHwTagPicker` are called from shell code
    outside the extraction boundary (`renderHardwareModelHeader`, `bindEvents`) and had to be
    re-imported back into the shell, following the same re-export pattern used in steps 6 and 8.
    New shell exports added for this step: `_hfFilesPost`, `_extractQuantLabel` (both needed by
    the new module). A second-order dependency surfaced: `spawn-wizard-hf-origin.js` (extracted in
    step 5) was importing `_refreshHwTagsRow`/`resetTagsRowOrigin` from `./spawn-wizard.js` — since
    both moved to the new module, that import was repointed to `./spawn-wizard-hf-tags.js` directly
    rather than round-tripping through the shell. Also fixed a stale direct-mutation bug: a
    model-path-reset flow in the shell did `_tagsRowOrigin = '';` directly, which no longer compiles
    once the variable is module-private to `spawn-wizard-hf-tags.js` — replaced with a call to the
    exported `resetTagsRowOrigin()` mutator (same pattern as `_lastQuantSearchFile`/
    `_quantSwapSearching` will need in step 11). Validated: `npm run validate-js` clean, ESLint clean
    on all three touched files, `cargo build` succeeded, focused Playwright 34/34 passed.
11. `spawn-wizard-hardware-model.js` (5993-6182, 7266-7632) — hardware-step model header + quant-swap discovery; owns `_lastQuantSearchFile`/`_quantSwapSearching` (needs `resetQuantSwapSearchState()` mutator).
    **Done 2026-08-02**: extracted `spawn-wizard-hardware-model.js` (~558 lines) from three
    non-contiguous shell segments — `getRecommendedQuant`/`_fetchAndShowQuantOptions` (had drifted
    far from the rest of the cluster, sitting right after `getAuthHeaders`), `renderHardwareModelHeader`,
    and the full quant-swap discovery block (`_autoDiscoverLocalModelQuants`,
    `_showQuantSwapCandidateList`, `_showQuantSwapManualInput`, `_renderQuantSwapActions`, plus the
    module-local `_lastQuantSearchFile`/`_quantSwapSearching` state). Added the anticipated
    `resetQuantSwapSearchState()` mutator and used it to replace three shell call sites that had been
    directly reassigning `_lastQuantSearchFile`/`_quantSwapSearching` (in `resetWizardState`, a
    model-path-reset flow, and the quant-swap-trigger click handler) — same pattern as step 10's
    `_tagsRowOrigin` fix. Deliberately left in the shell as shared utilities: `_hfFilesPost` and
    `_extractQuantLabel` (both consumed by `spawn-wizard-hf-tags.js` as well, so keeping them
    shell-resident avoids a three-way import tangle), and `_modelStemForSearch`/`_autoFindMmprojRepo`
    (despite sitting inside the same source region, these are mmproj-specific — used by the
    not-yet-extracted step 12 cluster — not hardware-model). New shell exports added: none required
    (the four extracted functions needed re-export back into the shell instead, see below).
    `renderHardwareModelHeader`, `_autoDiscoverLocalModelQuants`, `_renderQuantSwapActions`, and
    `resetQuantSwapSearchState` are all called from shell code outside the extraction boundary
    (`bindEvents`, `resetWizardState`, a model-path-reset flow) and were re-imported back into the
    shell, following the step 6/8/10 re-export pattern. Two second-order dependencies surfaced:
    `spawn-wizard-hf-origin.js` (step 5) and `spawn-wizard-hf-tags.js` (step 10) both imported
    `renderHardwareModelHeader` (and `_fetchAndShowQuantOptions` for hf-origin) from `./spawn-wizard.js`
    — both were repointed to import directly from `./spawn-wizard-hardware-model.js`. This introduces
    a genuine circular import: `spawn-wizard-hardware-model.js` imports `_openHwRepoEditor`/
    `_refreshHwTagsRow` from `spawn-wizard-hf-tags.js`, which in turn imports `renderHardwareModelHeader`
    back from `spawn-wizard-hardware-model.js`. This works because all bindings involved are hoisted
    function declarations invoked only at runtime (inside event handlers/renders), never during
    top-level module evaluation — confirmed safe by the full 34/34 focused Playwright pass, but worth
    watching if either module gains top-level side effects later. Validated: `npm run validate-js`
    clean, ESLint clean on all four touched files, `cargo build` succeeded, focused Playwright 34/34
    passed.
12. `spawn-wizard-mmproj.js` (6183-6249, 6444-6525, 7633-7902) — mmproj recommendation/render/HF-fetch cluster.
    **Done 2026-08-02**: extracted as `spawn-wizard-mmproj.js` (423 lines) from three non-contiguous
    shell segments interleaved around two islands that stayed in the shell — the MTP-draft trio
    (`_bestDraftForModel`/`_checkGemma4MtpDraft`/`_renderDraftCandidatePills`, reserved for step 13)
    and the shared-utility trio (`_modelStemForSearch`/`_hfFilesPost`/`_extractQuantLabel`, consumed by
    both mmproj and MTP-draft clusters so kept shell-resident). Moved: the mmproj name-matching helpers
    (`_mmprojQuantLabel`, `_preferredMmprojQuant`, `_isRecommendedMmproj`, `_mmprojPracticalRank`,
    `_bestMmprojForModel`), `renderMmprojSection`, and the HF auto-search/manual-fetch/download cluster
    (`_autoFindMmprojRepo`, `_renderMmprojDownloadFromHf`, `_showMmprojHfFetchForm`,
    `_downloadMmprojFromHf`, `_pollMmprojDownload`). New shell exports added so the new module could
    import them: `_modelStemForSearch` and `_inferFamilyFromName` (both were previously module-private).
    `_deriveMmprojSaveName` stayed shell-resident (already exported, shared with step 9's
    `spawn-wizard-hf-download.js`). Two functions needed re-export back into the shell:
    `renderMmprojSection` (called from two shell sites — a model-load block and a local-directory-scan
    block) and `_bestMmprojForModel` (called from a local-directory mmproj-autopick block). One
    second-order dependency fix: `spawn-wizard-hf-download.js` (step 9) imported `renderMmprojSection`
    from `./spawn-wizard.js` — repointed to import directly from `./spawn-wizard-mmproj.js`. This adds
    another circular import (`spawn-wizard-mmproj.js` imports shared utilities from `spawn-wizard.js`,
    which imports `renderMmprojSection`/`_bestMmprojForModel` back) — same hoisted-function-declaration
    pattern already validated safe in step 11, confirmed again by the 34/34 Playwright pass. Note: the
    focused Playwright specs have since moved to `tests/ui/core/*.spec.js` (must run
    `npx playwright test` from inside `tests/ui/`, not the repo root, since `tests/ui` has its own
    Playwright install/config — running from the root picks up a different `@playwright/test` and fails
    with "did not expect test.describe() to be called here"). Validated: `npm run validate-js` clean,
    ESLint clean on all three touched files, `cargo build` succeeded, focused Playwright 34/34 passed.
13. `spawn-wizard-mtp-draft.js` (6250-6443, 7903-8094) — **llama.cpp GGUF draft-model/MTP-tensor** logic; explicitly NOT the Rapid-MLX sidecar flow already in `spawn-wizard-rapid-mlx.js` — per the MTPLX audit these are three unrelated speculative-decoding mechanisms and must stay named/documented as distinct.
    **Done 2026-08-02**: extracted as `spawn-wizard-mtp-draft.js` (378 lines) from two shell segments
    (line numbers in the plan text above were stale by this point; re-derived fresh against the
    post-step-12 6,303-line shell) separated by the shared-utility island left behind in step 12
    (`_modelStemForSearch`/`_hfFilesPost`/`_extractQuantLabel`, which stayed shell-resident). Moved:
    the draft-model matching/check/pill trio (`_bestDraftForModel`, `_checkGemma4MtpDraft`,
    `_renderDraftCandidatePills`) and the MTP hardware-step section (`renderMtpSection`). Also moved
    the `_mtpUserConfigured` exported `let` flag itself (previously shell-resident specifically pending
    this step, per its own in-code comment) since every read/write site was inside the extracted
    `renderMtpSection`. Three functions needed re-export back into the shell (call sites at model-load,
    hardware-step-nav, and template-restore blocks): `renderMtpSection`, `_bestDraftForModel`,
    `_checkGemma4MtpDraft`. One second-order dependency fix: `spawn-wizard-binary-prereq.js` (step 3)
    imported `_mtpUserConfigured` from `./spawn-wizard.js` — repointed to import directly from
    `./spawn-wizard-mtp-draft.js`. Module header comment documents the three-way distinction from the
    Rapid-MLX sidecar flow and the future MTPLX runtime, as directed. Validated: `npm run validate-js`
    clean, ESLint clean on all three touched files, `cargo build` succeeded, focused Playwright 34/34
    passed (run from inside `tests/ui/`, per the invocation fix discovered in step 12).
14. `spawn-wizard-vram-display.js` (5351-5787) — hottest render path (`updateVramDisplay`, called from 8 sites); do late once its callees already live in modules.
    **Done 2026-08-02**: extracted as `spawn-wizard-vram-display.js` (~610 lines) from two shell
    segments (re-derived fresh against the post-step-13 5,940-line shell; original plan line numbers
    were stale). Segment A (`updateMlockWarning` → `renderScenarioCards`, contiguous) held the whole
    entry-point cluster: `updateMlockWarning`, `updateVramDisplay`, `setSegWidth`,
    `updateMoeSliderVisuals`, `updateContextRailSummary`, `renderScenarioCards`. Segment B
    (`updateLegacyVramPill` → `_showMetalLimitFallback`, contiguous, separated from segment A by
    `_refreshThreadsHint`/`_fetchSystemInfoAndRefreshHints`/the mmproj-era shared-utility island/
    `_updateSpecHint`, all of which stayed shell-resident) held `updateLegacyVramPill`,
    `applyMetalGpuLimit`, `_showMetalLimitFallback`. All of segment B's functions turned out to be
    called only from within segment A, so nothing from segment B needed re-export back to the shell.
    Confirmed the module is backend-agnostic by design: all VRAM math comes from the single
    `/api/vram-estimate` backend endpoint via `scheduleEstimate`; this module only renders whichever
    fields the response contains (llama.cpp's unified `kv_cache_bytes` vs. Rapid-MLX's active/
    retained KV split and `mlx_prefix_cache_bytes`), so it needs no per-engine branching and will not
    need to be split further when MTPLX lands — new engines just add new response fields and a new
    conditional render branch, same pattern as the existing Rapid split.
    Only two functions needed re-export back into the shell (7 external call sites for
    `updateVramDisplay`, 1 for `updateMoeSliderVisuals`): `updateVramDisplay`, `updateMoeSliderVisuals`.
    Exported a set of previously shell-private helpers/state for the new module to import back
    (deliberately left in the shell rather than moved, since they're either used elsewhere in the
    shell too or belong to a later step): `metalCap`, `suggestedMetalLimitMb`, `cachedRamTotal`,
    `cachedRamUsed`, `cachedMetalGpuLimitMb`, `maybeResetHardwareStepScroll`,
    `maybeRestoreHardwareStepScroll`, `fetchGpuVram`, `fetchMetalGpuLimit`, and — pre-emptively for
    step 15 — `CTX_TARGETS`, `updateCtxModelMaxHint`, `updateCtxQuickPickActive`,
    `updateCtxTrainWarning` (these will move to `spawn-wizard-context-fit.js` in step 15, at which
    point this module's import of them gets repointed, same second-order-dependency pattern as step
    13). No second-order dependency fixes were needed elsewhere — grepped every moved/exported symbol
    across all other feature modules and found no external consumers besides two unrelated
    identically-named locals in `models.js`/`setup-view.js`. Validated: `npm run validate-js` clean,
    ESLint clean on both touched files, `cargo build` succeeded, focused Playwright 34/34 passed
    (52.0s, run from inside `tests/ui/`).
15. `spawn-wizard-context-fit.js` (5788-5992, 8309-8450) — context rail summary/quick-picks; owns `CTX_TARGETS`.
    **Done 2026-08-02**: extracted as `spawn-wizard-context-fit.js` (146 lines) from a single contiguous
    shell segment (re-derived fresh against the post-step-14 5,272-line shell; original plan line
    numbers were stale). The cluster was the whole "Context quick-picks" section: `bindCtxQuickPicks`,
    `updateCtxQuickPickActive`, `updateCtxModelMaxHint`, `updateCtxTrainWarning`, `CTX_TARGETS`,
    `showCtxFitWarning` — all six symbols, so nothing context-related was left behind in the shell.
    Five needed re-export back into the shell (`bindCtxQuickPicks` for the `bindEvents()` call site;
    `updateCtxQuickPickActive`/`updateCtxModelMaxHint`/`updateCtxTrainWarning`/`showCtxFitWarning` for
    remaining shell call sites in the model-load and autosize-result blocks); `CTX_TARGETS` itself has
    no shell call site anymore, so it's imported by the new module only, not re-exported to the shell.
    Confirmed the pre-emptive second-order fix anticipated in step 14: `spawn-wizard-vram-display.js`'s
    import of `CTX_TARGETS`/`updateCtxModelMaxHint`/`updateCtxQuickPickActive`/`updateCtxTrainWarning`
    was repointed from `./spawn-wizard.js` to `./spawn-wizard-context-fit.js`. This creates a
    circular import between the two new modules (context-fit imports `updateVramDisplay` from
    vram-display, vram-display imports the four context symbols back) — safe under ES modules since
    all cross-references are used only inside function bodies, never at module-evaluation time. No
    other second-order dependency fixes were needed — grepped every moved/exported symbol across all
    other feature modules and found only one unrelated identically-named local (`updateCtxTrainWarning`
    in `models.js`). Validated: `npm run validate-js` clean, ESLint clean on all three touched files,
    `cargo build` succeeded, focused Playwright 34/34 passed (52.1s, run from inside `tests/ui/`).
16. `spawn-wizard-autosize.js` (8095-8291) — Metal GPU limit application + auto-size trigger; optional merge into 15.
    **Done 2026-08-02**: extracted as `spawn-wizard-autosize.js` (200 lines) from two non-contiguous
    shell segments (re-derived fresh against the post-step-15 5,137-line shell; original plan line
    numbers were stale and referred to functions already relocated). Confirmed
    `applyMetalGpuLimit`/`_showMetalLimitFallback` had already moved into `spawn-wizard-vram-display.js`
    during step 14, so this step's actual scope was just the two functions in the auto-size call chain:
    `clampAutoSizeResultToSizingMath` (validates/binary-searches the auto-size result against
    `/api/vram-estimate`, the same endpoint the live hardware-step display uses) and `triggerAutoSize`
    (the button handler, its only caller — a shell `bindEvents()` listener re-exported and repointed to
    the new module). Only `triggerAutoSize` needed re-export back to the shell;
    `clampAutoSizeResultToSizingMath` had no shell callers left. `doIntrospect` was exported from the
    shell for the first time (previously shell-private) since it's called both here and from a shell
    debounce timer, so it stays shell-resident but importable. `showCtxFitWarning` lost its only shell
    call site when `triggerAutoSize` moved, so its now-unused import was dropped from the shell
    (still exported by `spawn-wizard-context-fit.js` for the new autosize module and any future
    consumer). No second-order dependency fixes were needed elsewhere — grepped
    `triggerAutoSize`/`clampAutoSizeResultToSizingMath` across all other feature modules (no hits) and
    confirmed the two files mentioning `doIntrospect` in comments only, not imports. Validated:
    `npm run validate-js` clean, ESLint clean on both touched files, `cargo build` succeeded, focused
    Playwright 34/34 passed (54.2s, run from inside `tests/ui/`). Final line counts: `spawn-wizard.js`
    = 4,951 lines, `spawn-wizard-autosize.js` = 200 lines.
17. `spawn-wizard-review-step.js` (8560-9503) — summary/sampling-sync/save-preset/params-review; shell must **re-export `buildPresetPayload`** (Playwright test contract).
    **Done 2026-08-02**: extracted as `spawn-wizard-review-step.js` (957 lines) from a single
    contiguous shell segment (re-derived fresh against the post-step-16 4,951-line shell — the whole
    "Model-specific sampling defaults" through "Preset parameters review (step 5)" run of sections,
    ending right before "Spawn config preview card (step 6)", which stays shell-resident for step 18).
    By far the largest single-segment extraction of the 18-step plan. Contained: model-sampling-preset
    application/pills, `renderSummary` (step 4 summary card), the full sampling-field sync/bind cluster
    (`_syncSamplingFields`, `_structuredOutputMode`, `_syncStructuredOutputFields`, `_syncThinkingFields`,
    `_bindThinkingFields`, `_bindSamplingFields`), `saveAsPreset`, `buildPresetPayload`, `runHealthCheck`
    (found to have no live caller anywhere — `dom.healthCheckBtn` is never assigned via
    `getElementById` and nothing binds a click handler to it; left in place as dead code rather than
    removed, since cleanup wasn't in scope), and `_renderPresetParamsStep` (step 5 params table). Six
    functions needed re-export back to the shell: `renderSummary`, `_renderPresetParamsStep`,
    `_bindSamplingFields`, `saveAsPreset`, `_fetchAndApplyModelSamplingDefaults`, and — separately, via
    its own `export { buildPresetPayload } from './spawn-wizard-review-step.js'` line since the shell
    itself has no remaining call site for it — `buildPresetPayload`, satisfying the Playwright
    test-contract requirement (`tests/ui/core/{spawn-wizard,phase7-presets}.spec.js` dynamically
    `import()` it from `/js/features/spawn-wizard.js`). New module imports directly from their owning
    modules rather than through shell re-export, per established pattern: `detectModelFamily` from
    `spawn-wizard-chat-template.js`, `syncRapidSpeculativeFields` from `spawn-wizard-rapid-mlx.js`,
    `kvBpe`/`formatCtx`/`formatGB` from `spawn-wizard-format.js`,
    `buildEstimateBody`/`rapidEstimatePolicyFromWizardHardware` from `vram-estimate.js`, `showToast`
    from `toast.js`; `showStep`/`buildSpawnPayload`/`effectiveAvailBytes`/`getModelBytes`/
    `getSizingArch`/`isUnifiedMemory`/`dom`/`wizardState` from the shell (all already exported, none
    needed new `export` keywords). No second-order dependency fixes were needed — grepped every
    moved/exported symbol across all other feature modules and found zero external consumers.
    Validated: `npm run validate-js` clean, ESLint clean on both touched files, `cargo build` succeeded,
    focused Playwright 34/34 passed (57.5s, run from inside `tests/ui/`, including the
    "review step exposes structured output and full sampling defaults" and both `buildPresetPayload`
    test-contract specs). Final line counts: `spawn-wizard.js` = 4,017 lines,
    `spawn-wizard-review-step.js` = 957 lines.
18. `spawn-wizard-spawn.js` (9504-10079) — command preview/spawn-server/payload building; shell must **re-export `buildSpawnPayload`, `launchPortForPayload`, `supportsTunePanelForPayload`** (test contract — `tests/ui/**` dynamically imports these from `/js/features/spawn-wizard.js`).
    **Done 2026-08-02**: extracted as `spawn-wizard-spawn.js` (601 lines) from the shell's entire
    remaining tail (old lines 3433-4017, everything from "Spawn config preview card (step 6)" through
    EOF) — the final decomposition step. Contains: `_renderCommandPreview` (internal), the exported
    command-preview/submission trio (`_renderSpawnConfigCard`, `spawnServer`, `resetSpawnStatus`), the
    three Playwright test-contract payload builders (`buildSpawnPayload`, `launchPortForPayload`,
    `supportsTunePanelForPayload`), an internal `waitForSpawnReadiness` (called only at its own call
    site — confirmed via grep this is an unrelated, pre-existing same-named function from
    `spawn-readiness.js` used by `attach-detach.js`/`presets.js`; a naming coincidence predating this
    refactor, not a shared dependency), and small internal helpers (`_u32`, `_threadsValue`,
    `_modelNameLower`, `_imageMinTokens`/`_imageMaxTokens`, the status-message helpers). Two real bugs
    surfaced during extraction: (1) ESLint `no-undef` on `_binaryReady` — fixed by importing it directly
    from `spawn-wizard-binary-prereq.js` (its owning module) rather than through the shell, then removing
    the now-dead `_binaryReady` import left behind in the shell; (2) a genuine second-order dependency —
    `spawn-wizard-review-step.js` (step 17) still imported `buildSpawnPayload` from `./spawn-wizard.js`,
    but it had just moved here, so its import was repointed to `./spawn-wizard-spawn.js` directly. This
    creates a second intentional circular-module relationship (`spawn-wizard-spawn.js` ↔
    `spawn-wizard-review-step.js`, via `buildPresetPayload`/`buildSpawnPayload` cross-references), safe
    for the same reason as the step-15 circular pair: both references occur only inside function bodies,
    never at module-evaluation time. Validated: `npm run validate-js` clean, ESLint clean on all three
    touched files, `cargo build` succeeded. The full Playwright suite (268 tests, not just the three
    focused specs, since this is the final decomposition step) surfaced two additional issues, both now
    fixed and re-verified: (a) `core/performance.spec.js`'s JS-module-count baseline was stale (61 vs the
    now-correct 80-81 after 18 new modules) — updated via `cd tests/ui && LLAMA_MONITOR_UI_URL=http://127.0.0.1:<port> npm run update-baseline`
    against a manually-started `run-server.mjs` instance, `61 → 81`; (b) a genuine regression in
    `tests/ui/core/spawn-wizard.spec.js`'s "Spawn payload leaves fit parameters unset until the toggle is
    enabled" test — it dynamically imported `spawn-wizard.js?fit-payload-test=1` (a cache-busting query
    string, unique to this one test) to get an isolated module instance, then mutated that instance's
    `wizardState.hardware.fitEnabled` and called `buildSpawnPayload()`. Pre-decomposition this worked
    because both lived in the same module instance; post-decomposition `buildSpawnPayload` lives in
    `spawn-wizard-spawn.js`, which always imports the canonical (non-cache-busted) `./spawn-wizard.js`
    singleton — so the query-busted `wizardState` the test mutated was a different object than the one
    `buildSpawnPayload`'s closure actually reads, and `fit_enabled` came back `null` instead of `false`.
    Fixed by dropping the `?fit-payload-test=1` query string (the test already explicitly sets all three
    states itself, so it doesn't need a pristine fresh instance — every other `wizardState`-mutating test
    in the suite uses the plain cached import path for the same reason). This is a pattern worth
    remembering: any future test that dynamically imports a cache-busted `spawn-wizard.js?...` and expects
    it to share live state with a function now living in an extracted module will hit the same failure —
    prefer the plain import path unless a genuinely fresh module graph is required. Re-ran
    `core/performance.spec.js` + the three focused specs (36/36 passed, 54.4s) to confirm both fixes.
    Final line counts: `spawn-wizard.js` = 3,435 lines, `spawn-wizard-spawn.js` = 601 lines. **All 18
    steps of the spawn-wizard decomposition are now complete.**

Only steps 1-2 are parallel-safe; 3-18 must run sequentially against the shrinking monolith (shared
module-scoped state and `initSpawnWizard`/`bindEvents` mutations couple them). Validate after every
step: `npm run validate-js`, ESLint, and the focused Playwright suite on `LLAMA_MONITOR_TEST_PORT=17778`.
End state: shell shrinks to ~3.4k lines (state/DOM-cache/event-switchboard/step-nav/validation/engine
selection/memory detection/sizing kernel/status helpers + test-contract re-export shims) plus 18
feature modules. Merge candidates if file count feels excessive: 7↔8, 15↔16.

`static/js/features/spawn-wizard.js` is about 11,168 lines (pre-extraction figure; see above for the
post-extraction figure). Rapid is currently an overlay inside a
llama.cpp flow; CSS hides the reusable `wizard-main` and `hw-vram-sidebar` on step 2 even though the
server estimate is still calculated. Do not merely restyle `#rapid-hardware-panel`.

Create `static/js/features/spawn-wizard-mlx.js` as a capability/presentation controller and keep the
shared wizard shell in `spawn-wizard.js`:

- Shared: open/close, step navigation, use-case selection, engine/model/HF search, typed model source,
  server-authoritative estimate scheduling, persistent fit rail, preset/save/review/start lifecycle,
  validation/error focus, command preview, and evidence drawer.
- llama.cpp flow: GGUF quant selection, GPU layers, mmap/mlock, CPU MoE, llama batching, RoPE, llama
  speculative flags, and llama-only fit decisions remain isolated in the existing path (extract later
  only when behavior-preserving tests make it safe).
- MLX flow: model profile/capabilities, active KV/storage, retained cache, prefill/scheduler policy,
  thinking/sampling/parser protocol, server/safety, and qualified companions. Backend adapters own
  actual flag names/ranges; never reuse `RapidMlxConfig` as the future MTPLX contract.

Recommended MLX progression using the existing six-step shell:

1. Use case/profile (shared).
2. Engine + MLX model/HF selection (shared shell, MLX-specific qualification and immediate fit).
3. **Memory & Fit:** persistent estimator is the visual center; active memory and retained-cache
   choices are progressive cards, with weights/quant, active KV, retained cache, companion overhead,
   and safety headroom visible. Never hide the estimator/sidebar for MLX.
4. **Generation & Runtime:** sampling/thinking/protocol first; Cache & Performance and Server & Safety
   are organized subsections/disclosures, not one giant two-column advanced dump. Single-user
   defaults stay prominent; batching/concurrency and parked MTP remain advanced.
5. Save preset with the same MLX section vocabulary and final fit strip.
6. Review/start with memory composition, effective policy, deviations from recommended defaults,
   command preview/evidence, and launch blockers.

Behavior-preserving extraction candidates from `spawn-wizard.js`:

- ~~`_bindRapidMlxAdvancedControls`, `_syncRapidSpeculativeFields`, trust/pin/sidecar helpers,
  `_applyRapidMlxDefaults`, `_applyReasoningModeLock`.~~ **Done 2026-08-02** — moved to
  `spawn-wizard-mlx.js`.
- ~~`_fetchRapidMlxModelProfile`, `_scheduleRapidMlxProfileFetch`, `_renderRapidMlxProfileHints`.~~
  **Done 2026-08-02** — moved to `spawn-wizard-mlx.js`.
- ~~`renderRapidExclusionWarnings`~~ **Done 2026-08-02** — moved to `spawn-wizard-mlx.js`. Rapid
  request/config construction inside `buildSpawnPayload`'s `rapid_mlx` branch is still **not**
  extracted; it should become an adapter helper (e.g. `buildRapidMlxConfig(h, m)`) while shared
  payload submission stays in the shell.
- Preserve current control IDs until round-trip tests pass; introduce semantic MLX components before
  changing serialized contracts.

Required Wizard tests/captures:

- Keep server estimator parity and add assertions that the fit rail is visible on MLX steps 2–5 and
  updates for KV dtype, retained memory, prefill size, model/quant, and companion changes.
- Preserve `spawn-wizard.spec.js`, `phase7-presets.spec.js`, `rapid-phase7-fields.spec.js`, and
  command-preview coverage.
- Add a still-focused MLX scenario derived from `scenarioSpawnWizardRapidMlxGif`; do not rely on the
  GIF for design review. Captures must be full screen with backdrop, scroll the intended subsection
  into view, and use names matching the visible state.

#### Dashboard premium visual system: implement after Wizard/Preset contracts settle

Rapid telemetry currently parks the llama.cpp cards and renders uniform label/value cards through
`static/js/features/rapid-mlx-cards.js`. Available trustworthy signals are:

- runtime model/health/readiness/uptime;
- prompt and generation tokens/sec;
- running/waiting requests;
- Metal active/peak/cache memory;
- cache hit rate, entry count, and cache memory;
- completed requests, prompt/completion totals, scheduler steps;
- optional sanitized active-request state and normalized progress.

Rapid currently provides **no trustworthy TTFT history or speculative-acceptance/speedup metric**;
do not invent either and remove `observed TTFT` from visual requirements until the backend supplies
it. Recommended premium composition:

- hero throughput card with prompt/generation sparklines and current generation stage;
- unified-memory composition/pressure visual (stacked bar or ring) with active, cache, peak, and
  headroom semantics where a trustworthy capacity exists;
- queue/admission card with active/waiting pressure and active-request rail;
- prefix-cache card with hit-rate gauge, entries, memory, and clear unavailable/zero/stale states;
- cumulative work card with compact totals and deltas derived only from locally observed snapshots;
- runtime/effective-policy status strip with evidence/staleness grade.

Reuse the premium primitives/helpers in `cards-inference.css` and `dashboard-render.js` where their
semantics fit, or extract backend-neutral primitives. Do not copy llama DOM IDs while
`parkLlamaCards()` owns them. Prefer a capability-driven metric model so MTPLX can provide a subset
or superset without another dashboard rewrite. Client-side transient history may drive sparklines;
it must be labeled live-session history and must not masquerade as persisted server telemetry.

Dashboard capture/test corrections still required:

- `dashboard-rapid-mlx` must assert visual hierarchy, zero/unavailable/stale states, DOM stability,
  accessibility, dark/light, narrow, and reduced motion.
- `rapid-mlx-live` currently captures telemetry **before** chat, so the dashboard shot has zero totals;
  move/add a live dashboard capture after the request and before stop. Keep the stopped/historic frame
  separate. Its status assertion reported `model stopped: false` even though process inspection found
  no Rapid process; fix the assertion to inspect the active session/process contract rather than the
  managed-runtime installation status.

#### Capture-script cleanup remaining

- The scenario named `model-library` captures HF Download/discovery, not the actual Library tab.
  Either rename it to `model-discovery` and update help/docs, or make it capture the Library tab;
  `models-v2` is the current valid Library evidence.
- Never return to capture-element closeups for these redesign reviews. Use full-screen/modal backdrop,
  scroll to the intended section, wait for asynchronous estimator/telemetry state, and ensure artifact
  names describe what is visibly captured.

#### Final gates before the next checkpoint commit

After all three surfaces are complete, run the mandatory order from `AGENTS.md`: clippy, full Rust
tests, JS validation, lint, diff check, release build, formatting check, clean status. Then run the
full isolated release Playwright suite on port 17778 outside sandbox. Inspect all dark/light/narrow
artifacts manually. Run `scripts/check-unused-screenshots.sh`; artifacts remain ignored unless a
reference doc actually uses them. Use a conventional commit and do not push or label a PR without
explicit user direction.
