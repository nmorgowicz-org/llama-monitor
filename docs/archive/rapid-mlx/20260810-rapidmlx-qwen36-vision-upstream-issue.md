# Rapid-MLX Qwen3.6 vision issue — upstream submission plan

Status: source audit complete. Do not submit automatically; copy the checked values below into Rapid-MLX's required **Bug Report** form after attaching the final, untruncated logs.

## Finding

This is a real upstream compatibility defect, not a missing optional dependency or a llama-monitor filename/introspection problem.

The managed environment is correctly provisioned:

- Rapid-MLX `0.12.7`
- Python `3.14.6`
- `mlx-vlm` `0.6.3`
- `mlx` `0.31.2`
- `mlx-lm` `0.31.3`
- Apple M5 Max, 64 GB (`Mac17,7`), macOS `26.5.1` (`25F80`)
- managed extras: `guided`, `vision`

Gemma 4 12B QAT served an image request with HTTP 200 in this same environment, proving that the vision extra and general VLM path work. Qwen3.6-35B-A3B and Qwen3.6-27B checkpoints have vision metadata/heads, but their GatedDeltaNet/linear-attention language backbone creates `ArraysCache` state. Explicit `--mllm` startup rejects that state before HTTP readiness because the current MLLM continuous-batching path assumes mergeable standard `KVCache`/`RotatingKVCache`. Qwen3.5-9B reproduces the same hybrid-cache failure.

## Source assessment and proposed fix

Sol's review of the installed Rapid-MLX source found a bounded, correctness-first upstream fix is feasible. Confidence is approximately **85%** for a single-active-request lane making Qwen3.6 vision work, versus **55%** for unrestricted concurrent hybrid batching without deeper scheduler work. This is larger than a model allowlist change, but it is not an unbounded scheduler rewrite.

Relevant areas:

- `vllm_mlx/mllm_cache_compat.py` — cache compatibility gate and error reporting.
- `vllm_mlx/engine/batched.py::_start_mllm` — probes the language-model cache during startup.
- `vllm_mlx/mllm_batch_generator.py` — creates request caches, merges/filter/extracts batched state, and advances `MLLMScheduler`.
- `vllm_mlx/api/utils.py`, `vllm_mlx/server.py`, `vllm_mlx/cli.py` — request limits, routing, and user-facing capability behavior.

`ArraysCache` in the pinned `mlx-lm 0.31.3` exposes the operations needed for a safe first implementation (`merge`, `filter`, `extend`, `extract`, `prepare`, and `finalize`). GPU-backed checks also merged mixed `[KVCache, ArraysCache]` request caches into `[BatchKVCache, ArraysCache]`, filtered a batch, and extracted an individual `ArraysCache`.

There is model-side evidence for singleton handling: `mlx_vlm.models.qwen3_5.language.Qwen3_5Model` already extracts a one-row batch cache, runs that row, and remerges it. Qwen3.6 similarly returns mixed cache layers (`ArraysCache(size=2)` for linear-attention layers and `KVCache()` for full-attention layers). That makes the current failure an intentional compatibility guard inherited from #354, not proof that the pinned cache implementation lacks the required primitives.

Recommended first upstream PR (single-active hybrid MLLM mode):

1. Accept `ArraysCache` in the MLLM compatibility layer and preserve its recurrent-state semantics; do not cast it to ordinary KV cache.
2. When a hybrid/`ArraysCache` model is selected, force `max_num_seqs=1`, `prefill_batch_size=1`, and `completion_batch_size=1` (or the equivalent effective policy), and queue additional requests rather than batching them. Preserve `max_concurrent_requests` as the waiting-queue bound.
3. Continue rejecting Mamba, quantized, and unknown cache types with an actionable capability error.
4. Emit an explicit startup warning that singleton hybrid compatibility mode is active.
5. Keep automatic hybrid-to-text routing unchanged initially; users opt in with explicit `--mllm`. Automatic vision routing should follow only after both Qwen3.6 targets pass live qualification.
6. Keep concurrent hybrid batching as a separate follow-up PR after correctness and throughput are measured.

Likely first-PR touch points are `vllm_mlx/mllm_cache_compat.py::first_incompatible_mllm_cache_type`, `vllm_mlx/engine/batched.py::_probe_mllm_cache_type` and `BatchedEngine._start_mllm`, and `vllm_mlx/mllm_batch_generator.py::MLLMBatchGenerator._process_prompts`. Add focused coverage in `tests/test_mllm_hybrid_probe.py` and `tests/test_mllm_batch_generator.py` (or equivalent existing test modules). A follow-up routing PR would then examine `vllm_mlx/api/utils.py`, `vllm_mlx/server.py`, and `vllm_mlx/cli.py` after live qualification.

Required first-PR tests include real `ArraysCache` acceptance; mixed-cache merge/filter/extract; forced single-active limits; a queued second request; cancellation; streaming and non-streaming output; explicit `--no-mllm`; Gemma regression; and live image requests for both a Qwen3.6 and a control model. Automatic Qwen3.6 routing should be covered by the follow-up routing PR, after the explicit lane is qualified. Removing the startup guard without these tests is not an acceptable fix.

With a Sol/Opus-level implementation agent working continuously, the bounded PR is medium effort: approximately 3–6 active hours for code and unit tests, 4–8 hours for two-model live qualification/debugging, and 1–3 hours for documentation/review fixes (roughly 8–17 hours total, normally 1–2 uninterrupted working days). Runtime startup, model downloads, GPU generation, and maintainer review remain wall-clock costs and do not become instant just because a stronger model is used. Fully concurrent hybrid MLLM batching is a separate large effort, approximately 2–5 engineering days with Sol/Opus, plus sustained-load qualification and likely review iterations. Main risks are Qwen-specific recurrent-state or image-position shapes exposed only during generation, differing image-token lengths, prefix-cache reuse, and memory pressure. Upstream PR #115's hybrid corruption report is why the first PR must serialize active hybrid requests.

Upstream context already found:

- Issue/PR #352/#354: explicitly reject hybrid `ArraysCache` in `--mllm`.
- PR #1178: automatic text-only fallback; explicit `--mllm` remains fail-closed.
- PR #1277: accepts native `mlx-vlm` KV-cache classes, but does not make `ArraysCache` compatible.
- No open PR matching `ArraysCache`, hybrid MLLM, or Qwen3.6 vision support was found during the 2026-08-10 review.

## Copy/paste Bug Report form values

Use Rapid-MLX's repository **Bug Report** form (`.github/ISSUE_TEMPLATE/bug_report.yml`), not a blank issue, Performance Issue, or Feature Request form. The form requires all fields marked required below.

### Title

```text
Qwen3.6 hybrid vision models fail explicitly in --mllm because ArraysCache cannot enter MLLM batching
```

### Rapid-MLX version (required)

```text
0.12.7
```

### Last version where it worked (optional)

```text
Unknown — first verified managed guided+vision profile
```

### Hardware (required)

```text
Apple M5 Max, 64 GB (Mac17,7)
```

### macOS version (required)

```text
macOS 26.5.1 (25F80)
```

### Python version (optional)

```text
Python 3.14.6
```

### Model (required)

Lead with one canonical model/revision per issue. Recommended primary:

```text
nightmedia/Qwen3.6-35B-A3B-Fable-Holo3.1-mxfp4-mlx
revision: 16279aa65cee814c6b23e068a71eec7e1617fae0
```

Additional reproduction (include only if the form permits):

```text
nightmedia/Qwen3.6-27B-Architect-Polaris2-Fable-B-F451-Tess-mxfp4-mlx
revision: 721c71607072ecc0f0904db862d64ea1d0ac59fb
```

Qwen3.5-9B is a useful hybrid control. If the maintainer requires a canonical upstream alias, repeat the run with `mlx-community/Qwen3.6-35B-A3B-4bit` and replace the model/revision evidence with that exact snapshot.

### Full serve command (required)

```shell
rapid-mlx serve /path/to/the/pinned/snapshot \
  --host 127.0.0.1 \
  --port 18001 \
  --mllm \
  --max-tokens 16
```

Replace the path with the immutable local snapshot used for the log. Include the exact revision if the launcher supports a revision argument.

### Streaming or non-streaming? (required)

```text
N/A — this is a startup compatibility failure; the explicit --mllm server exits before HTTP readiness.
```

### What happened? What did you expect? (required)

```text
Actual: With rapid-mlx[guided,vision] installed, explicitly launching Qwen3.6-35B-A3B with --mllm exits during FastAPI lifespan startup. The checkpoint contains vision metadata/heads, but its GatedDeltaNet/linear-attention language backbone creates ArraysCache state. The MLLM continuous-batching path currently accepts only standard KVCache/RotatingKVCache state and rejects the model before the port becomes usable. The same failure reproduces with Qwen3.6-27B and Qwen3.5-9B hybrid controls. A non-hybrid Gemma 4 12B QAT control reaches readiness and returns HTTP 200 for an image request in the same environment.

Expected: A vision-capable Qwen3.6 checkpoint should either serve image requests through a hybrid-aware MLLM implementation (a correctness-first single-active fallback is acceptable initially) or report a clear, supported capability state. Installing the vision extra should not leave modern Qwen vision models unusable in the primary vision path.
```

### Minimal reproduction (required)

```text
1. Install the exact managed profile:
   uv tool install --no-config 'rapid-mlx[guided,vision]==0.12.7' --link-mode copy --no-progress --no-color
2. Start the pinned Qwen3.6-35B-A3B snapshot:
   rapid-mlx serve /path/to/Qwen3.6-35B-A3B-snapshot --host 127.0.0.1 --port 18001 --mllm --max-tokens 16
3. Observe that startup exits before HTTP readiness with the ArraysCache/hybrid continuous-batching error.
4. As a positive control, run Gemma 4 12B QAT with the same --mllm flags and submit one non-streaming image request; it reaches ready and returns HTTP 200.
```

### Server logs / error output (required)

Attach complete, untruncated stderr/stdout from the run. Preserve the detected model family, exact revision, effective `--mllm`, dependency versions, and cache class; redact tokens, usernames, and private paths. The diagnostic line is of this form (use the exact emitted text in the issue):

```text
RuntimeError: '<model snapshot>' hybrid/linear-attention (ArraysCache), incompatible with --mllm continuous batching (requires standard KVCache or RotatingKVCache).
```

## Submission checklist

- [ ] Confirm the primary model and immutable revision in the `model` field.
- [ ] Attach full startup logs, not only the final exception.
- [ ] Include the managed dependency receipt: `mlx-vlm==0.6.3`, `mlx==0.31.2`, `mlx-lm==0.31.3`.
- [ ] Include the successful Gemma HTTP-200 image-request control.
- [ ] Link issue #352 and PRs #354, #1178, and #1277.
- [ ] Ask for a hybrid-aware scheduler with an explicit single-active fallback; do not propose simply deleting the startup guard.
- [ ] Submit through the required Bug Report workflow/template and wait for maintainer triage.
## Upstream PR #1798 review (2026-08-10)

Rapid-MLX PR [#1798](https://github.com/raullenchai/Rapid-MLX/pull/1798) is merged (merge commit `b1b1fa7ac36818562780b8fb54b8557c63933225`) with title `fix(mllm): serve hybrid vision models through a serialized lane`. This supersedes the initial audit note that no matching PR was open. Its implementation matches the bounded fix proposed above:

- admits only the concrete `mlx-lm` `ArraysCache` when the compatibility lane is enabled;
- forces `max_num_seqs`, prefill batch size, and completion batch size to `1` while retaining the waiting-queue admission cap;
- preserves fail-closed behavior for Mamba, quantized, and unknown cache types;
- exercises concrete `ArraysCache` merge/filter/extract primitives and claims a real Qwen3.5-9B image request plus queued/cancelled-request checks.

Qualification remains incomplete for this plan: the PR's real-weight run used Qwen3.5-9B, not either pinned Qwen3.6 checkpoint, because the author's 18-GB runner could not safely load the larger model. GitHub's green Apple-Silicon smoke job used Qwen3.5-4B in the text-only `--no-mllm` lane, so it is not an independent vision validation. The PR has no maintainer review; an automated scorecard marked its targeted-test command `exit 2` while still reporting the check as PASS.

Release status: PyPI/latest Rapid-MLX release is `0.12.8` (uploaded before PR #1798 merged), so the fix is not yet available in the managed `0.12.7`/`0.12.8` binaries. Re-test the exact Qwen3.6-27B and Qwen3.6-35B snapshots against a release containing the merge before closing Phase 14.5. Do not submit the original bug report as an unresolved defect unless that release test still fails; retain the report fields as a regression/qualification fallback.

## Local source validation (2026-08-10)

Fetched upstream `main` at `f0d82d97833c15126154f0207594ceb6e7c8b8f5` into the isolated checkout `/private/tmp/rapid-mlx-upstream-review`. With the installed vision stack (`mlx==0.31.2`, `mlx-lm==0.31.3`, `mlx-vlm==0.6.3`) and GPU access:

- `tests/test_mllm_hybrid_probe.py`: **14 passed**;
- MLLM batch-generator/continuous-batching selection: **82 passed, 3 pre-existing numerical failures** (the same three failures reproduce on the pre-PR base commit);
- MLLM cancellation/cache/core suites: **54 passed, 6 deselected**.

The locally cached `~/.config/llama-monitor/models/mlx/native/nightmedia-27b-mxfp8-mlx` snapshot is Qwen3.6-27B (`model_type=qwen3_5`, `Qwen3_5ForConditionalGeneration`, vision tower present; 27 GB). Running upstream `main` against this real snapshot with `--mllm --prefill-step-size 2048 --max-num-seqs 1 --prefill-batch-size 1 --completion-batch-size 1 --disable-prefix-cache --pflash off --cache-memory-mb 4096 --gpu-memory-utilization 0.75` produced:

- startup success with the expected `ArraysCache` serialized-lane warning and effective `max_num_seqs=1`, `prefill_batch=1`, `completion_batch=1`;
- grounded image request HTTP 200 with a visual description;
- two concurrent image requests both HTTP 200, completing sequentially (about 9.8 s and 17.9 s, total about 18.0 s), demonstrating the singleton lane queues the second request.

The exact 35B Nightmedia MLX snapshot should receive the same live check when selected; no managed runtime was overwritten.

## Config-dir materialization and validation (2026-08-10)

The preferred Nightmedia 35B snapshot was copied (not removed from the HF
cache) into the llama-monitor model tree as a self-contained model:

```text
/Users/nick/.config/llama-monitor/models/mlx/native/nightmedia-35b-mxfp4-mlx
```

The copy dereferenced the Hugging Face snapshot's blob symlinks. It is 18 GB,
contains four safetensors shards plus the tokenizer/config/processor files,
and has no symlinks. Upstream Rapid-MLX `main` with the `vision` extra loaded
this exact config-dir path successfully using the serialized hybrid lane;
`GET /v1/models` reported `modality: image` and capabilities `text`,
`vision`, and `tools`. The temporary validation server was stopped after the
health check. The original HF snapshot remains intact.

## VLM prefill qualification (2026-08-10)

The existing text-only `512` recommendation should not be applied blindly to
image requests. With the Nightmedia 35B model and a representative captured UI
image (1,283 prompt tokens), `--prefill-step-size 1024` correctly rejected the
request because the image prompt exceeded the per-batch cap. The same request
completed HTTP 200 at `--prefill-step-size 1536` (16 generated tokens in 3.28 s).

This supports a VLM-specific recommendation of `1536` as the initial tier,
with `2048` as the next escalation for larger screenshots/prompts. `512`
remains the text-only default. The estimator, Spawn Wizard, and Preset Editor
should use the model's verified multimodal capability when selecting this
default; the cross-surface implementation is recorded below.

## Cross-surface default implementation (2026-08-10)

Rapid-MLX now keeps the generic text default at `512` while applying `1536`
automatically when the live model profile confirms `has_vision_tower`. This
behavior is shared by the Spawn Wizard, Preset Editor, and VRAM-estimate policy
builder; an explicit user-selected prefill value is preserved. The stale
Rapid-MLX mmproj qualification note was removed, and the runtime comparison now
records PR #1798's serialized hybrid vision lane and the Qwen3.6 live evidence.

The JS module baseline updater now serves an ephemeral static asset server when
`LLAMA_MONITOR_UI_URL` is not supplied, so baseline refresh no longer requires
a running llama-monitor instance.
