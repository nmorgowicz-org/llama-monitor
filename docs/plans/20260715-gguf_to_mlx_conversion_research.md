# GGUF to MLX Conversion Research & Experimental Import Plan

| Field | Value |
|---|---|
| Created | 2026-07-15 |
| Status | Research plan; intentionally outside the Rapid-MLX release gate |
| Product goal | Let users recover valuable GGUF-only finetunes for MLX without presenting reverse conversion as lossless or universally safe |
| Initial host | Apple Silicon macOS |
| Verified native runtime baseline | Rapid-MLX `v0.10.10` current, `v0.10.9` retained rollback / mlx-lm |
| First upgrade qualification | Rapid-MLX `v0.10.10` at `5ca536275e89ddf0de3b49bd6f55fad80e42656e` |
| Reverse-converter research seed | Minimal llama-monitor fork of `barrontang/gguf2mlx` at audited SHA `6a0da6529f233df79362cbf62dd96221c895351f`; never run unmodified |

## Executive Decision

GGUF-only conversion remains a desired llama-monitor capability, but it is a separate
Experimental Import Lab rather than a prerequisite for the first Rapid-MLX release.

Phase 5.5 also owns one bounded runtime-upgrade qualification. Rapid-MLX `v0.10.10`
was published on 2026-07-15. Its short release page highlights release-artifact
acceptance hardening and the version bump, but the tag comparison contains 15 commits,
including inference/runtime changes. Release notes alone are therefore not compatibility
evidence. The lab must exercise an isolated managed `v0.10.9` -> `v0.10.10` upgrade
before using `v0.10.10` in GGUF conversion parity gates. Phase 6 still owns the
production updater and its app UI.

The supported model paths remain:

1. native MLX directory or revision-pinned MLX Hugging Face repository;
2. authoritative original/merged Hugging Face safetensors converted with pinned
   official `mlx_lm.convert` tooling;
3. experimental GGUF-only recovery after an architecture-specific profile passes all
   tensor, config, tokenizer, runtime-load, and behavioral-parity gates in this plan.

The application must never imply that expanding quantized GGUF tensors to FP16 restores
the precision removed by the original quantization. A later MLX quantization is a second
approximation with different block/group rules; it does not preserve GGUF K-quant or
importance-matrix semantics.

## Why This Work Is Separate

Rapid-MLX `v0.10.9` does not load GGUF and does not ship a reverse converter. Official
mlx-lm converts authoritative Hugging Face/local safetensors into MLX and can export in
the other direction to GGUF, but it does not reverse arbitrary GGUF into a loadable HF
model.

Reverse conversion must reconstruct more than a tensor container:

- exact model architecture and model-class identity;
- every architecture-specific config field;
- complete tensor-name, shape, transpose, split, and merge mappings;
- tied-embedding and attention-bias semantics;
- tokenizer vocabulary, merges, scores, byte fallback, special tokens, and chat template;
- MoE expert/router/shared-expert topology;
- hybrid attention, SSM/DeltaNet, sliding-window, MLA, and other per-layer patterns;
- MTP/NextN heads and their runtime support;
- multimodal projector and processor assets;
- quantization provenance and fidelity expectations.

A converter that writes syntactically valid safetensors can still produce a model that
loads but generates incorrect tokens. Success therefore means validated behavior, not
only process exit zero or file existence.

## Primary-Source Findings

### Rapid-MLX and mlx-lm

- Rapid-MLX `v0.10.9` accepts a native MLX model alias/repository/directory. It has no
  GGUF load path or bundled `gguf2mlx` command.
- Rapid-MLX's source explicitly describes mlx-lm's GGUF direction as export-only for
  this use case.
- Official mlx-lm `v0.31.3` conversion loads Hugging Face/local safetensors and may
  create FP16/BF16 or newly quantized MLX output.
- The quality/reference path is therefore authoritative safetensors -> official
  mlx-lm conversion, not GGUF round-tripping.

### `barrontang/gguf2mlx`

Positive signals at audited SHA `6a0da6529f233df79362cbf62dd96221c895351f`:

- active development as of 2026-07-09;
- MIT declared in `pyproject.toml`;
- Python package and `gguf2mlx` CLI;
- uses the GGUF Python library's dequantization primitives;
- emits sharded safetensors, a generated config, and extracted tokenizer assets;
- contains mappings for many established families and some advanced MoE/MLA work;
- supports metadata-only inspection before weight conversion.

Release blockers in its current upstream form:

- no tagged release; `pyproject.toml` reports `2.0.2`, so a commit SHA is the only
  reproducible dependency identity;
- only a small smoke-test suite and no representative end-to-end model corpus;
- per-tensor dequantization/mapping failures may be counted and skipped while the tool
  continues and ultimately reports conversion success;
- missing/unknown architecture metadata may fall back to `llama`, which is unsafe for
  unattended conversion;
- config construction includes defaults and heuristics that can create a plausible but
  incorrect model;
- no proof that MTP, multimodal projectors, the latest hybrid architectures, or every
  advertised quant type loads and behaves correctly in current mlx-lm/Rapid-MLX.

### Other tools considered

- `acampkin95/gguf-to-mlx` is a newer wrapper/alternative that advertises convenient
  quantization and safety checks, but it is less established and does not remove the
  need to validate the underlying reverse mappings.
- No official llama.cpp GGUF -> Hugging Face/safetensors reverse converter was found in
  the inspected upstream toolchain. This is an absence of an approved path, not proof
  that no experimental third-party script exists.
- Broad multi-format converter repositories without model-family parity evidence are
  not acceptable merely because they list both GGUF and MLX.

Future research must continue searching for maintained tools, but every candidate is
evaluated by the gates below rather than README claims or repository popularity.

## Real Local Probe Evidence

Metadata-only probes were run on 2026-07-15. No weight conversion was started.

### Qwen 3.6 hybrid MoE

Source:

`~/.config/llama-monitor/models/Qwen3.6-35B-A3B-uncensored-heretic-Q6_K.gguf`

Observed GGUF facts:

- architecture: `qwen35moe`;
- 733 tensors;
- 40 blocks;
- MoE expert count/used/shared feed-forward metadata;
- `full_attention_interval`;
- SSM convolution, group, inner-size, state-size, and time-step-rank metadata;
- tokenizer and chat-template metadata.

The candidate converter recognized the architecture name but generated a generic
`Qwen35moeForCausalLM` config without expert topology, hybrid full-attention pattern,
or SSM/DeltaNet fields. That output is known-invalid and must be rejected before weight
conversion.

### Gemma 4

Source:

`~/.config/llama-monitor/models/gemma-4-31B-it-uncensored-heretic-Q5_K_M.gguf`

Observed GGUF facts:

- architecture: `gemma4`;
- 833 tensors;
- 60 blocks;
- sliding-window size and per-layer pattern;
- shared-KV layer metadata;
- separate SWA key/value dimensions and RoPE parameters;
- per-layer input embedding dimensions;
- final-logit softcapping;
- tokenizer and chat-template metadata.

The candidate converter generated a generic Gemma 4 config that omitted the
sliding-window pattern, shared-KV layout, per-layer input dimensions, and logit
softcapping. That output is known-invalid and must be rejected before weight conversion.

These probes prove why metadata/config validation is the first gate. Writing tens of
gigabytes of weights for either model would currently be wasted work.

## Conversion Paths and Fidelity

### Path A: Authoritative safetensors (supported)

Input:

- original full BF16/F16 Hugging Face model; or
- known base model plus LoRA/adapter, merged with authoritative HF/PEFT tooling into a
  full safetensors model.

Pipeline:

1. pin source repository and revision or hash local files;
2. validate model config, tokenizer, and model class;
3. use pinned official `mlx_lm.convert`;
4. optionally select an MLX quantization recipe supported by that pinned mlx-lm;
5. load and smoke-test with pinned mlx-lm and Rapid-MLX;
6. atomically publish a manifest-backed cache entry.

This is the preferred path because it begins with the finetune's authoritative weights
and metadata and performs at most one quantization.

### Path B: GGUF -> recovered FP16 (experimental)

Input: a complete GGUF containing the merged model and sufficient metadata/tokenizer.

Pipeline:

1. identify an exact verified architecture profile;
2. inventory every tensor name, type, shape, and quantization;
3. build config/tokenizer from profile-owned mappings with no unsafe architecture
   fallback;
4. dequantize tensors to staged FP16 using a pinned converter implementation;
5. abort on the first skipped, unknown, duplicated, missing, or shape-mismatched tensor;
6. validate config/tokenizer/tensor set against the pinned runtime model class;
7. load the FP16 result and run parity probes;
8. optionally produce a separate newly quantized MLX derivative;
9. publish only after all gates pass.

FP16 is an expanded representation of already-quantized values. It is useful as a
runtime/intermediate format but is not equivalent to the original pre-GGUF finetune.

### Path C: GGUF -> FP16 -> MLX quant (experimental derivative)

This saves runtime storage/memory but adds a second quantization. The output manifest
must name both the source GGUF quantization and the new MLX recipe. UI copy must never
call it a lossless conversion or preserve the GGUF quant label.

## Source Quantization Guidance

| GGUF source | Initial policy | Rationale |
|---|---|---|
| F32/F16/BF16 | Preferred experimental source | Closest available source to authoritative weights, subject to complete metadata/mappings |
| Q8_0 | Strong candidate | Relatively small source error; still not lossless |
| Q6_K / comparable high quant | Candidate with caution | Often practical for GGUF-only finetunes; validate against source carefully |
| Q5 / Q4 K-family | Explicit compounded-loss warning | Useful when it is the only artifact, but later MLX quantization adds approximation |
| IQ4 and unusual importance-aware quants | Separate profile required | Dequantization loses the original importance-matrix/block recipe |
| IQ3/Q3/Q2 and lower | High risk; disabled until model-specific proof | Larger source error and strongest risk from re-quantization |
| Unknown/new quant type | Unsupported | Never guess or skip tensors |

Quant tier is not the only risk. A Q8 model with an incorrect architecture mapping is
less usable than a Q4 model with a fully verified mapping.

## Architecture and Asset Policy

An architecture profile is promoted independently. Initial policy:

- Established text-only Llama/Qwen2/Mistral families may be first candidates after
  exact tensor/config/tokenizer proof.
- MoE requires expert/router/shared-expert tensor and config validation.
- Hybrid attention/SSM/DeltaNet requires per-layer pattern and state-space fields.
- MLA/custom routing requires exact split/merge and head-dimension validation.
- Tied embeddings and attention bias are read from authoritative metadata/profile, not
  guessed from a family-name list.
- MTP/NextN output is unsupported until both conversion and the target runtime support
  its heads and execution semantics.
- Multimodal import requires the full projector/vision tower, processor config, image
  token configuration, and paired runtime test. A language-model GGUF plus a nearby
  `mmproj` filename is not sufficient proof.
- Adapter-only GGUF or unknown base/adapter provenance is unsupported. Require the
  merged full model or authoritative base plus adapter.

## Fail-Closed Converter Contract

The app must wrap or fork the selected converter behind a versioned adapter. Production
acceptance requires:

- exact converter commit/package identity;
- no arbitrary executable discovery from `PATH`;
- no shell command construction;
- canonicalized paths and bounded output locations;
- zero skipped/unmapped tensors;
- exact expected tensor inventory and shape rules per profile;
- finite numeric values and bounded tensor/output sizes;
- no unsafe `llama` fallback for unknown architectures;
- no default config value unless the profile proves it is semantically correct;
- redacted bounded logs;
- cancellable child process and cleanup;
- staged output on the same filesystem as the final cache;
- atomic final rename only after validation;
- no launchable output when interrupted or invalid.

If upstream cannot provide these guarantees, llama-monitor may maintain a small pinned
fork or a strict pre/post-validation wrapper. The preference is to contribute hardening
upstream, but product safety does not depend on upstream accepting changes immediately.

## Cache and Provenance Manifest

Each attempted import receives a stable key derived from:

- source canonical path or repository revision;
- source size and SHA-256;
- complete GGUF metadata/tensor/quant inventory hash;
- converter package and commit SHA;
- architecture-profile version;
- config/tokenizer source and fingerprints;
- output dtype;
- mlx-lm version and optional quantization recipe;
- Rapid-MLX compatibility profile.

Layout:

```text
~/.config/llama-monitor/models/rapid-mlx/imports/
  <cache-key>/
    .converting
    manifest.json
    logs/
    fp16/
    mlx-<recipe>/
    validation.json
    .complete
```

Rules:

- `.converting` is never launchable.
- `.complete` is written only after validation and atomic promotion.
- Sentinel disappearance alone never means success.
- Failed/stale imports retain bounded diagnostics and are safely cleanable.
- The original GGUF is never modified.
- Concurrent imports of the same key coalesce or reject safely.
- Cache cleanup shows source, output sizes, validation status, and dependencies before
  deletion.

## Resource Planning

Before conversion, estimate:

```text
required free space =
  FP16 staging (approximately 2 bytes * parameter count)
  + optional final MLX output
  + largest shard/temp overhead
  + filesystem safety margin
```

The source GGUF already occupies disk and must remain intact. For a 35B total-parameter
model, FP16 staging alone is roughly 70 GB before the final MLX derivative and safety
margin. A flat `3 * GGUF file size` rule is not reliable across source quantizations.

Conversion must expose phases and allow cancellation:

1. Inspecting source
2. Validating architecture/assets
3. Checking disk and memory
4. Recovering config/tokenizer
5. Dequantizing tensors
6. Validating FP16 model
7. Optional MLX quantization
8. Runtime/parity validation
9. Publishing cache

No native browser dialogs are allowed. Failures keep the original model selected and
offer llama.cpp or a diagnostics view.

## Validation Matrix

### Static and structural

- metadata schema and architecture profile;
- tensor names/counts/shapes/dtypes/quant types;
- required and forbidden tensors;
- shard index completeness;
- no NaN/Inf;
- config fields and model class;
- tokenizer vocabulary/merges/scores/special tokens/chat template;
- auxiliary asset completeness.

### Runtime

- load with pinned mlx-lm;
- load and become ready with pinned Rapid-MLX;
- no warnings about missing/unexpected tensors;
- bounded memory and startup diagnostics;
- deterministic short generation.

### Behavioral parity

For fixed prompts and greedy decoding:

- tokenizer input IDs match;
- first-token ranking/top-k and logits are within profile-defined tolerances;
- deterministic token output is compared with llama.cpp on the source GGUF;
- recovered FP16 is the reference for evaluating a later MLX quantized derivative;
- chat-template, tool/reasoning tokens, stop tokens, and long-context behavior are
  covered when the model claims them.

Exact logit tolerances must be calibrated per source quantization/profile. A converter
cannot require bit-identical logits across different kernels and quant formats, but it
must detect gross mapping/config/tokenizer failures.

## Initial Local Fixture Program

Start with metadata-only rejection/coverage fixtures from the user's library:

1. `Qwen3.6-35B-A3B-uncensored-heretic-Q6_K.gguf` plus matching mmproj and
   Native-MTP-Preserved variant.
2. `gemma-4-26B-A4B-it-UD-Q5_K_XL.gguf` plus matching mmproj/MTP variants. This is a
   separate rejection fixture from the 31B Q5_K_M model used in the recorded probe.
3. `Qwen3.5-122B-A10B-abliterated.i1-IQ3_XXS.gguf` plus matching mmproj.

These are deliberately difficult. They prove that unsupported models fail early and
correctly. They are not first successful conversion candidates.

The first success fixture should be a small, text-only, well-understood architecture
available in both authoritative safetensors and multiple GGUF quantizations. That lets
the team compare:

- authoritative safetensors -> official MLX;
- F16/Q8/Q6/Q4 GGUF -> recovered FP16;
- recovered FP16 -> MLX quant;
- llama.cpp source output versus both MLX outputs.

Only after this harness is trustworthy should profiles expand to MoE, hybrid, MTP, or
multimodal models.

## Milestones and Gates

### R0.5 — Rapid-MLX v0.10.10 upgrade qualification

- Verify the tag, source SHA, package version, wheel identity, and installed
  `rapid-mlx --version`; never accept version text alone as provenance.
- Install `v0.10.10` into a new app-scoped staging environment without changing the
  active `v0.10.9` environment or any user-owned Brew/Pip installation.
- Run capability discovery, command/profile validation, readiness, one deterministic
  chat stream, telemetry/status parsing, stop, and the Phase 5 native-MLX fixture.
- Atomically activate the staged version only after every probe passes, retain
  `v0.10.9` as last-known-good, then prove rollback restores it.
- If app-owned upgrade plumbing is not implemented yet, capture the same evidence with
  an isolated manual harness and carry its contract into the Phase 6 runtime manager.
- Gate: a failed install, provenance check, capability probe, launch, or chat leaves
  `v0.10.9` active and produces bounded, redacted diagnostics.
- Checkpoint: recorded `v0.10.10` compatibility and rollback evidence; no production
  updater UI is implied by this research milestone.

#### R0.5 qualification evidence (2026-07-16)

Status: **qualified by an isolated manual harness; managed stable compatibility policy
implemented**. The production updater remains intentionally outside this checkpoint.

Provenance:

- GitHub tag `v0.10.10` resolves to verified commit
  `5ca536275e89ddf0de3b49bd6f55fad80e42656e`; the source `pyproject.toml` declares
  `0.10.10`.
- The `v0.10.9...v0.10.10` comparison is 15 commits ahead. In addition to release
  hardening, it includes model onboarding, Gemma 4 routing/KV-share changes, KV-cache
  export/import, model-profile refactoring, and DFlash security changes. Treat this as
  a runtime release, not a metadata-only bump.
- PyPI published `rapid_mlx-0.10.10-py3-none-any.whl` with SHA-256
  `4cb43a8c21b35436251023a33e51720b9a42bca0b9c76085023bcc8284ca0d71`.
  The downloaded wheel matched that digest and its metadata declared package/version
  `rapid-mlx==0.10.10`. PyPI's publish-provenance endpoint binds that digest to
  `raullenchai/Rapid-MLX`, `publish.yml`, tag `v0.10.10`, and the same commit SHA.

Isolation and capabilities:

- The retained last-known-good runtime remained at
  `~/.config/llama-monitor/runtimes/rapid-mlx/0.10.9/venv`; its live CLI continued to
  report `rapid-mlx 0.10.9` before and after qualification.
- The verified wheel was installed into
  `~/.config/llama-monitor/runtimes/rapid-mlx/.staging/0.10.10-qualification/venv`
  with Python `3.12.13`, `mlx-lm==0.31.3`, and `mlx==0.32.0`. No Brew, Pipx, external
  virtual environment, or active `v0.10.9` files were changed.
- Live `rapid-mlx --version` reported `0.10.10`. All flags used by llama-monitor were
  present: `--host`, `--port`, `--log-level`, `--served-model-name`, `--timeout`, and
  `--max-cache-blocks`. Relative to `0.10.9`, the only removed serve-help tokens were
  the deprecated `--continuous-batching` and `--chunked-prefill-tokens`; llama-monitor
  does not construct either flag.

Runtime gate:

- Host: Apple Silicon macOS `26.5.1`; telemetry disabled and API authentication enabled.
- Fixture: the Phase 5 app-cache snapshot of `mlx-community/Qwen3-0.6B-4bit` at
  revision `73e3e38d981303bc594367cd910ea6eb48349da8`, exposed as
  `phase5-qwen3-fixture`.
- The model loaded, `/health/ready` returned `ready: true`, authenticated `/v1/models`
  returned the served alias, and authenticated `/v1/status` plus `/v1/cache/stats`
  parsed successfully.
- Three authenticated SSE chat requests completed. Two repeated greedy requests with
  `temperature=0` and `seed=55` both produced `**PHASE55_OK**`. Post-request status
  reported three processed requests, 63 prompt tokens, 24 completion tokens, 28 engine
  steps, and zero running/waiting requests.
- The server was stopped and the bounded follow-up probe confirmed no listener remained
  on the qualification port.

Manual activation and rollback contract:

- Because the Phase 6 updater does not exist yet, the qualification-only app-scoped
  `.qualification-r05/active` symlink was atomically replaced on the same filesystem
  after all probes. It moved from retained `0.10.9` to staged `0.10.10`, then
  atomically rolled back to `0.10.9`; each pointer target and live CLI version was
  checked after replacement. The qualification pointer remains present and points to
  `~/.config/llama-monitor/runtimes/rapid-mlx/0.10.9/venv/bin/rapid-mlx`.
- A deliberately incorrect expected wheel digest was rejected before pointer
  replacement; the qualification pointer remained on `0.10.9`. This demonstrates the
  required fail-before-activation ordering without modifying the product's configured
  runtime.

Managed compatibility follow-through completed in R0.5:

- The managed stable channel accepts Rapid-MLX versions at or above `0.10.9` only when
  all six required serve flags pass live discovery. It deliberately avoids a
  per-release source allowlist: a newer stable release such as `0.10.11` passes with
  the required capabilities and fails if any required capability is absent.
- Managed prerelease and local variants are excluded from the stable channel. They may
  still be configured explicitly as user-owned/custom provisional runtimes.
- `0.10.10` remains the latest directly qualified evidence baseline and `0.10.9` the
  retained rollback version; those values are defaults/evidence, not a maximum-version
  policy. Explicit verified aliases are version-agnostic, while uncatalogued aliases
  remain free-form. GGUF rejection diagnostics name the selected runtime version.
- `CompatibilityState::Verified` means only that a managed runtime passed the stable
  version floor and live interface/capability probes. `RuntimeSource::Managed` is a
  configured path classification, not proof of artifact authenticity or completed
  activation qualification.

Remaining Phase 6 follow-through:

- Phase 6 must verify official artifact provenance and atomically switch an app-owned
  pointer only after staged install, capability, readiness, deterministic chat, status,
  stop, and rollback gates pass. It must persist artifact digest/provenance; version
  output by itself remains insufficient provenance.
- Only after those Phase 6 gates pass may the updater activate and supply the app-owned
  managed runtime path to normal launch discovery.

### R0 — Tool survey and evidence matrix

- Continue surveying maintained reverse-conversion tools.
- Record license, releases, activity, CLI/API, supported quant types, architecture
  mappings, tests, failure semantics, and security posture.
- Re-run the survey before selecting a production candidate.
- Gate: written selection with primary-source citations and rejected alternatives.
- Checkpoint: `docs(models): select GGUF recovery toolchain`.

#### R0 evidence matrix and selection (2026-07-16)

Status: **complete**. No inspected general-purpose converter is safe to invoke
unmodified. R1 may build the metadata-only inspector; R2 must use the pinned,
profile-scoped fork and wrapper contract selected below.

| Candidate | Identity, license, activity, distribution | CLI/API and claimed coverage | Source truth, tests, and failure semantics | Security / supply-chain fit | R0 decision |
|---|---|---|---|---|---|
| `barrontang/gguf2mlx` | `main` and audited snapshot are `6a0da6529f233df79362cbf62dd96221c895351f`, merged 2026-07-09. No Git tag or GitHub release existed. `pyproject.toml` declares version `2.0.2` and MIT, but the repository has no tracked `LICENSE` file and GitHub reports no detected license. The named PyPI project endpoint returned 404 during this survey, so there is no PyPI artifact/provenance to pin. The merge commit is GitHub-verified. | Importable Python `convert()` plus `gguf2mlx` CLI with `--input`, `--output`, `--dtype`, and `--skip-weights`. README claims every quant from Q2_K through F16 and 45+ architectures. Source explicitly handles F32/F16/F64/integer tensors and delegates every other enum to the installed `gguf.quants.dequantize`; `gguf>=0.18.0` is only a lower bound. Architecture entries and config generation are mappings/heuristics, not load or parity evidence. | Unknown metadata/name falls back to `llama`; config invents defaults such as 32 layers, 4096 hidden size, 32 heads, and 4096 context. Per-tensor dequantization or processing exceptions increment `skipped` and continue; orphaned split tensors can be discarded; any non-empty tensor set is sharded and the tool prints `Conversion complete` even when tensors were skipped. Main contains five smoke/unit test functions. Its test header refers to opt-in `tests/test_e2e.py`, but that file is absent. There is no checked-in real-model corpus, runtime-load gate, tokenizer parity gate, cancellation test, or malformed-input corpus. | No checked-in CI workflow or dependency lock. README installation tracks mutable Git `main`; dependencies use floors. The caller chooses an arbitrary existing output directory, which is created/written in place before conversion is known good. Logs are free-form and unbounded for an app protocol. | **Fork as a seed, reject upstream execution.** Preserve attribution and the declared MIT terms in our snapshot. Pin the Git tree and every Python artifact by hash. Retain only audited GGUF reading/dequantization and profile-needed mapping code; patch fail-open behavior before R2. |
| `acampkin95/gguf-to-mlx` | Current `main` is unsigned `cc2177738694902444a824ea3ee441dd9e9929ad` from 2026-07-07. The only tag/release is `v1.1.0` at `e44960ab7f44730dd17c54fb4be98dce3dc1dc4e`; current docs call HEAD `v1.4.0`, while `pyproject.toml` still says `1.0.0`. MIT text is tracked. The named PyPI endpoint also returned 404. | `cpmm`/`convert.py` supplies guided menus, scanning, Hugging Face download, resource estimates, 2/4/8-bit mlx-lm re-quantization, validation, and optional source deletion. It vendors a divergent older `gguf2mlx` core rather than independently reconstructing models. Dependencies including `gguf`, `mlx`, `mlx-lm`, `transformers`, and `requests` are broad minimums. | The vendored core also defaults unknown architectures to `llama`, skips `None` mappings, continues after dequantization/processing errors, and succeeds with a partial tensor set. Its `validate_output()` catches load/config failures, warns, and explicitly never blocks the pipeline. The test file has 326 test functions and the project reports 326 passing tests, but inspected GGUF-reader/download/conversion paths are mocked or synthetic; there is no checked-in real-GGUF end-to-end corpus or behavioral parity gate. | Much larger surface than needed: interactive prompts, network search/download, config/token persistence, mutable output overwrite, and `--delete-gguf`. No checked-in CI workflow or dependency lock; HEAD is not release-identical or version-identical. These features duplicate app-owned policy and expand the trust boundary. | **Reject.** Useful UX ideas do not compensate for the same unsafe conversion core, non-blocking validation, version ambiguity, and unnecessary network/deletion surface. Do not vendor or shell out to it. |
| `chaosste/local-mlx-tune` | Unsigned `5bbdc251916497f7b751384d9aee2595c5a34cf6`, 2026-05-31; no tag/release. | Shell workflow installs `barrontang/gguf2mlx` from mutable Git `main`, invokes its CLI, then optionally runs `mlx_lm.convert`. | It is not an independent converter. The repository adds a Gemma 3 post-hoc config patch containing further defaults and explicitly describes the GGUF/Gemma path as unreliable. It adds no strict tensor-closure or real conversion corpus for our profiles. | Unpinned Git dependency and shell orchestration violate the managed, hash-pinned adapter boundary. | **Reject as a tool.** Keep only as corroborating evidence that plausible config output still needs model-specific repair. |
| `duoyuncloud/ModelConverterTool` | Unsigned `dcac0a0fdf0153c1d4b52bbc3af4438b91a67c38`, last pushed 2025-08-15; no inspected tag/release or detected license. | Advertises many input/output formats, including GGUF and MLX. | Source implements Hugging Face/model-object -> official `mlx_lm.convert` and Hugging Face -> llama.cpp GGUF export. It does not implement arbitrary GGUF -> recovered HF/MLX weights. Its MLX validation checks config/weight presence, with optional one-token inference, rather than reverse-mapping closure/parity. | It may clone mutable `mlx-examples` HEAD at runtime and removes/replaces output directories. That is incompatible with pinned offline execution and app-owned atomic staging. | **Reject.** It is a forward multi-format wrapper, not a reverse converter. |
| `kuotient/hy-mt2-mlx` | Unsigned `cc3b9ce816a3ea40aa470361ec71e43d4104d7a9`, 2026-07-03; Apache-2.0 with tracked license/notice; no tag/release. | Converts one Hy-MT2 1.25-bit/STQ1_0 architecture to an MLX model using an authoritative reference directory. | Not general-purpose, but its source demonstrates the correct profile model: exact tensor-name allowlist, asserted quant types, per-tensor round-trip checks, exactly 224 linear tensors, required authoritative config plus tokenizer assets sourced from the same reference directory, real-GGUF opt-in tests, and a llama.cpp-vs-MLX greedy parity harness. Individual missing tokenizer files are silently skipped, so the stricter R2 asset-closure gate remains necessary. It still writes directly to the chosen output and uses assertions in conversion code, so it is not reusable unchanged. | Dependencies use minimum versions and there is no release artifact, but the narrow mapping and explicit corpus greatly reduce semantic ambiguity. | **Reject as a dependency; accept as design evidence.** Its architecture-specific, exact-count, real-fixture approach validates this plan's profile-by-profile strategy. |

Repository/code search found no maintained official llama.cpp, mlx-lm, or Rapid-MLX
GGUF -> Hugging Face/MLX reverse path. Other results were model managers, inference
engines, forward exporters, or one-model experiments rather than credible general
reverse converters. Absence from this bounded survey is not proof that no private or
future converter exists; R5 must re-run this evidence check before each new family is
promoted.

##### Selected R1/R2 toolchain

1. **R1 is llama-monitor-native and converter-free.** Reuse the app's pinned GGUF
   parser/metadata knowledge to select an explicit versioned architecture profile.
   Unknown architecture, missing metadata, unsupported quant types/assets, and any
   heuristic-only match return `Unsupported`; there is no model-name or `llama`
   fallback.
2. **R2 starts from an internal minimal fork of Barron Tang's exact Git snapshot
   `6a0da6529f233df79362cbf62dd96221c895351f`.** Record the exact snapshot source URL
   and its downloaded artifact SHA-256 in the R2 provenance note; GitHub's codeload,
   API tarball, and zipball are distinct byte artifacts even at one commit. The
   2026-07-16 codeload tarball at
   `https://codeload.github.com/barrontang/gguf2mlx/tar.gz/6a0da6529f233df79362cbf62dd96221c895351f`
   has SHA-256
   `a4d4bb8d9c673ebbec348cf32a40b95aa051ebccd5e761bbad107286d11006fc`; the
   upstream `pyproject.toml` blob SHA-256
   `71c3903cbf8040862a4d8299489ecec10ace73fc0293b6d884395b1c939a209c` in the fork
   provenance note. The Git SHA, not upstream's `2.0.2` string, is its identity.
3. **Forking is required for the selected R2 implementation, not left as an optional
   post-process wrapper.** Atomic staging plus exact closure validation could prevent a
   partial upstream result from being promoted, but it would detect failure only after
   partial shards had already been emitted and would retain an unnecessarily broad,
   fail-open execution path. The minimal fork must remove architecture guessing/config
   defaults, abort on the first unknown, duplicate, skipped, non-finite,
   shape-mismatched, or failed tensor, and return a bounded machine-readable report
   containing the complete input/output inventory.
4. **The llama-monitor adapter owns policy.** It passes only an R1-approved profile and
   canonical staged paths, never discovers a converter from `PATH`, never uses a shell,
   pins exact dependency wheels/hashes in an app-scoped environment, enforces resource
   limits/cancellation, and validates tensor/config/tokenizer closure before the
   profile-specific mlx-lm and Rapid-MLX runtime/parity gates.
5. **Profiles own config and assets.** Do not call upstream's heuristic
   `build_config()` for accepted imports. Prefer authoritative config/tokenizer assets;
   otherwise every reconstructed value must be required and proven by the versioned
   profile. Dequantization support is the intersection of the pinned `gguf` library,
   the selected profile, and a real fixture test—not the upstream README's broad quant
   claim.
6. **Nothing from R0 is production-launchable.** The fork snapshot, patch set,
   dependency lock, license/attribution, malformed fixtures, and first successful real
   corpus must be reviewed at R2. Only atomically promoted output bearing a complete
   provenance manifest may enter the model inventory.

R0 gate result: **passed for research selection**. The chosen path is a pinned minimal
fork plus a strict llama-monitor-owned profile adapter; every inspected unmodified
tool is rejected. This checkpoint authorizes R1 research/implementation only and does
not claim that any architecture can yet be converted safely.

### R0.75 — Runtime and transport hardening

Complete the high-leverage cross-backend improvements identified in the Phase 5
post-implementation review before adding conversion workloads:

- Move recursive model scans, complete-file hashing, manifest validation, and other
  blocking filesystem/JSON work off Tokio workers with bounded `spawn_blocking` tasks.
  Preserve full content verification and fail-closed tamper detection; do not replace it
  with an unsafe time-only cache.
- Avoid duplicate safetensors/index walks inside one resolution operation. Reuse an
  explicitly invalidated validation snapshot only when its file identity rules preserve
  the complete manifest contract.
- Replace the global single-permit inference bottleneck with a backend-aware policy:
  retain llama.cpp's safe serialization while allowing bounded concurrent requests to
  the active Rapid-MLX target. Session/backend changes while waiting must be detected and
  rerouted safely rather than sending to a stale target.
- Replace unbounded chat SSE queues with bounded backpressure. A slow or disconnected
  client must bound memory, drop the upstream stream promptly, release its permit, and
  leave no orphan generation owned by llama-monitor.
- Reuse long-lived upstream and Rapid-MLX polling clients with per-request deadlines.
  Do not create a new poller/client for every metrics cycle or disable pooling without a
  measured correctness reason.
- Pass Rapid-MLX SSE data through byte-for-byte when it contains no reasoning field that
  requires normalization; preserve the existing reasoning/tool/usage/finish mapping and
  llama.cpp byte-exact behavior.

Hard gates:

- Tests prove llama.cpp remains serialized, Rapid-MLX accepts the configured bounded
  concurrency, queued work cannot cross a session/backend switch, and permits are always
  released on success, error, timeout, and client disconnect.
- A deliberately slow SSE consumer demonstrates a fixed queue bound and upstream
  cleanup. Fast consumers retain streaming order with no dropped events.
- Large fixture hashing and recursive validation execute outside Tokio workers while
  cancellation, path containment, manifest closure, and cache-tamper rejection remain
  intact.
- Client-reuse tests or observable connection evidence cover chat retries and the three
  Rapid-MLX telemetry endpoints without weakening authentication or timeouts.
- Full Rust tests, clippy, auth routing, Windows GNU cross-check, and verifier sign-off.
- Checkpoint: `perf(api): harden concurrent inference transport`.

The review intentionally does **not** authorize a wholesale `Mutex` -> `RwLock` rewrite,
session `Vec` -> `HashMap` conversion (sessions are capped at ten), or dynamic import of
the approximately 13 KiB Rapid-MLX card module. Revisit those only with profiling or a
larger product need.

### R1 — Strict metadata-only inspector

- Reuse llama-monitor's GGUF metadata knowledge where possible.
- Produce an architecture/asset/quant compatibility report without writing weights.
- Gate: Qwen 3.6 and Gemma 4 known-invalid configs are rejected with exact missing
  profile fields; no large output is created.
- Checkpoint: `feat(models): add GGUF import compatibility inspector`.

### R2 — Small-model recovered FP16 spike

- Pin/fork/wrap the chosen converter.
- Convert the small authoritative fixture at F16/Q8/Q6/Q4 source tiers.
- Implement strict tensor/config/tokenizer validation and manifests.
- Gate: zero skipped tensors, pinned runtime load, parity evidence, cancellation and
  disk-failure cleanup.
- Checkpoint: `feat(models): validate experimental GGUF recovery pipeline`.

### R3 — Optional MLX re-quantization

- Use pinned official mlx-lm on the validated recovered FP16 result.
- Compare 4/6/8-bit recipes where supported.
- Gate: recorded fidelity/performance/size results and clear source/output labels.
- Checkpoint: `feat(models): add validated MLX quantization recipes`.

### R4 — Experimental Import Lab UX

- App-native compatibility report, resource estimate, progress, cancellation,
  diagnostics, cleanup, and engine fallback.
- Show `Verified`, `Experimental`, or `Unsupported` before conversion begins.
- Keep one in-memory typed model inventory while the library is open. Search, filter,
  sort, and view-mode changes render locally; refetch only on open, explicit refresh, or
  a known mutation such as download, migration, conversion, tag update, or deletion.
- Fetch platform/Rapid-MLX availability through one shared cached state/promise rather
  than independently from models, presets, setup, and wizard flows. Explicitly
  invalidate it after runtime installation or platform-relevant configuration changes.
- Preserve stable telemetry card DOM and update values/states incrementally for each
  poll instead of rebuilding all eight Rapid-MLX cards. Preserve focus, accessible live
  regions, reduced-motion behavior, stale-card history, and backend switching.
- Gate: release build, dark/light screenshots, reduced motion, isolated Playwright,
  security review, no native dialogs, and tests proving UI-only inventory controls do
  not refetch `/api/models` or `/api/llama-binary/platform-info`.
- Checkpoint: `feat(ui): add experimental GGUF import lab`.

### R5 — Architecture promotion

- Add one profile at a time with dedicated real fixtures.
- Promote a profile to `Verified` only after all structural/runtime/parity gates pass.
- MTP and multimodal promotion are independent from base text-model promotion.
- Gate: the promoted profile passes complete structural, pinned-runtime, and behavioral
  parity validation at its declared source quantization tiers; unsupported assets fail
  closed.
- Checkpoint: focused conventional commit per promoted family.

## Open Research Questions

- Can converter hardening be contributed upstream while retaining a pinned known-good
  fork for release reproducibility?
- Which small public model provides the best authoritative safetensors + F16/Q8/Q6/Q4
  comparison corpus?
- Which Rapid-MLX/mlx-lm model classes currently implement Qwen 3.5/3.6 hybrid and
  Gemma 4 semantics, and what exact config/tensor contracts do they consume?
- How should imatrix provenance be surfaced after dequantization when it cannot be
  transferred into MLX quantization?
- Which parity metrics best distinguish normal cross-kernel drift from broken tensor
  mapping at each quant tier?
- Can multimodal projector conversion share any llama.cpp metadata safely, or must the
  original HF vision assets always be required?
- Is MTP useful after recovery if Rapid-MLX lacks the exact head/runtime execution path,
  or should the importer intentionally strip and label it as base-model-only?

## Primary References

- Rapid-MLX `v0.10.9`:
  <https://github.com/raullenchai/Rapid-MLX/tree/3edb3ac69c1d1c5e81836a5d146e5f81048658d9>
- Rapid-MLX `v0.10.10` release:
  <https://github.com/raullenchai/Rapid-MLX/releases/tag/v0.10.10>
- Rapid-MLX `v0.10.10` release-artifact hardening:
  <https://github.com/raullenchai/Rapid-MLX/pull/1115>
- Rapid-MLX GGUF direction note:
  <https://github.com/raullenchai/Rapid-MLX/blob/3edb3ac69c1d1c5e81836a5d146e5f81048658d9/vllm_mlx/_download_gate.py#L58-L73>
- Official mlx-lm `v0.31.3` conversion:
  <https://github.com/ml-explore/mlx-lm/blob/v0.31.3/mlx_lm/convert.py>
- Official mlx-lm `v0.31.3` loader:
  <https://github.com/ml-explore/mlx-lm/blob/v0.31.3/mlx_lm/utils.py#L282-L319>
- Candidate `gguf2mlx` audited source:
  <https://github.com/barrontang/gguf2mlx/tree/6a0da6529f233df79362cbf62dd96221c895351f>
- Candidate conversion/error behavior:
  <https://github.com/barrontang/gguf2mlx/blob/6a0da6529f233df79362cbf62dd96221c895351f/src/gguf2mlx/gguf2mlx.py#L1040-L1231>
- Candidate config heuristics:
  <https://github.com/barrontang/gguf2mlx/blob/6a0da6529f233df79362cbf62dd96221c895351f/src/gguf2mlx/gguf2mlx.py#L302-L425>
- Candidate unsafe architecture fallback:
  <https://github.com/barrontang/gguf2mlx/blob/6a0da6529f233df79362cbf62dd96221c895351f/src/gguf2mlx/gguf2mlx.py#L194-L212>
- Candidate tests (including the absent E2E-file reference):
  <https://github.com/barrontang/gguf2mlx/blob/6a0da6529f233df79362cbf62dd96221c895351f/tests/test_smoke.py>
- Candidate package/dependency floors and declared license:
  <https://github.com/barrontang/gguf2mlx/blob/6a0da6529f233df79362cbf62dd96221c895351f/pyproject.toml>
- `acampkin95/gguf-to-mlx` audited source:
  <https://github.com/acampkin95/gguf-to-mlx/tree/cc2177738694902444a824ea3ee441dd9e9929ad>
- Alternative's vendored fail-open tensor loop:
  <https://github.com/acampkin95/gguf-to-mlx/blob/cc2177738694902444a824ea3ee441dd9e9929ad/gguf2mlx/core.py#L921-L1052>
- Alternative's explicitly non-blocking validation:
  <https://github.com/acampkin95/gguf-to-mlx/blob/cc2177738694902444a824ea3ee441dd9e9929ad/convert.py#L1957-L1985>
- Alternative `v1.1.0` release:
  <https://github.com/acampkin95/gguf-to-mlx/releases/tag/v1.1.0>
- `local-mlx-tune` mutable upstream converter dependency:
  <https://github.com/chaosste/local-mlx-tune/blob/5bbdc251916497f7b751384d9aee2595c5a34cf6/pyproject.toml>
- `ModelConverterTool` forward-only MLX engine:
  <https://github.com/duoyuncloud/ModelConverterTool/blob/dcac0a0fdf0153c1d4b52bbc3af4438b91a67c38/model_converter_tool/engine/mlx.py#L46-L77>
- Architecture-specific strictness precedent (`hy-mt2-mlx`):
  <https://github.com/kuotient/hy-mt2-mlx/blob/cc3b9ce816a3ea40aa470361ec71e43d4104d7a9/src/sherry_mlx/convert.py#L72-L169>
- Architecture-specific real-fixture/parity evidence:
  <https://github.com/kuotient/hy-mt2-mlx/blob/cc3b9ce816a3ea40aa470361ec71e43d4104d7a9/tests/test_stq.py>
  and
  <https://github.com/kuotient/hy-mt2-mlx/blob/cc3b9ce816a3ea40aa470361ec71e43d4104d7a9/scripts/parity.py>

## Documentation Closeout

When the research track is completed or superseded:

1. Move durable user-facing source/quality/resource guidance into the model-management
   reference documentation.
2. Move converter/profile/cache/security contracts into developer documentation.
3. Record accepted/rejected tool decisions and verified SHAs in a concise dev note.
4. Archive this dated plan with a completion/supersession banner.
5. Remove contradictory historical conversion instructions from older Rapid-MLX plans.
