# Phase 7 / Phase 8 gap register

**Written:** 2026-07-30 by Coordinator, after the full UNVERIFIED reconciliation sweep
(7A1, 7A2, 7A3, 7B1, 7B2, 7B3, 7B4, 7.5A, 7.5B, 7.5C, 8A, 8B1, 8B2).

**What this is.** The sweep's job was to decide whether each row's claim was true. Six defects
were found and fixed inline because they were live bugs in a shipping path. Everything below is
different: it is work that was *never wired*, *silently deleted*, or *wired to nothing*. None of
it is a regression, so none of it was fixed during the sweep — by agreement, the sweep finished
first and the fixes land in one pass afterward.

**Why the test suite never caught any of it.** 1041 library tests, clippy, eslint and 40 Playwright
tests are all green, and were green while every item below was true. The recurring shape is a
correct backend and a frontend that never calls it. A unit test that supplies its own input proves
the rule and proves nothing about the wiring — `d25_multi_slot_conflicts_with_mtp` passed for weeks
while no caller ever supplied a slot count of 2.

---

## 1. Backends that are correct and consumed by nothing

Each of these computes real, correct output against a running binary. Nothing under `static/` reads it.

| Surface | Where | Evidence |
| --- | --- | --- |
| `/api/rapid-mlx/settings` (the settings catalog) | `src/inference/rapid_mlx/settings.rs` | No `fetch` to it anywhere in `static/js/` |
| `/api/rapid-mlx/command-preview` | `src/web/api/rapid_mlx_runtime.rs` | No frontend caller |
| `requested_vs_effective` | `/api/vram-estimate` response | Never read; the Prompt-storage selector is a visible no-op while the backend honestly reports `k8v4 → none` |
| `effective_policy` | `/api/vram-estimate` response | Never read |
| `mtp_admission` | `/api/vram-estimate` response | Live output for embedded MTP is substantive — `fallthroughs: [non_greedy_sampling, logits_processor_installed]`, `warnings: [mtp_eligible_but_not_recommended]` — and no UI shows it |
| `/api/hf/qualify` | `src/web/api/hf.rs:879` | Registered, revision-pinned, no frontend caller |
| `/api/hf/identity` | `src/web/api/hf.rs:920` | Registered, no frontend caller |
| `/api/models/mlx-introspect` | `src/web/api/models.rs:1393` | Registered, no caller of any kind |
| `CommunitySourceCatalog` | `src/models/community_source_catalog.rs` | **No route at all.** `save_catalog` / `reset_catalog` / the upsert helpers are called from nothing under `src/web/`, so "user-editable" is unrealized and the catalog is always the bundled default |

**The command preview is the one the user has explicitly asked for:** "we definitely need the
command-preview to see how the model was launched." The endpoint exists and was fixed during 7A2
(it had a second config→argv mapping that dropped 12 flags and invented 3). It needs a consumer.

## 2. `RapidMlxConfig` fields with no UI

21 of 57 fields have no mention in `static/js/` or `static/index.html` under either snake_case or
camelCase:

`executable_path`, `managed_runtime_path`, `trust_remote_code_consent`, `auto_tool_choice`,
`no_thinking`, `hybrid_cache_entries`, `pflash_policy`, `response_cache_policy`,
`disk_checkpoint_policy`, `max_num_seqs`, `max_concurrent_requests`, `prefill_batch_size`,
`completion_batch_size`, `batching_policy`, `speculative_policy`, `gpu_memory_utilization`,
`endpoint_compatibility`, `request_safety_policy`, `default_frequency_penalty`, `parser_policy`,
`security_policy`.

Two of those (`executable_path`, `managed_runtime_path`) are runtime-resolution internals and
arguably should stay hidden. The rest are real launch parameters. Per standing direction, advanced
is not a reason to omit — anything called out in our docs should be exposed. The highest-priority
ones by that test:

- **`pflash_policy`** — 7B1 found it never reached any deserialized config, so llama-monitor was
  shipping rapid-mlx's `always` default on Qwen3.5/3.6 against a measured 0–40% recall collapse.
  Fixed in the backend; still has no control.
- **`speculative_policy`** — the whole spec-decode lane is documented and measured, and unreachable
  from the UI.
- **`trust_remote_code_consent`** — a consent flag with no way to consent.
- **`gpu_memory_utilization`** — commit `712c261` claims a `--gpu-memory-utilization` control was
  added. It was not; the claim is false.
- **`max_num_seqs` / `max_concurrent_requests` / `batching_policy`** — the concurrency story that
  `mtp_admission` reasons about.

## 3. Teaching and workload content that was deleted, not shipped

Commits `712c261` and `58cfa42` removed the step-3 workload picker and everything rendered inside
it. The ledger rows for that work said "verified" and described a UI that no longer existed.

- **7B3 roleplay teaching panel** — gone with `WORKLOAD_PROFILES`, along with its three tests. No
  `roleplay-teaching` identifier survives in `static/` or `tests/ui/`.
- **7B4 MTP/concurrency teaching** — four D25 cards and five tests, gone. The orphan
  `#pe-mtp-concurrency-teaching` container is still in `static/index.html` (~line 3246) with nothing
  to fill it.
- **7.5C tool-research captures** — `spawn-wizard-engines` was supposed to gain tool-research and
  deterministic profile captures. No `tool_research` identifier exists in `capture.mjs`.

**This content is release-gating**, not optional: teaching + troubleshooting is one of the release
pillars, and it currently has no home. The deletion was correct (the panels were unreachable); the
replacement was never built.

## 4. Discovery and lineage defects (Phase 8)

- **Sort options lie.** `resolveSortParam` (`hf-browse.js:294`) maps Name→`createdAt` and
  Size→`downloads`. Five visible options collapse to two behaviours: picking "Name" sorts by
  creation date, picking "Size" sorts by download count. `SimpleModelInfo` carries `last_modified`
  and `model_size_bytes`, so both could be sorted correctly client-side. There is also a reference
  to `HF_SORT.AUTO`, which is not defined in the enum.
- **`workload_profile` is sent and discarded.** `hf-browse.js:570` puts it in the search body;
  `src/web/api/hf.rs` never reads it. Worse, `sessionState.workloadProfile` is read in four places
  and **written in none**, so it is permanently null — which also pins the Models-modal VRAM
  estimate to the hardcoded `interactive_coding_agent` (`models.js:2863`).
- **Role badges bypass the backend catalog.** The badges come from `KNOWN_CONVERTER_PATTERNS`, a
  hardcoded JS regex list with three roles, not the seven-role user-editable `CommunitySourceCatalog`.
  It classifies `Qwen/` as a converter, which is wrong — Qwen is the original author.
- **Lineage on library cards is dead code.** `models.js:518-568` gates on
  `hf_repo_id || originRepo || repo_id`. Live-verified across all 68 real inventory entries: none of
  `hf_repo_id`, `originRepo`, `repo_id`, `hf_revision`, `original_author`, `converter`,
  `hf_source_info` or `provenance` is present, because `ModelInventoryEntry` has no such fields, and
  zero `.llama-monitor-source.json` / `llama-monitor-conversion.json` sidecars exist on disk. The
  `if` is never true.
- **Revision preservation is vacuous.** `SimpleModelInfo` has no `revision`/`sha` field, so the
  selection payload's `revision` (`hf-browse.js:240`) is always null. "Revision-bound qualification
  badges" cannot be revision-bound, and the revision-pinned `/api/hf/qualify` is never called.
- **`HF_SORT_LABELS` and `HF_SCOPE_LABELS` are exported and unused** — the scope buttons hardcode
  their own labels and only borrow the tooltips. There is no `HF_SCOPE.AUTO`, so the documented
  four-way Auto/GGUF/MLX/All scope set is three.
- **Legacy quantizer surface is still the live one.** The UI reads `/api/hf/quantizers` and
  `/api/hf/community-picks`; neither `community-source-catalog.json` nor the legacy
  `hf-quantizers.json` exists in the real config dir, so the KnownQuantizer→role migration has never
  run on this machine.

## 5. Smaller recorded items

- `/api/vram-estimate` silently ignores an unknown `workload_scenario` rather than rejecting it.
- No use-case card maps to a multi-slot scenario, so the D25 multi-slot MTP warning — now reachable
  after the 7B4 fix — still cannot be triggered from the UI.
- `prefill_step_size` has a clamp that keeps the documented 4096 value unreachable (deliberate,
  recorded in 7A2).
- `ValidationContext` populates `capabilities` and `workload_scenario` that every rule ignores.
  Deliberate and documented, guarded by a drift test — recorded so it is not "discovered" again.
- `prefix_cache_policy` is catalog-only; the live system uses three raw config fields instead.
- Playwright tag coverage is partial and inert: `@in-memory-test` appears in 2 of 15 spec files,
  `@fake-data-bypass` in 3, and `playwright.config.js` defines no tag-filtered project, so the tags
  select nothing.
- The `capture.mjs` mock-vs-real scenario table is now stale for the removed workload scenarios.
- `rapid-mlx-live` (7.5B) is coherent but has not been run in this campaign; its three screenshots
  were not re-approved.

## 6. Carried open items from earlier phases

- The `--cache-ram` multi-branch / slot-pressure run (Phase 6 item 9) is still outstanding.
- The Library tab UI changes from the Disk/cache-audit work were never visually verified.
- Phase 8B3's stated dependency is "8B2 verified + screenshots approved". 8B2 is now verified; the
  screenshots are not approved. 8B3's headline deliverable — additive MLX+GGUF scope toggles — is
  already implemented (`models.js:2264`), so that part of its scope is already done.

## 7. Decisions already closed (do not reopen)

- **`workload_scenario` is not persisted in a preset.** It is spawn-wizard entry guidance only. For
  llama.cpp it should do one thing — pick q8_0 vs q4_0 KV against GGUF size for agentic vs roleplay.
  There is not enough divergence beyond that to justify storing it.
- **`CONSTRAIN_TOOLS` is not a product setting.** Dropping tool grammar to buy speculation speed is
  the wrong trade.


## 8. Resolved — command preview (2026-07-30)

`/api/rapid-mlx/command-preview` had no consumer, and could not have had one. It
returned `BAD_REQUEST` unless the caller supplied `executable_path`, and no frontend
surface can learn that path — `/api/llama-binary/platform-info` exposes only the boolean
`rapid_mlx_local_available`. Two further divergences from the real launch path sat in the
same handler: `models_dir` was the relative literal `"models"` resolved against the
server's CWD, and `runtime_version` was passed as an empty string.

The one spec covering the endpoint, `phase7-command-preview.spec.js`, is `@runtime-required`
and skipped unless `LLAMA_MONITOR_HAS_RUNTIME=1`, so it had never run. Its payloads also
carry no `executable_path`, so it would have failed had it run. This is the defect class in
its purest form: the test that would have caught it was the one that never executed.

Fixed in `12a9739` (backend falls back to `Discovery::resolve_binary`, the launcher's own
explicit → managed → PATH precedence; real `models_dir`; qualified runtime version) and
`ef64903` (step-6 config card renders the argv, plus `requested_vs_effective` and `reasons`).
`command-preview-ui.spec.js` covers it, negative-control verified.

### Retracted — two findings logged here on 2026-07-30 were wrong

Both were recorded in the commit message for `ef64903` and are corrected here.

1. **"Capability-gated flags are dropped without a reason."** False. A
   `turboquant_mode: "k8v4"` request does omit `--kv-cache-turboquant` from argv, but that
   omission is deliberate (`build_effective_policy`, and the block at
   `rapid_mlx_runtime.rs:846-866`): TurboQuant needs an exact model/revision qualification
   receipt, and until that evidence is wired the value is persisted and shown but not
   launched. `requested_vs_effective` reports it in full, with
   `effective: "none"` and the reason "No exact model/revision TurboQuant qualification
   receipt is available; disabled". Verified live 2026-07-30.

   The original observation came from a payload that also carried a `speculative_policy` the
   installed runtime does not support. That hard-errors before the diff is ever built, so the
   response contained no diff at all — and the empty diff was misattributed to turboquant.
   The lesson is the register's own: a probe that changes two things at once proves nothing
   about either.

   **But the probe did expose a real defect, one layer over.** See below — the retraction
   was of the diagnosis, not of the symptom.

2. **"Two pre-existing load-sensitive test races."** Not a defect. `playwright.config` sets
   `fullyParallel: false`, `workers: 1` under CI and `retries: 2` under CI. The failures were
   produced by running locally, where `workers` is undefined and therefore parallel and
   `retries` is 0 — concurrency the suite is explicitly configured never to have. Under the
   real settings the same five specs pass 40/40, twice over.



## 9. Resolved — speculative decoding suppressed every other setting's diagnostics (2026-07-31)

Found while retracting the bad turboquant finding above. TurboQuant and speculative decoding
are unrelated features that were handled at different layers with incompatible failure
semantics:

- TurboQuant lives in the response layer. `build_effective_policy` forces it to `Off`,
  `build_requested_vs_effective` reports the downgrade with a reason, and it never reaches a
  capability check that can fail.
- Speculative called `capabilities.require("--speculative")?`, aborting the whole command
  build.

`--speculative` is absent from `ServeCapabilities::verified_baseline()`, so on a baseline
runtime one `speculative_policy` blanked the entire preview — no argv, no `effective_policy`,
and no `requested_vs_effective` for any unrelated setting. One feature's unavailability
suppressed every other feature's diagnostics, which is also what made the original
misdiagnosis possible.

Speculative is a throughput feature like TurboQuant and PFlash and is now treated like them:
omitted from argv when the flag is absent, reported as a downgrade instead. PFlash already
set that precedent at `command.rs:487`. `build_effective_policy` now takes capabilities so it
reports `speculative_policy: "off"` rather than echoing the request back and disagreeing with
the argv beside it.

Fixed in `7948ff3`. Regression test `unsupported_speculative_is_omitted_without_failing_the_build`,
negative-control checked. Verified live: a payload setting both features returns argv plus two
independent diff entries, with an unrelated `max_num_seqs` still reaching argv.

## 10. Resolved — nine Rapid-MLX preset-editor controls were unreachable (2026-07-31)

`presets.js` reads sixteen `modal-rapid-*` controls when saving a Rapid-MLX preset. Nine of
them rendered nowhere, so the save path wrote their hardcoded defaults over whatever the
spawn wizard had set. The controls existed, the save path worked, and unit tests were green;
nobody had asked whether a user could reach them.

Three separate causes, found by measuring computed style up the ancestor chain rather than
by reading selectors:

- `pe-row-rapid-parser-overrides` (tool_call_parser, reasoning_parser) and
  `pe-row-rapid-architecture-overrides` (hybrid_mode, prefill_step_size) were nested inside
  `#spawn-wizard-overlay`, not the preset modal, while using `pe-*` classes and
  `modal-rapid-*` ids. Invisible in both surfaces.
- `#pe-row-rapid-webui` / `-webui-expert` had no Rapid-mode display override, so the group
  was hidden for every backend. The comment calling these llama.cpp-only was wrong —
  `web_ui_availability`, `web_ui_static_path` and `web_ui_config_json` are `RapidMlxConfig`
  fields the Rapid command builder emits.
- `#pe-row-rapid-reasoning-mode` and `#pe-row-rapid-cache-memory` were missing from the
  deny-by-default allowlist in `modal-premium.css:2343`.

That allowlist is the structural cause worth remembering: it hides Rapid controls unless
both the row **and** the field are exempted by id, so every control added to the modal is
invisible until someone edits two `:not()` chains. `rapid-preset-visibility.spec.js` now
derives its list by scanning `presets.js` for `modal-rapid-*`, so a control added later is
covered without anyone updating the test.

Whole-suite negative control: against the unfixed tree the new spec is the only failure in
236 tests. Fixed in `b7f7437`.

### Carried — the full UI suite fails 2-3 tests per run, at random

Four full 241-test serial runs on 2026-07-31 each failed 2-3 tests, never the same ones:
`tls-certificates`, `settings-modal`, `phase7-presets`, `preset-flow`. None reproduce solo
or in combination. It reproduces on the **unmodified tree**, so it is not attributable to
any Phase 7/8 change, and CI's `retries: 2` almost certainly masks it. The shared long-lived
test server accumulating state across ~240 tests is the leading suspect. Unresolved; it
means the suite cannot currently certify a change by pass count alone — attribution requires
re-running the stashed tree.

## 11. Twelve Rapid-MLX config fields were gated on flags that do not exist

Found while answering "what exactly are the missing fields?". The manual audit found five;
a guard test found twelve. All were removed.

### How they were found

`ServeCapabilities::from_help` derives capabilities by parsing `rapid-mlx serve --help`, so a
flag absent from that help can never be detected. That makes the defect mechanically
checkable: capture the runtime's real flag inventory and compare it against every `--flag`
literal in the source. `settings.rs::serve_flag_literals_exist_in_the_real_runtime` does
exactly that against `testdata/serve-flags.txt` (85 flags, rapid-mlx 0.11.1). It scans only
probe and emission sites — `has_flag(`, `.require(`, `.contains(`, `args.push(` — so
`assert!(!args…)` negative assertions do not trip it.

### Why the existing tests did not catch it

`command.rs`'s Phase 7 argv test built its capability set from a hand-written help string that
**declared all twelve invented flags**. The assertions then confirmed the builder's mistakes.
Green tests were not merely silent here; they were actively wrong. The synthetic string is now
constrained to real flags by the same guard.

### Severity

Every one of the twelve called `capabilities.require()`, which returns `Err` and aborts the
*entire* command build. Setting any of them blanked the command preview and failed the launch —
the same defect fixed for the throughput flags in `ac31388`, but twelve times over. Four of them
(`endpoint_compatibility`, `request_safety_policy`, `parser_policy`, `security_policy`) were the
worst combination: `settings.rs` reported them `=> true`, *always available*, while the builder
required a nonexistent flag. The UI advertised support, then the launch hard-failed.

### Two distinct origins, established by comparing against both runtimes

**Group A — llama.cpp cross-contamination (3 fields).** `web_ui_availability`,
`web_ui_static_path`, `web_ui_config_json` probed `--ui`/`--no-ui`/`--path`/`--ui-config`/
`--ui-config-file`. All five are real `llama-server` flags, verbatim, including the `--ui`/
`--no-ui` pairing. They were copied into the wrong builder. Note the inversion: `ServerConfig`
(llama.cpp) has **no** web-UI fields at all, so llama-monitor could not configure the web UI of
the runtime that has one, while offering to configure the web UI of the runtime that does not.

`abc6419` earlier the same day added CSS making this group visible for
`.preset-editor--rapid-mlx` — the one backend where it can never work. Reverted here.

**Group B — invented, present in neither runtime (9 fields).**

- *Near-misses of real rapid-mlx capabilities:* `response_cache_policy` → the real flag is
  `--response-cache-entries`; `disk_checkpoint_policy` → `--kv-disk-checkpoint-interval`;
  `speculative_policy` → `--speculative-config`. All three real flags take an integer or a JSON
  object, not a policy word, so these are shape mismatches and not renames. `disk_checkpoint_policy`
  was also redundant: the real `disk_checkpoint_interval: u32` already sat beside it.
- *No equivalent anywhere:* `batching_policy`, `concurrency_policy`, `endpoint_compatibility`,
  `request_safety_policy`, `parser_policy`, `security_policy`. Five of six end in `-policy` — a
  designed vocabulary rather than an observed one.

### Collateral

- `ExclusionMatch::Present` became dead code: its only user was the
  `speculative_policy`/`max_num_seqs` exclusion rule. Removed rather than kept warm.
- `build_effective_policy` no longer needs its `capabilities` parameter.
- `vram-estimate.js` — `concurrency_policy` is a **real** estimator input
  (`workload_scenarios.rs:525`, read at `vram.rs:388`), not a CLI flag. But both its JS sources
  are now dead: `rapidMlx.concurrency_policy` was the deleted field, and
  `hardware.concurrencyPolicy` is written by no surface and was already dead. The estimator now
  always takes its default. Tracked separately; not silently repaired.

### Left unexposed on purpose

`--response-cache-entries` (greedy-only response cache), `--speculative-config`, and llama.cpp's
real web-UI flags are all genuine capabilities with no UI after this change. Exposing them is a
separate decision, not part of removing the phantoms.

## 12. The three real fields, exposed (2026-07-31)

The other half of the same audit. `hybrid_cache_entries`, `prefill_batch_size` and
`completion_batch_size` map to flags that genuinely exist in `rapid-mlx serve --help`, were
plumbed end to end (`RapidMlxConfig` → adapter → `command.rs`) and validated in `settings.rs`
— and had no control on any surface. The only value anyone could launch with was the default.

Now in both the preset editor and the spawn wizard, with both CSS allowlist chains updated.

### Where they were put, and why

- **Retained prefix entries** sits with the retained-cache toggle and memory ceiling, because
  it is meaningless without them. It is an entry *count*, not a size: raising it keeps more
  branches alive but splits the same memory budget, and the hint says so.
- **Prefill / completion batch size** are their own row, defaulted to Auto and labelled as
  counting *sequences, not tokens*. At the single-stream settings this app defaults to
  (`--max-num-seqs 1`, `--max-concurrent-requests 1`) they do nothing on the text path, and on
  the vision path a prefill batch that never fills can stall generation outright
  (`mlx_vlm/generate/ar.py:2695`). They are exposed because they are real and measurable, not
  because they are recommended.

### A second defect found while wiring them

`_buildFormPreset` spreads `out` over the stored `rapid_mlx` object, so a key the save path
omits keeps its previous value. The four throughput fields shipped with the
`if (value) out.x = value` idiom, which meant **selecting Auto on a preset that already had a
value silently did nothing** — the old number survived the spread and kept reaching argv. For
`hybrid_cache_entries` this was worse than cosmetic: `command.rs:393` emits
`--hybrid-cache-entries` on the field alone without consulting the retained-cache toggle, so a
stale entry count would be passed to a server whose prefix cache the user had just turned off.

All seven now write `null` on Auto rather than omitting the key. Serde reads null into the same
`None` the missing key produced, and the load path fills every one of these controls, so
re-reading them preserves untouched values without relying on the spread.

`core/rapid-preset-throughput.spec.js` gained a test for exactly this — set a stored value back
to Auto, assert it clears — since the old spec asserted the spread behaviour approvingly and
would have kept passing.

### Coverage

`core/rapid-preset-cache-batch.spec.js` (new, 4 tests): load, edit round-trip, the
cache-off gate, and Auto. `core/rapid-preset-visibility.spec.js` derives its id list from
presets.js and so covers the new controls without being edited — which is what it was for.

### Collateral

`_configureBackendPresetEditor` still listed `pe-row-rapid-webui` and
`pe-row-rapid-webui-expert`, ids deleted in `9527e35`. Removed, along with the two one-off
`prefix-cache`/`cache-memory` lines that duplicated what the loop already does.

## 13. The omission class, generalised (2026-07-31)

Section 12 fixed the spread-omission bug for the throughput fields. The user asked whether more
of it remained. It did — considerably more, in two directions.

### Every Rapid control had it, not four

`if (value) out.x = value` was the idiom throughout the Rapid branch of `_buildFormPreset`, so
`(unset)` and `Auto` were **unreachable states on any preset that already had a value**: the
spread restored the old one and it kept reaching argv. That covered `enable_thinking`,
`kv_cache_dtype`, `turboquant_mode`, `tool_call_parser`, `reasoning_parser`, `sampling_mode`,
`prefill_step_size`, and all seven sampling defaults. All now write unconditionally —
`null` for the `Option<…>` fields, the control's value for `prefill_step_size`, which is a plain
`u32` backend-side.

### Seven controls were read on save but unreachable in the editor

Found by trying to write the test for the above: `page.fill` failed, because
`modal-temperature`, `modal-top-p`, `modal-top-k`, `modal-min-p`, `modal-repeat-penalty` and
`modal-presence-penalty` live in the **generation** section, and the Rapid nav allowlist hid
every section except model and advanced. `modal-max-tokens` was in advanced but in a row with no
id, so the row-level chain hid it too.

All seven map to real flags (`--default-temperature` … , `--max-tokens`). They are now reachable:
the generation section is exempted for Rapid with its own deny-by-default field chain that keeps
the llama.cpp-only half (thinking / preserve-thinking / reasoning / reasoning-budget /
tool-call-format) hidden, and the max-tokens row got an id.

**This is the section-11 defect again, in the surface that was supposed to prevent it.** Worth
being precise about the near-miss: had the class-wide fix landed without this, every save would
have written `null` over the user's stored sampling defaults, because the inputs are unreachable
and therefore always read empty. The conditional idiom was masking the reachability bug.

### Why the existing guard missed it

`rapid-preset-visibility.spec.js` derived its id list with `/modal-rapid-[a-z0-9-]+/`. Six of the
seven do not carry the `rapid` infix, so the guard could not see them, and it only ever measured
visibility under the advanced section.

It now brace-matches the Rapid branch of `_buildFormPreset` and takes **every** `modal-*` id it
reads, then checks each against every section a Rapid preset can open. That broadened guard is
what found `modal-max-tokens` — a control neither the audit nor the first fix had noticed.

## 14. Disposition of the four capabilities left unexposed (2026-07-31)

Section 11 listed four things as "genuine capabilities with no UI after the phantom deletion"
and deferred the decision. Deciding them, because an undecided list is how the phantoms got
written in the first place — someone saw a gap and filled it with an invented flag.

### llama.cpp's web-UI flags — closed, no action

Not a gap. `default_launch_argv_omits_experimental_webui_mcp_proxy`
(`llama_cpp.rs:1009`) pins the launch argv exactly, and it deliberately omits the web-UI and
MCP-proxy flags: llama-monitor takes llama-server's default web UI as-is and does not configure
it. The phantom `web_ui_*` fields were those flags mis-copied into the *Rapid* builder, so
deleting them restored the intended state rather than creating a hole. The "inversion" noted in
section 11 — we could configure the web UI of the runtime without one, and not the runtime with
one — was only ever half real. Recording this so it does not get re-litigated.

### `--kv-disk-checkpoint-interval` — keep pinned at 0, and say why

Fully plumbed (`command.rs:421`, `mod.rs:77`) and passed on every launch — as a hardcoded `0`
from both the preset save path and the wizard. It is not missing UI; it is a constant with a
plumbing chain attached. `0` means "no disk checkpointing", which is the right default for
interactive use, where the retained cache lives in unified memory and a disk write buys nothing.
The question it actually raises is a measurement one — does checkpointing shorten cold restart
enough to matter? — so it belongs in the benchmark suite, not a dropdown. Leaving it pinned.

### `--response-cache-entries` — deferred pending measurement

No config field exists; adding one is a new feature, not a restoration. It caches whole responses
for identical greedy requests. Duplicate requests are rare in interactive chat and plausible in
agentic tool loops, and we have no data on which. Do not expose a knob whose benefit we cannot
state. Benchmark first.

### `--speculative-config` — promoted to its own task

The only one of the four that is a real product feature. The benchmark suite already builds the
flag correctly (`rapid-mlx-benchmark-suite.mjs:726`) and MTP measures 59–61% acceptance on
Qwen3.6-27B, so the capability is proven — but exposing it needs a config field carrying a
vLLM-style JSON object (`method`, `model`, `num_speculative_tokens`, `disable_auto_k`), external
sidecar model management, and the K-clamping rules for hybrid SSM architectures. That is a
feature with its own design, not a loose end from deleting a phantom. Tracked separately.
