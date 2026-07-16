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

#### R0.75 checkpoint evidence (2026-07-16)

Status: **Builder complete; independent Verifier approved.**

Delivered design:

- llama.cpp retains one application inference permit. Rapid-MLX has a separate fixed
  four-permit gate so continuous batching is usable without allowing an unbounded app
  queue. A request snapshots the complete active target before waiting and compares it
  again after permit acquisition and capacity waiting; any session, backend, endpoint,
  model, or protected-key change drops the old permit and retries against the current
  target.
- Chat SSE delivery uses a 32-event bounded channel. Backpressure waits instead of
  allocating without limit; receiver closure ends the producer, drops the upstream
  response stream, and releases the inference permit through RAII. Valid Rapid-MLX SSE
  JSON without `reasoning` or `reasoning_text` is forwarded byte-for-byte. Chunks that
  need reasoning normalization retain tool, usage, and finish fields, malformed chunks
  remain rejected, and llama.cpp payload forwarding remains byte-exact.
- Chat routes share one long-lived pooled `reqwest::Client`; every call still applies
  its route-specific 30/60/120-second request deadline. Rapid-MLX polling retains one
  poller/client per active endpoint and protected key, applies individual endpoint
  deadlines, and replaces the poller only when the target changes. Adapter polling also
  caches by port and invalidates on runtime/API-key reconfiguration.
- Resolver preview, recursive model validation, complete-file hashing, and conversion
  manifest closure validation run through a two-task bounded `spawn_blocking` gate.
  Source validation returns the canonical MLX path to avoid a second launch-time walk,
  and content hashing now validates and reads a safetensors index once while preserving
  path containment, complete-file SHA-256, symlink, staging, and tamper rejection.

Verification evidence:

- Focused tests cover llama.cpp serialization, the four-request Rapid-MLX bound,
  permit release after routing failure/cancellation/disconnect, active-target switching,
  bounded slow-consumer backpressure, fast-consumer ordering, upstream-owner cleanup,
  raw/normalized/malformed SSE behavior, singleton upstream client identity and request
  deadlines, retained authenticated polling across `/health`, `/v1/status`, and
  `/v1/cache/stats`, adapter poller reuse, bounded blocking-worker concurrency, Tokio
  responsiveness, and the one-pass safetensors-index contract.
- Full Rust suite: **900 passed / 6 ignored**. The restricted sandbox denied localhost
  binds for existing and new mock-server tests; the required unsandboxed loopback rerun
  passed completely.
- `cargo clippy -- -D warnings`, the 39-test `auth_routing` suite,
  `cargo check --target x86_64-pc-windows-gnu`, formatting, and `git diff --check`
  passed.

Remaining caveats for later profiling/product work:

- Four concurrent Rapid-MLX requests is an intentionally conservative app bound, not a
  claim about every model's optimal continuous-batching depth.
- Rapid-MLX still exposes no compatible public request ID for native cancellation.
  Disconnect now promptly drops llama-monitor's upstream stream and permit; runtime-side
  work stops according to Rapid-MLX's HTTP disconnect behavior.
- An already-running filesystem syscall cannot be preempted by Tokio cancellation, but
  resolver work is capped at two blocking workers and no longer occupies Tokio workers.

Independent verification found and corrected two issues before sign-off. Protected
Rapid-MLX keys used by queued-target and poller-reuse checks now use constant-time
comparison instead of ordinary string equality, including an explicit key-rotation
rerouting test. The raw Rapid-MLX SSE path now detects actual delta field names after
one successful parse, so ordinary content containing the word `reasoning` remains
byte-exact while malformed JSON still fails closed.

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

#### R1 checkpoint evidence (2026-07-16)

Status: **Builder complete; independent Verifier approved after one correction.**

Delivered design:

- A llama-monitor-native inspector reuses the pinned GGUF metadata reader and adds a
  strict tensor-directory inventory. It performs no conversion, child-process launch,
  model write, cache creation, download, or network request.
- The versioned report records the canonical source, size, modification identity, a
  bounded GGUF-header SHA-256 (explicitly not a full weight hash), authoritative
  architecture, tensor count, per-type quant inventory, tokenizer/config/asset
  observations, compatibility, exact gaps, resource warning, and remediation. Output is
  fixed-schema and contains no vocabulary, tensor-name, or arbitrary metadata dump.
- Inspection accepts only a library-relative regular `.gguf` inside the canonical
  configured `models_dir`. Traversal, absolute/root-relative paths, every symlinked
  component, missing/empty files, non-GGUF input, incomplete tensor directories, and
  metadata/tensor directories over 64 MiB fail before policy evaluation. Reads and seeks
  are capped during parsing, and inspection is limited to two `spawn_blocking` workers
  with a 15-second wait/work deadline so it cannot block Tokio or create an unbounded
  blocking queue.
- Architecture comes only from `general.architecture`; there is no filename, unknown
  architecture, or Llama fallback. R1 has no `Verified` conversion profile. Complete
  text-only Llama/Qwen2/Mistral metadata can be classified `Experimental`; all missing
  profile fields/assets fail `Unsupported`.
- Qwen3.5/Qwen3.6 hybrid/MoE, Gemma4 alternating attention, MTP/NextN, and multimodal
  projector inputs have explicit early-rejection reasons and exact missing contracts.
  Unknown tensor types and Q3/Q2/IQ3-or-lower sources fail closed; IQ4 requires a
  separate importance-aware profile; Q4/Q5 reports compounded-loss warnings.
- The authenticated preview endpoint is
  `POST /api/models/gguf/import/compatibility/preview`. Malformed JSON and invalid paths
  return 400; missing or incorrect API tokens return 401.

Builder evidence:

- Synthetic GGUF fixtures cover the experimental text-only path, Qwen3.6 hybrid
  MoE+MTP+projector gaps, Gemma4+MTP+projector gaps, unknown architecture, low and unknown
  quant types, traversal, outside paths, non-GGUF extensions, symlinks, and the strict
  header limit.
- Read-only local probes passed against the migrated Qwen3.6 Q6_K and Gemma4 Q4_K_XL
  fixtures under `~/.config/llama-monitor/models/gguf/`; both returned `Unsupported` with
  their exact profile gaps and hashed only bounded header bytes. No local model changed.
- Full Rust suite: **918 passed / 6 ignored**. The 39-test `auth_routing` suite,
  `cargo clippy -- -D warnings`, `cargo check --target x86_64-pc-windows-gnu`, focused
  inspector and route tests, formatting, and `git diff --check` passed. No converter,
  network operation, runtime, or model write was used by R1 validation.

Independent Verifier evidence:

- Review covered auth ordering, canonical containment, symlink/traversal handling,
  bounded reads/seeks/worker concurrency, fixed-schema output, fail-closed architecture
  and quant policy, and the absence of production writes, subprocesses, conversion, or
  network access.
- One security-contract issue was corrected: the initial endpoint accepted absolute
  paths when they canonicalized inside `models_dir`. User-supplied file paths now must be
  library-relative and explicitly reject absolute and root-relative `/` or `\` input;
  unit tests cover an absolute path inside the library and a backslash-rooted path.
- Focused inspector tests passed **8/8**, the route test passed **1/1**, auth routing
  passed **39/39**, library Clippy passed with warnings denied, focused formatting passed,
  and `git diff --check` passed.

### R2 — Small-model recovered FP16 spike

- Pin/fork/wrap the chosen converter.
- Convert the small authoritative fixture at F16/Q8/Q6/Q4 source tiers.
- Implement strict tensor/config/tokenizer validation and manifests.
- Gate: zero skipped tensors, pinned runtime load, parity evidence, cancellation and
  disk-failure cleanup.
- Checkpoint: `feat(models): validate experimental GGUF recovery pipeline`.

#### R2 checkpoint (2026-07-16)

Status: **complete; independent Verifier approved after remediation and detached-host
semantic validation.** This checkpoint authorizes later R3 research only. The recovered
cache remains experimental and non-launchable; production/UI promotion remains
unauthorized.

Selected corpus and provenance:

- Authoritative model: `HuggingFaceTB/SmolLM2-135M-Instruct` at revision
  `12fd25f77366fa6b3b4b768ec3050bf629380bac`; its BF16 safetensors SHA-256 is
  `5af571cbf074e6d21a03528d2330792e532ca608f24ac70a143f6b369968ab8c`.
- GGUF source: `unsloth/SmolLM2-135M-Instruct-GGUF` at revision
  `9e6855bc4be717fca1ef21360a1db4b29d5c559a`. F16, Q8_0, Q6_K, and Q4_K_M
  source size and SHA-256 are pinned by the versioned profile.
- The approximately 937 MiB corpus lives only below
  `~/.config/llama-monitor/models/experimental/import-lab/fixtures/smollm2-135m-v1/`;
  Hugging Face's default cache is not used.
- The converter seed is the audited `barrontang/gguf2mlx` Git snapshot
  `6a0da6529f233df79362cbf62dd96221c895351f`. The minimal fork retains attribution
  and records that upstream declares MIT in `pyproject.toml` but does not track a
  standalone license file. Exact direct dependencies are installed from a hash-locked
  requirements file into the app-owned `r2-v1` environment: `gguf==0.19.0`,
  `numpy==2.5.1`, and `safetensors==0.8.0`.

Delivered safety contract:

- One explicit SmolLM2/Llama profile owns source hashes, architecture/config/assets,
  tensor count, source-tier quant inventories, output dtype, and Q/K RoPE mapping.
  There is no architecture guessing, model-name fallback, skipped tensor, or network
  path in the worker.
- The Apple-Silicon-only executor accepts library-relative source/reference paths,
  rejects traversal and symlinks, verifies the isolated dependency environment,
  bounds report/diagnostic/output/disk use, supports active cancellation, cleans failed
  staging, and atomically promotes only a completely hashed cache.
- Worker directories bind the worker, profile, dependency lock, and third-party notice
  identities. Profile evolution therefore creates a new immutable worker directory
  instead of colliding with an older embedded asset.
- Every R2 result is permanently `launchable: false` with status
  `experimental_structurally_validated`. It is outside production model inventory and
  cannot be selected by Rapid-MLX.

Tensor evidence:

| Source tier | Exact observed quant inventory | Tensor closure | Global max absolute delta vs authoritative BF16 | Global minimum cosine |
|---|---|---:|---:|---:|
| F16 | F16/F32 as pinned | 272/272 | `2.9802322387695312e-08` | `0.9999998807907104` |
| Q8_0 | F32/Q8_0 as pinned | 272/272 | `0.03564453125` | `0.9999547004699707` |
| Q6_K | 61 F32, 30 Q6_K, 181 Q8_0 | 272/272 | `0.09423828125` | `0.9998226165771484` |
| Q4_K_M | 61 F32, 16 Q4_K, 166 Q5_0, 14 Q6_K, 15 Q8_0 | 272/272 | `0.28662109375` | `0.9971904754638672` |

All four reports have zero skipped, unknown, duplicate, shape-mismatched, or
non-finite tensors. The parity reader decodes the documented BF16 safetensors
representation directly and has no MLX/Metal dependency. This exposed and corrected a
critical defect inherited from the audited seed: with `gguf==0.19.0`, unquantized
`tensor.data` is already in logical Hugging Face order. Reshaping it through GGML
dimension order scrambled the F16 model even though it remained loadable. The fork now
uses unquantized data directly and applies only the explicit inverse Llama RoPE
permutation to Q/K weights.

Failure evidence:

- Focused tests reject every nonzero failure counter, output mutation, promoted-cache
  mutation, traversal/symlinks, and materialized-asset identity drift.
- A live child-process test cancels an actively sleeping worker, writes the sentinel,
  kills/reaps the child, and returns in under five seconds. Real-corpus pre-cancel and
  one-byte output-bound failures leave `.staging` empty.
- Mixed-quant tier profiles use the exact real-file inventories above. The initial
  narrower Q6_K profile rejected Q8_0 fail-closed; no broad allow-all fallback was
  added.

R2 remediation after independent review:

- Managed runtime, worker, imports, staging, and final-cache paths are now created one
  component at a time below canonical app roots. Preexisting symlinks/non-directories
  fail before writes, the cache root itself cannot be a symlink, and final promotion
  rejects any path that appeared after preflight.
- Complete source hashing, recursive cache hashing/validation, asset materialization,
  report closure, and promotion validation run through the existing two-permit bounded
  blocking-worker contract rather than on Tokio workers.
- The toolchain probe has a 30-second deadline, cleared environment, concurrently
  drained 16 KiB output bounds, and a persisted identity covering all 10 installed
  distributions plus 1,176 non-bytecode installed files. Package version strings alone
  are no longer accepted.
- Worker stdout/stderr drain concurrently from process start. Output overflow,
  cancellation, and timeout terminate and reap the worker process group on Unix;
  unsupported off-Apple execution retains an explicit portable direct-child stub.
  Post-exit pipe draining is also deadline-bound: a descendant that inherits pipes is
  terminated even after its direct parent exits, and the group receives SIGKILL after
  the grace interval even when the leader has already been reaped.
- The worker rehashes the GGUF and every authoritative reference asset after conversion.
  Rust independently validates typed source/reference/tensor provenance, exact profile
  worker/tier/count/quant/tensor-inventory identities, and complete file hashes before
  promotion and cache reuse. The embedded profile pins every authoritative config,
  tokenizer, generation, vocabulary, merges, special-token, and weight asset hash; a
  mutable reference manifest cannot bless substituted assets.
- The request limit now applies to actual complete FP16 directory bytes, including the
  safetensors header and copied assets, both at promotion and cache reuse. The complete
  MIT permission/warranty text is retained in the third-party notice. Environment-lock
  and notice hashes participate in worker and cache identities, so older results cannot
  collide with the remediated cache.
- Cache manifest and validation JSON are rejected through non-following metadata checks
  and fixed size bounds before any parse or hash; `.complete` must be an empty,
  non-symlink regular file. Rust applies the same bounded pre-open check to the
  authoritative reference manifest.
- The remediated F16 cache is
  `a21cca76ec236c3c71ea2bf5eb6f78716602b90fb16d78c3aef4da51e1ff4177`.
  Fresh parity remains 272 tensors, global max absolute delta
  `2.9802322387695312e-08`, and global minimum cosine `0.9999998807907104`.
  It remains permanently non-launchable; passing R2 does not promote it into the model
  inventory.

Detached runtime and independent Verifier evidence:

- The one-time normal-Terminal gate loaded the exact recovered F16 cache through pinned
  mlx-lm in **0.955 seconds**, returning `Model` and `TokenizerWrapper` with exit code
  zero and no timeout.
- Rapid-MLX `0.10.10` loaded the recovered cache as `r2-smollm2-f16`.
  `/health/ready`, authenticated `/v1/models`, `/v1/status`, and `/v1/cache/stats` all
  succeeded. Two identical greedy 32-token requests produced byte-identical text,
  finish reason `length`, and usage of 41 prompt plus 32 completion tokens. The second
  request also exercised a 41-token prefix-cache hit.
- The managed llama-server loaded the pinned F16 GGUF and completed the matching
  request with the same 41 prompt and 32 completion tokens. Both backends selected
  `The` as the first token and returned the same ordered top-five candidates:
  `The`, `\"`, `In`, `A`, and `When`. The winning logprob was `-0.171875` in
  Rapid-MLX versus `-0.1745922863` in llama-server; the largest absolute delta across
  the five candidates was approximately `0.0209`.
- Full 32-token text diverged after the shared opening, which is acceptable cross-kernel
  greedy behavior here: the exact prompt-token count and ordered first-token logits
  establish chat-template/tokenization and semantic compatibility, while both outputs
  remained coherent. Rapid-MLX also used its documented default int4 KV cache, unlike
  the llama-server execution path. R2 never required byte-identical full generation.
- Bounded logs contained no runtime error, traceback, crash, hang, or non-finite score.
  Rapid-MLX's SIGTERM stack dump was its expected signal-observability output during
  intentional shutdown and was followed by complete engine/server cleanup. Both owned
  process groups stopped, and ports `18082` and `18083` were closed.
- Independent rereview passed the 14-test focused recovery suite, the live complete
  toolchain-identity test, the real one-byte cache/output-bound test, Clippy with
  warnings denied, and `git diff --check`. The Verifier approved the code, corpus
  provenance, failure semantics, tensor evidence, runtime semantics, and cleanup state.
- The bounded machine report and logs are retained locally at
  `/tmp/llama-monitor-r2-host-gate/` for this checkpoint; they are evidence artifacts,
  not shipped application data.
- Eight superseded non-launchable caches remain cleanup debt and were not deleted:
  `f1c445614ffebf3aecfb4fac68e3bbf71b18c4b086a5324bc160e614afb38f06`
  (known-corrupt pre-fix F16),
  `3380a94ba4fd3051e68b025dfd9f38b055d3d0030066d6f63807952d7733eb97`
  (superseded F16 profile), and
  `32e1551e889bc055260bb83c3a55c3d5632cb3468ae487676020bcaae4ea6f67`
  (superseded Q8_0 profile), and
  `31593ba6879e2f601b0a6f079e27503005d751bb768c0fd56060c192d58c0e52`
  (pre-remediation final-profile F16),
  `063a8d96739d3c8e1f3dcff11fb00b8f5941c80a6d609c82bd1cd9fe37d94733`
  (pre-remediation Q8_0),
  `2f9f5c8709e4e586f7814f406f2099e3dd118994a1f28eb00b6bf79b6c7dc3ef`
  (pre-remediation Q6_K), and
  `cfba79ce6db8dd07aeead1375c944b5ea155f94077e94b496e3d3f52b9484d79`
  (pre-remediation Q4_K_M), and
  `390a32bf103be00e7e35fd36c78e3def327b4f7e2953dc84b8b0a24c69ffbc98`
  (superseded before authoritative asset closure was profile-pinned). They must be
  removed explicitly after review; R2 never treats them as launchable.

### R3 — Optional MLX re-quantization

- Use pinned official mlx-lm on the validated recovered FP16 result.
- Compare 4/6/8-bit recipes where supported.
- Gate: recorded fidelity/performance/size results and clear source/output labels.
- Checkpoint: `feat(models): add validated MLX quantization recipes`.

#### R3 checkpoint (2026-07-16)

Status: **complete; independent Verifier approved after hardening and detached-host
validation.** This authorizes R4 implementation only. Every production/`Verified`
promotion remains unauthorized.

Pinned quantizer:

- Official `mlx-lm==0.31.3` wheel `mlx_lm-0.31.3-py3-none-any.whl` (408,890 bytes,
  SHA-256 `758cfddf1180053b7613db76fad3d246a331a2a905808e1164a275621fc983b8`).
- Apple-Silicon `mlx==0.32.0` wheel
  `mlx-0.32.0-cp312-cp312-macosx_15_0_arm64.whl` (558,890 bytes, SHA-256
  `e5f778001562ccce26cf6e5be1050d2afc78e2902bad206201ab9f5a6d0f886a`).
- Execution uses the retained app-owned Rapid-MLX `0.10.10` qualification venv. Its
  exact 65-package, 6,770-file non-bytecode closure is pinned by environment SHA-256
  `a0a97c14483e1e24ba4e4dbac7505b54c8e686e90b8b77df32eb1f83f39f556a`.

Input and policy:

- The only accepted input is remediated R2 F16 cache
  `a21cca76ec236c3c71ea2bf5eb6f78716602b90fb16d78c3aef4da51e1ff4177`.
  Its manifest, report, weight, 272-tensor closure, and original GGUF `f16` tier are
  pinned and revalidated before and after quantization.
- R3 invokes official `mlx_lm.convert` offline with `dtype=float16`, uniform affine
  weight quantization, group size 64, no mixed predicate, and no remote code. Exactly
  three recipes exist: 4-, 6-, and 8-bit. Unknown recipes fail closed.
- Official `tokenizer.save_pretrained()` normalizes this fast-tokenizer layout by
  dropping merges/vocabulary/special-token files, extracting the chat template, and
  adding a model card. The wrapper rejects that ambiguity: it removes generated-only
  card/template files and restores all six R2 tokenizer/generation assets byte-for-byte.
- R3 directly reuses R2 canonical paths, immutable worker assets, complete environment
  verification, bounded blocking gate, offline process-group runner, cancellation,
  time/diagnostic/output/disk bounds, staging cleanup, recursive hashes, tamper checks,
  and atomic promotion. Outputs live below `models/rapid-mlx/requantized/`, remain
  outside inventory, use status `experimental_requantized_structurally_validated`, and
  are always `launchable: false`.

Structural and size evidence:

| Recipe | Immutable cache | Quantized modules | Output tensors | Complete bytes | Size vs recovered FP16 | Quantizer seconds |
|---|---|---:|---:|---:|---:|---:|
| affine 4-bit, group 64 | `bd494370cb354097bc67e714deb0f91d5ef6bb001e4cc8b4d695a0a6962e4522` | 211 | 694 | 79,216,280 | 29.1% | 0.657 |
| affine 6-bit, group 64 | `40244bc4b24630c490a003a615d33a8c7705e12aec2268b7646e2ec81e4038ab` | 211 | 694 | 112,836,563 | 41.4% | 0.715 |
| affine 8-bit, group 64 | `08f132594443be6efcfbb4b3d85c5c870e2c28048c06b702c16926e0384c5b29` | 211 | 694 | 146,456,727 | 53.8% | 0.727 |

Each recipe has exactly 211 scale and 211 quantization-bias tensors, exact config
metadata, finite floating tensors, complete hashes, and the exact R2 tokenizer identity.
Recovered FP16 is 272,437,300 bytes. Failed initial tokenizer-closure attempts removed
their output and left R3 staging empty; no partial cache was promoted.

Validation design:

- `tools/mlx_requantize/validate_fidelity.py` dequantizes each recipe through pinned
  mlx-lm, requires exact 272-key/shape closure against recovered FP16, and records
  max/mean absolute error, minimum cosine, worst tensors, and load time.
- `tools/mlx_requantize/run_host_gate.py` runs that check plus managed llama-server F16
  and recovered-FP16/4/6/8 Rapid-MLX targets sequentially. It records size, load,
  readiness/model/status/cache, two fixed greedy chats, usage/finish/text/top-five,
  throughput, deterministic behavior, candidate compatibility, and bounded diagnostics.
  It creates ephemeral API keys, forces offline/telemetry-disabled execution, tears down
  each process group, proves each port can be rebound, and requires human coherence
  review.
- Run the harness once in a normal interactive Terminal using the exact command in
  `docs/agents/gguf-recovery.md`. Direct sandbox Python has no Metal device, so its
  immediate failure is not fidelity evidence and must not be repeatedly retried.

Independent Verifier hardening and sign-off:

- Cache validation pins the exact 211 quantized modules, 694 output tensors, and each
  recipe's complete inventory SHA. It derives the cache key again from the manifest's
  recipe and rejects recipe substitution, cache-root symlinks, content tampering, and
  output beyond the declared byte bound.
- The host harness passes ephemeral API keys only through the child environment,
  redacts diagnostics, requires the served identity plus valid usage/finish data, and
  kills the complete process group even when the leader exits first. The fidelity
  validator rejects empty and non-finite results explicitly.
- Adversarial recipe-substitution and symlink tests joined the focused suite. Final
  independent evidence was 20 focused tests passed / 7 intentionally ignored, all
  three retained caches accepted under the stricter closure, the exact 65-package /
  6,770-file environment accepted, the real one-byte output-bound rejection accepted,
  and Python syntax, Clippy with warnings denied, Windows GNU, and `git diff --check`
  clean.

Detached-host evidence:

- The one-shot harness exited zero and retained its bounded machine report at
  `/tmp/llama-monitor-r3-host-gate/report.json`. It sequentially stopped every owned
  process group and successfully rebound every port. All four Rapid-MLX targets
  repeated byte-identical greedy output with valid usage/finish data, and human review
  found the llama-server reference plus every Rapid-MLX output coherent. The 4-bit
  response reached the 32-token bound but remained grammatical; 6-bit produced the
  best concise `stop` response at 25 completion tokens; 8-bit text was identical to
  recovered FP16.
- Exact measured results from this single cold sequential tiny-model run:

| Target | Load seconds | Completion tok/s | Fidelity vs recovered FP16: max abs / mean abs / min cosine | First-token compatibility |
|---|---:|---:|---|---|
| recovered FP16 | 0.0844 | 218.40 | baseline | ordered top-five matches llama GGUF; max logprob delta `0.02088` |
| affine 4-bit | 0.0860 | 188.25 | `0.550781` / `0.0115801` / `0.993535` | winner matches; four shared; order differs; max delta vs FP16 `1.671875` |
| affine 6-bit | 0.0872 | 174.03 | `0.136719` / `0.00277082` / `0.999611` | winner and all five candidates match; order differs; max delta `0.390625` |
| affine 8-bit | 0.0870 | 192.98 | `0.0390625` / `0.000686215` / `0.999955` | ordered top-five matches; max delta `0.078125` |
| llama-server source F16 GGUF | n/a | 304.29 | R2 source reference | reference |

- Fidelity is monotonic in the expected direction and all recipes retain exact
  272-source-tensor dequantized key/shape closure. This one sample does **not** show a
  throughput win: none of the MLX recipes outperformed recovered FP16. For a 135M
  model, fixed quantization/dequantization and dispatch overhead can dominate the
  memory-bandwidth savings. Treat these figures as bounded recipe evidence, not a broad
  performance ranking for larger models or production workloads.

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
