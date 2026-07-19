# Final Rapid-MLX Follow-ups: First-Class Runtime, Memory, and Product Parity

| Field | Value |
|---|---|
| Created | 2026-07-18 |
| Branch audited | `feat/rapid-mlx-integration` |
| Local audit HEAD | `d47a714` |
| Upstream Rapid-MLX source audit | default-branch `75b1fe3b3a8ab12967f64150524296f179dd9979`; release `v0.10.12` at `d56ae629a9c4fd6d76ef713a7342bd1bf19a8dad` |
| Scope | Research-backed follow-up implementation plan; text/LLM features only |
| Explicit exclusion | Rapid-MLX audio/TTS/STT routes, models, extras, and UI |
| Intended executor | A context-free coordinator agent using Builder and Verifier sub-agents |
| Phase sizing | Every implementation phase must fit comfortably inside one 200k-context window |
| Execution companion | [`20260718-final_rapidmlx_followups_execution.md`](./20260718-final_rapidmlx_followups_execution.md) — start here for phase routing, checkpoints, and agent briefs |
| Status | Source-of-truth plan; no product implementation from this audit is included |

## 1. Purpose

Rapid-MLX is now a substantial backend in llama-monitor, but it is not yet first-class in the same sense as llama.cpp. The remaining gap is not simply “expose more flags.” The audit found four different classes of work:

1. **Correctness defects in already-shipped paths.** Typed Rapid-MLX model sources are authoritative in Rust but ignored by several frontend readers; current tool-parser remediation generates invalid Rapid-MLX arguments; multiple visible controls do not reach the launch configuration.
2. **Insufficient architecture and memory evidence.** The MLX estimator reads simple flat configs but misses nested `text_config`, mixed layer types, recurrent-state dimensions, global-vs-local attention geometry, effective Rapid-MLX cache dtype, and several additive model/runtime components. The affected families are exactly the important Qwen3.6 and Gemma4 dense/MoE models this feature needs to guide accurately.
3. **Upstream capability drift.** Rapid-MLX 0.10.12 added two opt-in caches and already contains important performance, disk, security, diagnostics, and structured-output behavior that current capability profiles and UI do not represent. Some assumptions in the completed Phase 8 plan do not match the actual source.
4. **Product and information-architecture drift.** The wizard, preset editor, welcome cards, Model Library, and Hugging Face flows do not share one source/settings/estimate contract. Some MLX surfaces show llama.cpp vocabulary or omit critical fit guidance entirely.

This plan closes those gaps without trying to expose every Rapid-MLX or llama.cpp flag. A setting earns a first-class surface only when it materially affects one of the following:

- whether a model can load and fit;
- common performance or quality trade-offs;
- a common user workflow;
- security, reliability, or operational safety;
- observability, diagnosis, or reproducibility;
- an app-owned recommendation that can be explained with evidence.

Everything else is either capability-detected and hidden, placed in a curated Advanced/Troubleshooting surface, or deliberately omitted.

### 1.1 Canonical workload and product priority

The primary product is an **OpenAI-compatible local-model endpoint for external agents**, not the built-in chat UI. Unless a later decision says otherwise, design, defaults, estimates, cache guidance, diagnostics, and end-to-end tests use this workload mix:

- **80% interactive coding agent:** OpenCode-style development against one or more active repositories, with long system/tool definitions, iterative append-only turns, streaming responses, core coding tools, and MCP servers.
- **20% scheduled/research agent:** Hermes Agent/OpenClaw-style cron or delegated research/development tasks with multiple MCP/tool calls, occasional overlapping jobs, long observations, and potentially less predictable prompt reuse.
- **Additional first-class roleplay/storytelling workload:** SillyTavern-style third-party use, plus a first-party Roleplay recommendation option. The preferred/default integration profile is **instruct/client-formatted text completion**: llama.cpp via `/completion`, and Rapid-MLX via its OpenAI-compatible `/v1/completions` route using a compatible SillyTavern Generic/VLLM-style connection after payload qualification. Structured OpenAI Chat Completions remains a separately qualified alternative because many users prefer it. This workload has long histories, personas, example dialogue, world-info/lorebook injection, swipes/regeneration, custom stopping, and sampling needs that differ sharply from coding agents.
- **Secondary regression workload:** Llama Monitor's built-in chat. It must remain correct and capable, but it must not anchor performance recommendations or feature placement.

This changes the interpretation of “common” and “advanced”:

- tool calling, parser correctness, structured output, streaming, long stable prefixes, agent-session concurrency, request admission, timeouts, provenance, and endpoint diagnostics are primary workflows;
- agent harness compatibility must be tested with real OpenAI-compatible request shapes, not inferred from the in-app chat transport;
- cache advice must be based on active agent sessions, prompt-prefix stability, MCP/tool-definition churn, PFlash interaction, and tool replay safety;
- UI copy should say “coding/agent endpoint” and show workload-specific examples rather than treating ordinary chat as the default workload;
- model/browser/HF recommendations should prioritize tool/reasoning/template compatibility, stable long-context behavior, effective memory under agent concurrency, and time-to-first-token for agent profiles; roleplay adds prose quality, sampler/template compatibility, long-history behavior, and supported context-management evidence.

The default guided workload profile should be **Interactive coding agent**. Also provide **Tool/research agent**, **Roleplay/storytelling**, **General chat**, and **Deterministic batch/eval API** profiles as recommendation inputs, not opaque magic presets. `80% coding / 20% tool-research` defines product priority, validation effort, documentation prominence, and default ordering; it is not a blended runtime preset, traffic split, or resource-allocation formula. Scheduling, delegation, cron use, and overlap are editable assumptions inside Tool/research agent rather than a separate client-branded profile. Users must be able to inspect every assumption and derived setting.

## 2. Research Baseline and Evidence Sources

Future agents must revalidate drift-prone upstream facts before implementation. Do not replace source evidence with documentation summaries.

### 2.1 Upstream sources audited

- Rapid-MLX repository: <https://github.com/raullenchai/Rapid-MLX>
- Audited default-branch commit: <https://github.com/raullenchai/Rapid-MLX/commit/75b1fe3b3a8ab12967f64150524296f179dd9979> (documentation-only follow-up after the release tag)
- Release `v0.10.12`: <https://github.com/raullenchai/Rapid-MLX/releases/tag/v0.10.12>
- Release source commit: <https://github.com/raullenchai/Rapid-MLX/commit/d56ae629a9c4fd6d76ef713a7342bd1bf19a8dad>
- Hybrid/recurrent prefix reuse: <https://github.com/raullenchai/Rapid-MLX/pull/1111>
- Sliding-window correctness and reuse: <https://github.com/raullenchai/Rapid-MLX/pull/1124>
- Deterministic response cache: <https://github.com/raullenchai/Rapid-MLX/pull/1123>
- Cache documentation fix: <https://github.com/raullenchai/Rapid-MLX/pull/1129>
- Rapid server sampling-default config and precedence: <https://github.com/raullenchai/Rapid-MLX/blob/75b1fe3b3a8ab12967f64150524296f179dd9979/vllm_mlx/config/server_config.py#L63-L95> and <https://github.com/raullenchai/Rapid-MLX/blob/75b1fe3b3a8ab12967f64150524296f179dd9979/vllm_mlx/service/helpers.py#L1449-L1471>
- Rapid request/default cascade, including explicit-zero preservation: <https://github.com/raullenchai/Rapid-MLX/blob/75b1fe3b3a8ab12967f64150524296f179dd9979/vllm_mlx/service/helpers.py#L1627-L1701> and <https://github.com/raullenchai/Rapid-MLX/blob/75b1fe3b3a8ab12967f64150524296f179dd9979/tests/test_server_utils.py#L173-L349>
- Rapid model `generation_config.json` sampling loader: <https://github.com/raullenchai/Rapid-MLX/blob/75b1fe3b3a8ab12967f64150524296f179dd9979/vllm_mlx/utils/generation_config.py#L1-L107>
- Rapid server default CLI exposure: <https://github.com/raullenchai/Rapid-MLX/blob/75b1fe3b3a8ab12967f64150524296f179dd9979/vllm_mlx/server.py#L2066-L2090>
- Rapid-MLX dependency declarations: `pyproject.toml` at audited upstream commit
- MLX-LM template support reference: <https://github.com/ml-explore/mlx-lm/blob/v0.31.3/mlx_lm/server.py#L1793-L1852>
- Reference alternate project: <https://github.com/waybarrios/vllm-mlx>
- Current llama.cpp release `b10068`: <https://github.com/ggml-org/llama.cpp/releases/tag/b10068>
- Pinned llama.cpp source baseline: <https://github.com/ggml-org/llama.cpp/commit/571d0d540df04f25298d0e159e520d9fc62ed121>
- llama.cpp server contract at that commit: <https://github.com/ggml-org/llama.cpp/blob/571d0d540df04f25298d0e159e520d9fc62ed121/tools/server/README.md>
- llama.cpp host-memory prompt cache design: <https://github.com/ggml-org/llama.cpp/pull/16391>
- llama.cpp SWA/hybrid checkpoints: <https://github.com/ggml-org/llama.cpp/pull/15293>
- llama.cpp function-calling reference: <https://github.com/ggml-org/llama.cpp/blob/571d0d540df04f25298d0e159e520d9fc62ed121/docs/function-calling.md>
- llama.cpp speculative-decoding reference: <https://github.com/ggml-org/llama.cpp/blob/571d0d540df04f25298d0e159e520d9fc62ed121/docs/speculative.md>

External-client evidence must come from the official OpenCode, Hermes Agent/OpenClaw, and SillyTavern repositories/documentation and be pinned in Phase 0. Client names are workload labels; real request captures against a test server are required because documentation may not fully specify streaming, retries, tool ordering, or endpoint selection.

### 2.2 Current upstream dependency reality

At the audited commit, Rapid-MLX directly depends on:

- `mlx>=0.31.2`;
- `mlx-lm>=0.31.3`;
- `transformers>=5.0.0,<5.13`;
- optional `mlx-vlm>=0.6.3,!=0.6.4` for vision/DFlash paths;
- optional `mlx-embeddings` for embeddings;
- optional `outlines[mlxlm]` through the `[guided]` extra for structured generation.

`waybarrios/vllm-mlx` is **not** a Rapid-MLX dependency. The projects share historical lineage and the `vllm_mlx` Python package name, while Rapid-MLX retains deprecated `vllm-mlx*` executable aliases. They now ship independently. Treat waybarrios as:

- a source-comparison and contract-design reference;
- a watchlist for useful features;
- never a source of flags that can automatically be passed to Rapid-MLX.

Features currently seen in waybarrios but not Rapid-MLX include multi-model registry/lazy-unload policies, reranking, SSD KV tiers, warm-prompt files, explicit remote-code trust control, and default template kwargs. Any adoption requires a separate implementation or upstream Rapid-MLX work.

### 2.3 Real MLX architecture fixtures required

The following raw configurations were inspected and must become pinned test fixtures with source URL, revision, fetch date, and checksum:

- <https://huggingface.co/mlx-community/Qwen3.6-27B-4bit/raw/main/config.json>
- <https://huggingface.co/mlx-community/Qwen3.6-35B-A3B-4bit/raw/main/config.json>
- <https://huggingface.co/mlx-community/gemma-4-31b-it-4bit/raw/main/config.json>
- <https://huggingface.co/mlx-community/gemma-4-26b-a4b-it-4bit/raw/main/config.json>
- Flat baseline: `mlx-community/Qwen3-0.6B-4bit`
- Standard MoE baseline: `mlx-community/Qwen3-30B-A3B-4bit`

Do not write fixtures from the implementation struct. Capture real upstream files, minimize only after preserving the exact fields under test, and document what was removed.

### 2.4 Local evidence inspected

Primary implementation areas:

- `src/inference/rapid_mlx/`
- `src/inference/launch.rs`
- `src/web/api/rapid_mlx_runtime.rs`
- `src/web/api/vram.rs`
- `src/llama/vram_estimator/`
- `src/models/library.rs`
- `src/hf/mod.rs`
- `static/js/features/spawn-wizard.js`
- `static/js/features/presets.js`
- `static/js/features/setup-view.js`
- `static/js/features/models.js`
- `static/js/features/hf-browse.js`
- `static/js/features/vram-estimate.js`
- `static/index.html`
- relevant CSS, API docs, UI fixtures, and Playwright tests.

Prior plans are evidence, not authority when they conflict with code or upstream source:

- `docs/plans/archived/20260710-rapid_mlx_integration.md`
- `docs/plans/archived/20260710-rapid_mlx_roadmap.md`
- `docs/plans/20260718-rapid_mlx_phase8_parity_and_power_features.md`
- `docs/plans/20260718-rapid_mlx_phase8_remediation.md`

### 2.5 Screenshot evidence from this audit

The audit built the current release and ran current real capture scenarios. Artifacts remain gitignored under `docs/screenshots/artifacts/`.

Successful scenarios:

- `spawn-wizard`
- `spawn-wizard-engines`
- `models-v2`
- `welcome`
- `rapid-preset`

The original `preset-editor` capture failed because Puppeteer found the New button but could not calculate a clickable point. This audit repaired the harness to use the DOM click and wait for the real open state. It also expanded `rapid-preset` with:

- legacy and typed Rapid-MLX source fixtures;
- Rapid-MLX Model and Server captures;
- dark, light, and narrow Server views;
- a dedicated typed-source capture.

The typed-source fixture visibly proves the current frontend bug: the welcome card reports “No model configured” even though Rust can launch the typed HF source. The harness now captures the typed card immediately before Edit and the modal result as a pair, and logs requested versus opened preset IDs, because the current selection bug can open/populate the legacy preset instead of the typed one.

### 2.6 External-client workload evidence

Pinned source revisions audited on 2026-07-18:

- OpenCode `b8142c7aa8f88222873fb79d636e312e28037c2d`;
- Hermes Agent `614dc194ea7d853d39f9e84582ec62156f41a475`;
- OpenClaw `ec740e79a48c1d7879fe3e8f211b4f0719d5ec0e`;
- SillyTavern app `8172dcd0ee672d3cd9a5e5f7af134f91a45cd2b8`;
- SillyTavern docs `70e5e4d3c239253fca4692fe82e3936cb9c4b1b1`.

Documented facts:

- OpenCode supports an OpenAI-compatible base URL, normally calls a streaming model path with tools/tool choice, sorts model-facing tools by name, incorporates MCP tools, and has an outer session retry policy.
- Hermes Agent supports custom OpenAI-compatible endpoints, streams interactive requests with usage, canonicalizes whitespace/tool-call JSON for stable prefixes, keeps a stable-first session-cached system prompt, and can execute multiple tools concurrently.
- OpenClaw supports custom/local OpenAI-completions providers, streams and sends tools/tool choice, documents stable-prefix/volatile-suffix prompt construction and deterministic MCP ordering, preserves recent turns byte-for-byte in a pruning path, and has configurable agent/sub-agent/cron concurrency.
- SillyTavern distinguishes Chat Completions (structured messages) from Text Completions (one rendered prompt). Its dedicated llama.cpp Text Completion backend uses `/completion`, forwards streaming, and leaves context/instruct templates, samplers, response reserve, persona, world info/lore, and injection position under client control.

Evidence-backed implications:

- ordinary OpenCode, Hermes, and OpenClaw traffic streams; Rapid's exact-response cache bypasses it;
- deterministic tool ordering and append-only histories make prefix reuse plausible, but dynamic plugins/skills/MCP inventory/environment/provider transforms can still break byte identity;
- tool-worker or framework concurrency limits are not cache-entry recommendations and are not necessarily simultaneous generations against one model;
- SillyTavern text formatting is entirely client-owned; roleplay prefix reuse is workload-dependent because lore/persona/context reconstruction can change early tokens;
- these facts justify measurement profiles, not a universal cache N or slot count.

The exact immutable URLs are in Section 13. Real protocol captures remain mandatory because source documentation cannot prove the user's installed client version, plugins, request transforms, or concurrency.

### 2.7 Community finetune and MLX artifact evidence

Pinned upstream/runtime facts revalidated on 2026-07-18:

- Rapid accepts a known alias, HF-shaped `owner/repo`, or local directory, then its text path uses `mlx_lm.load`; arbitrary PyTorch/Transformers finetunes are not converted during `serve`: <https://github.com/raullenchai/Rapid-MLX/blob/75b1fe3b3a8ab12967f64150524296f179dd9979/vllm_mlx/cli.py#L8388-L8465> and <https://github.com/raullenchai/Rapid-MLX/blob/75b1fe3b3a8ab12967f64150524296f179dd9979/vllm_mlx/utils/tokenizer.py#L742-L865>.
- The production download/load gate requires the expected case-sensitive MLX tensor layout such as `model*.safetensors`; `.bin`, `.gguf`, `.npz`, adapters, or unrelated safetensors do not prove loadability: <https://github.com/raullenchai/Rapid-MLX/blob/75b1fe3b3a8ab12967f64150524296f179dd9979/vllm_mlx/_download_gate.py#L45-L75> and <https://github.com/raullenchai/Rapid-MLX/blob/75b1fe3b3a8ab12967f64150524296f179dd9979/vllm_mlx/_download_gate.py#L181-L235>.
- MLX-LM conversion is a separate, explicit pre-serve operation with source revision, output, dtype, quant bits/group/mode, mixed-quant predicate, remote-code choice, and optional upload controls: <https://github.com/ml-explore/mlx-lm/blob/ed1fca4cef15a824c5f1702c80f70b4cffc8e4dd/mlx_lm/convert.py> and <https://github.com/ml-explore/mlx-lm/blob/ed1fca4cef15a824c5f1702c80f70b4cffc8e4dd/README.md#L92-L110>.
- Exact Rapid aliases carry qualified parser/architecture/accelerator facts; arbitrary HF repos may load but cannot inherit alias-only eligibility such as DDTree: <https://github.com/raullenchai/Rapid-MLX/blob/75b1fe3b3a8ab12967f64150524296f179dd9979/vllm_mlx/cli.py#L2058-L2085>.

Representative immutable community artifacts prove the desired ecosystem exists and that qualification cannot rely on one metadata field:

- Froggeric Qwen3.6 A3B Uncensored Heretic MLX 4-bit at `32939a6cef2750f18ccf352443f22f2e4dfe3613`: <https://huggingface.co/froggeric/Qwen3.6-35B-A3B-Uncensored-Heretic-MLX-4bit/tree/32939a6cef2750f18ccf352443f22f2e4dfe3613>. Its pinned config contains mixed nominal 4-bit/selected 8-bit quantization, so the repo suffix alone is insufficient: <https://huggingface.co/froggeric/Qwen3.6-35B-A3B-Uncensored-Heretic-MLX-4bit/blob/32939a6cef2750f18ccf352443f22f2e4dfe3613/config.json>.
- Its parent finetune is separately authored by llmfan46 and points to the official Qwen base, demonstrating official base -> behavioral finetune -> MLX conversion lineage: <https://huggingface.co/llmfan46/Qwen3.6-35B-A3B-uncensored-heretic/tree/dbfd9eb0cdc7c33fc970b06429f6e043b6851190>.
- Youssofal publishes a separate Heretic MLX conversion with publisher-specific `mlx_variant_metadata.json`; useful evidence, not a universal schema: <https://huggingface.co/Youssofal/Qwen3.6-35B-A3B-Abliterated-Heretic-MLX-4bit/tree/fb27c56fe8af376b08d2722091866e7d15eb31dc>.
- `mlx-community/gemma-4-31B-it-uncensored-heretic-4bit` has a valid-looking MLX layout while HF declares `library_name=transformers`, proving `library_name=mlx` cannot be mandatory: <https://huggingface.co/mlx-community/gemma-4-31B-it-uncensored-heretic-4bit/tree/a4c7561cc890307b95b473f8564634c3d598734a>.
- Unsloth publishes both MLX and GGUF for one lineage: <https://huggingface.co/unsloth/Qwen3.6-35B-A3B-UD-MLX-4bit/tree/6700c3e5bdeb050a379c8d2a4133f43f3647f20f> and <https://huggingface.co/unsloth/Qwen3.6-35B-A3B-GGUF/tree/a483e9e6cbd595906af30beda3187c2663a1118c>.
- LM Studio Community likewise publishes MLX and GGUF variants: <https://huggingface.co/lmstudio-community/Qwen3.6-35B-A3B-MLX-4bit/tree/0c4a20a6437ae5985ddc9eb1a3f122ee6c151c3b> and <https://huggingface.co/lmstudio-community/Qwen3.6-35B-A3B-GGUF/tree/68a34855558af61cbef0324d31f411be8a506b08>.

Product implication: maintain independent discovery/curation evidence and technical qualification. Known author, Community Pick, popularity, downloads, likes, heretic/uncensored/distill tags, repo suffix, `library_name`, and MLX tags may rank or explain candidates; none can promote a revision from Candidate to Runtime compatible. Qualification requires immutable identity, structural completeness, config/tokenizer/template/processor evidence, architecture/dependency/remote-code review, quantization map, real load/API smoke tests, and versioned evidence.

## 3. Audit Verdict: Current Critical Gaps

### 3.1 P0: invalid tool-parser launch arguments

Current local types store `tool_call_parser` as `bool`. `src/inference/rapid_mlx/command.rs` then emits:

- bare `--tool-call-parser`, despite upstream requiring a parser string;
- `--auto-tool-choice`, despite upstream defining `--enable-auto-tool-choice`.

The diagnostics advisor and Apply Fix action persist those invalid booleans. This is a concrete correctness defect. It must be repaired before any new Rapid-MLX tuning work.

Required shape:

- `tool_call_parser: Option<String>` or an equivalent typed selection;
- `enable_auto_tool_choice: bool` with the exact upstream flag;
- parser recommendation carrying evidence (`alias_profile`, regex inference, user override, unknown);
- no one-click fix unless a concrete valid parser value is known;
- explicit incompatibility handling for runtimes lacking the probed flags.

### 3.2 P0: typed model-source split brain

`RapidMlxModelSource` is the authoritative Rust model identity. The wizard creates typed sources. Rust correctly prefers `model_source` over legacy `model_path`.

Several frontend readers still use only `rapid_mlx.model_path`, including preset display/edit/save, welcome cards, welcome estimates, Model Library preset association, and live profile lookup. Consequences include:

- valid presets shown as model-less;
- Start becoming “Set up model”;
- editing a blank or stale legacy string;
- saving rejecting a valid typed source;
- edits to legacy `model_path` having no launch effect because typed source still wins;
- no welcome estimate or Model Library association.

Implement one shared frontend codec/view model for every source variant. Legacy `model_path` is accepted only at a migration boundary; it must not remain a second writable identity.

### 3.3 P0: visible controls that do nothing

The wizard renders and mutates state for:

- speculative configuration;
- MLLM/vision;
- embedding model.

Those values are absent from `buildSpawnPayload()` and `RapidMlxConfig`, so they never reach command construction. The preset editor fetches a live profile and never renders it. The Rapid-MLX advanced controls are mounted in the unrelated preset/config modal rather than the wizard, and the preset editor cannot edit the escape-hatch values the wizard can save.

Rule for all later phases:

> No visible control may land until `capability/evidence -> UI -> typed schema -> validation -> launch/request mapping -> restore/edit -> review summary -> test` is complete.

Until then, show eligibility as read-only or hide the control.

### 3.4 P0: Rapid preset sampling is persisted but not applied

The wizard persists shared sampling values at the top level. `request_from_preset()` maps them only to llama.cpp. Rapid-MLX clones only its backend config. The UI copy says these are request defaults, while actual Llama Monitor chat parameters live per tab.

This requires a product decision before implementation:

- Should preset request defaults apply to all clients proxied through llama-monitor?
- Or should they seed only Llama Monitor chat tabs?

The accepted product contract is a visible, selectable, provenance-bearing sampling-mode catalog whose selected values become omission-only server/request defaults. Explicit client fields from OpenCode, Hermes/OpenClaw, SillyTavern, or Llama Monitor chat always win, including explicit `0`, `false`, empty reset values, and empty stop lists.

Current llama.cpp behavior is the starting point but has known catalog/recognition bugs:

- `src/llama/model_defaults.rs` contains multiple Unsloth-backed Qwen/Gemma modes and generic fallbacks;
- `/api/model-defaults` returns every matched mode, but frontend pills and family recognition can collapse or misclassify renamed/opaque finetunes;
- detection still relies too heavily on filename plus a coarse GGUF architecture hint;
- the wizard also has a separate hard-coded `SAMPLING_DEFAULTS` table, creating competing authority;
- descriptions are largely tooltip-only and do not provide persistent workload/source badges;
- Rapid presets do not receive the same selected defaults at launch.

Every model on both backends must show a complete mode selector. At minimum it includes Model/author defaults, General, Coding/Agentic, Precise/Deterministic, Creative/Roleplay, and Custom; family-specific thinking/non-thinking or other modes are added when applicable. For a recognized family, all curated modes remain visible and selectable—never only the first/recommended mode. Each option shows persistent best-use badges and source provenance. Unsloth-published model-page values are user-approved authoritative inputs: preserve them exactly, cite the pinned page/retrieval date, and label them **Unsloth recommended** rather than blending them with heuristics. Where Unsloth has no published values, use model-author `generation_config.json`/model-card evidence, then qualified runtime alias evidence, then an explicitly labeled Llama Monitor starting point. Never present an extrapolation as Unsloth-authored.

Finetune resolution must prefer real lineage and metadata—GGUF base-model metadata, architecture, tokenizer/template evidence, HF base-model relationship and pinned revision, MLX `config.json`/`generation_config.json`, and qualified Rapid alias profile—over filenames. Provide a persisted manual base-family override with provenance when authoritative lineage cannot be recovered. A renamed finetune must not lose its family's options merely because its filename lacks `qwen`, `gemma`, or another family token.

Rapid-MLX already resolves request sampling in the correct order: explicit request > CLI default > alias `recommended_sampling` > model `generation_config.json` > hard fallback. At the pinned commit, the config/resolver represents temperature, top-p, top-k, min-p, repetition, presence, and frequency penalties, but the public server parser exposes only `--default-temperature`, `--default-top-p`, and `--default-top-k`. Phase 0/3 must revalidate this exposure as a **fact to pin**, not a gap this plan promises to close: the scope is the sampling controls Rapid already exposes (temp / top-p / top-k), and this plan does **not** commit to upstreaming additional `--default-*` flags. Where a control is not exposed, the UI shows per-field backend coverage and never claims a selected Rapid default is active when it cannot reach the runtime.

The plan also does not ship opinionated default sampling *values*. Unsloth's published values are use-case-specific guidelines (agentic-thinking vs non-thinking, general, etc.) that harnesses routinely override; they remain optional, user-selectable, provenance-bearing presets and are never silently applied. What the plan pins is the **resolver precedence** — explicit client/harness value > user preset > backend default (= D22 / D27 / A2 / A40) — with the §12.13 precedence matrix as the validation surface, and the §13.2 `request_from_preset()` omission test guarding the persisted-not-applied defect.

### 3.5 P0: MLX memory metadata is not architecture-complete

`src/inference/rapid_mlx/mlx_meta.rs` models mostly flat top-level config fields. Modern Qwen3.6 and Gemma4 MLX repos nest their text architecture in `text_config`. Even when a config is labeled `Exact`, important parsed or absent fields are not represented:

- `layer_types` / full-vs-local-vs-linear layer counts;
- `full_attention_interval`;
- recurrent/linear-attention state dimensions;
- `num_global_key_value_heads` and `global_head_dim`;
- `sliding_window_pattern` use;
- `top_k_experts` and shared-expert semantics;
- embedded MTP layer count and tensor ownership;
- per-component evidence and provenance;
- context ceiling propagation.

Current risks:

- Qwen3.6 may count every layer as full KV, roughly a 4x KV error;
- Gemma4 may treat every layer as local-window KV and omit global KV, producing a large long-context underestimate;
- recurrent state, external drafters, embeddings, and some VLM costs disappear or double-count;
- `max_position_embeddings` is parsed but not enforced in recommendations.

### 3.6 P0: Rapid memory controls and estimates use llama.cpp vocabulary

The estimator accepts `ctk`/`ctv` values such as `q8_0`, `q4_0`, and `f16`. Rapid-MLX 0.10.12 instead has:

- canonical `--kv-cache-dtype {bf16,int8,int4}`, default `int4`;
- `--reasoning`, which pins KV cache to `int8` for hard-math quality;
- automatic bf16 downgrade for unsafe sliding-window/MLA families;
- TurboQuant `v4`, `k8v4`, or `none` with asymmetric K/V behavior and alias-driven defaults.

The app launches none of these fields, while estimates read shared llama values. Rapid-MLX also has no server `--ctx-size`; current context is a planning target rather than a launch-enforced allocation. Present estimates therefore look precise while being disconnected from the actual runtime policy.

Two distinct facts must not be conflated when this is repaired (see §13.1 for the pin split):

- **llama.cpp KV floor for tool calling is a known heuristic.** For agentic / tool-calling workloads on these smaller models, KV below `q8_0` (e.g. `q4_0`) becomes unreliable and prone to loops. This is lived and community-corroborated; encode it as a warning when a tool-enabled llama runtime is configured below `q8_0` KV.
- **The Rapid-MLX / MLX KV floor for tool calling is unknown and must not be guessed.** TurboQuant and RotorQuant redistribute precision rather than naively truncating, so "4-bit MLX" is not equivalent to `q4_0` quality and the llama floor does **not** port. Surface each backend's native KV option honestly (D20 — no cross-backend KV-vocabulary normalization). The *factual* flags/choices/defaults are grep-able from the vllm_mlx parser and may be pinned; the *recommended* tool-calling value is measured on the M5 Max, not asserted.

### 3.7 P1: estimator data-source and arithmetic defects

Repair as part of the memory foundation:

- `estimate_param_b_from_size` misses the `* 8` conversion from bytes to bits;
- MLX HF `hf_file_path` can be misused as the config filename;
- HF size resolution lacks robust revision, recursion, and pagination handling;
- local MLX Model Library entries do not parse config/index and therefore lack estimates;
- Model Browser HF preview hard-codes llama.cpp;
- Rapid auto-size follows GGUF-first logic and writes llama-specific settings;
- only the wizard shows Approximate/Degraded evidence; preset and welcome totals suppress it.

The HF/Model Library audit also found that existing first-class GGUF concepts are only partially wired and Rapid parity is currently illusory:

- backend search types and an unused format-toggle helper understand MLX, but neither Wizard nor Models-modal search passes a format, so both effectively remain GGUF-first;
- `/api/hf/files` and `hfListFiles()` are GGUF-only; native MLX identity is repo + immutable revision + variant, not a fake GGUF filename;
- Models-modal preview hard-codes llama.cpp, 16k context, q8 K/V, one slot, and ubatch 512;
- Wizard quant comparison omits backend/repo/file identity, so Rapid follows llama math; context/KV/concurrency changes do not refresh it;
- the current VRAM-bucket `getRecommendedQuant()` ignores model, context, KV, concurrency, and headroom, and the file list computes but does not apply a true authoritative preselection;
- Wizard alternate-quant switching is substantially present, but one fallback can visually select an option without updating `wizardState`; variant changes also leave stale comparison state;
- sorts differ between surfaces, the Models-modal active-discover path has a likely constant/window reference bug, Wizard public search unnecessarily asks for a token, and name-derived minimum-size filters can hide opaque finetunes;
- the current author list and Community Picks schema are GGUF/repo-centric, and model-card/base-lineage metadata is too weak for community finetune -> conversion -> quant grouping.

### 3.8 P1: upstream `info` and extras assumptions are false

`rapid-mlx info <HF repo>` is not equivalent to a verified alias:

- regex inference may yield parser/architecture hints;
- DFlash/DDTree eligibility is emitted only when a registered alias profile resolves;
- unknown finetunes do not gain alias eligibility;
- `info` does not emit vision or embeddings extras;
- PFlash/TurboQuant alias tiers cannot be inferred for arbitrary repos from this command.

Every profile field must carry source and confidence. Unknown must remain unknown. Never upgrade a finetune to verified-alias confidence because the text parser returned a partial table.

### 3.9 P0/P1: stock chat templates are tool-call-unreliable, and no revision-pinned template substitution exists

**Driving correctness defect (P0/P1 for a tool endpoint).** The stock chat templates shipped with Qwen3.5/Qwen3.6 and Gemma4 models — including the exact families this feature must guide, and the finetuned Qwen3.6-27B this project itself relies on — are unreliable for tool calls: they loop or fail to emit well-formed tool-call output. This is corroborated by widespread community reports and is not specific to one loader. The community-standard remedy for this use case is a substituted template (the **Froggeric** template for Qwen3.5/3.6; for Gemma4 the official Google template is the current candidate to qualify, since the community `jscott` template has underperformed in practice). Because tool calling is a primary workload (§1.1), shipping the stock template unchanged means the endpoint can loop or drop tool calls under exactly the traffic it is designed for. The mitigation is a **revision-pinned template-substitution layer** with a **tool-call smoke-test matrix** that must pass before a candidate template becomes active. This is the driving defect behind the comparison / rollback / smoke-test machinery in §12.7 — that machinery exists to make this class of failure detectable and reversible, on **both** backends.

**Architecture: one selection layer, two appliers.** Do **not** build native template-override support into Rapid-MLX. Build ONE revision-pinned template-selection layer (identity, rollback, provenance — mostly existing llama.cpp plumbing that needs improvement anyway) with two thin appliers:

- **llama.cpp:** apply via the existing `--chat-template` / `--chat-template-file` flags.
- **Rapid-MLX:** apply by **file placement** — the template is just a file in the model directory. See the narrower truthfulness note below for why this cannot be a flag today, and Phase 0 for the pins that decide whether it can become symmetric.

Safety rule for the Rapid applier: it must **never** mutate the canonical / HF-cache model directory. Use a copy or overlay that llama-monitor owns, so swaps are reversible and a re-download cannot be clobbered.

**Narrower truthfulness sub-point (was the whole of §3.9).** Rapid-MLX 0.10.12 has no `--chat-template`, `--chat-template-file`, or server template overlay; it uses the model/tokenizer repository template, and its request `chat_template_kwargs` support forwards only a narrow supported subset such as thinking behavior — it does not replace the template. Underlying MLX-LM supports a template override, but Rapid-MLX does not invoke the MLX-LM server CLI, so the feature is not inherited. The UI must therefore never claim an override that Rapid cannot honor: on the Rapid backend, template substitution is file-placement provenance, not a runtime flag. This is honesty about the applier, not the reason to skip the work — the work is required by the tool-call defect above.

### 3.10 P1: security and dependency gaps

- Rapid-MLX `BatchedEngine` currently defaults `trust_remote_code=True` with no serve-time force-off flag and passes that permission into tokenizer/processor loading. This does **not** mean an ordinary data-only MLX repository executes Python: standard config/tokenizer/Jinja files and safetensors are artifacts consumed by architecture implementations already installed with MLX-LM. Repository code is relevant only when the pinned source declares custom loading hooks such as `auto_map`, references custom configuration/tokenizer/processor classes or Python modules, or cannot be proved data-only. The product must distinguish permission from actual use and should still seek a native force-off control as defense in depth.
- `[guided]` is a separate optional extra and is not included by the misleadingly named `[all]` extra. Its presence changes strict JSON/schema behavior and must be probed.
- Rapid excludes broken `mlx-vlm==0.6.4`; `0.6.5` now exists and satisfies the range. Qualify it before treating inherited Qwen/Gemma paths as verified.
- Rapid defaults disk KV checkpoints to an interval of 256 with a default 20 GiB cap. Storage ownership, cleanup, visibility, and troubleshooting are not represented in llama-monitor.

### 3.11 P0/P1 cross-backend agent-workload gaps

The new canonical workload exposes shared and llama.cpp-specific risks that a Rapid-only parity pass would miss:

- Current UI copy and recommendation code are partly framed around “single-user interactive chat,” while the primary use is an external coding/tool agent plus occasional overlapping scheduled jobs.
- The wizard automatically sets llama.cpp `--cache-ram -1` on unified-memory Macs and describes it as “no limit, no reservation.” Current llama.cpp defines `-1` as **no host-RAM cache limit**. The cache is demand-filled rather than eagerly reserved, but on Apple unified memory its growth competes with weights, active KV, buffers, applications, and OS headroom. Unlimited cannot be a safe universal recommendation for long agent sessions.
- The launcher currently decides whether idle-slot caching is enabled with `cache_ram_mib > 0`; this incorrectly classifies valid `-1` (unlimited) as disabled and can suppress the requested `--cache-idle-slots`. Treat `0` as disabled, positive values and `-1` as enabled, and test every sentinel explicitly.
- llama.cpp now enables server prompt caching by default and offers host-RAM prompt cache, idle-slot preservation, context checkpoints, cache reuse/KV shifting, automatic/unified slots, and continuous batching. Llama Monitor exposes only part of this policy and does not teach the difference between active slots, hot reusable sessions, cached idle prompts, and host-RAM budget.
- `parallel_slots = 1` maximizes single-request decode resources but can let a background cron/research generation block interactive coding. More slots improve overlap/admission but consume KV/working memory and divide throughput. The setting must derive from simultaneous active generations and latency priority—not chat count or hot prefix count.
- Current cache-idle help appears only when slots exceed one, although prompt reuse and host-cache behavior require a broader end-to-end audit. The app must not imply that more slots are automatically faster.
- Request-level `cache_prompt`, server-level `--cache-prompt`, `--cache-idle-slots`, `--cache-ram`, `--cache-reuse`, and context checkpoints have different scopes. They need capability/version qualification and a shared explanation rather than one “Prefix Cache RAM” field.
- Tool-template compatibility, streaming SSE health, long prefill progress, cancellation, request timeouts, admission/503 behavior, and structured tool calls become primary endpoint quality signals for both engines.
- The llama.cpp estimator and fit guidance must model active KV/slots separately from retained host prompt-cache growth. Existing formulas and UI totals must be revalidated against current unified-KV/automatic-slot semantics before agent-concurrency recommendations change.
- Current llama.cpp context/slot math is internally inconsistent. Llama Monitor presents `context_size` as per slot and estimates roughly `context × slots`; current upstream treats `--ctx-size` as total KV capacity. With partitioned KV, per-sequence context is divided by parallelism; with unified KV, sequences share the total pool and an individual sequence may grow within it. The app can therefore overestimate memory while simultaneously overpromising guaranteed per-request context. This is P0 for any multi-agent recommendation.
- `src/inference/llama_cpp.rs` unconditionally adds the experimental `--webui-mcp-proxy`, even though external OpenCode/Hermes MCP use does not require llama-server's Web UI proxy and upstream warns not to enable it in untrusted environments. Normal presets must not emit it.
- The wizard silently forces `parallel_slots = 1` when llama.cpp MTP speculation is enabled. For the user's 5090 workflow and older/model-specific implementations this is the correct required operating mode; even pinned `b10068`, which can initialize per-sequence MTP state for multiple slots, carries extra recurrent-state memory and weak parallel benefit. The defect is not choosing one slot—it is allowing a conflicting value and rewriting it invisibly. MTP must be an explicit single-stream profile unless the exact build/model has separately qualified experimental multi-slot evidence.
- Quant comparison accepts a use case but currently scores against a generic small-context baseline; Model Browser omits workload, target context, concurrency, engine, and architecture. A “Recommended” quant can therefore fail the selected coding/roleplay target.
- Workload and model-default precedence is split between Rust and JavaScript, and explicit zeros can be lost. Coding, research, and roleplay recommendations need field-level provenance and omission-safe precedence.

Treat the automatic `-1` assignment as an urgent recommendation defect: Phase 1 must stop applying it to new/auto-sized presets until Phase 5/6 establishes a bounded, headroom-aware policy. Preserve explicit user values and explain migration; do not silently rewrite existing presets.

## 4. Capability Inventory and Product Priority

### 4.1 First-class settings or policies

These should become typed, capability-gated product concepts rather than free-form flags:

- model source, revision/provenance, runtime and dependency compatibility;
- planning context and effective context cap policy;
- active concurrency/admission limits;
- prefill/completion batch sizing where evidence supports user adjustment;
- effective KV dtype with model-safe downgrade explanation;
- prefix-cache enablement and byte/percent budget;
- hybrid/SWA retained-prefix policy;
- GPU memory utilization after calibration;
- PFlash mode and model/workload safety guidance;
- verified speculative method/configuration;
- parser/reasoning selections and disable overrides;
- MLLM/text-only mode and required extra;
- external embedding model and its additive memory;
- request timeout/body/admission safety;
- structured-generation capability state;
- chat-template provenance and override availability;
- runtime/dependency version and evidence confidence.
- workload profile and external-client protocol/endpoint compatibility;
- simultaneous active generations, reusable inactive sessions, guaranteed context, and elastic burst context as distinct concepts;
- llama.cpp prompt-cache/host-cache policy with bounded retained-memory budget;
- text-completion versus structured-chat formatting ownership;
- request-default provenance and explicit third-party client precedence.

### 4.2 Advanced, Experimental, or API-workload controls

- deterministic response cache;
- TurboQuant `v4`/`k8v4`/`none`;
- system-prompt pinning;
- disk KV checkpoint interval/cap;
- prefix-cache index regression fallback;
- prefill step size;
- streaming interval;
- paged cache and block controls while upstream calls it experimental;
- default sampling overrides;
- llama-server bundled Web UI availability, UI config/file, and custom static path;
- llama-server Web UI MCP proxy and built-in tools only under separate Experimental security gates;
- llama.cpp cache reuse/FIM trial and non-default context checkpoints;
- foreground/background admission experiments and workload-specific stream-usage injection;
- cache export/import only after a concrete app-owned workflow and security review;
- PFlash tool-inclusion and endpoint-collapse diagnostics.

### 4.3 Troubleshooting-only escape hatches

- `force-hybrid` / `no-hybrid`;
- `force-spec-decode` / `no-spec-decode`;
- Harmony force-on/off;
- parser auto-detection disable overrides;
- any version-probed fallback used to diagnose upstream profile errors.

These controls need warnings, compatibility evidence, mutual-exclusion validation, and command preview. They are not normal performance recommendations.

### 4.4 Intentionally omitted

- all audio functionality;
- `--watchdog-ppid` and `--listen-fd` as user controls;
- inline CLI API/cloud secrets;
- hidden deprecated/no-op Rapid flags;
- interactive `rapid-mlx chat`, share/tunnel, upgrade, agent/bootstrap, and `jlens` commands;
- waybarrios-only flags without a deliberate new backend/feature;
- arbitrary free-text arguments.

## 5. Major Design Decisions: Alternatives and Recommendations

Every decision below is more than minor. A Builder must not silently choose the non-recommended approach. If new evidence invalidates the recommendation, stop and return to the coordinator.

### D1. Estimator architecture

**Approach A — accepted:** create a normalized `ModelMemoryProfile` with field-level evidence, plus backend-specific execution policies (`LlamaCppExecutionPolicy`, `RapidMlxExecutionPolicy`) feeding strictly backend-native calculators into a common `MemoryBreakdown`. The shared profile describes model geometry and components, not runtime flags or allocation behavior. Existing GGUF introspection remains authoritative and regression-protected; MLX metadata must never be translated into llama quantization vocabulary.

Why:

- preserves one result/card vocabulary while keeping runtime semantics honest;
- expresses requested vs effective KV dtype and downgrade reasons;
- represents asymmetric TurboQuant, concurrent sequences, cache pools, and additive companion models;
- prevents llama.cpp flags from leaking into Rapid payloads.

**Approach B:** maintain completely separate llama and MLX model-profile/estimator types and translate only their final results into a common API response.

Approach B isolates implementation risk but duplicates architecture/lineage evidence, MoE/active-parameter handling, companion ownership, provenance, context inputs, and result semantics. It also makes GGUF-versus-MLX comparisons vulnerable to metadata-model drift. Reject unless Phase 0 proves the normalized geometry boundary infeasible.

### D2. Architecture metadata representation

**Approach A — accepted as part of D1:** normalize layer groups from real config (`full`, `local/sliding`, `linear/recurrent`) with per-group layer count, KV heads, head dimension, window/state geometry, and evidence provenance. Include weight/quantization components, dense/MoE active topology, embedded MTP, vision/companions, model context ceiling, and per-field confidence.

**Approach B:** add family-specific Qwen3.6 and Gemma4 structs/branches.

Family branches are faster initially but brittle for opaque finetunes and the next hybrid architecture. Use them only as parsing adapters that populate the normalized representation, never as the estimator’s public model.

### D3. Rapid context semantics

**Approach A — accepted:** call it **Planning context per request**, persist it as an estimator/recommendation target, calculate memory and fit for that target, and treat it as advisory for direct external-client traffic. External OpenCode, Hermes/OpenClaw, and SillyTavern clients own history compaction and response reserve; Rapid/model runtime limits remain the actual ceiling. Llama Monitor built-in chat or a separately qualified future proxy path may offer Advisory, Warn, or opt-in Strict reject, but the product must never imply it enforced a request it did not receive. Capability-probe any future native Rapid context-ceiling control before offering backend enforcement.

**Approach B:** require all external clients to use a new Llama Monitor inference proxy so one app-owned context policy can be enforced.

Approach B could centralize enforcement, default injection, admission, and diagnostics, but creates a substantial authenticated streaming/protocol/cancellation/retry/tool/raw-completion proxy surface. Do not introduce it merely to enforce an estimator target. The UI must distinguish planning fit from the model's architectural ceiling and from client-owned compaction.

### D4. Cache fit presentation

**Approach A — accepted:** show two scenarios on every memory surface:

1. **Active workload fit:** weights, active KV/recurrent state at planning context, expected simultaneous generations, work/runtime buffers, companion models, OS/app reserve, and safety headroom.
2. **Reusable-cache growth:** current retained bytes, recommended working set, configured cap, remaining safe headroom, and pressure/eviction risk.

Anchor “will it run?” to active footprint plus safe headroom. A cache cap is not presented as eagerly allocated. On unified memory, also show the combined outcome if retained state grows to its cap. Compact cards use a primary Fits/Tight/Does not fit state plus a secondary cache-growth state; expanded Wizard, Preset Editor, Model Library, HF, and welcome details share the same backend result and breakdown.

**Approach B:** add the full configured cache cap into one headline total.

That is conservative but misleading because a cache cap is not necessarily eagerly reserved. It can mark viable models as impossible. Reject as the primary presentation.

### D5. Typed source handling

**Approach A — accepted:** one shared source codec/view model; `RapidMlxModelSource` is the only writable, authoritative model identity. Continue reading legacy `model_path` indefinitely, migrate it to a typed source at the read/save boundary, and stop writing it after this phase. If both fields exist, typed source wins and a disagreement is surfaced as migration/diagnostic evidence rather than silently synchronized. Preserve free-form compatibility through the typed Alias variant. Keep `served_model_name` separate as the API-facing name, never a model-source fallback.

**Approach B:** mirror a display string into `model_path` forever.

Mirroring preserves the exact split-brain bug found here. Reject.

### D6. Backend settings UI

**Approach A — accepted:** use one backend-owned semantic setting catalog and reusable frontend settings system across the wizard and preset editor. Rust owns stable IDs, types, defaults/omission behavior, capability and evidence state, validation constraints, mutual exclusions, requested-versus-effective semantics, and whether a value controls launch, proxy/request behavior, or recommendation input. The frontend owns categories, ordering, progressive disclosure, responsive layout, educational copy, and purpose-built interactions. Simple fields may use shared renderers; model source, context, cache, VRAM fit, and conversation-format ownership use custom UI bound to the same descriptors. This is not a fully server-generated form.

Every exposed setting must trace through `capability/evidence -> typed schema -> shared UI binding -> validation -> launch or request mapping -> save/restore/edit -> review summary -> diagnostics -> tests`. Until that trace is complete, hide the control or show clearly read-only eligibility.

**Approach B:** keep a shared frontend-owned JavaScript catalog/components while Rust independently owns API types and capability results.

Approach B improves over duplicated markup, but still duplicates semantic authority across Rust and JavaScript. Contract tests can detect some drift but cannot make the frontend authoritative about runtime support, launch mapping, or effective safety downgrades. Use frontend ownership for presentation, never for backend semantics.

### D7. Preset information architecture

**Approach A — accepted end state:** use seven stable categories, in the same order for llama.cpp and Rapid-MLX, with backend-adaptive controls and explanations:

1. Model & compatibility
2. Memory & context
3. Performance
4. Generation & reasoning
5. Tools & conversation formatting
6. Network & observability
7. Advanced

The category shell should not unpredictably reorder when the backend changes. Hide empty subsections, not the user's learned navigation. Use summaries, progressive disclosure, and changed/recommended filters to manage complexity rather than mixing unrelated controls.

**Approach B:** use five broader categories—Model & Fit, Workload & Context, Speed & Memory, Behavior & Tools, and Serve & Diagnose—with Advanced controls distributed inside them.

Approach B is initially more approachable but creates ambiguous boundaries for memory/context, crowds behavior/tools, and makes rare or risky controls harder to isolate. Implement accepted Approach A in two stages: first restore correct Rapid content and cross-surface parity without broad relocation; then render both backends from the real completed control inventory and move to the seven-category shell only after dark/light/narrow screenshot review and explicit user approval. The shared semantic catalog prevents this staging from creating two independent implementations.

### D8. Spawn wizard ordering

**Approach A — accepted:** preserve the six-step interaction shell but reorganize it as (1) Goal & Guidance, (2) Model & Engine, (3) Fit & Context, (4) Behavior & Endpoint, (5) Review Plan, and (6) Start & Verify. Interactive coding is the default workload. Rename the current Quick/Balanced/Advanced setup "profile" to **Guidance level**—Guided, Customize, and Expert—so it cannot be confused with workload profiles. Deep links from Model Library or Hugging Face prefill Model & Engine while still confirming the workload assumptions that drive fit and performance advice.

Step 4 contains request-level generation defaults, reasoning, tools/parser compatibility, conversation-format ownership, cache policy, and endpoint/network compatibility using progressive disclosure. Step 5 shows requested versus effective values, memory scenarios, evidence/warnings, command preview, and client setup. Step 6 owns installation/conversion/download progress, launch health, effective capability proof, endpoint details, and client-specific connection guidance.

**Approach B:** use a model-first flow that chooses source/engine before workload intent.

Approach B learns backend capabilities earlier but cannot recommend discovery, quant, context, cache, or endpoint behavior correctly until intent is known. Deep-link prefilling captures its main advantage. Implement the accepted reorder only after parity and the real control inventory are complete, then require dark/light/narrow/error/provisional/long-copy screenshot review.

### D9. Model Library cards

**Approach A — accepted:** use a two-level hierarchy without removing the existing first-class GGUF workflows:

- primary: model/finetune identity, quant/precision, GGUF/native-MLX/convertible-safetensors format, target engine, workload/context fit plus confidence, retained-cache pressure, and best action;
- secondary: original author and artifact publisher/converter, repo/revision/hash/base lineage, lifecycle, tool/reasoning/template/sampling/roleplay compatibility, companions, presets, conversion recipe, evidence, and warnings.

Group GGUF, native MLX, and convertible safetensors variants under one logical model only when authoritative lineage/revision evidence supports the relationship; never group by similar names alone. Within a qualified group, compare backend/artifact variants and quants using the same selected workload/context/concurrency policy while preserving backend-native estimates. Keep original finetune/distillation authorship prominent and show the quantizer/converter as a separate role.

**Approach B:** retain the equal-weight badge rail and add separators.

The current screenshot demonstrates badge overload. Approach A improves the core “what is it, will it run, what do I click?” scan path.

### D10. Hugging Face discovery

**Approach A — accepted:** provide Auto for this workload/device, GGUF/llama.cpp, MLX/Rapid-MLX, and All-candidates search scopes plus the existing explicit Downloads/Likes/Newest/Trending sorting, discover categories, curated-author quick picks, user-customized author list, and externally generated Community Picks. Search is lightweight candidate discovery; an authoritative post-selection endpoint pins a revision and inspects real config/generation/tokenizer/template/index/file/header/lineage/companion/security/runtime evidence. Public search remains tokenless; request a token only for private/gated repositories.

Preserve and improve the existing pre-download fit loop: users may preselect an artifact/model quant; after choosing workload, planning context, active concurrency, and backend-effective KV mode, recompute every available model quant or conversion recipe and let the user explicitly switch before download. Show Best quality that fits, Fits selected context, More context, Better quality if context is reduced, and Does not fit as distinct explanations. Changing context, KV mode, concurrency, model quant, or companion selection reruns the same backend estimate; never silently replace the selected artifact.

For llama.cpp, retain GGUF provider/imatrix/UD classification, alternate-quant discovery, origin resolution, and qualified `mmproj` selection. Rapid-MLX has no fake `mmproj` equivalent: hide that row and represent only real integrated/qualified MLX-VLM components. Native MLX repositories, variants across repos, and app-supported conversion recipes must participate in the same quant/context feedback loop using MLX-native weight/KV math.

**Approach B:** generic search with qualification only after selection.

Approach B is a valid first increment if HF filtering proves unreliable, but it increases incompatible browsing. Never treat `filter=mlx` as proof of Rapid-MLX compatibility.

### D11. Rapid custom chat templates

**Approach A — accepted investigation path:** pursue a native Rapid-MLX template-file/template-config contract. Rapid—not Llama Monitor—must load and apply the selected Jinja through its existing tokenizer/MLX-LM rendering path. Llama Monitor only stores/selects/version-tracks the template, passes the qualified native launch option, and reports effective provenance/capability. First revalidate current upstream, specify and locally qualify the smallest native contract, and return for explicit approval before submitting an upstream issue/PR, pinning an unreleased commit/fork, or changing managed dependency policy.

Treat the existing llama.cpp template library as the product-level precedent while preserving the implementation distinction: llama.cpp already has a native template-file path and therefore does not need a model/tokenizer overlay merely to select an alternative. Improve the shared library experience for llama.cpp, and reuse it for any future native Rapid path, by representing multiple revisioned alternatives per model family, immutable source URL/revision/hash, claimed compatibility, workload-specific qualification, user notes, comparison/trial history, stale/update state, and one-action rollback to the model-provided template. Never silently replace an existing choice or assume the newest community template is universally best.

The current implementation is a useful foundation but not yet this versioned model. `static/js/features/chat-template-registry.js` registers Froggeric through an HF repository and Gemma 4 through one community GitHub `main` URL. `src/web/api/spawn_wizard.rs` stores source/fetch URL, install time, and SHA-256 sidecar metadata and can detect an upstream content change, but the HF installer fetches `raw/main`, each logical template uses one stable destination filename, and a forced update overwrites the active file. There is no immutable HF revision identity, retained release history, comparison set, or rollback target. Preserve the existing SHA/update behavior while migrating it into a release-aware catalog; do not regress current Froggeric discovery or update checks.

For Gemma 4, make the applicable Google-published model repository template a first-class **Official model-author** candidate alongside separately labeled community alternatives. Track Google's template/config history by immutable HF commit and file hash rather than treating mutable `main` as a version. Relevant discussions on the official model repository may identify symptoms, capabilities, proposed patches, or community forks, but discussion claims and linked forks are discovery evidence only: record the discussion URL/comment/revision and claimed fix, inspect the exact fork/revision, and independently qualify it before recommendation. Do not imply Google endorses a linked fork or that an official update fixes every agent/tool workload.

Install a newly discovered release alongside the active release, show a semantic/content diff and claimed/tested capability changes, and require explicit activation. Retain the previous active release and model-provided template for one-action rollback; allow rollback to any retained pinned version subject to a bounded, user-visible retention/cleanup policy. A failed tool/reasoning/role/streaming smoke test must leave the active selection unchanged. Template identity must include provider, repo/source, immutable revision, file path, SHA-256, source role (model-author/community/user), family/scope, and qualification receipt—not a mutable display name.

**Approach B:** create a managed local model/tokenizer overlay that replaces `chat_template.jinja`/tokenizer config while linking or reusing weights.

If separately approved later, Approach B should be a full template troubleshooting and comparison system rather than a one-off file substitution. Start with the pinned model repository template unchanged. When the user observes looping, malformed/missing tool calls, role leakage, reasoning corruption, or other formatting symptoms, offer explicitly selected, revision-pinned alternatives that are qualified for the same model family and workload—for example the relevant Google-published Gemma template or distinct Froggeric releases such as v20 and v21.3. Preserve source URL, immutable revision, template/config hashes, claimed model family, qualification evidence, and user notes. Show “model-provided” versus each overlay as separate provenance; provide reversible trial, comparison, rollback, and stale/update states; never silently replace a template or infer that newer is universally better.

The overlay design must reuse weights without duplicating them where safely supported, materialize a complete immutable tokenizer/config view rather than mutating the downloaded HF snapshot, and prove closure for every referenced tokenizer/processor artifact. Template alternatives must be smoke-tested independently for system/developer roles, ordinary chat, coding, single/parallel tools, malformed/tool-error recovery, reasoning modes, streaming, and multimodal behavior where applicable. A template that fixes conversational looping but breaks tool calls is not qualified for Coding or Tool/research.

Approach B introduces model identity, symlink/copy, update, hashing, cleanup, tokenizer fidelity, and security complexity. It remains a deferred, explicitly gated fallback—not authorization to implement it now. Do not implement it unless native support is rejected/unavailable, this phase stops, and the user approves a separate overlay design and threat model.

Explicitly reject a Llama Monitor Jinja renderer, request-rewriting proxy/shim, or pre-rendered raw-prompt substitution; those paths bypass or risk corrupting Rapid tool/reasoning/multimodal behavior.

Gemma 4 evidence verified on 2026-07-19 and requiring Phase 9 refresh:

- Google official `google/gemma-4-31B-it` current repository revision `b9ea41a2887d8607f594846523f94c6cc75ac8a4` and canonical template: <https://huggingface.co/google/gemma-4-31B-it/blob/b9ea41a2887d8607f594846523f94c6cc75ac8a4/chat_template.jinja>
- Merged official-repository discussion/PR #118 documents null handling, reasoning preservation, turn balance, input validation, later tool-response/turn-closure fixes, and multiple successive commits before merge: <https://huggingface.co/google/gemma-4-31B-it/discussions/118>
- The same discussion links the jscott3201 community template currently represented in Llama Monitor and explicitly compares it with Google's evolving official version; this is provenance/discovery evidence, not proof that either template is universally superior.
- Still-open official-repository reports demonstrate why qualification must remain workload/revision specific: multi-round tool calling #115, reasoning reinjection/repetition #119, and rendering/newline behavior #135: <https://huggingface.co/google/gemma-4-31B-it/discussions/115>, <https://huggingface.co/google/gemma-4-31B-it/discussions/119>, and <https://huggingface.co/google/gemma-4-31B-it/discussions/135>.

### D12. Remote-code trust

**Approach A — accepted:** distinguish Rapid's permission from a repository's actual use of it. A pinned, inspected, data-only MLX repository with no custom-code declarations launches normally without a remote-code consent dialog. Detect `auto_map`, referenced Python, custom configuration/tokenizer/processor classes, and equivalent dynamic-loading hooks. If custom repository code is required, explain the detected evidence and require consent scoped to the exact source and immutable revision; if inspection is incomplete, show a provisional warning before launch. Browsing, search, metadata inspection, qualification, and VRAM estimation never execute repository code. Persist provenance and continue seeking a native Rapid force-off flag as defense in depth.

**Approach B:** accept Rapid’s unconditional `trust_remote_code=True` behavior and document it.

Approach B maximizes compatibility but incorrectly makes ordinary data-only MLX repositories look dangerous and grants consent too broadly. Do not use blanket consent or a blanket warning merely because Rapid sets `trust_remote_code=True`.

### D13. Runtime capability profiles

**Approach A — accepted with automation constraint:** persist a bounded, automatically generated capability snapshot keyed by exact executable identity/version/help hash and resolved dependency versions. Trust a successful installation that satisfies Rapid-MLX's published `pyproject.toml` contract as the normal upstream-supported baseline; do not require the user or maintainer to manually certify each fast-moving release, and do not show a global Provisional/disclaimer banner merely because the exact dependency tuple is new to Llama Monitor.

Build qualification as automation, not a human release treadmill. On-device discovery must enumerate installed package metadata, parse version/help/info output, test optional imports/extras, and run bounded non-destructive self/protocol probes. CI should watch new Rapid releases or selected commits, create clean environments from Rapid's own dependency contract, exercise the representative text/tool/guided/template/cache/model matrix, and publish a machine-readable capability/regression manifest consumed by the app. Cache results until the executable/dependency/help fingerprint changes. Escalate to a human only when automation detects a concrete incompatibility or cannot safely determine a consequential Advanced capability.

User-facing warnings must be actionable and evidence-bearing: missing extra, failed import/probe, known-incompatible dependency, removed/changed flag, unsafe source, or an explicitly selected Advanced feature whose support cannot be determined. A new-but-upstream-compatible environment keeps ordinary features low-friction; uncertain optional controls may stay hidden or be labeled only at the control itself without degrading the entire runtime. Preserve the exact resolved receipt and a last-known-good rollback for app-managed installs, but do not replace Rapid's upstream ranges with a manually curated dependency matrix by default.

**Approach B:** bump hard-coded flags and versions each release.

Current hard-coded qualification says 0.10.10 while the branch already assumed 0.10.12. Approach B will keep drifting. Use hard-coded gates only for demonstrated safety/incompatibility boundaries; otherwise render optional controls from automated discovery, probes, and upstream declarations.

### D14. Response-cache exposure

**Approach A — accepted:** default Off and keep it out of normal-user recommendations and the primary Wizard path. Expose it only as an expert Advanced control for an explicitly selected Deterministic batch/eval API workload. Explain that normal OpenCode-style streaming, evolving agent transcripts, Hermes/OpenClaw tool/MCP turns, Llama Monitor chat, and sampled/streaming SillyTavern traffic receive no benefit. Treat tool-enabled responses as ineligible for an app recommendation until replay/idempotency behavior is explicitly qualified. Do not claim a universal numeric recommendation until byte/working-set telemetry exists.

**Approach B:** expose an ordinary cache-size field with an example such as 256.

No primary upstream evidence supports `256` as a response-cache recommendation, and the cache has no byte cap. Reject Approach B.

### D15. Hybrid/SWA cache guidance

**Approach A — accepted and simplified:** present the shared user concept as **Reusable prompt state: Auto / Off / Custom**, with Custom under Advanced. Auto is the normal default and uses backend-native controls. For Rapid, derive the smallest memory-safe hybrid/SWA working set from model eligibility, byte-stable agent-prefix behavior, predicted/observed entry bytes, PFlash interaction, prefix-cache budget, and available headroom—without a normal-user cache questionnaire. Optimize for the dominant single-user interactive loop, not rare peak overlap: approximately two states for one revisited coordinator conversation and up to four when a sequential helper has a revisited transcript and memory permits. A brief one-shot or 5–10 minute Hermes/OpenClaw cron job may cold-prefill or evict rather than causing permanent overprovisioning. Increase toward six-to-eight only for regularly revisited long-lived sessions with memory fit and observed benefit.

Auto resolves to Off when the model is ineligible, states do not fit safely, prefixes are unstable, PFlash owns the same optimization, or the workload is one-shot. Off disables only Rapid's special non-trimmable retention, not every form of ordinary prefix reuse. Custom exposes the raw entry ceiling plus predicted/current bytes, budget, hits, misses, and evictions. Runtime telemetry may recommend a different value for the next launch, but must never silently mutate the preset or restart the runtime.

**Approach B:** expose raw Off/1/4/custom presets and rely on the global byte cap.

Approach B cannot explain why requested entries may not fit or why N=1 may yield no useful hit. Use it only as a temporary Advanced fallback while telemetry is unavailable.

### D16. Workload profiles and recommendation authority

**Approach A — accepted:** expose five transparent workload profiles—Interactive coding agent, Tool/research agent, Roleplay/storytelling, General chat, and Deterministic batch/eval API—as inputs to a recommendation engine. Interactive coding is the default; General chat is supported but secondary; Deterministic batch/eval is Advanced. Show the assumptions they set (streaming, tool use, prompt construction/format owner, stable-prefix likelihood, hot sessions, concurrency, scheduling/delegation, sampling ownership, response-cache eligibility), let the user change them, and require confirmation before persisting derived settings. Treat `80% coding / 20% tool-research` as product priority, never as a blended preset or resource-allocation formula.

**Approach B:** silently apply an “agent optimized” preset.

Approach B hides the exact properties that determine whether caches help and becomes wrong as soon as a harness changes streaming, tool order, or concurrent jobs. Reject it.

### D17. Response caching for tool-enabled agents

**Approach A — accepted:** keep the process-wide response cache Off for ordinary coding/research agents and never recommend it for a runtime serving OpenCode, Hermes, OpenClaw, MCP, or other tool-enabled traffic. Permit nonzero Custom only for a dedicated deterministic, non-streaming endpoint/workload whose tools are absent or explicitly proven replay-safe/idempotent. Teach that Rapid caches the assistant response/tool-call object; it does not cache downstream tool results or make downstream execution idempotent. A runtime shared between agent traffic and an eligible deterministic caller remains Off process-wide.

**Approach B:** enable it for deterministic tool-calling retries because the exact request key prevents semantic drift.

Approach B may save decode time, but the cached assistant tool call can be delivered again after a client/network retry and the harness may execute it again. This is unacceptable as a default for filesystem, shell, issue tracker, email, or other side-effecting tools. A future per-request or no-tool cache policy could narrow the risk, but current Rapid exposes a process-wide entry count.

### D18. llama.cpp host prompt-cache budget on unified memory

**Approach A — accepted with a unified-memory conservative default:** remove the automatic `--cache-ram -1` behavior. Use shared **Reusable prompt state: Auto / Off / Advanced Custom**. On Apple unified-memory systems, Auto resolves the llama.cpp host prompt-state cache to `--cache-ram 0` for the normal single-user Interactive coding profile: ordinary per-slot/common-prefix `cache_prompt` reuse remains available, while extra host-backed saved states do not compete with weights, active KV, applications, and OS headroom. A brief Hermes/OpenClaw cron or one-shot helper may reprocess rather than permanently retaining another state.

Auto may recommend a bounded positive unified-memory cap only when the user regularly revisits multiple interleaved long-lived conversations, the estimator proves surplus memory after weights/active KV/slots/buffers/companions/OS-app reserve/safety headroom, and predicted or observed reuse justifies the cost. Require confirmation before persisting that change. On discrete-GPU systems, Auto may use a bounded system-RAM allowance when host headroom and transfer/reuse evidence support it. Show active footprint and possible retained prompt-cache growth separately; allow Unlimited (`-1`) only as an explicit Advanced Custom choice with memory-pressure warning and telemetry.

**Approach B:** retain `-1` because the cache is allocated on demand and agent sessions benefit from maximum reuse.

Approach B avoids an eager reservation but still permits unbounded growth from the same unified-memory pool used for inference and the desktop. Long coding/tool sessions are exactly the workload likely to fill it. Reject as an automatic default. Do not describe the 8 GiB default cap as a pre-reservation; it is a demand-filled ceiling.

### D19. Agent concurrency and llama.cpp slots

**Approach A — accepted:** treat the normal home/single-user policy as one active foreground generation with rare background work queued. Keep **simultaneous active generations** separate from hot reusable sessions. Recommend two only when the user explicitly requires interactive coding to remain available during a background Hermes/OpenClaw job and the memory/throughput estimate fits; do not infer that need from the mere existence of cron jobs or sub-agents. Higher values require an explicit multi-client throughput workload and measured evidence. Llama.cpp MTP remains an explicit `--parallel 1` single-stream mode; Rapid retains admission behavior but its MTP fast path is opportunistic for one live eligible request as specified separately.

**Approach B:** default the 80/20 profile to two slots because OpenCode and scheduled jobs may overlap.

Approach B avoids queue blocking but permanently increases memory pressure and can slow the 80% foreground workload even when no cron job is running. Prefer an explained choice and, later, priority-aware admission if the backends support it reliably.

### D20. Cross-backend cache vocabulary

**Approach A — accepted:** present a shared conceptual hierarchy—Active context/KV, Reusable prompt state, Exact completed-response cache, and Persistent/disk state—then progressively disclose backend-native controls, units, effective values, memory location, eligibility, and limitations inside each level. Shared intent must never imply shared implementation: llama.cpp host-cache MiB/slot behavior and Rapid prefix-budget/hybrid-entry behavior remain separately estimated and mapped.

**Approach B:** force llama.cpp and Rapid settings into identical field names and values.

Approach B would repeat the current `ctk`/`ctv` mistake. Shared concepts should improve comprehension; command semantics, memory locations, eligibility, and recommendation formulas remain backend-specific.

### D21. SillyTavern prompt-format ownership

**Approach A — accepted:** model two explicit integration modes: (1) **client-formatted instruct/text completion as the Roleplay default**, where SillyTavern alone owns context/instruct/role/special-token formatting; use llama.cpp `/completion` for its dedicated connector or Rapid-MLX `/v1/completions` through a compatible Generic/VLLM-style SillyTavern Text Completion connection; llama-monitor/backend chat-template selection is irrelevant to both raw-prompt paths; and (2) OpenAI Chat Completions, where the client sends structured messages and the backend applies the selected/model template. Show the exact endpoint/mode and test only combinations whose payload/stream contracts pass qualification.

**Approach B:** recommend one universal OpenAI-compatible configuration and let users troubleshoot formatting failures.

Approach B hides the most consequential roleplay integration error: duplicated roles/special tokens or the wrong template. Reject it.

### D22. Roleplay sampling defaults

**Approach A — accepted:** provide clearly labeled model-family/evidence-aware sampling modes from D27, but preserve omission-only semantics so explicit SillyTavern request samplers always win. Explain sampler interactions and expose the effective request in diagnostics.

**Approach B:** encode global server sampling defaults as the recommended Roleplay preset.

Approach B can unexpectedly override or combine with SillyTavern presets and makes two clients using the same endpoint behave differently from what their UI shows. Keep client-owned values authoritative.

### D23. llama.cpp context and slot contract

**Approach A — accepted:** replace `context_size + parallel_slots` assumptions with an explicit workload policy: target context per active request, expected simultaneous generations, hot reusable sessions, admission/queue ceiling, KV pool mode (`unified`, `partitioned`, `auto`), shared KV capacity, minimum guaranteed per-request context, burst maximum when other requests are idle, and client compaction owner. Launch and estimator derive from the same policy. Keep the normal home-user view to Context target, Auto active requests (one by default), and the resulting guarantee; reveal pool details when overlap or Advanced is selected. Hot inactive/reusable sessions never multiply active KV and remain in the separate retained-state scenario.

**Approach B:** keep `context_size` as a per-request UI value and silently multiply/translate it when building `--ctx-size`.

Approach B is feasible but hides potentially huge total allocations and does not explain elastic unified-pool contention. Use a derived total internally only if the UI review proves the full policy is too complex, and still expose guaranteed versus burst context.

### D24. Speculation versus overlapping agents

**Approach A — accepted:** treat llama.cpp MTP as an explicit **single-stream mode**: lock `--parallel 1` before save/launch, explain that sub-agents and scheduled jobs queue, and recommend sequential orchestration to preserve cache/checkpoint locality. Never silently rewrite a conflicting saved value. A multi-agent/overlap profile disables MTP and enables a separately estimated slot/context-guarantee policy. Multi-sequence MTP remains Experimental and separately qualified even when a current build technically initializes it. This matches the user's proven RTX 5090 operating practice and remains safe for builds/models that truly require one slot.

**Approach B:** on a specifically qualified current build such as `b10068`, allow experimental MTP with multiple slots because upstream now maintains per-sequence state and initializes MTP with `n_parallel`.

Approach B is technically possible in current upstream, but recurrent-state memory grows with parallelism and upstream reports little general MTP benefit during parallel generation. Keep it out of Auto recommendations until model/build/hardware benchmarks prove otherwise. In either approach, never let the UI accept N slots and silently submit 1.

### D25. Rapid-MLX MTP and concurrency

The audited source requires a per-family companion audit before presenting MTP as “native” or “sidecar-free.” Qwen3.5/3.6 detection uses intrinsic config metadata, but the production injector constructs an MTP module and refuses non-test operation without separate sidecar weights; HY3 automatically resolves a revision-pinned external sidecar because converted base weights strip the MTP head; Gemma4 has a four-layer Google assistant loader but is absent from the production dispatch table and detection explicitly leaves sidecar promotion disabled pending validation. Conflicting source comments that call a path baked-in/native do not override the actual loader. Represent every loaded MTP weight source and cache separately and stop if eligibility, dispatch, and weight ownership disagree.

**Approach A — accepted, memory-first default with optional exploration:** default the normal single-user **Active generations: Auto** policy to one for near-capacity model/context fitting. Resolve Rapid's effective `max_num_seqs` to one when needed to guarantee that direct external clients cannot accidentally admit a second long request outside Llama Monitor's control. The MTP sidecar/native weight component is loaded once per process and remains additive even when a request falls back; each additional active request adds target KV/recurrent/working state and may add per-request speculative state, but not another full copy of the sidecar weights. Show the exact base weights + MTP companion + one-active-request KV/state + buffers + retained state + OS/app reserve fit.

Offer **Allow overlap** as an Advanced explicit alternative. It preserves admission capacity (initially two, Custom only with evidence), models the worst allowed simultaneous active-request memory and reduced guaranteed context, and refuses or recommends a smaller quant/context when safe headroom is insufficient. At the audited commit, MTP activates only for an eligible one-request greedy batch without incompatible logits processors; overlap uses ordinary autoregressive decoding. Do not call the handoff lossless until the exact runtime passes mid-stream overlap tests—the audited scheduler source documents a bounded duplicate/stale-token risk in some MTP-to-plain-decode transitions. Show active/fallback reasons and observed peak memory.

**Approach B:** preserve upstream admission capacity by default and rely on opportunistic MTP fallback plus runtime memory pressure.

Approach B avoids queueing but cannot honor a near-maximum quant/context fit when direct clients overlap: the concurrency ceiling itself is not an eager reservation, yet a second admitted long request can grow another active KV/state working set and exceed headroom. Keep it as the explicit Advanced overlap choice rather than the normal memory-first default.

### D26. llama.cpp Web UI MCP proxy

**Approach A — accepted and extended:** default `--ui-mcp-proxy`/`--webui-mcp-proxy` Off. External clients bring their own MCP/tool orchestration. Add a coherent Advanced **llama-server Web UI** group rather than exposing the proxy alone. Capability-probe the exact build and represent:

- **Web UI availability:** Auto/follow-upstream, On, Off (`--ui`/`--no-ui`), plus a post-launch “Open llama-server Web UI” action using the effective host/port/API prefix. The audited build enables the bundled UI by default; preserve that baseline unless a later explicit security/UX decision changes it.
- **Default UI preferences:** validated `--ui-config` JSON or canonicalized `--ui-config-file`, with requested/effective preview and safe failure. Prefer a file/library workflow for reusable settings rather than forcing long raw JSON into the normal form.
- **Custom static UI path:** Expert-only `--path`, canonicalized to an authorized directory with traversal/symlink/size/lifecycle checks; never imply it is required for the bundled UI.
- **MCP CORS proxy:** Experimental, Off by default, loopback-only by default, with explicit explanation that it is for the bundled browser UI and unrelated to OpenCode/Hermes/SillyTavern MCP/tool orchestration.
- **Built-in tools:** treat `--tools` as a separate Experimental Agent Tools capability, not an innocuous Web UI preference. If later approved, use an explicit tool allowlist and distinguish read-only (`read_file`, search, datetime) from mutating/shell tools (`write_file`, `edit_file`, `exec_shell_command`). Require a dedicated threat model, loopback/auth/network constraints, visible effective list, and prompt-injection warning. Do not expose upstream `--agent` as a magic one-click bundle because it silently enables all built-in tools plus the MCP proxy.
- **Network/access:** keep host, port, API prefix, API key, TLS, and CORS in the existing Network & Access category, but show their effective security consequences beside Web UI/proxy/tools. Non-loopback UI access, permissive origins/credentials, or tools require explicit review; never duplicate these semantics in a separate Web UI schema.

Do not place these controls in the normal Wizard path. The ordinary external-agent preset omits the MCP proxy, built-in tools, and `--agent`; the bundled UI may remain available according to the explicit/follow-upstream UI setting.

**Approach B:** retain it because it may make future embedded Web UI tools work automatically.

Upstream marks the proxy experimental and unsafe for untrusted environments. Future convenience does not justify unconditional exposure. Reject.

### D27. Cross-backend sampling mode catalog and default authority

**Approach A — accepted:** replace filename-centric/default-only handling and the wizard's competing hard-coded table with one Rust-owned `SamplingModeCatalog`. Every model and both backends expose the complete applicable mode selector across Wizard, Preset Editor, review/card summaries, and diagnostics. Preserve all curated family modes; add universal Model/author defaults, General, Coding/Agentic, Precise/Deterministic, Creative/Roleplay, and Custom choices; add family-specific modes where meaningful. Each mode carries stable ID, full parameter bundle, reasoning/thinking coupling, best-use badges, source URL/revision/retrieval date, source class, family/lineage match evidence, backend field coverage, and workload recommendation rank.

Unsloth-published values are authoritative by user decision and remain exact. Other sources are visibly distinguished. Resolve finetunes from authoritative metadata/lineage first, filename only as fallback, and allow a persisted manual family override with provenance. Selecting a mode creates omission-only server/request defaults: explicit external-client or app-chat values always win and explicit falsy values are preserved. For llama.cpp, map supported defaults to launch flags. For Rapid, use its native default cascade and capability-probed `--default-*` flags; seek upstream coverage for missing fields rather than claiming proxy-only parity.

**Approach B:** keep family branches plus generic fallback pills in `model_defaults.rs`, duplicate use-case defaults in the wizard, and add Rapid mappings opportunistically.

Approach B is the source of the current finetune, visibility, provenance, and cross-surface drift. Reject.

### D28. Estimator calibration and confidence qualification

**Approach A — accepted:** use three evidence levels: Qualified, Calculated, and Provisional. Qualified requires real architecture metadata, pinned backend/runtime/model/hardware/configuration evidence, reproducible measurements, and estimates within a target ±10% over the stated qualification envelope. Calculated has complete reviewed geometry/math but lacks a matching measurement; it displays an explicit uncertainty range. Provisional contains inferred/missing geometry or an unqualified runtime path and cannot produce an unconditional Fits result near the boundary.

Preserve the existing llama.cpp RTX 5090/CUDA-WDDM and Apple M5 Max/Metal calibrations as versioned regression evidence; do not replace their calibrated overhead with generic formulas. Apply the same methodology to Rapid-MLX using platform-appropriate process, Metal, and unified-memory-pressure measurements. Cover dense, MoE, and hybrid/recurrent families; Qwen3.6 dense/A3B; Gemma4 dense/A4B; multiple contexts, effective KV modes, active sequences, cold/active states, retained-cache growth, and companion models. A measurement qualifies only its documented hardware/runtime/model/configuration envelope. Missing M2/M3/M4 or other hardware does not block implementation; it remains Calculated until measured, while the existing M5 Max baseline remains valid evidence for its qualified path.

Even a Qualified estimate becomes Tight when remaining margin is smaller than estimator uncertainty plus required safety headroom. Store raw observations, commands/procedure, versions, model revisions, predicted/observed component totals, residuals, and known platform measurement limitations.

**Approach B:** use source-derived formulas plus one broad 15–25% safety multiplier and label every backend/model Approximate.

Approach B avoids false precision but discards the first-class calibration standard already established for llama.cpp, over-restricts viable contexts, and hides where evidence is genuinely strong. Reject.

### D29. Curated community finetunes, distillations, and alignment variants

**Approach A — accepted:** promote the current GGUF quantizer quick-pick concept into a user-editable `CommunitySourceCatalog` with explicit roles such as original model/finetune author, dataset/merge/distillation author, GGUF quantizer, MLX converter/publisher, and curator. Preserve the bundled high-quality/popular creators—including the existing heretic/abliterated/uncensored specialists—as first-class discovery lanes and ranking signals. Keep Heretic/uncensored, updated-dataset finetunes/distillations, coding, roleplay, and other curated categories visible rather than burying them under official/base models.

Use broad community-first discovery as the product default. Give users persisted discovery preferences such as Include/Prefer/Only curated creators, Include/Prefer/Only heretic or uncensored variants, and Include/Prefer updated finetunes/distillations. An installation may remain configured to prefer heretic/uncensored and updated-data models without imposing that ranking globally. These preferences change discovery/ranking, not technical truth. A creator's curated status, popularity, or community reputation is valuable product evidence but remains separate from immutable-revision format/runtime/template/tool/security/fit qualification.

Rapid-MLX receives equivalent treatment even when the original creator publishes no native MLX repo: discover the authoritative safetensors finetune, preserve original authorship and base/dataset lineage, then offer a qualified native MLX derivative if one exists or an app-supported conversion recipe with explicit disk/time/quant/provenance implications. When a community converter republishes the model, display original finetune author and converter separately. Do not make `mlx-community` or any converter appear to be the model's original author.

**Approach B:** keep curated creators and heretic/finetune discovery GGUF-only, while Rapid search favors official/base or already-known native MLX repositories.

Approach B would make Rapid-MLX second-class for the user's dominant model-selection workflow and discard one of llama-monitor's most valuable discovery capabilities. Reject.

### D30. Unified-memory readiness, recovery, and high-memory-app guidance

The existing app already collects useful macOS evidence, but it does not yet have one trustworthy definition of memory available for inference. The audited paths disagree materially:

- Wizard and Preset Editor treat the configured/heuristic Metal wired cap minus 512 MiB as available, largely independent of current application pressure: `static/js/features/spawn-wizard.js` (`metalCap`, `effectiveAvailBytes`) and `static/js/features/presets.js` (`_presetAvailBytes`).
- Welcome/setup cards derive a smaller live budget from non-reclaimable use, a fraction of purgeable plus inactive pages, an OS reserve, and a separate safety margin: `static/js/features/setup-view.js` (`_renderUnifiedMemoryBar`).
- Model Browser/HF preview sums Apple unified memory reported as GPU total and passes the whole value as `available_vram_bytes`: `static/js/features/models.js` (`fetchGpuVram`, `updateVramDisplay`).
- `src/gpu/apple.rs` maps mactop's system-wide unified-memory `memory.used` to `vram_used`; that is not process-specific Metal allocation and must not be presented as such.
- The top-process endpoint exists and a safe DOM renderer exists, but the renderer has no call site/container. The API currently returns full command lines and RSS, which can disclose command-line secrets and is not the same as unique physical footprint or Metal ownership: `src/web/api/system_tools.rs` and `static/js/features/dashboard-render.js`.
- Cache purge is real, authenticated, confirmed at the API, and cooldown-limited, but different UI callers handle failure inconsistently. Setup can show success when the JSON body is `{ok:false}` because it checks HTTP/error fields but not `ok`; no caller measures before/after benefit.
- The wired-limit mutation has administrator-token and native-authorization protection. The user has directly confirmed that the app-applied 57,344 MiB value persists across reboot on the audited 64 GiB M5 Max setup, so preserve that working path. The implementation still lacks RAM-relative bounds, a restore/default operation, exact equality verification, and recorded OS/mechanism evidence for portability; `actual >= requested` can falsely report a failed lowering as success. A wired limit permits Metal to wire more memory; it does not make currently occupied memory free.
- Rapid Wizard initialization currently skips the normal GPU/RAM/wired-limit fetches and VRAM scheduling under its `if (!rapid)` branch, so Rapid can inherit zero or stale caches from a prior llama/HF flow rather than receive first-class memory evidence.
- `/api/vram-estimate` centralizes estimator math but trusts caller-supplied `available_vram_bytes`; it therefore reproduces whichever incompatible budget a frontend chose. Existing Rapid reusable-prefix fields are not propagated consistently by the shared frontend request, and launched `max_cache_blocks` is not modeled.
- Apple documents `purge` as a disk-buffer-cache flush that does not affect anonymous allocations. It does not release live model weights, KV, Metal buffers, or MLX allocator cache. Current **Free Memory** wording overstates the action for inference fit.

**Approach A — accepted:** create one backend-owned `MemoryAvailabilitySnapshot` and memory-readiness policy shared by llama.cpp and Rapid-MLX and consumed unchanged by Welcome cards, Wizard, Preset Editor, Model Browser/HF, review-before-launch, and runtime diagnostics. It must distinguish:

1. **Safe now:** conservative launch budget under current non-reclaimable use, kernel pressure, swap/churn, OS/driver/app reserve, backend effective limit, and estimator uncertainty.
2. **After freeing cache:** conditional budget based on a conservative bounded reclaim estimate, never all inactive pages and never a guarantee.
3. **After closing applications:** diagnostic scenario based on privacy-safe, grouped high-memory applications. Label RSS/physical-footprint evidence honestly and do not promise every byte is recoverable.
4. **Configured ceiling:** theoretical wired/Metal/backend cap, clearly not current available memory.
5. **Does not safely fit:** required bytes still exceed all qualified scenarios.

The snapshot/request must also state whether launch is **additional** or **replaces an app-owned runtime**. An additional launch keeps every current runtime footprint in use. A replacement scenario may add back only the measured footprint of the runtime that the same action will actually stop; it must not assume unrelated applications or externally managed model servers disappear. Model Browser capacity planning should show stable machine capacity and current launch readiness separately rather than silently choosing one meaning.

Make recovery contextual and distinguish four different actions instead of one misleading purge button:

1. clear a backend's free-buffer allocator cache, where a qualified API exists; active buffers remain;
2. evict or reduce app/runtime-owned reusable prompt/cache state under its own safe policy;
3. stop another app-owned model runtime or ask the user to close a grouped high-footprint application;
4. flush the OS disk buffer cache only as an explicitly described secondary action, never as the normal remedy for live model memory.

Offer an action only when its estimated recovery could change the selected model/context fit result, then remeasure and report actual before/after change. Show grouped high-memory candidates when closing them could plausibly cross the boundary; redact command arguments, aggregate helpers by parent/application where possible, identify app-owned model runtimes separately, and initially provide guidance/Open Activity Monitor or the existing safe stop-runtime action rather than arbitrary process termination. On macOS prefer `phys_footprint` as the primary process-pressure metric and retain RSS only as a labeled secondary diagnostic; pair both with backend allocator active/cache/peak metrics and explain that shared mappings, compression, and timing prevent perfect summation. Store a successful-run envelope—model/revision/quant, context/KV/concurrency, backend/runtime, predicted and observed peak, effective limits, and pressure snapshot—so a configuration that worked previously can say why it is unsafe *right now* and what recovery would restore it.

Integrate accepted A4 in two stages. Initially Rapid Auto follows upstream `0.90` of Metal `max_recommended_working_set_size` and exposes the resolved byte cap. After calibration, Auto may choose a bounded effective value from the same snapshot and estimator. Never combine the raw `iogpu.wired_limit_mb` value and Rapid's utilization as if they were independent pools; the effective Rapid ceiling is the runtime-reported base multiplied by the effective utilization. DFlash/DDTree paths that ignore custom utilization must report that capability truthfully.

**Approach B:** retain backend/raw cap calculations and add a diagnostic top-process drawer plus better purge copy. This is a useful incremental repair, but it leaves conflicting fit answers and cannot safely automate whether a model works now, after reclaim, or after closing applications.

Approach A is the required end state because cross-surface consistency and launch-time safety cannot be added reliably on top of four frontend-owned budget formulas. It may ship incrementally, but no surface may continue calling a theoretical Metal cap or total unified memory “available.”

Treat persistence as empirically verified for the user's current 64 GiB M5 Max/macOS configuration: the app-applied 57,344 MiB value survives reboot. Preserve the existing working mechanism unless validation proves it unreliable. Because primary Apple/MLX sources document the live sysctl but not a universal persistence contract, record the exact macOS version/mechanism, add post-reboot/readback qualification across supported versions, and avoid silently generalizing one machine's result. Only introduce a separate boot mechanism such as a LaunchDaemon if the current approach fails on a supported configuration, and then require explicit opt-in, previous-value capture, uninstall/rollback, and post-reboot verification. MLX snapshots the recommended working-set value at first device initialization, so an already initialized Rapid process must restart after a wired-limit change before its effective base can be trusted.

External evidence to revalidate during implementation:

- Apple Metal `recommendedMaxWorkingSetSize`: <https://developer.apple.com/documentation/metal/mtldevice/recommendedmaxworkingsetsize>
- Apple `sysctl` live-setting semantics: <https://github.com/apple-oss-distributions/system_cmds/blob/408bba7453608006b89772db185defbac8fe2fd0/sysctl/sysctl.8>
- Apple `purge` scope: <https://github.com/apple-oss-distributions/system_cmds/blob/408bba7453608006b89772db185defbac8fe2fd0/purge/purge.8>
- Apple/XNU process RSS and physical-footprint fields: <https://github.com/apple-oss-distributions/xnu/blob/f6217f891ac0bb64f3d375211650a4c1ff8ca1ea/osfmk/mach/task_info.h#L363-L395>
- MLX working-set snapshot, allocator limits, cache clearing, and wired-limit APIs: <https://github.com/ml-explore/mlx/blob/b7c3dd6d27f45b5365b08a840310187dc503f1db/mlx/backend/metal/device_info.cpp#L28-L60>, <https://github.com/ml-explore/mlx/blob/b7c3dd6d27f45b5365b08a840310187dc503f1db/mlx/backend/metal/allocator.cpp#L45-L106>, and <https://github.com/ml-explore/mlx/blob/b7c3dd6d27f45b5365b08a840310187dc503f1db/python/src/memory.cpp#L84-L124>
- MLX-LM wired-memory guidance at pinned source: <https://github.com/ml-explore/mlx-lm/blob/15b522f593b7ca5fbc0cac6f7572d40859d2d8fe/README.md#L271-L283>
- Rapid CLI default/range: <https://github.com/raullenchai/Rapid-MLX/blob/75b1fe3b3a8ab12967f64150524296f179dd9979/vllm_mlx/cli.py#L7222-L7230>
- Rapid Metal-cap resolution and request admission: <https://github.com/raullenchai/Rapid-MLX/blob/75b1fe3b3a8ab12967f64150524296f179dd9979/vllm_mlx/scheduler.py#L2981-L3017> and <https://github.com/raullenchai/Rapid-MLX/blob/75b1fe3b3a8ab12967f64150524296f179dd9979/vllm_mlx/scheduler.py#L3310-L3387>

### D31. TurboQuant retained-prefix policy

**Approach A — accepted:** expose one typed **Reusable prompt storage** policy with **Auto**, **Standard**, **TurboQuant K8V4 — Advanced trial**, and **TurboQuant V-only — Expert legacy/A-B**. Existing presets that omit the field migrate to visible Auto. Auto preserves Rapid's exact upstream alias/runtime resolution; it must not infer eligibility for an unknown or community finetune from a family-like name. Unknown finetunes remain Standard until the exact immutable revision is qualified. Label upstream alias defaults separately from Llama Monitor workload recommendations.

TurboQuant applies only to retained memory-aware prefix snapshots and decompresses those snapshots before model forward. It does not reduce model weights, active-generation KV, recurrent/Mamba state, MTP state, prefill/transient memory, or every cache path. PFlash bypasses it; paged-cache mode does not wire it; hybrid entries may be discarded before compression when their entry limit is zero. K8V4 compresses conventional KV layers while recurrent state remains full. The `none` choice means Rapid's Standard retained-storage policy—normally int4—not uncompressed FP16.

Do not market the stale upstream `4.6x total` claim as a fit guarantee. The pinned implementation implies approximately 34% retained-storage savings for V-only and approximately 57–58% for K8V4 in applicable conventional KV portions, before recurrent/uncompressed components and transient decompression costs. Treat these as implementation-derived planning estimates until measured. V-only Auto may use 3-bit values while still nibble-packing like 4-bit, reducing resolution without reducing index storage; keep it Expert-only.

For the dominant OpenCode coding and Hermes/OpenClaw tool workload, recommend K8V4 only when immutable-revision qualification plus real traces demonstrate stable revisited prefixes, lower retained bytes/pressure, acceptable hit TTFT, and no correctness/tool regressions. Default one-shot research jobs and Roleplay/Storytelling to Standard. Never claim that TurboQuant improves a cold prompt or active-generation fit. Show requested/effective policy, eligibility and fallback reason, stored bytes, realized compression ratio, encode/decode latency, cache-hit TTFT, skip/fallback counts, and transient peak. Keep raw bits/group-size knobs out of normal UI.

**Approach B:** force TurboQuant `none` and expose only Standard until broad workload calibration exists. This is safer but hides a real upstream Auto behavior and prevents qualified users from controlling a potentially valuable retained-prefix trade-off. Reject in favor of transparent Auto plus gated Advanced trials.

External evidence to revalidate during implementation:

- Rapid CLI/default resolution: <https://github.com/raullenchai/Rapid-MLX/blob/75b1fe3b3a8ab12967f64150524296f179dd9979/vllm_mlx/cli.py#L6955-L7010>
- TurboQuant implementation and packing: <https://github.com/raullenchai/Rapid-MLX/blob/75b1fe3b3a8ab12967f64150524296f179dd9979/vllm_mlx/turboquant.py#L1-L190>
- Retained-prefix compression/decompression path: <https://github.com/raullenchai/Rapid-MLX/blob/75b1fe3b3a8ab12967f64150524296f179dd9979/vllm_mlx/memory_cache.py#L1302-L1375>

### D32. Preset schema migration and versioning (cross-cutting)

**Approach A — accepted:** own preset schema evolution with ONE explicit contract instead of ad-hoc `serde(default)` scattered across phases. This plan changes preset shape in many places — typed model source (D5 / A20), sampling mode catalog (D27), cache vocabulary (D20), memory policy (D30), and context/slot contract (D23) — and every one of those is a place where a user's already-saved preset can silently lose meaning.

The contract:

- a preset carries an explicit **schema version** field;
- reads perform **forward migration** from any prior version to current, with the migration path recorded and inspectable;
- **round-trip tests** (save → load → save) prove a preset's meaning survives, and prove that an older-version preset migrates without data loss;
- **downgrade behavior** is defined: a newer preset opened by an older build must fail safe and legible, never silently corrupt;
- every phase that changes preset shape plugs into this contract rather than adding another one-off default; the phase that "completes migrations" (P13) references it as the single source of truth.

This survives the single-cutover release model independently of any staging question: even one final release must migrate the presets users **already have** from today's shipped llama-monitor. The round-trip tests additionally guard against the local executor corrupting preset shape during mid-iteration builds, when many preset-touching phases are in flight at once.

**Approach B:** keep per-field `serde(default)` and let each phase absorb its own shape change.

Approach B is what exists today and is exactly why a typed source can be dropped to a legacy string on read, or a new sampling field can vanish on the next save by an older code path. It provides no version identity, no migration audit, and no downgrade safety. Reject as the owning strategy; `serde(default)` may remain only as a within-version leniency, never as the migration mechanism.

## 6. Required Educational Cache Design

The two 0.10.12 cache flags are not variants of the same feature. They need separate language, eligibility, memory treatment, and recommendations.

### 6.1 `--hybrid-cache-entries N`

Authoritative upstream evidence:

- CLI definition: <https://github.com/raullenchai/Rapid-MLX/blob/75b1fe3b3a8ab12967f64150524296f179dd9979/vllm_mlx/cli.py#L6831-L6859>
- Design, safety argument, measurements, and sizing: <https://github.com/raullenchai/Rapid-MLX/pull/1111>
- Sliding-window extension/correctness: <https://github.com/raullenchai/Rapid-MLX/pull/1124>

#### What it actually does

- Default `0` drops whole prefix-cache entries containing non-trimmable layers.
- `N > 0` retains at most N opportunistic non-trimmable entries with an LRU bound among that subset.
- Relevant cache classes include recurrent hybrid state (GatedDeltaNet/Mamba) and rotated sliding-window state (Gemma4/GPT-OSS).
- The retained object is the whole per-layer prefix entry: full-attention KV, local rotating state, recurrent/conv arrays, quant metadata, and related state—not merely “a conversation ID.”
- Entries still count against the global prefix-cache byte cap and global entry limit. N is a ceiling, not a memory reservation or guarantee that N entries fit.
- Current source exempts `protected=True` persisted/imported entries from the opportunistic/live hybrid candidate bound. Therefore N is not a universal bound over every retained non-trimmable state. Upstream metrics prose is broader than implementation here; Phase 6 must test imported/protected snapshots and seek clarification before final UI wording.
- Safe benefit is primarily byte-stable prefix extension. A rotated sliding-window exact re-request deliberately cold-prefills for correctness; trim-requiring LCP/supersequence paths remain refused.
- PFlash-compressed prompts bypass the normal prefix-cache lookup, so PFlash can eliminate the benefit for the affected long prompts.

#### Upstream sizing evidence

The scheduler stores roughly two non-trimmable entries per turn/conversation: a prompt snapshot and post-generation state. PR #1111 measured three interleaved conversations:

| N | Warm-turn hit rate |
|---:|---:|
| 1 | 0% |
| 4 | 0% |
| 6 | 67% |
| 8 | 100% |
| 12 | 100% |
| 16 | 100% |

The evidence-backed heuristic is approximately:

```text
minimum working entries ≈ 2 × concurrently active reusable conversations
recommended trial = minimum working entries rounded upward with modest headroom
effective entries = min(recommended trial, entries that fit in cache byte/headroom budget)
```

Do not derive N from `max-num-seqs` (default 256). That would imply an enormous, unjustified entry count.

This is one contributor's measured workload and scheduler observation, not a universal optimum. Treat it as a starting hypothesis to validate against the selected model, Rapid build, actual client serialization, imported/protected entries, and memory telemetry.

For the canonical agent workload, a “conversation” means a **hot agent session whose exact byte prefix will be revisited after another state is stored**. Count separately:

- each OpenCode session/worktree that the user alternates between;
- each overlapping Hermes/OpenClaw cron or research job that makes another model turn later;
- delegated/sub-agent threads when they have independent transcripts;
- parallel branches of an agent loop if the client returns to them.

Do not count completed historical sessions, simultaneous HTTP requests that never resume a prefix, raw MCP tool-call count, or the server's admission ceiling. Tool count matters indirectly because a large, stable tool schema can make each hit more valuable; it does not increase N by itself.

Advanced/reference trial tiers, subject to the byte/headroom gate, may explain Auto's calculation but must not become a normal-user questionnaire:

| Peak hot reusable agent sessions | Minimum working estimate | Suggested trial with turnover headroom |
|---:|---:|---:|
| 1 | 2 | 4 |
| 2 | 4 | 6 |
| 3 | 6 | 8 |
| 4 | 8 | 10–12 |

Only the approximately `2 × sessions` minimum is directly grounded in upstream's reported scheduler behavior. The extra two-to-four entries are a llama-monitor trial policy that must be validated by telemetry and memory fit; it is not an upstream default.

#### Recommendation policy

Default UI policy is **Auto**. Rapid's effective hybrid retention remains **Off** until all of these are true:

- model metadata/runtime proves a non-trimmable recurrent or sliding-window cache class;
- workload has a long, byte-stable system/tool/history prefix and append-only turns;
- user identifies one or more concurrently active reusable conversations;
- at least the minimum working set fits inside the prefix-cache policy without consuming required active-model headroom;
- PFlash is not expected to bypass reuse for the same prompts.

For the normal Interactive coding profile, infer the conservative single-user baseline rather than asking a cache-specific question: one revisited coordinator may use approximately two states; a revisited sequential helper may raise the memory-safe ceiling to four. Do not permanently size for a brief one-shot or 5–10 minute background cron. Only advanced workload customization or observed telemetry should increase the long-lived-session assumption. Explain the effective calculation. Example copy:

> Reusable prompt state is Auto. Rapid-MLX may retain up to four hybrid states for your iterative coding session and a revisited sequential helper. Brief background jobs may reprocess rather than reserving memory permanently. Estimated retained memory: 1.3 GiB.

Do not present a numeric effective value if per-entry memory cannot be bounded conservatively. Resolve Auto to Off and offer an explicit Advanced “Run a cache trial” action instead.

Agent-specific best practices:

- Favor the cache for long, iterative coding sessions whose system prompt, tool definitions, repository instructions, and prior transcript remain byte-identical while new tool results/turns append.
- Keep tool/MCP definition ordering deterministic. Reordering JSON properties, tools, repositories, environment banners, timestamps, or capability lists near the front of the prompt can destroy the reusable prefix.
- Treat dynamic system-prompt data—current time, changing git status, rotating workspace summaries, ephemeral IDs—as a likely miss source. Put dynamic data as late as the harness permits; never change agent semantics solely to chase a cache hit.
- Measure OpenCode and scheduled-agent sessions separately. One long foreground coding loop may benefit at N=2–4, while overlapping cron/delegated jobs can evict it and justify a higher count.
- Compare PFlash and prefix reuse on the actual long-agent prompt. PFlash can be better for cold long prompts while bypassing prefix lookup; do not recommend both as if benefits stack.
- Evaluate benefit using TTFT and prefill tokens/time saved across real tool loops, not only cache hit rate.

#### Memory estimate

Pre-launch estimate for prefix length `L`:

```text
entry_bytes(L) =
  full_attention_KV(L, effective dtype)
  + local_SWA_KV(min(L, window), effective dtype)
  + recurrent/conv state arrays
  + quantization scales/bias/group padding

conservative_entry_bytes = entry_bytes × 1.25 until calibrated
requested_retention = N × conservative_entry_bytes
effective_retention = min(requested_retention, assigned cache budget, available safe headroom)
```

Use target stable-prefix length, not advertised maximum context. Runtime observation must replace the prediction with p95 stored-entry bytes once available.

#### Keep disabled when

- pure full-attention/trimmable model;
- short, single-turn, or constantly re-rendered prompts;
- prompt templates/tool ordering are not byte-stable;
- rotated SWA workload consists of exact repeats rather than prefix extension;
- cache is already at cap or pressure evictions are frequent;
- a single predicted entry exceeds safe headroom;
- minimum persistent memory matters more than repeat-turn TTFT;
- a new/unknown non-trimmable cache class lacks a correctness qualification;
- PFlash handles the same prompts.

#### Required telemetry and teaching

Ingest and display:

- configured N and actual non-trimmable entries;
- non-trimmable bytes, p50/p95 entry bytes;
- prefix cache current/cap bytes;
- exact/prefix-extension hits and misses;
- non-trimmable skips;
- global and hybrid-bound eviction reasons;
- prompt tokens and prefill milliseconds saved;
- Metal active/peak changes before/after stores.

Recommendation is validated only when repeat-turn TTFT/prefill work falls, active memory plateaus, and actual entries stay inside both count and byte bounds.

### 6.2 `--response-cache-entries N`

Authoritative upstream evidence:

- CLI definition: <https://github.com/raullenchai/Rapid-MLX/blob/75b1fe3b3a8ab12967f64150524296f179dd9979/vllm_mlx/cli.py#L6860-L6875>
- Implementation: <https://github.com/raullenchai/Rapid-MLX/blob/75b1fe3b3a8ab12967f64150524296f179dd9979/vllm_mlx/response_cache.py#L1-L58>
- Design/release PR: <https://github.com/raullenchai/Rapid-MLX/pull/1123>

#### What it actually does

- Default `0` is inert.
- Stores complete `ChatCompletionResponse` Python/Pydantic objects in a process-local entry-count LRU.
- An exact eligible hit returns the prior completion before GPU admission/decode.
- Eligible requests must be non-streaming, text-only/non-MLLM chat completions, and greedy (`temperature == 0` or `top_k == 1`).
- Sampled/seeded, streaming, and multimodal requests bypass it.
- Concurrent identical cold misses are not single-flight coalesced.
- Model reload clears/epoch-invalidates the cache.
- It has no byte cap. Long reasoning, logprobs, tools, or content can make entry size vary widely.

Normal OpenCode-style coding requests and Llama Monitor chat commonly stream, so this cache provides **no benefit to those streaming calls**. Evolving agent transcripts also rarely repeat the exact canonical request. Label it “Exact-response cache for deterministic non-streaming APIs,” never “Chat cache,” “Agent cache,” or a generic “Faster responses.”

#### Recommendation policy

Default and normal-app recommendation: **Off (0)**.

There is no evidence-backed universal N, and no primary upstream evidence supports 256 as a recommended value. Do not invent one.

Do not offer a built-in guided trial value. The expert may enter a Custom count only after the UI establishes:

- an external OpenAI-compatible client;
- non-streaming requests;
- deterministic greedy generation;
- exact request retries/replays/evals;
- adequate host/unified-memory headroom;
- acceptance that completions remain in process memory.

For tool-enabled agents add two more gates:

- tools are absent, read-only, or explicitly proven idempotent/replay-safe;
- the client retry contract is understood, so returning the same cached assistant tool call cannot unexpectedly duplicate a side effect.

Because Rapid's control is process-wide, a runtime shared by OpenCode, cron agents, and deterministic eval callers should remain Off even if one caller is eligible. Prefer a separate dedicated runtime/endpoint for the deterministic workload until upstream supports per-request policy or exclusion of tool-bearing responses.

Do not suggest 8, 16, 32, 256, or any other entry count. If future byte/working-set telemetry and a bounded memory policy support an evidence-derived value, return for a new product decision before adding a recommendation.

Future recommendation formula:

```text
recommended_N = min(
  p95 eligible exact-repeat reuse working set,
  floor(response_cache_byte_budget / p95_response_entry_bytes)
)
```

Until upstream or llama-monitor measures bytes and enforces a byte budget, no automatic recommendation is allowed.

#### Keep disabled when

- Llama Monitor’s streaming chat is the workload;
- any streaming, sampled, multimodal, or low-repeat client dominates;
- responses contain large logprob/tool/reasoning payloads;
- memory pressure exists;
- the deployment should not retain completions in process memory;
- hit opportunity cannot be established.
- any eligible-looking request can return side-effecting tool calls that the client may execute again;
- streaming coding traffic and non-streaming batch traffic share the same process;
- an agent changes messages, tool schemas, tool choice, response format, or request defaults on every turn.

#### Required telemetry

- eligible and bypassed request counts by reason;
- hits, misses, stores, and evictions;
- entries and capacity;
- current and p95 serialized/deep-size bytes;
- completion tokens/decode time saved;
- privacy-safe repeat/reuse-distance observation if the cache is disabled.

**Accepted Approach B — local privacy-preserving shadow observation:** when the cache is disabled, Llama Monitor may detect repeat eligibility and reuse distance using a keyed fingerprint held only in the local running process. Use a cryptographic HMAC with a random per-runtime key that is never persisted or exported; never use a plain content hash. Bound the in-memory fingerprint map by entries, bytes, and short TTL; destroy it and its key on process exit/restart. Emit only aggregate counts and reuse-distance histograms. Never expose or retain raw fingerprints, prompts, responses, source code, paths, tool arguments/results, private metadata, or request bodies in the database, logs, metrics export, diagnostics/support bundles, backups, or network traffic. This feature is local-only and makes no outbound telemetry request.

Shadow observation may improve an advisory recommendation, but it must never enable a cache, mutate a preset, or restart a runtime automatically. If the local-only, ephemeral, bounded, aggregate-only contract cannot be proved, disable shadow observation and fall back to an explicit low-cap trial. Do not store request content solely to make a recommendation.

### 6.3 Cache UI placement

Recommended Performance -> Cache section:

1. **Active KV policy** — effective dtype, requested/effective mode, quality downgrade reason.
2. **Reusable prompt state** — Auto/Off/Custom shared concept, with backend-native effective controls and current/cap bytes.
3. **Rapid hybrid/SWA detail** — effective retained-state ceiling, eligibility, estimated/current bytes, and reason, shown progressively rather than as a normal-user question.
4. **Exact-response cache for external APIs** — Advanced, Off by default.
5. **Disk KV checkpoints** — Advanced storage policy, current disk use/cap/cleanup.

Never combine these into one “cache size” control.

### 6.4 Canonical workload examples

#### Interactive OpenCode-style development — default profile

- Streaming: assumed On; verify from observed requests/capability rather than hard-code client brand.
- Tools/MCP: heavy; schemas may form a large stable prefix.
- Transcript: normally append-only within a session; high hybrid-prefix opportunity if serialization is stable.
- Hot sessions: ask for concurrently revisited worktrees/sessions; default the question to 1, not the recommendation.
- Hybrid cache: Guided candidate when architecture is eligible and bytes fit; initial one-session trial is N=4, clearly labeled an app trial.
- Response cache: Off because streaming is ineligible and tool-call replay is undesirable.
- Primary metrics: prefix-extension hits, prefill tokens/ms saved, TTFT, entry p95 bytes, evictions, PFlash bypasses, and tool-schema/prefix churn.
- Llama MTP profile: explicitly `--parallel 1`; tell OpenCode/sub-agents to run model turns sequentially. They queue rather than decode concurrently, preserving the single-stream optimization and avoiding parallel MTP state growth.
- Sequential does not mean “one reusable session.” If the parent and several sub-agents alternate independent transcripts, each transcript that will resume counts toward the reusable prompt-state working set and can evict another. Estimate cache/checkpoint retention from revisited transcripts while active-generation concurrency remains one.
- Rapid MTP profile: keep scheduler admission separate. MTP accelerates only eligible single-live-request greedy steps; overlapping requests fall back to ordinary decode. Sequential orchestration increases MTP eligibility but is not a hard server requirement.

#### Tool/research agent — Hermes/OpenClaw-style interactive, delegated, or scheduled work

- Streaming: unknown per client/job; observe or ask.
- Tools/MCP: heavy, sometimes with long observations and side effects.
- Transcript: append-only inside a job but jobs may start from distinct or dynamically generated system prompts.
- Hot sessions: peak overlapping jobs that later make another model turn, including delegated threads.
- Hybrid cache: valuable for multi-turn jobs with stable prefixes; little benefit for single-turn cron jobs or prompts regenerated from scratch.
- Response cache: Off by default even when non-streaming, because exact repeats are uncommon and cached tool calls may be replayed. Consider only a dedicated read-only deterministic research endpoint.
- Primary metrics: per-workload prefix stability, warm-turn TTFT, evictions between jobs, completion of tool loops, and failure/retry behavior.

#### General chat — secondary profile

- Streaming: normally On, but editable and observable.
- Tools/MCP: normally Off unless the user explicitly enables them; switching them on changes cache and compatibility guidance.
- Formatting: backend chat template is relevant for structured chat; raw/text completion remains an explicitly separate integration mode.
- Concurrency and hot sessions: ask rather than inherit coding-agent or scheduled-job assumptions.
- Hybrid/llama prompt cache: Guided only from measured stable-prefix reuse and memory headroom.
- Response cache: Off by default; ordinary evolving or stochastic conversations are poor candidates.
- Product priority: supported and regression-tested, including Llama Monitor's built-in chat, but it does not anchor cross-product defaults, navigation prominence, or performance recommendations.

#### Deterministic batch/eval API

- Streaming: Off; temperature 0/top_k 1; exact repeats expected.
- Tools: preferably absent. If present, require replay-safe/idempotent qualification.
- Hybrid cache: workload-dependent; often less important for independent one-shot requests.
- Response cache: only profile where an explicit low-cap trial is ordinarily appropriate, still bounded by observed bytes and isolated from general agent traffic.

#### SillyTavern/roleplay and storytelling

- Streaming: commonly desired for interactive reading, but must be observed/configured rather than assumed for every connection profile.
- Formatting: default guidance to SillyTavern instruct/client-formatted text completion, reflecting the primary user workflow. In that path SillyTavern owns all formatting; do not expose or recommend a backend chat template for it. Qualify llama.cpp `/completion` and Rapid `/v1/completions` independently because their request fields and SSE shapes differ. Also support structured chat as a distinct option whose backend template is relevant.
- Transcript: long and often append-only, which can make prompt caching valuable; persona, author's note, example dialogue, world-info/lorebook activation, summaries, and context trimming can insert or change tokens before the tail and reduce exact-prefix reuse.
- Sampling: normally stochastic and rich; swipes/regeneration intentionally seek different outputs. Explicit client samplers win over preset defaults.
- Hot sessions: usually one active character/chat, but ask about multiple tabs/users. This is distinct from simultaneous generations.
- Hybrid/llama prompt cache: Guided candidate after measuring actual serialized-prefix stability. Teach that a large static persona/world prefix increases potential savings, while dynamically injected lore near the front can invalidate it.
- Response cache: Off. Streaming and stochastic generation are ineligible, and exact reuse would defeat swipes/regeneration even if a request happened to qualify.
- Context: reserve room for response generation and injected world/persona material; show when client-side trimming/summarization, server context shift, or model context ceiling determines behavior.
- Primary metrics: cached prefix tokens, prompt re-evaluation after lore activation, TTFT, context truncation/shift, effective sampler/template/request shape, and stop reason.

Roleplay recommendation must cover, without overriding SillyTavern:

- model-family and quant evidence for prose/roleplay quality, repetition behavior, instruct format, and usable context—not generic “chat” or MoE tags;
- planned context split among rendered history/persona/lore/examples, response reserve, and safety margin;
- effective K/V cache dtype and any quality/performance warning at the selected long context;
- streaming and stop-sequence compatibility for the chosen Text or Chat route;
- temperature, top-p, top-k, min-p, repetition/presence/frequency penalties, DRY/XTC or backend-specific samplers only when the route supports them;
- interaction/precedence among SillyTavern sampler presets, app omission-only defaults, and backend defaults, with the effective request visible in diagnostics;
- speculative decoding as a measured model/backend option, not a universal roleplay default;
- one versus multiple simultaneous generations, multiple revisited chats, context trimming/shift, and prompt-cache consequences.

Do not publish universal roleplay sampler numbers. Phase 0/7 must gather official client semantics and representative model-family evidence, then offer two or more named starting styles with the values and trade-offs visible. A client preset remains authoritative.

### 6.5 llama.cpp cache and concurrency design for the same workloads

Current pinned baseline: llama.cpp `b10068`, commit `571d0d540df04f25298d0e159e520d9fc62ed121` (2026-07-18). This section must be revalidated with the exact selected executable.

#### Separate mechanisms

1. **Per-slot common-prefix reuse (`cache_prompt`)** — enabled by default at server and request levels. Reuses the longest common token prefix and evaluates the suffix. Upstream warns prompt/decode batch differences may make results non-bit-identical.
2. **Host-RAM prompt-state cache (`--cache-ram`)** — stores saved states as extra host-backed slots for interleaved conversations. Default cap is 8192 MiB; `-1` is unlimited; `0` disables. It grows on save rather than preallocating the cap.
3. **Idle-slot publication (`--cache-idle-slots`)** — publishes idle slots to the host prompt cache and can clear active unified KV for new work. It is not the same as ordinary per-slot prefix reuse and Off does not mean “flush every prompt on every request.”
4. **Post-divergence chunk reuse (`--cache-reuse N` / request `n_cache_reuse`)** — searches for matching chunks after divergence and shifts KV positions when supported. It is not needed for ordinary append-only common-prefix reuse and is incompatible with some contexts such as multimodal.
5. **SWA/hybrid context checkpoints** — preserve branchable states for architectures that cannot safely rewind arbitrary recurrent/rotating state. Current default is 32 checkpoints per slot with 8192-token minimum spacing.
6. **Slot save/restore** — persistent/manual state workflow with storage/privacy implications. Keep out of primary recommendations until a concrete app-owned workflow and security design exist.

Never label all six “prompt cache.” UI uses the shared taxonomy from D20 and names the backend-native mechanism below it.

#### Recommended baseline

- Keep `cache_prompt` at the qualified upstream default and preserve an explicit client override.
- Keep continuous batching enabled unless a measured regression requires a troubleshooting override.
- Use one guaranteed active generation for the default coding and roleplay profiles; let the user choose foreground-plus-background overlap after seeing queue/memory/throughput trade-offs.
- Keep host prompt caching enabled only with a bounded Auto cap derived from safe retained-memory headroom. On Apple unified memory, count it against the same physical pool as the model and active execution.
- Keep context checkpoints at the qualified upstream default initially; remove the hard-coded duplicate flag unless a non-default policy is justified.
- Leave `cache_reuse` at 0 for ordinary agent/roleplay chat. Offer a clearly labeled `256` trial only for FIM/document rewrite or other large repeated chunks after divergence, with before/after evidence. The value comes from upstream coder/FIM presets, not a universal optimum.
- Keep built-in llama-server tools, agent mode, and Web UI MCP proxy Off for external OpenCode/Hermes/OpenClaw. Their MCP/tool execution is client-side.

#### Context and concurrency teaching

Do not ask only for “Parallel slots.” Ask:

- How many generations must actively run at once?
- How many inactive conversations/jobs should remain cheap to resume?
- What context must each simultaneous request be guaranteed?
- May one request temporarily use unused shared KV capacity?
- Does the client compact/truncate its own history, and at what target?

For partitioned KV, per-sequence capacity derives from total context divided across sequences. For unified KV, sequences share a total pool and may use elastic capacity, but simultaneous long requests contend. The estimator, launch command, and UI must show:

```text
total/shared KV capacity
minimum guaranteed context per active request
single-request burst ceiling
simultaneous generation slots
hot inactive/reusable sessions
retained host-cache budget
```

For the canonical mix, present two starting policies:

- **Maximum foreground context/throughput:** one active slot, bounded host prompt cache for inactive sessions; coding, cron, and other clients queue.
- **Foreground plus background overlap:** two active slots with an explicit guaranteed or elastic context policy; show increased memory, possible foreground slowdown, and queue improvement.

Do not default to upstream Auto/four slots for long-context local agents. If true priority isolation is required and no qualified request-priority mechanism exists, recommend app-owned scheduling or separate runtimes rather than implying continuous batching prioritizes the foreground.

#### Workload-specific guidance

- **OpenCode:** stable repository/system/tool prefixes favor common-prefix and host prompt caching. Multiple worktrees are inactive reusable sessions; concurrent sub-agents are active generations. Keep those counts separate.
- **Hermes/OpenClaw:** overlapping cron/delegated jobs may justify two active slots; completed/single-turn jobs do not justify retained entries. Dynamic timestamps/job headers may reduce prefix reuse.
- **SillyTavern instruct/text:** SillyTavern owns the raw prompt. Long stable persona/history prefixes can reuse well; lore/world-info insertion, context reconstruction, or author-note movement can change the prefix. Sampling/swipes do not disable prompt caching, but they do make exact-response caching inappropriate. Server chat templates are irrelevant on this path.
- **SillyTavern Chat Completions:** backend template and capability become relevant; cache stability is evaluated after actual server tokenization/rendering.

#### Required evidence and telemetry

Use structured per-response cached-token/timing fields where available, plus active/deferred requests, busy slots, context high-water, queue time, TTFT, prompt processing, and generation rates. Do not enable prompt-revealing slot debug in ordinary operation. Current upstream lacks a stable comprehensive Prometheus API for host-cache entries/bytes/hits/evictions; treat log parsing as version-bound troubleshooting, not a durable product contract, and track an upstream structured-metrics request.

## 7. Target Data Contracts

Names below are illustrative. Builders may refine names but not collapse responsibilities.

### 7.1 Model source view contract

```text
RapidMlxModelSourceView {
  kind,
  display_name,
  canonical_identity,
  repo_id?,
  revision?,
  local_path?,
  conversion_recipe?,
  provenance_hash?,
  editable_fields,
  launchability,
  warnings
}
```

Requirements:

- Rust owns parsing, validation, canonicalization, and safe edit semantics.
- Frontend never flattens a typed source into a lossy string and then reconstructs it.
- Every source variant supports display, edit, clone, estimate, Model Library association, and launch-preview behavior.
- Legacy `model_path` migration is covered by fixtures and never reintroduced on save.

### 7.2 Model memory profile

```text
ModelMemoryProfile {
  identity,
  weights: WeightComponents,
  layer_groups: [LayerMemoryGroup],
  experts: ExpertTopology?,
  recurrent_state: RecurrentStateGeometry?,
  vision: VisionComponent?,
  embedded_mtp: EmbeddedMtpComponent?,
  external_companions: [CompanionModel],
  model_context_limit?,
  quantization: WeightQuantizationEvidence,
  tokenizer_template: TemplateEvidence,
  field_evidence: map<Field, Evidence>,
  source_revision,
  warnings
}

Layer groups must support different KV heads/dimensions/windows within one model.

### 7.3 Rapid execution policy

```text
RapidMlxExecutionPolicy {
  planning_context_tokens,
  planning_context_mode: advisory | warn_app_traffic | strict_app_traffic,
  enforcement_scope: estimator_only | llama_monitor_received_requests | native_runtime,
  max_num_seqs,
  max_concurrent_requests,
  prefill_batch_size,
  completion_batch_size,
  requested_kv_mode,
  effective_kv_mode,
  kv_mode_reason,
  turboquant_mode,
  prefix_cache_policy,
  hybrid_cache_policy,
  response_cache_policy,
  disk_checkpoint_policy,
  gpu_memory_utilization,
  pflash_policy,
  speculative_policy,
  parser_policy,
  request_defaults,
  optional_models,
  security_policy
}
```

The server command must be built from this typed policy, never from UI strings or arbitrary flag tuples.

### 7.4 Normalized estimate result

```text
MemoryBreakdown {
  active: {
    weights,
    active_kv,
    recurrent_state,
    working_buffers,
    companions,
    runtime_overhead,
    total,
    safe_headroom
  },
  retained: {
    prefix_cache,
    hybrid_swa_entries,
    response_cache_host_ram,
    disk_checkpoints,
    configured_cap,
    expected_total_range
  },
  requested_policy,
  effective_policy,
  fit,
  evidence,
  evidence_reasons,
  calibration_envelope,
  warnings
}
```

All surfaces consume the same result shape. No surface independently reconstructs a total or renames Rapid modes to llama quant names.

### 7.5 Capability snapshot

```text
RapidMlxCapabilitySnapshot {
  executable_identity,
  rapid_mlx_version,
  help_hash,
  serve_flags,
  endpoint_capabilities,
  package_versions,
  installed_extras,
  qualified_features,
  provisional_features,
  incompatible_features,
  evidence_timestamp,
  source
}
```

Qualification is more than flag presence. For example, `mlx-vlm 0.6.5` may satisfy dependency resolution but remains provisional until real Qwen/Gemma smoke matrices pass.

## 8. Decision and Assumption Register

The coordinator must present these to the user before the phase that depends on them. Record answers in this document before implementation. Do not let a Builder decide them implicitly.

| ID | Decision | Recommended position | Blocking phase |
|---|---|---|---|
| A1 | Rapid planning context: advisory or enforced? | Accepted: advisory estimator/fit target for direct external clients; client owns compaction/reserve and runtime owns its actual ceiling. Built-in chat or a separately qualified future proxy may offer Advisory/Warn/opt-in Strict, but never claim enforcement for bypass traffic | Phase 5/7/13 |
| A2 | Preset request defaults apply to external clients or app chat only? | Accepted: a selected sampling mode supplies omission-only server/request defaults for app chat and direct external clients where the backend supports them; explicit client values always win, including zero/false/empty reset values; never claim coverage for an unmapped Rapid field | Phase 2/3/7 |
| A3 | Cache headline includes retained cap? | Accepted: primary fit uses active workload plus safe headroom; retained current/working-set/cap is a separate growth scenario; unified memory also shows combined pressure at cap without claiming eager allocation | Phase 5/6/8/10/13 |
| A4 | App owns `gpu-memory-utilization`? | Accepted two-stage Approach A: expose Inference memory limit as Auto/Advanced Custom; initially Auto follows Rapid's upstream 0.90 and reports the resolved byte cap, then calibrated Auto may derive a bounded value from the canonical live-memory snapshot and estimator. It is a Metal working-set/admission/cache-pressure ceiling, not a reservation or KV-only percentage; never claim effect on engines that ignore it | Phase 5/7 |
| A5 | TurboQuant exposure | Accepted Approach A: Reusable prompt storage exposes Auto, Standard, Advanced-trial K8V4, and Expert legacy/A-B V-only. Auto preserves exact upstream alias/runtime resolution; unknown/community finetunes remain Standard until immutable-revision qualification. TurboQuant is retained-prefix compression, not active KV/weights/recurrent/MTP/prefill savings; recommendations require workload correctness, latency, transient-peak, and realized-memory evidence | Phase 5/7/11 |
| A6 | Hybrid cache policy budget | Accepted: shared Reusable prompt state uses Auto/Off/Advanced Custom; Rapid Auto selects the smallest memory-safe hybrid/SWA working set inside a dedicated prefix-cache policy slice/byte budget, and N remains a secondary ceiling | Phase 6 |
| A7 | Response cache in primary wizard? | Accepted: no; Off by default and excluded from normal-user recommendations/primary Wizard. Expert Advanced control only for explicitly selected Deterministic batch/eval API workloads | Phase 6/7 |
| A8 | Response-cache trial default | Accepted: none. Expert Advanced control offers Off and Custom only; do not suggest 8 or any other count until telemetry plus a bounded memory policy justify a new product decision | Phase 6 |
| A9 | Disabled-mode repeat telemetry | Accepted (default inverted per E9): ship the **explicit opt-in cache trial** as the default path; the ephemeral per-runtime-key HMAC fingerprint + reuse-distance shadow observer is **DEFERRED/optional**, not the initial mechanism. Rationale: the cache is Off by default (D14/D17), so a privacy-sensitive fingerprinting subsystem the local executor must build exactly-right is not justified before an explicit trial proves demand. If the shadow observer is later built, it keeps the original contract: memory-only, short-TTL, hard-bounded, aggregate output only; never plain hashes, content, persistence, logs/exports/backups, or network telemetry | Phase 6/11 |
| A10 | Custom template path | Resolved (E1 supersedes the prior "investigate native Rapid support first / pause for approval" gate): do NOT build native template-override into Rapid. Ship ONE revision-pinned template-selection layer (identity/rollback/provenance — mostly existing llama.cpp plumbing) with two thin appliers — llama.cpp via `--chat-template`/`--chat-template-file`, Rapid-MLX via **file placement** into an llama-monitor-owned copy/overlay (never the canonical/HF-cache dir). The driving reason is the §3.9 tool-call-reliability defect, not truthfulness alone; keep the §12.7 comparison/rollback/smoke-test machinery, now driven by that defect. One retained [escalate→device] checkpoint: verify on M5 Max that the first real template swap loads and kills the tool-call loop | Phase 9 |
| A11 | Remote-code trust | Accepted: inspected data-only pinned MLX repositories launch normally; custom-code declarations require evidence-bearing per-revision consent; incomplete inspection is provisional; browse/search/estimate never execute code; seek native force-off as defense in depth | Phase 1/12 |
| A12 | Cache export/import | Defer until a concrete warm-start/migration workflow is approved | Phase 11/12 |
| A13 | Reranker/adapters | Watchlist/separate scope, not part of parity closure | Phase 12 |
| A14 | Pin MLX-VLM or follow floating range | Accepted: trust Rapid's published constraints for ordinary managed/external installs; pin the selected Rapid release, record the exact resolved receipt, automatically probe it, retain rollback, and add an app override only for a demonstrated incompatibility. A merely new compatible tuple is not globally Provisional | Phase 3/12 |
| A15 | Unknown finetune enrichment source | Real config/tokenizer + runtime probes; never alias-equivalent by assumption | Phase 3/8 |
| A16 | Preset IA redesign timing | Accepted two-stage rollout: repair source/control/save/restore parity first; then adopt the seven-category shell only after real-control dark/light/narrow screenshot review and explicit user approval | Phase 10 |

### 8.1 Additional decisions discovered during roadmap design

| ID | Decision | Recommended position | Blocking phase |
|---|---|---|---|
| A17 | Unknown/provisional model launch policy | Allow with a prominent warning when the selected runtime can attempt it; block only known-incompatible or security-disallowed cases | Phase 1/3 |
| A18 | Managed optional dependencies | Accepted automation constraint: install the explicitly product-supported upstream extras, detect other extras automatically, and provide actionable enablement only when a selected feature needs one. External environments that satisfy upstream constraints and pass automated baseline checks are normal, not arbitrarily disclaimed; warn only on a concrete missing/broken/indeterminate selected capability. Reframe (E6): the "automated baseline check" is an **on-device probe** the user runs, not a Nick-owned CI gate. Rapid-MLX + dependencies update near-daily; drift is validated on the user's device, user-driven, independent of llama-monitor's release cycle (modeled on and upgrading the existing thin llama.cpp beta-update validation). Any upstream-monitoring CI is additive/optional and must NOT gate Phase 3 | Phase 3/12 |
| A19 | Rapid on Linux/Windows | Preserve preset browsing/editing and evidence, but show Rapid as unavailable with an actionable macOS/Apple-silicon explanation; never hide or corrupt data | Phase 1/13 |
| A20 | Legacy `model_path` lifetime | Accepted: read and migrate indefinitely; stop writing it as soon as Phase 2 lands; typed source wins conflicts and disagreement is diagnosed; preserve free-form input through Alias; keep `served_model_name` separate; document downgrade incompatibility rather than maintaining two identities | Phase 2 |
| A21 | Recommendation authority | Recommendations are advisory until the user accepts them; safety-enforced effective downgrades are automatic and explicitly explained | Phase 5/6 |
| A22 | Estimator calibration bar | Accepted: preserve/version the existing RTX 5090 CUDA-WDDM and M5 Max Metal llama.cpp calibrations; target ±10% within each explicitly qualified Rapid/llama hardware-runtime-model-configuration envelope; label unmeasured complete math Calculated and missing/inferred paths Provisional | Phase 5 |
| A23 | Cache telemetry retention | Accepted with A9 (now deferred per E9): applies only if/when the shadow observer is built. Until then there is no keyed-fingerprint retention to govern. When built: retain only bounded aggregate counters/histograms; raw keyed fingerprints and their random per-runtime key remain memory-only for a short bounded observation window and are destroyed on exit; no prompt/response content, raw fingerprint, stable identifier, database/log/export/backup retention, or outbound telemetry | Phase 6/11 |
| A24 | Disk KV ownership/default | Keep upstream behavior visible but do not add app-managed cleanup or recommend enablement until ownership, path, and recovery semantics are proved | Phase 11/12 |
| A25 | Companion-model ownership | Each drafter, vision tower, and embedding model is an explicit source component with separate download, provenance, lifecycle, and additive memory | Phase 4/8 |
| A26 | `[guided]` absent | Show structured generation as unavailable/provisional and give installation guidance; never silently weaken strict schema requests | Phase 3/7 |
| A27 | Template fallback if upstream declines | Resolved (E1): the question is moot — the plan no longer depends on Rapid gaining native override, so there is no "upstream declines" fork to pause on. Rapid applies templates by file placement into an llama-monitor-owned copy/overlay today; llama.cpp uses its existing flags. Still forbidden: a Llama Monitor Jinja renderer, request-rewriting proxy/shim, mutation of the canonical/HF-cache model dir, or an unreleased dependency pin. The Phase-0 grep of the vllm_mlx parser only decides whether the Rapid applier can become flag-symmetric; it is not a gate on doing the work | Phase 9 |
| A28 | Shared IA effect on llama.cpp | Accepted target: the same stable seven-category order for llama.cpp and Rapid-MLX, with backend-adaptive contents; hide empty subsections rather than reorder navigation; reorganize only after parity repair and screenshot-backed explicit user approval | Phase 10 |
| A29 | HF qualification cache | Key by repo plus immutable revision and runtime/dependency snapshot; short-lived lookup aliases may refresh, but pinned evidence does not mutate | Phase 3/8 |
| A30 | No-op controls during remediation | Hide controls that have never worked; show read-only eligibility only when it teaches something useful and is clearly labeled not configurable yet | Phase 1 |
| A31 | Automatic cache recommendations | Require user confirmation; telemetry may update the explanation and suggested value but must not mutate a preset or restart a runtime automatically | Phase 6/11 |
| A32 | Canonical workload | 80% OpenCode-style coding and 20% Hermes/OpenClaw-style tool/research agents defines product priority, not a blended preset, traffic split, or resource-allocation formula; roleplay is separately first-class and built-in chat is secondary | All phases |
| A33 | Workload-profile UX | Accepted five-profile taxonomy: Interactive coding agent (default), Tool/research agent, Roleplay/storytelling, General chat (secondary), and Deterministic batch/eval API (Advanced); assumptions are transparent and editable, never an opaque optimization preset | Phase 6/7/10 |
| A34 | Response cache with tool calls | Accepted: never recommend for OpenCode/Hermes/OpenClaw/MCP or other tool-enabled/shared agent runtimes; allow nonzero Custom only on an isolated deterministic non-streaming endpoint with absent or explicitly replay-safe/idempotent tools | Phase 6/11/12 |
| A35 | Agent-session hybrid trial tiers | Accepted simplified Auto: approximately two states for one revisited coordinator and up to four for a revisited sequential helper when safe; do not permanently provision for brief one-shot/5–10 minute cron overlap; `2 × regularly hot reusable sessions` remains the advanced minimum and all headroom is constrained by bytes and evidence | Phase 6 |
| A36 | Mixed client runtime | Optimize safety/correctness for the union of clients; do not enable a process-wide cache because one minority caller qualifies | Phase 6/7 |
| A37 | Tool-schema stability guidance | Diagnose prefix churn and teach deterministic serialization/order where client control exists; never mutate agent semantics automatically | Phase 6/11/13 |
| A38 | Roleplay as first-class use case | Add Roleplay/storytelling to wizard/preset recommendations and SillyTavern to external-client qualification | All phases |
| A39 | SillyTavern formatting mode | Accepted: explicit structured-chat vs client-formatted-text ownership; Text Completion is the Roleplay default; no universal/double-template configuration | Phase 7/9/13 |
| A40 | Roleplay sampler ownership | Accepted: client values win; app presets provide omission-only, model-aware sampling modes and effective-request diagnostics | Phase 2/7/11 |
| A41 | Roleplay cache policy | Prompt/hybrid cache guided by observed prefix stability; exact-response cache Off; no guarantee when lore/context injection changes early tokens | Phase 6/11 |
| A42 | llama context guarantee | Accepted: explicit shared-pool/partitioned policy with target context, active requests, admission ceiling, minimum guaranteed and burst per-request context, plus separate hot retained sessions/client compaction owner; normal single-user Auto uses one active request; never reuse legacy `context × slots` assumptions | Phase 0/5 |
| A43 | llama MTP and overlap | Accepted: MTP is an explicit `--parallel 1` single-stream mode; sub-agents queue and should run sequentially; conflicting values are rejected before save/launch rather than silently rewritten; overlap mode disables MTP unless experimental multi-slot is separately qualified | Phase 5/7 |
| A44 | llama Web UI and MCP proxy | Accepted: proxy Off by default; add capability-gated Advanced Web UI availability/config/static-path controls; keep network/access shared; treat built-in tools as separately gated Experimental with explicit allowlist/threat model; never expose `--agent` as a magic all-tools/proxy switch | Phase 1/7/12/13 |
| A45 | Workload discovery default | Coding agent first/default; Tool/research and Roleplay separate; scheduling/delegation/overlap are editable Tool/research assumptions; General chat secondary; Deterministic batch/eval Advanced | Phase 2/8/10 |
| A46 | Recommended quant meaning | Must fit selected workload/context/concurrency policy; otherwise label quality-only or not recommended | Phase 5/8 |
| A47 | Context compaction owner | External client owns compaction/history; app chat compaction is never presented as protection for OpenCode/Hermes/SillyTavern | Phase 5/7/13 |
| A48 | Rapid MTP concurrency | Accepted: memory-first single-active Auto for maximum quant/context fit, resolving the effective admission ceiling to one where required; separately estimated Advanced Allow overlap starts at two and must refit quant/context/guarantees; never derive fit without additive MTP companion/cache ownership or assume audited mid-stream fallback is lossless | Phase 3/5/7/11 |
| A49 | Backend-settings ownership | Accepted: Rust owns the semantic setting catalog and runtime truth; reusable frontend components own teaching, hierarchy, responsive layout, and custom interaction; never reduce this to a fully generated form or duplicate setting semantics in JavaScript | Phase 7/10/13 |
| A50 | Spawn Wizard flow | Accepted intent-first six-step shell: Goal & Guidance; Model & Engine; Fit & Context; Behavior & Endpoint; Review Plan; Start & Verify. Rename setup depth to Guidance level (Guided/Customize/Expert), default workload to Interactive coding, and allow source deep links to prefill without bypassing workload confirmation | Phase 7/10 |
| A51 | Sampling mode visibility and provenance | Accepted: every model on both backends shows the complete applicable mode selector everywhere; preserve all family modes; provide universal Model/author, General, Coding/Agentic, Precise/Deterministic, Creative/Roleplay, and Custom choices; persistent best-use/source badges replace tooltip-only guidance | Phase 2/7/8/10/13 |
| A52 | Sampling source and finetune authority | Accepted: exact pinned Unsloth-published values are authoritative; otherwise use model-author config/card, qualified runtime alias, then labeled app guidance. Resolve finetunes by metadata/lineage before filename and permit a persisted manual family override with provenance | Phase 0/2/3/8 |
| A53 | Estimator architecture | Accepted: GGUF and MLX parsers populate one evidence-bearing normalized model-geometry profile; separate llama.cpp and Rapid-MLX execution policies/calculators produce one result vocabulary. Shared geometry must never force shared runtime math or llama vocabulary into MLX | Phase 4/5/8/13 |
| A54 | Estimator confidence UX | Accepted: Qualified, Calculated, and Provisional tiers; uncertainty plus safety headroom controls Fit/Tight boundaries; absent hardware remains Calculated rather than blocking implementation; raw reproducible calibration evidence is retained | Phase 5/8/10/13 |
| A55 | HF/Model Library parity preservation | Accepted: guided qualification and hierarchical cards must preserve sorting, discover categories, curated/user-editable author quick picks, Community Picks, provider/imatrix/UD classification, origin/alternate-quant discovery, preselection, context/KV/concurrency-driven model-quant comparison, explicit pre-download switching, and llama-only mmproj handling | Phase 8/10/13 |
| A56 | Community finetunes across backends | Accepted: curated popular creators plus heretic/uncensored and updated-dataset finetune/distillation discovery are first-class for both engines; preserve original author versus quantizer/converter roles; support native MLX or qualified safetensors-to-MLX conversion; persisted preference affects ranking but not technical qualification | Phase 0/2/3/8/10/13 |
| A57 | Default community/alignment discovery posture | Accepted: broad community-first product default with persisted Include/Prefer/Only controls for curated creators and heretic/uncensored variants plus Include/Prefer updated finetunes/distillations; personal preference may persist without becoming the global ranking default | Phase 8/10/13 |
| A58 | Unified-memory readiness and recovery | Accepted Approach A: replace conflicting frontend budgets with one backend-owned live snapshot and capacity/safe-now/recovery/configured-ceiling/unsafe scenarios; distinguish allocator cache, reusable state, app-owned runtimes/high-memory apps, and OS disk cache; make actions conditional on changing fit and remeasure; preserve privacy. Preserve the user-verified reboot-persistent 57,344 MiB M5 Max sysctl path while adding safe bounds, exact readback, reset/rollback, restart handling, and cross-version evidence | Phase 0/5/7/8/11/13 |

### 8.2 Measurements or user facts required before numeric recommendations

The workload mix is decided; these operational details are not. Phase 0 should capture them from real traffic where possible and ask the user only when observation cannot answer safely:

1. Peak simultaneous model generations from OpenCode, including sub-agents, versus merely concurrent tool calls.
2. Whether Hermes/OpenClaw cron/delegated jobs may overlap foreground coding and whether queueing is preferable to reducing foreground throughput/context.
3. Minimum guaranteed context per active coding/research request and acceptable single-request burst ceiling.
4. Actual API modes used by installed OpenCode/Hermes/OpenClaw versions (Chat Completions, Responses, Anthropic Messages, or another adapter) and their cancellation/retry behavior.
5. Whether tool/MCP definitions and system/environment fields remain byte-stable between turns in the user's installed plugins/configuration.
6. Whether interactive/background priority should be app-owned admission on one runtime or separate foreground/background runtimes. Present both approaches before expanding architecture.
7. Whether a given preset is the default MTP single-stream/sequential mode or a multi-agent overlap mode with MTP disabled; current upstream multi-slot MTP remains Experimental even where technically supported.
8. Typical number of concurrently revisited SillyTavern chats, preferred text-connection type for Rapid (`Generic` versus `VLLM`), target context/response reserve, and whether text streaming is enabled.
9. Whether omission-only `stream_options.include_usage=true` injection is compatible with every canonical client; terminal usage chunks can affect stream parsers.
10. Acceptable bounded host-cache budget/calibration envelope on the user's unified-memory machines. Unlimited is not an Auto candidate regardless of answer.

Until measured, UI returns scenario-based choices and ranges—not one universal slot, cache, context, speculation, or sampler value.

### 8.3 Consolidated M5 Max measurement envelope `[escalate→device]`

Most A-register items are already Accepted with a concrete approach (decisions were front-loaded here, not left for the local executor to settle mid-phase). The genuine residual is a small set of items whose *recommended numeric value* cannot be derived and must be **measured on the M5 Max** (see the gate taxonomy in §9.6 — this is bucket 3, `[escalate→device]`; it does not spend frontier quota). Naming the envelope lets the local executor return "range now, number after measurement" instead of stalling on a value it cannot compute.

Measurement-blocked items: A4 (stage-2 calibrated Auto memory limit), A5 (TurboQuant realized-memory), A6 (hybrid cache budget), A22 (calibration ±10% envelope), A35 (agent-session hybrid tiers), A41 (roleplay cache prefix stability), A42 (guaranteed/burst context numbers), A48 (MTP admission ceiling), A54 (calibration confidence tiers), A58 (live memory-availability numbers). These are fed by the ten §8.2 operational facts and overlap the KV floor (§13.1, E3) and the wire-capture oracles (§12.11, E8).

Discipline: the plan already degrades each of these to honest scenario ranges rather than a fabricated value. This section only consolidates them so their device-measurement dependency is explicit and routed to Nick + the local model on the M5 Max, not to a frontier model.

## 9. Mandatory Pipelined Implementation Protocol

This protocol applies to every phase. It restates the repository's Builder/Verifier model so a context-free agent cannot accidentally turn this roadmap into a single-agent implementation pass.

### 9.1 Coordinator responsibilities

The main agent is the Coordinator. Before each phase it must:

1. read this entire plan, the repository `AGENTS.md`, the phase section, and any directly referenced security/platform documents;
2. record current branch, `HEAD`, dirty-worktree state, prior completed phase, upstream evidence date, and relevant resolved decision IDs;
3. verify that unrelated user changes are preserved and define the exact allowed file/scope boundary;
4. create a phase brief containing the exact section below, objective, prerequisites, non-goals, hard gates, required sources, and a context ceiling comfortably below 200k;
5. spawn a **Builder** sub-agent to implement and test only that phase;
6. inspect the Builder handoff and actual diff, then spawn a **fresh Verifier** sub-agent that has the specification and diff but is instructed not to accept Builder claims as evidence;
7. decide whether the phase passes, requires a narrowly scoped remediation Builder, or must stop for the user/upstream;
8. repeat Builder/Verifier loops until all hard gates pass or a stop condition is reached;
9. alone manage phase commits, branch checkpoints, pushes, PRs, and updates to this plan's completion ledger.

Never ask the Builder to verify its own work as final sign-off. Never ask the Verifier to opportunistically redesign the feature. No sub-agent may commit, push, open a PR, or make an unresolved product decision unless the user explicitly changes this protocol.

### 9.2 Context and checkpoint policy

- One phase is one bounded unit and must fit within 200k context, including implementation and targeted tests. The suggested budgets below are ceilings, not targets.
- If a phase approaches its ceiling, the active agent stops at a compilable, documented checkpoint and returns a structured handoff. The Coordinator spawns a fresh continuation agent.
- Never combine phases merely because an earlier phase completed quickly.
- A phase may be split further by the Coordinator, but its final Verifier must check the complete phase contract.
- External facts are revalidated at the start of the phase that uses them. Record drift in the evidence ledger rather than quietly adapting behavior.

### 9.3 Builder brief and return contract

Every Builder brief must include:

```text
Phase and exact plan section:
Frozen HEAD and dirty-state notes:
Resolved decision IDs:
Allowed files and expected migrations:
External/local evidence to inspect:
Objective and user-visible outcome:
Ordered implementation requirements:
Required tests and captures:
Hard gates:
Non-goals:
Stop/escalation conditions:
Context ceiling:
```

Every Builder returns:

```text
Status: COMPLETE | PARTIAL | BLOCKED
Requirements implemented (mapped to plan bullets):
Files changed and why:
Schema/API/migration behavior:
Tests added or changed:
Commands run and exact outcomes:
Sequential screenshot artifacts and observations:
Security/platform review performed:
Assumptions made:
Known risks or incomplete items:
Current git status and diff summary:
Recommended Verifier focus:
```

### 9.4 Verifier brief and return contract

The Verifier independently reads the phase, source evidence, and changed code; maps each requirement to evidence; reruns proportionate tests; inspects negative/error paths; and reviews security, cross-platform degradation, API compatibility, migrations, UI, accessibility, and documentation. For UI work it must inspect real sequential screenshots generated after a release build.

Every Verifier returns:

```text
Verdict: PASS | PASS WITH EXPLICIT CONDITIONS | FAIL
Requirement-to-evidence table:
Commands rerun and exact outcomes:
Defects/regressions with severity and file locations:
Missing or weak tests:
Security/platform/API findings:
Screenshot/accessibility findings:
Unverified assumptions:
Required remediation scope:
```

`PASS WITH EXPLICIT CONDITIONS` is allowed only for a recorded external limitation that does not violate a hard gate. A test failure, missing required surface, false UX claim, or incomplete migration is `FAIL`.

### 9.5 Remediation and user gates

- The Coordinator converts each Verifier failure into a minimal remediation brief and uses a new or narrowly re-briefed Builder.
- A fresh verification pass is required after remediation; do not rely only on the formerly failing test.
- Stop for the user when a blocking decision in Section 8 is unanswered, two viable approaches have materially different behavior, a requested behavior expands security/storage authority, or screenshots expose a consequential IA choice.
- Present the best two approaches, recommend one, explain why, and identify the affected later phases.
- Screenshot scenarios run sequentially because the harness shares a port. For any UI phase, run `cargo build --release` first and use captures as the source of truth.

### 9.6 Gate taxonomy: four buckets, one tag per hard gate

This project's implementation is executed 90%+ by a finetuned local model (a Qwen3.6-27B with a stable, months-proven 200k context), with roughly 10% escalation to a frontier model (Opus/Sol). The Builder→Verifier loop above is a **local** dev-iteration loop, not Claude sub-agent fan-out. Because of that, "escalate to the Coordinator/user" (§9.5) is not one thing — it splits by *who or what can actually decide the gate*. Every hard gate in this plan — the §9.4 Verifier verdict contract and every per-phase hard-gate paragraph — carries exactly one of these four tags:

1. **`[local-verifiable]`** — the local model self-runs it. MUST carry an exact CHECK command and a machine-decidable `PASS iff` condition. If no CHECK can be written, it is not this bucket. This is the default and the largest bucket.
2. **`[decide-once]`** — Nick settles it once during refinement (a copy string, a threshold, an A-item position); after that it *becomes* `[local-verifiable]`. It is not a per-run stop. Educational copy (E7) and fit thresholds live here.
3. **`[escalate→device]`** — Nick + the local model on the M5 Max: measurements, wire captures (§12.11), Phase 5 calibration, the KV floor (§13.1). Real hardware, but it does **not** spend frontier quota. The §8.3 envelope is this bucket.
4. **`[escalate→frontier]`** — genuine reasoning judgment the local model cannot do. This is the ONLY bucket that spends the ~10% Opus/Sol budget, so it is deliberately kept small and pre-counted.

Gate line format:

```text
GATE [tag]: <assertion>
  CHECK | JUDGMENT: <exact command, or the specific judgment to be made>
  PASS iff | HAND UP: <machine-decidable condition, or who it hands up to and what they decide>
```

Rules:

- Buckets 2 and 3 absorb most of what naive "escalation" would have sent to a frontier model; the bucket-4 list is the minimized, pre-counted frontier budget.
- A `[decide-once]` gate that has been decided is rewritten in place as `[local-verifiable]` with its resolved value inlined, so a later run does not re-open it.
- The execution companion (`20260718-final_rapidmlx_followups_execution.md`) carries the concrete per-phase bucket assignments in its checkpoint tables; this section defines the taxonomy those tables apply.

## 10. Phase Index and Dependency Map

| Phase | Deliverable | Depends on | Blocking decisions | Suggested context ceiling | State |
|---:|---|---|---|---:|---|
| 0 | Frozen evidence, fixtures, decisions, traceability | None | A1–A58 as applicable to Phases 1–3 | 80k | Not started |
| 1 | Urgent launch correctness and interim safety | 0 | A11, A17, A19, A30, A44 | 100k | Not started |
| 2 | Typed source, sampling catalog, and request-default contracts | 1 | A2, A20, A32, A38, A40, A45, A51–A52 | 160k | Not started |
| 3 | Runtime/dependency capability qualification | 1–2 | A2, A14, A15, A17–A19, A26, A29, A51–A52 | 140k | Not started |
| 4 | Normalized MLX architecture metadata | 0, 2 | A25, A53 | 170k | Not started |
| 5 | Backend execution policies and first-class estimator (formal sub-phases 5a → 5b, §E5) | 3–4 (5b also needs 5a Verified) | A1, A3–A5, A21–A22, A42–A43, A46–A48, A53–A54, A58 | two sub-phases, each <=120k, each own gate + fresh Verifier | Not started |
| 6 | Cross-backend cache policies, recommendations, and teaching | 5 | A6–A9, A21, A23, A31–A37, A41 | 170k | Not started |
| 7 | Critical settings and shared wizard/editor component | 2–3, 5–6 | A2, A5, A7–A8, A26, A30, A33, A38–A40, A43, A45, A47–A49, A51 | two Builder packets, each <=120k | Not started |
| 8 | HF discovery and Model Library convergence | 2–5 | A15, A17, A25, A29, A45–A46, A51–A57 | two Builder packets, each <=120k | Not started |
| 9 | Conversation formatting, client routes, and revision-pinned chat-template substitution (tool-call reliability) | 2–3 | A10, A27 (both resolved by E1), A38–A40 | 120k | Not started |
| 10 | Screenshot-driven wizard/preset/library/HF IA | 7–9 | A16, A28, A32–A33, A50–A51 | 170k | Not started |
| 11 | Diagnostics, metrics, cache/storage observability | 3, 5–7 | A9, A12, A23–A24, A31 | 170k | Not started |
| 12 | Security, dependency, and watchlist closure | 3, 8–11 | A11–A14, A18, A24, A27 | 120k | Not started |
| 13 | Cross-surface convergence, migrations, documentation | 5–12 | All resolved | 130k | Not started |
| 14 | Full validation and release gate | 1–13 | All resolved | 120k | Not started |

The dependency table above is authoritative. Do not infer prerequisites from a simplified diagram; every listed dependency phase must be independently `Verified complete` before the dependent phase starts.

## 11. Executable Implementation Phases

### Phase 0 — Evidence freeze and user decision gates

**Objective and outcome:** Establish an immutable, reproducible baseline so later Builders do not implement against mutable `main` files, stale CLI assumptions, or implicit product decisions.

**Budget:** 80k. **Prerequisites:** clean understanding of current worktree; no product implementation. **Files:** this plan, the execution companion, evidence manifests, `tests/fixtures/`, and a new fixture provenance/readme if needed. Phase 0 must not change runtime/dependency behavior or rewrite `Cargo.lock`; if evidence proves such a change is needed, return a scoped Phase 1/3 decision packet to the Coordinator. **External evidence:** all Section 13 sources, current Rapid release/tag, `rapid-mlx serve --help`, package metadata, and exact HF revisions.

**Builder brief:**

1. Record branch/HEAD/worktree and inventory currently installed or managed Rapid/MLX packages without mutating the environment.
2. Re-fetch the audited commit/release/PR evidence and record any drift since 2026-07-18.
3. Resolve mutable HF `main` URLs to commit revisions, download the six real config fixtures, record SHA-256 and fetch date, and preserve exact source URLs. Minimize fixtures only with explicit removal notes.
4. Capture current and minimum-supported CLI help fixtures plus package/extras metadata; include exact executable identity and help hash.
5. Add a P0/P1 requirement-to-phase-to-test traceability table.
6. Return unresolved decisions needed by Phases 1–3 to the Coordinator as two-approach decision packets with a recommendation. Only the Coordinator presents them to the user and records answers in Section 8.
7. Define macOS functional support and Linux/Windows graceful-unavailability expectations.
8. Pin current llama.cpp server/cache/OpenAI/tool/template evidence and official OpenCode, Hermes/OpenClaw, and SillyTavern integration documentation. Record what is documented versus inferred/observed from real requests.
9. `[local-verifiable]` Template applier fact-pin (E1): grep the `vllm_mlx` argument parser to determine whether Rapid accepts **any** template-path/template-file argument. Record the exact answer. If yes, the Rapid template applier can become flag-based and symmetric with llama.cpp; if no (expected at the pinned commit), the Rapid applier is file-placement only. Also record the safety rule for later phases: the Rapid applier must never mutate the canonical / HF-cache model directory — it operates on an llama-monitor-owned copy or overlay so swaps are reversible and a re-download cannot clobber them. CHECK: grep result recorded with file:line evidence. This is a fact to pin, not a gate to stop on.
10. `[local-verifiable]` KV-dtype fact-pin (E3): grep the `vllm_mlx` parser for the factual KV-cache flags, their allowed choices, and their defaults (e.g. `--kv-cache-dtype {bf16,int8,int4}`, `--reasoning`, TurboQuant tiers). Pin exactly what exists. Do **not** pin a recommended tool-calling KV value — that value is unknown on MLX and belongs to the §8.3 `[escalate→device]` measurement envelope (TurboQuant vs RotorQuant vs plain 4-bit), not to Phase 0. Separately record the llama.cpp heuristic to encode later: tool-enabled runtimes below `q8_0` KV are unreliable (§3.6).

**Verifier brief:** Independently reproduce hashes/revisions, check that fixtures assert facts rather than mirror structs, confirm sources are immutable, verify every Phase 1–3 decision is recorded, and confirm the template-arg and KV-dtype greps pin only what the parser actually contains (no recommended MLX KV value asserted).

**Hard gates:** no mutable evidence is treated as pinned; exact CLI/dependency/extras state is captured; fixtures cover the four named modern families plus flat Qwen3 and standard Qwen3 MoE; the `vllm_mlx` template-arg and KV-dtype greps are recorded with file:line evidence and no guessed MLX KV recommendation is pinned; traceability has no orphan P0/P1 finding; no application behavior changes.

**Stop/escalate:** upstream HEAD invalidates a major recommendation; an HF revision cannot be resolved; current and audited Rapid behavior conflict; a blocking user choice remains unanswered.

**Non-goals:** fixing code, installing/upgrading runtime packages, broad UI capture, estimator formulas.

**Handoff emphasis:** evidence table, checksums, drift report, answered/remaining decision IDs, exact fixture paths.

### Phase 1 — Urgent correctness and interim security behavior

**Objective and outcome:** Eliminate invalid tool-parser arguments and prevent currently visible no-op controls or unsafe implicit behavior from misleading users.

**Budget:** 100k. **Prerequisites:** Phase 0 and A11/A17/A19/A30. **Files:** `src/inference/rapid_mlx/{mod.rs,command.rs,compatibility.rs,info_query.rs}`, `src/inference/launch.rs`, `src/web/api/rapid_mlx_runtime.rs`, `src/presets/mod.rs`, `static/js/features/{spawn-wizard.js,presets.js}`, relevant HTML, `tests/rapid_mlx_test.rs`, UI specs.

**Builder brief:**

1. Replace the parser boolean with a typed optional parser value and rename auto choice to the exact upstream concept.
2. Emit `--tool-call-parser <parser>` and `--enable-auto-tool-choice` only when capability evidence permits; reject invalid combinations before spawn.
3. Change diagnostics so regex/unknown inference cannot create a one-click mutation without a concrete parser and evidence.
4. Hide the unwired speculative, MLLM, and embedding controls, or render read-only eligibility per A30; preserve state safely if an existing preset contains values.
5. Implement accepted A11 without claiming upstream enforcement that does not exist: inspect the pinned source for `auto_map`, referenced Python, and custom configuration/tokenizer/processor hooks; allow a proven data-only MLX repository without a consent interruption; require evidence-bearing consent scoped to the immutable revision when custom repository code is required; show incomplete inspection as provisional. Never execute repository code to perform this inspection.
6. Ensure Rapid-unavailable platforms can view/edit presets and receive an actionable unavailable state.
7. Stop automatically assigning llama.cpp `--cache-ram -1` during auto-size/platform initialization. For new/default unified-memory single-user profiles, shared Auto resolves the extra host prompt-state cache to `0` while preserving ordinary common-prefix reuse. Preserve explicit existing values; show a migration warning without silently rewriting them. Fix the launcher sentinel check so `0` alone is disabled and `-1` remains enabled/unlimited for users who explicitly retain it.
8. Stop unconditionally emitting llama.cpp `--webui-mcp-proxy`; implement the safe Phase-1 subset of A44 and test loopback/non-loopback security behavior. External client MCP support must not depend on this flag. Preserve the audited upstream-enabled bundled UI baseline without enabling its proxy/tools; defer the complete Advanced Web UI group to Phase 7 after capability/schema/security design.
9. Correct immediate llama cache/context copy: 8 GiB is a demand-filled cap, not a pre-reservation; `-1` is unlimited; `0` disables only the extra host prompt-state cache rather than ordinary common-prefix reuse; idle-slot cache is not ordinary per-request prefix reuse; `--ctx-size` is not universally per-slot.
10. Remove the hard-coded `--ctx-checkpoints 32` when it merely duplicates the qualified upstream default, or capability-gate/document the reason for an explicit value.
11. Add exact argv, advisor, serialization-default, unavailable-runtime, cache recommendation, and negative-capability tests.

**Verifier brief:** Search all command paths for the invalid flag spellings, inspect advisor mutation paths, test an old/missing-flag help fixture, check no secrets/paths leak into diagnostics, and verify hidden controls cannot enter a launch payload.

**Hard gates:** no bare `--tool-call-parser`; no emitted `--auto-tool-choice`; normal presets do not emit `--webui-mcp-proxy`; explicit unsupported options fail before process spawn; no visible control claims an effect it lacks; `#[serde(default)]` preserves old presets; data-only MLX repositories are not presented as executing remote code merely because Rapid grants permission; detected custom-code consent is source-and-revision specific and visible before execution; no repository code runs during inspection; no new/auto-sized unified-memory preset receives unbounded llama host cache automatically; normal single-user Auto resolves extra host prompt-state caching to `0`; sentinel tests prove `0` disabled, positive bounded, and `-1` enabled/unlimited; cache/context copy matches pinned upstream semantics; otherwise llama.cpp launch behavior is unchanged.

**Stop/escalate:** upstream parser values cannot be reliably enumerated; security posture would require executing/downloading repository code; hiding a control would destroy saved state.

**Non-goals:** full capability snapshot, wiring speculative/MLLM/embedding, estimator redesign, IA redesign.

**Handoff emphasis:** old/new schema mapping, exact argv table, diagnostics evidence, platform behavior, search proving invalid forms are gone.

### Phase 2 — Typed model source, sampling catalog, and backend-neutral request defaults

**Objective and outcome:** Make typed Rapid sources round-trip everywhere, replace fragmented sampling heuristics with one provenance-bearing cross-backend mode catalog, and give presets one explicit request-default contract.

**Budget:** 160k. **Prerequisites:** Phase 1; A2/A20/A51–A52 resolved. **Files:** `src/llama/model_defaults.rs`, `src/inference/rapid_mlx/{mod.rs,model_resolver.rs}`, `src/inference/launch.rs`, `src/presets/mod.rs`, `src/web/api/{benchmark.rs,presets.rs,spawn_wizard.rs,models.rs,vram.rs}`, `src/models/library.rs`, proxy/chat request path under `src/web/api/`, `static/js/features/{presets.js,spawn-wizard.js,setup-view.js,models.js,vram-estimate.js}`, fixtures, UI tests/captures.

**Builder brief:**

1. Introduce the Rust-owned `RapidMlxModelSourceView`/codec and canonical identity semantics from Section 7.1.
2. Cover MLX directory, HF repo/revision, alias, authoritative safetensors local/HF, GGUF conversion source, and unknown future variants.
3. Migrate legacy `model_path` once at read/save boundaries; typed source wins; stop writing a second identity.
4. Use the view contract for create, display, edit, clone, save, launch preview, estimate, welcome, and Model Library association.
5. Replace `get_model_presets` family branches as public authority and the wizard's separate `SAMPLING_DEFAULTS` with one Rust-owned `SamplingModeCatalog`; parsing adapters may still populate it.
6. Represent stable mode ID, complete sampler/reasoning bundle, workload badges, exact provenance URL/revision/retrieval date, source class, lineage evidence, backend field coverage, recommendation rank, and Custom/Model-default omission semantics.
7. Preserve exact pinned Unsloth modes and expose every applicable mode. Add the universal choices required by A51 for every model without mislabeling app guidance as model-author or Unsloth guidance.
8. Resolve renamed/opaque finetunes from GGUF/HF/MLX metadata and base-model lineage before filename. Persist and round-trip a manual family override plus provenance for unresolved cases.
9. Implement A2 as omission-only server/request defaults with explicit zero/false/empty preservation and no double application. Map llama defaults to launch; leave Rapid per-field coverage for Phase 3 probing rather than claiming parity prematurely.
10. Prove precedence with OpenCode/Hermes tool requests and SillyTavern roleplay requests: explicit streaming, sampler, stop, tool, and response-format fields win; formatted-text and structured-chat paths are not conflated.
11. Implement field provenance/dirty-state precedence: explicit user/request > explicitly selected model mode > workload-aware model recommendation > family-safe default > omission.
12. Make Interactive coding the new-user default workload, with Tool/research, Roleplay/storytelling, General chat, and Advanced Deterministic batch/eval separate. Migrate stored workload values without rewriting user-chosen settings.
13. Add migration, canonicalization/path, revision, lineage, catalog completeness, source-integrity, lossy-round-trip, and end-to-end request tests.
14. Update `rapid-preset`/`preset-editor` captures for every mode-selector state, including renamed finetunes and both backends.

**Verifier brief:** Run the full source-variant matrix; inspect every frontend `model_path` read and competing sampling table; test typed and legacy fixtures through UI and launch; verify every applicable mode appears for canonical and renamed finetunes on both backends; compare Unsloth values byte-for-value with pinned fixtures; verify explicit client values win; test path traversal/symlink/revision/lineage cases and unknown variants.

**Hard gates:** typed fixture never says “No model configured”; all source variants work across every required surface; production frontend does not treat legacy string as authoritative; no save creates divergent identities; one sampling catalog is authoritative; every model has the universal selector and every recognized family exposes all curated modes; renamed finetunes preserve family modes; Unsloth values are exact and cited; request defaults apply exactly once; explicit falsy values win; no llama preset regression.

**Stop/escalate:** a source variant lacks safe edit semantics; downgrade compatibility requires continued dual writes; the proxy owner cannot be identified without a broader architecture change.

**Non-goals:** HF search redesign, new model conversions, estimator math, broad visual reordering.

**Handoff emphasis:** source matrix results, sampling catalog and lineage fixtures, Unsloth source comparison, migration examples, request-default precedence/coverage table, security tests, before/after captures.

### Phase 3 — Runtime, dependency, and feature qualification

**Objective and outcome:** Replace version-based optimism with automated capability discovery that trusts Rapid's upstream dependency contract for the ordinary baseline, detects concrete mismatches, and distinguishes per-capability availability without creating a manual qualification treadmill or global warnings for healthy new releases.

**Budget:** 140k. **Prerequisites:** Phases 1–2; A14/A15/A17–A19/A26/A29. **Files:** `src/inference/{capabilities.rs,rapid_mlx/{compatibility.rs,discovery.rs,info_query.rs,runtime.rs,updater.rs}}`, `src/web/api/rapid_mlx_runtime.rs`, managed installer/update paths, fixtures and runtime tests.

**Builder brief:**

1. Implement Section 7.5 keyed by executable identity, Rapid version, exact help hash, resolved dependency versions, extras, evidence timestamp, and source. Generate it automatically and invalidate only when its fingerprint changes.
2. Probe exact-token flags and installed package metadata with bounded time/output; invalidate on executable/help/dependency changes.
3. Represent MLX, MLX-LM, MLX-VLM, embeddings, and guided/outlines separately.
4. Keep arbitrary finetunes distinct from verified aliases; source each parser, architecture, PFlash/TurboQuant, DFlash/DDTree, vision, embedding, and guided claim independently.
5. Treat environments satisfying Rapid's published constraints and automated baseline import/self/protocol checks as normally upstream-supported. Do not require manual maintainer certification and do not apply a global Provisional banner solely because an exact compatible tuple is new. Reserve per-feature indeterminate state for consequential optional capabilities automation cannot safely establish.
6. `[escalate→device]` Make the **on-device** update-validation probe first-class (E6): the real cadence is near-daily rapid-mlx + dependency updates, validated on the **user's device**, user-driven, independent of llama-monitor's release cycle — modeled on and upgrading the existing thin llama.cpp beta-update validation. The Phase 3 hard gate depends only on these on-device probes. Any Nick-owned automated upstream-monitoring CI (clean environment from upstream metadata, representative text/tool/guided/template/cache/model smoke cases, machine-readable capability/regression manifest) is **additive/optional** and must NOT gate Phase 3, because Phase 3 blocks Phases 5/7/8/11/12. If the optional CI runs, a failed concrete gate may create a known-incompatible override and maintainer alert and a passing run needs no manual approval — but its absence never blocks a phase.
7. Implement the selected missing-extra installation/diagnostic policy and A26 strict-generation behavior. The managed easy button uses Rapid's upstream dependency contract plus explicitly product-supported extras, records the resolved receipt, and retains a last-known-good rollback; it does not maintain an independent hand-curated lock by default.
8. Treat waybarrios only as a comparison/watchlist source.
9. Add a parallel bounded llama.cpp capability snapshot for the shared workload contract: cache prompt/RAM/idle/reuse, context checkpoints, unified/partitioned KV, auto/explicit slots, continuous batching, Chat Completions, Responses, raw/text completion routes, streaming usage/progress, template caps, tools/parallel tools, and speculative modes.
10. Bind all optional behavior to the exact selected llama executable/build/help hash. Do not infer current `b10068` semantics for older user binaries.
11. Qualify MTP concurrency separately: older/model/backend combinations that require `parallel=1`; current llama builds that technically support per-sequence MTP; and Rapid's single-live-greedy fast-path with fallback. Capability does not automatically equal a product recommendation.
12. Revalidate Rapid's native request > CLI > alias > `generation_config.json` > fallback sampling cascade and probe each `--default-*` field independently. Seek/upstream missing CLI fields needed for direct-client parity; record exact partial coverage and prevent unmapped selected defaults from being reported effective.

**Verifier brief:** Test old/current/future help fixtures, hash invalidation, missing/broken extras, arbitrary finetunes, stale snapshots, bounded probes, upstream-constrained but previously unseen dependency tuples, CI manifest ingestion, known-bad overrides, rollback receipts, and no version-only optional-feature inference. Prove a healthy unseen tuple receives no global warning while a concrete failure produces an actionable per-feature diagnosis.

**Hard gates:** no manual per-release certification requirement; no arbitrary global Provisional/disclaimer state for an upstream-compatible environment passing automated baseline checks; warnings identify concrete evidence and remediation; flag presence alone cannot justify consequential semantics; help matching is exact; snapshots invalidate correctly; upstream constraints and resolved receipts are preserved; the on-device update-validation probe is automatic, timed, and bounded and is the only qualification the Phase 3 gate depends on; any upstream-monitoring CI is additive/optional and its absence never blocks this or a dependent phase; no search/estimate executes repository code.

**Stop/escalate:** qualification would require downloading/executing untrusted repo code; package versions cannot be safely enumerated; installer authority differs from A18.

**Non-goals:** UI for every flag, waybarrios backend support, estimator implementation, automatic dependency mutation outside approved policy.

**Handoff emphasis:** capability state machine, probe commands/limits, qualification matrix, stale behavior, dependency findings.

### Phase 4 — Normalized MLX architecture metadata

**Objective and outcome:** Parse real MLX model geometry with field-level evidence so Qwen3.6, Gemma4, dense, and MoE memory can be calculated correctly.

**Budget:** 170k. **Prerequisites:** Phase 0 fixtures and Phase 2 identity; A25. **Files:** `src/inference/rapid_mlx/mlx_meta.rs`, new backend-neutral memory-profile module, `src/llama/vram_estimator/`, `src/hf/mod.rs`, `src/models/library.rs`, `src/web/api/vram.rs`, `tests/fixtures/mlx_configs/`.

**Builder brief:**

1. Implement `ModelMemoryProfile` and normalized full/local/linear/recurrent layer groups with field evidence.
2. Parse nested `text_config` and flat configs without allowing irrelevant wrapper fields to override text geometry.
3. Model Qwen3.6 full-attention interval and DeltaNet/recurrent state; model Gemma4 global/local heads, dimensions, windows, and pattern.
4. Represent dense/MoE expert count, active experts, shared experts, and active-vs-total weights separately.
5. Track embedded MTP tensor ownership and external drafter/vision/embedding companions without double counting.
6. Propagate context ceiling and fix byte-to-bit size conversion.
7. Make HF lookup revision-aware, recursive, paginated, and bounded; prevent `hf_file_path` from becoming an MLX config name.
8. Parse local MLX config and safetensors index; use name heuristics only as a degraded fallback.

**Verifier brief:** Assert independent expected facts from all six pinned fixtures; review formulas and ownership; inject missing/malformed fields; test HF pagination/revision and local paths; compare index totals.

**Hard gates:** Qwen3.6 does not treat all layers as full KV; Gemma4 includes global KV and caps local KV; recurrent state is explicit; `* 8` conversion is tested; every component has provenance; fixtures include 8k/32k/131k/high-bound contexts where supported; no llama GGUF regression.

**Stop/escalate:** source configs contradict architecture documentation; tensor ownership cannot be established; a heuristic would be labeled exact; external fetch bounds would exclude required metadata.

**Non-goals:** effective Rapid cache policy, UI totals, calibration, arbitrary future-family tuning.

**Handoff emphasis:** normalized profiles, fixture provenance, expected facts, math ownership notes, degraded-state behavior.

### Phase 5 — Backend execution policies and first-class estimator

**Objective and outcome:** Produce one honest estimate for all surfaces using backend-native runtime semantics, separate active from retained memory, and revalidate llama.cpp slots/unified-KV/host-cache math for external-agent concurrency.

**Formal sub-phase split (E5).** Phase 5 is executed as two formal sub-phases, each with its own hard gate and its own **fresh Verifier pass** — not merely "two Builder packets." The reason is coherence-per-packet for the local executor (this is not a token-fit problem; the 200k context holds the whole phase), and it compounds with the §9.6 gate taxonomy (fewer interdependent gates per Verifier pass) and the front-loaded A-decisions (each sub-phase enters with its decisions already resolved):

- **Phase 5a — execution policy + estimator core (Builder brief items 1–14):** backend execution policies, `MemoryBreakdown`, effective-KV/TurboQuant modeling, planning-context contract, active-vs-retained separation, calibration, llama slot/unified-KV/host-cache revalidation, workload scenarios, quant-comparison rebasing, and MTP modeling — ending in cross-surface estimate equality. 5a must pass its own gate before 5b starts.
- **Phase 5b — memory-availability, reclaim, and acquisition repairs (Builder brief items 15–18):** the backend-owned `MemoryAvailabilitySnapshot`, outcome-aware reclaim guidance, hardened wired-limit handling, and the concrete acquisition/propagation-gap repairs.

**Budget:** 190k total (each sub-phase Builder packet ≤120k). **Prerequisites:** Phases 3–4; A1/A3–A5/A21–A22/A58; 5b additionally requires 5a `Verified complete`. **Files:** new execution-policy/calculator and memory-availability modules, `src/inference/rapid_mlx/{mod.rs,command.rs}`, `src/inference/launch.rs`, `src/system.rs`, `src/gpu/apple.rs`, `src/web/api/{vram.rs,system_tools.rs}`, `src/llama/vram_estimator/`, `static/js/features/{vram-estimate.js,spawn-wizard.js,presets.js,setup-view.js,models.js,dashboard-render.js}`. **Read D31 in full.**

**Builder brief (items 1–14 = Phase 5a; items 15–18 = Phase 5b):**

1. Implement `RapidMlxExecutionPolicy` and `MemoryBreakdown` without translating Rapid modes to llama `ctk`/`ctv` strings.
2. Model requested/effective bf16/int8/int4, reasoning's int8 override, model-safe bf16 downgrade, and machine-readable reasons.
3. Model accepted D31 precisely: requested/effective Auto/Standard/K8V4/V-only, exact-alias resolution, conventional-KV-only retained savings, recurrent/uncompressed portions, bypass/ineligible paths, decompression/transient peak, concurrency, buffers, companions, runtime overhead, and headroom. Never apply TurboQuant savings to active KV, weights, MTP, prefill, or cold prompts.
4. Implement the accepted advisory planning-context contract. Distinguish estimator target, model/runtime ceiling, and client-owned compaction/reserve. Warn/reject only for requests Llama Monitor actually receives and only under an explicit policy; capability-probe any native runtime ceiling before claiming enforcement.
5. Separate active/cold fit from retained cache footprint and configured caps.
6. Return evidence/calibration envelope and use the same API result on wizard, preset, welcome, Model Library, and HF preview.
7. Calibrate representative cases and document measurement commands/hardware/error bounds.
8. Preserve llama.cpp response semantics or version a shared contract explicitly with regression tests.
9. Revalidate current llama.cpp `--ctx-size`, automatic/explicit parallel slots, unified KV, continuous batching, idle-slot cache, context checkpoints, and host prompt-cache memory. Model active slots and retained host cache separately; do not assume a legacy per-slot formula without measured/source proof.
10. Add workload scenarios for foreground-only coding, coding plus one background job, multiple scheduled jobs, single roleplay chat, and multiple roleplay sessions. Explain throughput, queueing, per-request context, and memory consequences.
11. Replace the quant-comparison 8k/generic use-case baseline with the selected workload, architecture, target context, guaranteed/elastic concurrency, backend, and unified-memory state. A Recommended badge requires policy fit.
12. Make llama.cpp MTP an explicit `--parallel 1` single-stream policy before save/launch, with queueing and sequential-sub-agent guidance. Benchmark it against a multi-slot/MTP-Off overlap policy; permit current-upstream multi-slot MTP only as a build/model/hardware-qualified experiment. Include all MTP recurrent/draft memory.
13. For Rapid, inventory per-family embedded versus external MTP weights from actual loader behavior, not config labels/comments; include sidecar download/disk/provenance plus resident weights and MTP cache/state additively even while inactive. Implement the selected D25 admission policy. For Advanced overlap, model every allowed active request's KV/recurrent/working/speculative state, reduced context guarantee, and audited single-live greedy/no-logits MTP gate; do not claim lossless mid-stream fallback until exact-runtime stress tests pass. Return eligibility/fallback/handoff reasons, count MTP-active steps, and record peak memory.
14. Distinguish external-client compaction ownership from Llama Monitor chat compaction and show observed prompt/context pressure without promising app-side protection.
_— Phase 5a gate falls here; 5b begins. A fresh Verifier must pass Phase 5a (items 1–14, ending in cross-surface estimate equality) before the following items start. —_

15. Implement the accepted D30/A58 backend-owned `MemoryAvailabilitySnapshot`. Feed the identical timestamped snapshot and effective backend ceiling to every fit surface. Return safe-now, conditional-after-reclaim, after-closing-apps diagnostic, configured-ceiling, and unsafe states without calling total RAM or a wired cap available memory. Carry explicit additional-versus-replace launch intent; add back only a measured app-owned runtime that the launch action will actually stop. Show stable capacity and current readiness separately in Model Browser/HF.
16. Make reclaim guidance outcome-aware and action-specific: distinguish backend allocator-cache clear, reusable-state eviction, app-owned runtime stop/high-memory-app guidance, and OS disk-cache purge. Use a conservative bounded recovery estimate, offer an action only when it can cross the selected fit boundary, remeasure afterward, and report actual change. Group/redact high-memory-process evidence, do not return full command arguments by default, use macOS `phys_footprint` with labeled RSS/backend metrics where available, and do not add arbitrary process termination.
17. Harden wired-limit handling with RAM-relative safe bounds, explicit consequence/confirmation, exact readback, restore/default, failure provenance, and process restart. Preserve the user's verified reboot-persistent 57,344 MiB M5 Max path; record mechanism/macOS evidence and test supported versions rather than replacing it preemptively. If a different boot mechanism is ever required, it needs opt-in ownership and rollback. Keep raw wired limit, Metal recommended working set, backend effective utilization, and current safe availability as separate values.
18. Repair the concrete acquisition/propagation gaps: Rapid Wizard must fetch a fresh snapshot rather than reuse llama/HF caches; Model Browser must stop passing total unified memory as available or forcing llama-only preview assumptions; partial `/api/system/info` must not overwrite a full pressure snapshot; Preset Editor must refresh rather than cache memory state indefinitely; Rapid prefix/cache reservations and `max_cache_blocks` must reach prelaunch estimates.

**Verifier brief:** Recalculate representative cases independently; cross product KV/reasoning/TurboQuant/concurrency/context; compare every UI surface; prove unknown finetunes do not inherit alias eligibility; test PFlash/paged/hybrid-zero/recurrent bypasses and transient peaks; inspect false precision and model ceilings; run llama regression tests.

**Hard gates — Phase 5a (items 1–14):** no Rapid estimate accepts/displays llama KV vocabulary; requested/effective policies are distinct; active and retained totals are distinct; TurboQuant affects only qualified retained-prefix components and its transient decompression peak remains visible; Standard is not mislabeled FP16; unknown finetunes do not inherit exact-alias eligibility; the same estimator API result feeds wizard/preset/welcome/Model Library/HF preview and they agree (cross-surface equality is the 5a exit gate); Qwen3.6/Gemma4 errors meet or truthfully display the calibrated envelope; external MTP/vision/embedding companions and Rapid cache reservations are additive and never hidden/double-counted; eligibility/dispatch/weight ownership disagreements stop MTP qualification; concurrency fit covers the worst admitted active-request state rather than the non-reserving ceiling alone; Rapid context is not presented as server allocation; llama slot/context math matches the pinned current runtime; unbounded host cache is never included as a finite fit promise.

**Hard gates — Phase 5b (items 15–18):** all surfaces consume the same `MemoryAvailabilitySnapshot` and agree; Rapid never inherits stale/zero llama memory caches; no total unified memory or theoretical wired cap is called available; current pressure can downgrade a previously successful configuration with actionable recovery scenarios; reclaim is conservative and remeasured; OS disk purge is not presented as clearing live model/Metal memory; process command secrets are absent and process metrics are labeled honestly; sysctl mutation is bounded, exactly verified, reversible, restart-aware, and persistence-qualified; the acquisition/propagation-gap repairs (Rapid fresh snapshot, Model Browser availability, partial-info overwrite, Preset Editor refresh, `max_cache_blocks` reaching prelaunch estimates) all land.

**Stop/escalate:** calibration exceeds A22; runtime safety overrides cannot be determined; shared API changes would break existing clients without a migration; evidence cannot bound a component.

**Non-goals:** cache recommendation UI, all Advanced launch controls, IA reorder.

**Handoff emphasis:** formula table, calibration data, effective-policy cases, cross-surface equality proof, llama regression evidence.

### Phase 6 — Cross-backend cache policy, recommendations, and educational UX

**Objective and outcome:** Expose the two new Rapid cache controls only where useful and bring llama.cpp prompt-cache recommendations to the same evidence/teaching standard for coding, scheduled-agent, and roleplay workloads.

**Budget:** 170k. **Prerequisites:** Phase 5; A6–A9/A21/A23/A31. **Files:** Rapid config/command/capability/poller, memory API, metrics modules, shared settings descriptors, wizard/preset UI, `static/js/features/rapid-mlx-cards.js`, cache fixtures/tests/captures.

**Builder brief:**

1. Add typed, capability-gated `hybrid_cache_entries` and `response_cache_entries`; response cache defaults zero, while shared Reusable prompt state defaults Auto and resolves Rapid to zero or a bounded effective N. Omit flags at effective zero.
2. Implement Section 6.1 Auto eligibility, conservative single-user baseline (approximately two states for one revisited coordinator, up to four for a revisited sequential helper), advanced `2 × regularly hot reusable conversations`, memory cap/headroom, PFlash overlap, exact-repeat/SWA cautions, and effective Off when entry bytes cannot be bounded.
3. Keep entry count and byte budget separate and show the effective calculation, confidence, memory consequence, reasons Auto resolved Off/lower, and the choice to tolerate reprocessing for brief background jobs.
4. Place response cache in Advanced/API workloads; explicitly state streaming app chat receives no benefit; do not invent a default.
5. Per accepted A8, expose only Off and Custom for the expert response-cache control; provide no trial value or numeric recommendation. Require explicit confirmation for nonzero Custom and explain the lack of a byte cap.
6. Add the accepted A9/A23 local shadow-observation contract: per-runtime random HMAC key, memory-only bounded/TTL fingerprint map, aggregate-only counters/reuse-distance histograms, no plain hashes or raw fingerprint exposure, no persistence/log/export/backup/network path, destruction on exit, and explicit-trial fallback if the contract cannot be proved. Add metrics necessary to observe eligibility/hits/bytes without content retention; defer unavailable upstream fields honestly.
7. Add Auto/Off/Custom round-trip, effective-value, argv, workload, memory-pressure, and copy tests.
8. Audit and capability-gate llama.cpp server/request prompt caching, host `--cache-ram`, idle-slot preservation, `--cache-reuse`, context checkpoints/minimum spacing, slot save/restore, and continuous/unified slot interaction. Expose only controls that meet the plan's priority rule.
9. Replace unlimited llama host-cache guidance with accepted D18: unified-memory Auto resolves extra host prompt-state caching to `0` for the normal single-user profile while preserving ordinary common-prefix reuse; only evidence-backed regularly interleaved sessions plus proven surplus may produce a confirmed bounded positive cap. Discrete-GPU Auto may use a bounded system-RAM allowance. Treat `-1` as Advanced explicit Unlimited, never Auto, and handle its enabled sentinel correctly.
10. Teach the difference among simultaneous generations, hot reusable sessions, cached idle prompts, and exact response reuse. Provide separate coding-agent, scheduled-agent, and roleplay examples.
11. Qualify SillyTavern prompt stability with structured-chat and client-formatted-text request fixtures, including persona/world-info insertion, context trimming, swipes, and sampling.

**Verifier brief:** Check every claim against pinned sources; test Rapid hybrid N=1/4/6/8, interleaving, prefix extension, exact repeat, PFlash, and pressure; test response streaming/greedy/sampled/multimodal/tool/reload; test llama cache budgets/slots/checkpoints/reuse against the selected build; test roleplay stable and lore-mutated prefixes; inspect dark/light/narrow captures.

**Hard gates:** response cache defaults Off; Reusable prompt state defaults Auto but Rapid hybrid retention resolves Off when ineligible/unbounded and never overprovisions for rare transient overlap; N never derives from `max-num-seqs`; no numeric hybrid effective value without a memory bound; response cache never claims benefit for streaming/stochastic/tool-heavy profiles or a byte cap; llama unified-memory Auto defaults the extra host cache to `0` without disabling ordinary common-prefix reuse, and any positive recommendation is bounded/modeled/confirmed; `-1` is never Auto and is not misclassified as disabled; all guidance explains what/when/why/not per workload; no automatic preset mutation/restart; shadow observation is local-only, keyed, ephemeral, bounded, aggregate-only, and absent from every persistence/log/export/backup/network surface; no content in telemetry.

**Stop/escalate:** upstream metrics cannot support a promised recommendation; privacy design needs hashes/content; calculated entries exceed safe headroom; UI would present an unevidenced universal value.

**Non-goals:** disk cleanup, automatic tuning/restart, cache export/import, general dashboard redesign.

**Handoff emphasis:** recommendation examples, refusal cases, exact copy, memory math, argv/round-trip tests, capture paths.

### Phase 7 — Critical launch settings and shared backend-settings UI

**Objective and outcome:** Make every retained critical Rapid control real and keep wizard and preset editor on one descriptor/schema path.

**Budget:** 190k. **Prerequisites:** Phases 2–3 and 5–6; A5/A7–A8/A26/A30. **Files:** Rapid config/command/compatibility, presets/spawn APIs, a shared `static/js/features/` settings module, `spawn-wizard.js`, `presets.js`, HTML/CSS, tests and module baseline. **Read D31 in full.**

**Builder brief:**

1. Build one schema/descriptor-driven component covering capability, evidence, default, help, validation, serialization, command/request mapping, summary, and unsupported reason.
2. Wire qualified speculative decoding, MLLM, embeddings, KV policy, reasoning, PFlash, batching/concurrency, request safety, cache policies, and troubleshooting overrides end to end.
3. Enforce mutual exclusions and source/extra/model eligibility before launch.
4. Provide exact command preview with secrets redacted and effective-vs-requested policy explanations.
5. Restore/edit/clone existing presets including unsupported/provisional saved values without silent loss.
6. Preserve current tab/step shell until Phase 10; add new JS module baseline when applicable.
7. Add transparent workload inputs for all five accepted profiles; display derived streaming/tool/format/sampling/concurrency/cache assumptions and require confirmation.
8. For Roleplay, cover long-context reserve, client-owned samplers/stops, chat-vs-text formatting owner, and prompt-cache stability without encoding SillyTavern-specific guesses as server facts.
9. Replace raw Parallel Slots teaching with active-generation, guaranteed-context, burst-context, and inactive-session inputs. Selecting llama.cpp MTP visibly locks the resulting policy to one slot and explains sequential queueing before the user confirms; switching to overlap mode visibly disables MTP. Rapid's memory-first Auto also uses one active generation to protect near-capacity quant/context fit, while Advanced Allow overlap refits two active working sets and shows MTP eligibility/fallback/handoff evidence; do not claim MTP itself is the only reason for the admission policy.
10. Show endpoint compatibility per workload: Chat Completions, Responses, Anthropic Messages if qualified, and raw/text completion routes. Do not imply one OpenAI-compatible label proves every client protocol.
11. Implement the capability-gated Advanced llama-server Web UI group from accepted D26/A44: Auto/On/Off availability, open action, validated config JSON/file, Expert custom static path, and effective Network & Access explanation. Keep the MCP proxy Off and built-in tools/`--agent` absent unless the separately approved Experimental security gate is satisfied.
12. Render the complete shared sampling mode selector in Wizard and Preset Editor with persistent best-use/source badges, full values, Custom and Model/author-default choices, backend coverage, and visible explicit-client precedence. Never hide non-default family modes or collapse a finetune to one option.
13. Render accepted Reusable prompt storage choices with Standard as the safe fallback, exact requested/effective/upstream-default provenance, workload-specific teaching, and Advanced/Expert placement. Do not expose raw TurboQuant bits/group size, imply cold/active-memory savings, or recommend it for Roleplay/one-shot research without new qualification.

**Verifier brief:** For each visible control trace capability → UI → schema → validation → launch/request → restore → summary → test; switch backends repeatedly; test unsupported runtimes and mutual exclusions; inspect JS/HTML/CSS references and XSS/accessibility.

**Hard gates:** no visible no-op control; wizard/editor labels/defaults/help match; hidden backend fields never cross payloads; unsupported explicit settings fail clearly; saved values are not lost; command preview equals tested argv; no arbitrary flags; workload profiles expose assumptions; explicit third-party request fields win; llama settings remain correct.

**Stop/escalate:** a feature is only provisionally qualified; shared descriptor cannot represent backend-specific behavior without lying; implementing a control expands installer/security authority.

**Non-goals:** broad IA reorder, every Rapid flag, free-form CLI, full dashboard metrics.

**Handoff emphasis:** control traceability matrix, shared descriptor contract, switch/round-trip results, captures, baseline update.

### Phase 8 — Hugging Face discovery and Model Library convergence

**Objective and outcome:** Make GGUF and MLX discovery/library flows truthful about format, identity, qualification, fit, provenance, and best next action.

**Budget:** 190k. **Prerequisites:** Phases 2–5; A15/A17/A25/A29/A45–A46/A51–A57. **Files:** `src/hf/mod.rs`, `src/web/api/{hf.rs,models.rs,vram.rs}`, `src/models/library.rs`, `src/inference/rapid_mlx/info_query.rs`, `static/js/features/{hf-browse.js,models.js,spawn-wizard.js,vram-estimate.js}`, Community Picks/creator schemas and fixtures, tests/captures.

**Builder brief:**

1. Add Auto/GGUF/MLX/All discovery scopes and workload/device-derived copy/ranking, but treat search results only as candidates. Preserve Downloads/Likes/Newest/Trending, discover categories, author quick picks/customization, Community Picks, pagination, and explicit user search controls.
2. Run authoritative post-selection qualification keyed by repo, immutable revision, runtime snapshot, config/tokenizer/index/extras evidence.
3. Preserve public tokenless search; request credentials only for private/gated access; bound pagination/body/time/concurrency.
4. Keep unknown finetunes provisional and never inherit alias-only capabilities. Resolve original author, base model, dataset/merge/distillation lineage, artifact publisher, quantizer, and converter as separate evidence-bearing roles.
5. Resolve recursive total size and local MLX config/index; preserve repo/revision through selection, estimate, download, library, and launch.
6. Use canonical typed source for preset association and distinguish revisions/conversions.
7. Implement the accepted two-level card data hierarchy and evidence-backed cross-format grouping without name-only merges; defer final visual polish to Phase 10.
8. Start discovery from the selected workload, with Coding agent as default. Keep model tags/author claims, architecture fit, template/tool qualification, roleplay/prose evidence, endpoint support, and observed benchmarks as separate evidence dimensions.
9. Pass workload, backend, target/guaranteed context, concurrency/pool policy, architecture, and memory topology into quant comparison. Never award Recommended from a generic 8k quality score.
10. Replace the GGUF-only `KnownQuantizer` public concept with the backward-compatible, user-editable community-source role catalog. Preserve existing bundled creators and customization/migration; add heretic/uncensored and updated-dataset finetune/distillation preferences and categories without treating them as compatibility proof.
11. Make native MLX artifacts and authoritative safetensors conversion candidates discoverable from original community finetunes. Preserve original author and immutable source revision through conversion; show MLX converter/publisher separately.
12. Preserve the complete pre-download quant workflow. Enumerate real GGUF files, native MLX variants across qualified lineage, or supported conversion recipes; recompute all choices after workload/context/KV/concurrency/companion changes; show quality-fit/context trade-offs; require explicit user selection before download/conversion.
13. Keep llama.cpp `mmproj` discovery/recommendation intact and backend-gated. Rapid must hide `mmproj` and show only actual qualified integrated/MLX-VLM components.
14. Repair current hidden gaps: both HF UIs must pass explicit format; MLX cannot use the GGUF-only files endpoint/renderer; Models-modal estimates cannot hard-code llama/16k/q8; Rapid quant advice cannot use llama math; context/KV/concurrency changes must invalidate the comparison; and visible variant selection must equal authoritative saved state.

**Verifier brief:** Exercise public/private/gated, every sort/scope/category/curated-author path, user catalog migration, Community Picks missing/malformed/present, heretic/uncensored and updated finetunes, paginated results, pinned revision, unknown/renamed finetune, local download, alternate quant/origin resolution, native MLX, conversion, two revisions, context/KV/concurrency requantization, explicit pre-download switching, llama mmproj, mutable-revision invalidation, and failure recovery; inspect auth/logging and compare GGUF regression.

**Hard gates:** search is never qualification; exact revision survives; model groups do not merge distinct sources; existing sorts/categories/curated creators/customization/Community Picks and quant-switch workflows do not regress; original author never becomes converter; community finetunes are first-class on Rapid through native MLX or qualified conversion; no hard-coded llama backend in MLX preview; selected identity/revision/variant and visible selection never drift; context/KV/concurrency changes recompute available model quants without silently switching; Model Library answers identity/fit/readiness/action first; failures preserve selection; tokens/private metadata do not leak; local MLX receives first-class estimates; llama mmproj remains correct and never leaks into Rapid.

**Stop/escalate:** HF filtering is unreliable enough to exclude valid results; a qualification step requires remote-code execution; credential policy expands; card hierarchy needs consequential redesign.

**Non-goals:** downloading every candidate, multi-model registry, reranking, adapter support.

**Handoff emphasis:** identity/creator/converter flow diagram, old-to-new feature parity matrix, network bounds, qualification evidence states, quant/context feedback matrix, library association/grouping matrix, captures.

### Phase 9 — Conversation formatting and chat-template substitution

**Objective and outcome:** Fix the §3.9 tool-call-reliability defect by substituting revision-pinned chat templates on **both** backends, and tell the truth about how each backend applies them. This is not "investigate whether Rapid can get native override first" — E1 resolved the architecture: ONE revision-pinned selection layer (identity/rollback/provenance) with two thin appliers. The driving reason is functional tool-call correctness (stock Qwen3.6/Gemma4 templates loop/fail on tool calls), not truthfulness alone.

**Budget:** 120k. **Prerequisites:** Phases 2–3; A10/A27 (both resolved by E1); Phase 0 template-arg grep (item 9). **Files:** Rapid compatibility/command/info/model resolver, `src/web/api/templates.rs`, chat-template registry/updater/UI, wizard/preset/chat transport, related tests.

**Builder brief:**

1. Apply a substituted template on Rapid by **file placement** into an llama-monitor-owned copy/overlay of the model directory — never the canonical/HF-cache dir (Phase 0 item 9 safety rule). If Phase 0's grep found that Rapid accepts a template-path argument, the Rapid applier may instead pass that flag (symmetric with llama); otherwise file-placement is the mechanism. Surface the applier honestly: on Rapid, substitution is file-placement provenance, not a runtime flag, and `chat_template_kwargs` (kept separate, only probed supported kwargs) does not replace the template.
2. Apply a substituted template on llama.cpp via its existing `--chat-template` / `--chat-template-file` flags. Both appliers consume the same revision-pinned selection layer.
3. Prevent one backend's template state from leaking into the other; preserve each for switching back.
4. **Tool-call smoke-test matrix gates activation (E1):** a candidate template becomes active only after it passes a tool-call smoke test for its model family/workload — single/parallel tool calls, null/empty arguments, tool errors/retries, reasoning preservation, role integrity, streaming shape. A failed smoke test leaves the active selection unchanged. This is the machinery §12.7 describes, now driven by the §3.9 defect. On Rapid specifically, one retained `[escalate→device]` checkpoint: verify on the M5 Max that the first real substitution loads and kills the observed tool-call loop.
5. Preserve the existing Froggeric HF/SHA/update foundation, but migrate mutable `main` plus overwrite-in-place installs into immutable `TemplateRelease` records. Fetch/resolve exact revisions, install new releases alongside the active one, retain version history, show content/capability diffs, require explicit activation, and support rollback to the model-provided template or any retained pinned release under a bounded cleanup policy. The community-standard candidates already exist: Froggeric for Qwen3.5/3.6, and the official Google template for Gemma4 (the community jscott template underperformed) — qualification, not a search for a second alternative, is the work.
6. Forbidden regardless of backend: a Llama Monitor Jinja renderer, request-rewriting shim/proxy, a fork or unreleased dependency pin, and any mutation of the canonical/HF-cache model directory. File placement operates only on the llama-monitor-owned copy/overlay so it is reversible and re-download-safe. There is no "native support unavailable → stop for A27" fork: the plan never depended on Rapid gaining native override.
7. Upgrade the llama.cpp template-library experience without inventing an overlay where its native template-file path suffices: support multiple revisioned alternatives per family, immutable provenance/hashes, compatibility and workload evidence, reversible trials/comparison, stale/update state, user notes, and rollback to model-provided behavior. For Gemma 4, discover and version the applicable Google official model-repository template as a first-class model-author source; separately ingest relevant official-repo discussion links and community forks as untrusted candidates requiring exact-revision inspection and independent workload qualification. Reuse this library/selection contract for Rapid only when a qualified native Rapid hook exists; shared UX does not imply shared runtime mechanics.
8. Add SillyTavern integration qualification for structured OpenAI chat and client-formatted text completion. For text completion, assert that SillyTavern owns the entire prompt and no backend chat-template field participates; qualify llama.cpp `/completion` and Rapid `/v1/completions` payload filtering, streaming, samplers, stops, context, response shape, errors, and cache behavior separately. For structured chat, qualify the backend template and role/tool behavior separately.
9. Qualify recommended coding/tool templates against the actual OpenCode and Hermes/OpenClaw protocols observed in Phase 0: Chat Completions, Responses, or Anthropic Messages as applicable; single/parallel tool calls; null/empty arguments; tool errors/retries; long observations; reasoning preservation; and streaming message shapes. `/props` template capabilities are necessary evidence, not sufficient proof.

**Verifier brief:** Test current-runtime truthfulness, backend switching, unsupported selection, provenance/update state, and—only if supported—actual argv and real-runtime semantic smoke tests. For a separately approved overlay prototype, verify immutable source/template/config hashes, complete tokenizer closure, no downloaded-snapshot mutation or unnecessary weight duplication, explicit selection, reversible trial/rollback, stale revisions, and independent Chat/Coding/Tool/research/Roleplay qualification.

**Hard gates:** the Rapid applier never mutates the canonical/HF-cache model dir — only an llama-monitor-owned copy/overlay, reversible and re-download-safe; the applier is described honestly per backend (llama flag vs Rapid file-placement) with no false parity claim; no raw-prompt rendering, Jinja renderer, or request-rewriting shim/proxy on any path; no fork or unreleased dependency pin; no template intervention by llama-monitor on the SillyTavern text path; kwargs are not labeled templates; a candidate template becomes active only after passing the tool-call smoke-test matrix, and a failed smoke test leaves the active selection unchanged; wrong-engine or wrong-family template cannot launch; client-formatted roleplay prompts receive no server chat template; Froggeric SHA/update behavior does not regress; mutable `main` is not a release identity; update checks never overwrite the active file before explicit activation; previous/model-provided versions remain rollback-safe; official Google sources and discussion-linked community forks have distinct provenance/trust labels; no silent template substitution or universal “newer is better” claim. A heavier full tokenizer/config-replacement overlay (beyond the sanctioned template-file copy) still requires its own approval/threat model.

**Stop/escalate:** tokenizer closure/remote code prevents a safe file-placement copy; template changes break tool/reasoning semantics on the M5 Max device check; a candidate cannot pass the tool-call smoke test for a required family.

**Non-goals:** managed overlay by default, modifying remote repos, universal template correctness inference.

**Handoff emphasis:** upstream status/link, current copy, backend-switch tests, provenance model, semantic smoke evidence.

### Phase 10 — Screenshot-driven information architecture

**Objective and outcome:** After functional parity, compare and adopt a clearer backend-adaptive organization across preset editor, wizard, Model Library, and HF flows.

**Budget:** 170k. **Prerequisites:** Phases 7–9; A16/A28/A50. **Files:** relevant JS/HTML/CSS, `tests/ui/capture.mjs`, `tests/ui/README.md`, Playwright specs, module baseline.

**Builder brief:**

1. Preserve a working baseline and capture current parity for both engines.
2. Render the accepted stable seven-category Preset Editor and intent-first six-step wizard from the completed control inventory. Preserve baseline captures and retain the documented alternatives only as fallbacks if real screenshots invalidate the accepted hierarchy.
3. Capture dark/light, desktop/narrow, empty/eligible/provisional/warning/error, long names, and expanded instructional copy sequentially after release builds.
4. Present captures and any newly exposed trade-offs to the user for final visual approval; do not silently change the accepted hierarchy.
5. Implement the approved design with shared descriptors, semantic headings, focus/keyboard order, screen-reader labels, visible warnings, reduced-motion, and light-theme coverage.
6. Update capture scenarios/docs as much as needed to make each consequential state reproducible.
7. Include coding-agent, scheduled-agent, and Roleplay workload-selection/recommendation states for both backends. Roleplay captures must show format ownership, sampler precedence, context reserve, and cache explanation without overwhelming the primary flow.

**Verifier brief:** Inspect real pixels and behavior, not descriptions; test both backends, switching, focus, keyboard, narrow clipping, theme/reduced-motion, CSS specificity/duplicates, cross-module references, XSS, and review-summary completeness.

**Hard gates:** user approval recorded; parity existed before reorder; dark/light/narrow captures reviewed; no horizontal clipping; help is not tooltip-only; warnings do not rely on color; every new animation respects reduced motion; one shared descriptor remains authoritative.

**Stop/escalate:** real captures expose a serious flaw in the accepted hierarchy; reordering changes preset semantics; visual changes require a design not approved; harness cannot reproduce a key state.

**Non-goals:** unrelated app-wide redesign, decorative animation, changing backend semantics to fit a layout.

**Handoff emphasis:** baseline and accepted-design capture sets, final visual approval, accessibility checklist, CSS audit, final scenario list.

### Phase 11 — Diagnostics, metrics, cache/storage observability, and the cross-backend Doctor

**Objective and outcome:** Expose enough bounded, privacy-safe evidence to troubleshoot effective policies, validate recommendations, and understand storage/memory pressure — and grow the existing Doctor into a **cross-backend teaching + troubleshooting pillar** (E11). Teaching users "wherever and whenever we can" is release-gating, not decoration (see Phase 14 release bar); the Doctor is the mechanism for the troubleshooting half and raises novice skill by explaining *why*, not just fixing symptoms.

**Budget:** 170k. **Prerequisites:** Phases 3 and 5–7; A9/A12/A23–A24/A31. **Files:** Rapid poller, inference/web metrics and capabilities, runtime API, dashboard cards/updater, model diagnostics, fixtures, auth/UI tests.

**Builder brief:**

1. Ingest tolerant optional metrics for cache bytes/caps, entries, hits/misses/skips/evictions, eligibility/bypass reasons, TTFT/prefill/decode savings, and Metal active/peak where upstream provides them.
2. Display requested/effective policy, downgrade reason, parser source, runtime/dependency versions, evidence confidence/staleness, and unknown/unavailable states.
3. Use bounded aggregates/histograms with units and epochs; distinguish zero from absent; never log content/secrets.
4. Implement only the approved privacy-safe repeat telemetry.
5. Surface disk checkpoint interval, cap, current use, path ownership, errors, and recovery. Add cleanup only if A24 defines safe app ownership and authorization.
6. Add schema-drift fixtures, restart/reset behavior, malformed optional/required status cases, auth and bounded-retention tests.
7. Add endpoint/workload diagnostics that distinguish streaming, tools, format path, simultaneous requests, hot-session reuse, cached-prefix tokens, context trimming/shift, and stop reason without recording prompts. Include roleplay lore-insertion/cache-miss and agent tool-schema-churn cases.
8. **Expand the Doctor into a cross-backend troubleshooting system (E11).** Grow the existing rapid-mlx-focused Doctor to cover **llama.cpp too**, drawing the llama side on the Phase 3 llama capability snapshot. Discipline (keeps it bounded for the local executor and the single-cutover release): each Doctor check traces to a **real** failure mode with the same defect→test rigor as the rest of the plan — no speculative "cover everything." Each check = detected condition + plain explanation + concrete remediation + a short "why this happens" teaching note. Provide **dual reading levels** per check from one detection engine: novice (symptom + fix + why, safe language) and power-user (flag/value/source-evidence/threshold) — reuse the `[decide-once]` educational copy from Phase 6/§13.2, do not duplicate it. Ship the checks already surfaced by this plan: KV below `q8_0` for tool-enabled llama runtimes (§3.6/§13.1), tool-call-loop template mismatch (§3.9/Phase 9), invalid `--tool-call-parser` argv (§3.1), and a stale/incompatible rapid-mlx update (the on-device update-validation probe from Phase 3/A18 is itself a Doctor surface).

**Verifier brief:** Fuzz absent/unknown/malformed metrics, check units/epochs, inspect privacy/logs, test stale capability and disk errors, validate auth/path constraints and dashboard dark/light/narrow states. For the Doctor: confirm every check maps to a real failure mode with a reproducing condition, both reading levels render the same underlying detection, and remediation text names a concrete action (no vague "check your configuration").

**Hard gates:** no prompt/response content; metrics schema drift degrades safely; required corruption remains detectable; recommendations can cite observed evidence without self-mutating; disk cleanup cannot escape owned paths or delete active data; all new routes are authenticated and bounded; every Doctor check is anchored to a real failure mode (no speculative checks), covers both backends where the failure applies, and provides condition + explanation + remediation + teaching note at both novice and power-user reading levels from one detection engine.

**Stop/escalate:** desired metric requires content capture; upstream exposes no stable signal; storage cleanup ownership is ambiguous; destructive operation lacks authority/confirmation design.

**Non-goals:** cache export/import, long-term analytics service, automatic restart/tuning, unapproved deletion.

**Handoff emphasis:** metric dictionary/units, privacy proof, schema fixtures, dashboard captures, storage threat model.

### Phase 12 — Security, dependency, and watchlist closure

**Objective and outcome:** Resolve remaining high-risk policies and explicitly classify useful external-project features without scope creep.

**Budget:** 120k. **Prerequisites:** Phases 3 and 8–11; A11–A14/A18/A24/A27. **Files:** security docs, runtime/installer/capability/model-source/storage APIs, auth tests, dependency manifests, this plan/watchlist.

**Builder brief:**

1. Revalidate Rapid remote-code behavior and upstream force-off work; preserve the distinction between the installed Rapid/MLX-LM implementation and model-repository code. Verify data-only detection, custom-code signals, provisional inspection, and exact source/revision consent; do not show blanket warnings for ordinary MLX artifacts.
2. Finalize the automated upstream-contract/resolved-receipt/rollback policy and external-environment diagnostics; ensure `[guided]` is represented separately from `[all]`. Add app-side dependency overrides only for evidenced incompatibilities, not as a manually maintained parallel package policy.
3. Complete security review for source/path canonicalization, template files/overlays if any, probes, storage, secrets, auth, limits, serialization defaults, and the llama-server Web UI group. Verify UI config/static paths, effective bind/API prefix/TLS/auth/CORS, MCP-proxy loopback defaults, and any separately approved built-in-tool allowlist. Never let `--agent` bypass individual consent or enable all tools implicitly.
4. Decide/defer cache export/import with a threat model.
5. Record waybarrios-only reranking, adapters, SSD KV tiers, multi-model registry, lazy unload, warm prompts, and remote-code controls as watchlist—not parity gaps.
6. Update upstream issue/PR links and ownership for unresolved requirements.

**Verifier brief:** Adversarially test repo identity spoofing, consent scope, traversal/symlinks, auth, malformed JSON, probe bounds, secret logging, missing extras, and dependency drift; confirm watchlist flags cannot reach Rapid argv.

**Hard gates:** no blanket remote-code warning or consent; ordinary inspected data-only MLX repositories remain low-friction; custom-code consent is evidence-bearing and revision-specific; no repository code during browsing/estimation/inspection; new read/write routes meet auth rules; secrets use existing constant-time checks; destructive operations require appropriate confirmation; optional extras are truthful; no waybarrios flag leakage.

**Stop/escalate:** secure force-off is unavailable and policy would block important models; overlay/export/import is requested; dependency pin conflicts with supported macOS/runtime matrix.

**Non-goals:** implementing waybarrios features, new backend, unapproved installer behavior.

**Handoff emphasis:** security findings, dependency matrix, consent behavior, upstream tracking, explicit deferred/watchlist table.

### Phase 13 — Surface convergence, migrations, and documentation

**Objective and outcome:** Make every user surface tell the same story and document the final behavior as if it always existed.

**Budget:** 130k. **Prerequisites:** Phases 5–12; all relevant decisions. **Files:** all touched UI/API contracts; `docs/reference/{spawn-wizard.md,vram-estimator.md,model-library.md,api.md,chat.md,cli-flags.md,dashboard.md,windows-support.md}` as applicable; migration/troubleshooting docs and screenshot references.

**Builder brief:**

1. Audit wizard, preset editor, welcome cards, Model Library, HF, diagnostics, and API for identical identity, fit, evidence, effective-policy, and warning vocabulary.
2. Ensure Approximate/Degraded/Provisional/Unavailable/Stale/Incompatible are consistently distinct.
3. Complete legacy preset/source migrations and retain unavailable-platform editability.
4. Document cache guidance with examples and non-use cases, planning context, KV modes, calibration envelope, extras, remote-code posture, templates, storage, and troubleshooting.
5. Document API schemas/default precedence and unsupported/missing-extra behavior.
6. Promote only screenshots referenced by docs; run unused-screenshot check.
7. Document OpenCode-style coding, Hermes/OpenClaw-style scheduled tools, and SillyTavern/Roleplay setup as evidence-backed client profiles. Cover endpoint URL/auth, supported API shape, streaming/SSE, tools, template ownership, request-default precedence, concurrency, context, caches, and troubleshooting.

**Verifier brief:** Cross-compare representative models/sources/settings across every surface; read docs against actual code; test migration/downgrade/unavailable states; verify screenshot references and no stale terminology.

**Hard gates:** one result vocabulary; no surface suppresses evidence grade; no docs claim unsupported template/cache/capability behavior; migrations preserve data; all required reference docs updated in the same change; promoted screenshots are referenced.

**Stop/escalate:** surfaces intentionally require different semantics not represented by the shared contract; migration is lossy; documentation reveals unresolved product behavior.

**Non-goals:** new capabilities, late IA experiments, unrelated doc overhaul.

**Handoff emphasis:** surface-consistency matrix, migration examples, docs list, screenshot references, stale-term searches.

### Phase 14 — Full validation and release gate

**Objective and outcome:** Independently prove all findings are closed, regressions are absent, and the branch is ready for human PR/release decisions.

**Single-cutover release model (B3 resolved).** This is the **one and only** release checkpoint. Execution is continuous local-model iteration (12+hrs/day); dead or unwired code *between* phase gates is expected and correct, so there is no per-phase "releasable main" invariant — the per-phase guard is only "no VISIBLE no-op / no false UX claim." The "releasable" check (no half-wired user-visible control, no partial read-path migration) applies **only here**. There is no intermediate release.

**Release bar — both required (Nick):**

1. **Full parity:** backend + frontend function as intended with rapid-mlx, verified.
2. **Dual-audience UX (release-gating, not cosmetic):** a properly designed UI/UX flow that serves both a minimally-experienced local-LLM user (safe defaults, progressive disclosure, educational copy) and a powerful technical user (full tweakability, no hidden ceilings). This elevates §12.8 (UI + a11y), the `[decide-once]` educational copy/thresholds, the template-selection UX (Phase 9), the cross-backend Doctor teaching pillar (Phase 11, E11), and progressive-disclosure IA (Phase 10) from "nice to have" to gate criteria.

**Budget:** 120k. **Prerequisites:** Phases 1–13 and all decisions. **Files:** whole diff, tests, docs, generated assets, plan completion ledger. No feature expansion.

**Builder brief:** The Builder in this phase is a release-preparation agent. It may fix only mechanical validation failures or return focused defects for separate remediation. It updates traceability and prepares evidence; it does not waive gates.

Run the mandatory checks in this exact order, committing any auto-changes before continuing when the Coordinator is authorized to commit:

```bash
rtk cargo clippy -- -D warnings
rtk cargo test
rtk npm run validate-js
rtk npm run lint
rtk git diff --check
rtk cargo build --release
rtk cargo fmt
rtk git status
```

Then run:

```bash
rtk env CI=1 LLAMA_MONITOR_USE_RELEASE=1 LLAMA_MONITOR_TEST_PORT=17778 npm test --prefix tests/ui
rtk bash scripts/check-unused-screenshots.sh
```

If platform-sensitive Rust/tray/config paths changed, also run the documented Windows target check. Run all required screenshot scenarios sequentially after the release build. If a new imported JS module exists, update and verify `tests/ui/core/js-module-baseline.json`.

**Verifier brief:** A fresh Verifier repeats risk-focused tests and a representative end-to-end matrix, audits auth/security/platform behavior, inspects final screenshots, maps every P0/P1/decision/hard gate to evidence, and confirms clean intended status.

**Hard gates:** all mandatory commands pass; full isolated Playwright passes; no P0 defect reproduces; representative Qwen3.6/Gemma4/simple/MoE cases pass; both backends pass shared-regression cases; every source and control trace is complete; security review has no unresolved high/critical finding; all decisions and external citations are current; the "releasable" check holds (no half-wired user-visible control, no partial read-path migration); the dual-audience UX release bar is met (novice safe-defaults/progressive-disclosure/educational-copy path AND power-user full-tweakability path both verified) and the cross-backend Doctor teaching pillar is present; the Builder provides a complete handoff, a fresh Verifier returns PASS, and the Coordinator accepts the evidence and closes traceability.

**Stop/escalate:** any failure needs non-mechanical product code; external baseline drifted; uncommitted unrelated changes overlap; required hardware/real runtime evidence is unavailable. Return a precise blocker and remediation phase rather than marking complete.

**Non-goals:** adding features, opportunistic refactors, creating/pushing a PR without user authorization, adding `ready-to-test`.

**Handoff emphasis:** exact check results, end-to-end traceability, final screenshots, security report, remaining external limitations, git status.

## 12. Required Validation Matrices

These are minimum matrices. Builders may reduce a Cartesian product only by documenting pairwise/risk coverage; named rows cannot be removed.

### 12.1 Architecture and memory fixtures

| Fixture | Layout | Topology | Mandatory assertions |
|---|---|---|---|
| Qwen3.6-27B | nested `text_config`, dense | mixed full + DeltaNet/linear | full layer count, recurrent state, context ceiling, KV not all-layer |
| Qwen3.6-35B-A3B | nested, MoE | mixed full + DeltaNet/linear | total/active/shared experts, active vs total weights, recurrent state |
| Gemma4-31B | nested, dense | alternating global/local SWA | distinct KV heads/dims, global KV, local window cap/pattern |
| Gemma4-26B-A4B | nested, MoE | alternating global/local SWA | expert topology plus global/local geometry, no double count |
| Qwen3-0.6B | flat, dense | full attention | flat-config regression and simple exact evidence |
| Qwen3-30B-A3B | flat/standard, MoE | ordinary attention | active-expert math; not misclassified as Qwen3.6 hybrid |

Run each applicable fixture at 8k, 32k, 131k, and its high supported boundary with bf16/int8/int4; reasoning off/on; TurboQuant none/v4/k8v4 where qualified; concurrency 1 and a bounded multi-sequence case; companions absent/present. Assert requested/effective policy, per-component bytes, active/retained separation, fit, evidence, and warnings.

### 12.2 Source lifecycle matrix

For `MlxDirectory`, `HuggingFaceRepo`, `Alias`, local/HF authoritative safetensors, `GgufFile`, legacy `model_path`, invalid/unavailable local source, and unknown future kind, cover:

| Operation | Required proof |
|---|---|
| Deserialize/migrate | no loss; safe defaults; typed wins over stale legacy |
| Display/review | canonical identity, revision, provenance, warnings |
| Edit/cancel/save/clone | correct mode/fields; cancel immutable; no dual identity |
| Estimate | same canonical source and revision; evidence preserved |
| Welcome/start | correct label, estimate, and action; never false “No model” |
| Model Library | correct association; revisions/conversions distinct |
| Launch preview/argv | exact source, no client reconstruction |

### 12.3 Control traceability matrix

For every visible Rapid and shared setting, maintain a checked table:

```text
control | capability/evidence | descriptor/UI | typed schema/default |
validation | launch/request mapping | restore/edit/clone | review/preview |
unit test | integration test | UI test | unsupported behavior
```

At minimum include model source, planning context, concurrency/admission, prefill/completion batches, KV mode, reasoning, TurboQuant, prefix budget, hybrid entries, response entries, PFlash, speculative mode/config, MLLM, embedding, parsers, request defaults, timeouts/body limits, disk checkpoint policy, and troubleshooting overrides.

### 12.4 Cache workload matrix

| Cache | Cases |
|---|---|
| Hybrid/SWA | Off; N=1/4/6/8; 1/2/3 interleaved conversations; byte-stable extension; exact repeat; changed template/tool order; full-attention ineligible; recurrent eligible; rotated SWA; PFlash overlap; count cap; byte cap; memory pressure; reload |
| Response | Off; streaming; deterministic non-streaming exact repeat; temperature 0; top_k 1; sampled; seeded; multimodal; tools/logprobs/long reasoning; concurrent cold duplicates; eviction; reload; unknown byte size |

For each case assert eligibility/bypass reason, emitted argv, recommendation/copy, predicted/observed bytes when available, metrics, and no unsupported quality/performance claim.

### 12.5 Runtime/dependency matrix

For Rapid, cover minimum-supported, audited 0.10.12, current qualified, old incompatible, missing executable, and future-compatible help fixtures against managed/external environments with MLX/MLX-LM present, MLX-VLM missing/qualified/incompatible, embeddings missing/present, guided missing/present, and changed help hash. For llama.cpp, cover the project's minimum supported build, pinned `b10068`, a pre-host-cache build, missing/changed flags, unified/partitioned/Auto semantics, and Chat/Responses/Text route availability. Verify installed vs flag-present vs provisional vs qualified vs incompatible per feature; version alone is insufficient.

MTP-specific cases:

- llama required/product single-stream mode emits `--parallel 1`, disables conflicting slot input, and queues two client requests;
- pinned current llama multi-slot MTP is labeled Experimental and must prove initialization, recurrent-state memory, output correctness, acceptance, foreground latency, and aggregate throughput before exposure;
- switching llama to overlap mode disables MTP explicitly and restores the chosen slot/context policy;
- Rapid one live greedy request activates MTP;
- Rapid non-greedy/logits-processor request and two live requests fall back to autoregressive decoding without failure or admission-policy mutation;
- Rapid returns observable MTP active/fallback reasons and includes sidecar/native MTP memory even while the fast path is inactive.

### 12.6 HF and Model Library matrix

Cover public tokenless, private, gated, missing token, paginated siblings, nested files, mutable branch resolved to revision, pinned revision, unknown finetune, verified alias, local MLX, converted derivative, two revisions, interrupted download, stale qualification, and unavailable runtime. Preserve identity/revision and distinguish search candidate from qualification throughout.

### 12.7 Template matrix

| Backend/runtime | Built-in model template | Custom library template | Missing/invalid | Tools/reasoning/multimodal |
|---|---|---|---|---|
| llama.cpp supported | regression baseline | selectable/uploadable/pinned, applied via `--chat-template`/`--chat-template-file` | actionable validation | existing behavior retained |
| Rapid 0.10.12 | model-provided/provenance | **substituted by file-placement** into an llama-monitor-owned copy/overlay (never the canonical/HF-cache dir); activation gated by the tool-call smoke test | truthful applier label (file-placement, not a runtime flag) | kwargs only where probed; template substitution fixes tool-call looping (§3.9) |
| Rapid with a template-path arg (if Phase 0 grep finds one) | capability-probed | flag-based/symmetric with llama argv | safe failure | real-runtime qualification required |
| Heavier full tokenizer/config-replacement overlay | design-only, separate approval; beyond the sanctioned template-file copy | revision-pinned, family-qualified alternatives with provenance and explicit trial/rollback | threat-model and tokenizer-closure gate | independently qualify chat, coding, tools/reasoning, roleplay, streaming, and multimodal behavior |
| SillyTavern instruct/text on llama.cpp | not applicable: client renders raw prompt | SillyTavern instruct/context template only | client-side actionable guidance | server chat-template/tool capability not part of this route |
| SillyTavern instruct/text on Rapid | not applicable: client renders raw prompt for `/v1/completions` | SillyTavern instruct/context template only | qualify Generic/VLLM-style payload/stream contract | Rapid chat-template/tool capability not part of this route |
| SillyTavern custom chat | backend model/custom template applies | only where backend supports it | prevent instruct + chat double formatting | qualify structured role/tool behavior separately |

### 12.8 UI and accessibility matrix

Surfaces: spawn wizard, preset editor, welcome card, Model Library, HF browse/selection, dashboard/diagnostics. Backends: llama.cpp and Rapid. States: empty, exact, approximate, degraded, provisional, unavailable, warning, validation error, long identity. Render dark/light and desktop/narrow. Verify focus order, keyboard operation, labels, visible focus, reduced motion, non-color warnings, no clipping, no unsafe HTML, and shared copy/defaults.

Required sequential scenarios include `spawn-wizard`, `spawn-wizard-engines`, `spawn-wizard-hf-download`, `preset-editor`, `rapid-preset`, `welcome`, `models-v2`, and—when introduced—`dashboard-rapid-mlx`/`rapid-mlx-runtime`.

### 12.9 Security, API, and platform matrix

- Auth: every new read endpoint requires `api-token`; mutations require at least `api-token`; destructive storage action requires `db-admin-token` plus confirmation where applicable.
- Input: malformed JSON → 400; repo IDs/revisions/paths canonicalized; reject traversal, absolute injection, symlink escape, oversize input, unbounded pagination, and timeouts.
- Secrets/privacy: constant-time token checks; no tokens, prompts, responses, command secrets, private metadata, or raw repeat keys in logs/metrics.
- Remote code: browse/search/estimate never execute it; consent or block is source-and-revision specific; display names cannot spoof trust.
- Platforms: full runtime behavior on supported Apple silicon; Linux/Windows preserve view/edit/migration and fail gracefully/actionably; run Windows target checks for shared platform-sensitive code.
- Shared regressions: every shared schema/API/IA change includes llama.cpp tests.

### 12.10 Representative end-to-end release cases

1. Typed HF Qwen3.6-27B: search → qualify → wizard → estimate → preset → welcome → launch → dashboard.
2. Typed HF Qwen3.6-35B-A3B: same, proving MoE and recurrent math.
3. Local Gemma4-31B: Model Library, global/local KV, hybrid guidance.
4. Local Gemma4-26B-A4B: MoE plus alternating attention.
5. Flat Qwen3 and standard Qwen3 MoE regressions.
6. Legacy Rapid preset one-time migration without loss.
7. Unknown finetune remains provisional and receives no alias-only benefit.
8. Streaming app chat: response cache ineligible; request defaults still correct.
9. Deterministic external API: response eligibility and explicit trial behavior.
10. Old unsupported and future compatible runtimes.
11. Free-form remote-code repo under selected security posture.
12. Custom llama template → Rapid → llama switch without leakage.
13. SillyTavern structured-chat roleplay: streaming, stochastic samplers, long history, lore activation, correct backend template.
14. SillyTavern client-formatted text completion: no double template, custom stops, context reserve, prefix-cache behavior.
15. One OpenCode foreground generation overlapping one scheduled research job: explicit queue/two-slot trade-off and bounded memory.

### 12.11 External-client protocol and workload matrix

| Client/profile | Routes to qualify | Request properties | Required proof |
|---|---|---|---|
| OpenCode coding | actual installed Chat/Responses/OpenAI-compatible mode | streaming, MCP/tools, tool choice, retries, long observations, sub-agents | exact event/tool shapes; cancellation; usage; prefix stability; no response-cache claim |
| Hermes Agent | actual configured OpenAI-compatible mode | streaming/include usage, stable system prompt, canonical tool JSON, concurrent tool execution | tool loop/retry/error; tool workers not counted as model sessions; cache benefit measured |
| OpenClaw scheduled/research | configured OpenAI-completions provider | streaming, tools/tool choice, stable prefix, pruning, main/sub-agent/cron overlap | actual same-model active/reusable counts; queue behavior; stable/churn prefixes |
| SillyTavern instruct/text | llama.cpp `/completion`; Rapid `/v1/completions` via qualified Generic/VLLM-style connection | already-rendered prompt, streaming, client samplers/stops, persona/lore/context budget | no server template; raw prompt round-trip; field/SSE compatibility per backend; prefix/lore churn; context reserve; swipes remain stochastic |
| SillyTavern custom chat | `/v1/chat/completions` on supported backends | structured messages, streaming, samplers/stops, backend template | correct template/roles; no client instruct double-format; long history/lore; evidence per backend |
| Deterministic batch/eval | qualified non-streaming route | greedy, exact repeats, preferably no tools | response-cache eligibility, byte/working set, isolation from interactive clients |

For every external client, capture the real request/event shape at a local test endpoint for a pinned client version. Test explicit `0`/`false`, unknown-field preservation, timeouts, cancellation, SSE pings, terminal usage chunks, tool-call streaming, retry behavior, authentication, and model alias. Documentation evidence is not a substitute for a wire capture.

These wire captures are `[escalate→device]` (§9.6 bucket 3): they are **produced during implementation** on real hardware, not pre-existing fixtures the local executor can self-validate against, so they are part of the §8.3 M5 Max device envelope — Nick + the local model capture them on the M5 Max. This does not spend frontier quota; it is a device/measurement dependency, not a reasoning judgment.

### 12.12 llama.cpp execution/cache matrix

Cross:

- KV pool: unified, partitioned, Auto;
- active slots: 1, 2, current upstream Auto;
- target context: 32k, 131k, selected model high boundary;
- occupancy: one long active request, two short, two long contending, one active plus inactive cached sessions;
- prompt cache: default/on/off;
- host cache: 0, bounded Auto, explicit fixed cap, explicit Unlimited warning;
- idle-slot publication: default/on/off;
- cache reuse: 0, 256 FIM trial, unsupported/multimodal;
- checkpoints: upstream default, disabled/changed only as troubleshooting;
- speculation: off, n-gram trial, llama MTP with required product `parallel=1`, experimental current-build multi-slot MTP, and qualified external drafter;
- workloads: coding foreground, coding + scheduled background, multiple scheduled jobs, SillyTavern text roleplay, SillyTavern chat roleplay.

Assert total KV capacity, minimum guaranteed and burst per-request context, actual launch `-c/-np/-kvu`, active and retained memory, queue/deferred behavior, TTFT, cached tokens, prompt processing saved, generation/aggregate throughput, foreground latency, context high-water/truncation, and UI/API equality. No test may multiply context by slots without naming the pool mode and guarantee being modeled.

### 12.13 Workload recommendation and precedence matrix

For Coding agent (default), Scheduled tool/research, Roleplay/storytelling, General assistant/chat, and Deterministic batch/eval, verify:

- default target context/response reserve and its evidence;
- active-generation versus reusable-session assumptions;
- backend/endpoint compatibility;
- model/template/tool/roleplay qualification;
- quant fit under the complete execution policy;
- cache and speculation recommendations with reasons/non-use cases;
- request default provenance and explicit-client precedence;
- client compaction owner and exhaustion behavior;
- Model Browser/HF filtering/ranking and evidence labels;
- identical restored state in wizard, preset editor, welcome, library, and diagnostics.

Changing model or workload updates only fields still owned by recommendations. Explicit user values—including zero/false—survive. A recommendation badge is prohibited when the selected quant/model cannot meet the workload policy.

## 13. Claim-to-Source Evidence Ledger

All external sources were checked on 2026-07-18 unless a later phase records a newer date. Prefer the immutable commit/blob URLs below. PR pages are included for rationale, discussion, and measurements; implementation claims must also point to code at a commit.

| Claim | Primary verifiable evidence | Used by |
|---|---|---|
| 0.10.12 release boundary; 75b1fe3 is a later docs-only commit, not feature implementation | <https://github.com/raullenchai/Rapid-MLX/commit/d56ae629a9c4fd6d76ef713a7342bd1bf19a8dad>, <https://github.com/raullenchai/Rapid-MLX/commit/75b1fe3b3a8ab12967f64150524296f179dd9979>, <https://github.com/raullenchai/Rapid-MLX/releases/tag/v0.10.12> | all phases |
| Hybrid cache flag/default/help | <https://github.com/raullenchai/Rapid-MLX/blob/75b1fe3b3a8ab12967f64150524296f179dd9979/vllm_mlx/cli.py#L6831-L6859> | §6.1, Phase 6 |
| Hybrid cache design, two-state heuristic, N measurements | <https://github.com/raullenchai/Rapid-MLX/pull/1111> | §6.1, Phase 6 |
| Rotated SWA correctness and prefix-extension behavior | <https://github.com/raullenchai/Rapid-MLX/pull/1124> | §6.1, Phase 6 |
| Hybrid config/store/LRU implementation and protected-entry exception | <https://github.com/raullenchai/Rapid-MLX/blob/75b1fe3b3a8ab12967f64150524296f179dd9979/vllm_mlx/memory_cache.py#L810-L850>, <https://github.com/raullenchai/Rapid-MLX/blob/75b1fe3b3a8ab12967f64150524296f179dd9979/vllm_mlx/memory_cache.py#L1750-L1785>, <https://github.com/raullenchai/Rapid-MLX/blob/75b1fe3b3a8ab12967f64150524296f179dd9979/vllm_mlx/memory_cache.py#L1966-L2030> | §6.1, Phase 6/11 |
| Hybrid metrics and wording requiring protected-entry qualification | <https://github.com/raullenchai/Rapid-MLX/blob/75b1fe3b3a8ab12967f64150524296f179dd9979/vllm_mlx/routes/metrics.py#L1140-L1238> | §6.1, Phase 6/11 |
| TurboQuant CLI/default resolution and exact alias eligibility (alias inventory/checksum must be frozen in Phase 0) | <https://github.com/raullenchai/Rapid-MLX/blob/75b1fe3b3a8ab12967f64150524296f179dd9979/vllm_mlx/cli.py#L6955-L7010>, <https://github.com/raullenchai/Rapid-MLX/blob/75b1fe3b3a8ab12967f64150524296f179dd9979/vllm_mlx/aliases.json> | D31, Phase 0/3/5/7 |
| TurboQuant retained-prefix compression/decompression and implementation-derived storage math (empirical stored-byte receipts still required for Qualified status) | <https://github.com/raullenchai/Rapid-MLX/blob/75b1fe3b3a8ab12967f64150524296f179dd9979/vllm_mlx/turboquant.py#L1-L190>, <https://github.com/raullenchai/Rapid-MLX/blob/75b1fe3b3a8ab12967f64150524296f179dd9979/vllm_mlx/memory_cache.py#L1302-L1375> | D31, Phase 0/5/11 |
| Cache documentation correction | <https://github.com/raullenchai/Rapid-MLX/pull/1129> | §6, Phase 6/13 |
| Response cache CLI/default | <https://github.com/raullenchai/Rapid-MLX/blob/75b1fe3b3a8ab12967f64150524296f179dd9979/vllm_mlx/cli.py#L6860-L6875> | §6.2, Phase 6 |
| Response cache entry-count LRU/object storage | <https://github.com/raullenchai/Rapid-MLX/blob/75b1fe3b3a8ab12967f64150524296f179dd9979/vllm_mlx/response_cache.py#L1-L58> | §6.2, Phase 6/11 |
| Response cache eligibility/rationale | <https://github.com/raullenchai/Rapid-MLX/pull/1123> | §6.2, Phase 6 |
| Response route eligibility and key inputs | <https://github.com/raullenchai/Rapid-MLX/blob/75b1fe3b3a8ab12967f64150524296f179dd9979/vllm_mlx/routes/chat.py#L2298-L2350>, <https://github.com/raullenchai/Rapid-MLX/blob/75b1fe3b3a8ab12967f64150524296f179dd9979/vllm_mlx/response_cache.py#L330-L392> | §6.2, Phase 6/11 |
| Response cache model reload invalidation | <https://github.com/raullenchai/Rapid-MLX/blob/75b1fe3b3a8ab12967f64150524296f179dd9979/vllm_mlx/server.py#L1535-L1568> | §6.2, Phase 6 |
| Tool-parser exact value/flag spelling | <https://github.com/raullenchai/Rapid-MLX/blob/75b1fe3b3a8ab12967f64150524296f179dd9979/vllm_mlx/server.py#L2030-L2052> | §3.1, Phase 1 |
| Rapid chat render path and limited `chat_template_kwargs` | <https://github.com/raullenchai/Rapid-MLX/blob/75b1fe3b3a8ab12967f64150524296f179dd9979/vllm_mlx/utils/chat_template.py#L877-L1015>, <https://github.com/raullenchai/Rapid-MLX/blob/75b1fe3b3a8ab12967f64150524296f179dd9979/vllm_mlx/api/models.py#L1572-L1585> | §3.9, D11, Phase 9 |
| Google Gemma 4 official template is revisioned and actively evolved through model-repository discussions/PRs; official and community alternatives require distinct provenance and workload qualification | <https://huggingface.co/google/gemma-4-31B-it/blob/b9ea41a2887d8607f594846523f94c6cc75ac8a4/chat_template.jinja>, <https://huggingface.co/google/gemma-4-31B-it/discussions/118>, <https://huggingface.co/google/gemma-4-31B-it/discussions/115>, <https://huggingface.co/google/gemma-4-31B-it/discussions/119>, <https://huggingface.co/google/gemma-4-31B-it/discussions/135> | D11, Phase 0/9/13 |
| Rapid raw-prompt OpenAI-compatible `/v1/completions` route and schema | <https://github.com/raullenchai/Rapid-MLX/blob/75b1fe3b3a8ab12967f64150524296f179dd9979/vllm_mlx/routes/completions.py#L73-L115>, <https://github.com/raullenchai/Rapid-MLX/blob/75b1fe3b3a8ab12967f64150524296f179dd9979/vllm_mlx/api/models.py#L2124-L2205> | D21, §6.4, Phase 3/9 |
| Rapid dependency/extras declarations | <https://github.com/raullenchai/Rapid-MLX/blob/75b1fe3b3a8ab12967f64150524296f179dd9979/pyproject.toml#L30-L58>, <https://github.com/raullenchai/Rapid-MLX/blob/75b1fe3b3a8ab12967f64150524296f179dd9979/pyproject.toml#L107-L165> | §2.2, Phase 3/12 |
| Rapid remote-code default and normal construction | <https://github.com/raullenchai/Rapid-MLX/blob/75b1fe3b3a8ab12967f64150524296f179dd9979/vllm_mlx/engine/batched.py#L745-L802>, <https://github.com/raullenchai/Rapid-MLX/blob/75b1fe3b3a8ab12967f64150524296f179dd9979/vllm_mlx/server.py#L1504-L1522> | §3.10, D12, Phase 1/12 |
| Rapid MTP single-live greedy gate, AR handoff, and documented mid-stream caveat | <https://github.com/raullenchai/Rapid-MLX/blob/75b1fe3b3a8ab12967f64150524296f179dd9979/vllm_mlx/scheduler.py#L532-L580>, <https://github.com/raullenchai/Rapid-MLX/blob/75b1fe3b3a8ab12967f64150524296f179dd9979/vllm_mlx/scheduler.py#L686-L725> | D25, Phase 3/5/7/11 |
| Rapid Qwen MTP runtime injection requires separately resolved sidecar weights at the audited commit | <https://github.com/raullenchai/Rapid-MLX/blob/75b1fe3b3a8ab12967f64150524296f179dd9979/vllm_mlx/spec_decode/mtp/qwen3_5_inject.py#L1-L25>, <https://github.com/raullenchai/Rapid-MLX/blob/75b1fe3b3a8ab12967f64150524296f179dd9979/vllm_mlx/spec_decode/mtp/qwen3_5_inject.py#L338-L425>, <https://github.com/raullenchai/Rapid-MLX/blob/75b1fe3b3a8ab12967f64150524296f179dd9979/vllm_mlx/engine/batched.py#L133-L225> | D1/D25, Phase 3–5/8 |
| Rapid HY3 automatically resolves a pinned external MTP sidecar because the converted base strips the head | <https://github.com/raullenchai/Rapid-MLX/blob/75b1fe3b3a8ab12967f64150524296f179dd9979/vllm_mlx/spec_decode/mtp/hy3_inject.py#L33-L51>, <https://github.com/raullenchai/Rapid-MLX/blob/75b1fe3b3a8ab12967f64150524296f179dd9979/vllm_mlx/spec_decode/mtp/hy3_inject.py#L175-L197> | D1/D25, Phase 3–5/8 |
| Rapid Gemma4 assistant loader exists but production dispatch/detection do not enable it at the audited commit | <https://github.com/raullenchai/Rapid-MLX/blob/75b1fe3b3a8ab12967f64150524296f179dd9979/vllm_mlx/spec_decode/mtp/gemma4_inject.py#L1-L70>, <https://github.com/raullenchai/Rapid-MLX/blob/75b1fe3b3a8ab12967f64150524296f179dd9979/vllm_mlx/spec_decode/mtp/dispatch.py#L38-L74>, <https://github.com/raullenchai/Rapid-MLX/blob/75b1fe3b3a8ab12967f64150524296f179dd9979/vllm_mlx/spec_decode/mtp/detect.py#L136-L157> | D25, Phase 0/3–5 |
| MLX-LM has a template override, but this does not prove Rapid exposes it | <https://github.com/ml-explore/mlx-lm/blob/v0.31.3/mlx_lm/server.py#L1793-L1852> | D11, Phase 9 |
| Rapid and waybarrios are separate repositories/projects | <https://github.com/raullenchai/Rapid-MLX>, <https://github.com/waybarrios/vllm-mlx> | §2.2, Phase 3/12 |
| Rapid arbitrary HF/local text loading uses MLX-LM and does not perform Transformers-to-MLX conversion during serve | <https://github.com/raullenchai/Rapid-MLX/blob/75b1fe3b3a8ab12967f64150524296f179dd9979/vllm_mlx/cli.py#L8388-L8465>, <https://github.com/raullenchai/Rapid-MLX/blob/75b1fe3b3a8ab12967f64150524296f179dd9979/vllm_mlx/utils/tokenizer.py#L742-L865> | §2.7, D29, Phase 3/8 |
| MLX conversion is a separate revision/options/provenance-bearing operation | <https://github.com/ml-explore/mlx-lm/blob/ed1fca4cef15a824c5f1702c80f70b4cffc8e4dd/mlx_lm/convert.py>, <https://github.com/ml-explore/mlx-lm/blob/ed1fca4cef15a824c5f1702c80f70b4cffc8e4dd/README.md#L92-L110> | §2.7, D29, Phase 2/8 |
| Community Heretic MLX lineage and mixed quant require original-author/converter separation and config inspection | <https://huggingface.co/froggeric/Qwen3.6-35B-A3B-Uncensored-Heretic-MLX-4bit/tree/32939a6cef2750f18ccf352443f22f2e4dfe3613>, <https://huggingface.co/froggeric/Qwen3.6-35B-A3B-Uncensored-Heretic-MLX-4bit/blob/32939a6cef2750f18ccf352443f22f2e4dfe3613/config.json>, <https://huggingface.co/llmfan46/Qwen3.6-35B-A3B-uncensored-heretic/tree/dbfd9eb0cdc7c33fc970b06429f6e043b6851190> | §2.7, D9/D29, Phase 0/8 |
| Unsloth and LM Studio publish both GGUF and MLX siblings for one lineage | <https://huggingface.co/unsloth/Qwen3.6-35B-A3B-UD-MLX-4bit/tree/6700c3e5bdeb050a379c8d2a4133f43f3647f20f>, <https://huggingface.co/unsloth/Qwen3.6-35B-A3B-GGUF/tree/a483e9e6cbd595906af30beda3187c2663a1118c>, <https://huggingface.co/lmstudio-community/Qwen3.6-35B-A3B-MLX-4bit/tree/0c4a20a6437ae5985ddc9eb1a3f122ee6c151c3b>, <https://huggingface.co/lmstudio-community/Qwen3.6-35B-A3B-GGUF/tree/68a34855558af61cbef0324d31f411be8a506b08> | §2.7, D9/D29, Phase 8 |
| Current llama.cpp release/source freeze | <https://github.com/ggml-org/llama.cpp/releases/tag/b10068>, <https://github.com/ggml-org/llama.cpp/commit/571d0d540df04f25298d0e159e520d9fc62ed121> | §3.11, §6.5, Phase 0/3/5/6 |
| llama host-cache default 8192 MiB | <https://github.com/ggml-org/llama.cpp/blob/571d0d540df04f25298d0e159e520d9fc62ed121/common/common.h#L622-L627> | §3.11, D18, Phase 1/5/6 |
| llama `-1` means unlimited rather than reserved | <https://github.com/ggml-org/llama.cpp/blob/571d0d540df04f25298d0e159e520d9fc62ed121/tools/server/server-task.h#L633-L648> | §3.11, D18, Phase 1/6 |
| llama prompt state allocates on save and may evict/skip | <https://github.com/ggml-org/llama.cpp/blob/571d0d540df04f25298d0e159e520d9fc62ed121/tools/server/server-task.cpp#L1655-L1735>, <https://github.com/ggml-org/llama.cpp/pull/16391> | §6.5, Phase 5/6/11 |
| llama total context and unified/partitioned per-sequence semantics | <https://github.com/ggml-org/llama.cpp/blob/571d0d540df04f25298d0e159e520d9fc62ed121/src/llama-context.cpp#L287-L304> | §3.11, D23, Phase 5 |
| llama Auto slots resolve to four with unified KV at pinned build | <https://github.com/ggml-org/llama.cpp/blob/571d0d540df04f25298d0e159e520d9fc62ed121/tools/server/server.cpp#L146-L150> | §6.5, Phase 3/5/7 |
| llama cache/slot/checkpoint/continuous-batching CLI | <https://github.com/ggml-org/llama.cpp/blob/571d0d540df04f25298d0e159e520d9fc62ed121/tools/server/README.md#L163-L177> | §6.5, Phase 3/6 |
| llama idle-slot cache behavior differs from ordinary prefix reuse | <https://github.com/ggml-org/llama.cpp/blob/571d0d540df04f25298d0e159e520d9fc62ed121/tools/server/server-context.cpp#L1409-L1420>, <https://github.com/ggml-org/llama.cpp/blob/571d0d540df04f25298d0e159e520d9fc62ed121/tools/server/server-context.cpp#L2393-L2405> | §3.11, §6.5, Phase 1/6 |
| llama request prompt-cache semantics/non-bit-identical warning | <https://github.com/ggml-org/llama.cpp/blob/571d0d540df04f25298d0e159e520d9fc62ed121/tools/server/README.md#L520-L520> | §6.5, Phase 6/13 |
| llama request cache/progress schema | <https://github.com/ggml-org/llama.cpp/blob/571d0d540df04f25298d0e159e520d9fc62ed121/tools/server/server-schema.cpp#L17-L69> | Phase 6/11 |
| llama host-cache slot selection uses LCP/LRU | <https://github.com/ggml-org/llama.cpp/blob/571d0d540df04f25298d0e159e520d9fc62ed121/tools/server/server-context.cpp#L1528-L1583> | §6.5, Phase 6 |
| llama cache reuse behavior and multimodal exclusion | <https://github.com/ggml-org/llama.cpp/blob/571d0d540df04f25298d0e159e520d9fc62ed121/tools/server/server-context.cpp#L1209-L1229> | §6.5, Phase 6 |
| llama coder/FIM presets use cache reuse 256 | <https://github.com/ggml-org/llama.cpp/blob/571d0d540df04f25298d0e159e520d9fc62ed121/common/arg.cpp#L4248-L4344> | §6.5, Phase 6 |
| llama SWA checkpoints/default/rationale | <https://github.com/ggml-org/llama.cpp/pull/15293>, <https://github.com/ggml-org/llama.cpp/blob/571d0d540df04f25298d0e159e520d9fc62ed121/tools/server/server-context.cpp#L3225-L3330> | §6.5, Phase 3/6 |
| llama bundled Web UI is default-on and exposes explicit availability/config/static-path controls | <https://github.com/ggml-org/llama.cpp/blob/571d0d540df04f25298d0e159e520d9fc62ed121/common/arg.cpp#L3093-L3155>, <https://github.com/ggml-org/llama.cpp/blob/571d0d540df04f25298d0e159e520d9fc62ed121/common/arg.cpp#L3191-L3197>, <https://github.com/ggml-org/llama.cpp/blob/571d0d540df04f25298d0e159e520d9fc62ed121/tools/server/README.md#L2053-L2061> | D26/A44, Phase 1/3/7/12/13 |
| llama built-in tools/agent/UI MCP proxy are experimental/security-sensitive | <https://github.com/ggml-org/llama.cpp/blob/571d0d540df04f25298d0e159e520d9fc62ed121/common/arg.cpp#L3157-L3189>, <https://github.com/ggml-org/llama.cpp/blob/571d0d540df04f25298d0e159e520d9fc62ed121/tools/server/README.md#L195-L202> | §3.11, D26/A44, Phase 1/7/12 |
| llama function calling/template capability fields | <https://github.com/ggml-org/llama.cpp/blob/571d0d540df04f25298d0e159e520d9fc62ed121/docs/function-calling.md#L1-L28>, <https://github.com/ggml-org/llama.cpp/blob/571d0d540df04f25298d0e159e520d9fc62ed121/common/jinja/caps.h#L8-L29> | Phase 3/9 |
| llama Chat Completions and Responses routes | <https://github.com/ggml-org/llama.cpp/blob/571d0d540df04f25298d0e159e520d9fc62ed121/tools/server/README.md#L1237-L1429> | Phase 3/7/9 |
| llama metrics available and cache-metric limitation | <https://github.com/ggml-org/llama.cpp/blob/571d0d540df04f25298d0e159e520d9fc62ed121/tools/server/README.md#L1055-L1076> | Phase 11 |
| llama speculative modes and non-normative defaults | <https://github.com/ggml-org/llama.cpp/blob/571d0d540df04f25298d0e159e520d9fc62ed121/docs/speculative.md#L1-L188>, <https://github.com/ggml-org/llama.cpp/blob/571d0d540df04f25298d0e159e520d9fc62ed121/common/arg.cpp#L4403-L4420> | D24, Phase 5/7 |
| pinned llama.cpp can initialize MTP with `n_parallel` and keeps per-sequence state | <https://github.com/ggml-org/llama.cpp/blob/571d0d540df04f25298d0e159e520d9fc62ed121/tools/server/server-context.cpp#L360-L376>, <https://github.com/ggml-org/llama.cpp/blob/571d0d540df04f25298d0e159e520d9fc62ed121/tools/server/server-context.cpp#L1260-L1280>, <https://github.com/ggml-org/llama.cpp/blob/571d0d540df04f25298d0e159e520d9fc62ed121/common/speculative.cpp#L1204-L1255> | D24, Phase 3/5 |
| upstream report: multi-slot MTP adds recurrent-state memory and weak benefit | <https://github.com/ggml-org/llama.cpp/issues/24320> | D24, Phase 0/5 |
| OpenCode custom OpenAI-compatible provider/base URL | <https://github.com/anomalyco/opencode/blob/b8142c7aa8f88222873fb79d636e312e28037c2d/packages/opencode/src/session/llm/native-request.ts#L145-L180> | §1.1, Phase 0/3/7 |
| OpenCode streaming request with tools/tool choice | <https://github.com/anomalyco/opencode/blob/b8142c7aa8f88222873fb79d636e312e28037c2d/packages/opencode/src/session/llm.ts#L270-L325> | §2.6, §6.4, Phase 6/9 |
| OpenCode deterministic tool ordering and MCP tool conversion | <https://github.com/anomalyco/opencode/blob/b8142c7aa8f88222873fb79d636e312e28037c2d/packages/opencode/src/session/llm/request.ts#L180-L190>, <https://github.com/anomalyco/opencode/blob/b8142c7aa8f88222873fb79d636e312e28037c2d/packages/opencode/src/session/tools.ts#L390-L450> | §2.6, §6.1/6.5 |
| OpenCode outer retry behavior | <https://github.com/anomalyco/opencode/blob/b8142c7aa8f88222873fb79d636e312e28037c2d/packages/opencode/src/session/processor.ts#L630-L675>, <https://github.com/anomalyco/opencode/blob/b8142c7aa8f88222873fb79d636e312e28037c2d/packages/opencode/src/session/retry.ts> | D17, Phase 6/11 |
| Hermes Agent custom OpenAI-compatible provider | <https://github.com/NousResearch/hermes-agent/blob/614dc194ea7d853d39f9e84582ec62156f41a475/cli-config.yaml.example#L37-L102> | §1.1, Phase 0/3/7 |
| Hermes streaming with usage | <https://github.com/NousResearch/hermes-agent/blob/614dc194ea7d853d39f9e84582ec62156f41a475/agent/chat_completion_helpers.py#L2168-L2195>, <https://github.com/NousResearch/hermes-agent/blob/614dc194ea7d853d39f9e84582ec62156f41a475/agent/chat_completion_helpers.py#L2550-L2615> | §2.6, §6.4, Phase 6/9 |
| Hermes prefix canonicalization and stable system prompt | <https://github.com/NousResearch/hermes-agent/blob/614dc194ea7d853d39f9e84582ec62156f41a475/agent/conversation_loop.py#L995-L1045>, <https://github.com/NousResearch/hermes-agent/blob/614dc194ea7d853d39f9e84582ec62156f41a475/agent/system_prompt.py#L480-L545> | §2.6, §6.1/6.5 |
| Hermes tool concurrency is not conversation count | <https://github.com/NousResearch/hermes-agent/blob/614dc194ea7d853d39f9e84582ec62156f41a475/agent/tool_executor.py#L65-L78>, <https://github.com/NousResearch/hermes-agent/blob/614dc194ea7d853d39f9e84582ec62156f41a475/agent/tool_executor.py#L665-L682> | §6.1, Phase 6 |
| OpenClaw local/custom provider support | <https://github.com/openclaw/openclaw/blob/ec740e79a48c1d7879fe3e8f211b4f0719d5ec0e/docs/gateway/local-models.md> | §1.1, Phase 0/3/7 |
| OpenClaw OpenAI-completions transport streams with tools | <https://github.com/openclaw/openclaw/blob/ec740e79a48c1d7879fe3e8f211b4f0719d5ec0e/src/agents/openai-completions-transport.ts#L1660-L1725> | §2.6, §6.4, Phase 6/9 |
| OpenClaw stable-prefix/deterministic MCP ordering | <https://github.com/openclaw/openclaw/blob/ec740e79a48c1d7879fe3e8f211b4f0719d5ec0e/docs/reference/prompt-caching.md#L125-L150> | §2.6, §6.1/6.5 |
| OpenClaw concurrency defaults are capacity, not cache N | <https://github.com/openclaw/openclaw/blob/ec740e79a48c1d7879fe3e8f211b4f0719d5ec0e/docs/gateway/config-agents.md#L385-L460>, <https://github.com/openclaw/openclaw/blob/ec740e79a48c1d7879fe3e8f211b4f0719d5ec0e/docs/tools/subagents.md#L390-L410>, <https://github.com/openclaw/openclaw/blob/ec740e79a48c1d7879fe3e8f211b4f0719d5ec0e/docs/automation/cron-jobs.md#L545-L575> | §6.1, Phase 6/11 |
| SillyTavern Chat versus Text Completion distinction | <https://github.com/SillyTavern/SillyTavern-Docs/blob/70e5e4d3c239253fca4692fe82e3936cb9c4b1b1/Usage/API_Connections/index.md#L13-L33> | D21, Phase 3/7/9 |
| SillyTavern custom OpenAI-compatible Chat Completion | <https://github.com/SillyTavern/SillyTavern-Docs/blob/70e5e4d3c239253fca4692fe82e3936cb9c4b1b1/Usage/API_Connections/openai.md#L100-L137> | D21, Phase 3/9 |
| SillyTavern llama.cpp Text Completion calls `/completion` | <https://github.com/SillyTavern/SillyTavern/blob/8172dcd0ee672d3cd9a5e5f7af134f91a45cd2b8/src/endpoints/backends/text-completions.js#L116-L136>, <https://github.com/SillyTavern/SillyTavern/blob/8172dcd0ee672d3cd9a5e5f7af134f91a45cd2b8/src/endpoints/backends/text-completions.js#L305-L345> | D21, §6.4, Phase 9 |
| SillyTavern Generic/VLLM Text Completion maps to `/v1/completions` | <https://github.com/SillyTavern/SillyTavern/blob/8172dcd0ee672d3cd9a5e5f7af134f91a45cd2b8/src/endpoints/backends/text-completions.js#L280-L325> | D21, §6.4, Phase 9 |
| SillyTavern Text Completion forwards streaming | <https://github.com/SillyTavern/SillyTavern/blob/8172dcd0ee672d3cd9a5e5f7af134f91a45cd2b8/src/endpoints/backends/text-completions.js#L394-L412> | §6.4, Phase 6/9 |
| SillyTavern client owns streaming/context/samplers and instruct formatting | <https://github.com/SillyTavern/SillyTavern-Docs/blob/70e5e4d3c239253fca4692fe82e3936cb9c4b1b1/Usage/Common-Settings.md#L7-L65>, <https://github.com/SillyTavern/SillyTavern-Docs/blob/70e5e4d3c239253fca4692fe82e3936cb9c4b1b1/Usage/Prompts/instructmode.md#L10-L31> | D21–D22, Phase 2/7/9 |
| SillyTavern context/persona/lore injection can change prefix | <https://github.com/SillyTavern/SillyTavern-Docs/blob/70e5e4d3c239253fca4692fe82e3936cb9c4b1b1/Usage/Prompts/context-template.md#L9-L80>, <https://github.com/SillyTavern/SillyTavern-Docs/blob/70e5e4d3c239253fca4692fe82e3936cb9c4b1b1/Usage/worldinfo.md#L19-L35>, <https://github.com/SillyTavern/SillyTavern-Docs/blob/70e5e4d3c239253fca4692fe82e3936cb9c4b1b1/Usage/personas.md#L36-L45> | §6.4, Phase 6/11 |
| Qwen3.6-27B real nested config | <https://huggingface.co/mlx-community/Qwen3.6-27B-4bit/raw/main/config.json> | Phase 0/4/5 |
| Qwen3.6-35B-A3B real nested MoE config | <https://huggingface.co/mlx-community/Qwen3.6-35B-A3B-4bit/raw/main/config.json> | Phase 0/4/5 |
| Gemma4-31B real global/local config | <https://huggingface.co/mlx-community/gemma-4-31b-it-4bit/raw/main/config.json> | Phase 0/4/5 |
| Gemma4-26B-A4B real global/local MoE config | <https://huggingface.co/mlx-community/gemma-4-26b-a4b-it-4bit/raw/main/config.json> | Phase 0/4/5 |

The HF URLs in this table are discovery links and are mutable. Phase 0 must replace or supplement each with `resolve/<commit>/config.json`, the returned revision, SHA-256, and a local provenance manifest before using it as a test oracle.

### 13.1 Evidence still requiring an immutable line-level pin in Phase 0

The audit verified these against upstream source, but a future implementation agent must locate and record exact blob lines at the frozen commit because source line locations can move and this plan must not turn recollection into authority:

- `pyproject.toml`: MLX/MLX-LM/Transformers ranges, MLX-VLM exclusion, embeddings, guided/outlines, and whether `[all]` excludes `[guided]`;
- `BatchedEngine` construction: `trust_remote_code=True`, plus serve CLI proving no force-off flag;
- CLI and runtime paths for KV dtype defaults/choices, reasoning's effective int8 behavior, model-safe bf16 fallback, TurboQuant, PFlash, prefix-cache budget, batching/concurrency, and disk checkpoint interval/20 GiB cap;
- `rapid-mlx info`: alias-profile-only eligibility, partial regex inference, and absence of vision/embedding extras;
- deprecated `vllm-mlx*` executable aliases and both projects' package/dependency declarations;
- Rapid tokenizer/chat-template application path and the absence of a Rapid custom override;
- exact response-cache eligibility/hash/lookup/store lines beyond the LRU class;
- exact scheduler/cache implementation behind hybrid entry counting, global byte limits, PFlash bypass, and SWA exact-repeat behavior;
- HF API semantics used for pagination, revisions, gated/private access, and sibling/file sizing;
- community/Froggeric template repository, revision, license, provenance, and update policy before any Rapid-related reuse claim; and the official Google Gemma4 template revision (the community jscott template underperformed) as the candidate to qualify.

KV-dtype pin discipline (E3): the pins above are **factual** — the flags, allowed choices, and defaults that exist in the `vllm_mlx` parser. Do NOT record a *recommended* tool-calling KV value on the MLX side here: because TurboQuant/RotorQuant redistribute precision, "4-bit MLX" is not `q4_0`, the llama.cpp `q8_0`-for-tools floor does not port, and the right MLX value is unknown and belongs to the §8.3 `[escalate→device]` M5 Max measurement envelope (TurboQuant vs RotorQuant vs plain 4-bit). Encode the llama.cpp heuristic (warn below `q8_0` KV for tool-enabled runtimes) as a fact; leave the MLX floor "measured, not asserted" (D20 — no cross-backend KV-vocabulary normalization).

### 13.2 Local defect evidence to preserve in tests

External citations do not replace local regression tests. Before fixing each defect, add or preserve a fixture that reproduces it:

| Local claim | Current area | Required regression proof |
|---|---|---|
| invalid parser arguments | `src/inference/rapid_mlx/command.rs` | exact pre-fix argv fixture; post-fix valid value/flag |
| typed-source frontend split brain | presets/wizard/setup/models/vram JS and Rust source contract | typed capture no longer says “No model configured”; source matrix |
| visible speculative/MLLM/embedding controls do not launch | wizard payload, `RapidMlxConfig`, command builder | control traceability tests before re-exposure |
| Rapid sampling ignored | `request_from_preset()` and proxy/chat request path | omission/explicit-value integration tests |
| architecture loss | `mlx_meta.rs` and estimator | pinned Qwen3.6/Gemma4 expected-fact fixtures |
| bytes-to-bits error | MLX size fallback | numeric unit test with known size/bit width |
| llama vocabulary leaks into Rapid | VRAM API/UI and auto-size | request/result/visual regression tests |
| `info` confidence overreach | `info_query.rs`/profile callers | unknown-finetune fixture remains provisional |
| stock template tool-call unreliability; no revision-pinned substitution (§3.9) | command/capability/template UI, chat-template registry | reproduce a stock-template tool-call loop; prove the substituted (Froggeric/Google-official) template via file-placement kills it and passes the tool-call smoke test; honest per-backend applier label |
| Metal auto-selects unlimited llama host cache | wizard auto-size/platform initialization | no new Auto preset emits `-cram -1`; explicit legacy value preserved/warned |
| llama context/slot estimator-launch mismatch | llama command, VRAM estimator/API, all estimate surfaces | unified/partitioned/Auto matrix with guaranteed/burst context |
| unconditional llama Web UI MCP proxy | `src/inference/llama_cpp.rs` | ordinary external-agent argv omits flag; explicit secure Web UI case only |
| cache-idle copy conflates mechanisms | preset/wizard HTML/JS/docs | lifecycle test plus reviewed educational copy `[decide-once]` (Nick approves the copy string once in refinement; the gate then becomes `[local-verifiable]`) |
| llama MTP slot policy is hidden | wizard payload and launch policy | explicit MTP single-stream lock/queue guidance; multi-slot mode disables MTP; no silent rewrite |
| Rapid MTP benefit is conditional | scheduler/profile/diagnostics | single-greedy-request activation and batched/non-greedy fallback tests/metrics |
| generic quant recommendation ignores workload | estimator/Model Browser/HF/wizard | workload-fit recommendation equality and badge gate `[decide-once]` (Nick sets the fit threshold once; the gate then becomes `[local-verifiable]`) |
| explicit sampler zeros are lost | wizard/preset/default/request path | zero/false/omission round-trip and client-precedence tests |

## 14. Upstream Revalidation Procedure

At Phase 0 and again before any phase that depends on drift-prone upstream behavior:

1. Query the latest Rapid release and default-branch HEAD. Compare them with `75b1fe3` and record commits touching CLI, cache, templates, model loading, info, dependencies, metrics, and memory.
2. Fetch the exact tag/commit source; never reason only from README/release prose.
3. Capture `rapid-mlx --version`, `rapid-mlx serve --help`, executable hash/path, and package versions from the actual managed runtime. Hash normalized help text.
4. Inspect `pyproject.toml` and lock/resolution output for exact core/optional packages. Verify extras empirically without installing unless authorized.
5. For each source-ledger claim, mark `unchanged`, `changed-compatible`, `changed-breaking`, or `removed`, attach immutable URLs, and identify affected decision/phase/tests.
6. Resolve each HF repo to a commit via the Hub API; fetch config/tokenizer/index at that revision; record ETag/commit/SHA-256 and retrieval date. Do not refresh a pinned fixture in place without reviewing the diff.
7. Re-run capability/help fixtures and the six architecture expected-fact tests. A changed upstream default requires a product decision, migration analysis, updated educational copy, and recalibration—not merely a constant update.
8. Check open/merged upstream issues/PRs for custom templates, remote-code force-off, cache byte metrics, response-cache byte cap, and relevant model support. Record URLs and status.
9. If evidence conflicts, stop implementation, update Sections 2/6/8/13, and present the two best product approaches to the user.

## 15. Completion and Traceability Ledger

The Coordinator maintains this table after each independently verified phase. A phase is not complete merely because code exists.

| Finding/requirement | Owning phase(s) | Required evidence | Status |
|---|---|---|---|
| Invalid tool-parser args | 1 | argv unit/integration tests; source search | Open |
| Typed source split brain | 2 | full source matrix; Playwright; captures | Open |
| No-op controls | 1, 7 | hidden interim; full control traceability | Open |
| Rapid request defaults ignored | 2 | proxy/app/external omission tests | Open |
| Architecture metadata gaps | 4 | six pinned fixture suites | Open |
| Rapid-native memory policy | 5 | policy matrix, calibration, cross-surface equality | Open |
| Hybrid/SWA cache guidance | 6, 11 | workload/memory/copy/telemetry evidence | Open |
| Response-cache guidance | 6, 11 | eligibility/bypass/copy/bytes evidence | Open |
| Runtime/extras qualification | 3, 12 | capability/dependency/smoke matrix | Open |
| HF and Model Library parity | 8, 10 | identity/network/library/UI matrix | Open |
| Stock chat-template tool-call unreliability + revision-pinned substitution (§3.9, E1) | 9, 11 | reproduce stock-template tool-call loop; substituted (Froggeric/Google-official) template via llama flag / Rapid file-placement kills it; tool-call smoke-test-gated activation; M5 Max device check; honest per-backend applier label; backend-switch/no-leak tests | Open |
| Preset schema migration/versioning (D32, E10) | 2, 13 | schema-version field; forward-migration on read; save→load→save round-trip; migration of presets from today's shipped llama-monitor; safe downgrade | Open |
| Cross-backend Doctor teaching/troubleshooting pillar (E11) | 11 | every check anchored to a real failure mode; both backends where applicable; condition + explanation + remediation + teaching note at novice and power-user reading levels from one detection engine | Open |
| Dual-audience UX + single-cutover release bar (B3, release bar) | 14 | novice safe-default/progressive-disclosure/educational-copy path AND power-user full-tweakability path both verified; "releasable" check applies only at this one checkpoint | Open |
| Remote-code posture | 1, 12 | decision, consent/block tests, upstream status | Open |
| llama unlimited Metal cache recommendation | 1, 5, 6 | no auto `-1`; bounded retained-memory policy/calibration | Open |
| llama context/parallel contract | 0, 5, 7 | unified/partitioned/Auto launch-estimator-surface matrix | Open |
| llama Web UI MCP proxy exposure | 1, 12 | default-off argv and security tests | Open |
| llama cache teaching/telemetry | 6, 11 | mechanism/workload matrix, bounded bytes, cached-token evidence | Open |
| workload/default/quant precedence | 2, 5, 7, 8 | explicit-zero/provenance and fit recommendation tests | Open |
| llama/Rapid MTP concurrency policy | 3, 5, 7, 11 | llama explicit single-stream lock; Rapid memory-first one-active Auto plus fully refitted Advanced overlap, companion ownership, and activation/fallback/handoff metrics | Open |
| OpenCode/Hermes/OpenClaw qualification | 0, 3, 6, 9, 11 | pinned source plus real wire/tool-loop fixtures | Open |
| SillyTavern Text and Chat qualification | 3, 6, 7, 9, 11 | llama `/completion`, Rapid `/v1/completions`, Chat path fixtures | Open |
| UI/IA improvement | 7, 10 | accepted seven-category/six-step direction, then screenshot-backed final visual approval and accessibility evidence | Open |
| Diagnostics/storage | 11, 12 | schema/privacy/auth/path/recovery tests | Open |
| Documentation/convergence | 13 | surface and doc audit | Open |
| Full release readiness | 14 | mandatory checks, full Playwright, security, clean status | Open |

Final completion requires: all rows closed; every decision recorded; immutable external evidence current; no unresolved hard-gate failure; all Builder/Verifier handoffs retained in the working record; and explicit Coordinator confirmation that the implementation matches this plan rather than merely passing tests.
