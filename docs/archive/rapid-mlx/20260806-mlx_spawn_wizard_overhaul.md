# MLX Spawn Wizard Overhaul — Unified Loader Architecture

Date: 2026-08-06 (rev 2) · Branch: `feat/rapid-mlx-integration` · Status: **EXECUTED — archived**

Phases 1-5 (findings, unified registry, MLX group translation, capture-harness migration,
reference documentation) are complete and committed. Phase 6 (MTPLX readiness check) is
tracked separately going forward.

Rev 2 supersedes the first pass. The grounded findings in §1 are carried forward and extended; the "Options A/B/C" menu is **removed** — there is one recommended architecture (§3), executed in phases (§5). Every open question from rev 1 is resolved in §2 from code and archive evidence.

---

## 0. Corrections to the working assumptions

- **Six steps, not 3–4.** `static/index.html:3521-3533`: 0 How it works · 1 Choose model · 2 Hardware & memory · 3 Settings · 4 Review · 5 Start server.
- **MLX step 3 is already grouped at runtime.** `static/js/features/spawn-wizard-mlx-ia.js` relocates the flat grid into 3 supersections / 8 groups (`SUPERSECTIONS` L8-24, `GROUPS` L26-77) from `renderEngineSelection()` (`spawn-wizard.js:2118`). The defects are narrower than "no structure": no tier gating (only `companions` is `collapsible`, L69) and wrong DOM parent (§1.1).
- **New, and important:** the first pass said MLX family detection "depends on a local path." It is worse than that — it is **broken on every path** (§1.7).

---

## 1. Findings from code investigation

### 1.1 Blank right sidebar on MLX step 3 — a DOM-parenting bug

`#spawn-rapid-advanced-fields` (`index.html:3949`) is nested in `#rapid-hardware-panel`, a **direct child of `#wizard-step-2` at `index.html:3941`, before `<div class="wizard-main">` opens at `4205`**. The inline comment at `3948` admits it: "moved here from wizard-main so they're visible."

The whole MLX config surface therefore sits *above* the `main | sidebar` grid. The sidebar renders one screen tall beside nothing, then ~1,500 lines of MLX controls scroll past with no memory feedback. Fix = re-parent into `.wizard-main` + `position: sticky` sidebar rail. Not a missing feature.

### 1.2 The three "Context fit modes" cards show identical values

`renderScenarioCards()` (`spawn-wizard-vram-display.js:416-500`) hardcodes three **llama.cpp KV-quant** scenarios differing only in `ctk`/`ctv` (`q8_0`/`q4_0`/`f16`, L426-455). Rapid-MLX has no `ctk`/`ctv`; the MLX branch spreads `rapidEstimatePolicyFromWizardHardware(hw)` at L478 and ignores both fields — all three requests are byte-identical. It also early-returns unless `modelBytes` **and** `model_path` are set (L417); an MLX HF-repo selection sets neither (`spawn-wizard-hf-browse.js:152-162` sets `rapidMlxSource` + `modelBytes`, never `path`).

The rev-1 guess that the MLX axes are `kv_cache_dtype × turboquant_mode` is **wrong**. See §2.4 — both axes are inert in this build.

### 1.3 Step 2 has no memory sidebar for MLX

The step-2 sidebar (`index.html:3868-3936`) is ctx pills → VRAM bar → **Quant advisor**. The advisor is GGUF-intrinsic (`loadQuantAdvisor()`, `spawn-wizard-hf-browse.js:186+`), gated on `wizardState.model.paramB > 0`; on the MLX HF path `paramB` comes from `m.param_b` (L154), usually absent for MLX repos, so `triggerQuantAdvisor()` no-ops and the column is empty. The ctx pills *are* wired for both engines (`initHfBrowseWidgets`, L98-108) but lose their container context.

An MLX repo **is already a specific quant** — there is no ladder to advise on. The answer is a different sidebar body, not a fixed advisor. §2.7 specifies its content.

### 1.4 Chat template on MLX — backend works, frontend never triggers it

Backend path is sound:
- `RapidMlxAdapter.chat_template_file: Option<String>` — `src/inference/rapid_mlx/mod.rs:282`.
- `build_launch_argv()` — `mod.rs:1043-1080`: calls `model_resolver::create_template_overlay(...)` and swaps `launch_argument` to the overlay dir. Failure is **non-fatal**: warning into `launch.warnings` (`mod.rs:897,916`).
- `create_template_overlay()` — `model_resolver.rs:1784-1839`: symlinks every file into `~/.config/llama-monitor/rapid-mlx/template-overlays/<hash[..16]>/` and writes `chat_template.jinja`, which Rapid-MLX's `_apply_chat_template_sidecar()` prefers (comment L1834).
- HF-repo sources work: `RapidMlxModelSource::HuggingFaceRepo` resolves via `resolve_hf_snapshot()` to a local snapshot dir before `launch_argument` is set (`model_resolver.rs:663-683`), so `is_dir()` at L1798 passes.

Four holes:

1. **`Alias` sources degrade silently.** `RapidMlxModelSource::Alias` sets `launch_argument = value` — a bare string (`model_resolver.rs:690-702`). `create_template_overlay` bails at `model_resolver.rs:1798` (`!model_path.is_dir()`); the user gets a buried launch warning and the native template. Resolved in §2.2.
2. **The MLX HF-repo path never calls `autoInstallChatTemplate()`.** `spawn-wizard-hf-browse.js:159-163`: `if (m.format === 'mlx') { … selectWizardEngine('rapid_mlx', true); updateSelectedModelDisplay(); } else { fetchHfFiles(m.id); }`. `autoInstallChatTemplate()` is reached only from the GGUF file-list callback (`hf-browse.js:932`) and `onModelPathChanged()` (`spawn-wizard.js:2377`, local/import only — correctly, per the comment at 2373-2376). `#chat-template-section` is `display:none` in markup (`index.html:3855`) and is only unhidden by `_renderChatTemplateStatus()` (`spawn-wizard-chat-template.js:256`). **The chat-template panel is invisible for every MLX HF-repo selection.**
3. **Family detection for MLX is dead code** — see §1.7.
4. **Command preview lies.** `src/web/api/rapid_mlx_runtime.rs:2727, 2809, 2871` build preview adapters with `chat_template_file: None`, so the preview shows the raw model dir where the real launch shows the overlay dir — violating the stated contract at `mod.rs:1036-1042`.

### 1.5 HF file-browser scope-toggle default

- Component: `hfCreateScopeSelector({ container, onChange })` — `hf-browse.js:1560`, reads initial state from container dataset attrs at L1577-1579 (`data-hf-scope-mlx`/`-gguf`/`-all`).
- Caller: `initHfBrowseWidgets()` — `spawn-wizard-hf-browse.js:70-85`. The default is **platform**-based: `const isMac = navigator.platform?.includes('Mac'); hfBrowseState.mlxActive = !!isMac;` with `data-hf-scope-gguf` unconditionally `'1'`. On Apple Silicon both scopes are on regardless of engine.
- Timing is fine: `selectWizardEngine()` (`spawn-wizard.js:2070`) is the single mutation point and already fires a refresh cascade (L2089-2097).
- Reverse edge: `hf-browse.js:159-161` auto-switches the engine to `rapid_mlx` when an MLX result is picked → engine→scope coupling creates a feedback loop. Guarded by `userTouchedScope` + not re-firing search on programmatic scope changes.
- `resolveScope()` (`hf-browse.js:355-372`) is additive (`mlx && gguf → 'both'`), so an engine-driven default is a pure narrowing, not a semantic change.

### 1.6 "Detected: unknown"

`spawn-wizard-rapid-mlx.js:609-610` — `toolDetected.textContent = profile.tool_format || 'unknown'`. `scheduleRapidMlxProfileFetch` has no tri-state. Render `Detecting…` in flight, `Not reported by this model` on a null result, and `unknown` only when the API explicitly says so. Same for the reasoning-parser span (`index.html:4006`) and the hybrid-mode select.

### 1.7 **NEW — MLX family detection is broken end-to-end, not merely weak**

Three independent defects stack:

1. **Response-shape mismatch.** `spawn-wizard-chat-template.js:87` reads `meta.model_type || meta.config?.model_type`. The backend returns
   `{"ok": true, "model_path": …, "data": {"recursive_size_bytes":…, "config": {"model_type":…}, "has_vision_adapter_in_index":…}}`
   (`src/web/api/models.rs:1592-1598`, config nested under `data.config` at L1543). There is no top-level `model_type` and no top-level `config`. **`modelType` is always `undefined`**, so the MLX introspection branch (L78-92) never resolves a family — *even with a local path*. Rev 1's framing ("depends on a local path") understated this: the local path doesn't help either.
2. **Gated behind `if (localPath)`** (L76) — so HF-repo selections never even attempt it.
3. **The family vocabulary is two entries.** `communityFamilyFromGgufArchitecture()` (`chat-template-registry.js:68-73`) returns only `qwen` or `gemma4`. Everything else falls through to HF-tag heuristics (`_communityFamilyFromHfTags`, `spawn-wizard-chat-template.js:36-45`) which check the same two strings.

Consequence: for MLX, the "recommended template" path is effectively never reached; users always land in `embedded`. This is a one-line fix for (1) plus §2.3 for (2).

### 1.8 **NEW — "variant capability searching" already exists and is underused**

Nick's term maps to a real, shipped mechanism with three layers:

- **Variant grouping in HF search.** `createGroupVariant()` (`hf-browse.js:456-671`) renders a logical model as a group with per-format/per-quant *variants*; MLX variants are directly selectable (L533-551), GGUF variants expand a file list. `buildSelectionPayload()` (L288-305) carries `revision / variant / originalAuthor / converter / format / backendHint`.
- **Per-variant evidence, on demand, no download.** Each variant gets a "?" button wired to `openHfEvidence()` (`hf-browse.js:308-352`), which POSTs `/api/hf/qualify` **and** `/api/hf/identity` in parallel (L323-324).
- **The backend qualification surface is far richer than the drawer displays.** `HfQualification` (`src/hf/qualify.rs:50-75`) returns, for a repo **+ pinned revision, with no weight download**:
  - `config: HfConfigEvidence` — `modelType`, `architecture`, `hiddenSize`, `numLayers`, `numKeyValueHeads`, `headDim`, `contextLength`, `slidingWindow`, `ropeTheta`, `paramCountEstimate` (L80-106)
  - `tokenizer: HfTokenizerEvidence` — presence of `tokenizer.json` / `tokenizer.model` / `tokenizer_config.json`, special tokens (L110-115; derived at `qualify.rs:462-481`)
  - `chatTemplate: HfTemplateEvidence` — `hasChatTemplate`, `templateSource`, `templateFamily` (L119-125)
  - `extras: HfExtrasEvidence` — `vision` **with `visionSource` + `visionConfidence` ("confirmed"/"heuristic")**, `toolUse`, `reasoning`, `generationConfigParams`, `readmeModelFamily`, `securitySignals` (L138-163). The doc comment at L141-146 is explicit that a repo name or tag "is somebody's label, not a fact about the weights."
  - `backendQualified` + `qualificationReason`, `backendHint` (request takes `backend: Option<String>`, L47).

  The evidence drawer surfaces only four of these lines (`hf-browse.js:339-345`).
- **Lineage/derivative search.** `hf_discover_mlx_derivatives()` (`src/hf/mod.rs:3618-3800`) walks HF's author-declared `base_model:quantized:` relation graph (`fetch_mlx_relation_derivatives`, L3569+) and only falls back to fuzzy stem search when the graph is empty — the comment at L3644-3646 calls the relation graph authoritative and the stem search "a guess." For each candidate it calls `fetch_mlx_quant_for_repo()` (L3541-3549) → `fetch_mlx_config()` (L801) → `fetch_mlx_config_bytes_at()` (L850) — **a bounded remote fetch of `config.json` over HTTP, no weights**.
- **The generic primitive exists:** `fetch_raw_bytes_at(repo_id, revision, path, max_bytes)` (used at `qualify.rs:504` for `config.json`, 256 KiB cap). It takes an arbitrary file path.

So the mechanism Nick remembered is real, it is revision-pinned, it distinguishes confirmed evidence from heuristics, and it already fetches remote config JSON. It is simply not connected to family detection or to the wizard's MLX path.

### 1.9 **NEW — the Rapid-MLX effective policy is far more constrained than the UI implies**

`build_effective_policy()` (`src/web/api/rapid_mlx_runtime.rs:720-757`) hardcodes:
- `effective_kv_cache_dtype = Some(KvCacheConfig::Int8)` (L737) — unconditionally.
- `"reasoning_mode": "on"` (L750) — unconditionally.
- TurboQuant `V4`/`K8V4` → `Off` (L725-731).

`build_requested_vs_effective()` (L759+) then emits an honest diff: any requested dtype ≠ `int8` is reported as `effective: "int8"` with reason *"the always-on Rapid reasoning quality profile pins the KV cache to int8 regardless of the requested dtype"* (L804-813), and `reasoning_mode: off` is reported as `effective: "on"` (L814-823). `mod.rs:1132-1134` omits TurboQuant from argv entirely with the comment "Keep the requested setting persisted, but omit TurboQuant until a receipt is available."

**Therefore `#spawn-kv-cache-dtype` and `#spawn-turboquant-mode` are, today, requested-only controls with no effect on the launched server.** Their tooltip text at `index.html:3960-3966` ("int8 recommended for tool calling; bf16 for maximum quality; int4 for memory-constrained setups") reads as if they are live. This is the single largest honesty defect in the MLX wizard and it dictates the whole memory-feedback design (§2.4, §2.5).

---

## 2. Resolved questions

### 2.1 Why full unified restructuring, and why staged/MLX-first was rejected

For the record: rev 1 recommended a staged, MLX-first adoption. That is rejected. Two engines with two IA vocabularies means every future loader (MTPLX is already anticipated as needing its own adapter module — `spawn-wizard-rapid-mlx.js:1-13`) re-litigates the same layout, tiering, and memory-feedback decisions, and the capture harness pays for the migration twice (once per group now, once again at the end). The de-risking fact from rev 1 still holds and is what makes the full move affordable:

> `buildSpawnPayload()` (`spawn-wizard-spawn.js:~430-505`) and `buildRapidMlxConfig()` (`spawn-wizard-rapid-mlx.js:794+`) read `wizardState.hardware.*`, **not DOM order**. `mlx-ia.js` already proves live DOM relocation preserves the id-based serialization contract (`rowForControl` L83-88, `originalPositions` restore L97-103). A pure presentation refactor cannot break spawn.

Risk is therefore concentrated in *visual* regression, which is exactly what the capture harness covers — hence its promotion to a first-class phase (§5).

### 2.2 Chat-template failure mode: loud, specific, non-blocking

**Decision: keep graceful degradation at launch, but make the degradation impossible to miss, and make it visible *before* launch.** No hard-fail — hard-failing a launch over a template the user may not care about is the opposite of "simple and easy to understand."

Three surfaces, three moments:

| When | Where | Copy |
|---|---|---|
| At selection, the moment an Alias source is chosen with a custom/auto template active | `#chat-template-section` (`index.html:3855`) — new `'degraded'` state in `_renderChatTemplateStatus()` (`spawn-wizard-chat-template.js:249`), amber `ct-status` reading **`⚠ Not applied`** | *"This model was selected by alias, so llama-monitor cannot install a chat template for it. The server will use the template built into the model. To use **{template name}**, select the same model by HuggingFace repo or local folder instead."* Plus a **"Switch to repo selection"** button that re-enters step 1 pre-filtered to that model's repo (uses `hf_discover_mlx_derivatives` for the candidate list). |
| Step 4 (Review) | A dedicated `wizard-review-warning` block, rendered at the **top** of the review body, not appended to a details list | *"Chat template: not applied (alias source). The model's built-in template will be used."* |
| Step 5 (Start server) | Toast, `'warning'` severity, 8 s, plus a persisted entry on the server card | Same one-liner + link to the launch warnings. |

Wiring: promote the existing non-fatal launch warning (`mod.rs:897,916`) to a **typed** warning. Add a `RapidMlxLaunchWarning::ChatTemplateNotApplied { reason: AliasSource }` variant so the frontend can match on a code rather than a string, and surface the same code from the **preview** endpoint (which requires fixing §1.4 item 4 first — the preview adapters must carry `chat_template_file` for the warning to be computable pre-launch). This is what makes the step-4 surface possible at all.

Also: extend `create_template_overlay()` to be honest about *why* it bailed — currently `model_resolver.rs:1798` returns a generic failure for both "not a directory" and "path missing." Split those.

### 2.3 MLX family/template detection without a local path — concrete mechanism

Everything needed exists (§1.8). Proposal, in dependency order:

**(a) Fix the response-shape bug first (§1.7 item 1).** `spawn-wizard-chat-template.js:87` →
```js
const modelType = meta.data?.config?.model_type || meta.model_type;
```
One line; unblocks local MLX family detection immediately. Add a unit test asserting the exact `/api/models/mlx-introspect` envelope so this cannot silently rot again.

**(b) Add a remote mode to the same endpoint rather than inventing a second one.** `POST /api/models/mlx-introspect` accepts `{ repo_id, revision }` as an alternative to `{ model_path }`, and returns the **identical response envelope**. Implementation reuses what is already there:
- `config.json` → `fetch_mlx_config_revision_aware()` (`src/hf/mod.rs:814`), which already parses into the same `MlxConfig` type `read_mlx_local_config()` returns (`info_query.rs:621-632`). The two branches converge on the same struct, so the serializer at `models.rs:1476-1543` is reused unchanged.
- `tokenizer_config.json` → `fetch_raw_bytes_at(repo_id, revision, "tokenizer_config.json", 512 * 1024)`. This is the authoritative introspection Nick wants: it carries `chat_template` (or `chat_template_file`), `bos_token`/`eos_token`, `tokenizer_class`, and on Qwen/Gemma checkpoints the reasoning/tool markers. **Never trust HF tags** — tags are the *fallback* below the tokenizer, not above it.
- `model.safetensors.index.json` → the existing `has_mmproj_in_index()` logic (`info_query.rs:640-661`) applied to remotely-fetched index bytes, for vision.
- `recursive_size_bytes` → `resolve_mlx_repo_size_bytes()` (already used at `src/hf/mod.rs:3712`).

Cost per call: 2–4 small HTTPS GETs of JSON, all bounded, all revision-pinnable. No weight download. Reuse `HF_EVIDENCE_GATE`'s semaphore + 90 s timeout pattern (`src/web/api/hf.rs:16-18`) so this can't be used to hammer HF.

**(c) Widen the family vocabulary and give it an evidence ladder.** Replace `communityFamilyFromGgufArchitecture()`'s two-branch string match (`chat-template-registry.js:68-73`) with a table keyed on `model_type`/`architectures[0]` and, critically, record **where the answer came from**, mirroring `HfExtrasEvidence.visionSource`/`visionConfidence` (`qualify.rs:141-152`):

| Rank | Source | Confidence |
|---|---|---|
| 1 | Persisted `family:` tag in model-tags.json | `pinned` |
| 2 | `tokenizer_config.json` → embedded `chat_template` fingerprint | `confirmed` |
| 3 | `config.json` → `model_type` / `architectures[0]` | `confirmed` |
| 4 | `base_model:` → repeat 2–3 against the base repo | `confirmed (inherited)` |
| 5 | HF tags / README / repo name | `heuristic` |

The UI shows the confidence next to the family (`Detected family: qwen · confirmed (config.json)`), and **only auto-installs a template overlay at confidence `confirmed` or better**. At `heuristic` it renders the recommendation as an offer with a one-click "Use this" — never silently. This is the same "refuse to act on a guess" discipline the vision evidence already follows.

**(d) Extend variant capability search to the MLX wizard.** When an MLX HF result is selected (`spawn-wizard-hf-browse.js:159-163`), fire the existing `POST /api/hf/qualify` with `backend: 'rapid_mlx'` alongside `autoInstallChatTemplate()`. Its `extras.reasoning` / `extras.toolUse` / `extras.vision` + `visionConfidence` feed §1.6's tri-state directly, and `config.contextLength` seeds the ctx pills for MLX — which is the missing piece behind §1.3's empty sidebar. When the selected repo is a leaf quant with poor metadata, `hf_discover_mlx_derivatives()` gives sibling variants whose config *is* readable; inherit the family from the declared base model at confidence `confirmed (inherited)`.

### 2.4 KV cache dtype for MLX reasoning models — the archive's actual verdict

**This was never a benchmarking outcome. It is upstream runtime behaviour that llama-monitor is obliged to mirror.**

Primary evidence — `docs/archive/rapid-mlx/20260727-phase6_rapidmlx_integration_completion_handoff.md:32-40`, quoting the installed/source-build `rapid-mlx serve --help` verbatim:

> "Reasoning profile: pins `--kv-cache-dtype` to int8 regardless of the dtype flag"

and immediately after (L40): *"Reasoning is the expected normal use case. The UI must not quietly estimate INT4 while launching a reasoning profile that uses INT8."*

Formalised as a contract in `docs/archive/rapid-mlx/rapidmlx-integration-contract.md:52-59`:

```
reasoning_mode = true  -> effective INT8, regardless of requested dtype
reasoning_mode = false -> effective dtype = requested BF16/INT8/INT4
```
> "Wizard/Editor/cards show 'INT4 → INT8 (reasoning profile)' whenever reasoning overrides the request."

Reinforced in `20260727-…:194` (parity matrix row "Reasoning forces effective INT8 … **Must fix**"), `:498` (flag catalogue: *"`--reasoning` … Pins active KV to INT8"*), `:570`, `:994`, `:1053`, and the four required assertions at `:1104-1109`. `20260731-phase7_8_remainder-handoff.md:231` adds the clarification that reasoning mode *"sets the reasoning KV policy to int8; it does not mean 'show thinking.'"*

Implemented at `src/web/api/rapid_mlx_runtime.rs:732-737` (comment quotes the help text) and `:804-813`. Crucially, **llama-monitor now always launches with the reasoning profile on** (`:750` `"reasoning_mode": "on"` unconditional; `:814-823` treats a stored `off` as legacy and converts it to `--no-thinking`). So the effective active-KV dtype is `int8` for **every** Rapid-MLX launch, not only "reasoning models."

A second, independent archive finding closes the door on the alternative axis: `20260724-rapidmlx-benchmark-continuation.md:427` —

> "An int8-vs-int4 KV comparison on this backend is **not measurable** right now — do not spend a benchmark pass on it."

**Decision: `#spawn-kv-cache-dtype` becomes a read-only "Effective: int8 (reasoning profile)" readout in Quick/Balanced, and an Advanced-tier control whose selection is immediately annotated with the requested→effective diff already returned by `build_requested_vs_effective()`.** It must not look like a live knob. The estimator already models this correctly (`reasoning_mode_overrides_kv_to_int8`, per the comment at `rapid_mlx_runtime.rs:734`) — the wizard is the surface that disagrees.

### 2.5 TurboQuant — what actually happened

**Not ruled out. Withheld, deliberately, pending per-model/revision qualification receipts — and separately shown to be measuring the wrong thing in the harness.** Four strands:

1. **Design constraint from day one.** `20260710-rapid_mlx_integration.md:368`: *"TurboQuant cannot be combined with standard KV-cache quantization."* Since reasoning pins standard KV to int8 (§2.4), TurboQuant and the shipped default configuration are mutually exclusive by construction.
2. **Gated at launch, by design, with the reason in code.** `mod.rs:1132-1134` `.turboquant_mode(None)` — *"Keep the requested setting persisted, but omit TurboQuant until a receipt is available."* `rapid_mlx_runtime.rs:725-731` maps `V4`/`K8V4` → `Off` in the effective policy. `rapidmlx-integration-contract.md:166`: *"TurboQuant — UI wired, but launch keeps it disabled pending per-model qualification."* `20260727-…:1622-1624` records it as Item 16 with the JS defaults (`spawn-wizard.js:426 turboquantMode: 'none'`, `presets.js:1314 || 'none'`) and `:1727` classifies it *"intentional, documented."*
3. **It does not do what the scenario cards would need it to do.** `20260724-rapidmlx-benchmark-continuation.md:232`: *"TurboQuant affects retained reusable prefix snapshots, not cold active KV or weights — do not compare it using only cold rows."* It is a *retained-cache* lever, not an active-KV lever. A "context fit" card set built on TurboQuant would be measuring the wrong pool.
4. **The harness numbers were suspect.** `20260724-…:430` flags the "LARGER BUG": whether `scripts/rapid-mlx-benchmark-suite.mjs` recorded the *effective* or the *requested* `--kv-cache-turboquant`, given the V4/K8V4→Off fallback — the same silent-fallback class as the KV-dtype issue. Savings coefficients that were derived (`K8V4 = 0.575`, `V4 = 0.34`, applied to retained KV only — `docs/plans/20260718-final_rapidmlx_followups_execution.md:271`) therefore have an unresolved provenance question.

**And, for completeness, PFlash is genuinely ruled out** — `20260724-…:281`, verdict dated 2026-07-24: `--pflash auto` is **not recommended** at Rapid-MLX 0.11.0; recall collapsed to 0.0/0.2/0.4/0.2 at 63k/131k/160k/200k vs 1.0 with `pflash off`, while throughput jumped ~4×, i.e. the compressed region was being dropped rather than lossily retained. *"In an agentic coding loop this is a silent failure mode."* Default guidance stays `off`; do not re-open without a source-level fix.

**Decision: TurboQuant stays an Advanced-tier control with an explicit "Requested — not applied (awaiting model qualification receipt)" badge, and it is excluded from any memory-estimate scenario axis.** Same for PFlash, plus a warning badge on `auto`/`always` citing the 2026-07-24 recall result.

### 2.6 So what *are* the MLX context-fit scenario axes?

With `kv_cache_dtype` pinned to int8, TurboQuant force-Off, and PFlash steered to off, the remaining levers that genuinely move Rapid-MLX unified-memory occupancy are, from `build_effective_policy()`'s own list (`rapid_mlx_runtime.rs:738-756`):

- **context length** (`n_ctx`) — the dominant term, at fixed int8 KV;
- **concurrency** — `max_num_seqs` × `max_concurrent_requests`, which multiply active KV;
- **retained prompt cache budget** — `retained_cache_mib` + `hybrid_cache_entries`;
- **`gpu_memory_utilization`** — the ceiling the other three are measured against.

**Decision: replace the three KV-quant cards on the MLX branch with three *workload-shaped* cards varying concurrency and retained-cache budget at the user's chosen context, on a fixed int8 KV.** This also finally makes the cards mean something the user chose, rather than three views of an inert knob.

| Card | `max_num_seqs` | retained cache | Framing |
|---|---|---|---|
| **Single interactive user** *(default/recommended)* | 1 | measured coding-agent recommendation (8 GiB / 16 entries — the "Auto" value already documented at `index.html:3969`) | "One conversation at a time, warm prompts reused." |
| **Long single context** | 1 | 0 | "Maximum room for one very long conversation; nothing retained between prompts." |
| **Shared / multi-client** | 4 | 8 GiB | "Several clients at once; each admitted request reserves its own active KV." |

Each card asks `/api/vram-estimate` with the same `buildEstimateBody()` (`spawn-wizard-vram-display.js:465`) plus the MLX policy spread, and — unlike today — with *different* policy values per card. Alongside the cards, render the **fixed** facts once, not per-card: `KV: int8 (pinned by reasoning profile)`, `TurboQuant: off (awaiting receipt)`, `PFlash: off`.

Also fix the L417 early-return: gate on `modelBytes` only, and let the MLX branch pass `repo_id`/`revision` instead of `model_path` (the estimator already has an MLX profile path via `fetch_mlx_model_profile_revision_aware()`, `src/hf/mod.rs:830`).

### 2.7 MLX step-2 sidebar body

Not a quant advisor. Contents, top to bottom:

1. **What this repo already is** — bits/group size from `config.json` `quantization` (already parsed: `models.rs:1531-1542`, and remotely available via `fetch_mlx_quant_for_repo`), plus resolved size.
2. **Ctx pills**, seeded from `config.contextLength` returned by `/api/hf/qualify` (`qualify.rs:101`) rather than left at the static list.
3. **Memory estimate bar** — the existing `#wizard-sidebar-vram-bar` markup, unchanged, fed by the MLX estimator.
4. **Sibling variants** — from `hf_discover_mlx_derivatives()`: "This model is also published at 4-bit / 8-bit / bf16 by *converter*", each row with its size and a "?" evidence button. This is the MLX analogue of the quant ladder, and it is the honest one: real published variants, not a synthetic ladder.

### 2.8 Profile tiers — the llama.cpp philosophy, stated then translated

`applyProfileVisibility()` (`spawn-wizard.js:1948-1963`) is only 15 lines, and its behaviour is the whole philosophy:

```js
const isAdv = wizardState.profile === 'advanced';
const isQ   = wizardState.profile === 'quick';
if (dom.advancedFields) dom.advancedFields.open = isAdv;   // #spawn-advanced-fields, index.html:4375
if (isQ) { contextSize.disabled = true; batchSize.disabled = true;
           gpuLayers.value = 'auto'; gpuLayers.disabled = true; }
else     { …all three re-enabled… }
```

with the comment at L1951-1953 stating the rule explicitly: *"Advanced options stay visible (collapsed) on every profile — only their open/closed state follows the profile, so Quick/Balanced users aren't blocked."* `PROFILE_HINTS` (L1953-1957 region) confirms the intent: Quick = *"Fully auto-tuned … No knobs to turn"*; Advanced = *"Full control … including MoE tuning, KV cache quant, and multi-GPU."*

**Three invariants fall out, and they are the rules the unified registry must encode:**

- **I1 — Tier never hides.** Every control is reachable at every tier. Tier controls *editability* (Quick disables) and *default disclosure* (Advanced auto-opens). Nothing is `display:none` by tier.
- **I2 — Quick means "the wizard already decided."** A Quick-disabled control is one the wizard writes a derived value into (`gpuLayers.value = 'auto'`). Disabling without writing a value would be a dead control.
- **I3 — Advanced-tier = "needs a reason."** The `#spawn-advanced-fields` hint (`index.html:4377`) is literally *"Change only if you have a specific reason."* Expert-only concepts — tensor split (`index.html:4440`, multi-GPU), MoE offload (`4425`), thread priority (`4404`), speculative decoding (nested `<details>` at `4488`, i.e. *two* levels of disclosure) — all live there. Note the precedent at `4488`: a genuinely experimental subsystem gets its **own** nested collapse inside Advanced. That is the pattern for MLX companions.

**Resulting llama.cpp tier table (current behaviour, made explicit):**

| Tier | Controls |
|---|---|
| Quick (disabled, auto-written) | `spawn-context-size`, `spawn-batch-size`, `spawn-gpu-layers` |
| Balanced (editable, visible) | the three above + `spawn-cache-type-k/v`, `spawn-kv-unified`, `hw-quant-select`, `hw-mmproj-select`, ctx quick-picks, `hw-use-mtp` |
| Advanced (auto-opened `<details>`) | everything in `#spawn-advanced-fields` (`4375`): `spawn-ubatch-size`, `spawn-parallel-slots`, `spawn-flash-attn`, `spawn-prio`, `spawn-threads`, `spawn-threads-batch`, `spawn-n-cpu-moe` (+autotune), `spawn-tensor-split`, `spawn-cache-mode`, `spawn-cache-ram`, `spawn-fit-enable`/`-target`, `spawn-mlock` |
| Advanced, nested | `#spawn-spec-details` (`4488`): `spawn-spec-type`, `spawn-spec-draft-type-k/v`, `spawn-draft-model`, `spawn-spec-draft-n-min`, `spawn-spec-draft-p-min` |

**Now the MLX translation — every group and every control, not just the eight groups:**

| Supersection | Group (`mlx-ia.js`) | Control | Tier | Rationale / llama.cpp analogue |
|---|---|---|---|---|
| Generation | `thinking` | `spawn-rapid-reasoning-mode` | **Quick (read-only readout)** | Effective value is always "on" (`rapid_mlx_runtime.rs:750`). Showing an editable toggle that the backend overrides violates I2. Render as "Reasoning profile: on — pins KV to int8". Editable only at Advanced, with the requested→effective diff shown inline. |
| Generation | `sampling` | `spawn-sampling-mode` | **Balanced** | Server-level sampling defaults; conceptually the MLX peer of KV-quant-as-quality-dial. Client params always win (group description, L39), so it is low-risk. Quick auto-derives from the use-case card. |
| Generation | `protocol` | `spawn-rapid-tool-call-parser` | **Balanced** | Auto-detected (§1.6 / §2.3d); user override is a "my finetune is unusual" case — same weight as picking a chat template, which is Balanced-visible on llama.cpp. |
| Generation | `protocol` | `spawn-rapid-reasoning-parser` | **Balanced** | Same as above. |
| Generation | `protocol` | `spawn-rapid-hybrid-mode` | **Advanced** | Hybrid/linear-attention forcing has **no llama.cpp equivalent** and the archive is explicit that it can disagree with runtime introspection (`20260724-…:83`: config declares GatedDeltaNet layers while `rapid-mlx info` says pure attention; *"Surface a conflict instead of silently choosing one"*). Getting it wrong changes the memory model, not just speed. This is the MLX `tensor-split`: structural, expert-only, wrong answer is expensive. |
| Cache & Perf | `active-memory` | `spawn-kv-cache-dtype` | **Advanced (annotated)** | Direct peer of `spawn-cache-type-k/v` — but inert (§2.4). Quick/Balanced show the effective readout; Advanced allows a request and displays the override reason. |
| Cache & Perf | `active-memory` | `spawn-turboquant-mode` | **Advanced (badged "not applied")** | No llama.cpp peer; retained-KV only; withheld pending receipts (§2.5). |
| Cache & Perf | `active-memory` | `spawn-rapid-prefill-step-size` | **Advanced** | Peer of `spawn-ubatch-size` (`index.html:4389`), which is Advanced. |
| Cache & Perf | `retained-cache` | `spawn-retained-cache-mib` | **Balanced** | Direct peer of `spawn-cache-ram` / `-cram`… but `-cram` is Advanced on llama.cpp. **Deliberate divergence:** on MLX the retained cache is a *scenario axis* the user is asked to choose in §2.6's cards, so it must be reachable at the tier where those cards live. Presented via the `spawn-rapid-cache-mode` Auto/Off/Custom select (`index.html:3969`), with the numeric field revealed only on Custom — a Balanced-tier three-way choice gating an Advanced-tier number. |
| Cache & Perf | `retained-cache` | `spawn-rapid-hybrid-cache-entries` | **Advanced** | Working-set count; no llama.cpp peer; `20260731-hybrid-cache-entries-sweep.md` exists precisely because its behaviour needed a dedicated sweep to characterise. |
| Cache & Perf | `scheduler` | `spawn-rapid-max-num-seqs` | **Balanced** | Direct peer of `spawn-parallel-slots` (`4385`). Note llama.cpp puts `parallel-slots` in Advanced — but it is *also* a §2.6 scenario axis, so same reasoning as retained cache. Recommend **promoting `spawn-parallel-slots` to Balanced on llama.cpp too** during the unification, for symmetry. |
| Cache & Perf | `scheduler` | `spawn-rapid-max-concurrent-requests` | **Advanced** | Admission limit distinct from sequence count; no llama.cpp peer. |
| Cache & Perf | `scheduler` | `spawn-rapid-gpu-memory-utilization` | **Advanced** | Peer of `spawn-fit-target` (`4466`) — a memory-budget ceiling. Advanced on both. |
| Cache & Perf | `scheduler` | `spawn-rapid-pflash-policy` | **Advanced (badged, default off)** | No peer. Ruled out at 0.11.0 (§2.5); the badge must cite the recall result, since the failure is silent. |
| Cache & Perf | `scheduler` | `spawn-rapid-prefill-batch-size` | **Advanced** | Peer of `spawn-batch-size` (`4381`). |
| Cache & Perf | `scheduler` | `spawn-rapid-completion-batch-size` | **Advanced** | No direct peer; batching internals. |
| Server & Safety | `tool-integration` | `spawn-rapid-auto-tool-choice` | **Balanced** | User-facing capability toggle, gated on parser compatibility (group description, L63). Peer in spirit to `hw-use-mtp` (`4321`), which is Balanced-visible. |
| Server & Safety | `companions` | all 8 `spawn-rapid-speculative-*` ids (L70-75) | **Advanced, nested collapse** | Exact structural peer of `#spawn-spec-details` (`4488`). Already `collapsible: true` (L69) — this is the one group that already got its tier right. Keep it nested inside the Advanced disclosure rather than at top level. |

Two cross-cutting notes:
- Under I1, none of the Advanced rows are hidden at Quick — they render inside a closed `<details>` that Quick users can open, exactly as `#spawn-advanced-fields` behaves today.
- Under I2, every **Quick-disabled** MLX control must have a wizard-written derived value. `spawn-rapid-reasoning-mode` gets `on`; `spawn-kv-cache-dtype` gets `int8`; `spawn-rapid-cache-mode` gets `auto`. Anything we cannot derive must not be Quick-disabled.

---

## 3. Target architecture

### 3.1 The registry

New module `static/js/features/spawn-wizard-groups.js`. Single exported `GROUPS` array; `spawn-wizard-mlx-ia.js` is reduced to its MLX rows and its (already-proven) relocation engine is generalised into `spawn-wizard-ia.js`.

```js
{
  id: 'active-memory',            // stable; used by capture harness selectors
  step: 2,                        // which of the 6 wizard steps this group lands in
  section: 'cache-performance',   // supersection id (see 3.2)
  title: 'Active memory',
  description: '…',
  tier: 'advanced',               // group-level default tier
  loaders: ['rapid_mlx'],         // or ['llama_cpp'] or ['llama_cpp','rapid_mlx']
  controls: [
    { id: 'spawn-kv-cache-dtype',
      tier: 'advanced',           // per-control override of the group tier
      loaders: ['rapid_mlx'],
      quickValue: 'int8',         // I2: what Quick writes before disabling
      effective: 'reasoning-pins-int8',  // key into the requested-vs-effective registry
    },
    { id: 'spawn-rapid-prefill-step-size', tier: 'advanced' },
  ],
}
```

Four fields carry all the per-loader difference:
- **`loaders`** — a group or control absent from the active loader is simply not rendered. No `rapid-only` / `llama-only` CSS classes, no `hidden` attributes to keep in sync. This is how "fields that exist for one loader and not another" is handled: declaratively, in one table, reviewable in one screen.
- **`tier`** — drives a single generic `applyProfileVisibility()` (replacing the three hardcoded `dom.*` references at `spawn-wizard.js:1956-1962`).
- **`quickValue`** — enforces I2. A lint test fails the build if a control is Quick-disabled without one.
- **`effective`** — links a control to its requested-vs-effective explanation, so the "this knob is inert" annotation (§2.4, §2.5) is data, not scattered markup.

A third loader (MTPLX) is then a set of rows with `loaders: ['mtplx']` plus its own adapter module — no wizard surgery.

### 3.2 The six steps after restructuring

Step count stays at **6**. Divergent step counts would break the shared badge strip (`index.html:3521-3533`), the guardrail logic, and the review step, and would make the capture harness loader-forked at the navigation level. Same steps, different group *contents*.

| Step | Contents | Per-loader difference |
|---|---|---|
| 0 · How it works | unchanged | — |
| 1 · Choose model | source cards, HF browse w/ variant groups, chat-template panel, evidence drawer | scope default follows loader (§1.5); MLX gets the §2.7 sidebar body instead of the quant advisor; MLX gets the §2.2 degraded-template warning |
| 2 · Hardware & memory | supersection **Memory & context** (ctx, scenario cards, KV precision readout/control) + **Placement** (llama: gpu-layers/tensor-split/MoE; MLX: gpu-memory-utilization/hybrid-mode) | registry `loaders` |
| 3 · Settings | supersections **Generation & runtime**, **Cache & performance**, **Server & safety** — the existing MLX vocabulary (`mlx-ia.js:8-24`), now shared | llama.cpp's threading/batch/flash-attn rows join *Cache & performance*; speculative decoding joins *Server & safety* as a nested collapse for both loaders |
| 4 · Review | unchanged shape; gains the §2.2 top-of-body warning block and a requested-vs-effective table driven by `control.effective` | — |
| 5 · Start server | unchanged; typed launch warnings surfaced (§2.2) | — |

Both loaders' step-3 supersection headings become identical, which is the point: a user who has configured llama.cpp already knows where to look in MLX.

### 3.3 What does *not* change

- `wizardState` shape, DOM ids, `buildSpawnPayload()`, `buildRapidMlxConfig()`, `/api/spawn` payloads, preset schemas. Presentation only. No migration.
- `mlx-ia.js`'s relocation technique (`rowForControl` L83-88, `rememberPosition`/`restorePositions` L90-103, `hideSource`/`restoreSources` L137-151) — it is generalised, not replaced.

---

## 4. Risks

- **Visual regression on llama.cpp step 2/3 is near-certain and intended.** Mitigated by making capture-harness migration a gating phase, not a cleanup task (§5).
- **The `<details>`-per-group pattern multiplied across ~50 controls can produce a wall of collapses.** Mitigated by tiering at the *group* level by default and using per-control overrides sparingly; and by I1 meaning a closed group is one click away, not a mode switch.
- **Re-parenting `#rapid-hardware-panel` (§1.1) touches CSS that the sticky sidebar also touches.** Do these together, in Phase 0, before any registry work, so the capture baseline moves once.
- **The remote-introspection endpoint (§2.3b) adds an outbound-HF dependency to a formerly local code path.** Mitigated by the existing semaphore/timeout pattern and by keeping every failure non-fatal with a `heuristic`-confidence fallback.

---

## 5. Phased plan

Sequencing principle: **all baseline-invalidating layout work lands before the registry refactor, and the harness is re-baselined once per phase boundary, never mid-phase.**

### Phase 0 — Correctness fixes, no architecture (~3 days)
Independently shippable; several are one-liners with outsized value.
1. `spawn-wizard-chat-template.js:87` → read `meta.data.config.model_type` (§1.7). Add an envelope-shape test.
2. Re-parent `#rapid-hardware-panel` into `.wizard-main` (`index.html` 3941 ↔ 4205); sticky `.wizard-sidebar` in `spawn-wizard.css`. **(baseline-invalidating — do it first)**
3. Call `autoInstallChatTemplate()` on the MLX HF branch (`spawn-wizard-hf-browse.js:159-163`).
4. Tri-state the three "Detected:" surfaces (`spawn-wizard-rapid-mlx.js:609-610`; `index.html:3993`, `4006`).
5. Pass `chat_template_file` through the three preview adapters (`rapid_mlx_runtime.rs:2727, 2809, 2871`).
6. Split `create_template_overlay()`'s bail reasons (`model_resolver.rs:1798`) and introduce the typed `ChatTemplateNotApplied` launch warning.
7. Correct the `#spawn-kv-cache-dtype` / `#spawn-turboquant-mode` / `#spawn-rapid-pflash-policy` hint text (`index.html:3960-3966` et al.) so it no longer describes inert controls as live. *(Text-only stopgap; the full treatment is Phase 3.)*

**Exit:** capture baseline re-recorded for `wizard-rapidmlx/*`. `wizard-llamacpp/*` should be unchanged — verify, don't assume.

### Phase 1 — Loud template degradation + engine-driven HF scope (~4 days)
1. `'degraded'` state in `_renderChatTemplateStatus()`, review-step warning block, launch toast (§2.2).
2. `_applyScopeDefaultForEngine()` in `spawn-wizard-hf-browse.js`; call before `hfCreateScopeSelector(...)` (replacing L71-77) and from `selectWizardEngine()` (`spawn-wizard.js:2070`); `userTouchedScope` guard; suppress search re-fire on programmatic scope change (§1.5).
3. Add a `setScope()` setter to the `hfCreateScopeSelector` handle rather than relying on teardown/rebuild (`hf-browse.js:1564`).

### Phase 2 — Family detection & variant capability search (~1 week, backend-led)
1. Remote mode on `POST /api/models/mlx-introspect` (§2.3b) — `config.json` + `tokenizer_config.json` + safetensors index, identical envelope, revision-pinned, semaphore-gated.
2. Evidence-ladder family resolution with confidence, replacing `communityFamilyFromGgufArchitecture()` (§2.3c). Auto-install only at `confirmed`+.
3. Fire `/api/hf/qualify` with `backend: 'rapid_mlx'` on MLX selection; feed `extras.*` into the tri-states and `config.contextLength` into the ctx pills (§2.3d).
4. Surface sibling variants from `hf_discover_mlx_derivatives()` in the step-2 sidebar (§2.7).

### Phase 3 — MLX memory feedback, honestly (~1 week)
1. Rewrite `renderScenarioCards()` (`spawn-wizard-vram-display.js:416-500`) with a loader branch; MLX cards per §2.6 (concurrency × retained cache at fixed int8).
2. Relax the L417 early-return; MLX passes `repo_id`/`revision`.
3. Fixed-facts strip (`KV: int8 (pinned)` / `TurboQuant: off (awaiting receipt)` / `PFlash: off`) rendered once above the cards.
4. MLX step-2 sidebar body (§2.7) replacing the quant advisor.

**Exit:** `wizard-rapidmlx/*` baseline re-recorded. This is the last MLX-only baseline move.

### Phase 4 — Registry extraction, both loaders (~1.5 weeks)
1. Create `spawn-wizard-groups.js` with the full table (§3.1) — MLX rows ported from `mlx-ia.js:26-77` with tiers from §2.8, llama.cpp rows enumerated from `index.html` step-2/3 markup, `#spawn-advanced-fields` (`4375`), `#spawn-spec-details` (`4488`).
2. Generalise `mlx-ia.js`'s relocation engine into `spawn-wizard-ia.js`; keep `rowForControl`/`originalPositions` semantics intact.
3. Rewrite `applyProfileVisibility()` (`spawn-wizard.js:1948-1963`) to drive tiers from the registry; retire the three hardcoded `dom.*` references.
4. Retire the hand-written `<details>` blocks at `4375` and `4488` in favour of generated ones (ids preserved).
5. Add the `quickValue` lint test enforcing I2, and a test asserting every registry `controls[].id` resolves to a real DOM node for at least one loader (catches id drift).

**Exit gate:** `buildSpawnPayload()` and `buildRapidMlxConfig()` golden-payload tests pass byte-identically before and after. This is the phase's real safety net.

### Phase 4b — Capture-harness migration (runs *interleaved with* 4, not after)
Explicitly budgeted, not an afterthought. Current scenarios: `tests/ui/capture/scenarios/wizard-llamacpp/{spawn-wizard,spawn-wizard-engines,spawn-wizard-gif,spawn-wizard-hf-download}.mjs` and `wizard-rapidmlx/{spawn-wizard-rapid-mlx-gif,rapid-mlx-runtime,rapid-mlx-live,dashboard-rapid-mlx}.mjs`.
1. **Before** Phase 4 code lands: refactor the wizard scenarios to select by **group id** (`[data-wiz-group="active-memory"]`) rather than by ad-hoc DOM position/nesting. Group ids are the registry's stable contract; positions are not. This decouples the scenarios from the restructuring so they survive it.
2. Introduce a shared `openGroup(page, groupId)` helper in `tests/ui/capture/harness/` that opens the containing `<details>` regardless of tier — otherwise every Advanced-tier screenshot forks on profile.
3. Add a **tier-matrix** scenario: the same wizard step captured at Quick / Balanced / Advanced for each loader. This is new coverage; today's harness never exercises `applyProfileVisibility()` across profiles.
4. Re-baseline `wizard-llamacpp/*` **once**, at the Phase 4 exit gate.
5. `spawn-wizard-gif.mjs` and `spawn-wizard-rapid-mlx-gif.mjs` are the largest scenarios (12.9 K / 15.2 K) and will need re-scripting, not just re-baselining — budget them separately.

### Phase 5 — Reference documentation (~3 days, gated on Phase 4 exit)
Sequenced *after* the restructuring settles so the docs are written once against a stable IA.
1. `docs/reference/spawn-wizard.md` — rewrite the step-by-step around the six unified steps and the shared supersection vocabulary; document the tier invariants I1–I3 and the registry as the source of truth.
2. `docs/reference/rapid-mlx-runtime.md` — record the always-on reasoning profile and the int8 pin as *documented behaviour*, with the `serve --help` quote and the archive citations from §2.4; record TurboQuant/PFlash as withheld with reasons.
3. `docs/reference/vram-estimator.md` — the new MLX scenario axes (§2.6) and why the KV-quant axes do not apply.
4. `docs/reference/capabilities.md` + `hf-model-library.md` — document the evidence ladder and confidence levels (§2.3c), and the remote introspection endpoint's contract.
5. `docs/reference/inference-tuning.md` — the tier table from §2.8, as the user-facing "which knob at which level" guide.
6. Archive this plan into `docs/archive/` once executed.

### Phase 6 — MTPLX readiness check (~1 day)
No new loader implementation; just prove the architecture by adding a stub `loaders: ['mtplx']` row set behind a dev flag and confirming the wizard renders it with zero wizard-core changes. If that requires touching anything outside `spawn-wizard-groups.js` and an adapter module, the registry is not done.

**Total: ~5 weeks**, with Phases 0–1 shippable inside the first week and delivering most of the user-visible bug fixes independently of the restructuring.

---

## 6. Decisions

### 6.1 `spawn-parallel-slots` promotion — **accepted, with an MTP interaction caveat**

**Default stays 1 for both loaders, unconditionally.** Promoting these controls to Balanced tier changes *visibility*, not the default value — `spawn-parallel-slots` and `spawn-rapid-max-num-seqs` must both ship with `quickValue`/default `1` (per the I2 invariant, §2.8), matching how all archive benchmarking/testing was actually run and how the overwhelming majority of home users (single VRAM-constrained machine, one conversation at a time) will use either loader. Concurrency > 1 remains something a user has to deliberately opt into — it is the "Shared / multi-client" card in §2.6, not the recommended default card — so the MTP-interaction caveat below only ever surfaces for users who intentionally raise it.

Nick's home-user hardware is VRAM-constrained; he does not expect `spawn-parallel-slots` to be set above 1 in practice, so the Balanced-tier promotion is low-risk on its own. But he flagged a real interaction that Phase 4 must account for: **on llama.cpp, `spawn-parallel-slots > 1` disables MTP speculative decoding** (multi-slot batching is incompatible with the draft-model sequencing MTP relies on) — this is presumably enforced somewhere in the existing llama.cpp payload-building/validation logic and needs to be located and confirmed during Phase 4, not assumed away. Promoting the control to Balanced makes it easier to bump past 1 without realizing MTP silently drops, so Phase 4 must add a visible warning/disable-with-reason on the MTP toggle (`hw-use-mtp`) whenever `spawn-parallel-slots > 1`, sourced from wherever that constraint currently lives in validation code (`buildSpawnPayload()` or nearby — grep for `parallel` + `mtp`/`spec` interaction in `spawn-wizard-spawn.js` during implementation).

**Rapid-MLX/MTPLX side — investigated, no interaction found.** Checked `src/inference/rapid_mlx/mod.rs`'s launch-config validation (`validate_speculative_config()` and the surrounding builder) for any mutual-exclusion or forced-downgrade logic between `max_num_seqs`/`max_concurrent_requests` and `speculative_config`: none exists. This is architecturally expected — Rapid-MLX's speculative decoding is vLLM-style continuous batching (multiple concurrent sequences by design), not llama.cpp's single-sequence draft-model chain, so there is no equivalent of MTP's "multi-slot batching breaks draft sequencing" constraint to guard against. Conclusion: **no warning UI is added on the Rapid-MLX side** — inventing one without evidence would misrepresent an unconstrained control as a hazard. `spawn-rapid-max-num-seqs` and the speculative companions group remain independently adjustable.

llama.cpp side — implemented (`static/js/features/spawn-wizard-mtp-draft.js`, `renderMtpSection()`): `#spawn-parallel-slots` is disabled and forced to display `1` (with a `.field-hint` explaining why) whenever MTP is effectively enabled, mirroring the silent `parallelSlots = 1` override `buildSpawnPayload()` already applied at spawn time (`spawn-wizard-spawn.js:446`) — the displayed value no longer diverges from what's actually launched.

### 6.2 "Switch to repo selection" button — **preserve-and-restore**

Nick's instinct (preserve-and-restore) is also the right one on the evidence already in this plan, and is the recommendation: discarding step-2/3 configuration as the cost of fixing a template-degradation warning would be a disproportionately punishing UX for what is meant to be a one-click recovery action — directly against the "keep things automated and simple/easy for the user to understand" principle that shaped §2.2 in the first place. Concretely:

- Snapshot `wizardState.hardware` / `wizardState.settings` (whatever step-2/3 have already populated) into a short-lived `wizardState._pendingRestore` object before re-entering step 1.
- After the user picks a repo/revision in the re-entered step 1 flow, re-apply the snapshot on re-arrival at step 2, **re-validating each field against the newly-selected model's constraints** (context length ceiling, quant-derived defaults, etc.) rather than blindly replaying raw values — a field that no longer makes sense for the new selection (e.g. a context size pill beyond the new model's native window) should fall back to that model's default with a brief inline note, not silently carry over an invalid value.
- Discard the snapshot if the user backs out of the re-entry flow without completing a new selection, or after a short idle timeout, so it doesn't leak into unrelated future sessions.

This is a small, self-contained addition to Phase 1 (§2.2's implementation), not a new phase.

---

### Critical Files for Implementation
- `/Users/nick/SCRIPTS/CLAUDE/llama-monitor/static/js/features/spawn-wizard-mlx-ia.js` — becomes the shared registry + generalised relocation engine
- `/Users/nick/SCRIPTS/CLAUDE/llama-monitor/static/js/features/spawn-wizard.js` — `applyProfileVisibility()` (1948-1963), `selectWizardEngine()` (2070)
- `/Users/nick/SCRIPTS/CLAUDE/llama-monitor/static/js/features/spawn-wizard-chat-template.js` — the `meta.data.config` bug (87), evidence-ladder family detection
- `/Users/nick/SCRIPTS/CLAUDE/llama-monitor/src/hf/qualify.rs` + `/Users/nick/SCRIPTS/CLAUDE/llama-monitor/src/hf/mod.rs` — variant capability search; `fetch_raw_bytes_at` / `fetch_mlx_config_revision_aware` remote introspection
- `/Users/nick/SCRIPTS/CLAUDE/llama-monitor/src/web/api/rapid_mlx_runtime.rs` — `build_effective_policy` (720-757) / `build_requested_vs_effective` (759+), the int8 pin and TurboQuant gate
- `/Users/nick/SCRIPTS/CLAUDE/llama-monitor/static/index.html` — step-2/3 markup, `#rapid-hardware-panel` (3941), `#spawn-advanced-fields` (4375), `#spawn-spec-details` (4488)
