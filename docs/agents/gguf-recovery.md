# Experimental GGUF Recovery Validation

Phase 5.5 R2 is a developer-only research adapter. It does not add a UI, API route,
production model-inventory entry, or launchable Rapid-MLX model. Read the R2 checkpoint
in `docs/plans/20260715-gguf_to_mlx_conversion_research.md` before changing its profile,
worker, or dependency lock.

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
