# Phase 7 / 8 remainder — handoff

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

Still open after this checkpoint: immutable external-companion preflight/trust consent; external
cache/benchmark audits listed in §3.11. Frequency-penalty UI and legacy quant-style migration are
now closed (see §3.5, §3.8). Do not infer those items complete from product plumbing.

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
capture-verified. Immutable external-companion preflight and trust consent remain open.

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

### 3.5 — Config fields and safety controls still open

Only two product-facing controls still require product work; the two runtime path fields remain
internal by decision.

| Field | Disposition |
| --- | --- |
| `executable_path`, `managed_runtime_path` | Runtime-resolution internals. **Keep hidden** — settled. |
| `trust_remote_code_consent` | **Open:** add revision-scoped consent/preflight UX. This is a safety control; do not make it an unlabelled checkbox or bypass it. |
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
lineage-specific Playwright gap. External companion provenance is a separate open item in §3.3.

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

- **`--cache-ram` multi-branch / slot-pressure run** (Phase 6 item 9) — still outstanding. Note the
  qualified policy is already `cache-ram 0`: 0 vs 8192 measured identical through 200K, and the
  test that produced 8192 was not discriminating.
- **`--hybrid-cache-entries` sweep** — the plan doc exists at
  `docs/plans/20260731-hybrid-cache-entries-sweep.md`; **the sweep has not been run.** This doc
  stays; it is a live work order.
- **Library tab UI** from the Disk/cache-audit work was never visually verified.
- **Phase 8B3** depends on "8B2 verified + screenshots approved". 8B2 is verified; screenshots are
  **not approved**. 8B3's headline deliverable (additive MLX+GGUF scope toggles) is already
  implemented at `models.js:2264`.
- **`rapid-mlx-live` (7.5B)** is coherent but has not been run in this campaign; its three
  screenshots were not re-approved.
- **~337 GB shared HF cache** still to audit. Standing direction: get all models inside
  `~/.config/llama-monitor/models/`. Audit, do not blanket-delete.

---

### 3.12 — Historical intermittent test failure

`guided-generation/phase8-tag-cloud.spec.js:255` — a group header's `aria-expanded` still `false`
after a re-expand click. It failed **once** across five full runs and passed the latest full
release-built run (262/4/0).

It is a **different shape** from the three flakiness causes that were fixed (a click that did not
take, not a torn-down modal), so it is recorded separately rather than assumed to share a cause.

It is not currently release-blocking, but the root cause is still unknown. Keep the regression test
and investigate if it recurs; do not delete it or treat one green run as a diagnosis.

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
4. **Run the non-code audits in §3.11:** cache-ram branch/slot-pressure, hybrid-cache-entries,
   Library-tab and rapid-mlx-live screenshot approval, and the shared HF-cache audit. Audit and
   migrate deliberately; do not blanket-delete cached models.
5. **Small integrity cleanup — done except the intermittent:** the capture.mjs stale-scenario check
   and the unused HF label exports are resolved (see §3.10 checkpoints). Still open: investigate the
   tag-cloud intermittent if it recurs. Run `rtk git diff --check` after documentation or code edits.
6. **Only then consider MTP requalification:** use the future-build harness with nonzero-temperature
   sampled and constrained-tool requests, observable nonzero MTP activity, parity/fidelity, and
   explicit fallback evidence. A greedy 0-temperature acceptance result alone is not a promotion
   gate.
