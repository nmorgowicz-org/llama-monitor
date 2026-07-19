# Experimental GGUF Recovery Validation

Phase 5.5 exposes the validated R2 adapter through the Experimental GGUF Import Lab.
The UI and authenticated API can inspect GGUFs and run the one exact SmolLM2 recovery
profile, but no recovered or re-quantized cache is launchable. Read the R2–R4 checkpoints
in `docs/plans/20260715-gguf_to_mlx_conversion_research.md` before changing a profile,
worker, dependency lock, job contract, or inventory rule.

## Immutable boundaries

- Corpus: `~/.config/llama-monitor/models/experimental/import-lab/fixtures/`
- Experimental outputs: `~/.config/llama-monitor/models/rapid-mlx/imports/`
- Converter environment: `~/.config/llama-monitor/runtimes/gguf-recovery/r2-v1/venv`
- Pinned Rapid-MLX qualification environment:
  `~/.config/llama-monitor/runtimes/rapid-mlx/.staging/0.10.10-qualification/venv`

Do not point the worker at the default Hugging Face cache, discover a converter from
`PATH`, write into an existing output, or change `launchable` to true. A profile or
worker change intentionally changes cache and worker-asset identities.

## Normal validation

Run one real tier at a time; the test is ignored by default because it requires the
local pinned corpus and app-owned Python environment:

```bash
rtk env LLAMA_MONITOR_R2_TIER=f16 cargo test --lib models::gguf_recovery::tests::real_smollm2_r2_selected_tier -- --ignored --exact --nocapture
rtk env LLAMA_MONITOR_R2_TIER=q8_0 cargo test --lib models::gguf_recovery::tests::real_smollm2_r2_selected_tier -- --ignored --exact --nocapture
rtk env LLAMA_MONITOR_R2_TIER=q6_k cargo test --lib models::gguf_recovery::tests::real_smollm2_r2_selected_tier -- --ignored --exact --nocapture
rtk env LLAMA_MONITOR_R2_TIER=q4_k_m cargo test --lib models::gguf_recovery::tests::real_smollm2_r2_selected_tier -- --ignored --exact --nocapture
```

Use `tools/gguf_recovery/validate_tensor_parity.py` with the hash-locked recovery Python
to compare each emitted `fp16/model.safetensors` to the authoritative BF16 weights.
The script must report 272 tensors. Preserve the full JSON as verification evidence.

## Detached Apple-Silicon runtime gate

Run this gate once from a normal interactive Terminal, outside Codex or another
headless/sandboxed executor. Use only the final-profile F16 cache:

```text
~/.config/llama-monitor/models/rapid-mlx/imports/a21cca76ec236c3c71ea2bf5eb6f78716602b90fb16d78c3aef4da51e1ff4177/fp16
```

Required evidence:

1. From the pinned `0.10.10` environment, call `mlx_lm.load()` on that exact directory
   and record a bounded success/error log plus elapsed time.
2. Start its `rapid-mlx serve` on a free loopback port with telemetry disabled, an
   ephemeral API key, served name `r2-smollm2-f16`, and an explicit request timeout.
3. Prove `/health/ready`, authenticated `/v1/models`, `/v1/status`, and
   `/v1/cache/stats`; then send the same chat request twice with `temperature: 0`,
   `seed: 55`, and a 32-token bound. Record complete text, finish reason, usage, and
   first-token/top-k logprobs when the API exposes them.
4. Stop the server and prove its listener and process are gone.
5. Start `~/.config/llama-monitor/bin/llama-server` against the pinned source
   `SmolLM2-135M-Instruct-F16.gguf` on a different loopback port. Send the identical
   chat template, messages, greedy parameters, seed, and token bound. Record the same
   evidence and stop it cleanly.

Do not accept “the model loaded” as semantic validation: the pre-fix scrambled model
loaded successfully. Compare deterministic output and compatible first-token logits.
Any missing chat-template parity, unexplained tokenization difference, nonsensical
output, crash, hang, or non-finite score keeps R2 open. Never overwrite or delete a
cache as part of this runtime procedure.

### Recorded R2 result (2026-07-16)

The one-time gate passed for cache
`a21cca76ec236c3c71ea2bf5eb6f78716602b90fb16d78c3aef4da51e1ff4177` and
Rapid-MLX `0.10.10`. Pinned mlx-lm loaded `Model` and `TokenizerWrapper` in 0.955
seconds. Rapid-MLX readiness, model discovery, status, cache statistics, and two
deterministic greedy requests passed; llama-server completed the matching request
against the pinned F16 GGUF. Both backends used 41 prompt and 32 completion tokens and
returned the same ordered first-token top five (`The`, `\"`, `In`, `A`, `When`), with
a maximum absolute logprob delta of approximately 0.0209. Both texts were coherent;
later greedy tokens diverged across the MLX and GGML kernels, with Rapid-MLX using its
default int4 KV cache. All owned process groups stopped and both listeners closed.

The independent Verifier accepted this as compatible semantic evidence under the R2
contract. The bounded report/logs are retained locally at
`/tmp/llama-monitor-r2-host-gate/`. Do not rerun this gate unless the cache, converter
profile, pinned runtime, chat template, or semantic acceptance contract changes.

Use two normal Terminal tabs. First prove load-only behavior:

```bash
rtk proxy /Users/nick/.config/llama-monitor/runtimes/rapid-mlx/.staging/0.10.10-qualification/venv/bin/python -I -c 'from mlx_lm import load; p="/Users/nick/.config/llama-monitor/models/rapid-mlx/imports/a21cca76ec236c3c71ea2bf5eb6f78716602b90fb16d78c3aef4da51e1ff4177/fp16"; model,tokenizer=load(p); print(type(model).__name__,type(tokenizer).__name__,flush=True)'
```

Then start Rapid-MLX in the first tab. Replace `R2_REPLACE_WITH_RANDOM_KEY` with a
new random value used only for this probe:

```bash
rtk env RAPID_MLX_TELEMETRY=0 /Users/nick/.config/llama-monitor/runtimes/rapid-mlx/.staging/0.10.10-qualification/venv/bin/rapid-mlx serve /Users/nick/.config/llama-monitor/models/rapid-mlx/imports/a21cca76ec236c3c71ea2bf5eb6f78716602b90fb16d78c3aef4da51e1ff4177/fp16 --served-model-name r2-smollm2-f16 --host 127.0.0.1 --port 18082 --api-key R2_REPLACE_WITH_RANDOM_KEY --timeout 60 --max-tokens 64 --log-level INFO
```

Run each probe from the second tab, substituting the same key:

```bash
rtk curl -s http://127.0.0.1:18082/health/ready
rtk curl -s -H 'Authorization: Bearer R2_REPLACE_WITH_RANDOM_KEY' http://127.0.0.1:18082/v1/models
rtk curl -s -H 'Authorization: Bearer R2_REPLACE_WITH_RANDOM_KEY' http://127.0.0.1:18082/v1/status
rtk curl -s -H 'Authorization: Bearer R2_REPLACE_WITH_RANDOM_KEY' http://127.0.0.1:18082/v1/cache/stats
rtk curl -s -H 'Authorization: Bearer R2_REPLACE_WITH_RANDOM_KEY' -H 'Content-Type: application/json' -d '{"model":"r2-smollm2-f16","messages":[{"role":"user","content":"Explain why the sky is blue in one short sentence."}],"temperature":0,"seed":55,"max_tokens":32,"stream":false,"logprobs":true,"top_logprobs":5}' http://127.0.0.1:18082/v1/chat/completions
```

Stop Rapid-MLX with Control-C. Start llama-server in the first tab:

```bash
rtk proxy /Users/nick/.config/llama-monitor/bin/llama-server -m /Users/nick/.config/llama-monitor/models/experimental/import-lab/fixtures/smollm2-135m-v1/gguf/SmolLM2-135M-Instruct-F16.gguf --host 127.0.0.1 --port 18083 --jinja
```

Send the matching request from the second tab, then stop llama-server with Control-C:

```bash
rtk curl -s -H 'Content-Type: application/json' -d '{"model":"r2-smollm2-f16","messages":[{"role":"user","content":"Explain why the sky is blue in one short sentence."}],"temperature":0,"seed":55,"max_tokens":32,"stream":false,"logprobs":true,"top_logprobs":5}' http://127.0.0.1:18083/v1/chat/completions
```

## Promotion rule

R2 is complete only after the detached runtime evidence and an independent Verifier
approve the full code, corpus provenance, failure semantics, tensor evidence, and
cleanup state. R2 completion authorizes R3 research only; it does not authorize a
production import flow or a `Verified` architecture profile.

## R3 optional MLX re-quantization

R3 accepts only the final remediated R2 F16 cache. It uses official pinned mlx-lm to
produce three uniform affine/group-64 experimental recipes. Keep the artifact labels
distinct:

```text
original: GGUF F16
recovery: MLX-compatible FP16 safetensors, R2 cache a21cca76...
output:   MLX quantized safetensors, affine_4bit_g64 / affine_6bit_g64 / affine_8bit_g64
```

Run one structural recipe at a time only when its pinned environment and R2 cache
exist:

```bash
rtk env LLAMA_MONITOR_R3_RECIPE=affine_4bit_g64 cargo test --lib models::gguf_recovery::tests::real_smollm2_r3_selected_recipe -- --ignored --exact --nocapture
rtk env LLAMA_MONITOR_R3_RECIPE=affine_6bit_g64 cargo test --lib models::gguf_recovery::tests::real_smollm2_r3_selected_recipe -- --ignored --exact --nocapture
rtk env LLAMA_MONITOR_R3_RECIPE=affine_8bit_g64 cargo test --lib models::gguf_recovery::tests::real_smollm2_r3_selected_recipe -- --ignored --exact --nocapture
```

Results remain non-launchable below
`~/.config/llama-monitor/models/rapid-mlx/requantized/`. They appear in inventory with
first-class `Experimental` and recipe badges but no supported backend or launch action.
Never copy one into the normal MLX model directory or change its manifest to launchable.

### One-time detached R3 host gate

Direct Python launched from a headless/sandboxed executor may have no Metal device.
After all three structural caches exist, open a normal interactive Terminal in the
repository and run exactly once:

```bash
rtk proxy /Users/nick/.config/llama-monitor/runtimes/rapid-mlx/.staging/0.10.10-qualification/venv/bin/python -I tools/mlx_requantize/run_host_gate.py --output /tmp/llama-monitor-r3-host-gate/report.json
```

No supplied secret is needed. The harness creates a different ephemeral API key for
each loopback Rapid-MLX process, never writes keys to its report, forces offline and
telemetry-disabled execution, bounds subprocess/HTTP/report data, and stops process
groups and listeners between targets. It evaluates:

- managed llama-server with the pinned source F16 GGUF;
- Rapid-MLX with recovered FP16 and each 4/6/8-bit R3 output;
- exact dequantized tensor fidelity of each recipe against recovered FP16;
- load latency, size, readiness, identity, telemetry/cache state, deterministic greedy
  text, usage, finish reason, first-token top-five, and completion throughput.

Review every generated text for coherence; the report records that human review is
required. Any missing tensor, non-finite value, load/readiness error, unstable greedy
response, incoherent output, unexplained first-token/logprob drift, timeout, cleanup
failure, or unbounded diagnostic keeps R3 open.

### Recorded R3 result (2026-07-16)

The one-time host gate completed successfully. All four Rapid-MLX targets were
deterministic and coherent; the 4-bit output remained grammatical at the token limit,
6-bit produced a concise natural stop, and 8-bit matched recovered FP16 text exactly.
Fidelity improved monotonically from minimum cosine `0.993535` (4-bit), to `0.999611`
(6-bit), to `0.999955` (8-bit). Every recipe kept exact 272-tensor dequantized key and
shape closure. The 8-bit ordered top-five matched recovered FP16; 6-bit retained all
five with a different order; 4-bit retained the winner and four shared candidates.

Measured completion rates were 218.40 tok/s for recovered FP16, 188.25 for 4-bit,
174.03 for 6-bit, 192.98 for 8-bit, and 304.29 for llama-server's source F16 GGUF.
This is one cold sequential 135M-model sample: quantization overhead dominates at this
size, so it is not evidence that larger quantized MLX models are slower. Do not rerun
unless the quantizer, profile, cache, runtime, prompt, or acceptance contract changes.
The complete report remains at `/tmp/llama-monitor-r3-host-gate/report.json`.

R3 evidence is consumed by the R4 Import Lab and inventory surfaces. It never changes
an R2/R3 cache to launchable and never promotes this profile to `Verified`.

## R4 application contract

- The compatibility preview is converter-free. Recovery starts only after an exact
  profile match and a separate resource estimate.
- Resource estimates use a fail-fast two-worker blocking pool. Cheap path-syntax
  validation runs before pool admission; saturation returns `429`, and a timed-out
  blocking task retains its permit until the underlying work actually exits.
- Enqueue validates the library-relative regular GGUF and every path component before
  returning `202`; traversal and symlinks never become asynchronous jobs.
- One in-process worker runs at a time. Job records are bounded to 32 and diagnostics
  to 24 entries of 512 characters. Worker failures are mapped to stable, path-free
  public diagnostics. They are operational status, not durable state.
- Cancellation uses the R2 process-group cleanup path. Published caches are atomic;
  `.staging` content is never inventory.
- `rapid-mlx/imports/*/fp16` and `rapid-mlx/requantized/*/model` enter inventory only
  after strict validation of a zero-byte completion marker, bounded non-symlink typed
  manifest, exact embedded identities, validation-report hash, and the complete
  recursive published-file hash closure. Inventory exposes sanitized provenance, never
  raw manifest fields. Their source and lineage badges remain visible on every platform,
  with no runtime action.
- Local recovery is compiled and enabled only for Apple Silicon macOS. Compatibility
  inspection, inventory, and remote workflows remain cross-platform.
- Platform information is shared across frontend surfaces. R4 has no runtime-install or
  platform-changing mutation to invalidate it; the explicit refresh path is reserved
  for the Phase 6 installation workflow.
