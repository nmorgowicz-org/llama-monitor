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

### Open, found while verifying the above

- **Capability-gated flags are dropped without a reason.** A `turboquant_mode: "k8v4"`
  request against a runtime lacking `--kv-cache-turboquant` produces argv without the flag,
  an empty `requested_vs_effective`, and no `reasons`. An unsupported `speculative_policy`,
  by contrast, hard-errors. The preview surface renders whatever the backend reports, so it
  will show these correctly once the backend reports them — but today the silent-drop case
  is invisible in both places. Verified live on 2026-07-30.
- **Two pre-existing load-sensitive test races.** `phase7-presets.spec.js:52` reads
  `estimateScenario` as `interactive_coding_agent` where it expects `tool_research_agent`;
  `preset-flow.spec.js:249` times out polling `putCount`. Neither reproduces in the
  four-spec set alone (3/3 clean); both appear intermittently once a fifth spec adds
  parallel load. Not caused by the command-preview work.
