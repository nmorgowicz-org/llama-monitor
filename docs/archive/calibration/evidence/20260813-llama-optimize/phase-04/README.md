# Calibration Phase 4 — bounded funnel receipt

Date: 2026-08-14

## Final real-model macOS Balanced run

An isolated release-built `local-llm-foundry` instance ran the production
calibration API against:

```text
Qwen3.5-9B-The-Defiant-Fable-Uncnr-Heretic-NEO-MAX-MTP-Q4_K_M.gguf
```

Run configuration:

- managed `llama-bench` sibling, build `cb26014d9` (build number `10310`);
- q8/q8 KV cache (`-ctk q8_0 -ctv q8_0`);
- preset flash-attention unset, preserved as runtime `auto` (`-fa auto`);
- thinking workload, 1,024 prompt tokens and 4,096 generation tokens;
- minimum context 8,192, one request, Balanced budget;
- batch search values `512`, `1024`, `1536`, `2048`, and `4096`;
- every candidate enforced `ubatch_size <= batch_size`.

The planner produced 17 trials: the baseline, nine L9 rows, five explicit
batch-coverage trials, and two deterministic verification reruns. All 17
completed with structured JSON and three retained prompt/decode samples each.
No trial timed out, OOMed, returned malformed output, or exited non-zero.

The regenerated receipt contains non-default hardware, model, and runtime
fingerprints from the managed binaries, GGUF header, host, and workload. Raw
job artifacts remain in the disposable calibration workspace outside the
repository; job ID: `2e10e9b3cf1b6a6f8853b030`.

## Result

The receipt selected the baseline for both the fastest and Balanced picks. No
derived candidate demonstrated a trustworthy improvement over the baseline
under this workload. The Pareto set contains `baseline` and
`balanced-l9-r05`; the latter is retained as a measured Pareto point, not an
automatic recommendation.

| Result | Value |
|---|---|
| Baseline median decode | `56.0894 tok/s` |
| Selected candidate | `baseline` |
| Selected sample count | `3` |
| Pareto set | `baseline`, `balanced-l9-r05` |
| High-noise candidates | `baseline`, `balanced-l9-r05`, `balanced-batch-4096` |
| Receipt state | `complete` (`17/17`) |

This is valid negative evidence for this exact macOS hardware, runtime, model,
q8/q8 cache mode, and thinking workload. It must not be generalized as a
universal tuning recommendation. A future model or workload should generate a
new fingerprinted receipt.

## Integrity validation

Post-run validation confirmed:

- all 17 candidate IDs equal their measurement `trial_id`;
- all measurements have status `ok`, three prompt samples, and three decode
  samples;
- all bounded diagnostic arrays are empty;
- no `timeout`, `EOF`, OOM, malformed-output, or non-zero-exit diagnostics;
- all explicit batch candidates satisfy `ubatch_size <= batch_size`;
- the preset and active session remained unchanged throughout the job;
- hardware, model, and runtime fingerprint fields are populated and stable.

## Cache/flash compatibility

The managed macOS bundle accepts q8/q8 models when flash attention is `auto`
or enabled. Forcing `-fa off` failed context creation for both the 9B Qwen3.5
fixture and the larger 26B Gemma fixture, including `-b 512 -ub 512`.
Calibration therefore preserves the preset's three-state flash setting rather
than sweeping or silently changing it.

## Scope and deferred gates

The bounded 2.0 source-side macOS Balanced runtime gate is complete for the
expanded planner. Thorough search, Morris screening, automatic context-ceiling
probing, server-driver correctness qualification, and native Windows execution
remain explicit later gates.

## Reproduction runbook

This is the canonical recipe for repeating the receipt on macOS or Windows.
Use repository-relative paths in notes and substitute disposable local paths
for `<config-dir>`, `<model-library>`, and `<managed-bundle>`.

1. Build the release binary with `cargo build --release`.
2. Start one isolated monitor on a disposable loopback port:

   ```text
   target/release/local-llm-foundry --port <port> \
     --config-dir <config-dir> \
     --models-dir <model-library> \
     --presets-file <config-dir>/presets.json \
     --llama-server-path <managed-bundle>/llama-server \
     --llama-server-cwd <managed-bundle>
   ```

   Windows uses `target\\release\\local-llm-foundry.exe`,
   `<managed-bundle>\\llama-server.exe`, and PowerShell line continuations.
   Do not reuse port `7778` or a live user monitor.

3. Fetch the runtime API token from `GET /api/internal/api-token`; keep it in
   memory or a disposable file and never put it in a receipt or log.
4. Run `POST /api/calibrations/preflight` with the selected preset and this
   workload:

   ```json
   {
     "preset_id": "<preset-id>",
     "budget": "balanced",
     "workload": {
       "kind": "thinking",
       "prompt_tokens": 1024,
       "generation_tokens": 4096,
       "parallel_requests": 1,
       "minimum_context": 8192,
       "objective": "balanced",
       "fixture_id": "calibration-v1-thinking-qwen35"
     }
   }
   ```

5. Start `POST /api/calibrations` using the preflight's
   `preset_fingerprint`, `exact_confirmation: "CALIBRATE"`, and
   `allow_stop_active_server: false`. The response supplies the job ID.
6. Poll `GET /api/calibrations/<job-id>` until `state=complete`, then fetch
   `GET /api/calibrations/<job-id>/receipt`.
7. Validate all candidate/measurement IDs, `ok` statuses, three retained
   samples per measurement, empty bounded diagnostics, no timeout/OOM/EOF
   markers, `ubatch_size <= batch_size`, and non-default hardware/model/runtime
   fingerprint fields before closing the gate.

The Windows pass must repeat this procedure natively with the managed `.exe`
siblings and preserve the same q8/q8, flash `auto`, workload, and batch range;
it is a separate platform receipt, not an inference from the macOS result.
## Baseline semantics

`baseline` is the measured control: the selected preset after the product's
normalization rules have been applied for the calibration invocation. It is not
the universal llama.cpp default and can vary by OS, hardware, model, preset,
and managed bundle.

The receipt now records both sides explicitly:

- `baseline.effective`: context, threads, batch/ubatch, KV types, flash-attention
  mode, GPU layers, and CPU-MoE values actually used by the control. Each value
  carries a source (`preset`, `calibration_policy`, or a managed default).
- `baseline.llama_server_help_defaults`: explicit `(default: ...)` values parsed
  from the exact configured `llama-server --help` output. The receipt also
  stores the help hash, exit status, and truncation marker.

For this run, the important distinction is that an unset preset batch/ubatch is
normalized to `2048`/`512` for calibration, while the product KV policy uses
`q8_0` for K and `f16` for V. Those are measured-control values; they must not
be read as a claim that every llama.cpp build has those compiled defaults. The
UI presents the effective table beside the managed-help table for this reason.
For the managed macOS bundle used by this qualification, the observed help
defaults relevant to this sweep were `threads=-1`, `threads-batch=same as
threads`, `ctx-size=0 (loaded model)`, `batch-size=2048`, `ubatch-size=512`,
`flash-attn=auto`, `cache-type-k=f16`, `cache-type-v=f16`, and
`gpu-layers=auto`. The receipt remains authoritative for another bundle: if
that build advertises a different value, its help hash and parsed table travel
with the receipt.
## Baseline semantics (corrected wording)

`baseline` is the measured control: the selected preset after product
normalization for this calibration invocation. It is not a universal llama.cpp
default; it can vary by OS, hardware, model, preset, and managed bundle.

The receipt's `baseline.effective` table shows the values actually measured and
the source of each value. Its `baseline.llama_server_help_defaults` table shows
only explicit defaults parsed from the exact managed `llama-server --help`
output, alongside the help hash, exit status, and truncation marker. The UI
shows both tables together.

For this qualification, unset batch/ubatch normalized to `2048`/`512` for the
calibration control, while the product KV policy used `q8_0` for K and `f16`
for V. The managed macOS help advertised `threads=-1`,
`threads-batch=same as threads`, `ctx-size=0 (loaded model)`, `batch-size=2048`,
`ubatch-size=512`, `flash-attn=auto`, `cache-type-k=f16`, `cache-type-v=f16`,
and `gpu-layers=auto`. These are recorded per receipt so another bundle with
different help defaults remains self-describing.
## Qualification receipt interpretation

For receipt `2e10e9b3cf1b6a6f8853b030`, the measured baseline was the selected
preset's effective configuration: context `8192`, threads `6`, batch `512`,
ubatch `512`, KV `q8_0/q8_0`, flash attention `auto`, and GPU layers left at
the managed all-layers behavior. Those values describe that preset and run;
they are not substituted for the separate managed-help defaults table.
