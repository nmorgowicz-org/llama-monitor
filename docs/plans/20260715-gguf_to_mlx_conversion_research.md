# GGUF to MLX Conversion Research & Experimental Import Plan

| Field | Value |
|---|---|
| Created | 2026-07-15 |
| Status | Research plan; intentionally outside the Rapid-MLX release gate |
| Product goal | Let users recover valuable GGUF-only finetunes for MLX without presenting reverse conversion as lossless or universally safe |
| Initial host | Apple Silicon macOS |
| Verified native runtime baseline | Rapid-MLX `v0.10.9` / mlx-lm |
| First upgrade qualification | Rapid-MLX `v0.10.10` at `5ca536275e89ddf0de3b49bd6f55fad80e42656e` |
| Candidate reverse converter | `barrontang/gguf2mlx` at audited SHA `6a0da6529f233df79362cbf62dd96221c895351f` |

## Executive Decision

GGUF-only conversion remains a desired llama-monitor capability, but it is a separate
Experimental Import Lab rather than a prerequisite for the first Rapid-MLX release.

Phase 5.5 also owns one bounded runtime-upgrade qualification. Rapid-MLX `v0.10.10`
was published on 2026-07-15 with release-artifact acceptance and publication-integrity
hardening plus the version bump. It does not advertise an inference feature or API
change, but release notes alone are not compatibility evidence. The lab must exercise
an isolated managed `v0.10.9` -> `v0.10.10` upgrade before using `v0.10.10` in GGUF
conversion parity gates. Phase 6 still owns the production updater and its app UI.

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

### R0 — Tool survey and evidence matrix

- Continue surveying maintained reverse-conversion tools.
- Record license, releases, activity, CLI/API, supported quant types, architecture
  mappings, tests, failure semantics, and security posture.
- Re-run the survey before selecting a production candidate.
- Gate: written selection with primary-source citations and rejected alternatives.
- Checkpoint: `docs(models): select GGUF recovery toolchain`.

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
- Gate: release build, dark/light screenshots, reduced motion, isolated Playwright,
  security review, and no native dialogs.
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
- Alternative wrapper under review:
  <https://github.com/acampkin95/gguf-to-mlx>

## Documentation Closeout

When the research track is completed or superseded:

1. Move durable user-facing source/quality/resource guidance into the model-management
   reference documentation.
2. Move converter/profile/cache/security contracts into developer documentation.
3. Record accepted/rejected tool decisions and verified SHAs in a concise dev note.
4. Archive this dated plan with a completion/supersession banner.
5. Remove contradictory historical conversion instructions from older Rapid-MLX plans.
