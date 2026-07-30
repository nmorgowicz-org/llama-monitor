# Rapid-MLX MTP — Working Handoff

**Purpose.** This is the context-relay doc for passing MTP speculative-decoding work
between frontier models across hourly quota boundaries. It is deliberately short enough to
read into a fresh context in full. It carries *state and open questions only*.

| | |
|---|---|
| Branch | `feat/rapid-mlx-integration` |
| Last updated | 2026-07-29 |
| Evidence record | `docs/reference/rapid-mlx-mtp-evidence.md` — every measurement, source citation, and artifact hash |
| Runtime comparison | `docs/reference/apple-silicon-mtp-runtime-comparison.md` — Rapid 0.11.1 vs MTPLX 2.3.0 vs llama.cpp |
| Product plan | Phase 6.5 in `docs/plans/20260718-final_rapidmlx_followups.md` — roadmap, gates, integration contract |

> Do not add measurements, source quotes, or roadmap material to this file. They belong in
> the evidence record and Phase 6.5 respectively. Keep this under ~200 lines.

## 1. Where things stand

**MTP speculative decoding works on Qwen3.6-27B only in an artificial greedy/no-processor
envelope. As shipped, it is not useful for normal coding-agent traffic.** An earlier version
of this document concluded the head itself was non-viable. That was wrong, caused by two
stacked tooling bugs, and every measurement in it is void.

The two bugs, both *around* the model rather than in Rapid's decode path:

1. **A stale `extract_mtp_weights.py` produced a dead draft head.** It missed the `+1.0`
   RMSNorm shift on `pre_fc_norm_embedding` / `pre_fc_norm_hidden`. Symptom: ~0%
   acceptance, no errors, backbone fine, tool calls fine — which is exactly why it read as
   a capability limit. **Fixed upstream.**
2. **Writing the sidecar into the model directory silently corrupts the trunk.** `mlx_lm`
   globs `model*.safetensors`; the presence of `model-mtp.safetensors` flips a heuristic
   that shifts every trunk norm by `+1.0`, double-shifting an already-converted MLX repo
   into gibberish. Upstream's own extractor writes there by default. **Not fixed upstream**,
   but **now refused app-side** — see the `reject_in_trunk_mtp_sidecar` entry in §7.2.

Served, with both fixed: **72–97% acceptance** and **+22% to +59% generation throughput**
across three trunk quantizations. The nightmedia MXFP8 model is **not** broken.

⚠️ **Every one of those numbers is an optimistic bound, not a product result.** Rapid-MLX
0.11.1's MTP scheduler is explicitly greedy-only; sampled requests fall through to plain
autoregressive decode. The published sampling configuration therefore cannot be qualified
without an upstream runtime change. See §4.

## 2. What is proven

- Both root causes, from source and from reproduction.
- Served end-to-end operation on three trunks (unsloth 8-bit, nightmedia `qx64-hi` mixed
  4/6-bit, nightmedia MXFP8) against an external sidecar.
- Sidecar quantization is **independent** of trunk quantization. There is no "MTP requires
  an 8-bit trunk" rule.
- The sidecar does **not** need to live in the trunk directory, and must not.
- Effective draft depth is **clamped to K=1** on Qwen3.5/3.6 by an SSM-cache rollback
  limitation. This is a correctness boundary, not tuning.
- A clean 4-trial ABBA repeat on the unsloth trunk held greedy-lossless parity in all 8
  off-vs-MTP comparisons; all four baselines produced one digest.
- Installed 0.11.1 and current upstream `main` are **greedy-only/no-logits-processors**.
  Either condition deliberately falls through; the sampled control recorded zero activity.
- `aliases.json` is unreliable capability metadata — wrong twice, including a checkable
  architectural fact (`is_hybrid: false` on a model that is 48/64 linear-attention layers).
- Benchmark harness is repaired and passes non-model validation.

## 3. What is not proven

The three-trunk matrix remains **n=1**. The unsloth greedy lane now has four trials, but is
still a forced research lane rather than a qualification result.

1. **The speedup inversion.** Highest acceptance produced the *smallest* gain: control
   (97.3%, +21.8%) vs MXFP8 (95.0%, +59.4%), at ~1.94 tokens/round in both. The control has
   the slowest baseline and cheapest sidecar and should have gained most. Unresolved, and
   the most likely thing to invalidate a product claim.
2. ~~**The K=1 clamp was never directly observed.**~~ **CLOSED.** Directly observed:
   `[MTP-chain-of-K] SSM cache detected in model_cache — clamping max_k from 2 to 1`, with
   `clamp_verdict: clamp_observed` and `effective_max_k: 1` in the receipt.
3. **The shipped sampling configuration cannot currently use MTP.** The recommended lane
   resolved to temperature 1.0/top-p 0.95/top-k 20/reasoning-on, then Rapid deliberately
   bypassed MTP. This is an upstream capability boundary, not missing benchmark data.
4. **The depth controller looks nearly inert.** Park rate is flat at 3–4% whether
   acceptance is 72% or 97%, and is slightly *higher* at high acceptance. An EV controller
   should behave the opposite way.
5. Repeat-trial variance across the qx64 and MXFP8 trunks.
6. Tool-call warm/repeat/extension behaviour under MTP.
7. 65k/131k tiers; completion lengths other than 512.
8. ~~Greedy-lossless token parity on unsloth.~~ **CLOSED:** 8/8 comparisons matched.
9. Whether a same-shape wrong-parent sidecar can be rejected before serving.

## 4. Load-bearing caveats

- **The current runtime does not implement sampled MTP.** Its scheduler contract says
  temperature >0 falls through because residual-distribution rejection sampling is not
  exercised by the MVP. Every acceptance/speedup figure here is therefore a greedy-only
  ceiling, not an estimate of shipped sampled behavior.
- **Normal coding-agent traffic hits both exclusions.** Sampling overrides trip the
  non-greedy gate, while Rapid's default-on constrained tool grammar installs a logits
  processor for ordinary tool-enabled requests. Forcing temperature 0 and setting
  `RAPID_MLX_CONSTRAIN_TOOLS=0` can make MTP engage, but changes sampling semantics and
  removes structural tool-call enforcement. That is a diagnostic bypass, not a product.
- **Reasoning mode was also diverging.** Spec-decode workloads hardcoded
  `enable_thinking: false`, while published guidance for both families is reasoning-**on**.
  This is a second axis and plausibly the larger one: reasoning traces are long and
  structurally repetitive, which is exactly the token stream a draft head does best on.
  Direction of bias is **not known** — unlike temperature, it could cut either way.
- **Recommended settings are not universally knowable.** Do not build anything that assumes
  a model publishes them. The unsloth trunk ships no `generation_config.json` at all. The
  harness resolves through operator flags → checkpoint → curated vendor profile, records
  which source answered, and treats "none" as a legal state that clears
  `performance_claim_eligible` rather than as an error.
- **Acceptance does not predict benefit.** The 32k-prompt cell measured 95.3% acceptance
  and +31.0% generation, but TTFT regressed 11.7%, leaving **+1.6% end-to-end** on a
  512-token completion. Never ship an acceptance-only indicator.
- **All percentages in the evidence record are speedup ratios** (`new/old - 1`), not
  fractions of time saved. "+59.4%" is 1.594× tok/s, i.e. 37% less generation time.
- **The benchmark suite currently passes `--force-spec-decode` unconditionally** whenever a
  speculative config is present, because the tested aliases advertise
  `supports_spec_decode=false`. That is a recorded profile-eligibility override, not the
  product launch contract. Forced and naturally-eligible lanes must be split before any
  qualification claim.
- **`family="unknown"`** appears in the MXFP8 receipt's metric labels (symlinked trunk dir)
  where other cells say `family="qwen3.6"`. Anything aggregating on that label will
  silently drop the series.

## 5. Artifacts you must not trust

| Path | Why |
|---|---|
| `/Users/nick/mlx-models/nightmedia-27b-mxfp8-mlx` | Corrupted on load — in-dir `model-mtp.safetensors` triggers the trunk double-shift. Not modified; left as found. |
| `/Users/nick/mlx-models/nightmedia-27b-mxfp8-mtp-fixed` | Correct head, but still in-dir. Same defect. |
| `/Users/nick/mlx-models/control-unsloth8bit-official-mtp` | Built from a dead head. Invalid. |
| `tmp/spec-decode-{receipts,warm-receipts,control-receipts}/` | Produced with a dead head, some also with a corrupted trunk. Measure nothing. |

The **only** validated sidecar is `scratchpad/nm-mtp-fixed.safetensors`
(SHA-256 `5088ad43…25717a`). `mlx-community/Qwen3.6-27B-MTP-4bit` is the known-good
positive control (239 MB standalone sidecar).

**Norm sanity check, one line:** a valid head has `pre_fc_norm_embedding` mean ≈ **+0.56**.
≈ **−0.44** means the stale extractor.

## 6. Harness state

Committed on `feat/rapid-mlx-integration`:

| Commit | What |
|---|---|
| `c54615f` | Accept-ratio per-phase delta, control/subject roles, sidecar hashes/revisions, tensor-derived quantization, shifted-RMSNorm preflight, in-trunk-sidecar rejection, paired TG/TTFT/end-to-end/park reporting |
| `8217f55` | Full server stdout/stderr persisted per cell; `runtime.backend_log` with `clamp_verdict` / `effective_max_k` |
| `e74d841` | `--trials N` counterbalanced ABBA ordering, `--settle-seconds`, `trial_protocol` in the suite index |
| `f4c2b5f` | `--spec-decode-lane forced\|natural`, required and defaultless; lane verified against the backend log, mismatch fails the run |
| `adeb8e1` | Sidecar resolves to the snapshot path, not the realpath — HF blobs have no extension and `mx.load` dispatches on it, so a realpath sidecar makes rapid-mlx refuse to boot. Plus positive-control-fails-the-run enforcement |
| `2bf103a` | Effective draft depth predicted from `config.json` `layer_types` before serving, then verified against the backend log; contradiction fails the run |
| `96100e3` | `--sampling greedy\|recommended\|explicit` + `--sampling-variant`, resolved through ordered sources with recorded provenance; reasoning axis; `min_p`/penalties carried into request and receipt |

`tmp/` holds the surviving `-v2` receipts and is untracked. The void receipt
directories are deleted.

⚠️ Two consequences for anyone re-running: spec-decode invocations now **require**
`--spec-decode-lane`, and the evidence record cites `tmp/…` paths that will not survive a
clean checkout.

### 6.1 Requalification lane

`scripts/rapid-mlx-requalify-spec-decode.mjs` (measure whether a build engages a head) and
`scripts/build-mtp-head.py` (build a validated out-of-trunk head from a BF16 source) exist and
are the one command each. **Procedure, gates, exit codes, artifact locations, and the
`--profile-alias` requirement: `docs/reference/rapid-mlx-mtp-evidence.md` §12.** Not restated
here — the copy in this doc went stale within a day.

## 7. Next action

Phase 6.5a is at a **stop/escalate gate**. Separately, the capability wiring that had to land
whether or not upstream ever fixes this is **done** — see §7.1. That work is *not* the plan's
Phase 6.5b, which is managed sidecar artifacts (Builder items 12–15) and remains parked behind
6.5a.

The clean unsloth greedy repeat completed: 12/12 receipts, positive control passed, every
speculative cell observed the K=2→K=1 clamp, deterministic baselines, greedy parity 8/8. The
immediately following recommended lane resolved correctly to the vendor profile and then failed
its first control — the installed scheduler intentionally falls through for non-greedy
sampling, so the request recorded zero speculative activity. Coding temperature 0.6 would hit
the same guard. Per-trial numbers and the source reading are in
`docs/reference/rapid-mlx-mtp-evidence.md`; do not restate them here.

**Disposition:** suspend MTP product qualification and preserve the harness as an upstream
requalification lane. Do not run the remaining qx64/MXFP8, natural-lane, 65k/131k,
non-512, or tool-call MTP matrix now; it would qualify a workload real coding agents do not
run. Resume after a pinned upstream build produces nonzero speculative activity for both
nonzero-temperature and normal constrained-tool requests with parity/fidelity. Higher-context
runs remain planned after those cheap gates pass.

Continue the broader Rapid integration roadmap with MTP omitted. Do **not** start MTP-specific
artifact management, estimator, API, or UI work until 6.5a passes; unrelated Phase 7 and later
loader/cache/runtime work may proceed and must not advertise or auto-enable MTP. See the
cross-runtime audit in `docs/reference/apple-silicon-mtp-runtime-comparison.md`.

### 7.1 Capability wiring — upstream-independent (landed 2026-07-29)

Upstream commits daily, so MTP and vision will plausibly be fixed on someone else's schedule.
The app-side work a future fix would need is done, which makes that fix a re-measurement
rather than a re-design. **None of it enables MTP**, and it is *not* the plan's sub-phase
6.5b (that name is taken by managed sidecar artifacts). Full description, and the mapping onto
Builder items 9 and 11, in the Phase 6.5 section of `20260718-final_rapidmlx_followups.md`.

State, for a reader picking this up:

- `spec_decode` is a tri-state `FeatureQualification` in the Rapid `CapabilitySnapshot`
  (`capabilities.rs`), reaching `CapabilitySet.mtp`. Flag presence yields at most
  `Indeterminate`; `SPEC_DECODE_GREEDY_ONLY_VERSIONS` records `0.11.1` as `Unavailable`.
- `MtpAdmissionResult` models the decode shape and reports `engages_for_workload` +
  `fallthroughs` (`workload_scenarios.rs`). It no longer recommends MTP for `CodingAgent`.
  Reasoning is recorded but does **not** gate: it changes acceptance, not eligibility.
- The post-upgrade probe names the three outstanding gates and the requalification command.
- HF browse vision discovery reads `config.json` and the safetensors index weight map before
  the old filename/tag heuristics, and labels which source answered (`qualify.rs`).

Verified: `cargo clippy --lib` clean, `cargo fmt --check` clean, `cargo test --lib` 950 passed.

⚠️ Correction to the earlier entry here, which claimed `cargo clippy --all-targets` clean:
it is **not**. `--all-targets` reports 21 warnings on committed code (`clippy` 1.95.0) —
`memory_availability.rs`, `llama/vram_estimator/tests.rs`, `llama/model_memory_profile.rs`,
`inference/rapid_mlx/mod.rs`, `mlx_meta.rs`, `compatibility.rs`. Almost all are test-only
hygiene (`field_reassign_with_default` ×9, `unnecessary_cast` ×4, two unused variables). One
was in this workstream's own code (`hf/qualify.rs` `manual_find`) and is fixed. The rest are
pre-existing and deliberately left: they are unrelated files, and a drive-by sweep across them
is not this lane's scope. Do not restate "all-targets clean" without re-running it.

**The lane has now actually been executed** against a live server on 0.11.1 (`--gate sampled`,
MXFP8 trunk, official 4-bit control). Running it was worth it independently of the verdict: a
lane that is only syntax- and lint-clean had four real defects that no static check could see.

| Defect | Symptom | Fix |
|---|---|---|
| `rapid-mlx info` prints `(none)` for unresolved parsers on local dirs | `serve` died before health on *every* spec-decode cell: `argument --reasoning-parser: invalid choice: '(none)'` | `INFO_ABSENT_VALUES` sentinel filter in the suite, and the same class in `info_query.rs` (`info_value_or_absent`) — the Rust side stored the placeholder as a parser name too |
| Parsers unresolvable for a local path, silently | `constrained` gate would have run with no tool grammar and reported a **false pass** | `--profile-alias` + ordered resolution (flags → `info <model>` → `info <alias>`) with recorded `parserSource`; `constrained` now hard-refuses without a parser |
| `sampled` used the `recommended` lane: temperature 1.0, reasoning ON, `max_tokens: 512` | Model spent the whole budget inside `<think>`, content empty, inner benchmark failed its completion floor → gate `uninterpretable`, not a real answer | Gate pinned to `--sampling explicit --temperature 0.6` (reasoning off) |
| Generation length too short for the question being asked | 512 tokens cannot produce a stable accept ratio | `DEFAULT_SPEC_COMPLETION_TOKENS = 8192` cap with a 4096 `minimum_completion_tokens` floor, plus `--spec-completion-tokens` |

The exit-code design also earned its keep: the broken run returned `1` / `uninterpretable`
("harness broken"), not `20` / `still-blocked` ("upstream still limited"). Those two must never
be confusable, and they weren't.

Verdict and per-cell numbers: `docs/reference/rapid-mlx-mtp-evidence.md` §12. Not restated here.

### 7.2 What is still open after the wiring

- **Closed (2026-07-29): the in-trunk sidecar layout is refused at launch.**
  `reject_in_trunk_mtp_sidecar` in `src/inference/rapid_mlx/model_resolver.rs` reads the
  safetensors *header* of every file matching mlx_lm's own `model*.safetensors` glob and bails
  when a file's tensor keys are entirely `mtp.*`, with the remediation in the message. It is
  wired into both the `MlxDirectory` launch path and `validate_model_directory_assets`, so it
  covers models the app never created — the layout arrives by following upstream's documented
  extractor usage. The discriminator is deliberately "every tensor is `mtp.*`", not the file
  name: a shard mixing MTP with trunk tensors is a legitimately embedded-MTP checkpoint and
  must still launch. Five tests, including that case and the outside-the-glob case.

- Run the requalification lane against the next upstream bump. If it exits `20`, nothing in
  the app changes. If it exits `0`, remove the version from
  `SPEC_DECODE_GREEDY_ONLY_VERSIONS`, record the evidence, and only then resume 6.5a.
- `derive_mtp_concurrency` **now returns `SingleActiveGreedy`** for a version in
  `SPEC_DECODE_GREEDY_ONLY_VERSIONS` that exposes `--speculative`, and `Unknown` otherwise.
  Concurrency is a scheduler property, not a model property, and on those builds the scheduler
  states its own constraints in the backend log at install time
  (`single-request greedy K=1 chain-of-1; falls through on B>1 / non-greedy /
  logits-processors` — verbatim `SingleActiveGreedy`). That makes the version table, not the
  flag list, the evidence. No log-scraping was added to the install-time probe.
  - Still open: an *unrecognised* version stays `Unknown`, and there is still no path for a
    measured lane verdict to reach a `CapabilitySnapshot` (which is derived from
    `serve --help`). That remains a design change — do not bolt log-scraping onto the probe.
  - Also still open: nothing consumes `mtp_concurrency` except the runtime API's JSON
    (`src/web/api/rapid_mlx_runtime.rs:805`). It is reported, not acted on.
- `command.rs:565` still gates on `--speculative` flag existence rather than the behavioral
  verdict. Left deliberately: a user may want to launch with MTP to test it. Admission
  warnings carry the message instead.
- `RAPID_MLX_CONSTRAIN_TOOLS` appears nowhere in `src/`. The app has no representation of the
  tool-grammar axis as a runtime setting, only as a modelled request property, so it cannot
  currently *choose* to drop the grammar in exchange for speculation.
- `EstimatorOptions` **now carries `workload_scenario: Option<WorkloadScenario>`**, and
  `/api/vram` passes the scenario the request already stated (it was parsed and then dropped).
  `None` still falls back to `WorkloadScenario::default()` (`CodingAgent`), so an unstated
  scenario behaves as before; the difference is that a stated one is no longer ignored. Covered
  by `mtp_admission_follows_the_callers_workload_scenario`.
- The vision tensor check is wired into `hf_qualify_repo` only. Local-disk vision detection
  (`extract_vision_component` in `mlx_meta.rs`) remains a separate path.

---

## 8. Handoff protocol

When picking this up in a fresh context:

1. Read this file, then §11 of the evidence record (known-unknowns).
2. Read the full evidence record only if you need to *re-derive* a claim; it is long.
3. Read Phase 6.5 only when doing product work rather than measurement.
4. Update **this** file with state changes; update the evidence record with new
   measurements; update Phase 6.5 with scope decisions. Do not let them merge again — the
   previous single document reached 1113 lines and buried the 20% that was load-bearing.
