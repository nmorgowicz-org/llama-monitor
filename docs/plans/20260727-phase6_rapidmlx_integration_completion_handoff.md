# Phase 6 Rapid-MLX Integration Completion Handoff

**Date:** 2026-07-27  
**Status:** Execution handoff; Phase 6 is not complete  
**Branch/worktree:** Use the current checkout in place. It contains substantial, intentional, uncommitted Phase 5/6 work and benchmark receipts. Do not reset, discard, or broadly rewrite it.  
**Primary goal:** Finish the Rapid-MLX integration so the Spawn Wizard, Preset Editor, welcome-screen preset cards, launch backend, and canonical VRAM estimator all describe and launch the same effective configuration.

## 1. Why this handoff exists

The current branch contains a large amount of useful implementation and measured benchmark evidence, but several important Rapid-MLX paths are only partially connected. Some comments and older planning assumptions also describe behavior that the latest source build has superseded.

This document records repository truth as of 2026-07-27. It is deliberately explicit about:

- what is implemented;
- what is only represented in a schema;
- what reaches the Rapid-MLX command line;
- what reaches the estimator;
- what is visible in the UI;
- what is benchmark-qualified;
- what remains approximate;
- what is wrong or internally inconsistent;
- what an agent must change and verify before declaring Phase 6 complete.

This is an execution handoff, not a claim that the work described below has already been completed.

## 2. User requirements that are hard gates

The completed product must satisfy all of the following.

1. Rapid-MLX must be a first-class MLX loader across backend and frontend surfaces.
2. Active KV memory must be estimated for Rapid-native `bf16`, `int8`, and `int4` `--kv-cache-dtype` values using introspected model geometry.
3. Rapid's `--reasoning` profile must make the **effective** active KV dtype `int8`, regardless of the requested dtype. The installed/source-build help text states:

   ```text
   --reasoning
       Reasoning profile: pins --kv-cache-dtype to int8 regardless of the dtype
       flag (sub-4-bit drops -20pt on AIME-class math for Qwen3 thinking variants).
   ```

4. Reasoning is the expected normal use case. The UI must not quietly estimate INT4 while launching a reasoning profile that uses INT8.
5. The Spawn Wizard and Preset Editor must elevate:

   - `--tool-call-parser`;
   - `--reasoning-parser`;
   - `--force-hybrid`;
   - `--no-hybrid`.

6. Auto must remain the preferred setting. Auto means the Rapid model alias/profile owns the decision and Llama Monitor emits no override flag.
7. Explicit overrides must be available for modified finetunes and distillations whose metadata or Rapid alias detection is wrong.
8. Local MLX repositories and Hugging Face repositories must be introspected before download where possible, using bounded reads/fetches of the critical metadata files.
9. Standard context choices must be 32K, 65K, 131K, 160K, 200K, and 262K. Metadata's native context limit must be displayed and values above it must be marked advanced/unqualified, without inventing RoPE/YaRN flags.
10. Rapid retained prefix cache must be presented as optional memory **in addition to** weights, active KV, recurrent state, runtime overhead, and system reserve.
11. The qualified retained-cache default is 8 GiB. A 16 GiB option is for retaining older branches, not for increasing newest-fork speed.
12. Text prefill remains 512. Vision admission may try 1024, then 1536, then 2048, and must never exceed 2048.
13. Do not implement MTPLX now. Keep loader-facing UI/data structures broad enough that another MLX loader can be added later.
14. Do not claim modern-Qwen vision parity for Rapid-MLX. It is not currently qualified.

## 3. Evidence hierarchy

Use evidence in this order:

1. Current local source and tests.
2. Current source-build Rapid `serve --help`, exact binary identity, and captured receipts.
3. Local MLX `config.json`, safetensors index, tokenizer metadata, and generation metadata.
4. Bounded Hugging Face metadata fetches for the exact repo/revision.
5. `rapid-mlx info <model>` profile output, guarded by exact executable/version compatibility.
6. Model-family/name heuristics only as a degraded fallback.
7. Historical plan text only as context; it is not runtime truth.

The currently resolved ordinary executable reports:

```text
rapid-mlx 0.11.0
```

The benchmark corpus primarily qualifies the later source build:

```text
rapid-mlx 0.11.0+git.5fc6556c
```

Do not assume an older tagged release has the same live-KV behavior. Requalify the compact dtype matrix when the upstream KV fix is released as a tagged build.

## 4. Current validation baseline

The following checks passed during this audit:

```text
rtk cargo check
rtk cargo test --lib llama::vram_estimator::execution_policy
    29 passed
rtk npm run validate-js
rtk git diff --check
```

These checks establish that the half-wired tree currently compiles and its focused policy tests pass. They do **not** establish end-to-end UI/launch parity.

The audit also captured the live `rapid-mlx 0.11.0` `serve --help`. It confirms:

- `--reasoning` is a boolean switch and takes no `on`/`off` value;
- `--kv-cache-dtype` accepts `bf16`, `int8`, and `int4`;
- parser and hybrid flags listed in this document exist;
- temperature/top-p/top-k/min-p/repetition/presence/frequency server defaults exist;
- the generation-limit flag is `--max-tokens`, not `--default-max-tokens`;
- runtime text prefill defaults to 2048 unless Llama Monitor explicitly passes 512;
- verified Qwen aliases may default PFlash to `always`;
- Rapid's own cache auto-detection defaults to roughly 20% of RAM unless Llama Monitor passes an explicit cache budget.

## 5. Current architecture and important files

### 5.1 Persisted/runtime configuration

- `src/inference/rapid_mlx/mod.rs`
  - `RapidMlxConfig`
  - `RapidMlxAdapter`
  - `KvCacheConfig`
  - adapter-to-command mapping
- `src/presets/mod.rs`
  - top-level `ModelPreset`
  - backend-specific `rapid_mlx` configuration
- `src/inference/launch.rs`
  - preset-to-adapter construction

### 5.2 Runtime probing and model enrichment

- `src/inference/rapid_mlx/compatibility.rs`
  - exact `serve --help` capability flag snapshot
- `src/inference/rapid_mlx/capabilities.rs`
  - per-flag and per-field qualification, including `--default-*` sampling support
- `src/inference/rapid_mlx/info_query.rs`
  - version-guarded `rapid-mlx info <model>` text parsing
  - `ModelProfile`
- `src/inference/rapid_mlx/mlx_meta.rs`
  - bounded local/HF `config.json` parsing
  - normalized `ModelMemoryProfile`
- `src/web/api/rapid_mlx_runtime.rs`
  - `/api/rapid-mlx/models/<model>/profile`
  - active-preset flag advisor

### 5.3 Command construction

- `src/inference/rapid_mlx/command.rs`
  - `RapidMlxCommandBuilder`
  - capability-gated argv construction
- `src/inference/rapid_mlx/escape_hatch.rs`
  - allowlisted advanced flags, currently including `force-hybrid` and `no-hybrid`

### 5.4 Estimator

- `src/web/api/vram.rs`
  - canonical `/api/vram-estimate`
  - backend discriminator
  - local and HF metadata resolution
  - requested/effective Rapid policy serialization
- `src/llama/vram_estimator/execution_policy.rs`
  - requested/effective Rapid KV policy
- `src/llama/vram_estimator/estimate.rs`
  - KV geometry math
  - Rapid overhead approximation
  - active/retained/cache-cap accounting
- `src/llama/model_memory_profile.rs`
  - MLX profile to `ModelArch`
- `static/js/features/vram-estimate.js`
  - canonical frontend request builder

### 5.5 UI

- `static/index.html`
  - Spawn Wizard and Preset Editor markup
- `static/js/features/spawn-wizard.js`
  - wizard state, model selection, profile fetch, payload, estimator calls
- `static/js/features/presets.js`
  - editor load/save, profile fetch, estimator calls
- `static/js/features/setup-view.js`
  - welcome/preset-card estimator calls
- `static/css/spawn-wizard.css`
- `static/css/components.css`
- `tests/ui/capture.mjs`

### 5.6 Benchmark evidence

- `docs/reference/cache-benchmark-results.md`
- `docs/reference/model-runtime-benchmarking.md`
- `docs/plans/20260726-phase6_rapidmlx_cache_benchmarking.md`
- `scripts/rapid-mlx-benchmark-suite.mjs`
- `scripts/model-runtime-benchmark.mjs`
- `tests/fixtures/calibration/rapid-mlx-receipts/`

## 6. Executive status matrix

| Capability | Backend schema | Command argv | Estimator | Wizard | Preset Editor | Welcome cards | Status |
|---|---:|---:|---:|---:|---:|---:|---|
| BF16/INT8/INT4 requested KV | Yes | Yes | Yes | Partial | Partial | Via preset | Mostly wired |
| Reasoning forces effective INT8 | Yes | **Suspect argv** | Yes | Partial | Partial | Via preset | Must fix |
| Requested vs effective KV response | N/A | N/A | Yes | Not clearly rendered | Not clearly rendered | Not clearly rendered | Incomplete UX |
| MLX geometry introspection | Yes | N/A | Yes | Indirect | Indirect | Indirect | Strong base |
| Native context ceiling | Yes | N/A | Yes | Yes | Yes | Partial | Mostly wired |
| 8/16 GiB retained cache | Yes | Yes | **Accounting conflict** | Yes | Yes | Via preset | Must fix math |
| Tool-call parser | Yes | Yes | N/A | Hints only | No control | No control | Half wired |
| Reasoning parser | Yes | Yes | N/A | Hints only | No control | No control | Half wired |
| Auto tool choice | Yes | Yes, ungated | N/A | Not first-class | Not first-class | No | Incomplete |
| Force/no hybrid | Escape hatch only | Yes, ungated | Geometry unaffected | No control | No control | No | Incomplete |
| Rapid profile query | Yes | N/A | N/A | Fetch + hints | Fetch only | No | Partial |
| Local-repo parser inference | Geometry only | N/A | Geometry only | No recommendation merge | No recommendation merge | No | Missing |
| Sampling defaults | Capability model only | **Not emitted** | N/A | Generic fields exist | Generic fields exist | Via preset | Missing |
| Text prefill 512 | No structured field | **Not emitted** | UI commonly estimates 2048 | No Rapid control | No Rapid control | No | Critical missing |
| PFlash effective policy | Schema exists | Auto often omitted | Not modeled | Not first-class | Not first-class | No | Critical mismatch |
| Vision qualification | Partial metadata | `--mllm` tri-state | mmproj field exists | Partial | Partial | Partial | Not Phase 6 parity |

## 7. Active KV estimator: what is correct now

### 7.1 Requested/effective policy

`RapidMlxExecutionPolicy::new_with_eligibility` currently resolves:

```text
reasoning_mode = true  -> effective INT8
reasoning_mode = false -> explicit BF16 / INT8 / INT4
dtype omitted          -> effective INT4
```

This is the correct current policy for the source-build help contract. The focused policy suite covers requested/effective behavior and passes.

`/api/vram-estimate` accepts Rapid-native fields:

```json
{
  "backend": "rapid_mlx",
  "kv_cache_dtype": "bf16 | int8 | int4",
  "reasoning_mode": true
}
```

It returns:

```json
{
  "execution_policy": {
    "kv_cache_dtype": "int4",
    "reasoning_mode": true,
    "effective_kv_dtype": "int8"
  },
  "effective_kv_dtype": "int8"
}
```

The API maps effective Rapid dtypes into the shared estimator's byte-width labels:

```text
Rapid BF16 -> estimator f16   -> 2 bytes/element
Rapid INT8 -> estimator q8_0  -> 1 byte/element
Rapid INT4 -> estimator q4_0  -> 0.5 byte/element
```

This yields the expected ideal active-KV ratios:

```text
BF16 : INT8 : INT4 = 4 : 2 : 1
```

Fixed recurrent state, weights, and runtime overhead prevent total process memory from following that same ratio.

### 7.2 Geometry-aware KV math

The estimator does not use model parameter count as a KV multiplier. It reads actual model geometry and computes:

```text
KV bytes =
  effective KV layers
  × KV heads
  × head dimension
  × context tokens
  × slots
  × (K bytes/element + V bytes/element)
```

For Qwen 3.5/3.6 hybrid DeltaNet:

```text
effective KV layers = total layers / full_attention_interval
```

The recurrent/linear layers contribute fixed state separately and do not grow linearly with context.

For Gemma 4 local/global attention:

```text
global layers = full context × global KV geometry
local layers  = min(context, sliding window) × local KV geometry
```

Gemma 4's wider global head dimension is represented separately from local head dimension.

This is the correct shape of the active-KV model and directly addresses the earlier gross overestimation caused by treating all Qwen hybrid layers as full-attention layers.

### 7.3 Metadata path

The canonical estimator resolves Rapid models in this order:

1. local MLX directory;
2. MLX HF-style alias in `model_path`;
3. explicit `hf_repo_id` and revision.

Local estimates read:

- `config.json`;
- safetensors index/weight files;
- nested `text_config` for Qwen 3.6 and Gemma 4;
- full-attention interval;
- recurrent-state geometry;
- local/global attention geometry;
- model context limit.

HF estimates fetch the same bounded metadata before the full model is downloaded.

If substantive metadata is available, architecture evidence is marked approximate because Rapid runtime overhead remains formula-based. Missing geometry degrades to a heuristic and must be visibly labeled degraded.

### 7.4 Benchmark coverage

The current source-build corpus contains active-KV anchors for:

| Model family | Weight/model example | Dtypes | Context anchors |
|---|---|---|---|
| Qwen 3.5 9B hybrid | qualified MLX conversion | BF16, INT8, INT4 | 32K, 65K, 131K; INT8 also 160K/200K |
| Qwen 3.6 27B hybrid dense | Polaris2/Fable MLX | BF16, INT8, INT4 | through 131K BF16 and through 200K INT8/INT4 |
| Qwen 3.6 35B-A3B hybrid MoE | unsloth MLX | BF16 anchors plus INT8/INT4 | 32K through 200K depending lane |
| Gemma 4 26B-A4B | LM and VLM MLX conversions | BF16, INT8, INT4 | through 131K BF16 and through 200K INT8/INT4 |

The exact receipt directories and evidence boundary are documented in `docs/reference/cache-benchmark-results.md`.

### 7.5 What remains unproven

The active-KV formula is structurally correct and dtype selection is correctly implemented in the API, but Phase 6 still needs:

- endpoint tests over all standard contexts and all three dtypes;
- model-family fixtures for each of the four architecture tracks;
- assertions that reasoning changes BF16/INT4 requests to effective INT8;
- comparison of estimator component slopes against receipt-derived active-memory slopes;
- explicit tolerances and residual reporting;
- a tagged-release requalification after the upstream KV fix ships.

Do not assert that the **total** estimate is benchmark-calibrated merely because active KV is. Rapid base/working overhead remains approximate.

## 8. Critical estimator defect: retained-cache accounting

### 8.1 Current implementation

The frontend sends:

```text
retained_cache_mib = 8192 by default when prefix cache is enabled
```

The API converts that directly to:

```text
mlx_prefix_cache_bytes = retained_cache_mib × 1024 × 1024
prefix_cache_budget_bytes = same value
```

`full_estimate` then adds `mlx_prefix_cache_bytes` to total memory.

Separately, when `rapid_planning_context_tokens > 0`, `full_estimate` computes:

```text
active_kv_bytes   = KV bytes for rapid_planning_context_tokens
retained_kv_bytes = KV bytes for rapid_retained_cache_tokens
```

It also adds `retained_kv_bytes` to total memory.

Workload scenarios can populate `rapid_retained_cache_tokens` even when the caller did not explicitly send it.

### 8.2 Why this is wrong

`--cache-memory-mb` is a capacity ceiling for retained in-memory cache entries. It is not an additional allocation to add on top of a second token-derived retained cache reservation representing the same retained state.

The current total can therefore include both:

```text
full retained cache capacity (for example 8 GiB)
+
token-derived retained KV bytes
```

That is double reservation whenever both paths are nonzero. The existing unit test named `active_and_retained_totals_are_distinct_no_double_counting` proves only that `kv_cache_bytes = active + retained`; it does not test the separate `mlx_prefix_cache_bytes` addition and therefore does not catch this product-level double count.

### 8.3 Required model

Choose one canonical accounting contract and make the API/UI names match it.

Recommended contract:

```text
mandatory memory =
    weights
  + active KV at selected context/effective dtype
  + fixed recurrent state
  + mmproj/MTP where applicable
  + runtime/working overhead

optional retained-cache reservation =
  prefix_cache_enabled
    ? configured --cache-memory-mb ceiling
    : 0

total admission estimate =
    mandatory memory
  + optional retained-cache reservation
  + system reserve
```

Under this contract:

- do not also add token-derived retained KV to admission total;
- token-derived retained size may remain as an informational capacity calculation:

  ```text
  estimated bytes for one retained prefix
  estimated prefixes/branches fitting in selected cap
  cap utilization for one 131K/160K/200K branch
  ```

- if the runtime allocates cache lazily, show both:

  ```text
  current/one-branch expected use
  configured worst-case cap
  ```

  Admission and "will this preset remain safe at full configured use?" should use the cap.

Alternative contract, only if source/runtime evidence proves the cap is not a reservation and product policy wants expected-use estimates:

```text
optional retained use = min(configured cap, predicted retained entries)
```

If this is chosen, the UI must separately expose the configured maximum and expected use. Never sum the cap and the expected use.

### 8.4 Benchmark-calibrated retained-cache policy

The measured Qwen 3.6 35B-A3B source-build lanes establish:

| Context / active KV | 8 GiB | 16 GiB | Product conclusion |
|---|---|---|---|
| 160K INT8 | newest fork remained fast; evictions occurred | no evictions | 16 GiB retains older branches |
| 160K INT4 | no material deficit | no material speed gain | 8 GiB sufficient |
| 200K INT8 | newest fork remained fast; more evictions | no evictions | 16 GiB retains older branches |
| 200K INT4 | newest fork remained fast despite evictions | no material speed gain | 8 GiB baseline |

The estimator should therefore:

- recommend 8 GiB when mandatory memory plus 8 GiB plus reserve fits;
- offer 16 GiB when the user has headroom and wants older branch retention;
- offer Off for memory-starved configurations;
- never label 16 GiB a performance mode;
- avoid hard-coding 8 GiB as mandatory;
- show the direct headroom delta of Off/8/16 GiB.

Disk checkpoints must remain off by default. The measured 200K INT4 run wrote 16.78 GB, loaded nothing, increased cold TTFT by 56.5 seconds, and did not improve the fork. They are snapshots/manual warm-start machinery, not a lower cache tier.

## 9. Critical launch defect: reasoning flag semantics

The config stores:

```text
reasoning_mode: Option<String>  // auto/on/off
```

The command builder currently does:

```text
if mode != "auto":
    emit --reasoning <mode>
```

This must be checked against the exact current `serve --help`. The user-provided help text presents `--reasoning` as a boolean profile switch. If that is the actual parser contract, `--reasoning on` is wrong.

Required behavior should be:

```text
Auto -> omit --reasoning
On   -> emit --reasoning
Off  -> omit --reasoning, unless the exact build exposes a real negative flag
```

Do not invent `--reasoning off`.

The live 0.11.0 help captured during this audit confirms this defect: `--reasoning` is a no-value boolean flag. The current `--reasoning on` command construction must be fixed before launch qualification.

There are three distinct concepts that the UI and code must not conflate:

| Concept | Layer | Meaning |
|---|---|---|
| `enable_thinking` | request default | Default field added to requests when client omits it |
| `reasoning_effort` | request default | Default reasoning effort added to requests |
| `--reasoning` | server launch profile | Pins active KV to INT8 and activates Rapid's reasoning profile |

For known reasoning-capable Qwen/Gemma profiles, the UI should recommend server Reasoning On and request Thinking On, while still displaying that these are separate settings.

When server Reasoning is On:

- generated argv must include exactly the supported boolean flag;
- estimator `reasoning_mode` must be true;
- requested KV may remain visible;
- effective KV must display INT8;
- KV dropdown should either be disabled with an explanation or retain the requested value while showing an INT8 effective badge;
- summaries and welcome cards must use effective INT8 memory.

## 10. Parsers and hybrid mode: current state

### 10.1 Backend schema and argv

`RapidMlxConfig` and `RapidMlxAdapter` already contain:

```text
tool_call_parser: Option<String>
reasoning_parser: Option<String>
auto_tool_choice: bool
```

`RapidMlxCommandBuilder` emits:

```text
--tool-call-parser <value>
--reasoning-parser <value>
--enable-auto-tool-choice
```

The first two are capability-gated. `--enable-auto-tool-choice` and `--no-thinking` are currently emitted without a capability check and should be made consistent with the exact runtime snapshot.

`force-hybrid` and `no-hybrid` exist only as allowlisted `escape_hatch_flags`. They are not a structured mutually exclusive setting.

### 10.2 Model profile

`ModelProfile` already parses:

```text
tool_format
reasoning_parser
architecture
is_finetune
```

The authenticated endpoint exists:

```text
GET /api/rapid-mlx/models/<model>/profile
```

The Spawn Wizard fetches it and renders informational hints such as:

```text
Tool: qwen3_coder_xml · Reasoning: qwen3
```

The Preset Editor fetches it into a module variable but does not render or apply it.

The active-session flag advisor warns when a model profile declares a tool format but the active preset lacks an explicit parser/auto-tool-choice flag. This logic conflicts with the desired Auto contract: a correct Auto setting should not be warned merely because it intentionally relies on the Rapid alias profile.

### 10.3 Missing UI

There are no first-class Wizard or Preset Editor dropdowns for:

- tool-call parser;
- reasoning parser;
- hybrid override.

The current profile hints do not populate configuration, do not distinguish detected recommendation from effective launch behavior, and do not allow explicit override.

The Wizard currently initializes `reasoningMode` to `null`, and the Preset Editor reduces the setting to a checkbox. This does not meet the required reasoning-first Auto/On/Off UX, does not automatically recommend On for a detected reasoning-capable profile, and cannot explain why effective KV changed to INT8.

### 10.4 Required structured representation

Keep `None` as Auto for parser fields:

```text
tool_call_parser = None       -> Rapid alias/profile Auto; emit nothing
tool_call_parser = Some(x)    -> explicit override; emit flag

reasoning_parser = None       -> Rapid alias/profile Auto; emit nothing
reasoning_parser = Some(x)    -> explicit override; emit flag
```

Add a typed hybrid override:

```rust
enum RapidMlxHybridMode {
    Auto,
    Force,
    Disable,
}
```

Serialize it as `auto | force | disable`.

Command mapping:

```text
Auto    -> emit neither flag
Force   -> --force-hybrid
Disable -> --no-hybrid
```

Validate mutual exclusion in the typed path. Remove these two flags from ordinary UI dependence on the opaque escape-hatch list. Compatibility parsing may still diagnose old/manual entries, but the branch does not require a migration layer for upstream users because Rapid capabilities have not shipped upstream.

### 10.5 Parser options

Build dropdown options from the current exact runtime capability/help snapshot. At the time of this audit, the known canonical set includes:

Tool-call parser:

```text
auto
mistral
qwen
qwen3
qwen3_xml
qwen3_coder
qwen3_coder_xml
llama
llama3
llama4
hermes
nous
deepseek
deepseek_v3
deepseek_v31
kimi
moonshot
kimi_k2
granite
granite3
nemotron
nemotron3
xlam
functionary
meetkai
glm47
glm4
minimax
minimax_m2
harmony
gpt_oss
gpt-oss
gemma4
gemma_4
seed_oss
seed
```

Reasoning parser:

```text
auto
gemma4
qwen3
hy_v3
hy3
deepseek_r1
vibethinker
glm4
gpt_oss
harmony
minimax
ui_tars
```

Do not freeze this list forever in HTML. Recommended implementation:

1. probe exact `serve --help`;
2. normalize parser choices into the capability snapshot/API;
3. return the supported values to the UI;
4. use a conservative built-in list only as a fallback for a known verified baseline;
5. do not show unsupported explicit options for the active executable.

### 10.6 Auto/recommendation rules

For each field, show:

```text
Selected: Auto (Rapid alias profile)
Detected: qwen3_coder_xml
Effective source: Rapid profile
Override: [dropdown]
```

Precedence:

1. user explicit override;
2. concrete conflict recommendation from exact repo metadata plus exact Rapid profile;
3. Rapid alias profile Auto;
4. unknown.

Do not silently force a parser or hybrid override solely from a model name.

Suggested family defaults are recommendations, not unconditional emitted flags:

| Family/profile | Tool parser recommendation | Reasoning parser recommendation | Hybrid |
|---|---|---|---|
| Qwen 3.5/3.6 general | Rapid profile, commonly Qwen XML variant | `qwen3` | Auto; geometry should detect hybrid |
| Qwen coder finetune | profile may recommend `qwen3_coder_xml` | `qwen3` | Auto |
| Gemma 4 | profile may recommend `gemma4` | `gemma4` | Auto; local/global architecture is not Qwen DeltaNet |
| Unknown finetune/distillation | Auto plus visible uncertainty | Auto plus visible uncertainty | Auto unless a concrete conflict is detected |

## 11. Required repo introspection and recommendation merge

### 11.1 Existing local/HF introspection

`mlx_meta.rs` already handles the geometry needed for memory:

- architecture/model type;
- nested text config;
- layer/head dimensions;
- MoE geometry;
- full-attention interval;
- sliding window and global/local heads;
- context limit;
- recurrent state;
- embedded MTP metadata.

### 11.2 Missing behavioral introspection

Add bounded parsing/fetching for:

- `tokenizer_config.json`
  - embedded `chat_template`;
  - tokenizer class/model type;
  - tool-template clues;
- `generation_config.json`
  - temperature/top-p/top-k/min-p and generation defaults where present;
  - EOS/BOS token behavior;
- `config.json`
  - `model_type`;
  - `architectures`;
  - hybrid geometry/conflicts;
  - vision tower/multimodal markers;
- safetensors index
  - embedded MTP/vision component presence where names provide defensible evidence;
- repository identity and exact revision.

Do not execute remote code while introspecting. Use size caps, timeouts, path validation, and authenticated HF access already present in the codebase.

### 11.3 Unified recommendation response

Create or extend an authenticated endpoint that returns a merged recommendation object:

```json
{
  "model_identity": {
    "repo_id": "...",
    "revision": "...",
    "local_path": "..."
  },
  "memory_profile": {
    "architecture": "...",
    "hybrid": true,
    "native_context_limit": 262144
  },
  "rapid_profile": {
    "tool_call_parser": "qwen3_coder_xml",
    "reasoning_parser": "qwen3"
  },
  "recommendations": {
    "tool_call_parser": {
      "selection": "auto",
      "detected": "qwen3_coder_xml",
      "confidence": "profile",
      "reason": "Rapid alias profile"
    },
    "reasoning_parser": {
      "selection": "auto",
      "detected": "qwen3",
      "confidence": "profile"
    },
    "hybrid_mode": {
      "selection": "auto",
      "detected": true,
      "confidence": "config_json"
    },
    "reasoning_mode": {
      "selection": "on",
      "confidence": "family_profile"
    }
  },
  "conflicts": [],
  "warnings": []
}
```

Keep raw evidence and recommendation policy separate. A changed policy must not require re-parsing raw metadata.

### 11.4 Conflict handling

Examples that should produce a visible conflict:

- Rapid profile says non-hybrid but `config.json` has a valid full-attention interval plus recurrent layers.
- Profile recommends generic Qwen parser while tokenizer template clearly uses coder XML markers.
- Repo metadata says Gemma but alias profile resolves Qwen parser.
- Model is marked a finetune and profile confidence is lower than exact local metadata.

On conflict:

- leave selection Auto unless launch would be known-broken;
- show both evidence sources;
- recommend a specific override;
- require the user to accept it or make the override clearly reversible;
- persist the explicit override and its source/reason if the schema supports provenance.

## 12. Sampling parameters: current state and required work

### 12.1 Current state

Top-level presets already store generic sampling fields such as:

```text
temperature
top_p
top_k
min_p
repeat_penalty
presence_penalty
max_tokens
```

The llama.cpp launch path maps these to llama-server flags.

The Rapid capability probe already independently detects:

```text
--default-temperature
--default-top-p
--default-top-k
--default-min-p
--default-typical-p
--default-repetition-penalty
--default-presence-penalty
--default-frequency-penalty
--default-max-tokens
```

However, `RapidMlxConfig`/adapter/command builder do not currently carry and emit these values. `sampling_mode` is explicitly treated as persisted metadata and emits no argv.

Therefore the answer to "are temp/top-p/etc. mapped to Rapid `--default-*` flags?" is currently **no**.

### 12.2 Required mapping

Add typed optional fields to the Rapid config/adapter/command path, or intentionally reuse the top-level preset fields during `request_from_preset`. The latter reduces duplication but must keep backend serialization unambiguous.

Map:

| Preset field | Rapid launch flag |
|---|---|
| `temperature` | `--default-temperature` |
| `top_p` | `--default-top-p` |
| `top_k` | `--default-top-k` |
| `min_p` | `--default-min-p` |
| `repeat_penalty` | `--default-repetition-penalty` |
| `presence_penalty` | `--default-presence-penalty` |
| future frequency penalty | `--default-frequency-penalty` |
| `max_tokens` | `--default-max-tokens` |

Only emit each field when:

- the user/preset set it explicitly; and
- the exact executable capability snapshot reports that field supported.

If a field is configured but unsupported:

- do not silently discard it;
- return a validation/advisory result that says the server default cannot be applied;
- explain that explicit client request values may still work if the OpenAI endpoint supports them.

Server defaults and per-request sampling are separate:

- server `--default-*` applies when a client omits a field;
- explicit OpenCode/client request values override server defaults;
- Llama Monitor's request mapper must continue forwarding supported explicit fields.

### 12.3 Sampling profiles

`sampling_mode` may remain a UI convenience, but it must resolve to visible concrete fields before save/launch:

```text
Auto     -> model/generation config or no explicit defaults
Coding   -> concrete temperature/top-p/etc., displayed to user
Precise  -> concrete fields
Creative -> concrete fields
Custom   -> user fields
```

Do not invent opaque `--sampling-mode`; the current command builder correctly avoids doing so.

For reasoning models, avoid a hidden low token/reasoning budget. Smoke-test output limits must not leak into normal presets or benchmark lanes.

The current live build does not expose `--default-max-tokens`. It exposes:

```text
--max-tokens MAX_TOKENS
    Default max tokens for generation
```

Map the preset's server-default generation limit to `--max-tokens` after confirming that this field is intended as a launch default. Correct `SamplingDefaultFields`, its tests, and any capability cascade that currently probes the nonexistent `--default-max-tokens`.

## 13. Critical launch/estimator gaps beyond dtype and cache

### 13.1 Text prefill is not wired

The benchmark corpus and product decision use:

```text
--prefill-step-size 512
```

The live Rapid default is 2048. The application currently has `prefill_batch_size`, but that is a different flag and is not a substitute for `--prefill-step-size`.

There is no structured `prefill_step_size` in `RapidMlxConfig`, no command emission, and no first-class Wizard/Editor control. The estimator's `ubatch_size` frequently defaults to 2048, so the estimated Rapid overhead can also diverge from the actual intended 512 launch.

Required fix:

- add `prefill_step_size`, default 512 for text;
- emit `--prefill-step-size 512`;
- use the same value as the Rapid estimator's prefill/working-width input;
- label `prefill_batch_size` separately so the two cannot be confused;
- for vision, preserve the qualified ladder 512 -> 1024 -> 1536 -> 2048 and never allow more than 2048;
- add command, estimator-body, Wizard, Editor, and round-trip tests.

### 13.2 PFlash can silently change the workload

The live 0.11.0 help says verified Qwen 3.5/3.6 aliases may default:

```text
--pflash always
```

at long prompt lengths. The benchmark calibration lanes intentionally used PFlash off when measuring full-context PP/TG/RAM. Current config has `pflash_policy`, but Auto generally emits no flag and therefore accepts the alias default. The estimator assumes the selected full active context; it does not model PFlash's retained-token ratio or its quality/tool-call implications.

This creates two distinct risks:

1. launch behavior does not match the full-context benchmark evidence;
2. displayed full-context memory/performance can describe a workload the runtime compresses before prefill.

Recommended Phase 6 policy:

- default managed Llama Monitor presets to explicit `--pflash off` until PFlash is separately qualified for long coding-agent prompts, reasoning, tool definitions, and recall;
- show Auto as "Rapid alias default; may enable compression";
- do not reduce the VRAM estimate for PFlash during Phase 6;
- if Auto/Always is selected, display a clear warning that effective prompt retention and benchmark comparability change;
- defer PFlash-specific memory/performance/retrieval math to a measured follow-up.

If the product instead keeps Auto, it must prominently disclose the effective alias policy and cannot describe the existing PFlash-off receipts as direct launch calibration.

### 13.3 Runtime cache defaults must always be overridden intentionally

Live Rapid 0.11.0 auto-detects approximately 20% of RAM for retained cache when no explicit MiB value is passed. Therefore:

- enabled managed presets must pass the selected 8192 or 16384 MiB explicitly;
- Off must pass `--disable-prefix-cache`, not merely omit `--cache-memory-mb`;
- command preview and estimator must show the same explicit policy;
- omission should mean deliberate unmanaged/runtime Auto only if that option is exposed and labeled separately.

## 14. Spawn Wizard completion requirements

Add a coherent Rapid model-behavior group, not scattered expert controls.

Required controls:

1. Server Reasoning Profile:

   ```text
   Auto
   On — recommended for detected reasoning model
   Off
   ```

2. Request Thinking Default:

   ```text
   Client decides
   On
   Off
   ```

3. Tool-call parser:

   ```text
   Auto — Rapid alias profile
   <runtime-supported choices>
   ```

4. Reasoning parser:

   ```text
   Auto — Rapid alias profile
   <runtime-supported choices>
   ```

5. Hybrid handling:

   ```text
   Auto — detected from model/profile
   Force hybrid
   Disable hybrid
   ```

6. Effective KV:

   ```text
   Requested: INT4
   Effective: INT8 because Reasoning Profile is On
   ```

7. Retained prefix cache:

   ```text
   Off
   8 GiB — recommended
   16 GiB — retain more branches
   ```

8. Sampling defaults with explicit server/client semantics.

Profile fetch must:

- run after stable model selection;
- merge Rapid `info` with local/HF metadata;
- not overwrite an explicit user choice;
- refresh detected/effective labels;
- trigger a canonical estimator refresh when reasoning/effective KV changes;
- degrade without blocking if enrichment fails.

Payload must use structured Rapid fields, not translate ordinary controls into opaque escape-hatch pairs.

## 15. Preset Editor completion requirements

The Preset Editor must round-trip every Wizard field:

- load from `rapid_mlx`;
- show Auto vs explicit override;
- show live detected recommendation;
- preserve explicit overrides after async profile fetch;
- save structured parser/hybrid/reasoning fields;
- include sampling default fields;
- update command preview;
- update estimator using the same canonical request builder;
- display requested/effective KV and retained-cache memory separately.

The existing editor fetches the Rapid profile but does not render it. Finish that path rather than adding a separate fetch implementation.

Do not rely on "spread existing fields" as proof of support. Every first-class field needs:

- an HTML control;
- load code;
- change/update code;
- save code;
- schema field;
- command mapping;
- estimator mapping if memory-affecting;
- tests.

## 16. Welcome-screen preset cards

Welcome cards do not need every advanced dropdown, but they must truthfully summarize the saved/effective preset:

```text
Rapid-MLX
Qwen 3.6 35B-A3B
200K context
Reasoning On · effective INT8 KV
8 GiB retained cache
Tool/Reasoning parsers: Auto (qwen3_coder_xml / qwen3)
Estimated unified memory: X
Headroom: Y
```

They must use `/api/vram-estimate` through `rapidEstimatePolicyFromConfig`, not local math.

If the profile is not available during card rendering:

- display saved explicit values;
- label omitted values Auto;
- do not fabricate detected parser names.

## 17. Context choices and native limits

Current standard choices are correctly present in Wizard/Editor code:

```text
32768
65536
131072
163840
200000
262144
```

The canonical API returns:

```text
native_context_limit
context_extension_required
```

Keep this behavior:

- at or below metadata ceiling: standard/native;
- above metadata ceiling: advanced/unqualified;
- do not emit fake `--rope-scaling yarn`;
- do not call 262K supported if the model metadata says 131K.

Later RoPE/YaRN mapping remains out of Phase 6 scope, but the follow-up must remain in the execution plan.

Add estimator tests at every standard context for each active KV dtype. For models with a lower native ceiling, the math may still be tested as an extrapolation, but the API/UI must mark it `context_extension_required`.

## 18. Tests required before Phase 6 closure

### 18.1 Rust policy tests

- BF16, no reasoning -> effective BF16.
- INT8, no reasoning -> effective INT8.
- INT4, no reasoning -> effective INT4.
- omitted, no reasoning -> effective INT4.
- BF16 + reasoning -> effective INT8 with reason.
- INT4 + reasoning -> effective INT8 with reason.
- INT8 + reasoning -> effective INT8 without a misleading change reason.

### 18.2 Command tests

Using exact capability fixtures:

- explicit BF16/INT8/INT4 emit correct flag/value;
- Auto omits dtype;
- Reasoning On emits the exact supported boolean argv;
- Reasoning Auto/Off do not emit invalid values;
- tool parser Auto omits;
- explicit tool parser emits and capability-gates;
- reasoning parser Auto omits;
- explicit reasoning parser emits and capability-gates;
- hybrid Auto emits neither flag;
- Force emits only `--force-hybrid`;
- Disable emits only `--no-hybrid`;
- mutual exclusion is impossible/rejected;
- every configured `--default-*` field emits only when supported;
- unsupported configured sampling default returns an actionable error;
- cache Off omits `--cache-memory-mb`;
- 8/16 GiB emit 8192/16384;
- disk checkpoint interval remains `0`;
- generated command never includes unsupported placeholder policy flags.
- text launch emits `--prefill-step-size 512`;
- Rapid prefill batch size and prefill step size remain distinct;
- managed default PFlash policy is explicit and matches the documented benchmark posture.

### 18.3 Estimator unit/API tests

For each model architecture:

- Qwen 3.5 9B hybrid;
- Qwen 3.6 27B hybrid dense;
- Qwen 3.6 35B-A3B hybrid MoE;
- Gemma 4 26B-A4B global/local.

For each standard context:

```text
32K, 65K, 131K, 160K, 200K, 262K
```

Assert:

- active KV grows monotonically;
- BF16/INT8/INT4 active-KV ratios follow element widths within rounding;
- hybrid Qwen uses full-attention layers only;
- Gemma local layers saturate at sliding window;
- reasoning uses INT8 active KV;
- 8 GiB cache changes total/headroom by exactly one 8 GiB reservation under the chosen accounting contract;
- no cap-plus-retained double count;
- cache Off contributes zero;
- native context limit is returned and extension flag is correct;
- local and HF-introspection paths yield equivalent architecture fields for the same revision.
- estimator prefill width equals the launched Rapid prefill step size;
- PFlash does not silently reduce memory unless a future measured model explicitly implements it.

Add receipt-slope calibration tests or fixtures that compare predicted active-KV deltas, not whole-process peaks, because overhead and allocator behavior are separate components.

### 18.4 API/security tests

- profile/recommendation endpoints require API token;
- invalid JSON is 400;
- model identifiers reject traversal/absolute paths where appropriate;
- bounded remote metadata fetches have timeout and size limits;
- new fields use `#[serde(default)]`;
- no secrets are serialized or logged.

Run `tests/auth_routing.rs` when endpoint contracts change.

### 18.5 JavaScript tests

Add direct tests for:

- config -> estimator reasoning boolean;
- Wizard reasoning On -> effective INT8 display and request;
- parser Auto does not become explicit after profile fetch;
- explicit parser survives profile refresh;
- hybrid choices round-trip;
- Wizard payload equals Editor save representation;
- saved preset card estimate body equals Editor estimate body;
- cache Off/8/16 mapping;
- native ceiling badges.

### 18.6 UI E2E and screenshots

Add a Rapid-specific capture path if the existing generic scenario does not select Rapid.

Required visual states:

1. Spawn Wizard with a Qwen profile detected.
2. Spawn Wizard showing requested INT4 but effective INT8 because reasoning is On.
3. Spawn Wizard parser/hybrid Auto plus detected values.
4. Preset Editor with the same saved settings.
5. Welcome card summary for the same preset.
6. Light theme.
7. Narrow/responsive layout.

Per `AGENTS.md`:

```bash
rtk cargo build --release
rtk proxy node tests/ui/capture.mjs --scenario <rapid-specific-scenario>
```

Inspect real artifacts under `docs/screenshots/artifacts/`. Do not commit them unless promoted into referenced documentation.

## 19. Recommended implementation order

### Stage A — Freeze runtime truth

1. Capture exact current:

   ```text
   rapid-mlx --version
   rapid-mlx serve --help
   rapid-mlx info <qualified Qwen>
   rapid-mlx info <qualified Gemma>
   ```

2. Store exact executable identity/help fixture for tests.
3. Confirm boolean/value semantics for `--reasoning`, parser flags, hybrid flags, and every `--default-*`.
4. Correct stale comments that still describe pre-fix BF16-only active KV.
5. Record the live 0.11.0 prefill, PFlash, cache-auto, and `--max-tokens` semantics.

Exit gate: the capability fixture matches the source-build binary used for the benchmark corpus.

### Stage B — Repair backend schema and argv

1. Add typed hybrid mode.
2. Fix reasoning boolean emission.
3. Capability-gate all parser/thinking/tool-choice flags.
4. Wire explicit sampling fields to supported `--default-*`.
5. Wire text `--prefill-step-size 512`.
6. Choose and emit an explicit managed PFlash policy.
7. Add command tests.

Exit gate: a saved Rapid config deterministically produces valid argv, with Auto omissions.

### Stage C — Repair estimator accounting

1. Adopt one retained-cache cap/use contract.
2. Remove cap-plus-token double reservation.
3. Preserve active KV as effective-dtype geometry.
4. Add informational branch-capacity math if useful.
5. Add all dtype/context/model tests.
6. Calibrate active slopes against receipts and report residuals.

Exit gate: same config yields the same total/components on all UI surfaces, and cache delta is exact.

### Stage D — Merge behavioral introspection

1. Extend bounded repo metadata parsing.
2. Normalize exact Rapid profile output.
3. Build one evidence-bearing recommendation response.
4. Add conflict detection and tests.

Exit gate: known Qwen/Gemma repos produce detected parser/reasoning/hybrid hints without name-only guessing.

### Stage E — Complete Wizard

1. Add first-class controls.
2. Preserve explicit overrides.
3. Render detected/effective values.
4. Wire canonical estimator and payload.
5. Add JS tests.

Exit gate: Wizard summary, payload, command preview, and estimate agree.

### Stage F — Complete Preset Editor and cards

1. Mirror Wizard controls.
2. Round-trip all fields.
3. Render profile hints/conflicts.
4. Update welcome cards.
5. Add JS/E2E tests.

Exit gate: Wizard-created preset reopens unchanged and all three surfaces show the same effective policy.

### Stage G — Documentation and qualification

Update implemented/current-state reference docs:

- `docs/reference/rapid-mlx-runtime.md`
- `docs/reference/vram-estimator.md`
- `docs/reference/spawn-wizard.md`
- `docs/reference/cache-benchmark-results.md`
- `docs/reference/model-runtime-benchmarking.md`

Archive completed plan material under `docs/archive/rapid-mlx/` only after it is rewritten as historical/implemented state. Do not leave implementation plans masquerading as user reference docs.

Exit gate: docs state reasoning's INT8 override, cache accounting, parser/hybrid Auto semantics, sampling mapping, evidence boundary, and current vision limitation.

## 20. Phase 6 definition of done

Phase 6 is complete only when all statements below are true.

- [ ] Exact current Rapid CLI capability semantics are captured and tested.
- [ ] BF16/INT8/INT4 requested active KV is honored without reasoning.
- [ ] Reasoning always produces effective INT8 in launch, estimator, summaries, and cards.
- [ ] Requested and effective dtype are both visible.
- [ ] Qwen hybrid and Gemma local/global geometry are used from metadata.
- [ ] All six standard contexts are tested for all three dtypes across architecture fixtures.
- [ ] Retained-cache accounting cannot double count cap and retained tokens.
- [ ] Off/8/16 GiB cache choices produce exact, explainable memory deltas.
- [ ] Disk checkpoints remain off/rejected as a normal cache tier recommendation.
- [ ] Tool-call and reasoning parser Auto/override controls exist in Wizard and Editor.
- [ ] Hybrid Auto/Force/Disable exists as a typed mutually exclusive setting.
- [ ] Exact-runtime supported parser choices are exposed.
- [ ] Local/HF behavioral metadata and Rapid profile evidence are merged.
- [ ] Explicit overrides survive async enrichment.
- [ ] Sampling fields map to supported Rapid `--default-*` flags.
- [ ] `max_tokens` maps to the live build's real `--max-tokens` flag.
- [ ] Unsupported sampling defaults are reported, not silently dropped.
- [ ] Text prefill is explicitly 512 in config, argv, estimator, Wizard, and Editor.
- [ ] PFlash policy is explicit and benchmark evidence is not misrepresented.
- [ ] Wizard, Editor, launch argv, estimator, and cards share one config contract.
- [ ] Rapid-specific JS, Rust, API, and E2E tests pass.
- [ ] Real screenshots are captured and visually reviewed.
- [ ] Reference docs describe implemented behavior and evidence boundaries.
- [ ] Mandatory pre-PR checks pass in exact project order.

## 21. Mandatory final validation

Run in this exact order and fix every failure:

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

Then run isolated UI E2E:

```bash
rtk sh -c 'cd tests/ui && CI=1 LLAMA_MONITOR_USE_RELEASE=1 LLAMA_MONITOR_TEST_PORT=17778 npm test'
```

Use at least a ten-minute timeout. Do not run default `npm test` from the repository root because it may kill the active model on port 7778.

Run the screenshot harness sequentially, never in parallel.

If `cargo build` rewrites generated static assets, run `cargo fmt`, inspect the diff, and commit the proper generated changes. Do not treat normal generated-asset rewrites as unrelated dirt.

## 22. Known non-goals and later phases

The following do not block Phase 6:

- MTPLX implementation;
- MTP/speculative decoding qualification (Phase 6.5);
- RoPE/YaRN context extension controls and math;
- GGUF-to-MLX lossy conversion;
- resolving Rapid modern-Qwen vision incompatibility;
- automatic disk-backed paging of evicted Rapid prefix entries.

However:

- Phase 6 must not make schema/UI decisions that prevent another MLX loader later.
- Phase 6.5 must consume the corrected mandatory-memory and retained-cache accounting because MTP reduces available cache headroom.
- Modern-Qwen vision limitation must remain explicit in UI/docs until a later qualification proves otherwise.

## 23. Immediate first actions for the next agent

1. Read this document completely.
2. Read:

   - `docs/reference/cache-benchmark-results.md`;
   - `docs/reference/vram-estimator.md`;
   - `docs/reference/rapid-mlx-runtime.md`;
   - `docs/plans/20260718-final_rapidmlx_followups_execution.md`.

3. Inspect the dirty worktree and preserve every existing change/receipt.
4. Re-run the exact source-build help/version capture.
5. Start at Stage B with the reasoning argv semantics and typed hybrid mode.
6. Immediately follow with Stage C retained-cache accounting; do not build UI on the current ambiguous total.
7. Carry one Qwen preset through backend -> estimator -> Wizard -> Editor -> welcome card as the vertical integration fixture.
8. Add Gemma and the other Qwen architecture fixtures after that vertical slice works.
9. Do not declare Phase 6 complete at compile success. Use the definition-of-done checklist and real screenshots.

## 24. Bottom-line answers to the initiating questions

### Does the estimator do Rapid BF16/INT8/INT4 correctly?

**Active KV: mostly yes.** The current canonical API uses requested/effective Rapid policy, honors BF16/INT8/INT4 when reasoning is off, forces effective INT8 when reasoning is on, and applies architecture-aware Qwen/Gemma geometry.

**Whole-process total: not fully qualified.** Rapid overhead is still approximate, the model-family receipt slopes need formal assertion coverage, and retained-cache accounting can double reserve memory.

### Does it calculate `--cache-memory-mb` correctly?

**Not reliably when a workload scenario/token-derived retained cache is also present.** The explicit MiB cap is correctly converted to bytes and added, but a second token-derived retained KV component can also be added. The contract must be repaired so cap and expected retained use are not summed.

### Are sampling parameters mapped to Rapid server defaults?

**No.** Capability probing for `--default-temperature`, `--default-top-p`, and related fields exists, but config/adapter/command mapping is absent. `sampling_mode` currently emits nothing by design.

### Are parser and hybrid controls wired?

**Backend parser fields are half wired; UI is not.** Tool and reasoning parser schema/argv paths exist. Wizard only shows profile hints. Preset Editor fetches a profile but does not use it. Hybrid overrides exist only as opaque allowlisted escape-hatch flags, not a typed Auto/Force/Disable control.

### Is model introspection sufficient?

**Memory geometry is strong; behavioral recommendation merging is incomplete.** Local/HF `config.json` and weight metadata support the estimator. `rapid-mlx info` supplies profile hints. The code still needs a unified evidence-bearing merge with tokenizer/generation metadata, conflict handling, and UI persistence.

### Can Phase 6 be called complete now?

**No.** The branch contains the correct foundation and valuable benchmark evidence, but the defects and missing cross-surface wiring identified in this handoff are Phase 6 blockers.
