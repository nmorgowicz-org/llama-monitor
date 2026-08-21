# 2026-08-20 — Rapid-MLX non-greedy MTP + sidecar assembly

Branch: `fix/rapid-mlx-nongreedy-mtp`
Status: COMPLETE — Phases 3–6 implementation and validation are complete. The required live screen used the pinned custom Rapid-MLX fix build; full production qualification remains an explicit optional user operation.

## 2026-08-20 execution findings and tooling boundary

The public Hugging Face discussions for `nightmedia/Qwen3.8-27B-Brainwaves`
were readable without an HF token. Discussion #1 documents a merge artifact in
which `mtp.fc.weight` remained in the index but was absent from the shards;
Discussion #2 documents repairing the merged MTP set from the matching parent
and grafting/validating the result. This is the concrete precedent for the
parent-assisted repair path below.

The downloaded `nightmedia/Qwen3.6-35B-A3B-MTP-Holo3-Qwopus-*` fixture is a
different negative control: it advertises an allowlisted trunk `model_type` and
positive `mtp_num_hidden_layers`, but its tensor headers contain no MTP
namespace. It must be classified as a normal trunk with stale MTP metadata
removed; it must not be treated as a usable head and must not be silently
repaired from a guessed source.

The private sibling repo `../llama-local-tooling` is currently GGUF-oriented for
MTP: `Invoke-MtpGraft.ps1`, `prepare_mtp_hf_source.py`, `graft_mtp_gguf.py`,
`extract_mtp_gguf.py`, and `validate_mtp_graft.py` cover HF/GGUF preparation,
grafting, and validation. Its `Convert-HfModelToGguf.ps1` already accepts a
matching alternate MTP source. It does not yet produce a Rapid-MLX
`mtp.safetensors` sidecar. Any MLX repair path added here must therefore be
mirrored there with a standalone, structured MLX sidecar builder/validator;
the GGUF graft path and the future MTPLX loader remain separate mechanisms.

Repair source precedence is: exact BF16 parent/source; compatible published
HF MTP head; then a compatible integrated GGUF only after an explicit
dequantize/name-map conversion path exists. Parent discovery may suggest a
candidate from authoritative metadata, but pairing remains user-confirmed and
must be recorded in provenance. No filename-only inference is acceptable.

## Branch and commit convention

This is a **fix branch with fix commits**. The repo uses release-please, so the
commit type drives the version bump — `fix:` yields a patch release, which is
what this work is: correcting wrong copy, a wrong default, and a silent
trunk-corruption hazard. Do not use `feat:` for any commit on this branch, even
for Phase 4's new wrapper — it exists to repair the sidecar path, not to add a
product capability.

- Branch off `main`: `git switch -c fix/rapid-mlx-nongreedy-mtp`
- One commit per phase (or per task if a phase runs long), conventional format:

  ```
  fix(rapid-mlx): stop snapshot_download from poisoning trunks with MTP shards
  fix(wizard): correct greedy-only speculative decoding hint
  fix(rapid-mlx): default speculative ceiling to 3
  ```

- Scopes in use in this repo: `rapid-mlx`, `wizard`, `ci`, `docs`. Match the
  area you touched; do not invent new scopes.
- Docs-only commits (e.g. filling in this file's tables) use `docs:`.

## Reader's contract

This document is written to be executed by a model that has **not** read the
investigation transcript. Every claim it relies on is stated with the file and
line that proves it. **Do not skip the verification steps** — several of them
exist because the failure they guard against is *silent* (no error, no crash,
just degraded or corrupted output).

Three rules that apply to every phase:

1. **Never assume a gate passed.** Each phase ends with a Gate block. Run it.
   If it fails, stop and report — do not proceed to the next phase.
2. **Do not conflate the three speculative-decoding mechanisms in this repo.**
   See "Terminology" below. Touching the wrong one will look like it works and
   will be wrong.
3. **Nothing here waits on the upstream PR.** We authored rapid-mlx PR 2140 and
   build against our own PR branch. Assume it merges as-is or with minor
   reviewer-requested changes; if the reviewer changes the *behavior* contract
   (not just style), revisit Phase 1's copy. Every phase is executable now.

---

## Terminology — read this before touching any code

This repo contains **three unrelated things** that are all called "speculative
decoding". Mixing them up is the single most likely way to break this work.

| # | Mechanism | Runtime | Where it lives | In scope here? |
|---|-----------|---------|----------------|----------------|
| 1 | **Rapid-MLX MTP** — a multi-token-prediction head bolted onto a Qwen3.5-family trunk, loaded from a *sidecar* weights file | rapid-mlx (MLX) | `src/inference/rapid_mlx/`, `--speculative-config` | **YES — this is the whole document** |
| 2 | **llama.cpp GGUF draft model** — a separate small GGUF model used as a drafter | llama.cpp | `static/js/features/spawn-wizard-mtp-draft.js`, `src/models/mod.rs` (`is_draft_assistant`, `has_mtp`) | **NO — do not touch** |
| 3 | **MTPLX** — a third-party runtime metadata format | n/a | `src/hf/mod.rs:541` (`mtplx_runtime.json`) | **NO — do not touch** |

`spawn-wizard-mtp-draft.js` says so in its own header: "llama.cpp GGUF
speculative decoding … three unrelated speculative-decoding mechanisms."

**Key vocabulary for mechanism #1:**

- **Trunk** — the main MLX model directory (e.g. a quantized Qwen3.8-27B).
- **Sidecar** — the extracted MTP head weights, stored **outside** the trunk.
- **K / draft depth** — how many tokens the MTP head proposes per round.
- **EV controller** — rapid-mlx's adaptive depth chooser
  (`vllm_mlx/spec_decode/mtp/draft_k_controller_v2.py`). It picks K each round
  based on measured acceptance; `num_speculative_tokens` is only its *ceiling*.
- **`disable_auto_k`** — pins depth at the ceiling, bypassing the controller.

---

## Background: what is already true today

Verified during investigation. Do not re-derive; do verify anything you intend
to change.

### The launch knob exists and is fully wired

`--speculative-config '<JSON>'` accepts exactly these keys — unknown keys are a
**hard error** in rapid-mlx (`vllm_mlx/spec_decode/config.py`):

```json
{"method":"mtp","model":"<sidecar path>","num_speculative_tokens":3,"disable_auto_k":true}
```

Upstream mapping (`vllm_mlx/cli.py:2280-2311`):
`num_speculative_tokens` → `mtp_max_k`, `disable_auto_k` → `mtp_disable_auto_k`.
`scheduler.py:444-451` documents `mtp_max_k` as "the hard ceiling on the
per-round draft depth the controller may select" and `mtp_disable_auto_k` as
fixing "depth at `mtp_max_k` for A/B benching."

Our side is already typed, not stringly-typed
(`src/inference/rapid_mlx/mod.rs:45-74`):

```rust
fn default_num_speculative_tokens() -> u32 { 2 }
pub struct RapidMlxSpeculativeConfig {
    pub method: RapidMlxSpeculativeMethod,      // Mtp
    pub model: Option<String>,                  // sidecar path
    pub num_speculative_tokens: u32,            // validate()  1..=8
    pub disable_auto_k: bool,
}
```

Emission is capability-gated at `command.rs:483-498`; degradation with a stated
reason is at `rapid_mlx_runtime.rs:828-836`. Wizard emit is at
`spawn-wizard-rapid-mlx.js:899-910`; preset save/restore at `presets.js:1479-1489`
and `presets.js:2397-2405`; VRAM estimation consumes the typed struct at
`vram-estimate.js:33/38/49/154`.

**Conclusion: no new plumbing is needed for the knob itself.** Phases 1–2 are
copy, defaults, and gating only.

### `num_speculative_tokens` alone does NOT give you depth-3

This is the non-obvious part and the reason `disable_auto_k` must be reachable
in Pro mode. The EV controller settles on K=1 in practice. Measured on
Qwen3.5-9B: auto-K produced 42/70 rounds at run-length 1; pinned K=2 gave
60/118; pinned K=3 gave 74/138. Raising the ceiling changes nothing on its own.

### Model eligibility

`vllm_mlx/spec_decode/mtp/detect.py` applies **two** gates:

1. `model_type` ∈ `{"qwen3_5", "qwen3_5_moe", "hy_v3"}` (`detect.py:72-83`)
2. `_mtp_num_hidden_layers(config) > 0` — i.e. MTP weights actually present
   (`detect.py:197-209`, which warns "MTP-capable model_type but MTP weights not
   present on this checkpoint… stripped convert")

Qwen3.6-27B and Qwen3.8-27B MLX repos both report `model_type: qwen3_5` and pass
gate 1. Gate 2 is what the sidecar work in Phases 3–6 exists to satisfy. Note a
plain convert with MTP stripped (e.g. `mlx-community/Qwen3.6-27B-OptiQ-4bit`)
fails gate 2 and must fall back cleanly, not error.

### The in-trunk sensitivity is REAL and precisely understood

The user's recollection is correct. `scripts/build-mtp-head.py` documents the
mechanism in its own docstring:

> `mlx_lm` globs `model*.safetensors` when loading a trunk, so an in-trunk
> sidecar is picked up as a trunk shard. That sets `should_shift_norm_weights`
> and applies +1.0 to *every trunk RMSNorm weight*. On an already-converted MLX
> checkpoint the weights are already shifted, so the trunk is double-shifted
> into gibberish — silently, with no error.

The upstream extractor also **rewrites the trunk's `config.json` in place** to
add `mtp_num_hidden_layers`.

There is a second, independent silent failure: a stale extractor that misses the
`pre_fc_norm_*` keys yields norm means of ~-0.44 instead of ~+0.56, producing
**~0% draft acceptance with no error**. `build-mtp-head.py` guards this with
`validate_norms()` (dies if any `pre_fc_norm_*` mean `<= 0`) and
`STALE_EXTRACTOR_MEAN = -0.44`.

### What already exists to handle it

`scripts/build-mtp-head.py` (391 lines) already:

- refuses an extractor that lacks `pre_fc_norm` (`verify_extractor()`)
- refuses `--out` pointing inside the trunk
- refuses to run if `model-mtp.safetensors` is already staged in the trunk
- snapshots `config.json` and restores it in a `finally` block
- quarantines leftovers to `.quarantine-in-trunk-sidecars`
- runs the `pre_fc_norm` sign check
- writes `provenance.json` with `status: "built_unvalidated_online"`

`src/inference/rapid_mlx/sidecar_inventory.rs` already implements the
"separate dir that gets auto-pointed at" the user described:

```
~/.config/local-llm-foundry/models/rapid-mlx/mtp-sidecars/<trunk-slug>/
    mtp.safetensors
    provenance.json
```

with `discover_sidecars()`, `parse_provenance()`, `find_sidecar_for_path()`, and
`estimate_local_companion_vram()`.

`src/inference/rapid_mlx/spec_decode_store.rs` already records served-acceptance
verdicts to `spec-decode-verdicts.json` (schema v1).

### The one genuine hole in the download path

`src/inference/rapid_mlx/model_resolver.rs:1408`:

```rust
let code = "from huggingface_hub import snapshot_download; import sys; print(snapshot_download(repo_id=sys.argv[1], revision=sys.argv[2], cache_dir=sys.argv[3]))";
```

**No `allow_patterns`. No `ignore_patterns`.** The whole repo is pulled. This
means:

- Adopting a pre-quantized MTP repo (e.g. `mlx-community/Qwen3.8-27B-MTP-4bit`)
  lands `model-mtp.safetensors` **inside the trunk** → double-shift on load.
- Converting from a bf16 source that carries MTP weights poisons the convert,
  because `mlx_lm.convert` loads through the same glob.
- `validate_mlx_load()` (`model_resolver.rs:1441+`) runs `mlx_lm.load` and will
  **succeed** on a double-shifted trunk. It does not catch this.

`src/model_download.rs` is a generic single-file downloader (GGUF-oriented) and
is **not** on this path — do not modify it.

---

## Scope

**In scope:** Rapid-MLX MTP only (mechanism #1).

**Out of scope, do not modify:**
`spawn-wizard-mtp-draft.js`, `src/models/mod.rs` (`is_draft_assistant`/`has_mtp`),
`src/model_download.rs`, `src/hf/mod.rs:541` MTPLX handling.

---

## Phase 0 — Test runtime, not a version gate

PR 2140 (ours) fixes a **sampling-correctness** defect: the K≥2 verify path used
one shared `u ~ U(0,1)` across all proposed positions instead of an independent
draw per position. Accepting position 1 implies `u < p(d1)/q(d1)`, which biases
`u` small and inflates position 2's acceptance, skewing the emitted token toward
the draft distribution. Measured against the defect: TV 0.029 (K=2), 0.066
(K=3), 0.024 under top-p — on the **marginal**, not just the joint.

### No version-floor gating

An earlier draft of this plan proposed a `NONGREEDY_MTP_MIN_VERSION` constant
plus semver comparison plus a disabled-state UI below the floor. **That is
deliberately not being built.** The product assumption is that the user is on a
current rapid-mlx build — the runtime loaders already carry update
notifications, which is the mechanism that keeps that true. Adding a second,
speculative-decoding-specific version gate duplicates that machinery for one
feature and creates a UI state (controls visible but disabled with a version
explanation) that most users would never see and that we would have to keep
accurate against every future release.

Keep the gating that already exists and is correct:

- `command.rs:483-498` — emit `--speculative-config` only when the scraped
  capability list contains it.
- `rapid_mlx_runtime.rs:828-836` — degrade with a stated reason when it is
  absent.

Neither needs to change. If a user on a stale runtime enables sampling with
K≥2, they get the pre-2140 distribution skew; that is a quality regression on an
out-of-date build, not a crash or a corruption, and the update path already
addresses it.

### Tasks

- **0.1** Point the dev environment at our PR 2140 branch of rapid-mlx so
  Phases 1–2 can be exercised end-to-end.

  PR 2140 is ours, from our fork. Clone it somewhere durable — **not** a
  scratchpad or temp directory, since Phases 1–6 all need it:

  ```sh
  git clone https://github.com/nmorgowicz/Rapid-MLX.git ~/src/rapid-mlx
  cd ~/src/rapid-mlx
  git remote add upstream https://github.com/raullenchai/Rapid-MLX.git
  git switch feat/mtp-non-greedy-sampling
  uv sync                    # creates ./.venv with rapid-mlx installed editable
  ```

  As of this writing the branch is at `8c02ed3`; the fix commit itself is
  `7ce01bd1 fix(mtp): draw an independent uniform per draft position`. Record
  the **commit SHA** you actually built, not the branch name — the branch moves
  as review lands.

  **How the foundry picks this up:** `Discovery::resolve_binary`
  (`discovery.rs:13-54`) resolves in the order explicit path → managed path →
  `PATH` lookup for `rapid-mlx` then `vllm-mlx`. Nearly every call site passes
  `None, None`, so the PATH branch is the one that runs. Putting the fork's venv
  first on `PATH` *is* the integration — there is no config key to set:

  ```sh
  export PATH="$HOME/src/rapid-mlx/.venv/bin:$PATH"
  which rapid-mlx    # must print the fork's venv, not homebrew/pipx
  ```

  Confirm the foundry agrees before trusting any Phase 1–2 result: the runtime
  page reports the resolved binary path and classifies it via
  `classify_source` (`discovery.rs:56-73`), which should read `Pip` for a
  `.venv/bin/` path. If it says `Homebrew` or `Pipx`, the shell export did not
  reach the process that spawned the app. Capability snapshots are keyed by
  executable identity and revalidated per path, so swapping binaries does not
  need a manual cache clear.
- **0.2** Run the distribution tests against that build to confirm the fix is
  live in the runtime you are testing with. These live in the **rapid-mlx**
  checkout, not this repo:

  ```sh
  cd ~/src/rapid-mlx && uv run pytest tests/test_mtp_nongreedy_distribution.py
  ```

  All tests must pass. They are the tests that fail at TV 0.029 (K=2) / 0.066
  (K=3) against the defect, so a pass is positive evidence the runtime under
  test is the fixed one — which is why this substitutes for a version check.
  They mock the model and need no weights, so they run in seconds on any Mac.
- **0.3** If the reviewer lands behavior changes beyond the independent-`u`
  draw, re-read Phase 1's copy against the merged behavior before shipping.
  Style-only or naming-only review changes need no revisit.

- **0.4** Fetch the survey fixtures. Phases 3–6 assert against real repo
  layouts, and several gates cannot be satisfied without the bytes on disk. Pull
  all ten repos from "Observed MTP filenames" — **~70 GB total**, so start this
  early and let it run while you work Phases 1–2.

  Download into the normal HF cache (`~/.cache/huggingface/hub`) so the
  foundry's own `snapshot_download` path finds them already cached and Phase 3
  exercises classification rather than the network:

  ```sh
  # Tier A — head-only repos, ~1.6 GB total. Required for 3.1 and 4.2a.
  for r in \
    mlx-community/Qwen3.8-27B-MTP-mxfp4 \
    mlx-community/Qwen3.8-27B-MTP-4bit \
    mlx-community/Qwen3.6-27B-MTP-4bit \
    mlx-community/Qwen3.6-35B-A3B-MTP-4bit \
    mlx-community/Qwen3.5-9B-MTP-4bit \
    inferencerlabs/Qwen3.8-27B-MTP-MLX
  do hf download "$r"; done

  # Tier B — trunk + in-trunk sidecar, ~34 GB. Required for 3.2a and Gate 3.
  hf download Shiftedx/Qwen3.8-27B-MLX-MXFP4-MTP                 # 16.1 GB
  hf download AutomatosX/AX-Qwen3.8-27B-MLX-AXQ-4bit-MTP         # 18.2 GB

  # Tier C — trunks with MTP fused, ~34 GB. Negative controls only.
  hf download fcmeyer/Qwen3.8-27B-MLX-oQ4e-mtp                                        # 17.0 GB
  hf download dawncr0w/Qwen3.6-27B-uncensored-heretic-v2-Native-MTP-Preserved-oQ4-MLX # 17.0 GB
  ```

  ```sh
  # Tier D — config-lies negative control, ~19.3 GB. Any of the five
  # nightmedia variants works; all share the defect.
  hf download nightmedia/Qwen3.6-35B-A3B-MTP-Holo3-Qwopus-mxfp4-mlx
  ```

  Tier D is the only fixture that catches a classifier trusting `config.json`
  over tensor keys — see consequence 5 under "Observed MTP filenames". If Tier C
  is dropped for disk, **keep Tier D**: it tests something no other fixture
  does.

  If either oMLX repo in Tier C proves troublesome, substitute
  `t0rr3sp3dr0/Qwen3.8-27B-MLX-MTP-mxfp4` (mxfp4, verified equivalent). See
  "Non-oMLX alternatives" for the full list and what each covers.

  **If disk is tight, drop Tier C first.** Those two only prove that a trunk
  with no MTP-named file is adopted byte-identically — a negative control. Tier
  B cannot be dropped: `Shiftedx` is the only fixture carrying a vision shard
  that matches the trunk glob, which is the exact case task 3.3a exists for, and
  it is the only mxfp4 trunk+sidecar available. Tier A cannot be dropped; it is
  the path most users take.

  Note the deliberate contrast inside Tier B: `Shiftedx` names its vision tower
  `model-vision-00001-of-00001.safetensors` (**matches** `model*.safetensors`),
  while `AutomatosX` names its `vision.safetensors` (**does not match**). Having
  both on disk is what makes Gate 3's vision-shard check meaningful — one repo
  reaches the classifier and one never does, and the plan should behave
  correctly for each without the filenames deciding anything.

  Verify before proceeding: `du -sh ~/.cache/huggingface/hub` and confirm each
  repo's `snapshots/<rev>/` contains the file list the survey table records.
  A partial or resumed download that silently dropped a shard will surface in
  Phase 3 as a misclassification, which is much harder to diagnose there.

### Gate 0

- [x] The rapid-mlx build under test is recorded by commit SHA in the PR
      description for this branch.
- [x] `which rapid-mlx` resolves to the fork's venv, and the app's runtime page
      reports that same path with source `Pip`.
- [x] `test_mtp_nongreedy_distribution.py` passes against that build.
- [x] All Tier A and Tier B fixtures from 0.4 are present in the HF cache with
      complete file lists matching the survey table. Tier D present. Tier C
      present, or its absence recorded as a knowingly skipped negative control.
- [x] No `NONGREEDY_MTP_MIN_VERSION` or equivalent version-floor constant exists
      anywhere in `src/`. Check: `grep -rn "MIN_VERSION" src/inference/rapid_mlx/`

---

## Phase 1 — Truthful copy

The current wizard hint at `static/index.html:4146` reads:

> "Not recommended for normal chat or coding: Rapid 0.11.1 only engages on
> greedy, unconstrained requests. Sampling and constrained tool calls silently
> fall through."

Post-2140 the first sentence is false for sampling (it remains true for
constrained/tool-call requests). Shipping the old copy against a fixed runtime
tells users to avoid a feature that now works.

### Tasks

- **1.1** Replace the hint with a **single static string** describing current
  runtime behavior. No conditional variants, no version interpolation:

  > "Works with sampling (temperature, top-p). Constrained decoding and
  > tool-call grammars still bypass speculation."

  Do **not** claim tool calls are fixed. PR 2140 addressed sampling only — the
  second sentence is still true and must stay.

  Note the current string names a specific version ("Rapid 0.11.1 only engages
  on greedy…"). Do not replace it with a different hardcoded version; drop the
  version reference entirely. A version baked into copy goes stale silently.

- **1.2** Add hint text for `spawn-rapid-speculative-disable-auto-k`, which has
  none today. Required content: raising the token count alone usually does not
  increase depth, because the adaptive controller re-picks K each round from
  measured acceptance; this toggle pins depth at the chosen value. Note it can
  *reduce* throughput when acceptance is low, and that it is the setting to use
  for A/B benchmarking.
- **1.3** Mirror both hints into the preset editor row
  (`static/index.html:3439-3476`, `pe-row-rapid-speculative`).
- **1.4** Leave the existing capability-driven disable path alone. When the
  runtime does not advertise `--speculative-config`, the controls should remain
  **visible but disabled** with the reason shown inline — that is existing
  behavior and it is correct. Do not add a second disable path.
- **1.5** Verify the degradation message at `rapid_mlx_runtime.rs:836` still
  reads correctly as-is. It is not version-aware and should not become so.

### Gate 1

- [x] No string anywhere in `static/` still asserts "only engages on greedy".
      Check: `grep -rn "only engages on greedy" static/`
- [x] No speculative-decoding hint string contains a hardcoded version number.
      Check: `grep -rn "Rapid 0\." static/`
- [x] `disable_auto_k` has visible hint text in **both** wizard and preset editor.
- [x] `npm test` (or the repo's JS test command) passes.

---

## Phase 2 — Guided defaults

The guided/pro split **already works** and needs no structural change. The
`companions` group in `spawn-wizard-mlx-ia.js:85-94` is `critical: false`, and
`spawn-wizard-ia.js:12-16` opens `critical: false` groups only at the Advanced
profile. Quick/Balanced therefore collapse these controls behind `<details>`
with defaults applied; Advanced expands them. **Do not restructure the IA.**

Only the default *value* changes: 2 → 3.

Rationale: with `disable_auto_k` **off** (the guided default),
`num_speculative_tokens` is only a ceiling, and the EV controller will drop below
it whenever acceptance is poor. A ceiling of 3 gives the controller room to
exploit a good drafter and costs nothing when the drafter is weak. It is the
direct analogue of llama.cpp's `--draft-max 3`.

### Tasks

Change the default in all four places. Missing one produces a UI that shows 3
and launches 2, or vice versa.

- **2.1** `static/index.html` — `spawn-rapid-speculative-tokens`: move `selected`
  from the `value="2"` option to `value="3"`.
- **2.2** `static/index.html` — `modal-rapid-speculative-tokens`: same change.
- **2.3** `static/js/features/spawn-wizard-rapid-mlx.js:907` —
  `Number(h.speculativeTokens || 2)` → `|| 3`.
- **2.4** `src/inference/rapid_mlx/mod.rs` —
  `fn default_num_speculative_tokens() -> u32 { 2 }` → `{ 3 }`.
- **2.5** Leave `disable_auto_k` defaulting to **false**. Guided users get
  adaptive depth; Pro users opt into pinning.
- **2.6** Update the `command.rs` serialization tests (around `:891-907`) that
  assert the exact emitted JSON.
- **2.7** Confirm `vram-estimate.js` reflects the new default without code
  change (it reads the typed struct).

### Gate 2

- [x] `grep -rn "speculativeTokens || 2" static/js/` returns nothing.
- [x] `grep -rn "default_num_speculative_tokens" src/` shows `{ 3 }`.
- [x] Fresh wizard at Quick profile emits `"num_speculative_tokens":3`.
- [x] Fresh preset editor shows 3 selected.
- [x] `validate()` still rejects 0 and 9 (range `1..=8` unchanged).
- [x] Existing saved presets with an explicit `2` still load as `2` — the
      default must not overwrite stored values.
- [x] `cargo test` and the JS test suite pass.

---

## Phase 3 — Stop poisoning trunks at download time

**Highest-severity phase in this document** — it is the only one guarding
against silent output corruption.

**Read "Observed MTP filenames" at the end of this document before writing any
code here.** The survey is done, and it invalidates the obvious approach:
filename-based `ignore_patterns` is wrong in both directions. Specifically —

- The `mlx-community/*-MTP-*` repos are **standalone head-only repos** whose
  weights file is literally named `model.safetensors`. No `*mtp*` pattern
  matches it, and any pattern that did would also match real trunk shards.
- `mtp.safetensors` (Shiftedx, AutomatosX) does **not** match `model*.safetensors`
  and therefore never triggers the double-shift glob. Excluding it would delete
  the exact file we want to relocate into the sidecar directory.
- The `model-mtp.safetensors` shape that `build-mtp-head.py` guards against was
  **not observed in any published repo**. It is the local extractor's output
  name — a real hazard, but one produced by our own tooling, not by downloads.

So the decision procedure is **repo-shape classification plus tensor-key
inspection**, not globbing.

### Tasks

- **3.1** Classify the repo **before** treating it as a trunk. Read
  `config.json` from the snapshot and branch on `model_type`:

  - `qwen3_5_mtp` ⇒ this is a **head-only sidecar repo**, not a model. It must
    never be adopted as a trunk. Route it to Phase 4's registration path
    (see 4.2a) and do not run `validate_mlx_load()` on it — `mlx_lm.load` on a
    bare head is meaningless.
  - `qwen3_5` / `qwen3_5_moe` / `hy_v3` ⇒ real trunk. Continue to 3.2.
  - anything else ⇒ real trunk, not MTP-eligible. No hygiene work needed.

  This is the same `model_type` allowlist `detect.py:72-83` applies, plus the
  head-only `qwen3_5_mtp` case that the allowlist does not cover because
  upstream never loads a bare head as a model.

- **3.2** For real trunks, run a **tensor-key hygiene scan** over every file
  matching `model*.safetensors` in the snapshot. Read each file's safetensors
  header only (the key list — do not load tensors; a 27B trunk will not fit in
  the check's memory budget). A file is an MTP head iff its key set contains
  `pre_fc_norm_*` keys. If one is found:

  - move it to the sidecar staging area (Phase 4), and
  - restore `config.json` if it carries `mtp_num_hidden_layers`.

  Filename is **not** the decision and must not be used as one. It is acceptable
  to use `model*.safetensors` as the *enumeration* filter, since a file outside
  that glob cannot trigger the double-shift regardless of its contents.

- **3.2a** Also relocate an in-trunk `mtp.safetensors` when present (the
  Shiftedx / AutomatosX shape). This one is **not** a corruption hazard — it
  does not match the trunk glob — but it is a sidecar sitting in the wrong
  place, and relocating it is how the trunk becomes speculation-capable without
  a bf16 extraction. Apply the same `pre_fc_norm_*` key check before accepting
  it. Leave `mtplx_runtime.json` and `axquant_*` files alone; they belong to
  other tooling (mechanism #3, out of scope).

- **3.3** Do **not** add `ignore_patterns` to the `snapshot_download` call at
  `model_resolver.rs:1408`. The survey shows no pattern set that is both safe
  and useful. Downloading the full repo and then classifying it is correct here
  — the files we would want to skip are files we actually want, just not in the
  trunk directory.

  If a future need for `ignore_patterns` appears, pass patterns as additional
  `sys.argv` entries rather than interpolating them into the Python source
  string; interpolation there is an injection surface.

- **3.3a** One caveat found in the survey, flagged rather than fixed: Shiftedx's
  repo ships `model-vision-00001-of-00001.safetensors`, which **does** match
  `model*.safetensors`. It is a vision tower, not an MTP head, so the
  `pre_fc_norm_*` check correctly leaves it alone — but confirm that during
  testing rather than assuming it. If the check ever misclassifies a vision
  shard as a head, the trunk loses its vision tower silently.

- **3.4** Extend the hygiene check to snapshots that already exist on disk from
  before this change — a one-shot scan at startup or first rapid-mlx spawn.
  Users who already adopted an MTP repo have a poisoned trunk right now and no
  symptom other than bad output.

- **3.5** Do **not** modify `validate_mlx_load()` to try to detect double-shift.
  It cannot: a double-shifted model loads cleanly. The fix is prevention.

### Gate 3

- [x] `mlx-community/Qwen3.8-27B-MTP-mxfp4` is classified as a **head-only
      repo** and is never presented as a spawnable model.
- [x] `Shiftedx/Qwen3.8-27B-MLX-MXFP4-MTP` adopts as a trunk with its
      `mtp.safetensors` relocated to the sidecar directory, and the 18 trunk
      shards left intact.
- [x] `model-vision-00001-of-00001.safetensors` from that same repo is **still
      present in the trunk** afterward (3.3a).
- [x] The trunk's `config.json` after adoption contains **no**
      `mtp_num_hidden_layers` key.
- [x] The legacy scan (3.4) detects and repairs a deliberately poisoned trunk
      you construct by copying an MTP head in as `model-mtp.safetensors`.
- [x] A **non-MTP** repo adoption is byte-identical to before this change. Prove
      it — diff the resulting trunk directory listing against a pre-change run.
- [x] A normal `model-00001-of-00018.safetensors` still downloads and is not
      touched by the hygiene scan.
- [x] The header-only read in 3.2 does not load tensor data. Verify by peak RSS
      on a 27B trunk, not by inspection.
- [x] **A `nightmedia/Qwen3.6-35B-A3B-MTP-Holo3-Qwopus-*` variant classifies as
      NOT MTP-capable**, despite passing both config gates
      (`qwen3_5_moe` + `mtp_num_hidden_layers: 1`) and carrying "MTP" in its
      name. This is the config-lies case from consequence 5; a classifier that
      trusts config instead of tensors passes every other item on this list and
      fails this one.
- [x] Key matching accepts **both** the `mtp.`-prefixed and bare tensor
      namespaces (consequence 6) — assert against one repo of each.
- [x] Determined by test whether `mlx_lm`'s `model*.safetensors` glob reaches
      `mtp/model.safetensors` one directory down. Record the answer in this
      document; if it does match, the `mtp/` subdir layout is a new double-shift
      hazard and Phase 3 must handle it.
- [x] `cargo test` passes.

---

## Phase 4 — App-driven sidecar assembly

`scripts/build-mtp-head.py` is correct but manual: it requires `--bf16-source`
and `--mlx-model` typed by hand, and nothing in the app invokes or offers it.

### Tasks

- **4.1** Add a Rust wrapper that invokes `build-mtp-head.py` with the app's
  pinned Python, using `run_command_bounded` with a generous timeout (extraction
  on a 27B bf16 source is slow) and secret redaction, matching the conventions
  already used in `model_resolver.rs`.
- **4.1a** Add the corresponding MLX-side repair/validation CLI to the private
  `llama-local-tooling` repo. It must emit structured JSON and provenance,
  support complete-head extraction and parent-assisted missing-tensor repair,
  and share the same tensor-key, shape, namespace, and normalization gates as
  the app. Do not duplicate or reinterpret the existing GGUF graft contract.
- **4.2** Feed it from Phase 3 where possible: when 3.3 relocates an MTP shard
  out of a trunk, that shard **is** the sidecar payload — normalize it into
  `mtp-sidecars/<slug>/mtp.safetensors` and synthesize `provenance.json` rather
  than re-extracting from a bf16 source. Extraction is the fallback for trunks
  that never carried MTP weights.
- **4.2a** Add a **third, cheapest path: direct adoption of a head-only repo.**
  The survey found that `mlx-community/*-MTP-*` publishes bare heads (225–475 MB)
  with `model_type: qwen3_5_mtp` and unprefixed weight keys (`fc.weight`,
  `layers.0.*`). For these there is nothing to extract and nothing to relocate —
  download, verify, rename `model.safetensors` to `mtp.safetensors`, and register
  under the matching trunk's slug. This is the path most users will hit, and it
  needs no bf16 source and no 27B-scale compute.

  The rename is required, not cosmetic: leaving the file named
  `model.safetensors` in the sidecar directory means any future code that globs
  that directory the way `mlx_lm` globs a trunk would ingest it. Normalize on
  ingest.

  Pairing a head to a trunk is the user's choice, not an inference — the repo
  name is the only signal and it is unreliable. Offer the detected trunk as a
  default and let the user confirm. A head paired to the wrong trunk fails the
  `pre_fc_norm` check only sometimes; more often it produces ~0% acceptance,
  which looks like "speculation just isn't helping" rather than a mistake.

- **4.3** Preserve **every** safety behavior of the script. Do not reimplement
  it in Rust. Specifically the wrapper must not bypass: `verify_extractor()`,
  the refusal to write into the trunk, the `config.json` snapshot/restore
  `finally`, the quarantine of leftovers, or `validate_norms()`.
- **4.4** For the relocation path in 4.2, run the **same** `pre_fc_norm` sign
  check the script runs. A relocated shard from an unknown publisher has not
  been validated by anyone. Mean `<= 0` ⇒ reject the sidecar, do not register it.
- **4.5** Write `provenance.json` with the fields
  `sidecar_inventory.rs::SidecarProvenance` expects: `trunk`, `bf16_source`,
  `built_at`, `sha256`, `norm_check_passed`, `estimated_memory_bytes`,
  `quantization`, `mtp_depth_max`. For relocated shards set `bf16_source` to the
  originating repo id and add a status distinguishing relocation from extraction.
- **4.6** Surface failure honestly in the UI. A failed norm check must say the
  head is unusable and speculation will stay off — never silently continue.

### Gate 4

- [x] Extraction path produces `mtp.safetensors` + `provenance.json` under
      `~/.config/local-llm-foundry/models/rapid-mlx/mtp-sidecars/<slug>/`.
- [x] Relocation path (from Phase 3) produces the same layout.
- [x] Head-only adoption path (4.2a) produces the same layout, with the file
      renamed from `model.safetensors` to `mtp.safetensors`.
- [x] `provenance.json` parses cleanly via `parse_provenance()`, and its status
      field distinguishes all three origins.
- [x] `norm_check_passed: true` on a known-good build. Positive control:
      `mlx-community/Qwen3.6-27B-MTP-4bit` (named as such in `build-mtp-head.py`).
- [x] A deliberately corrupted head (negate the `pre_fc_norm_*` tensors) is
      **rejected**, not registered.
- [x] The trunk's `config.json` is unchanged before vs. after a build. Compare
      hashes.
- [x] Killing the wrapper mid-run leaves the trunk clean — the `finally` restore
      still executes.

---

## Phase 5 — Auto-point at the sidecar

### Tasks

- **5.1** On rapid-mlx spawn, call `find_sidecar_for_path()` for the selected
  trunk and populate `RapidMlxSpeculativeConfig::model` automatically.
- **5.2** Show the resolved sidecar path in the wizard (there is already a
  `spawn-rapid-speculative-sidecars-list` element). Auto-selection that the user
  cannot see or override is worse than manual selection.
- **5.3** Keep manual override working. Auto-point sets a default; it does not
  lock the field.
- **5.4** If no sidecar is found, the speculative controls must degrade to
  "off with a stated reason", consistent with `rapid_mlx_runtime.rs:836`.
- **5.5** Respect gate 2 of `detect.py`: if the trunk's `model_type` is not in
  the allowlist, do not offer speculation at all, regardless of sidecar presence.
- **5.6** Include the sidecar's `estimated_memory_bytes` in the VRAM estimate via
  `estimate_local_companion_vram()`.

### Gate 5

- [x] Spawning a trunk with a registered sidecar auto-fills the path.
- [x] The path is visible in the UI and editable.
- [x] A trunk with no sidecar shows speculation off with a reason, not an error.
- [x] A non-allowlisted `model_type` (e.g. a Llama MLX model) never offers
      speculation.
- [x] VRAM estimate increases when a sidecar is selected.
- [x] Launch command contains the correct `--speculative-config` JSON — verify
      the actual emitted argv, not the UI state.

---

### Gate 0–5 checklist reconciliation — 2026-08-20

The historical Gate 0–5 checkboxes are reconciled here against current source,
focused tests, immutable fixture snapshots, and the receipts below. A checked
item means it has positive evidence or an explicit negative/deferred fixture
disposition; it does not turn a rejected fixture into a supported sidecar.

- **Gate 0 — fixed runtime and fixtures:** the required runtime is
  `/Users/nick/src/rapid-mlx/.venv/bin/rapid-mlx`, version `0.12.17`, commit
  `8c02ed35e2f34b37e8e1365d9510a5bd5b180679`; the custom distribution test is
  `5 passed in 75.01s`. All planned Tier A and Tier B snapshots plus the Tier C
  negative controls are present in `/Users/nick/.cache/huggingface/hub`. The
  optional `t0rr3sp3dr0` Tier C snapshot is absent and is explicitly recorded
  as a skipped negative fixture; no gate result relies on it. No version-floor
  constant exists.
- **Gate 1 — truthful copy:** no `only engages on greedy` or `Rapid 0.*`
  strings remain in `static/`; both Wizard and Preset Editor retain the
  `disable_auto_k` explanation. This repository has no `npm test` script;
  `npm run validate-js` and `npm run lint` are the applicable JS gates and pass.
- **Gate 2 — defaults:** Quick/Basic and Preset Editor controls emit/select 3,
  Rust defaults to 3, explicit saved value 2 remains preserved, and validation
  remains bounded to 1–8. Focused Rust and JS validation gates pass.
- **Gate 3 — Shiftedx/vision matrix:** Shiftedx revision
  `df861d199426f8166bc138567894c61d1a42e4bb` retains all 18 trunk shards and
  its `model-vision-00001-of-00001.safetensors` vision file (921,497,189 bytes);
  the trunk index has zero MTP keys, while its external `mtp.safetensors`
  validates with positive norms. AutomatosX revision
  `1327acde70f0480cc10ab7dc8ffe043dce9b5de5` retains its separately named
  `vision.safetensors` (921,497,320 bytes), also header-contains no MTP keys;
  its published sidecar is rejected by the norm check (`-0.4590`, `-0.1572`)
  and is not registered. The nightmedia config-lie fixture remains `none`.
- **Gate 4 — assembly:** head-only adoption, relocation validation, corrupted
  head rejection, provenance handling, and config restoration are covered by
  the focused repair/hygiene suites. The real BF16 extraction positive control
  now passes using fcmeyer revision `fe34c8d6784c6d9b463756dd020492123137b732`
  (MTP tensors in shard 11) and Frosty40 revision
  `c4261c348aa8bdacbfac2dfbcb26ce284fedbe29` (bare-key BF16 source). Both
  produce the same validated external sidecar SHA-256
  `a010fd0259712851b8dcf5567066c88831305597061586aaa1550af5b84734c0` with
  positive `pre_fc_norm` means `+1.5391` and `+1.8359`.
- **Gate 5 — launch/UI:** managed sidecar auto-selection, editable path,
  speculation-off fallback, model eligibility, VRAM companion accounting,
  and exact `--speculative-config` emission are covered by the existing
  focused tests and the release-built UI evidence. The extracted positive
  control remains isolated under `/private/tmp/rapid-mlx-phase4-bf16-positive/`;
  no cache or managed trunk was mutated.

### Phase 5 implementation evidence — 2026-08-20

- `effective_speculative_config()` now auto-selects only a validated managed
  sidecar for an absolute, allowlisted Rapid-MLX trunk. An explicit manual
  sidecar remains intact for model aliases and local trunks.
- The wizard matches sidecars by provenance trunk, clears a stale
  auto-selected path after a model change, keeps typed/list-selected paths as
  manual overrides, and displays the degraded reason when no usable sidecar
  matches.
- Local `mtp.safetensors` bytes are carried as an approximate external
  companion in the canonical VRAM estimate and included in `total_bytes`.
- Rust tests cover eligible/non-eligible inventory selection, manual override
  preservation, external companion accounting, and exact emitted
  `--speculative-config` JSON. The Rapid-MLX wizard capture was rerun outside
  the sandbox on the existing `spawn-wizard-rapid-guided-baseline` scenario;
  its receipt is under `docs/screenshots/artifacts/wizard-rapidmlx/`.
- Validation: `cargo clippy -- -D warnings`, `cargo test` (1336 passed, 14
  ignored), `npm run validate-js`, `npm run lint`, `cargo build --release`,
  `cargo fmt -- --check`, and `git diff --check` pass.

### Phase 5 UI completion note

The preset editor now exposes first-class build/repair and validate actions for the selected local MLX trunk, accepting a direct MLX source, NuSLERP recipe, or pinned BF16 source. Repair and validation buttons remain disabled for the full managed job lifecycle, including status-fetch errors, so concurrent jobs cannot be launched. Models Library local MLX cards open this same shared repair surface with the trunk preselected.

### Phase 6 implementation note

The preset editor now offers a separate authenticated `Requalify sidecar` job after a managed sidecar exists. The job serializes with repair/validation, allocates an ephemeral loopback port, runs the configured `rapid-mlx-requalify-spec-decode.mjs` recipe with the selected sidecar and requested depth settings, then ingests the completed report through `SpecDecodeVerdictStore::ingest_requalification_report`. The report and schema-v1 verdict retain `num_speculative_tokens` and `disable_auto_k`; the sampled gate remains explicit at temperature 0.6. A safetensors structural preflight records malformed subject heads as `StillBlocked` rather than an ambiguous harness failure. Sidecar provenance records the served result, and the editor renders `Qualified`, `StillBlocked`, and `Uninterpretable` distinctly.

## Phase 6 — Requalification and verdict recording

### Tasks

- **6.1** Wire `scripts/rapid-mlx-requalify-spec-decode.mjs` to run after a
  sidecar is built or relocated. `build-mtp-head.py` already prints the follow-up
  command; the app should offer to run it rather than expecting the user to
  paste it.
- **6.2** Ingest the result through
  `SpecDecodeVerdictStore::ingest_requalification_report`, producing a
  `MeasuredSpecDecode` with the correct `rapid_mlx_version`.
- **6.3** Add a gate to the recipe (`scripts/spec-decode-recipe.json`) that runs
  a **sampled** (non-greedy) request, not only greedy. Pre-2140 this is exactly
  the configuration that was silently wrong; post-2140 it is the configuration
  users will actually run. A greedy-only qualification proves nothing about it.
- **6.4** Record `num_speculative_tokens` and `disable_auto_k` in the verdict so
  a qualification at K=1 is not read as qualifying K=3.
- **6.5** Surface `Qualified` / `StillBlocked` / `Uninterpretable` in the UI with
  the recorded reason. `Uninterpretable` must **not** be shown as success.

### Gate 6

### Phase 6 validation evidence

- `cargo check` passed outside the sandbox after the app-driven route, job, store, and provenance changes.
- Focused store and repair tests passed outside the sandbox, including depth preservation and requalification request coverage.
- `npm run validate-js`, `npm run lint`, and `git diff --check` passed outside the sandbox.
- **Fixed runtime is mandatory for this evidence:** `~/src/rapid-mlx/.venv/bin/rapid-mlx`, Rapid-MLX `0.12.17`, commit `8c02ed35e2f34b37e8e1365d9510a5bd5b180679` (our custom non-greedy sampling fix branch). The earlier unpinned `0.12.14` run is invalid and is not used for qualification or promotion.
- **Authoritative live screen:** `/private/tmp/rapid-mlx-phase6-fast-screen-fixed/requalification.json`, sampled/non-greedy route at temperature `0.6`, exact managed pair `nightmedia-27b-mxfp8-mlx` plus `qwen3.6-27b-nightmedia-f451-tess-8bit/mtp.safetensors`. The actual subject sidecar accepted `13/31` proposals; the known-good control accepted `39/78`. Verdict: `screened`. This proves live speculative activity for the exact sidecar on the production non-greedy route; it does not claim production promotion.
- **Qualification policy:** `screen` is the short gate; `full` runs sampled and constrained/tool-grammar production gates; `full-diagnostic` adds optional greedy parity diagnostics. Only sampled + constrained gates can promote a sidecar to `qualified`; greedy values are never a production requirement.
- **User-facing estimate:** the preset editor defaults to Fast screen (~2–5 minutes), with Full qualification (~40–60 minutes) and Full + greedy diagnostic (~60–90 minutes) available as explicit advanced choices. Each managed job reports elapsed time, estimate, and stage progress.
- **Full validation:** `cargo clippy -- -D warnings` passed; the full Rust suite passed with `1,255 passed; 0 failed; 14 ignored`; `cargo fmt -- --check`, `npm run validate-js`, `npm run lint`, `cargo build --release`, and `git diff --check` passed outside the sandbox.
- The survey fixtures are present in the standard read-only HF cache at `~/.cache/huggingface/hub`; they have not yet been migrated into Foundry's managed model root. The app-driven lane now discovers that external cache without moving or mutating it. A live multi-hour Rapid-MLX run remains separate evidence from fixture presence and is not claimed here.

- [x] A qualification run writes a well-formed entry to
      `spec-decode-verdicts.json` (schema v1).
- [x] The sampled gate appears in `gates_run` and produces a verdict.
- [x] Depth settings and qualification mode are recorded in the entry.
- [x] A trunk with a corrupted head yields `StillBlocked`, not `Qualified`.
- [x] `superseded_verdict` correctly invalidates an old verdict when the
      rapid-mlx version changes.
- [x] All three outcomes render distinctly in the UI.

---

## Acceptance criteria (whole effort)

Ship only when **all** hold:

1. **No trunk corruption.** Adopting any MTP-carrying MLX repo leaves the trunk
   free of MTP `model*.safetensors` files and free of an injected
   `mtp_num_hidden_layers` in `config.json`. Verified on at least three
   different publishers' repos.
2. **No silent bad heads.** Every registered sidecar has
   `norm_check_passed: true`. A head failing the `pre_fc_norm` sign check is
   rejected and the user is told why.
3. **Honest copy.** No UI string claims greedy-only, none claims tool calls are
   fixed (they are not), and none hardcodes a runtime version number.
4. **Capability gating unchanged.** The existing `--speculative-config`
   capability check still disables the controls with a stated reason when the
   flag is absent, and no second version-based gate was added.
5. **Guided default is 3** in all four locations, and Pro can reach both
   `num_speculative_tokens` and `disable_auto_k`.
6. **Presets round-trip.** Save → reload → spawn produces the identical
   `--speculative-config` JSON. Pre-existing presets are not mutated by the new
   default.
7. **Clean fallback.** Trunks with no sidecar, non-allowlisted `model_type`, or
   a runtime lacking `--speculative-config` all degrade to "speculation off"
   with a reason — never an error, never a silent wrong-output launch.
   Head-only repos are never offered as spawnable models.
8. **Measured, not assumed.** At least one end-to-end run on a real
   Qwen3.6-27B or Qwen3.8-27B trunk shows a non-zero acceptance rate under
   sampling at K≥2, recorded in the verdict store.

---

## Suggested execution order

Sequential, 0 → 6. Phase 0 is now a half-hour of environment setup rather than a
wait, so there is no reason to reorder around it.

Within that, note that 3 → 4 → 5 is the substantive work and the part that fixes
an active silent-corruption hazard; 1 → 2 are small. If the branch has to be
split for review size, split it there: `fix/rapid-mlx-nongreedy-mtp-copy`
(Phases 0–2) and `fix/rapid-mlx-mtp-sidecar` (Phases 3–6). Both are independent
and either can merge first.

If PR 2140 needs rework, Phases 3–6 are unaffected — correct sidecar handling
improves greedy speculation and stops trunk poisoning regardless of the sampling
fix. Only Phase 1's copy depends on 2140's behavior.

---

## Observed MTP filenames

Surveyed 2026-08-20 against the live Hub. This is real data, not a placeholder —
Phase 3 is written against it and should not be re-derived. Quantization
preference for anything we adopt as a test fixture is **mxfp4 first, 4bit
otherwise**.

**Four** distinct repo shapes exist in the wild, and they need different
handling. Every row below was verified by reading the actual safetensors header
or index — never by the repo name, which is unreliable in both directions (see
consequence 5).

| Repo | `model_type` | Shape | Weight files (bytes) |
|------|--------------|-------|----------------------|
| `mlx-community/Qwen3.8-27B-MTP-mxfp4` | head | head-only | `model.safetensors` 225,662,258 |
| `mlx-community/Qwen3.8-27B-MTP-4bit` | `qwen3_5_mtp` | head-only | `model.safetensors` 238,934,137 |
| `mlx-community/Qwen3.6-27B-MTP-4bit` | head | head-only | `model.safetensors` 238,934,137 |
| `mlx-community/Qwen3.6-35B-A3B-MTP-4bit` | `qwen3_5_mtp` | head-only | `model.safetensors` 475,130,833 |
| `mlx-community/Qwen3.5-9B-MTP-4bit` | head | head-only | `model.safetensors` 136,884,332 |
| `inferencerlabs/Qwen3.8-27B-MTP-MLX` | head | head-only | `model.safetensors` 238,934,137 (byte-identical to mlx-community's 4bit — a republish) |
| `Shiftedx/Qwen3.8-27B-MLX-MXFP4-MTP` | trunk | trunk + in-trunk sidecar | 18× `model-000NN-of-00018.safetensors` (14.3 GB); `mtp.safetensors` 849,400,403; **`model-vision-00001-of-00001.safetensors`** 921,497,189; `mtplx_runtime.json` — 16.1 GB total |
| `AutomatosX/AX-Qwen3.8-27B-MLX-AXQ-4bit-MTP` | `qwen3_5` | trunk + in-trunk sidecar | 4 shards (16.4 GB); `mtp.safetensors` 849,400,520; **`vision.safetensors`** 921,497,320; `axquant_*` manifests; `mtplx_runtime.json` — 18.2 GB total |
| `fcmeyer/Qwen3.8-27B-MLX-oQ4e-mtp` | `qwen3_5` | trunk, MTP fused | 4 shards, 17.0 GB, no MTP-named file |
| `dawncr0w/Qwen3.6-27B-uncensored-heretic-v2-Native-MTP-Preserved-oQ4-MLX` | `qwen3_5` | trunk, MTP fused | 4 shards, 17.0 GB, no MTP-named file |
| `rapid-mlx/Qwen3.8-27B-4bit-MTP-MLX` | `qwen3_5` | trunk + **`mtp/` subdir** | 3 shards; `mtp/model.safetensors` — 31 tensors, 2 `pre_fc_norm`, quantized. Trunk index has 0 MTP keys |
| `t0rr3sp3dr0/Qwen3.8-27B-MLX-MTP-mxfp4` | `qwen3_5` | trunk (MTP fused) **+ `mtp/` subdir** | 4× `model-0000N.safetensors`; index carries 22 MTP keys **and** `mtp/weights.safetensors` (22 tensors, 2 `pre_fc_norm`) |
| `vvsotnikov/Qwen3.6-27B-MTP-MLX-4bit` | `qwen3_5_mtp` | head-only | `model.safetensors`, 31 tensors, 2 `pre_fc_norm`, quantized |

No repo in the survey publishes `config.json` with a usable
`mtp_num_hidden_layers` on the head-only side — the heads carry
`model_type: qwen3_5_mtp` instead, and the fused trunks carry the layer count
inside the trunk config where `detect.py` already reads it.

### Consequences

1. **Filename shape is not diagnostic, in either direction.** The heads are
   literally named `model.safetensors`; the in-trunk sidecars are named
   `mtp.safetensors`, which does *not* match `mlx_lm`'s `model*.safetensors`
   glob. So the glob hazard is not triggered by any published repo, and no
   `*mtp*` exclusion would catch the heads. `model-mtp.safetensors` — the one
   shape that *is* a live corruption hazard — appears in **zero** published
   repos. It is *upstream's* extractor output name, written into the trunk;
   `build-mtp-head.py` relocates it out under `mtp.safetensors`
   (`UPSTREAM_OUTPUT_NAME` → `SIDECAR_NAME`, `build-mtp-head.py:60-61`) and
   refuses to run if one is already staged there. So the poisoning shape only
   exists on disk if someone ran the vendored extractor directly, which is why
   Phase 3's legacy scan still looks for it. Note the convergence: our relocated
   name matches what Shiftedx and AutomatosX already ship in-trunk, so 3.2a's
   relocation and the build path produce one layout, not two.

2. **Therefore: no `ignore_patterns`.** There is no pattern set that is both
   safe and useful. Excluding `mtp*` would delete exactly the sidecar we want
   from Shiftedx/AutomatosX; excluding `model*` would delete the trunk.
   Classify after download by tensor keys (`pre_fc_norm_*`), per task 3.2.

3. **Head-only repos are a first-class ingest path, not an edge case.** At
   137–475 MB they are far too small to be trunks, need no extraction, no bf16
   source, and no 27B-scale compute. This is what most users will actually
   download, and it motivates task 4.2a.

4. **Shiftedx ships `model-vision-00001-of-00001.safetensors`, which *does*
   match the trunk glob.** The `pre_fc_norm_*` check should skip it correctly,
   but that must be confirmed by test (Gate 3), not assumed — a
   misclassification silently strips the vision tower.

   AutomatosX ships the same vision tower, byte-for-byte the same size, named
   `vision.safetensors` — which does **not** match the glob. Two repos, one
   tensor payload, opposite filenames: this is the cleanest possible
   demonstration that naming carries no information here, and it is why both are
   Tier B fixtures in task 0.4.

5. **A repo can advertise MTP in its name *and* its config while shipping no
   MTP weights at all.** This is the sharpest failure mode found in the survey,
   and it defeats `detect.py`'s gate as currently written. All five
   `nightmedia/Qwen3.6-35B-A3B-MTP-Holo3-Qwopus-*` variants (`BF16`,
   `mxfp4-mlx`, `qx86-hi-mlx`, `qx64-hi-mlx`, `Coder-qx64y-hi-mlx`) publish
   `model_type: qwen3_5_moe` — **allowlisted** — and
   `mtp_num_hidden_layers: 1` — **> 0**. Both halves of the double gate
   (`detect.py:72-83`, `:197-209`) pass. Yet their indexes contain **zero**
   tensors matching `mtp` and **zero** matching `pre_fc_norm`, across 1,066
   (BF16) to 2,090 (quantized) keys. The head was stripped during conversion
   and the inherited config was never updated.

   The consequence for us is concrete: **config is not evidence.** A model that
   passes both gates can still have nothing to load, and the failure would
   surface as a runtime load error or a zero-acceptance draft rather than a
   clean "not eligible". Phase 3's classifier must require a positive
   `pre_fc_norm_*` tensor hit before declaring a trunk MTP-capable, and must
   not shortcut to the config. Gate 3 should include a nightmedia variant as
   the negative control for exactly this case — it is cheaper than the oMLX
   trunks and tests something none of the others do.

6. **MTP tensor keys come in two prefix conventions.** In-trunk and fused
   sidecars use a `mtp.`-prefixed namespace (`mtp.fc.weight`,
   `mtp.layers.0.…` — Shiftedx, AutomatosX, t0rr3sp3dr0, and the fused oMLX
   trunks). Head-only repos and `mtp/` subdirectory files use the **bare**
   namespace (`fc.weight`, `layers.0.…` — rapid-mlx's own repo, vvsotnikov,
   mlx-community). Any key matching in Phases 3–4 must accept both; a check
   anchored on a leading `mtp.` silently misses every head-only repo, which
   consequence 3 identifies as the most common user path. Matching on the
   `pre_fc_norm` substring (as `NORM_MARKER` already does,
   `build-mtp-head.py:63`) is prefix-agnostic and therefore correct as written
   — this note exists so nobody "tightens" it into a full-key comparison.

7. **The oMLX trunks are genuine, but they are not the only option.** Both
   fused oMLX repos verify clean — 29 MTP keys including 2 `pre_fc_norm`, in
   shard 4 of 4. If they nonetheless prove troublesome in practice, the
   non-oMLX substitutes below are verified equivalents and need no re-survey.

### Non-oMLX alternatives (verified 2026-08-20)

Each was confirmed by reading the actual tensor header, not the repo card:

- **`t0rr3sp3dr0/Qwen3.8-27B-MLX-MTP-mxfp4`** — the preferred substitute for
  either oMLX trunk. mxfp4 (matches our stated preference), `qwen3_5`,
  `mtp_num_hidden_layers: 1`, 22 MTP keys with 2 `pre_fc_norm` in the trunk
  index, plus a redundant `mtp/weights.safetensors` carrying the same 22
  tensors. Same author also publishes `-4bit`, `-8bit`, `-nvfp4`, `-mxfp8`, and
  `-bf16` of the identical build, so a quantization can be swapped without
  changing publisher.
- **`rapid-mlx/Qwen3.8-27B-4bit-MTP-MLX`** — the runtime project's *own*
  published model, and therefore the closest thing to a reference layout. Trunk
  shards carry no MTP keys; the head lives at `mtp/model.safetensors` (31
  tensors, 2 `pre_fc_norm`, quantized), with `mtp_num_hidden_layers: 1` nested
  under `text_config`. Worth treating as the layout to be most confident about
  supporting.
- **`vvsotnikov/Qwen3.6-27B-MTP-MLX-4bit`** — non-`mlx-community` head-only
  repo, `qwen3_5_mtp`, 31 tensors, 2 `pre_fc_norm`. Gives task 4.2a a second
  publisher so the head-only path isn't validated against one uploader's
  conventions. A `-bf16` sibling exists for the extraction path.

**Note the `mtp/` subdirectory shape**, which the original three-shape survey
missed entirely and which two of the three repos above use. Whether
`mtp/model.safetensors` is caught by `mlx_lm`'s `model*.safetensors` glob
depends on whether that glob is applied recursively — if it is, this layout is
a *new* instance of the double-shift hazard, hiding one directory down. Phase 3
must determine this by test rather than by reading the glob and reasoning about
it. The fact that rapid-mlx itself ships this layout makes the answer worth
knowing early.

### Recommended fixtures

- Head-only path: `mlx-community/Qwen3.8-27B-MTP-mxfp4` (mxfp4, 215 MB).
- Trunk + in-trunk sidecar: `Shiftedx/Qwen3.8-27B-MLX-MXFP4-MTP` (mxfp4, also
  the vision-shard case).
- Small end-to-end loop: `mlx-community/Qwen3.5-9B-MTP-4bit` — 4bit only, but
  it pairs with the Qwen3.5-9B trunk already used for the acceptance-rate
  measurements quoted earlier in this document.
## Private tooling implementation note

The MLX mirror now exists on private branch `fix/mlx-mtp-repair-tooling` at
commit `c4cfe32` as `converter/repair_mtp_mlx.py`, with focused tests and
operator documentation. It performs header-first inspection, accepts both
namespace conventions, assembles missing tensors from an explicitly chosen
source, validates both `pre_fc_norm` tensors and architecture fields, writes an
external sidecar, and records JSON provenance. It is intentionally a
local-source CLI for now; HF download/authentication remains the caller's
responsibility.

## Recipe-aware finetune repair requirement

For a finetune or merge, a matching base-model MTP head is only a fallback.
When recipe data is available, repair must reconstruct the missing MTP tensors
using the same parent model revisions, tensor weights, merge ordering, dtype,
and NuSLERP/mergekit semantics used to produce the trunk. This is especially
important for a dropped tensor such as `mtp.fc.weight`: copying one parent's
weight can leave the draft distribution mismatched to the merged trunk even
when every shape check passes.

The repair pipeline must therefore:

- discover recipe evidence from immutable local/HF metadata, model-card
  artifacts, or an explicitly supplied merge recipe;
- validate every referenced parent and revision before loading tensors;
- apply the recipe to the MTP namespace independently when the trunk merge
  omitted that namespace;
- record the recipe digest, method, parent revisions, coefficients, tensor
  lineage, and precision in `provenance.json`;
- label recipe reconstruction separately from direct-parent grafting in the UI;
- fall back to a direct compatible parent only with an explicit lower-confidence
  warning when no recipe is available.

Structural validation remains necessary but is not sufficient. Recipe-derived
heads must proceed through served requalification using the same sampling
settings as the target finetune, with acceptance rate and throughput recorded
before the app presents the sidecar as ready.
### Partial-parent qualification policy

Parent coverage is allowed to be incomplete for a user-requested candidate, but the app must never present a renormalized or guessed tree as an exact recipe reconstruction. A missing parent is recorded explicitly with its tree position and coefficient; only zero-weight branches may be omitted without changing the result. Direct-parent substitution remains a lower-confidence fallback.

The acceptance gate is evidence-based rather than an absolute promise: use a fixed, reproducible prompt set and sampling configuration, compare against the target's no-MTP and direct-parent controls, and record acceptance rate plus throughput. A 50% acceptance result may be usable when it beats the control and does not reduce throughput, but it is not sufficient by itself to mark a partial-parent candidate as ready. UI states remain `candidate`, `awaiting_requalification`, `qualified`, or `validation_failed`.

## 2026-08-20 implementation checkpoint

The first app-driven slice is now implemented on `fix/rapid-mlx-nongreedy-mtp`:

- `scripts/repair-mtp-mlx.py` is the public local-source repair contract. It
  performs header-first classification, supports direct-parent and nested
  NuSLERP reconstruction, validates `pre_fc_norm` signs, and writes schema-v2
  candidate provenance without editing the trunk.
- `src/inference/rapid_mlx/repair.rs` owns bounded execution, one-worker
  concurrency, cancellation, path validation, output limits, and managed
  sidecar placement.
- Authenticated API lifecycle endpoints now start, poll, cancel, and list
  repair jobs and sidecar provenance.

This slice intentionally does not infer HF parents, download recipe sources, or
mark a sidecar qualified. Requalification and HF/model-card discovery remain
the next phases; a successful repair is surfaced as a candidate requiring
served evidence.

### Phase 4 progress update

The Phase 4 implementation slice now includes:

- immutable BF16-source revision threading through `build-mtp-head.py` and the
  vendored extractor, with the Rust job manager invoking the guarded wrapper;
- direct-parent, recipe, head-only-adoption, and BF16-extraction job modes;
- schema-v2 top-level provenance fields for sidecar origin, digest, validation,
  memory estimate, quantization, and depth;
- relocation provenance marked explicitly as `candidate`/`pending`, with
  automatic sidecar selection refusing unvalidated entries;
- overwrite refusal for existing repair candidates and rollback if relocation
  provenance cannot be written.

Phase 4 implementation is complete. The cached `mlx-community/Qwen3.6-27B-MTP-4bit`
head-only fixture was adopted into an external `mtp.safetensors` and passed the
MLX norm validation; the cached Shiftedx `mtp.safetensors` relocation payload
also passed validation. The cached nightmedia config-lie fixture remains
`kind: none` despite `model_type: qwen3_5_moe` and
`mtp_num_hidden_layers: 1`. When a BF16 snapshot is incomplete, the supported
fallback is the explicit parent-assisted path: a compatible published head or
the recorded nested NuSLERP recipe. The sibling tooling's 7-test suite covers
that reconstruction, including lineage, dtype, and replacement validation. A
A genuine BF16 extraction positive control is now complete: the indexed fcmeyer BF16 source (revision `fe34c8d6784c6d9b463756dd020492123137b732`, MTP tensors in shard 11) and the bare-key Frosty40 source (revision `c4261c348aa8bdac2e2e2f352205def7436a625b3427e3752866c287`) both pass the vendored namespace-compatibility shim, whose updated extractor SHA-256 is `5debca6d49f33c4961237399f01c0e5ea110bc34dbce92ae6efa158396f421e3`, produce validated sidecar SHA-256 `a010fd0259712851b8dcf5567066c88831305597061586aaa1550af5b84734c0`, and report positive `pre_fc_norm` means `+1.5391` and `+1.8359`.
The sibling tooling's 7-test suite covers reconstruction, including lineage,
dtype, and replacement validation. The dedicated repair UI/requalification
flow remains in Phases 5–6.
### Phase 4 validation lifecycle

The app now exposes a read-only `validate --sidecar` command through the
authenticated repair lifecycle. It reads only MTP tensors, emits structured
digest and `pre_fc_norm_*` evidence, and atomically promotes pending provenance
to `candidate` only after the Rust boundary verifies the digest and positive
normalization means. The existing `repair_mode` is preserved.
