# Model runtime benchmarking

Use `scripts/model-runtime-benchmark.mjs` to collect comparable local-runtime evidence. The framework is intentionally OpenAI-compatible rather than Rapid-specific: Rapid-MLX, llama.cpp, OMLX, and a future loader may use different flags, but each can expose the same measured request, latency, memory, and fidelity record.

## Quick start: one command, one report

`scripts/backend-ab-suite.mjs` is the easiest way to generate and read cross-backend data. It runs one or both backends' standard matrix, puts every receipt under one folder, and writes a single comparison table — no manual file-hunting or table-building required.

```bash
node scripts/backend-ab-suite.mjs run \
  --label qwen35-9b-2026-07-25 \
  --llama-cpp-model /Users/nick/.config/llama-monitor/models/gguf/Qwen3.5-9B-The-Defiant-Fable-Uncnr-Heretic-NEO-MAX-MTP-Q4_K_M.gguf \
  --rapid-mlx-model nightmedia/Qwen3.5-9B-DS9-USS-Defiant-1M-q8-hi-mlx
```

This writes:

```
tests/fixtures/calibration/ab-runs/qwen35-9b-2026-07-25/
  llama-cpp/   # 10 receipts + suite-index.json
  rapid-mlx/   # 10 receipts + suite-index.json
  report.md    # the combined comparison table, ready to read
```

Open `report.md`: one table, sorted by backend then batch/step then prompt size, with prompt tokens, raw prefill (PP) tok/s, time-to-first-token, generation (TG) tok/s, completion tokens, and fidelity side by side for every row from both backends.

**You do not need both backends every time.** Pass only `--llama-cpp-model` to re-check llama.cpp after a llama.cpp update, or only `--rapid-mlx-model` to check a Rapid-MLX release — the script (and the report it generates) works fine with just one backend's data. Other flags:

- `--rapid-mlx-suite SUITE` — which Rapid-MLX suite to run (default `ubatch`, the suite built to match llama.cpp's batch-size matrix 1:1). See "Rapid-MLX suite driver" below for the full suite list.
- `--llama-cpp-server PATH` — path to `llama-server`, if not the default in `DEFAULT_SERVER` inside `llama-cpp-benchmark-suite.mjs`.
- `--out-root DIR` — where runs are written (default `tests/fixtures/calibration/ab-runs`).
- `--resume` — skip cells that already have a valid receipt from a prior interrupted run (passed through to both backend drivers).

A run takes a while — each cell starts a fresh server, so expect roughly one to several minutes per cell depending on context size, times 10 cells per backend. It is safe to run in the background and check `report.md` once it finishes.

### Already have receipts and just want the table?

If you've already run one or both backend suites separately (see the sections below) and just want the combined report without re-running anything:

```bash
node scripts/model-runtime-benchmark.mjs compare \
  --dir tests/fixtures/calibration/llama-cpp-receipts/qwen35-9b-defiant-ubatch \
  --dir tests/fixtures/calibration/rapid-mlx-receipts/qwen35-9b-prefill-512 \
  --out /tmp/comparison.md
```

`--dir` is repeatable and accepts any number of receipt directories from either backend (or multiple runs of the same backend); every `*.json` file in each directory except `suite-index.json` is read and folded into one sorted table.

Start a loopback-only server per manifest. For peak-memory or fit evidence, use a **fresh server per cell** and set `runtime.fresh_server_per_cell` to `true`; otherwise Rapid's peak metric is a lifetime high-water mark and can only be reported as diagnostic context. Record the immutable model revision, runtime version, hardware, and exact argv in the manifest; do not infer a setting from a model family or UI default.

```bash
node scripts/model-runtime-benchmark.mjs run \
  --manifest tests/fixtures/calibration/model-runtime-benchmark.example.json \
  --base-url http://127.0.0.1:18086 \
  --out tests/fixtures/calibration/receipts/example.json

node scripts/model-runtime-benchmark.mjs report \
  --input tests/fixtures/calibration/receipts/example.json \
  --out /private/tmp/model-runtime-benchmark.md
```

`report` builds a single-backend table from one or more individual receipt files (`--input`, repeatable). `compare` (below) is the cross-backend equivalent: it reads whole receipt directories instead of individual files and adds a `Backend`/`Batch-Step` column so llama.cpp and Rapid-MLX rows land in one sorted table. Most day-to-day use should go through `backend-ab-suite.mjs` (Quick start, above) rather than calling `run`/`report`/`compare` by hand.

## Rapid-MLX suite driver

Do not hand-author a manifest for every Rapid setting. Use the suite driver when a model needs the standard capability matrix; it resolves the locally cached snapshot/config hash, materializes temporary pinned manifests, starts/stops one fresh Rapid server per measurement, and writes a receipt index.

```bash
node scripts/rapid-mlx-benchmark-suite.mjs run \
  --model unsloth/Qwen3.6-35B-A3B-UD-MLX-4bit \
  --suite context \
  --out tests/fixtures/calibration/rapid-mlx-receipts/unsloth-context
```

Suites: `smoke`, `context`, `pflash`, `cache`, `tools`, `image --image PATH`, and `all`. Use `plan` instead of `run` to print the generated cells/argv without loading a model. `--resume` preserves valid prior receipts and runs only missing cells; a `suite-index.json` is always written, including partial receipts and an error log tail if a server cell fails. The driver deliberately waits after every server shutdown for Metal/file-backed model pages to settle before starting the next fresh cell. It gives every server an isolated temporary home directory while retaining the real Hugging Face cache, so persistent prefix-cache entries cannot contaminate a cold cache row or overwrite user cache data. The driver is Rapid-specific; `model-runtime-benchmark.mjs` remains the runtime-neutral execution and reporting contract for llama.cpp, OMLX, and future backends.

The runner records actual server-reported prompt/completion tokens, raw prompt throughput (raw prompt tokens divided by TTFT), generation tokens per second, selected Prometheus metrics before/after each request, and a bounded fidelity result. When a runtime exports PFlash compressed-token metrics, the receipt also records compressed and retained-token estimates. Raw PP includes compression/scoring time; it is not interchangeable with a native full-prefill kernel rate. Context cells scatter five numeric `CHECK_*` markers across the corpus (10/30/50/70/90% of context) and score fidelity as graduated recall (`marker_recall.recall_rate`) instead of a single verbatim string match, so results stay meaningful across sampling/temperature variation and cannot be satisfied by pattern-completing repeated filler text. Cache cells may use `cold`, `repeat`, and `extension` phases. Tool cells can declare tools and an expected tool name; image cells should be added only after the runtime has independently passed an image-input smoke test.

### Cache qualification

For a coding-agent cache test, select the `cache` suite and run the persistent
`cold → repeat → follow-up → fork` sequence. It measures a real shared-history
branch, not a cosmetic suffix appended to one user message. For Rapid, use
`--cache-contexts`, `--cache-memory-mb`, `--cache-dtypes`, and (only for a
separate write-cost experiment) `--cache-disk-checkpoint-intervals`.

Keep Rapid disk checkpoints at `0` in performance rows. Current Rapid source
writes snapshots but does not automatically reload evicted retained entries;
it is not disk-backed prompt caching. Read [cache benchmark results](cache-benchmark-results.md)
before turning a receipt into an application recommendation.

## llama.cpp GGUF micro-batch matrix

`scripts/llama-cpp-benchmark-suite.mjs` runs fresh `llama-server` processes for the requested GGUF with exactly `-ngl 99 -fa on -ctk q8_0 -ctv q8_0 -np 1 --metrics --jinja`. Its fixed matrix compares physical micro-batches (`-ub`) **512** and **2048** at 32k, 65,536, 131,072, 160k, and 200k context.

```bash
node scripts/llama-cpp-benchmark-suite.mjs run \
  --model /Users/nick/.config/llama-monitor/models/gguf/Qwen3.5-9B-The-Defiant-Fable-Uncnr-Heretic-NEO-MAX-MTP-Q4_K_M.gguf \
  --out tests/fixtures/calibration/llama-cpp-receipts/qwen35-9b-defiant-ubatch

args=()
for receipt in tests/fixtures/calibration/llama-cpp-receipts/qwen35-9b-defiant-ubatch/[0-9]*.json; do args+=(--input "$receipt"); done
node scripts/model-runtime-benchmark.mjs report "${args[@]}" --out /private/tmp/qwen35-9b-ubatch-report.md
```

Use `plan` in place of `run` to inspect all ten exact server commands without loading the model. `--resume` preserves valid receipts and resumes an interrupted matrix. The code-corpus fixture leaves completion/template headroom below each `-c` hard limit; compare actual server-reported prompt tokens, not the nominal context size. The report compares raw PP, TTFT, TG, and marker recall. **Server RSS after** is a post-request OS sample, not an allocator high-water mark; compare it only between these fresh, otherwise-identical cells. `suite-index.json` lists all completed rows.

For the llama.cpp host-cache lane, add `--suite cache`. It fixes `-ub 512`,
Q8_0 K/V, Flash Attention, one slot, `-ctxcp 32`, and `-cms 8192`, then varies
only `--cache-ram-mib 0,8192` across the requested `--cache-contexts`. A
one-slot repeat/fork test measures live-slot reuse even at `-cram 0`; it does
not by itself qualify an extra host-cache reservation.

## Tool protocol traces

`workload.tools` sends real OpenAI function definitions. The initial response is scored for the declared `expected_tool_name` and `expected_tool_arguments` subset after streamed tool-call fragments are reassembled. To test stateful agent behavior, add `workload.tool_trace.steps`: each step injects a synthetic tool result after the preceding tool call, sends the resulting assistant-plus-tool history back to the server, and scores its required next function name and argument subset.

```json
"tool_trace": {
  "steps": [{
    "tool_name": "read_file",
    "tool_result": "export const answer = 42;",
    "expected_tool_name": "apply_patch",
    "expected_tool_arguments": {"path": "src/example.ts"}
  }]
}
```

This is protocol fidelity, not a text-pattern test: the model must emit a structured tool call, accept the injected `role: tool` result, and emit the expected next structured call. Use separate manifests for single call, sequential calls, null/empty arguments, large observations, retry/error result, and parallel calls. Receipts sanitize prompts and tool-result payloads to lengths/hashes; they never store source or tool contents.

`tests/fixtures/calibration/model-runtime-benchmark-profiles.json` defines the three standard profiles: `cold_context_baseline`, `primary_opencode_agent`, and `long_context_research`. The primary coding-agent tool profile uses a 32,768-token server/request output ceiling and `reasoning_max_tokens: 16000`; the short 128-token cap belongs only to bounded throughput/retrieval probes and is never tool-fidelity evidence. Materialize each server configuration as a separate manifest; do not put an int4 and int8 cell in one run unless the running server has actually been restarted between them.

## Required comparison discipline

- Compare int4 and int8 with the same revision, template, prompt, output cap, concurrency, cache policy, and fresh server state.
- Use one manifest per launched configuration. Assert the live `expected_metrics` labels for every Rapid cell (at minimum KV dtype and TurboQuant mode) so a copied manifest cannot benchmark an unintended default.
- Compare TurboQuant only with prefix cache enabled and `cold`, `repeat`, and `extension` rows. It changes retained reusable prefix snapshots; it cannot make a cold prompt or active generation cheaper.
- Compare PFlash with fixed prompt fixtures and score retrieval/code-detail fidelity. Record raw prompt tokens and PFlash compressed-token metrics separately; a compressed prompt is not equivalent to native context capacity.
- Treat a run as failed when the request returns a server error, produces no usage record, or fails its declared fidelity criterion. Do not include it in PP/TG averages.
- Treat a 32k output or 16k reasoning setting as a safety ceiling, not an assumed normal completion. The default primary-agent evidence rows are 128, 1k, 4k, and 8k output, with 16k/32k boundary rows run explicitly and separately. For a requested long output, set both `max_tokens` and `minimum_completion_tokens`. This catches a model/template that stops early instead of silently calling a 32k-output row successful. For a reasoning budget, pass the runtime/client-specific request fields through `workload.extra_body` and record them in the manifest.
- Keep actual measurements and mathematical extrapolations separate. The report’s 160k/200k values are straight-line estimates from matching configuration rows, never fit, quality, or safety claims. Peak-memory estimates require fresh-server-per-cell evidence.

For agentic settings, add identical revision-pinned template/tool-schema cells for int8 and int4: single call, sequential multi-turn calls, parallel calls where supported, null/empty arguments, large observations, retry/error handling, and cached-prefix reuse. Only direct parse/name/argument fidelity evidence can support an agentic KV recommendation.
