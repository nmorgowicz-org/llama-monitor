# Rapid-MLX benchmark continuation handoff

## Purpose and authority

This is an operational handoff for a fresh agent continuing the **on-device** Rapid-MLX benchmark campaign on the user's Apple M5 Max with 64 GiB unified memory. It records measured evidence, the exact controls still required, and how to run them safely. Read it together with:

- [`model-runtime-benchmarking.md`](../reference/model-runtime-benchmarking.md) for the reusable runner contract.
- [`20260718-final_rapidmlx_followups.md`](./20260718-final_rapidmlx_followups.md) for product requirements and future loader abstraction.
- [`20260718-final_rapidmlx_followups_execution.md`](./20260718-final_rapidmlx_followups_execution.md) for phase routing; this benchmark campaign is evidence gathering, not permission to silently change product behavior.

Do not discard or reset the dirty worktree. Do not commit or push without explicit user approval. Benchmark/model commands need to run outside the sandbox. Documentation and runner work may remain in the workspace sandbox.

## One-paragraph status

The reusable OpenAI-compatible benchmark runner and profiles are implemented but uncommitted. It is designed to compare Rapid-MLX with llama.cpp, OMLX, or another future OpenAI-compatible runtime. The first Qwen 3.6 35B PFlash evidence is complete enough to establish capacity and a source-retrieval warning: PFlash with a 20% retain ratio fits 131,948 raw tokens on this machine, but it failed exact retrieval of a source-code sentinel at both 63,433 and 131,948 raw tokens. A matched 63,433-token **int8** run failed the same way as int4, so the failure is not explained by int4 alone. A PFlash-off int8 attempt at 63,433 tokens was rejected by Rapid/MLX's single-buffer limit; it was not an OS-level OOM. Do not claim compression is the sole cause until a smaller PFlash-off control is run.

## Resume here — Phase 5 evidence closeout (2026-07-24)

The **Phase 5 implementation is already verified complete**: 5a cross-surface equality is `791635e`; 5b verifier checkpoint is `6a14cc7`. The authoritative execution companion now records that the remaining work is evidence calibration and conservative recommendation eligibility, not a rebuild of the estimator. Preserve the dirty worktree and do not commit/push without the user's explicit approval.

Completed fresh verifier pass on the current tree: `cargo clippy -- -D warnings`, `cargo test` (1,766 passed, 26 ignored), `npm run validate-js`, `npm run lint`, `git diff --check`, release build, and sequential `models-v2`, `preset-editor`, and `spawn-wizard-engines` captures. Inspect the generated artifacts if UI work changes; do not claim visual proof merely because the capture command exits zero.

The tool driver now correctly makes primary-agent tool cells request **32,768 output tokens** with `reasoning_max_tokens: 16,000`; do not reuse the old 128-token tool receipt as agent-fidelity evidence. The corrected Froggeric-template receipt is `tests/fixtures/calibration/rapid-mlx-receipts/qwen35-9b-defiant-tools-froggeric-32k-r16k/00-00-tools-sequential-8k.json`: it produced a parsed tool call but selected `apply_patch` before required `read_file`, so it is a failure of the sequential trace, not proof of no tool capability. The HF snapshot's `chat_template.jinja` and `tokenizer_config.json` were restored to their original symlinks after every test.

**Calibration finding that must drive the next implementation step:** an isolated canonical estimator call for Qwen 3.5 9B Defiant, int8, 8,225 planning tokens, and 4,096 retained tokens predicted **11.64 decimal GB**; the matching fresh Rapid tool receipt peak is **16.64 decimal GB** (~43% underprediction). This is the expected first tuning datum, not a Phase 5 implementation failure. Build a durable, secret-safe calibration record that binds: the exact estimator request/response, model revision/config hash, runtime receipt path, actual peak metric, decimal/GiB units, residual, and evidence tier. Start the app in a temporary `--config-dir`, use its generated API token only in-process, and delete the temporary directory; never save a token in a receipt or log.

**v0.11.0 BF16 calibration update (2026-07-25):** the compact fresh 32k/65k/131k Qwen 3.5 PFlash-off requested-int8 matrix is now version-pinned and paired with exact token-free estimator responses in `tests/fixtures/calibration/rapid-mlx-receipts/qwen35-9b-v011-bf16-calibration/`. All estimator responses report effective active KV `bf16`, matching the current #1197 workaround. 32k and 65k tuning residuals are -1.50% and +4.93%; the 131k holdout is +17.06%. This is a Provisional v0.11.0 envelope, not a formula-promotion result. Preserve the receipts for the release containing the upstream fix, then repeat the compact confirmation set before claiming int4/int8 active-KV savings.

**Unreleased #1197 source gate (2026-07-25):** current `main` commit `5fc6556c9b9fbf63c56f69a71a0fd6482ece26e4` was run in an isolated source wrapper and is recorded as `rapid-mlx 0.11.0+git.5fc6556c`. At 32k PFlash-off, runtime metrics distinguish live `bf16`, `int8`, and `int4` cache dtypes for both Qwen 3.5 9B Defiant and `unsloth/Qwen3.6-35B-A3B-UD-MLX-4bit`; this is real #1197-path execution, not the v0.11.0 requested-dtype no-op. Qwen 3.5 peaks: bf16 17.07 GB, int8 16.70 GB, int4 16.47 GB, all 5/5 markers. Qwen 3.6 35B peaks: bf16 27.87 GB, int8 27.63 GB, int4 27.47 GB, all 5/5 markers. The earlier 4/5 int4 result was a scorer defect: it inspected only the 2,000-character persisted preview while the model returned a correct marker later in a 708-token completion. Receipts now score the complete in-memory response before removing it, so source int4 clears the narrow quality gate. Run the full BF16/int8/int4 context, cache, TurboQuant, tool, PFlash, prefill, and vision-capability matrices against both pinned models; source receipts are pre-release evidence and must be re-confirmed once a tagged Rapid release lands.

Ordered next work:

1. Add the durable estimator-to-runtime comparison writer, then pair it with the existing Qwen 3.5 9B fresh context receipts (8k/16k/32k, int8 and int4, PFlash off). Use some rows to tune a Rapid-specific fixed/load/allocator term and context slope; hold out at least one context/dtype row before declaring any calibration improvement.
2. Repeat only enough fresh rows to distinguish active KV, retained prefix state, and transient peak. Do not fit Metal lifetime high-water data; every peak row requires a fresh server and initial metric check.
3. Keep result tiers honest: out-of-envelope rows stay Calculated/Provisional; Qwen 3.6 35B's 32k retrieval failure remains a negative capability finding, not a memory-calibration row; no K8V4 recommendation without cache-enabled cold/repeat/extension evidence.
4. For Qwen 3.5 tools, run the same 32k-output/16k-reasoning trace with the original template, then the Froggeric template with thinking explicitly disabled. Only after the first call is correct, expand to sequential, large-observation, error/retry, null/empty, and cache-reuse traces.
5. Update `20260718-final_rapidmlx_followups_execution.md` and this file after every material result. Phase 6 may begin only with a clear qualified/provisional/refused cache/quant evidence table; a negative result is a valid closure outcome.

## Environment pinned for current Qwen rows

| Field | Value |
|---|---|
| Runtime | `rapid-mlx 0.10.17` |
| Hardware | Apple M5 Max, 64 GiB unified memory (`68719476736` bytes) |
| Model | `nightmedia/Qwen3.6-35B-A3B-Fable-Holo3.1-mxfp4-mlx` |
| Model revision | `16279aa65cee814c6b23e068a71eec7e1617fae0` |
| Config SHA-256 | `7a51dff046fe60fb82428666efcc9bc63b835a2d7edab25d7c410ab40ad8bd30` |
| Server mode | Text-only (`--no-mllm`), one sequence/request, prefix cache disabled for cold/PFlash rows |
| Retrieval fixture | generated TypeScript source; return the exact `RAPID_CONTEXT_VALIDATION.exact_value` sentinel |

### Conversion A/B authority (2026-07-24)

All Nightmedia rows below are conversion-specific, not a Qwen3.6-family verdict. The user has directed the next primary comparison to use `unsloth/Qwen3.6-35B-A3B-UD-MLX-4bit`. Download it, pin its resolved revision and config SHA-256, then repeat the same fresh-server PFlash-off/int8 source-retrieval controls at approximately 8k, 16k, and 32k before comparing cache, TurboQuant, tools, PFlash, or long-context claims. Do not merge rows from the two conversions in one performance average.

The requested Unsloth A/B is now available locally at revision `6700c3e5bdeb050a379c8d2a4133f43f3647f20f`, config SHA-256 `46f215270b159f88fdf2eda2151894e2803e5def43062c34e376499ded6f277a`. Its fresh-server uncompressed int8/int4 matrix also passes exact source retrieval at 7,698 and 15,295 tokens, then fails at 31,767 tokens for both dtypes. This removes the Nightmedia conversion as the primary explanation for the observed 32k source-retrieval cliff; it remains an execution-path finding until compared against another runtime.

Unsloth PFlash/int4 evidence: 63,433 raw tokens compressed by 50,746 (estimated 12,687 retained), 3.34 s TTFT, 58.58 tok/s TG, retrieval fail; 131,948 raw tokens compressed by 105,558 (estimated 26,390 retained), 11.95 s TTFT, 87.98 tok/s TG, 57.11 decimal GB Metal peak (about 53.2 GiB), retrieval fail. The 131k row fits below 64 GiB; it proves neither full-context source recall nor agent/tool fidelity.

The Nightmedia model is not on Rapid's verified PFlash/TurboQuant alias tiers; its automatic defaults are therefore not evidence. Always pin `--pflash`, `--kv-cache-dtype`, and `--kv-cache-turboquant` in the argv and assert the exported metric labels in the manifest.

## Valid Qwen evidence collected

These are single cold requests with `--max-tokens 64`, `--disable-prefix-cache`, `--kv-cache-turboquant none`, `--no-mllm`, and PFlash auto threshold `32768`. Values are server-reported prompt tokens; do not relabel the rows using fixture word counts.

| Raw tokens | KV | PFlash | Compressed | Estimated retained | TTFT | TG | Peak Metal memory | Exact code retrieval |
|---:|---|---|---:|---:|---:|---:|---:|---|
| 63,433 | int4 | auto | 50,746 | 12,687 | 3.52 s | 87.33 tok/s | invalid lifetime high-water | fail |
| 131,948 | int4 | auto | 105,558 | 26,390 | 12.26 s | 36.11 tok/s | 54.79 GB, fresh server | fail |
| 63,433 | int8 | auto | 50,746 | 12,687 | 3.78 s | 82.06 tok/s | 31.32 GB, fresh server | fail |

Interpretation limits:

- The 131k int4 row proves a narrow **fit/performance** point, not a comfortable usable envelope: 54.79 GB is near a 64 GiB machine's practical limit.
- PFlash is lossy prompt compression. A result based on its retained prompt is not equivalent to native full-context capability.
- `raw_prompt_tokens / TTFT` includes PFlash scoring/compression; it is not a native prefill kernel throughput measurement.
- Do not call int4 agent-safe or treat Rapid int8 as llama.cpp `q8_0`. Tool traces must establish that separately.
- The int8 failure makes int4 alone an insufficient explanation. It does **not** yet prove PFlash caused the miss, because the same full raw prompt cannot run uncompressed at 63k on this runtime configuration; a smaller PFlash-off control is required.

## Explicitly invalid or incomplete evidence

- An earlier PFlash-off roughly-31k attempt lost its listener/session. No process/log evidence identified the cause; it is **inconclusive** and must not be called a crash or OOM.
- A fresh 63,433-token PFlash-off int8 control produced a captured, recoverable Rapid/MLX allocation-limit error: it requested a 128,755,795,968-byte Metal buffer while the runtime maximum was 41,747,087,360 bytes. It was **not** a system OOM or macOS process kill; Rapid recovered the batch and emitted an empty SSE completion. It is a non-fit for this exact raw-context/configuration, and the runner now marks its zero-token response as failed.
- A first “65k code” fixture tokenized to 157,295 model tokens. Its failed result belongs to an approximately-160k workload, not a 65k row. The corrected fixtures are now 25,000 words (~63k observed tokens) and 52,000 words (~132k observed tokens).
- Two early PFlash rows ran on a server whose `rapid_mlx_metal_peak_memory_bytes` had already reached 67.02 GB. Those peak values are lifetime high-water, not per-row memory. Do not reuse them.
- The valid 131k int4 peak came only after killing the orphaned Python child, verifying the listener was down, starting a new server, and confirming the initial peak metric was 18.53 GB before the request.

### Completed control set: uncompressed source retrieval at 0.88 utilization

This is the first completed, directly comparable Qwen matrix. Every row used a fresh server with `--gpu-memory-utilization 0.88`, `--pflash off`, `--disable-prefix-cache`, `--kv-cache-turboquant none`, text-only mode, one sequence/request, and the same revision-pinned generated TypeScript source/sentinel fixture. It is a source-retrieval test, **not** a tool-fidelity result.

| Raw tokens | KV dtype | TTFT | TG | Metal peak | Exact sentinel |
|---:|---|---:|---:|---:|---|
| 7,698 | int8 | 2.04 s | 87.51 tok/s | 25.34 GB | pass |
| 7,698 | int4 | 2.02 s | 87.64 tok/s | 25.34 GB | pass |
| 15,295 | int8 | 4.32 s | 80.31 tok/s | 35.15 GB | pass |
| 15,295 | int4 | 4.32 s | 77.84 tok/s | 35.15 GB | pass |
| 31,767 | int8 | 25.83 s | 58.33 tok/s | 67.83 GB | fail |
| 31,767 | int4 | 11.93 s | 91.90 tok/s | 67.83 GB | fail |

The 32k peak is 67.83 decimal GB (about 63.2 GiB), within a 64 GiB machine but very near the practical edge; it must not be called comfortable. Both dtypes failing at the same point means this fixture's 32k quality cliff is not an int4-only artifact. Int4 was materially faster in that one 32k run, but one row does not establish a stable performance advantage. The matching 63,433-token PFlash-off int8 run did not execute: its requested 128.76 GB single buffer exceeded MLX's 41.75 GB per-buffer maximum.

## Current live server and immediate safe next action

The 63,433-token **int8, PFlash-off** control was attempted on a fresh server. Rapid/MLX rejected its requested 128.76 GB *single Metal buffer* against a 41.75 GB runtime maximum, recovered, and returned zero usage. This was not an OS-level OOM. The runner now classifies that empty response as `request_succeeded: false`. The 8k/16k/32k uncompressed int4/int8 matrix is complete above; the next uncompleted set is cache-enabled TurboQuant cold/repeat/extension, followed by tool-call fidelity.

The server may still be live on `127.0.0.1:18087`; verify rather than assume it is live:

```bash
rtk proxy curl -sS --max-time 5 http://127.0.0.1:18087/health
curl -sS --max-time 5 http://127.0.0.1:18087/metrics | grep '^rapid_mlx_kv_cache_dtype\|^rapid_mlx_metal_peak_memory_bytes'
```

The dedicated int8 PFlash-off manifest is `tests/fixtures/calibration/model-runtime-benchmark-int8-pflash-off.example.json`. Bracket the largest raw context that does fit (start around 32k) for a true uncompressed int8/int4 fidelity comparison. Preserve logs/process status for every non-fit—never infer a system OOM from a connection failure or a caught allocation error alone.

## Exact server lifecycle discipline

For **every** memory/fit cell, use a fresh server. Killing the wrapper alone leaves an orphan Python child; identify the child and kill that PID too.

```bash
rtk pgrep -af 'rapid-mlx.*18087'
ps -p <PIDS> -ww -o pid,ppid,command=
rtk kill <RAPID_PYTHON_PID>
rtk proxy curl -sS --max-time 3 http://127.0.0.1:18087/health  # must fail before relaunch
```

Example int8/PFlash-off server (all values must be copied into the manifest's `exact_argv`):

```bash
rtk proxy rapid-mlx --no-telemetry serve \
  nightmedia/Qwen3.6-35B-A3B-Fable-Holo3.1-mxfp4-mlx \
  --port 18087 --host 127.0.0.1 \
  --max-num-seqs 1 --max-concurrent-requests 1 \
  --disable-prefix-cache --pflash off --prefill-step-size 150000 \
  --max-tokens 64 --kv-cache-dtype int8 --kv-cache-turboquant none \
  --no-mllm --gpu-memory-utilization 0.75 --log-level INFO
```

Before a request, health must be `ready:true`; the metrics must show the expected KV dtype and an initial low peak. After the request, save the receipt outside `/private/tmp` under `tests/fixtures/calibration/rapid-mlx-receipts/` only after reviewing it. Temporary receipts currently exist under `/private/tmp` and are not durable.

## Runner and manifests

| File | Purpose |
|---|---|
| `scripts/model-runtime-benchmark.mjs` | Runtime-neutral OpenAI-compatible runner and report generator. |
| `scripts/rapid_mlx_context_probe.mjs` | Earlier single-cell helper; prefer the general runner for new evidence. |
| `tests/fixtures/calibration/model-runtime-benchmark-profiles.json` | Canonical matrix: cold context, primary OpenCode agent, long-context research. |
| `tests/fixtures/calibration/model-runtime-benchmark.example.json` | Pinned current Qwen int4/PFlash-auto source-retrieval manifest. |
| `tests/fixtures/calibration/model-runtime-benchmark-int8.example.json` | Pinned Qwen int8/PFlash-auto matched 65k source-retrieval manifest. |
| `docs/reference/model-runtime-benchmarking.md` | Contract, measurements, and comparison rules. |

Run and report:

```bash
rtk proxy node scripts/model-runtime-benchmark.mjs run \
  --manifest <MANIFEST> --base-url http://127.0.0.1:18087 \
  --cell <CELL_ID> --out /private/tmp/<RECEIPT>.json

rtk proxy node scripts/model-runtime-benchmark.mjs report \
  --input <RECEIPT_OR_COMBINED_RECEIPTS>.json --out /private/tmp/<REPORT>.md
```

The runner records sanitized request metadata only: character count, SHA-256, tool names, and no prompt/image payload. It records server-reported usage, TTFT, TG, selected Prometheus metrics, PFlash counters, retrieval scoring, tool-name/argument scoring, optional image input, and cold/repeat/extension phases. `expected_metrics` is deliberately fail-closed—update the manifest when changing the server, not the other way around.

## Required Qwen matrix, in order

1. **Causal source-retrieval controls:** at 63,433 raw tokens (and at the largest safely fitting raw context): PFlash off/auto × int8/int4, fresh server per cell. Same model revision, template, fixture, output cap, and concurrency. Repeat needle positions 10%, 50%, 90%; then retain ratio `.20` and `.50` where PFlash applies.
2. **Cold context/performance:** 8k, 32k, 65k, 131k; int8 and int4; PFlash off. Use 128 then 1k output, and record whether the raw context fits. Do not silently substitute PFlash fit for the PFlash-off row.
3. **Prefix cache/TurboQuant:** enable prefix cache with an explicit measured cache budget. For `none` and `k8v4`, run cold/repeat/extension. TurboQuant affects retained reusable prefix snapshots, not cold active KV or weights—do not compare it using only cold rows.
4. **Agent fidelity:** for int8 first, then int4 candidate, pin Qwen's tool template/parser and score single call, multi-turn, parallel calls if supported, null/empty args, large observation, retry/error, and cached-prefix reuse. PFlash off is the initial agent control; PFlash with tools is a separate risky experiment because tool content is skipped by default unless `--pflash-include-tools` is passed.
5. **Output/reasoning profile:** normal evidence is 128/1k/4k/8k output. The user wants 32k output and 16k reasoning as safety ceilings, not assumed normal behavior; run 16k/32k only as explicitly guarded boundary rows with `minimum_completion_tokens`. Pass runtime-specific reasoning fields via `workload.extra_body` and record them in the manifest.
6. **Long context:** only after fresh-server rows, generate straight-line 160k/200k estimates. Label them estimates, never proof of fit/fidelity/safety.

### Qwen 3.5 9B template A/B: first tool control (2026-07-24)

Receipt: `tests/fixtures/calibration/rapid-mlx-receipts/qwen35-9b-defiant-tools-froggeric/00-00-tools-sequential-8k.json`.

`nightmedia/Qwen3.5-9B-DS9-USS-Defiant-1M-q8-hi-mlx` was run at its pinned revision with the user-supplied `/Users/nick/froggeric_qwen-chat_template.jinja` (SHA-256 `d552c2bb68a6a6a6c07ac9f378cbf7e46ba9d8261c4510d7d79f1f0bbfa37b22`). Rapid-MLX/Transformers gives a standalone `chat_template.jinja` precedence over `tokenizer_config.json`, so the controlled test temporarily replaced **both** snapshot sources, then restored their exact original symlinks after shutdown. It did not modify the blob store or leave backup files behind.

To keep the test independent of alias inference, its argv explicitly pinned the profile reported by `rapid-mlx info`: `--tool-call-parser hermes --enable-auto-tool-choice --reasoning-parser qwen3 --no-hybrid`.

The first row accidentally retained the generic 128-token throughput-probe cap, so it is not agent-fidelity evidence. The corrected receipt is `tests/fixtures/calibration/rapid-mlx-receipts/qwen35-9b-defiant-tools-froggeric-32k-r16k/00-00-tools-sequential-8k.json`: server and request both permit 32,768 output tokens, while request `reasoning_max_tokens` is 16,000. It generated 286 reasoning + 610 completion tokens and emitted a real, parsed tool call—but chose `apply_patch` first instead of the required `read_file`. Thus this template/profile combination has demonstrated structured-call emission, but **fails this required sequential ordering trace**. It is not yet evidence of reliable coding-agent tool use. Compare a same-limit original-template control, then test thinking-off as a separate variable before attributing the ordering failure to template versus reasoning behavior.

## Other model evidence and limits

### Gemma 4 12B QAT

`mlx-community/gemma-4-12B-it-qat-4bit` is the only tested modern VLM that actually worked through Rapid MLLM. A real Preset Editor screenshot was accepted. Existing receipt: `tests/fixtures/calibration/rapid-mlx-receipts/gemma4-12b-qat-4bit-m5max-64gb-0.10.17.json`.

Early single-sequence, cache-disabled, PFlash-off text evidence:

| Text tokens | KV | TTFT | TG | Metal peak |
|---:|---|---:|---:|---:|
| ~6,041 | int4 | 4.13 s | 41.11 tok/s | 18.35 GB |
| ~15,041 | int4 | 17.52 s | 35.87 tok/s | 25.86 GB |
| ~6,041 | int8 | 4.09 s | 41.08 tok/s | 18.35 GB |
| ~15,041 | int8 | 16.60 s | 39.66 tok/s | 25.86 GB |
| ~6,316 + screenshot | int4 | 4.80 s | 40.99 tok/s | not separately measured |

Gemma's sliding-window behavior means those int4/int8 rows did not show a material active-memory difference; this does not settle tool safety. Its MLLM scheduler initially rejected a 32k prompt at default `--prefill-step-size 8192`; only claim a larger row when it has a fresh successful receipt with its exact admission setting.

### Qwen 3.5/3.6 vision

Rapid's MLLM route delegates to `mlx-vlm`, then uses Rapid's batched MLLM scheduler. The user’s Qwen 3.5/3.6 hybrid VLM models fail because their language cache is `ArraysCache`, whereas the scheduler can merge only `KVCache`/`RotatingKVCache`:

- `nightmedia/Qwen3.6-35B-A3B-Fable-Holo3.1-mxfp4-mlx`: text works; image fails with the `ArraysCache` MLLM blocker.
- `nightmedia/Qwen3.5-9B-DS9-USS-Defiant-1M-q8-hi-mlx`: same blocker.

Use llama.cpp for those Qwen vision workflows today. Do not recommend `waybarrios/vllm-mlx` as a vision fix: Rapid is a community fork built on the same MLX stack and upstream is not a demonstrated workaround. Future loader abstraction should target a generic OpenAI-compatible runtime contract.

## PFlash facts to preserve

- In installed Rapid 0.10.17, `auto` threshold is **32768** tokens, not the older documentation's 8192.
- PFlash is unavailable/rejected for MLLM.
- PFlash is lossy prompt compression, distinct from KV quantization and TurboQuant. Defaults include retain ratio `.20`, 256 sink tokens, and 2048 tail tokens.
- For a verified alias, default PFlash can be `always`; the Nightmedia profile resolves unknown, so it defaults off unless explicitly set.
- `--prefill-step-size` is scheduler/admission control, not a llama.cpp `-ubatch` substitute. Do not import llama.cpp settings blindly.
- **Verdict (2026-07-24, prefill-step-size 512 quant-baseline run):** `--pflash auto` is **not recommended** with default options at Rapid-MLX 0.11.0. On `nightmedia/Qwen3.5-9B-DS9-USS-Defiant-1M-q8-hi-mlx`, int8, step=512, across both `none` and `k8v4` TurboQuant, recall collapsed to 0.0/0.2/0.4/0.2 at 63k/131k/160k/200k tokens (vs 1.0 with `pflash off` at every tier). Reported prefill throughput simultaneously jumped to ~4x uncompressed (6700-9400 tok/s vs ~600-2300 tok/s) while ~80% of tokens were marked compressed — a pattern consistent with the compressed region being dropped rather than lossily retained, not confirmed against upstream source. In an agentic coding loop this is a silent failure mode: nothing signals the model that context was lost, so it confidently produces output grounded in partial/absent content rather than triggering a re-fetch. Default recommendation stays `pflash off`; do not re-open `auto`/`always` without a source-level fix or a materially different retain-ratio/threshold configuration re-tested against this same recall harness.

## Single-buffer ceiling root-cause investigation (2026-07-24, second session)

This section escalates a request for outside help. It is written for a fresh reader (Opus or another agent) who was not in the diagnostic session. It does not resolve the open question; it narrows it as far as this session's tools allowed, and it flags exactly where the trail runs out and why.

### What triggered this

The multi-marker Qwen 3.5 9B `all`-suite re-collection (`tests/fixtures/calibration/rapid-mlx-receipts/qwen35-9b-defiant-all-088-markers/`) reproduced the same silent-empty failure already logged above for the 35B model (line ~78: "requested a 128,755,795,968-byte Metal buffer while the runtime maximum was 41,747,087,360 bytes"), this time on `nightmedia/Qwen3.5-9B-DS9-USS-Defiant-1M-q8-hi-mlx` at native (PFlash-off) 131,072-token context, both int8 and int4, and separately on the PFlash 196,608-token cell. All three receipts show `server_errors: []`, `usage: {prompt_tokens: 0, completion_tokens: 0, total_tokens: 0}`, empty `completion_preview` — a clean, error-free, zero-token HTTP 200 response. This is the second independent model (9B and 35B, different architectures/quant sizes) hitting the identical class of failure, which raises it from "one model's quirk" to "a property of this rapid-mlx/vllm_mlx stack's cache admission path."

### What is now directly confirmed (not inferred)

A live repro (fresh server, `--force-hybrid`, `--kv-cache-dtype int8`, `--pflash off`, otherwise identical argv to the failing receipt) reproduced the crash with full server-side stderr captured:

```
ERROR:rapid_mlx.scheduler:Error in batch generation step: [metal::malloc] Attempting to
allocate 68719476736 bytes which is greater than the maximum allowed buffer size of
41747087360 bytes.
Traceback (most recent call last):
  File ".../vllm_mlx/scheduler.py", line 5310, in step
    raw_next = self.batch_generator.next()
  File ".../mlx_lm/generate.py", line 1855, in next
    return self._next()
  File ".../mlx_lm/generate.py", line 1841, in _next
    self._prompt_batch.prompt(prompts)
  File ".../mlx_lm/generate.py", line 1161, in prompt
    mx.eval([c.state for c in self.prompt_cache])
RuntimeError: [metal::malloc] Attempting to allocate 68719476736 bytes ...
WARNING:rapid_mlx.scheduler:[generation_error_recovery] aborted 1 running requests,
batch generator closed, Metal cache cleared
```

This is not a guess: **rapid-mlx's `generation_error_recovery` catches a real Metal allocation crash mid-generation and converts it into a clean, empty, `200 OK` SSE stream with zero usage** instead of surfacing an OpenAI-style `error` event. That swallowing behavior is a confirmed rapid-mlx robustness bug, independent of anything below, and should be reported upstream regardless of what causes the allocation to be oversized in the first place.

`41,747,087,360` bytes is the machine's real, queryable Metal ceiling, not a guess or a config value we set:

```python
import mlx.core as mx
mx.device_info()
# {'device_name': 'Apple M5 Max', 'max_recommended_working_set_size': 60129542144,
#  'memory_size': 68719476736, 'architecture': 'applegpu_g17s',
#  'max_buffer_length': 41747087360, 'resource_limit': 499000}
```

`max_buffer_length` is a hardware/driver constant for a given Mac, queryable once per machine, independent of model or request. It is the number the crash reports as "the maximum allowed buffer size." This should be captured once and recorded in every receipt's `hardware` block going forward — it was not captured this session.

**`--force-hybrid` does not fix the crash** (identical failure, identical byte count, with `rapid_mlx.engine_core` logging "Hybrid model: running full request warmup (compiling GatedDeltaNet kernels)" — confirming the linear-attention runtime path really was active). This rules out the first hypothesis tried this session (that `rapid-mlx info`'s "pure attention" misclassification of this specific community fine-tune, and the resulting `--no-hybrid` flag our suite driver forces, was the cause). It is not.

### The part that does not yet add up — flagged explicitly, not glossed over

The requested byte count (`68,719,476,736` = exactly 2^36 = 64 GiB) exactly equals `num_hidden_layers(32) × 2(K+V) × num_key_value_heads(4) × head_dim(256) × 1 byte(int8) × max_position_embeddings(1,048,576)` for this model's `config.json`. That is an exact match, verified in Python, not an approximation. It strongly suggests something in this stack sizes a cache buffer using the model's **advertised maximum context (1,048,576, the "1M" branding)** rather than the actual request length (131,952 tokens actually sent) or the server's actual configured ceiling (there is no `--context-length`/`--max-model-len` flag in `rapid-mlx serve --help` at all — confirmed by reading the full help text this session).

However, this session traced the generic `mlx_lm` cache-construction path and **could not find the code that does this**:

- `mlx_lm/models/qwen3_5.py:304-305` → `make_cache()` correctly builds `ArraysCache(size=2)` for linear-attention layers (fixed-size, does not grow with context — that's the point of linear attention) and plain `KVCache()` for full-attention layers.
- `mlx_lm/models/cache.py` `KVCache.update_and_fetch()` grows incrementally in `step=256` chunks as new tokens arrive — it does not pre-allocate to `max_position_embeddings` and would not produce one 64 GiB `mx.eval` on its own.
- The `qwen3_5.py` dataclass field `max_position_embeddings: int = 131072` (a default, distinct from the real config value 1,048,576) is used nowhere else in that file — a numerical near-coincidence with the 131,072 token count that this session confirmed is **not** causally connected; do not reuse that observation.

The generic `mlx_lm` package (open-source, Apple's) does not appear to be where the oversized allocation originates. The likely location is `vllm_mlx`'s own `BatchedEngine`/`scheduler.py` (the continuous-batching layer rapid-mlx builds on top of stock `mlx_lm` — referenced in the traceback as `vllm_mlx/scheduler.py:5310`), which this session did not have time to trace fully; `scheduler.py` is large (5000+ lines) and a first grep for `max_position_embeddings`/cache pre-allocation inside it did not surface an obvious hit, but the search was not exhaustive.

**Do not present the "sized by max_position_embeddings" explanation as confirmed.** It is a strong, numerically exact, falsifiable hypothesis — not a verified code path.

### Why this is confusing relative to llama.cpp — the question that should drive the next investigation

The user's own comparison point, and the reason this needs deeper help: on a 5090 box, `Qwen3.6-27B Q4_K_M` GGUF via llama.cpp with `q8_0` KV runs a full 200k-token context in ~31 GB total (17 GB model weights + KV/mmproj/mtp heads) — a *larger* model, a *larger* context, on *less* total memory than this 9B MLX model's 64 GiB single-buffer crash at 131k. That is the crux. The candidate explanation is architectural: llama.cpp's KV cache is allocated in fixed blocks sized to the **operator-configured `--ctx-size`**, processed and paged incrementally — it never materializes one contiguous buffer proportional to a model's *advertised maximum* context. If this MLX stack's cache pool is instead sized (even only in some admission/continuous-batching code path, not the plain single-request `mlx_lm` path) by the model's `max_position_embeddings` metadata — and rapid-mlx has no flag to override that ceiling — then any model advertising a large native/YaRN-extended context (this one claims 1M) could be structurally unable to serve requests anywhere near naive proportionality to available RAM, regardless of quant size or KV dtype. This would mean the failure is closely tied to how "long-context" models are marketed/configured upstream (YaRN `factor: 4.0`, `original_max_position_embeddings: 262144` → `max_position_embeddings: 1,048,576`) interacting badly with this specific serving stack's cache admission strategy.

### The most promising untested lever

`rapid-mlx serve --help` exposes `--use-paged-cache`, `--paged-cache-block-size` (default 64), and `--max-cache-blocks` (default 1000) — described as "Use paged KV cache for memory efficiency (experimental)." **None of this session's or the prior session's cells have ever set this flag.** Paged/blocked KV cache is exactly the mechanism llama.cpp and vLLM-proper use to avoid single giant contiguous buffers. If the default (non-paged) cache path is what materializes the oversized single buffer, `--use-paged-cache` may sidestep this ceiling entirely — or it may reveal the same underlying sizing bug in a different shape (e.g. `max_cache_blocks × paged_cache_block_size` might itself default to a value scaled by the model's max context). This is the single highest-value next experiment: rerun the same failing 131,072-token int8 cell with `--use-paged-cache --paged-cache-block-size 64 --max-cache-blocks <some bound>` and capture full server stderr regardless of outcome.

### Concrete next steps, in order

1. **Add `mx.device_info()` capture to the benchmark harness's hardware block** (one-time per machine, not per-cell) so `max_buffer_length` is a recorded fact in every future receipt, not something rediscovered by reading a crash traceback.
2. **Test `--use-paged-cache`** on the same failing 131,072-token int8 cell (`nightmedia/Qwen3.5-9B-DS9-USS-Defiant-1M-q8-hi-mlx`), fresh server, full stderr captured regardless of pass/fail. This is the fastest path to either a real fix or a sharper root cause.
3. **Trace `vllm_mlx/scheduler.py`'s `BatchedEngine` cache-pool construction** (not the generic `mlx_lm` path, which this session ruled out) to find where — if anywhere — a buffer is sized from `max_position_embeddings` rather than actual/configured context. `pip show vllm_mlx` / the installed site-packages path is `~/.local/share/uv/tools/rapid-mlx/lib/python3.11/site-packages/vllm_mlx/`. This is a proprietary-feeling fork, not vanilla open-source `mlx_lm` — expect it to take real time.
4. **Report the swallowed-exception behavior upstream to rapid-mlx** regardless of the sizing root cause: a mid-generation Metal allocation crash should never present to an OpenAI-compatible client as a clean 200 OK with zero usage and no `error` event. This is a correctness/observability bug on its own, separate from whatever is oversizing the buffer.
5. **Do not yet build a context-ceiling prediction formula into the VRAM/calibration estimator** (tasks #1-3 in the sibling handoff) based on the `32 × 2 × kv_heads × head_dim × max_position_embeddings` arithmetic above. It matched once, exactly, on one model — that is necessary but not sufficient evidence for a general formula, and the earlier 35B-model buffer request (`128,755,795,968` bytes at 63,433 tokens) has not yet been checked against the same formula using that model's own config.json. Do that check first if this path is picked back up.
6. Anomaly B (zero tool-call detection across all 4 tool cells in the same marker-methodology run) is still completely uninvestigated — none of this session's time went to it. It needs the same raw-receipt/server-log drill-down applied here before Task #13 (35B recollection) is safe to run, since a systemic tool-wiring bug would silently invalidate that entire suite the same way this context bug would have.

### Session discipline notes for whoever picks this up

- The scratch repro server (port 18099) and its manifest were run outside the durable receipts directory, under `/private/tmp/.../scratchpad/`, and are not durable evidence — they exist only to capture the stderr traceback above, which has been transcribed into this doc. Do not treat those scratch files as receipts.
- Preserve the dirty worktree; do not commit or push. This section is documentation-only and does not change runtime behavior.
- The `qwen35-9b-defiant-all-088-markers/` receipt directory (18 receipts) is otherwise good evidence for the multi-marker methodology itself: perfect 5/5 recall at 8k/16k/32k (int8 and int4) and both cache configurations. Only the 131k/196k context cells and all four tool cells are unresolved.

## RESOLVED — root cause is quadratic prefill attention, NOT KV-cache sizing (2026-07-24, third session)

**The `max_position_embeddings` / single-buffer-ceiling hypothesis above is wrong.** It was a numerical coincidence and is superseded by a live, instrumented root-cause capture. Preserved above for the audit trail; do not act on the "sized by advertised 1M context" explanation.

### How it was resolved

Instrumented the actual crash site (`mlx_lm/generate.py:PromptProcessingBatch.prompt`, the `mx.eval([c.state for c in self.prompt_cache])` line) with a one-time per-chunk dump of every cache object's `state` array shapes, then reproduced the exact failing cell against a fresh server (identical argv to the failing receipt: `--no-hybrid --pflash off --prefill-step-size 32768 --kv-cache-dtype int8`, model `nightmedia/Qwen3.5-9B-DS9-USS-Defiant-1M-q8-hi-mlx`). The instrumentation was reverted afterward; the installed `mlx_lm` is back to stock. A ~132,837-token request produced this sequence, captured from server stderr:

```
[schedule] prompt_tokens=132837 tokens_to_prefill=132837
[CACHE_PROBE] tokens_total=32768  TOTAL_STATE_BYTES=1.048GiB   # chunk 1, grew fine
[CACHE_PROBE] tokens_total=65536  TOTAL_STATE_BYTES=2.048GiB   # chunk 2, grew fine (linear)
ERROR: [metal::malloc] Attempting to allocate 68719476736 bytes ...   # chunk 3 forward pass
```

The KV cache is a non-issue: it grows **linearly** (~1 GiB per 32,768-token chunk, bf16) and was only ~2–3 GiB at the moment of the crash. The 64 GiB allocation is not a cache at all — it is the **attention score matrix** for one full-attention layer during a prefill chunk:

```
q_len (= prefill_step_size = 32768) × k_len (cumulative context = 65536) × num_attention_heads (16) × 2 bytes (bf16)
= 32768 × 65536 × 16 × 2 = 68,719,476,736 bytes = 2^36 = exactly the crash size
```

The earlier `num_hidden_layers(32) × 2 × kv_heads(4) × head_dim(256) × 1 × max_position_embeddings(1,048,576)` "match" was a coincidence: both expressions equal 2^36. `get_model_max_context()` (reads `max_position_embeddings`) is used only for **request admission and `/models` advertising**, never for cache sizing — verified by tracing. And the model's stock `make_cache()` returns 24 `ArraysCache` + 8 plain `KVCache`, `max_size=None`, no preallocation — verified by loading the model and probing directly.

### The confirmed trigger and threshold

The full-attention layers materialize the full `[1, heads, q_len, k_len]` score buffer during prefill instead of using a streaming/flash kernel. Crash condition:

```
prefill_step_size × context_tokens × num_attention_heads × dtype_bytes  >  max_buffer_length (41,747,087,360 on M5 Max)
```

For this model (`heads=16`, bf16 score dtype): crash when `prefill_step × context × 32 > 41.7e9`.
- `--prefill-step-size 32768`: crashes once **context > ~40k** (explains why 8k/16k/32k cells passed and 131k failed; even a 64k context would fail).
- `--prefill-step-size 4096`: safe up to **~318k context**.

This is **model-general and hardware-general**, not specific to 1M-branded checkpoints. The 262k mxfp4 sibling and the Qwen3.6-35B-A3B (262k) would hit the same wall at high context with a large prefill step. The reason nobody caught it in months of rapid-mlx development: the common docs/examples exercise ≤32k or use a small default prefill step, staying under the ceiling; the benchmark suite's `--prefill-step-size 32768` is what surfaces it.

### Confirmed workaround (live-verified)

Rerunning the **identical** 132,837-token request with only `--prefill-step-size 32768 → 4096` changed: **HTTP 200, `finish_reason=stop`, correct output `OK`**, KV grew linearly to ~3.7 GiB, no crash, ~122 s prefill. (Bonus: the large prefill step also produced garbage output — repeated `!` — at a 56k context that did *not* crash; the small step fixed both the crash and the quality.)

Recommended defaults for high-context on this stack: **`--prefill-step-size 4096`** (or ≤8192 for ≤131k). Tradeoff is slower prefill (more, smaller chunks) for correctness at context the large step cannot serve.

### Still open / next

1. **Benchmark harness fix:** the suite hardcodes `--prefill-step-size 32768`. Lower it (4096) for the ≥64k context cells, or make it context-adaptive via the threshold formula above, then re-collect the 131k/196k cells that previously logged silent-empty failures — they should now pass.
2. **`--kv-cache-dtype int8` is ignored** for these hybrid batch caches — the `BatchKVCache.state` was bf16, not int8, in every capture. **RESOLVED (root cause found)** — the continuous-batching `BatchGenerator` path structurally cannot quantize the live KV cache; see "### RESOLVED — kv-cache-dtype is a no-op on the batch generation path" and worklist item 1 below. Relevant to memory-envelope estimates: model rapid-mlx live KV as bf16.
3. **Upstream reports (two distinct bugs), both still valid:** (a) `generation_error_recovery` swallows a mid-generation Metal `[metal::malloc]` crash into a clean 200 OK with zero usage and no `error` event; (b) full-attention prefill materializes an O(prefill_step × context) score buffer that overflows the Metal single-buffer cap — should stream/flash the prefill attention, or clamp `prefill_step_size` against `max_buffer_length` automatically.
4. `--use-paged-cache` is now **moot for this failure** (the KV cache was never the bottleneck); it does not address the attention-score buffer. Not worth testing for this issue.

## Outstanding investigation items (prioritized worklist, 2026-07-24 third session)

Ordered by "larger bugs the smaller models struggle with" first. Primary goal throughout: **coding + agentic tool-calling fidelity** for OpenCode-style OpenAI-endpoint hosting, with a secondary lower-precision RP/SillyTavern profile. Everything here must ultimately feed the VRAM estimator (`src/web/api/vram.rs`, `src/llama/vram_estimator/`) with correct MLX-quant + rapid-mlx-param math so the UI can guide users.

1. **RESOLVED — `--kv-cache-dtype` is structurally ignored on the continuous-batching (BatchGenerator) path (LARGER BUG, CONFIRMED 2026-07-24 third session).** The observed bf16 `BatchKVCache.state` under `--kv-cache-dtype int8` is not a mis-resolution — the batch path *cannot* quantize the live per-request KV cache at all. Full code trace below; see "### RESOLVED — kv-cache-dtype is a no-op on the batch generation path".

   **Impact:** `--kv-cache-dtype {int8,int4}` has **zero effect on the live generation KV cache** used for every request. It only affects (a) the observability banner / Prometheus `kv_cache_dtype` gauge, and (b) the cross-request `MemoryCacheConfig` prefix-reuse layer. In the benchmark harness (fresh server per cell, prefix cache disabled) it therefore has **zero memory or quality effect** — which is exactly why "int8 failures match int4" (obs 2569) and why the crash threshold was identical across dtypes.

   **Consequences for the VRAM estimator and worklist:**
   - Any KV-dtype-based VRAM math in `src/web/api/vram.rs` (`Bf16→f16`, `Int8→q8_0`) is **wrong for the rapid-mlx batch backend** — the runtime always stores bf16 KV regardless of the requested dtype. The estimator must model rapid-mlx live KV as bf16 (2 bytes/elem) until/unless this is fixed upstream, and should not advertise int8/int4 KV savings for rapid-mlx.
   - An int8-vs-int4 KV comparison on this backend is **not measurable** right now — do not spend a benchmark pass on it. (TurboQuant, item 2, is a *separate* mechanism and may still apply — verify independently.)
   - This is a third distinct upstream bug: the serve flag is accepted, logged as "selected/applied", and silently dropped on the hot path. Worth an upstream report alongside the two crash bugs.

2. **TurboQuant harness numbers (LARGER BUG).** Review how `scripts/rapid-mlx-benchmark-suite.mjs` requests and records `--kv-cache-turboquant` and how `src/web/api/rapid_mlx_runtime.rs` (build_effective_policy, lines ~470-635) gates/persists TurboQuant modes (V4/K8V4→Off fallback, qualification-receipt requirement). Determine whether the harness is measuring the *effective* TurboQuant mode or the *requested* one, and whether the receipts reflect what the runtime actually applied. Same silent-fallback risk as the KV-dtype issue.

3. **`prefill-step-size` UX/hints review (frontend + backend).** The harness now hardcodes 4096 (`scripts/rapid-mlx-benchmark-suite.mjs:DEFAULT_PREFILL_STEP_SIZE`) but the param is **not surfaced in `static/js` at all**, and the Rust runtime config exposes `prefill_batch_size` (in `rapid_mlx_runtime.rs`), not `prefill_step_size` — confirm whether these are the same serve flag or two different levers. Add proper user-facing hints: what it does (per-chunk prompt width; drives the O(prefill_step × context × heads) score buffer), when to lower it (high context on Apple Silicon to stay under `max_buffer_length`), the tradeoff (slower prefill), and a suggested formula/auto-clamp. Do NOT finalize a "best-practice default" until items 1-2 and context-scaling math are measured.

4. **Do NOT re-collect the 131k/196k cells yet** (per user, 2026-07-24) — that pass is for a later Sonnet session once the harness default (4096), the KV-dtype question, and the TurboQuant question are settled, so the receipts are correct on the first durable collection.

5. **Backend/frontend gap: `reasoning_max_tokens` generation-time thinking budget (NEW, upstream last 24h, not yet in our schema anywhere).** Confirmed via `gh` against `raullenchai/Rapid-MLX`: PR #1185 (2026-07-23, merged) adds a per-request decode-time thinking-token budget — the model is force-closed to a single `</think>` token once `reasoning_max_tokens` is spent, replacing the old post-hoc-trim-only behavior (reasoning compute is now actually bounded, not just relabeled in output). PR #1186 (2026-07-23) fixed a correctness regression where nested-LM wrappers (Qwen3.5, Gemma3n — i.e. our exact target models) silently fell back to the unbounded post-hoc path because the output-head-width probe missed the nested `language_model.*` path; PR #1190 documents the field. **None of `settings.rs`, `command.rs`, or `mod.rs` in `src/inference/rapid_mlx/` reference `reasoning_max_tokens`** (only `reasoning_mode`/`reasoning_effort`/`enable_thinking` exist), and no `static/js/features/*.js` file surfaces it either (grepped `presets.js`, `spawn-wizard.js`, `models.js`, `vram-estimate.js`). This is a genuinely missing per-request field distinct from the existing enum-style `reasoning_effort` — it needs: a numeric field threaded through the chat-request builder (mirroring how `reasoning_effort` is defaulted in `mod.rs::apply_reasoning_defaults`), and preset/spawn-wizard UI exposure (a numeric input, not a select) alongside the existing thinking-mode controls in `presets.js`/`spawn-wizard.js`. Add to the followup docs' backend/frontend worklists once scoped.

6. **Related upstream fix worth a targeted re-test post-upgrade: PR #1192 "reasoning-gated forced tool grammar via thinking budget"** (2026-07-24, merged, ships in 0.11.0). When `reasoning_max_tokens` is set alongside `tool_choice="required"`/named-function on a reasoning model, the forced-tool grammar mask now stays off until `</think>` closes (via the #1185 budget mechanism), then hard-constrains the tool-call argument JSON against the schema — previously reasoning models opted out of grammar constraint entirely for forced tool calls and only got a best-effort forced-prefix (argument JSON was unconstrained). This is directly relevant to `docs/plans/20260718-final_rapidmlx_followups_execution.md`'s recorded finding (line ~272) that "Qwen 3.5 9B Defiant has a corrected 32k-output/16k-reasoning stateful-tool receipt with a parsed call but incorrect initial call ordering; it is not agent-qualified" — that result predates this fix and should be re-run post-0.11.0-upgrade with `reasoning_max_tokens` set (not just `reasoning_effort`) before concluding whether the ordering defect still reproduces. The benchmark suite's `tools` cells already pass `reasoning_max_tokens: 16000` in `extra_body` (`scripts/rapid-mlx-benchmark-suite.mjs`), so this is measured by the existing plan, not a new harness change — just note it explicitly as a re-test trigger, not an assumed-fixed item.

### RESOLVED — kv-cache-dtype is a no-op on the batch generation path

Confirmed by reading the installed source (mlx_lm 0.31.3 / vllm_mlx, paths under `~/.local/share/uv/tools/rapid-mlx/lib/python3.11/site-packages/`). The chain:

1. **CLI resolves the dtype correctly.** `vllm_mlx/cli.py:3005` `resolve_kv_cache_dtype(...)` → `cli.py:3014` `quant, bits = dtype_to_quantization_bits(kv_cache_decision.dtype)`; sets `args.kv_cache_quantization = quant` (True for int8/int4) and `args.kv_cache_quantization_bits = bits`. Also stashes `kv_cache_dtype` on the ServerConfig purely for `/metrics`. So the *decision* is right — the server log "int8 selected (operator override)" is truthful about the decision.

2. **The decision reaches only the prefix-cache layer, not the generator.** `vllm_mlx/scheduler.py:2023` `elif self.config.use_memory_aware_cache:` builds a `MemoryCacheConfig(... kv_quantize=..., kv_bits=self.config.kv_cache_quantization_bits, ...)` (scheduler.py:2027-2030). That object governs the **cross-request prefix reuse cache**, not the live per-request KV cache.

3. **The live generation cache is built by `BatchGenerator`, which has no quantization knobs.** `vllm_mlx/scheduler.py:2449` assembles `bg_kwargs = dict(model, max_tokens, stop_tokens, sampler, prefill_batch_size, completion_batch_size, prefill_step_size)` — **no `kv_bits`, `kv_group_size`, or `quantized_kv_start`** — then `scheduler.py:2461` `BatchGenerator(**bg_kwargs, stream=...)`. The stock `mlx_lm.generate.BatchGenerator.__init__` (generate.py:1497) signature likewise has **no** kv-quantization parameters, and nothing in the `BatchGenerator` region (generate.py:1486-2000) calls `maybe_quantize_kv_cache`. (The lone `kv_bits` reference at generate.py:1981 is in the single-stream `generate()` / `--prompt-cache-file` guard, a different code path.)

4. **The cache objects *could* be quantized, but nothing triggers it.** `mlx_lm/models/cache.py` `BatchKVCache` (line 912) *does* have `to_quantized()` (line 1327), and `maybe_quantize_kv_cache` (generate.py:299) would call it once `offset >= quantized_kv_start` — but only the single-stream `generate_step`/`stream_generate` paths (generate.py:380, 538) ever call `maybe_quantize_kv_cache`. The continuous-batching path never does, so `BatchKVCache.state` stays a plain bf16 `(keys, values)` tuple for the whole request. This matches every live probe.

Note also: `vllm_mlx/utils/mamba_cache.py:216` `ensure_mamba_support()` is now a deliberate no-op (its `_patched_make_cache` broke hybrid Qwen3.5), so stock `mlx_lm._make_cache` runs; its `to_batch_cache` (mamba_cache.py:152) would also construct `BatchKVCache(left_padding)` with no quant args. Either way there is no quantization path for the live batch cache.

**Live-verify (for the next session, quick):** launch with `--kv-cache-dtype int4` and re-probe `BatchKVCache.state` at prefill — expect bf16 again, identical to int8/bf16. No need to re-run full benchmark cells to confirm.

**Terminology footnote for item 3:** `prefill_batch_size` (Rust `rapid_mlx_runtime.rs`) and `prefill_step_size` (harness `--prefill-step-size`) are **two different serve flags** — both appear side by side in `bg_kwargs` (scheduler.py:2454-2456), sourced from `config.prefill_batch_size` and `config.prefill_step_size` respectively. `prefill_batch_size` = how many sequences prefill together; `prefill_step_size` = per-chunk prompt width (the one that drives the O(prefill_step × context × heads) score buffer and the Metal-ceiling crash). The Rust runtime currently exposes only the former, not the crash-relevant latter — item 3 must add `prefill_step_size`.

## Definition of a finished evidence set

Do not state that Rapid is fit for the user's primary coding-agent workload until there are revision-pinned, fresh-server receipts proving the chosen KV/PFlash/cache configuration across the baseline contexts and direct tool-fidelity fixtures. Do not state a usable high-context envelope from model metadata. State only the highest successfully measured raw context for a specified configuration and distinguish: fit, performance, source retrieval, tool fidelity, image capability, and extrapolated estimates.
