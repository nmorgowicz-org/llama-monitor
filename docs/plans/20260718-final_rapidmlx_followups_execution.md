# Rapid-MLX Final Follow-ups: Execution Companion

| Field | Value |
|---|---|
| Created | 2026-07-18 |
| Purpose | Low-context phase router and checkpoint ledger |
| Authoritative specification | [`20260718-final_rapidmlx_followups.md`](./20260718-final_rapidmlx_followups.md) |
| Intended reader | A context-free Coordinator agent |
| Execution model | Coordinator → Builder → fresh Verifier → focused remediation |
| Maximum phase context | 200k; stop and checkpoint before compaction |
| Product implementation status | In progress — Phases 0–10 reconciled; Phases 11–14.5 remain open |

> **Coordinator reconciliation (2026-08-09):** The ledger below was stale after the Spawn Wizard Guided/Pro work landed. Phases 6, 8B3, 9d–9f, and 10a–10e are now recorded against current-source behavior, release-built validation, and the archived completion/evidence packet. This does not close the independent Rapid-MLX roadmap work in Phases 11–14.5.

## Bootstrap Prompt for a Fresh Agent

Give a fresh agent this document and the following instruction:

> Act as the Coordinator. Begin with Phase 0 in `docs/plans/20260718-final_rapidmlx_followups_execution.md`. Follow its startup procedure, read every referenced section of the comprehensive plan, and use the required Coordinator -> bounded Builder -> fresh Verifier -> focused remediation workflow. Do not implement from the execution companion alone, skip prerequisites, reopen frozen decisions, modify or discard unrelated user work, or proceed past a user-authority gate without asking me. Maintain the checkpoint ledger and durable phase handoffs as specified. Continue methodically through verified phase closure unless the plan requires user approval or external evidence blocks safe progress.

The agent must treat this execution companion as the route map and [`20260718-final_rapidmlx_followups.md`](./20260718-final_rapidmlx_followups.md) as the authoritative specification. No conversation history from the planning session is required.

## 1. How to Use These Two Documents

This companion is the **execution interface**. The comprehensive plan is the **source of truth** for requirements, evidence, design decisions, formulas, tests, and stop conditions.

Do not implement from this companion alone. It intentionally summarizes rather than duplicates the specification.

Priority when documents appear to conflict:

1. Current user instruction.
2. Repository `AGENTS.md` and security/platform rules.
3. Comprehensive plan's exact phase and referenced design/decision sections.
4. This companion's routing and checkpoint state.
5. Older archived plans and implementation notes.

Stable Markdown headings are authoritative references. The line hints below are conveniences captured on 2026-07-18 and may drift. Never use a stale line number to override a heading's actual content.

## 2. First 15 Minutes for a Context-Free Coordinator

**Phase 6 evidence update (2026-07-27):** Qwen 3.6 35B’s 160K/200K real
workspace-fork matrix supports an 8 GiB retained-cache baseline. It keeps the
newest fork fast while 16 GiB preserves older branch boundaries; 16 GiB is not
a speed recommendation. The authoritative measurements and limits are in
`20260726-phase6_rapidmlx_cache_benchmarking.md`. Disk checkpoint work must
remain a separate `0` versus `8192` write/persistence experiment: current
Rapid source does not automatically reload evicted cache entries from disk.
The completed 200K INT4/8 GiB `0` versus `8192` lane found a 56.5 s (24%) cold
TTFT penalty at `8192` with no fork-reuse gain; use `0` for interactive cache
recommendations until the separate manual export/import persistence test is
qualified.

1. Read repository `AGENTS.md` completely.
2. Read this companion completely.
3. Inspect branch, `HEAD`, worktree, and current phase ledger without changing files.
4. Read these comprehensive-plan sections before briefing any sub-agent:

   - Purpose and canonical workloads — Section 1.
   - Critical gaps — Section 3.
   - Decision register — Section 8.
   - Pipelined implementation protocol — Section 9.
   - Phase dependency index — Section 10.
   - Exact active phase — Section 11.

5. Read only the additional design, cache, contract, matrix, and evidence sections routed by the active phase card below.
6. Revalidate mutable upstream facts used by that phase. Follow comprehensive Section 14.
7. Resolve every blocking decision for the phase. Do not let a Builder decide it implicitly.
8. Split large phases into independent parts (§4.1): any phase with 5+ files, 3+ distinct deliverables, or ≈140k+ budget should split into 2-4 parts. Example: Phase 3 (12 builder items) splits into Part A (items 1-7), Part B (item 6 probe), Part C (items 8-12).
9. Brief one bounded Builder with the exact comprehensive-plan line ranges for that part only.
10. After the Builder handoff, inspect the actual diff and brief a fresh Verifier for that part.
11. Update this ledger only after independent verification of each part.
12. Compress conversation after each verified phase to maintain context window.

Do not begin at Phase 1 merely because it changes code. Phase 0 freezes evidence and decisions needed to prevent later rework.

## 3. Navigation Map

> **Refinement note (2026-07-19):** the comprehensive plan was refined in a single deliberate pass (the E1–E11 edits are applied inline and cited by their E-numbers throughout the plan). That pass added ~180 lines, so the line hints below are stale — **use the headings, not the numbers**, and refresh with the `rg` command after this note. New anchors added by the refinement: §3.9 (rewritten — chat-template tool-call reliability), §8.3 (M5 Max `[escalate→device]` measurement envelope), §9.6 (four-bucket gate taxonomy), and D32 (preset schema migration/versioning).

Current line hints in the comprehensive plan (pre-refinement — treat as approximate):

| Section | Current line | Why it exists |
|---|---:|---|
| 1. Purpose/workloads | ~16 | Scope and canonical OpenCode/Hermes/OpenClaw/SillyTavern workloads |
| 2. Research baseline | ~55 | Audited upstreams, fixtures, local areas, screenshot and client evidence |
| 3. Critical gaps | ~213 | P0/P1 defects that must not disappear during implementation |
| 4. Capability priority | ~392 | First-class, Advanced, troubleshooting, and omitted controls |
| 5. Design decisions | ~459 | Two approaches plus recommendation for consequential designs |
| 6. Cache design | ~838 | Rapid caches, workload profiles, and llama.cpp cache/concurrency policy |
| 7. Data contracts | ~1198 | Source, memory, execution policy, estimate, capability schemas |
| 8. Decisions/assumptions | ~1335 | A1–A58 and facts still requiring measurement |
| 9. Agent protocol | ~1422 | Coordinator/Builder/Verifier/remediation rules |
| 10. Phase dependencies | ~1514 | Phase ordering, decisions, and context ceilings |
| 11. Implementation phases | ~1545 | Exact Builder/Verifier work and hard gates |
| 12. Validation matrices | ~1997 | Cross-product and end-to-end requirements |
| 13. Evidence ledger | ~2159 | Claim-to-immutable-source mapping |
| 14. Revalidation | ~2279 | How to refresh drift-prone upstream evidence |
| 15. Completion ledger | ~2293 | Requirement-level closure status |

Refresh line hints with:

```bash
rtk rg -n '^## |^### Phase|^### 6\.|^### 8\.' \
  docs/plans/20260718-final_rapidmlx_followups.md
```

Do not mechanically rewrite links after small line drift. Update this table only when navigation becomes misleading.

## 4. Global Execution Rules

### 4.1 Context management (CRITICAL)

Sub-agents (Builder/Verifier) do NOT have compression tools. They blow context on large phases and degrade quality via auto-compaction. Coordinator (which has compression) MUST enforce these rules:

- **Split large phases into independent parts.** Any phase with multiple distinct deliverables or covering significant code (≈140k+ budget, 5+ files, or 3+ logical concerns) MUST be split into 2-4 independent parts. Example: Phase 3's 12 builder items split into Part A (capability snapshots + deps), Part B (on-device probe), Part C (endpoint matrix + MTP + sampling).
- **One part per Builder context, one part per Verifier context.** Sequential: Part A Builder → Part A Verifier → remediate → Part B Builder → Part B Verifier → etc. Never parallel parts.
- **Do NOT give sub-agents the entire comprehensive plan.** They will read thousands of lines and blow context before doing meaningful work. Provide ONLY exact line ranges: e.g. "read Phase 3 builder items 1-7 at lines 1718-1730" not "read Phase 3 in full."
- **Targeted reads only.** Sub-agent briefs must specify line ranges for every comprehensive-plan reference: gaps, decisions, contracts, phase sections. Never say "read Section X" without line numbers.
- **Do not combine phases** because an earlier phase was short.
- **Coordinator compresses after each verified phase** to maintain a clean context window.
- The phase Verifier (or final part Verifier) checks the complete phase, including work from all parts.

### 4.2 Agent authority

- Coordinator owns scope, decisions, phase state, commits, pushes, PRs, and user gates.
- Builder implements and tests only the assigned phase.
- Verifier independently checks specification, code, tests, security, migrations, platforms, and UI.
- Builder results are evidence inputs, not sign-off.
- Verifier does not redesign opportunistically.
- No sub-agent commits, pushes, opens a PR, or expands scope unless the user explicitly changes this rule.

### 4.3 UI work

- Build release before captures when static product assets changed.
- Run screenshot scenarios sequentially.
- Use real screenshots as the visual source of truth.
- Compare both backends where shared components change.
- Cover dark/light and desktop/narrow for material reflow.
- Stop for user review before adopting consequential IA changes.

### 4.4 Evidence and recommendations

- Exact executable identity, version, help hash, dependency set, model revision, and client version matter.
- “Installed,” “flag present,” “provisional,” “qualified,” and “incompatible” are distinct.
- Upstream source outranks README summaries; real runtime evidence outranks assumptions.
- A client/framework concurrency maximum is not a cache-entry recommendation.
- A capability is not automatically a recommended default.
- Every recommendation teaches what, when, why, when not, memory cost, evidence, and confidence.

### 4.5 Visible-control completeness

Every visible setting must complete this trace:

```text
capability/evidence
→ UI descriptor
→ typed schema/default
→ validation
→ launch/request mapping
→ edit/restore/clone
→ review/command preview
→ unit/integration/UI tests
```

Hide or render read-only any concept that cannot complete the trace.

### 4.6 Gate taxonomy (four buckets)

Implementation is executed 90%+ by a finetuned local model (Qwen3.6-27B, stable 200k context), with ~10% escalation to a frontier model. The Builder→Verifier loop is a **local** dev-iteration loop, not Claude sub-agent fan-out. So "escalate" (comprehensive §9.5) splits four ways — every hard gate carries exactly one tag (comprehensive §9.6):

| Tag | Who decides | Spends frontier quota? |
|---|---|---|
| `[local-verifiable]` | the local model self-runs an exact CHECK with a machine-decidable `PASS iff` | no |
| `[decide-once]` | Nick settles it once in refinement (copy strings, thresholds, A-items); then it *becomes* `[local-verifiable]` | no |
| `[escalate→device]` | Nick + the local model on the M5 Max: measurements, wire captures, calibration, KV floor (the §8.3 envelope) | no |
| `[escalate→frontier]` | genuine reasoning judgment the local model cannot do | **yes — the only bucket that does** |

Coordinator behavior: prefer `[local-verifiable]`; route measurement/wire-capture/calibration gates to `[escalate→device]` (real hardware, not quota); reserve `[escalate→frontier]` for the small pre-counted set of reasoning judgments. A `[decide-once]` gate, once decided, is treated as `[local-verifiable]` with its resolved value inlined — do not reopen it.

## 5. Phase Router

Each card identifies the minimum comprehensive-plan reading set. The exact phase section remains mandatory in full.

### Phase 0 — Evidence freeze and decisions

- **State:** Verified complete
- **Budget:** 80k
- **Depends on:** nothing
- **Read:** comprehensive Sections 1–3, 5, 8–10, Phase 0, 13–14.
- **Primary output:** pinned Rapid/llama/client/HF evidence, real fixtures/checksums, drift report, resolved near-term decisions, requirement traceability. For D31, pin the exact alias inventory/checksum, bypass and non-wiring paths, implementation-derived formulas, tests/commits, and any observed stored-byte receipts; do not treat the stale `4.6x total` claim as qualified evidence.
- **User gates:** only a triggered Section 6.1 authority gate. Accepted security, context, workload/admission, dependency, platform, and conservative cache policies are not reopened merely because measurements remain pending.
- **Completion proof:** immutable fixture/source manifest; all Phase 1–3 blockers answered or routed to a documented conservative policy; Phase 0 changes no runtime/dependency behavior.
- **Artifacts:** `docs/plans/handoffs/20260718-final-rapidmlx-followups/phase-0/` + `tests/fixtures/rapid_mlx/configs/` (6 model configs pinned to commits with SHA-256)
- **Key findings:** [E1] NO --chat-template/--template-file flag in Rapid-MLX; [E3] --kv-cache-dtype {bf16,int8,int4} default int4; runtime upgraded v0.10.10→v0.10.12 (2 non-breaking additions only); 2 decision packets unresolved per §6.1

### Phase 1 — Urgent correctness and interim safety

- **State:** Verified complete
- **Budget:** 100k
- **Depends on:** Phase 0
- **Read:** critical gaps 3.1–3.3, 3.10–3.11; decisions D12, D18, D24, D26; A11/A17/A19/A30/A44; Phase 1; security matrix.
- **Primary output:** valid Rapid tool-parser argv; truthful no-op controls; accepted data-only/custom-code/provisional source distinction; revision-scoped custom-code consent; no automatic unlimited llama host cache; no unconditional llama Web UI MCP proxy while preserving the upstream-enabled bundled UI baseline; corrected immediate copy.
- **Completion proof:** exact argv and negative-capability tests; ordinary inspected data-only MLX repositories launch without blanket remote-code warnings; custom-code detection never executes repository code and consent is immutable-revision specific; old presets deserialize; ordinary external-agent llama preset omits MCP proxy/tools/agent bundle while the explicit/follow-upstream UI state remains truthful; unified-memory single-user Auto emits `-cram 0`, never `-1`; sentinel tests prove explicit `-1` remains enabled/unlimited rather than being treated as disabled.
- **Fixes applied:** (1) tool_call_parser: bool→Option<String>, --tool-call-parser openai, --auto-tool-choice→--enable-auto-tool-choice; (2) force-spec-decode/no-spec-decode removed from escape-hatch; (3) A11 trust_remote_code: needs_trust_remote_code() heuristic + validate_trust_consent() with repo_id@revision format, launch blocks without consent, HF_TRUST_REMOTE_CODE=1 only with consent; (4-6) cache-ram/webui-mcp-proxy/context_size verified N/A for rapid-mlx (llama.cpp only)
- **Tests:** 17 new tests (10 command.rs + 7 model_resolver.rs); 146 rapid_mlx tests pass; build/clippy/fmt clean
- **Artifacts:** `docs/plans/handoffs/20260718-final-rapidmlx-followups/phase-1/`
- **Files changed:** command.rs, mod.rs, escape_hatch.rs, model_resolver.rs, launch.rs, sessions.rs, rapid_mlx_runtime.rs

### Phase 2 — Typed source, sampling catalog, and request defaults

- **State:** Verified complete
- **Budget:** 160k
- **Depends on:** Phase 1
- **Primary output:** one Rust-owned Rapid source codec; legacy migration; one cross-backend sampling mode catalog; metadata/lineage finetune resolution; complete mode visibility and provenance; omission-only request defaults; explicit-zero provenance; Coding agent default; Roleplay path semantics. Establish the **preset schema version/migration contract (D32, E10)**: a schema-version field, forward-migration on read, save→load→save round-trip tests, and safe downgrade — every preset-shape change (here and in D27/D20/D30/D23) plugs into it instead of ad-hoc `serde(default)`.
- **Completion proof:** every source variant survives display/edit/clone/save/estimate/library/launch; every model has universal sampling choices; every recognized family/finetune shows all curated modes on both backends; Unsloth values match pinned sources; explicit client values win; typed fixture no longer opens legacy data; presets from today's shipped llama-monitor migrate without loss and round-trip.
- **Changes:** RapidMlxModelSourceView codec with from_source() + preset_for_api() wiring; SamplingCatalog::modes_for_model() by family/arch with backend-aware coverage (llama_cpp_coverage/rapid_mlx_coverage); D32 schema v0→v1 migration (schema_version field, migrate_preset(), safe degradation); escape_hatch_flags omission-only defaults; 6 HF config fixtures in tests/fixtures/rapid_mlx/configs/
- **Tests:** 180 rapid_mlx/preset/sampling_catalog tests pass; migration tests with real fixtures; build/clippy/fmt clean
- **Files changed:** command.rs, escape_hatch.rs, mod.rs, model_resolver.rs, launch.rs, batch_import.rs, sampling_catalog.rs (new), model_defaults.rs, presets/mod.rs, benchmark.rs, presets.rs, rapid_mlx_runtime.rs, sessions.rs + fixtures/handoffs/
- **Read:** gaps 3.2/3.4; D5/D16/D21/D22/D27/**D32**; contracts 7.1/7.3; A2/A20/A32/A38/A40/A45/A51–A52; Phase 2; source/client matrices and pinned Unsloth evidence.
- **Primary output:** one Rust-owned Rapid source codec; legacy migration; one cross-backend sampling mode catalog; metadata/lineage finetune resolution; complete mode visibility and provenance; omission-only request defaults; explicit-zero provenance; Coding agent default; Roleplay path semantics. Establish the **preset schema version/migration contract (D32, E10)**: a schema-version field, forward-migration on read, save→load→save round-trip tests, and safe downgrade — every preset-shape change (here and in D27/D20/D30/D23) plugs into it instead of ad-hoc `serde(default)`.
- **Completion proof:** every source variant survives display/edit/clone/save/estimate/library/launch; every model has universal sampling choices; every recognized family/finetune shows all curated modes on both backends; Unsloth values match pinned sources; explicit client values win; typed fixture no longer opens legacy data; presets from today's shipped llama-monitor migrate without loss and round-trip.

### Phase 3 — Runtime and dependency qualification

- **State:** Verified complete (commit `a3e867cb`, 2026-07-23, confirmed ancestor of HEAD). Do not treat model-specific vision support as qualified until the Phase 7 real-model smoke lane passes.
- **Budget:** 140k total (~60k A + ~60k B + ~50k C)
- **Depends on:** Phases 1–2
- **Read:** gaps 3.4/3.8–3.10; D13/D24/D25/D27; contract 7.5; A2/A14/A15/A17–A19/A26/A29/A48/A51–A52; Phase 3; runtime/client matrices; evidence ledger.
- **Primary output:** automatically generated exact-executable capability snapshots for Rapid and llama; upstream dependency-contract and resolved-receipt handling; a first-class **on-device, user-driven update-validation probe** `[escalate→device]` (modeled on the existing thin llama.cpp beta-update validation) — the only qualification the Phase 3 gate depends on; dependency/extras states; endpoint matrix; alias/finetune confidence; MTP concurrency qualification; per-field Rapid sampling-default CLI/cascade coverage. Any Nick-owned upstream-monitoring CI/manifest is **additive/optional** and must not gate Phase 3 (E6).
- **Completion proof:** no manual per-release certification treadmill; drift is handled by the on-device probe (near-daily rapid-mlx/dependency updates validated on the user's device, independent of llama-monitor releases), and the absence of any upstream CI never blocks this or a dependent phase; an unseen environment satisfying upstream constraints and passing the on-device baseline receives no global disclaimer; only concrete failures or indeterminate selected Advanced capabilities produce actionable per-feature notices; probes are bounded; managed installs retain a resolved receipt and rollback; Rapid MTP fallback and llama MTP build/model distinctions are represented.
- **Part A (~60k):** capability snapshots + dependency handling — builder items 1-7 (~line 1718-1730), gap 3.8 (~351-362), gap 3.10 (~376-390), D13 (~627-638), D24 (~723-730); files: capabilities.rs, rapid_mlx/{compatibility.rs, discovery.rs, info_query.rs}, rapid_mlx_runtime.rs
- **Part B (~60k):** on-device probe — builder item 6 (~line 1725-1727), hard gates (~1734-1738), gap 3.10 (~376-390); files: rapid_mlx/{runtime.rs, updater.rs}
- **Part C (~50k):** endpoint matrix + MTP + sampling — builder items 8-12 (~line 1727-1738), hard gates (~1734-1738), D25 (~731-742), D26 (~743-759), D27 (~760-769); files: capabilities.rs (llama), rapid_mlx/{compatibility.rs, discovery.rs}, rapid_mlx_runtime.rs

#### Upstream release watch — 2026-07-23

- **Supported published baseline:** Rapid-MLX `v0.10.17` (`69b1fcf81c6a1dada4ea6103f71f358796c01c7e`), selected as release metadata and still subject to the bounded on-device capability probe. It includes the hybrid-VLM/base-wheel installation repair from `bf15f02f27`.
- **Do not claim yet:** `v0.10.17` predates main commit `9a5151e147` (`fix(mllm): auto-degrade to text lane for vision-config checkpoints with no vision tower`). Until that commit is included in a published release and passes the on-device probe, a vision-declared model without a verified vision tower must be diagnosed as unavailable rather than promised a text fallback.
- **Required next-release validation:** test (1) a real vision-tower model with MLX-VLM available and an image request, (2) MLX-VLM missing/incompatible remediation, (3) a vision-config/weightless-tower checkpoint in Auto mode falls back to text and reports post-degrade modality, and (4) explicit `--mllm` fails clearly for that checkpoint. Add dark/light/narrow capture coverage once Phase 7 wires MLLM through launch and review.

#### VLM + performance calibration locator (added 2026-07-23)

- **Authoritative detail:** comprehensive plan §3.8a is the revision-bound native-MLX vision qualification contract; D28/D31 “device measurement protocol” is the required real-model Standard/K8V4/cache/context/performance procedure. Do not restate or weaken either here.
- **Current evidence:** Gemma4 QAT aligned revision is text-capable but Vision Unavailable (declared config, 211 missing tower tensors); Gemma OptiQ is a distinct 6.01-bit OptiQ/MLX-LM companion path, not a standard Rapid baseline; Froggeric Qwen3.6-35B-A3B is Vision Components Present fallback/control; Nightmedia Qwen3.6-35B-A3B Fable Holo 3.1 MXFP4 revision `16279aa65cee814c6b23e068a71eec7e1617fae0` is the user-selected primary 35B performance target but Vision Unavailable on Rapid 0.10.17 (auto metadata falsely says vision; image request rejects; explicit `--mllm` rejects hybrid `ArraysCache`); Nightmedia Qwen3.5-9B Defiant 1M Q8 revision `59b2511bfda3a1ce17b999f90d22b63e98c87f7e` is the primary Vision Components Present/long-context fixture. Neither may be promoted until a live image request succeeds.
- **Where work lands:** Phase 5a-Part 5 consumes receipts for estimator/cross-surface calibration; Phase 5b consumes fresh pressure/fit observations; Phase 6 owns evidence-backed cache recommendations/copy; Phase 7 owns capability-gated `--mllm` and requested/effective control UI; Phase 8 owns revision-aware VLM badges/cards and source lifecycle; Phase 11 owns live modality/cache/Metal diagnostics; Phase 12/14 rerun the full matrix and release gate.
- **Immediate order:** pin 9B Nightmedia revision/config/index hashes → auto/explicit-MMLM smoke → text + minimal PNG + coding-screenshot image request → staged 8k/32k/128k/255k fit cells; in parallel retain the 35B Nightmedia text-only model for bounded cache-enabled Standard/K8V4 repeated-prefix performance cells → record raw receipts and only then assess recommendation eligibility. A text request, HF tag, config field, `info` line, live model metadata, or cache-disabled one-shot never earns a verified badge or performance recommendation.

### Phase 4 — Normalized MLX architecture metadata

- **State:** Verified complete (commits `28437172`/`6543f9f6`/`ae425377` Parts A/B/C, confirmed ancestors of HEAD).
- **Budget:** 170k total (~55k A + ~55k B + ~60k C)
- **Parts:** A (ModelMemoryProfile + config parsing), B (Qwen3.6/Gemma4/MoE/MTP geometry), C (context/*8/HF lookup/estimator integration)
- **Depends on:** Phase 0 fixtures and Phase 2 identity; A25
- **Part A (~55k):** Core profile + config parsing — builder items 1-2 (~line 1745-1747); files: new backend-neutral memory-profile module, rapid_mlx/mlx_meta.rs, tests/fixtures/mlx_configs/; deliver: ModelMemoryProfile/LayerMemoryGroup types, nested text_config parser, wrapper-field protection
- **Part B (~55k):** Architecture geometry — builder items 3-5 (~line 1747-1750); files: memory-profile module, rapid_mlx/mlx_meta.rs, tests/fixtures/mlx_configs/; deliver: Qwen3.6 DeltaNet/recurrent, Gemma4 heads/windows, MoE experts, MTP/companions ownership; hard gates: Qwen3.6/KV, Gemma4/global KV, recurrent state, no double-count
- **Part C (~60k):** Math + context + data sources — builder items 6-8 (~line 1750-1754); files: memory-profile module, hf/mod.rs, models/library.rs, web/api/vram.rs, rapid_mlx/mlx_meta.rs, llama/vram_estimator/; deliver: context ceiling propagation, *8 bug fix, HF revision-aware lookup, local MLX config/index parsing, heuristic fallback; hard gate: no llama GGUF regression
- **Read (exact ranges):** gap 3.5 (~291-311), gap 3.7 (~328-350), D1 (~475-489), D2 (~490-504), A25 (~1417), A53 (~1445), contract 7.2 (~1274-1295), Phase 4 builder items (~1745-1754), hard gates (~1758-1762)
- **Read:** gaps 3.5/3.7; accepted D1/D2; contract 7.2; A25/A53; Phase 4; architecture matrix and HF config evidence.
- **Primary output:** GGUF/MLX adapters into one evidence-bearing normalized geometry profile; full/local/linear/recurrent layer groups; nested config parsing; MoE/MTP/companions/context evidence; correct size math; no shared runtime math.
- **Completion proof:** six real pinned family fixtures with independent expected facts; Qwen3.6 and Gemma4 KV/recurrent geometry correct; degraded evidence is field-specific.

### Phase 5 — Execution policies and estimator

- **State:** Implementation verified complete (5a + 5b); bounded evidence/ledger closeout in progress as of 2026-07-24. The remaining work is not a rebuild of the estimator or availability policy: it is a fresh integration regression pass, revision-pinned estimator-versus-runtime calibration, and conservative evidence-tier recording before Phase 6 cache guidance can promote any recommendation.
- **Budget:** 190k total (5a ~120k across 5 parts, 5b ~70k across 3 parts)
- **Depends on:** Phases 3–4
- **Read:** gaps 3.5–3.7/3.11; accepted D1–D4, D18–D25, D28, and D30–D31; contracts 7.2–7.4; A1/A3–A5/A21–A22/A42–A43/A46–A48/A53–A54/A58; existing RTX 5090/M5 Max calibration evidence; Phase 5; memory/llama/client matrices.
- **Primary output:** Rapid-native policy and estimator; corrected llama unified/partitioned context contract; active versus retained memory; typed Auto/Standard/K8V4/V-only retained-prefix policy with exact alias eligibility; one backend-owned live unified-memory snapshot with safe-now/reclaim/app-close/configured-cap scenarios; workload-fit quant guidance; explicit llama MTP single-stream mode; explicit Rapid MTP companion ownership plus memory-first one-active Auto and fully re-estimated Advanced overlap.
- **Completion proof:** all surfaces consume the same timestamped snapshot and agree; Rapid cannot inherit stale llama/HF memory caches; no total RAM or wired cap is mislabeled available; TurboQuant savings apply only to qualified retained conventional-KV portions, Standard is not mislabeled FP16, unknown finetunes do not inherit alias eligibility, and transient decompression peaks remain visible; recovery actions distinguish allocator cache, reusable state, runtime/app memory, and OS disk cache; recovery is conservative and measured before/after; process diagnostics redact commands and use honest footprint/RSS/backend labels; sysctl mutation is bounded/reversible/exactly verified/restart-aware; the user's verified reboot-persistent M5 Max path is preserved and its mechanism/version evidence recorded without untested cross-version generalization; Qualified/Calculated/Provisional and uncertainty boundaries are honest; existing 5090/M5 calibration does not regress; raw Rapid measurements reproduce; every embedded/external MTP companion and cache reservation is additive; Rapid single-active Auto protects near-capacity quant/context fit while overlap refits worst-admitted memory and context guarantees; llama command and estimator describe the same KV pool; no Rapid `ctk/ctv`; recommended quant satisfies workload policy.
- **Formal sub-phases (E5, comprehensive §Phase 5):** Phase 5 is two formal sub-phases, each with its **own hard gate and its own fresh Verifier pass** — not one Verifier over two packets. **5a** = execution policy + `MemoryBreakdown` + estimator core + cross-surface estimate equality (comprehensive Builder items 1–14), ~120k across 5 parts; it must reach `Verified complete` before 5b starts. **5b** = `MemoryAvailabilitySnapshot` + reclaim + wired-limit + acquisition-gap repairs (items 15–18), ~70k across 3 parts. Rationale: coherence-per-packet for the local model (not token fit), compounding with the §4.6 gate taxonomy. Track 5a and 5b as distinct checkpoint rows.
- **Phase 5a Parts:**
  - **5a-Part 1 (~40k):** Verified complete — Rapid execution policy types + MemoryBreakdown foundation — items 1-2; new execution_policy.rs with KvCacheDtype {bf16,int8,int4}, TurboQuantMode {v4,k8v4,none}, RapidMlxExecutionPolicy (reasoning→int8 override), MemoryBreakdown (8 additive components); 19 tests pass; committed 3bba01c
  - **5a-Part 2 (~45k):** Verified complete — TurboQuant/D31 + active vs retained separation — items 3-5; TurboQuant savings (K8V4=0.575, V4=0.34) on retained KV only; active/retained split via rapid_planning_context_tokens/rapid_retained_cache_tokens; transient_peak_bytes included; eligibility gating; 36 new tests, 49 total pass; committed 4410a0f
  - **5a-Part 3 (~40k):** Verified complete — llama.cpp slot/unified-KV revalidation + MTP single-stream — items 8,9,12; no code changes needed (existing math correct); host cache/checkpoints correctly excluded; MTP overhead counted; 17 regression tests; 256 tests pass; committed 1b7fa34
  - **5a-Part 4 (~45k):** Verified complete — workload scenarios + quant rebase + Rapid MTP modeling — items 10,11,13,14; 5 scenario types (InteractiveChat/CodingAgent/ToolResearchAgent/BatchEval/Roleplay); quant_comparison_table scenario-based; agentic min 32K; Rapid MTP embedded/external with D25 admission; ClientType (App vs ExternalClient); 1013-line workload_scenarios.rs new; 143 tests pass; committed 345127a
  - **5a-Part 5 (~40k):** Verified complete — Cross-surface equality wiring + calibration fixtures — items 6-7; commit `791635e`; wizard/preset/welcome/Model Library/HF preview consume the same `MemoryBreakdown` API result; calibration fixtures are present; 5a exit gate passed.
- **Phase 5b Parts:**
  - **5b-Part A (~55k):** Verified complete — MemoryAvailabilitySnapshot core + Rapid fresh-snapshot repair — item 15 + 18 sub-item; commit `72d9efb`.
  - **5b-Part B (~60k):** Verified complete — Wired-limit hardening — item 17; commit `dcd6da8`.
  - **5b-Part C (~130k):** Verified complete — Reclaim + frontend propagation repairs — items 16 + remaining 18 sub-items; commit `b2a6fed`; verifier checkpoint `6a14cc7`.

#### Phase 5 evidence closeout (2026-07-24)

Implementation acceptance does not automatically qualify runtime recommendations. Before Phase 6 may call a cache/quant configuration Recommended or Verified, the closeout must: (1) rerun a focused integrated Phase 5 regression and cross-surface UI/API equality check; (2) pair revision-pinned fresh-server runtime receipts with the canonical estimator result and record peak-memory residuals against the ±10% qualified envelope; and (3) record each tested configuration as Qualified, Calculated, or Provisional without promoting a family-wide claim from one conversion.

Current conservative evidence: Qwen 3.6 35B A3B text receipts fail the source-retrieval fixture at roughly 32k raw prompt tokens for both int8 and int4, so no high-context or agentic recommendation may be made. Qwen 3.5 9B Defiant has a corrected 32k-output/16k-reasoning stateful-tool receipt with a parsed call but incorrect initial call ordering; it is not agent-qualified. K8V4 has no repeat/extension evidence sufficient for a recommendation. Tested hybrid Qwen image paths remain Vision Unavailable on Rapid 0.10.17, which is a runtime-path result—not a general model capability claim. Until the relevant rows qualify, Rapid int8 is only the conservative Provisional agentic control, int4 is a memory-saving candidate, and K8V4 is not recommended.

**Re-test trigger (2026-07-24, do not skip):** upstream PR #1192 ("reasoning-gated forced tool grammar via thinking budget", merged into 0.11.0) hard-constrains forced tool-call argument JSON once a `reasoning_max_tokens` budget closes `</think>` — this directly targets the "incorrect initial call ordering" defect recorded above, which predates the fix. The Qwen 3.5 9B Defiant stateful-tool receipt must be re-collected post-upgrade with `reasoning_max_tokens` set (not only `reasoning_effort`) before the ordering defect can be called resolved or still-reproducing. `reasoning_max_tokens` is also a net-new per-request field (upstream #1185/#1186) with no backend (`src/inference/rapid_mlx/*`) or frontend (`static/js/features/*.js`) exposure yet — see `docs/plans/20260724-rapidmlx-benchmark-continuation.md` "Outstanding investigation items" 5-6 for the full trace and required schema/UI additions.

**Current calibration posture (2026-07-25):** Rapid-MLX v0.11.0 issue #1197 keeps observed batch-generation active KV in bf16 despite requested int8/int4. Phase 5 therefore models active KV as effective bf16 while preserving the requested value in the response. Pair each fresh-server receipt with its exact token-free estimator request/response using `scripts/write-estimator-calibration-receipt.mjs`; tune only on designated rows and reserve a context/dtype holdout. The upstream fix may be evaluated at its exact commit now, then requires a compact release confirmation run before int4/int8 active-KV savings can be promoted.

**v0.11.0 BF16 calibration result (2026-07-25):** fresh Qwen 3.5 9B Defiant PFlash-off requested-int8 cells now have versioned, token-free paired calibration receipts under `tests/fixtures/calibration/rapid-mlx-receipts/qwen35-9b-v011-bf16-calibration/`. Requested int8 correctly resolves to estimator-effective bf16. The 32k and 65k tuning residuals are -1.50% and +4.93%; the independent 131k holdout is +17.06% (unsafe under-prediction), outside the ±10% Qualified bar. Keep this Rapid v0.11.0 envelope **Provisional** and do not promote a memory-fit/cache/quant recommendation or change global formula constants from this one conversion. A later release containing the upstream #1197 fix gets a compact confirmation/recalibration pass.

**Phase 5 → Phase 6 transition (2026-07-26):** finish the paired AilexLeon Gemma 4 26B LM/VLM cold-path comparison before beginning cache qualification. Preserve source-build Qwen KV rows as pre-release evidence. Phase 6 then starts with the telemetry-contract control (`cold → exact replay → +512`, one persistent server, PFlash/speculative decoding off, raw `/metrics` snapshots before/after each request). Normalize only series actually emitted and behaviorally validated by that control; retain unknown cache values as unavailable and document any derived values with their formula. The detailed Phase 6 benchmark plan is [`20260726-phase6_rapidmlx_cache_benchmarking.md`](./20260726-phase6_rapidmlx_cache_benchmarking.md). No cache recommendation, cache reservation formula, or UI policy may promote before this gate and the subsequent model-specific evidence pass.

**Deferred dense-Gemma scope (2026-07-26):** Gemma 4 31B QAT is deliberately deferred. It is a larger dense comparison than the already-qualified 27B dense Qwen and is not part of the Phase 5 closeout. After the Phase 6 cache harness and initial model tracks are established, run its cold and cache qualification through that single completed protocol rather than spending a separate pre-cache matrix now.
- **Dependencies:** 5a P1→P2→P4→P5; 5a P3 independent of Rapid chain but feeds P5; 5b A→B,C (B,C parallel); 5b D depends on A,B,C

### Phase 6 — Cross-backend cache guidance

- **State:** Shipped 2026-08-05, reduced scope (see note below)
- **Milestone (2026-08-10):** Added authenticated `/api/metrics/inference` with a stable privacy-safe metrics dictionary. Each aggregate metric reports `effective` or explicit `unavailable` state, unit, and reason; missing backend telemetry is never zero-filled. WebSocket payloads expose the same dictionary. Cross-backend Doctor now checks llama.cpp tool-enabled KV cache below `q8_0`, disabled/empty Jinja template paths, malformed Rapid-MLX `--tool-call-parser` values, and broken managed optional extras. Unit coverage covers the new state contract and Doctor checks. Remaining Phase 11 work: authenticated bounded exports/backups/network and disk-state visibility/approved cleanup.
- **Milestone (2026-08-10, audit):** Existing authenticated database surfaces satisfy the storage visibility/approved-cleanup baseline: `/api/db/stats` reports bounded counts and file size, `/api/db/integrity` is read-only, maintenance operations are explicit, backups use SQLite online backup, listing is authenticated, and deletion requires `db-admin-token`. No new raw database export path is needed for this packet.
- **Budget:** 170k
- **Depends on:** Phase 5
- **Read:** comprehensive Section 6 in full; D14/D15/D17–D20; A6–A9/A21/A23/A31–A37/A41; Phase 6; cache/client matrices; cache evidence ledger rows; [`20260726-phase6_rapidmlx_cache_benchmarking.md`](./20260726-phase6_rapidmlx_cache_benchmarking.md).
- **Primary output:** shared Reusable prompt state Auto/Off/Advanced Custom with backend-native effective behavior; Rapid hybrid and expert-only response-cache policies; bounded llama prompt-cache policy; educational workload profiles; recommendation/refusal logic. No per-runtime cache-repeat fingerprinting/telemetry subsystem is built — rejected outright (2026-08-05): the project does not collect usage data from users, so no HMAC-fingerprint shadow observer, opt-in trial, or later-built variant of it is in scope, full stop.
- **Completion proof:** response cache Off for normal agents/roleplay; Rapid Auto uses the smallest memory-safe working set for the dominant single-user loop, does not permanently provision for brief cron overlap, and resolves Off when ineligible/unbounded; llama unified-memory Auto defaults extra host states to `0` while ordinary common-prefix reuse remains active, and only confirmed evidence-backed surplus permits a bounded positive cap; no generic concurrency value becomes cache size; no fingerprinting or telemetry subsystem exists anywhere in the codebase.
- **What actually shipped (reduced scope, 2026-08-05):** a `CacheMode` enum (`Auto`/`Off`/`Custom`) on both backends, resolved at a single choke point per backend (`RapidMlxAdapter::apply_config` for Rapid — shared by the real launch path and the command-preview endpoint; `LlamaCppAdapter::append_kv_cache_args` for llama.cpp). `Custom` is the serde default so presets saved before this field existed keep their exact stored values unchanged. Rapid `Auto` resolves to the measured single-user coding-agent recommendation (`--cache-memory-mb 8192`, `--hybrid-cache-entries 16`); llama.cpp `Auto` resolves to the same disabled state as `Off` (`--cache-ram 0`) because no workload-scenario evidence is plumbed into the launch path yet to justify a bounded positive cap. Every surface that reads or writes the raw cache fields was audited and brought in line with `cache_mode`: `doctor.rs`'s cache diagnostics now resolve through `cache_mode` before building findings (previously it read the stale raw fields directly, so an Auto/Off preset would get findings describing values that weren't actually launched); `sessions.rs`'s cache-related `FixAction`s now force `cache_mode` back to `Custom` when they write a raw field (previously a fix applied to an Auto/Off preset would be silently overridden by `CacheMode::resolve()` at the next launch). UI: a "Prompt Cache Mode" selector in **both** the preset editor and the first-launch spawn wizard, for both backends, wired to show/hide (llama.cpp) or disable (Rapid) the underlying raw fields when not in Custom mode. Rust unit tests cover `resolve()` for both backends. **Not built:** the full workload-scenario-driven multi-tier Auto (`WorkloadScenario` stays scoped to `vram_estimator`, not threaded into launch config) and an elaborate recommendation/refusal-messaging engine. The pre-existing `PrefixCacheGuidance::derive` dead code in `capabilities.rs` was left untouched: it targets `--max-cache-blocks`, an unused experimental paged-cache flag, not the actually-shipped `--cache-memory-mb`/`--hybrid-cache-entries` mechanism.

### Phase 6.5 — Speculative decoding / MTP runtime qualification

**Objective:** Reconcile stale Phase 3/5 MTP assumptions with the selected
current Rapid source, then produce revision-pinned functionality, acceptance,
correctness, and memory evidence before Phase 7 exposes a control.

**Reason for a full phase:** no local MTP path has been independently
qualified. Recent source fixes are material inputs: `738a44e` repairs Qwen
sidecar extraction/loadability and reports K=1 acceptance ~0% → ~89%;
`eab126d` admits eligible local model directories; `a79997e` repairs Qwen 3.6
bundled-MTP norm corruption. They are source-build evidence, not release
qualification.

**Packets:** (1) source/capability inventory of embedded, extracted, and
external heads/sidecars plus exact requested/effective/fallback argv; (2)
benchmark-harness MTP lane recording proposal/accepted/rejected tokens,
acceptance, TTFT, PP/TG, correctness, active/peak memory, and fallback; (3)
Rapid Qwen 3.5/3.6 local/HF and eligible sidecar tests; (4) GGUF embedded and
`-md` draft-model tests, including Gemma 4, with normal llama MTP at one slot;
(5) fresh estimator ownership/admission audit. Missing metrics remain
unavailable rather than inferred from speed.

**Exit gate:** every Phase 7 MTP control traces capability → eligibility → argv
→ launch → observed metric → memory estimate → restore/summary. Unsupported
or provisional paths have an explicit reason and are not exposed as enabled.
Do not begin broad UI exposure, multi-slot experiments, or release claims.

### Phase 7 — Critical settings and shared UI

**NOTE:** Previous builder mislabeled Part B UI work as "Part A" (commits 3437201, cbf4476). Actual Part A (backend Rust) was never done. Phase 7 is now formally split into 7A and 7B below.

#### Phase 7A — Rust backend (split into 3 parts for context management) — Verified complete — 7A1/7A2/7A3 reconciled 2026-07-30

Phase 7A = builder brief items 1–5 from `docs/plans/20260718-final_rapidmlx_followups.md` §1850-1854.
Checkpoint: 774b611 (2026-07-21). 820 tests pass.

##### Phase 7A1 — Semantic catalog + config fields — Verified complete (Coordinator, 2026-07-30)

- **State:** Verified complete — Coordinator, 2026-07-30. The catalog reachability defect was discharged 2026-07-30;
  the two items carried on this row are both deliberate and documented, not outstanding work.
- **Commit:** original + catalog wiring 2026-07-30
- **`ValidationContext` fields are populated and unread, on purpose.** `capabilities` and `workload_scenario` are
  filled by the API caller and no rule consults them, because no rule in the catalog is currently capability- or
  workload-dependent. The type's own doc comment states this and says the fix is to decide whether such rules are
  warranted, not to invent rules so the fields are read. Note that unsupported settings are *not* one of those cases:
  `effective_policy` already downgrades them gracefully, and erroring in `validate` instead would contradict that
  design.
- **The catalog/API prefix-cache disagreement is a guarded exception.** The catalog models prefix caching as one
  `prefix_cache_policy`; the shipped API and every frontend consumer use the three raw config fields
  (`prefix_cache_enabled`, `retained_cache_mib`, `disk_checkpoint_interval`). Nothing under `static/` reads
  `prefix_cache_policy` at all. The pairing test in `rapid_mlx_runtime.rs` lists both sides as explicit exceptions, so
  it still fails on *new* drift while permitting this one. Reconciling the two models is Phase 7 UI work and must not
  be changed underneath `presets.js`, `setup-view.js`, `spawn-wizard.js`, and `vram-estimate.js`.

##### Phase 7A2 — Command builder + launch wiring — Verified complete

- **State:** Verified complete (2026-07-30, Coordinator). The July PASS was reinstated only
  after the defects below were fixed and the endpoint was exercised against the installed
  runtime; the original verdict is not what is being trusted here.
- **Files:** command.rs (26 setters, all Phase 7 flags in build()), `inference/launch.rs`
  (wire-through — the ledger previously said `launch.rs`, which reads as
  `rapid_mlx/launch.rs`; no such file exists), capabilities.rs (register flags)
- **Commit:** 774b611, reconciled in HEAD

**Reachability: clean.** `cargo check --bin llama-monitor` after touching command.rs emits no
dead-code warnings, and command.rs carries no `#[allow(dead_code)]`. All 26 setters are reached
from the running binary, so 7A2 does not have 7A1's defect.

**The config → argv mapping was written three times, and the copies had diverged.** The chain
was `config → (hand copy in inference/launch.rs) → adapter → (apply_phase7_adapter_config) →
builder` for a real launch, but `config → (apply_phase7_config) → builder` for the
command-preview endpoint. The launch-side copies were complete. The preview's was not, so the
command shown to the operator was not the command the supervisor would run:

- **Dropped, though configurable and applied at launch:** `--served-model-name`,
  `--reasoning-parser`, the hybrid switches, all eight sampling defaults
  (`--default-temperature`/`-top-p`/`-top-k`/`-min-p`/`-repetition-penalty`/
  `-presence-penalty`/`-frequency-penalty`/`--max-tokens`), and `--prefill-step-size`.
  The last is the worst of them: the launcher emits it on *every* launch, and it is the
  control that keeps long-context prefill under the Metal single-buffer ceiling.
- **Invented, though the launcher omits them:** `--log-level INFO` (launch skips the default),
  `--timeout 60` (launch omits when unset), and an empty `--api-key`.

Fixed by deleting the second mapping rather than syncing it. `build_launch_argv` and
`apply_phase7_adapter_config` are now the only adapter → argv path; the preview drives both
through `RapidMlxAdapter::for_settings_preview`, a settings-carrier adapter that never
launches anything. The launch.rs copy became `RapidMlxAdapter::apply_config`. There is no
longer a second list to drift.

**The endpoint was also broken against every real runtime — a live-only defect.** Capability
probing read `output.stderr`, but `rapid-mlx 0.11.1` writes all 27,066 bytes of
`serve --help` to stdout and nothing to stderr. `from_help("")` yields an empty capability
set, so *every* flag came back unsupported and the endpoint could not build a command at all
unless the caller passed a `capabilities` override. Every unit test passes such an override,
which is exactly why 817 green tests said nothing about it. It now uses the same
`compatibility::output_text(stdout, stderr)` helper as every other probe in the tree, and
treats an empty parse as a failed probe (fall back to `verified_baseline`) rather than as a
runtime that supports nothing.

**Evidence.** `command_preview_shows_the_settings_the_launcher_applies` and
`command_preview_does_not_invent_defaults_the_launcher_omits` pin both directions of the
argv defect. Live check against installed `rapid-mlx 0.11.1` with no capabilities override,
real model `nightmedia-27b-mxfp8-mlx`: argv contains `--served-model-name live-preview`,
`--reasoning-parser deepseek_r1`, `--prefill-step-size 2048`, `--default-temperature 0.7`,
`--max-tokens 4096`, and none of the three invented defaults. 1030 lib tests pass;
`cargo fmt --check` and `cargo clippy --all-targets` are silent on lib and bin.

**Noted, not changed:** `command.rs` clamps `prefill_step_size` to `1..=2048`. Correcting an
earlier note here that called this clamp too tight: `2048` is rapid-mlx's own native default,
`512` is llama-monitor's benchmarked text setting, and vision work advances at most
`1024 → 1536 → 2048`. The `4096` figure in the 2026-07-24 archive was a step *down* from a
crashing `32768`, not a target. The clamp covers every sanctioned use.

##### Phase 7A3 — API endpoint + preset migration — Verified complete

- **State:** Verified complete — Coordinator, 2026-07-30
- **Files:** rapid_mlx_runtime.rs (POST /api/rapid-mlx/command-preview with auth), presets/mod.rs (v3 migration), api/mod.rs (route)
- **Commit:** 774b611 + HEAD

**Reachability:** route registered at `rapid_mlx_runtime.rs:285`, mounted in `api/mod.rs:1034`,
auth filter at `:503-519`. Live check on a throwaway instance: request without a token returns
`401`, with a token returns argv. `migrate_preset` is called from `load_presets` and from the
save boundary, both reachable.

**Migration verified live, not just in unit tests.** A planted pre-Phase-7 v2 preset loads and
migrates to v3, preserving `prefix_cache_enabled=true` and `retained_cache_mib=8192`, and
materializing `prefill_step_size: 512`, `hybrid_mode: auto`, `disk_checkpoint_interval: 0`.

**Defect found and fixed — silent total preset loss.** `load_presets` deserialized the file as
one `Vec<ModelPreset>`, so a single unreadable entry failed the whole parse; the loader then
wrote `default_presets()` over the file it had just failed to read. Reproduced live: a file
with two good presets (one user-renamed) plus one malformed entry came back after restart as
the seed defaults only, with the renamed preset gone and nothing but a `[warn]` line. The v3
schema change makes an unreadable entry a realistic event, so this was reachable, not
theoretical.

Fixed by parsing entry by entry (`parse_presets`): a bad entry costs that one preset, is
reported by index and name, and the file is left byte-identical because a partial read
suppresses every write-back (migration save and GGUF backfill both). A file that is not JSON
at all is renamed to `presets.json.unreadable-<epoch>` rather than overwritten. Re-ran the
original scenario: the renamed preset survives and the on-disk file is unchanged. Three
regression tests added.

**Also corrected:** the v2→v3 comment in `migrate_preset` claimed existing presets "load with
None (safe degraded mode)". Serde defaults do make the migration marker-only, but the defaults
are llama-monitor's — a v2 preset moves from the runtime's `2048` to `512`. Intended policy,
but not a no-op, and the comment now says so.

#### Phase 7B — Shared Wizard/Editor UI, teaching, captures, tests (split into 4 parts)

Phase 7B = builder brief items 6–13. Each part requires screenshot validation before proceeding.

##### Phase 7B1 — Wire existing controls + Web UI/sampling/prompt storage — Verified complete (Coordinator, 2026-07-30)

- **State:** Verified complete — Coordinator, 2026-07-30. Four defects found and fixed (`a1f77fe`); one structural finding recorded below rather than fixed.
- **Commit:** 31af56b (2026-07-21), reconciled by `a1f77fe` (2026-07-30)
- **Screenshots:** spawn-wizard-rapid-mlx-advanced-controls.png, spawn-wizard-rapid-mlx-webui-group.png, rapid-mlx-preset-editor-advanced.png
- **Verification method:** live binary on a throwaway `HOME`, not the suite. Every defect below was invisible to 1040
  passing tests.
- **Defects found and fixed:**
  1. **PFlash shipped on.** `RapidMlxConfig::default()` names `Some("off")`, but the field carried a bare
     `#[serde(default)]`, and on an `Option` that yields `None` — only a *container*-level default routes a missing
     field through `Default::default()`. Every config arriving from disk or over the API therefore emitted no
     `--pflash`, and rapid-mlx 0.11.1 defaults it to `always` for the verified Qwen3.5/Qwen3.6 aliases, where the
     2026-07-24 verdict measured needle recall collapsing to 0–40% past 32768 tokens. Emitting nothing was not
     neutrality. Fixed with a named `default_pflash_policy`; the command builder no longer requires a `--pflash` flag
     in order to turn PFlash *off*, since a runtime lacking the flag has nothing to switch off and would otherwise
     fail to launch on the new default. Live argv now carries `--pflash off`.
  2. **Mutual exclusion fired on one participant.** `check_mutual_exclusions` used any-match, which makes a rule a ban
     on a single setting rather than an exclusion between several: `{"reasoning_mode":"on"}` alone was live-confirmed
     to report a conflict with a `sampling_mode` the caller never submitted. Participants are now value-aware
     (`ExclusionMatch::OneOf`, and `Present` for numeric settings that have no conflicting value to compare) and all
     must match. Re-verified live: reasoning alone is `valid: true`, the real pair still fails.
  3. **`--reasoning` divergence hidden on the surface built to show it.** The flag pins the KV cache to int8 inside the
     runtime whatever `--kv-cache-dtype` says. The VRAM estimator already modelled this, but `effective_policy` and
     `requested_vs_effective` echoed the *requested* dtype. Both now report `int4 → int8` with a reason.
  4. **Two 7B2-removal leftovers in the wizard.** The rapid_mlx spawn payload still carried a `workload_scenario` that
     serde silently dropped, and the Review summary looked the scenario up in the canonical estimator vocabulary
     rather than the one the use-case cards set, so it printed a raw snake_case key — including for the default
     selection, because the guard compared against a value nothing produces.
- **Structural finding, not fixed (feature work, not reconciliation):** the settings catalog has no frontend consumer.
  `/api/rapid-mlx/settings` exists but nothing under `static/` fetches it, and grep for `command-preview`,
  `requested_vs_effective`, and `effective_policy` across `static/js/` returns zero hits. The consequence is a
  violation of the "no visible no-op control" gate: the backend honestly reports `turboquant_mode: requested k8v4 →
  effective none`, while the wizard's Prompt storage selector and Review summary present K8V4 as in effect. Recorded
  here; the UI work to consume that metadata is scoped separately.
- **Coverage gap recorded:** 19 of 57 `RapidMlxConfig` fields have no UI presence at all under either snake_case or
  camelCase (`trust_remote_code_consent`, `auto_tool_choice`, `no_thinking`, `hybrid_cache_entries`, `pflash_policy`,
  `response_cache_policy`, `disk_checkpoint_policy`, `max_num_seqs`, `max_concurrent_requests`, `prefill_batch_size`,
  `completion_batch_size`, `batching_policy`, `speculative_policy` (deliberately omitted per brief item 2),
  `gpu_memory_utilization`, `endpoint_compatibility`, `request_safety_policy`, `default_frequency_penalty`,
  `parser_policy`, `security_policy`). Brief item 2 requires several of these end to end. Separately, `712c261`'s
  message claims it threaded `--gpu-memory-utilization` to a welcome-screen control; the diff touches backend files
  only and no such control exists.
- **Open, carried forward:** the settings catalog's `default_value()` for `PflashPolicy` still returns `"auto"`, now
  inconsistent with the config default of `"off"`.

##### Phase 7B2 — Workload profiles — Verified complete (Coordinator, 2026-07-30), rescoped

- **State:** Verified complete — Coordinator, 2026-07-30. This is the sub-phase whose broken confirmation flow
  triggered the mass UNVERIFIED flag on 2026-07-25, so it was re-checked against a running instance rather than
  re-read.
- **Commit:** 5d00ee0 (2026-07-21), rescoped by 712c261 / 71aa16e / 58cfa42, reconciled by `0a0a9a0` (2026-07-30)
- **Rescoped — the row above described a UI that no longer exists.** The dedicated step-3 picker with five profiles,
  editable assumptions, and a required confirmation checkbox was deleted as redundant with the page-1 "what are you
  running this for?" cards. The broken confirmation gate was removed, not repaired. The four workload screenshots
  document a deleted screen and are retained only as history.
- **What actually ships:** three page-1 use-case cards (`agentic`, `general`, `roleplay`) mapping through
  `USE_CASE_TO_PROFILE` to profile keys, then through `WorkloadScenario::from_profile_or_key` onto the canonical
  estimator enum. Workload scenario is spawn-time guidance for the VRAM estimate. It is not a launch setting and
  nothing about it is persisted.
- **Verified live end to end.** The card vocabulary reaches the estimator and materially moves the number — 27B mxfp8
  at 32768 context estimates 29.6 GB with no scenario, 44.4 GB `interactive_coding_agent`, 33.3 GB `general_chat`,
  38.5 GB `roleplay_storytelling`. All three `data-usecase` attribute values match the map's keys.
- **Defect found and fixed:** the picker removal stopped halfway. The preset editor kept its own Workload Scenario
  dropdown, which could never save anything — a preset stores a `RapidMlxConfig`, which has no `workload_scenario`
  field, so serde dropped the value. Reproduced live: saved a preset carrying `workload_scenario: "roleplay"`, read it
  back, the key was gone; the control would reopen at "(unset)" every time. It also spoke the canonical estimator
  vocabulary rather than the profile keys, and carried a `dataset.assumptions` read left over from the deleted
  editable-assumptions UI that nothing consumed. Removed, along with the same dead read in
  `rapidEstimatePolicyFromConfig`, which meant every non-wizard surface had been sending `workload_scenario: null`.
  Scope confirmed with the user 2026-07-30: workload scenario is initial guidance and does not warrant persistence.
- **Same run confirmed** the 7B1 PFlash fix reaches the preset path: a saved preset comes back with
  `pflash_policy: "off"`.
- **Recorded, not fixed:** `/api/vram-estimate` silently ignores an unrecognised `workload_scenario` and falls back to
  no scenario rather than rejecting it. Low stakes — the only producers are three fixed cards.

#### Phase 7.5 — Testing framework improvements — Verified complete — reconciled 2026-07-30

Phase 7.5 established CI-safe Playwright tests and minimal Rapid-MLX runtime testing.

##### Phase 7.5A — Playwright solidification — Verified complete — shipped, with one casualty (Coordinator, 2026-07-30)

- **State:** Verified complete — shipped, with one casualty (Coordinator, 2026-07-30)
- **Commit:** b16d50a (2026-07-21)
- **Work:** Tagged existing tests (@in-memory-test, @fake-data-bypass); added phase7-presets.spec.js, phase7-command-preview.spec.js, workload profile tests in spawn-wizard.spec.js
- **Tests:** 9 new Phase 7-specific tests (6 pass in CI, 3 runtime-gated)
- **Verification (Coordinator, 2026-07-30):** Both spec files exist and pass — `phase7-presets.spec.js` and `phase7-command-preview.spec.js` were run green in this campaign, having never been run since they were written. The `@in-memory-test`/`@fake-data-bypass` tagging is partial, not comprehensive: only `spawn-wizard.spec.js` (7) and `phase7-presets.spec.js` (3) carry `@in-memory-test`, and three files carry `@fake-data-bypass`, out of 15 spec files; `playwright.config.js` defines no tag-filtered project, so the tags document intent and select nothing. The "workload profile tests" this row claims were asserting a `workload_scenario` invariant that the 7B2 deletion had already made false; both were corrected in this campaign to assert the field stays out of the launch payload.

##### Phase 7.5B — Rapid-MLX runtime testing — Verified complete — scenario present, never run (Coordinator, 2026-07-30)

- **State:** Verified complete — scenario present, never run (Coordinator, 2026-07-30)
- **Commit:** 18397a7 (2026-07-21)
- **Work:** Added rapid-mlx-live capture scenario (seed preset → spawn → health → telemetry → chat → stop)
- **Screenshots:** rapid-mlx-live-dashboard-telemetry.png, rapid-mlx-live-chat-response.png, rapid-mlx-live-stopped.png
- **Note:** Developer-only (NOT CI); requires rapid-mlx on PATH + cached Qwen3-0.6B-4bit
- **Verification (Coordinator, 2026-07-30):** The scenario is real — `scenarioRapidMlxLive` runs preset → spawn → health (120s) → telemetry → chat → stop, with cleanup and a platform/PATH guard that skips rather than fails. It has not been run in this campaign and its three screenshots were not re-approved, so this row is verified as *present and coherent*, not as *exercised*.

##### Phase 7.5C — Harness hardening — Verified complete — table shipped, one capture lost (Coordinator, 2026-07-30)

- **State:** Verified complete — table shipped, one capture lost (Coordinator, 2026-07-30)
- **Commit:** f3d0153 (2026-07-21)
- **Work:** SCENARIO_REQUIREMENTS table documenting mock vs real per scenario; extended spawn-wizard-engines with tool-research and deterministic profile captures; mock note on dashboard-rapid-mlx
- **Verification (Coordinator, 2026-07-30):** The mock-vs-real table exists as the scenario header comment in `capture.mjs` (lines ~40-61) rather than a named `SCENARIO_REQUIREMENTS` constant, and it does document every scenario; the `dashboard-rapid-mlx` mock note is explicit both in the table and at runtime (`capture.mjs:2291`). The claimed tool-research and deterministic-profile captures in `spawn-wizard-engines` do not exist — no `tool_research` identifier survives in `capture.mjs`, another casualty of the 7B2 workload-picker deletion. The table itself is now stale for the removed workload scenarios.

##### Phase 7B3 — Roleplay-specific controls — Verified complete (Coordinator, 2026-07-30), feature removed

- **State:** Verified complete — Coordinator, 2026-07-30. The feature no longer exists; the row is corrected rather
  than re-verified.
- **Commit:** 91468fb (2026-07-21), removed by 712c261 / 58cfa42
- **What happened:** the roleplay teaching panel rendered inside the step-3 workload-profile picker. When 7B2 deleted
  that step, the panel became unreachable and `58cfa42` deleted it along with the rest of `WORKLOAD_PROFILES` (686
  lines out of `spawn-wizard.js`, 643 out of `spawn-wizard.css`, 424 out of `spawn-wizard.spec.js`). The three
  roleplay teaching tests went with it. `spawn-wizard-roleplay-teaching.png` documents a deleted screen.
- **Verified:** no `roleplay-teaching` identifier survives anywhere in `static/` or `tests/ui/`. The removal is
  complete, not partial.
- **Gap recorded, not rebuilt:** the teaching content itself (long-context reserve, client-owned samplers and stops,
  chat-vs-text formatting ownership, prompt-cache behaviour) is release-gating material under the teaching pillar and
  now has no home. Rebuilding it is feature work; see the gap register.

##### Phase 7B4 — Parallel slots/MTP teaching + endpoint compatibility — Verified complete (Coordinator, 2026-07-30), feature removed

- **State:** Verified complete — Coordinator, 2026-07-30. UI removed with 7B3; the backend behind it was checked and
  one defect fixed.
- **Commit:** 3b96564 (2026-07-21), removed by 712c261 / 58cfa42, backend reconciled by `f1e323e` (2026-07-30)
- **What happened:** both the four D25 MTP/concurrency cards and the per-workload endpoint-compatibility display lived
  in the profile step and were deleted with it. The five `@in-memory-test` tests (16–20) are gone. An orphan container
  `#pe-mtp-concurrency-teaching` remains in `static/index.html`, permanently `display:none` and never filled;
  `presets.js` carries the matching note "MTP/concurrency teaching panel removed (Phase 7B2)".
- **The backend survived intact and is live-verified.** `/api/vram-estimate` returns a full `mtp_admission` object —
  `eligible`, `recommended_for_workload`, `engages_for_workload`, `fallthroughs`, `warnings`, and the effective
  `concurrency_policy`. Confirmed against a running instance: an embedded-MTP request returns
  `fallthroughs: [non_greedy_sampling, logits_processor_installed]`, which is the 6.5a upstream capability gate
  restated per estimate. **No frontend code reads `mtp_admission`.** Same shape as the 7B1 finding: an honest backend
  and a silent UI.
- **Defect found and fixed:** `/api/vram-estimate` read `parallel_slots` from the request body, defaulted to 1 on
  absence, and never consulted the scenario, so a multi-slot workload was estimated and admitted as single-slot and
  the D25 `multi_slot_conflicts_with_single_stream_mtp` warning was unreachable from a scenario alone.
  `d25_multi_slot_conflicts_with_mtp` passed the whole time because it hands `compute` a slot count of 2 directly.
  Slots now follow the same explicit > scenario > default rule as the token counts. Live-verified: the warning fires
  for `tool_research_agent` and an explicit `parallel_slots: 1` still suppresses it.
- **Recorded:** no shipping UI path reaches a multi-slot scenario — all three use-case cards map to single-slot
  workloads — so the D25 teaching is unreachable twice over, once by deleted UI and once by unreachable input.


##### Phase 7B5 — GPU memory utilization welcome-screen control — Verified complete (Coordinator, 2026-08-03)

- **State:** Verified complete — found already fully done via other work, never marked here.
  (1) `scripts/rapid-mlx-benchmark-suite.mjs`'s `DEFAULT_UTILIZATION` is already `'0.90'`. (2)
  `--gpu-memory-utilization` is exposed as a real control in both the Spawn Wizard hardware step
  (`#spawn-rapid-gpu-memory-utilization`, `static/index.html:4038-4044`) and the Preset Editor
  (`#modal-rapid-gpu-memory-utilization`, `presets.js`), round-trips through `buildSpawnPayload`
  into `command.rs` (`--gpu-memory-utilization`), and is covered by
  `tests/ui/core/rapid-phase7-fields.spec.js` and `tests/ui/core/rapid-preset-throughput.spec.js`.
  Not literally on a "welcome screen" — it lives with the other advanced hardware/runtime tuning
  controls instead, which is the correct home for it; a first-run welcome screen is the wrong
  surface for a Metal-tuning flag. (3) No auto-recommendation copy claiming a system-memory-tier
  default exists anywhere in the UI (the dropdown hints — "leave room for other apps" / "balanced"
  / "dedicate the machine" — are generic and don't imply a 200GB threshold or any other automatic
  tier), so there was nothing misleading to correct.
- **Files:** scripts/rapid-mlx-benchmark-suite.mjs, static/index.html, static/js/features/presets.js,
  static/js/features/spawn-wizard.js, src/inference/rapid_mlx/command.rs.

**Phase 7 exit gate:** one fresh Verifier evaluates the complete Phase 7 diff after all of 7A and 7B are verified.

### Phase 8 — Hugging Face and Model Library

- **Budget:** 190k
- **Depends on:** Phases 2–5
- **Read:** gaps 3.2/3.7/3.8; accepted D9/D10/D29; A15/A17/A25/A29/A45–A46/A51–A57; Phase 8; existing sorting/creator/Community Picks/quant-swap code; HF/library and workload matrices.
- **Primary output:** Auto/GGUF/MLX/All plus preserved explicit sorting/category/curated-author discovery; revision-bound qualification; user-editable community-source roles; first-class heretic/uncensored and updated finetune/distillation paths; native/converted MLX lineage; local MLX introspection; context/KV/concurrency-driven artifact switching; canonical association; fit/template/tool/roleplay evidence; clear card hierarchy.
- **Completion proof:** search is not qualification; every mature GGUF discovery/quant/mmproj behavior has a regression gate; original author and converter stay distinct; community finetunes reach Rapid through qualified native MLX or conversion; repo/revision/variant survives end to end; context/KV changes recompute but never silently switch model quant; Recommended means workload fit; public search remains tokenless.
- **Mandatory Builder packets:** 8A (qualification/identity/lineage/fit APIs and fixtures) <=120k; then 8B split into 8B1 + 8B2 + 8B3 for context management. Each part returns its own handoff/checkpoint; one fresh Verifier evaluates the complete Phase 8 diff after all parts.

#### Phase 8A — Backend APIs and fixtures — Verified complete — reachable, largely unconsumed (Coordinator, 2026-07-30)

- **State:** Verified complete — reachable, largely unconsumed (Coordinator, 2026-07-30)
- **Commit:** 0fe6105 (2026-07-21)
- **Parts:** 8A1 + 8A2 + 8A3 (split for context management)
- **8A1 — CommunitySourceCatalog:** role-based catalog (OriginalAuthor, GgufQuantizer, MlxConverter, DatasetAuthor, Curator, MergerDistiller, Custom), user-editable, bundled creators (bartowski, mlx-community, DavidAU heretic/uncensored, etc.), KnownQuantizer migration, heretic/uncensored/updated-finetune preferences
- **8A2 — HF qualification/identity APIs:** POST /api/hf/qualify (revision-pinned, config/tokenizer/template/index evidence), POST /api/hf/identity (authorship/lineage with catalog role matching)
- **8A3 — MLX discovery/introspection:** POST /api/hf/mlx-derivatives (native MLX repos + conversion recipes), POST /api/models/mlx-introspect (recursive size, config, vision adapter), MLX local introspection in info_query.rs
- **Tests:** 825 pass, clippy clean, release build passes

#### Phase 8B — HF/Library discovery, cards, quant-switch UX (split into 3 parts)

##### Phase 8B1 — Discovery scopes, sorting, categories, curated authors, workload-start — Verified complete, defects since resolved by rework (re-verified 2026-08-07)

- **State:** Verified complete — the three 2026-07-30 defects were independently reworked and are now fixed, confirmed against current `HEAD`:
  1. **Sort mislabeling — fixed.** `b6d256b6 fix(hf): make the discovery sort dropdown actually sort, and remember the choice`. `resolveSortParam` (`static/js/features/hf-browse.js:405-419`) now only asks HF for a wide server-side pool (relevance/lastModified/downloads) and a separate `compareModels` client-side comparator (same file, ~line 401 onward, using `sizeRank`) does the real Name/Size ordering off `SimpleModelInfo.last_modified`/`model_size_bytes` — the old Name→createdAt/Size→downloads collapse no longer exists.
  2. **Workload-aware sorting/estimate — fixed by removal, not repair.** `f3e34644 fix(workload): delete the workload_profile ghost, and make the use case do one real thing`. `sessionState.workloadProfile` is gone; `static/js/features/models.js:3313-3320` documents in a comment why the Library estimate now intentionally omits `workload_scenario` (the Library has no use-case selection to derive it from — that only exists in the spawn wizard). This is a scope correction, not a live bug.
  3. **Curated authors — fixed, now backed by the real catalog.** `258e0175 feat(hf): serve the community source catalog, and back role badges with it` + `3b7668b5`. `KNOWN_CONVERTER_PATTERNS` is gone; `resolveAuthorRole()` (`static/js/features/hf-browse.js:137+`) resolves roles from the fetched `CommunitySourceCatalog` (`ensureCommunitySourceCatalog()`/`_fetchCommunitySources()`, same file) via `GET /api/hf/community-sources`, with catalog CRUD wired through `/api/hf/community-sources[/entry|/reset]` and consumed from `static/js/features/models.js:2396-2459`. The 8A note "`CommunitySourceCatalog` has no route at all" is also stale — it does now, and the frontend calls it.
- **Original commit:** f290273 (2026-07-22); rework commits above landed after the 2026-07-30 verification.
- **Scope:** builder brief items 1, 3, 8
- **Screenshot gates:** panels-model-library-discovery.png (Auto/GGUF/MLX/All toggle, workload-start discovery, curated authors)
- **Deliverables:** Additive MLX/GGUF/All discovery scopes (platform-defaulted, no separate `AUTO` value — additive toggling replaced the documented 4-way scheme; see `HF_SCOPE` at `hf-browse.js:11-16`), real Relevance/Name/Size/Last-updated sort, category badges, author/converter role badges sourced from the live catalog.

##### Phase 8B2 — Cards with lineage/qualification display — Verified complete, defects since resolved by rework (re-verified 2026-08-07)

- **State:** Verified complete — both 2026-07-30 defects fixed:
  1. **Lineage dead code — fixed.** `e5dba350 feat(models): record where downloaded models came from, and show it`. `ModelInventoryEntry` (`src/models/library.rs:71-105`) now has a real `download_provenance: Option<DownloadProvenanceView>` field; the old dead-field check in `models.js` was rewritten (comment at `models.js:415-421`: "row shipped in Phase 8B2 reading `hf_repo_id || originRepo || repo_id`. None of those are real fields the backend populates" — replaced with `m.download_provenance`). The nonexistent `hf_repo_id`/`originRepo`/etc. fields were never added to the struct; the code path that referenced them was replaced rather than the struct being extended to match it.
  2. **Revision-bound qualification — fixed.** `3b7668b5 feat(hf): harden identity/qualification and serve community source catalog`. `SimpleModelInfo` (`src/hf/mod.rs:275-304`) now carries `pub revision: String` (the immutable commit HF search returns). `/api/hf/qualify` is called from the frontend in two places: `static/js/features/hf-browse.js:323` (`openHfEvidence`, revision-pinned) and `static/js/features/spawn-wizard-hf-browse.js:567`.
- **Original commits:** 7011f5c + 0f0b575 (2026-07); rework commits above landed after the 2026-07-30 verification.
- **Scope:** builder brief items 4, 5, 6, 7, 11
- **Deliverables:** Two-level card hierarchy (group + variants), original author/converter distinct via live catalog, revision-bound qualification badges backed by a real `revision` field, real download-provenance lineage on library cards, model card panel wired in-app.

##### Phase 8B3 — Quant-switch UX, context/KV artifact switching, MLX fixes, scope UX fix, captures — Complete, all screenshot gates captured (2026-08-07)

- **State:** Complete. All screenshot gates captured against real HF data
  (`unsloth/Qwen3.6-27B-MTP-GGUF`): `panels-model-discovery.png`,
  `panels-model-discovery-qualification-badges.png`, `panels-model-discovery-quant-advisor.png`,
  `panels-model-discovery-context-requant.png`, `panels-model-discovery-mlx-only.png`
  (`docs/screenshots/artifacts/models/`).
  - **Real production bug found and fixed while capturing the quant-advisor/context-requant
    gates:** `fetchGpuVram()` (`models.js`) read `/metrics/gpu` exactly once, on Download-tab
    init, with no retry. On a freshly-spawned instance the Apple GPU backend's `mactop
    --headless` cold start takes several seconds (up to ~10s observed) before real metrics
    replace the initial empty `{}` response, so `cachedVram`/`cachedUnified` could permanently
    stay at their zero/false defaults for the rest of the session if a model was selected (and
    `loadQuantAdvisor()` ran) before that first poll resolved — silently hiding the quant advisor
    with no error. Not test-scenario-only: any real user opening the Download tab and picking a
    model within the first several seconds of a fresh server start hits the same bug. Fixed by
    making `fetchGpuVram(retriesLeft, background)` retry (30× at 1.2s, background/unawaited after
    the first attempt so it can't block other Download-tab init like wiring the search box), and
    re-triggering `triggerQuantAdvisor()` from the background retry's success path if a model is
    already selected by the time real data lands.
  Most of the formal scope turned out to already be shipped by earlier 8B1/8B2 rework, confirmed by
  direct code read against current `HEAD`; one genuine gap (mmproj backend gating) was found and
  fixed this pass. Not yet edited: `tests/ui/capture.mjs` coverage for the listed screenshot gates.
  - **Ad-hoc UX fix (2026-08-05, outside formal Builder brief scope, already landed):** user
    reported HF search-result rows required two clicks to select a GGUF quant (expand, then pick a
    file), and that the quant-advisor table rendered at the bottom of the left column instead of
    the right sidebar. Fixed: (1) `static/index.html`/`static/css/spawn-wizard.css` relocated
    `#quant-advisor` into `.wizard-sidebar` with a compact-column CSS override; (2)
    `static/js/features/hf-browse.js`'s `createGroupVariant()` now auto-selects the VRAM-recommended
    quant on first expand of a GGUF group variant; (3) root-caused and fixed the actual blocker —
    `hfListFiles()` (shared file-list renderer used by both the spawn wizard's `fetchHfFiles()` and
    the Models modal) had a deliberate "do NOT auto-select" no-op despite already computing
    `autoSelectFn`/`firstSelectFn`; now calls whichever is available.
  - **Additive scope-toggle UX — already shipped as part of the 8B1 rework**, not a remaining gap.
    `hfCreateScopeSelector()` (`static/js/features/hf-browse.js:1560+`) renders additive MLX/GGUF/All
    buttons (not single-value radio); platform default is set in `static/js/features/models.js:2662-2663`
    (`isMac` → MLX active by default) and `spawn-wizard-hf-browse.js:55+` (engine-driven default,
    documented as replacing the old platform-only default).
  - **Workload/context/KV-aware quant comparison — already real, not a generic score.**
    `POST /api/vram/quant-compare` (`src/web/api/vram.rs:628+`) branches on
    `Backend::RapidMlx`/`Backend::LlamaCpp` and consumes `use_case`, `parallel_slots`,
    `available_vram_bytes`, and real per-repo `available_files` when known — this is not the
    "generic Recommended from an 8k quality score" the brief warned against.
  - **MLX vs. GGUF file listing — already handled via a `format` parameter, not two endpoints.**
    Both formats go through `/api/hf/files` (`hfListFiles()`, `hf-browse.js:995+`) with an explicit
    `format: 'mlx'`/`'gguf'` field (e.g. `hf-browse.js:365,541`), so MLX repos are never queried
    with GGUF-shaped assumptions. The brief's literal "separate endpoint" framing didn't match, but
    the described failure mode (MLX treated as GGUF) does not occur.
  - **mmproj backend gating — two separate instances found, both genuinely missing, both fixed
    this pass.** mmproj is a llama.cpp/GGUF concept; Rapid-MLX vision uses a separate,
    currently-unqualified MLX-VLM component set (`[[project_rapidmlx_vision_off_the_table]]`), so
    neither of the below should ever be reachable for an MLX selection.
    1. **Spawn wizard** (`spawn-wizard-mmproj.js`'s `renderMmprojSection()`) had no backend check
       at all — the mmproj dropdown rendered under Rapid-MLX unconditionally. Fixed by gating on
       `wizardState.engine.selected === 'rapid_mlx'` and hiding the row for that backend.
    2. **Models modal Download tab** (`models.js`'s `detectMmprojCompanion()`) had the same gap
       (no `hfState.modelFormat === 'mlx'` check), plus a second, subtler bug once the format
       check was added: switching the additive scope toggle to MLX-only did not clear the prior
       GGUF selection's panels (mmproj/download/quant-advisor/VRAM), and `detectMmprojCompanion`'s
       own async `/api/hf/files` fetch re-validated `hfState.modelFormat` only at call-start, not
       after the `await` resolved — so a fetch started before the scope switch could still flip
       the section visible on resolution, after the toggle handler had already hidden it. Fixed
       by (a) clearing `hfState.selectedRepoId`/`modelFormat` and hiding all selection-scoped
       panels in the scope-toggle `onChange` handler, and (b) re-checking
       `hfState.modelFormat`/`hfState.selectedRepoId` a second time after the `await` in
       `detectMmprojCompanion` before showing anything. Verified via a live capture run
       (`model-discovery` scenario, `panels-model-discovery-mlx-only.png`) across two independent
       real-HF-data runs — mmproj/quant-advisor/download panels correctly stay hidden in MLX-only
       scope. `node -c` and eslint both clean on the changed file.
- **Budget:** 60k
- **Depends on:** 8B2 verified + screenshots approved
- **Scope:** builder brief items 9, 12, 13, 14 + scope UX fix
- **Work:** Quant comparison uses workload/backend/context/concurrency/memory topology; no generic Recommended from 8k quality score; preserve complete pre-download quant workflow; enumerate real GGUF files + MLX variants + conversion recipes; recompute on workload/context/KV/concurrency changes; llama.cpp mmproj backend-gated; Rapid hides mmproj, shows only actual MLX-VLM components; repair hidden gaps (MLX uses MLX files endpoint, not GGUF; Models-modal estimates not hard-coded llama/16k/q8; Rapid quant advice not llama math).
- **Scope UX fix (Phase 8B2 carryover):** Change HF_SCOPE from single-value radio to additive toggles (MLX+GGUF selectable together). Auto = platform default (macOS: MLX+GGUF, Win/Lin: GGUF). "All" button shows everything including NVFP4/unsupported. MLX tooltip: "Rapid-MLX native format. Faster on Apple Silicon. macOS only." On Windows, MLX-only models shown with macOS-only warning. Sort independent of scope.
- **Screenshot gates:** quant comparison with workload/context, MLX-only quant view, mmproj backend-gated, context/KV requantization, scope UX additive toggles (MLX+GGUF on macOS, GGUF on Windows, MLX-on-Windows warning)
- **Files:** `static/js/features/hf-browse.js`, `static/js/features/vram-estimate.js`, `static/js/features/models.js`, CSS, `tests/ui/capture.mjs`

**Phase 8 exit gate:** one fresh Verifier evaluates the complete Phase 8 diff after all of 8A and 8B are verified.

### Phase 9 — Formatting, endpoints, and revision-pinned template substitution

- **State:** 9a, 9-RapidMLX, 9b, 9c, 9d, 9e, and 9f verified complete (9f client-protocol qualification verified 2026-08-07). Remaining items are documentation/refinement notes only; no Phase 9 implementation packet is outstanding. The detailed 9f section below is authoritative: llama.cpp structured Chat, raw Text, and OpenAI legacy Text were live round-tripped with the qualified template and tool-call fixture; Rapid-MLX structured chat/tool behavior was covered by existing smoke tests, with raw-text behavior documented as shared OpenAI-compatibility inference.
  The execution companion was updated to reflect work already done but not recorded: 9a's revision
  pinning, retained history, and rollback shipped in spawn_wizard.rs without a ledger row. Phase
  9 is now split into ordered parts for context management: Part A (lifecycle core), Part R
  (Rapid-MLX overlay wiring), Part B (smoke-test gate), Part C (Gemma4 official template + provenance),
  then 9d–9f.
- **Design decision (Rapid-MLX template override, 2026-08-03):** Rapid-MLX has no CLI flag for
  external templates. Template loading in `vllm_mlx/utils/tokenizer.py::_apply_chat_template_sidecar()`
  checks only the model directory for `chat_template.jinja`/`chat_template.json`, falling back to
  the embedded tokenizer template. The chosen path is **overlay directory**: when a custom template
  is active for Rapid-MLX, create an overlay at
  `~/.config/llama-monitor/rapid-mlx/template-overlays/<model-hash>/` containing symlinks to all
  model files plus llama-monitor's chosen `chat_template.jinja` (copied, not symlinked). Pass the
  overlay path as the model argument to `rapid-mlx serve`. This preserves the read-only cache
  contract, supports the full lifecycle (version switching = change one file in overlay), and
  integrates with existing revision pinning/rollback machinery. The selection layer
  (`chatTemplatePath`/`chatTemplateMode`, `/api/chat-template/*`, frontend UIs) is already
  runtime-agnostic per `docs/reference/spawn-wizard.md` — adding Rapid-MLX support is a
  command-builder change. Future runtimes (MTPLX, etc.) follow the same pattern: overlay directory
  or runtime-native flag, wired through the same selection layer.
### Phase 9 Parts (ordered; each part has its own Builder/Verifier context)

#### Phase 9a — Revision-pinned install + retained history + rollback + Native — Verified complete (Coordinator, 2026-08-03)

- **State:** Verified complete — Coordinator, 2026-08-03. This work shipped but had no ledger row;
  reconciled by audit of `src/web/api/spawn_wizard.rs` and `static/js/features/spawn-wizard-chat-template.js`.
- **Commit:** present in HEAD (shipped prior to execution companion; reconciled 2026-08-03)
- **Files:** `src/web/api/spawn_wizard.rs` (revision pinning, release history, rollback endpoints),
  `static/js/features/spawn-wizard-chat-template.js` (History UI)
- **Scope:** Resolve and pin an exact commit SHA at install time (not `main`); keep every installed
  release per template name on disk instead of overwriting; UI to switch among retained releases.
  Closes the two highest-risk gaps: silent upstream drift and no way back.
- **Native:** already existed (`chatTemplateMode: 'embedded'` + `chatTemplatePath: null`) — no gap
  to close, confirmed during design.
- **Revision pinning:** `resolve_hf_commit_sha()` in `spawn_wizard.rs:131-149` resolves the exact
  commit SHA via `GET /api/models/{repo}`, then `install-hf` fetches from `raw/{sha}/{file}`
  instead of `raw/main/{file}`; falls back to unpinned `raw/main` if SHA resolution fails rather
  than failing the install. `revision: Option<String>` added to `ChatTemplateInstallMeta`.
  `install-url` has no git-revision concept — sha256 remains its immutability anchor.
- **Retained history:** every successful install (both endpoints) recorded into
  `~/.config/llama-monitor/chat-templates/releases/{name}.index.json` via `record_release()`,
  deduped by sha256, with standalone content copy per release so rollback survives later overwrites.
- **Rollback:** `POST /api/chat-template/activate` (`{name, sha256}`) copies a retained release's
  content back into the active `{name}.jinja` and re-writes its meta.json.
- **UI:** `GET /api/chat-template/releases?name=...` + History button/list in
  `spawn-wizard-chat-template.js`'s `installed` state, each non-active entry with an "Activate"
  button.
- **Evidence:** `cargo clippy -- -D warnings` clean, `cargo test --lib` 1084 passed,
  `npm run validate-js`/`npm run lint` clean, `cargo build --release` succeeds, `cargo fmt` clean.

#### Phase 9-RapidMLX — Rapid-MLX overlay wiring (prerequisite for 9b–9f on Rapid-MLX) — Verified complete (2026-08-03)

- **State:** Verified complete (2026-08-03). Design frozen and implemented.
- **Budget:** 60k
- **Depends on:** 9a verified complete
- **Read:** Phase 9 design decision above, `docs/reference/spawn-wizard.md` template lifecycle
  section, llama.cpp wiring in `src/inference/llama_cpp.rs` for `--chat-template-file`.
- **Primary output:** Rapid-MLX consumes `chat_template_file` via overlay directory approach.
  When a custom template is active for a Rapid-MLX launch, create an overlay directory at
  `~/.config/llama-monitor/rapid-mlx/template-overlays/<model-hash>/` with symlinks to all model
  files plus llama-monitor's chosen `chat_template.jinja` (copied, not symlinked). Pass overlay
  path as the model argument. `chat_template_file: None` = no overlay, use native model dir
  directly. The selection layer is already runtime-agnostic — this is command-builder wiring only.
- **Completion proof:** (1) selecting a chat template in wizard/preset editor applies to both
  llama.cpp and Rapid-MLX launches; (2) overlay dir created with correct symlinks + template file;
  (3) Rapid-MLX launch uses overlay path when template override active; (4) native template
  selection (`chatTemplateMode: 'embedded'`) uses real model dir, no overlay; (5) template version
  switching (via 9a's History/Activate API) takes effect on next Rapid-MLX launch; (6) no mutation
  of HF cache or user model directories; (7) `chat_template_file` field survives preset
  round-trip (save→load→save); (8) tests cover overlay creation, template resolution, and the
  "no overlay when native" path.
- **Implementation touches:** `src/inference/rapid_mlx/model_resolver.rs` (overlay creation),
  `src/inference/rapid_mlx/command.rs` (resolve through overlay when template active),
  `RapidMlxConfig` (new `chat_template_file` field mirroring llama.cpp), preset migration if
  needed for schema versioning.
- **Runtime-specific notes:** Rapid-MLX's `_apply_chat_template_sidecar()` prefers `chat_template.jinja`
  over embedded tokenizer template. The overlay path is accepted by Rapid-MLX's model argument
  (it supports local directory paths). Future runtimes follow the same pattern: overlay directory
  or runtime-native flag, wired through the same selection layer.
- **Files:** `src/inference/rapid_mlx/model_resolver.rs`, `src/inference/rapid_mlx/command.rs`,
  `src/inference/rapid_mlx/mod.rs` (config), `src/presets/mod.rs` (migration), `static/js/features/`
  (both surfaces already runtime-agnostic — verify they wire the field for Rapid-MLX)

#### Phase 9b — Tool-call smoke-test gate — Verified complete (Coordinator, 2026-08-03)

- **State:** Verified complete — Coordinator, 2026-08-03. Backend endpoint implemented and
  verified; frontend integration (activate-gate flow) not yet wired (left for 9b-frontend phase
  or Phase 13 surface convergence).
- **Commit:** present in HEAD (2026-08-03)
- **Budget:** 40k
- **Depends on:** 9a (API infrastructure), 9-RapidMLX (for Rapid-MLX qualification), existing
  `scripts/rapid-mlx-benchmark-suite.mjs` infrastructure.
- **Primary output:** Candidate template only becomes active after passing a ~1-2 minute
  tool-call fixture subset extracted from the existing benchmark suite. Failed test leaves
  selection unchanged (hard gate: failed-test-leaves-selection-unchanged).
- **Design:** Extract a short fixture subset from `scripts/rapid-mlx-benchmark-suite.mjs`
  (workspace-fixture-driven repeated tool-call rounds with scattered-marker fidelity checks).
  Wrap in a bounded launch: spin up a temporary server, run the subset, tear down, report pass/fail.
  New endpoint: `POST /api/chat-template/smoke-test {name, model_path}` or similar; the existing
  activate UI calls it before flipping the active selection.
- **Implementation:** `POST /api/chat-template/smoke-test` in `src/web/api/spawn_wizard.rs`:
  resolves template path, spawns temporary Rapid-MLX or llama.cpp server with candidate template
  applied (Rapid-MLX via overlay, llama.cpp via `--chat-template-file`), runs two tool-call
  fixture tests extracted from `scripts/rapid-mlx-benchmark-suite.mjs`: (1) single_tool_call:
  "Use read_file on src/example.ts" → verifies tool_call emitted with correct name and args;
  (2) sequential_tool_call: read_file → tool result → verify apply_patch expected. Returns
  structured `SmokeTestResult` with `ok`, `template_sha256`, per-test `pass`/`details`, and
  summary. Bounded timeouts: 45s server startup, 60s total test window, server killed on
  completion. Frontend activate-gate flow (UI calls smoke-test before activate, only activates
  on pass) not yet wired — that work belongs in Phase 13 surface convergence or a dedicated
  9b-frontend phase.
- **Completion proof:** (1) endpoint exists, registered, auth-required; (2) spawns server with
  candidate template; (3) runs fixture tests, returns structured result; (4) bounded timeout and
  cleanup verified; (5) `cargo clippy -- -D warnings` clean, `cargo test --lib` 1091 passed,
  `cargo fmt` clean, `cargo build --release` succeeds. Items (2)-(3) of original completion proof
  (UI integration) deferred to Phase 13.
- **Evidence:** 630 lines added to `src/web/api/spawn_wizard.rs`.

#### Phase 9c — Official Gemma4 template + provenance labeling — Verified complete (Coordinator, 2026-08-03)

- **State:** Verified complete.
- **Budget:** 20k
- **Depends on:** 9a (install/pin machinery)
- **Primary output:** `google/gemma-4-31B-it`'s `chat_template.jinja` added as a second,
  provenance-distinct Gemma4 candidate alongside existing jscott3201 community fork, labeled
  "Official" vs "Community". Reuses 9a install/pin machinery, no new plumbing.
- **Source:** https://huggingface.co/google/gemma-4-31B-it/blob/main/chat_template.jinja (the
  most actively updated Gemma4 model repo for this file).
- **Completion proof:** Both templates appear in the candidate list, each labeled with source
  provenance (official/community), both revision-pinned, both installable and activatable through
  existing UI.
- **Implementation details:** Restructured `chat-template-registry.js` to support multiple
  candidates per family via arrays. Added `provenance` field (official/community) and helper
  functions: `getTemplatesForFamily()`, `getDefaultTemplateForFamily()`, `getTemplateFamilies()`.
  Updated UI in `spawn-wizard-chat-template.js` force-family dropdown to show candidates grouped
  by family with provenance labels. Added `_templateDisplayName()` helper that appends provenance
  label when multiple candidates exist for a family. Preserved jscott3201 as default (proven
  tool-call variant) — Google's official template is second candidate. `presets.js` updated to
  use new helpers.
- **Evidence:** `chat-template-registry.js` gemma4 array contains both templates with distinct
  provenance values; `spawn-wizard-chat-template.js` force-family dropdown renders optgroups with
  provenance labels; linting and validation pass on all modified JS files.

#### Phase 9d — froggeric `-opencode-v3` transform script — Verified complete (Coordinator, 2026-08-03)

- **State:** Verified complete. Implementation includes transform script (`scripts/transform-froggeric-template.mjs`),
  backend wiring (`POST /api/chat-template/transform`), auto-transform on install-hf for froggeric repos,
  transform on rollback, and frontend integration. Self-validated against upstream v21.3 — byte-for-byte
  identical to user's reference `froggeric_qwen-chat_template-v21.3-opencode-v3.jinja`.
- **Budget:** 30k
- **Depends on:** 9a (install machinery), user-supplied reference files
- **Primary output:** Deterministic transform script that derives `<upstream-version>-opencode-v3`
  from any froggeric/Qwen-Fixed-Chat-Templates release. Anchored on Jinja control-flow markers,
  not blind line-number edits. Self-validated against known v21.3 before trusting on v21.4+.
- **Evidence:** User's hand-edited variant (removes JSON-handling blocks, ~227 of 328 lines
  touched, iterated against live Qwen3.6-27B) fixes the majority of tool-call looping.
- **Completion proof:** (1) transform script exists, runs deterministically, self-validation diff
  is clean (verified against reference at commit `e9568ff`); (2) derived variant installs as a normal
  candidate release via 9a machinery; (3) gated through 9b smoke-test like any other template.

#### Phase 9e — Passive HF discussion-activity signal + create-fix-from-discussion flow + lifecycle modal — Verified complete (Coordinator, 2026-08-03)

- **State:** Verified complete. Backend infrastructure, UI, lifecycle modal, and Discussions
  auto-route all implemented and screenshot-verified (2026-08-03).
- **Budget:** 20k infrastructure + refinements
- **Depends on:** existing install-hf machinery (9a), smoke-test endpoint (9b)
- **Primary output (complete):**
  - `GET /api/chat-template/discussions?name=<template-name>` polls HF discussions API for the
    template's source repo (resolved from release index, auto-routed by family if missing: qwen→froggeric/Qwen-Fixed-Chat-Templates, gemma-4→google/gemma-4-31B-it), returns discussion metadata (title, status, comment count, link)
  - `POST /api/chat-template/install-discussion` creates a discussion-derived release (separate
    from base template, with provenance link), stores as `<base>-discussion-<repo-slug>-<id>`
  - Lifecycle modal (`Manage` button replaces 8-button row): version info (SHA, source, version),
    history with rollback links, Discussions feed, Check updates, Create fix, Library, Upload
  - Create fix modal: auto-infers HF repo based on template family, pre-populates repo field
  - Fully gated: smoke test validates tool calls, failed test leaves active template unchanged
- **Completion proof:** Backend endpoints verified, lifecycle modal captured in screenshots
  (`preset-editor-lifecycle-modal-light.png`, `preset-editor-lifecycle-modal-dark.png`),
  Discussions feed verified with auto-route (`preset-editor-discussions-feed.png`),
  auto-infer repo tested live. Committed at `cf4e8ea`.
  1. **Modal theme:** Create fix modal should render light in light theme (currently dark opaque
     background in both themes). Background dimming is okay, but modal content must be readable.
  2. **Pre-populated editor:** Textarea must load current template content automatically — no blank
     paste required. Fetch from template file path stored in release index.
  3. **Inline discussion viewer:** Instead of just a link out to HF, render discussion thread
     content inline (title, posts, proposed changes) — similar to existing model card rendering
     from HF. User should be able to see proposed changes without leaving the app.
  4. **Upstream template as base:** For a Gemma4 preset, show Google's official `gemma-4-31B-it`
     chat_template.jinja as the editable base (latest commit), with dropdown to switch versions/commits.
     This lets user see the recommended upstream and compare against discussion-proposed changes.
  5. **Easy change extraction:** Highlight text in discussion → one-click copy to editor, or similar
     mechanism to reduce manual copy/paste friction.
  6. **Chat Template row redesign:** Input box currently truncated ("/pat"), buttons spill into
     2 rows. Give input its own full-width row. Organize buttons logically (primary actions vs
     secondary). Add more descriptions, hints, help text to guide user.
  7. **Rapid-MLX preset editor wiring:** Chat Template row is hidden in Rapid-MLX preset editor
     by CSS exclusion. Must add it to allowed fields in Model section. Same functionality applies:
     template overrides work via overlay directory (9-RapidMLX).
   8. **Wrap mode + line numbers alignment:** Create fix modal has "Wrap" checkbox that toggles
      pre-wrap vs pre whitespace. Current implementation measures visual line wraps via temporary
      span, but line numbers gutter doesn't reliably align with wrapped lines. Screenshot captures
      show wrap enabled (scrollHeight 5532px vs clientHeight 250px) but gutter rows don't match.
      Needs frontier model review of recalculateLineNumbers() measurement logic and gutter height
      sync. Escalate when frontend work resumes.
- **Not automated retrofixing:** HF discussions have no structured patch format; auto-applying
  discussion content to a template that controls tool-calling must stay human-reviewed.
- **Key repos:** `google/gemma-4-31B-it` discussions #140 and #137 (per user, 2026-08-03).
- **Completion proof (infrastructure):** Discussion feed visible in UI (spawn wizard + preset
  editor); create-fix flow tested via smoke-test endpoint; screenshot evidence captured.
- **Completion proof (refinements):** TBD — each refinement verified via screenshot capture.

#### Phase 9f — Client-protocol / SillyTavern qualification — Verified complete (llama.cpp; Coordinator, 2026-08-07)

- **State:** Verified complete for llama.cpp via live device round-trips (M5 Max, real
  `llama-server` binary, `Qwen3.5-9B-The-Defiant-Fable...Q4_K_M.gguf`, froggeric-qualified
  template applied via `--chat-template-file`, `--jinja`). Three client-facing protocol paths
  each tested with an actual request/response round-trip:
  1. **Structured Chat** (`POST /v1/chat/completions`, ST "Chat Completion" mode, OpenAI-style
     `messages[]`) — server applies the froggeric template; response correctly separates
     `reasoning_content` from `content`; verified with a plain prompt and separately with a
     `tools[]`/`tool_choice: auto` request — single clean `tool_calls` entry with correct name
     (`read_file`) and JSON args (`{"path":"src/example.ts"}`), no repetition-loop symptom
     (the exact §3.9 defect this template-qualification effort exists to fix).
  2. **Raw Text** (`POST /completion`, llama.cpp-native, ST "Text Completion" mode with a
     client-supplied pre-formatted prompt) — confirmed the server does NOT re-apply the chat
     template: a hand-formatted `### Instruction:/### Response:` prompt is completed verbatim
     with no template markers injected, matching what ST's own instruct-template formatting
     expects.
  3. **OpenAI legacy Text** (`POST /v1/completions`, prompt-string form some ST connection
     profiles use instead of `/completion`) — identical raw-passthrough behavior confirmed
     with the same hand-formatted prompt.
  - **Rapid-MLX:** not re-tested live this pass (would duplicate Phase 9-RapidMLX's overlay
    wiring verification and Phase 9b's smoke-test infrastructure, both already verified
    complete). Rapid-MLX's structured-Chat/tool-call path is already covered by 9b's
    `POST /api/chat-template/smoke-test` (spawns Rapid-MLX with the candidate template,
    runs the same tool-call fixtures used above). Rapid-MLX exposes only OpenAI-compatible
    routes (no llama.cpp-style raw `/completion`); grep of `src/inference/rapid_mlx/` confirms
    no separate raw-completion endpoint is implemented or assumed, so ST's raw Text mode against
    a Rapid-MLX backend would go through `/v1/completions`, which shares the same
    no-template-applied contract verified above for llama.cpp's OpenAI-compat surface (llama.cpp
    and Rapid-MLX both implement the same OpenAI spec for that route; this is a reasonable
    inference from the shared spec, not an independently observed Rapid-MLX round-trip).
- **Budget:** 30k
- **Depends on:** 9-RapidMLX (for full backend coverage) — satisfied
- **Primary output:** Qualify chat templates against external client protocols (SillyTavern raw
  Text vs structured Chat paths, OpenAI compatibility modes). Each path verified to preserve
  expected formatting and tool-call structure.
- **Completion proof:** llama.cpp — all three paths (`/v1/chat/completions`, `/completion`,
  `/v1/completions`) verified via live request round-trip against a froggeric-qualified template,
  including a tool-call fixture; documented above. Rapid-MLX — chat/tool-call path covered by
  existing 9b smoke-test infrastructure; raw-text path inferred from shared OpenAI-compat
  behavior rather than independently live-tested (acceptable given lowest-priority/deferrable
  status and no distinct Rapid-MLX raw-completion code path to diverge).

**Frontend duplication (applies to all Phase 9 UI work):** chat-template UI is hand-duplicated in
the Spawn Wizard (`spawn-wizard-chat-template.js`) and the Preset Editor (`presets.js` +
`static/index.html`'s `modal-chat-template-file` row) — every packet below that adds UI needs
changes landed in both places. No shared component, no automated parity check. See
`docs/reference/spawn-wizard.md`.
- **Budget:** 120k
- **Budget:** 120k
- **Depends on:** Phases 2–3; Phase 0 template-arg grep (item 9)
- **Read:** gap 3.9 (rewritten by E1); D11/D21/D22; A10/A27 (both resolved by E1)/A38–A40; Phase 9; template and external-client matrices; Rapid/MLX-LM/SillyTavern evidence.
- **Primary output (E1 — architecture is resolved, this is NOT a "native override investigation"):** ONE revision-pinned template-selection layer with two thin appliers — llama.cpp via `--chat-template`/`--chat-template-file`, Rapid via **file placement** into an llama-monitor-owned copy/overlay (never the canonical/HF-cache dir), or a template-path flag if Phase 0's grep found Rapid accepts one. The driving reason is the §3.9 **tool-call-reliability** defect (stock Qwen3.6/Gemma4 templates loop/fail on tool calls; Froggeric and the official Google Gemma template are the candidates to qualify). A tool-call smoke-test matrix gates activation; one retained `[escalate→device]` M5 Max check confirms the first real substitution loads and kills the observed loop. Preserve Froggeric SHA/update handling while adding immutable `TemplateRelease` records, alternatives, provenance-distinct official-Google/community candidates, comparison, stale/update state, bounded history, and rollback. Plus client-protocol qualification and SillyTavern raw Text / structured Chat paths.

- **2026-07-26 template-source decision:** the selection presented in both Spawn Wizard and Preset Editor is `(a) Native model template` by default, `(b) a qualified immutable `TemplateRelease`, or `(c) explicit local custom`. Native is not a copied library artifact: for an HF snapshot, the Rapid-owned overlay must re-link the original `chat_template.jinja`/tokenizer template source in that pinned snapshot; for a non-HF/local model, it must re-link the original model files. Selecting Native therefore undoes a prior override without mutating canonical/HF-cache files. Qwen3.5/Qwen3.6 selection offers the native entry plus version-specific Froggeric or other qualified releases, each identified by source repo, pinned revision, file path, content SHA-256, provenance, and qualification result. A release is installed alongside existing releases and activated explicitly; rollback reaches Native or any retained release. Never add app-side Jinja rendering, raw-prompt rewriting, or a proxy.
- **Later vision-compatibility follow-up (queued; does not block Phase 9 template work):** investigate the Qwen3.6 hybrid-VLM image failure as a layered, revision-pinned compatibility matrix, suitable for an upstream Rapid-MLX issue/PR when evidence identifies the owner. For the exact model conversion and image fixture, test: (1) direct `mlx-vlm` at Rapid's supported pin, (2) direct latest stable `mlx-vlm`, (3) Rapid at its supported vision dependency, and only if direct latest materially improves, (4) an isolated Rapid source build pinned to that newer `mlx-vlm`. Record converter/model revision, `mlx-vlm` version/commit, Rapid revision, effective MLLM/text lane, processor/template, first failure layer, full error/log, and image result. A direct-latest pass plus Rapid failure is Rapid MLLM routing/scheduler/cache integration evidence; a direct failure remains model/conversion or upstream `mlx-vlm` evidence. Never force-upgrade the normal Rapid environment or claim vision support until an actual OpenAI image request succeeds.

- **Later native-context extension follow-up (queued; does not block current loader work):** map native context ceilings from GGUF `*.context_length` and MLX `max_position_embeddings`/`model_max_length`, then qualify model-specific RoPE, YaRN, and related extension controls before exposing them. The work must cover effective launch arguments, tokenizer/template headroom, supported model families, KV/overhead math at each extended target, and an actual long-context benchmark matrix. Until then, 32K, 65K, 131K, 160K, 200K, and 262K are standard context choices only when at or below the introspected native ceiling; higher values are explicitly advanced-only and untested.
- **Completion proof:** the Rapid applier never mutates the canonical/HF-cache dir (only an owned copy/overlay, reversible/re-download-safe); the applier is labeled honestly per backend (no false parity); no Jinja renderer, shim/proxy, fork, or unreleased pin; a candidate becomes active only after passing the tool-call smoke test, and a failed test leaves the active selection unchanged; the M5 Max device check passes; Froggeric behavior does not regress; mutable upstream updates install alongside rather than overwrite the active release; official Google templates and community forks are provenance-distinct; rollback reaches the model-provided or any retained pinned release; no double template; SillyTavern owns raw instruct prompts; Rapid `/v1/completions` and llama `/completion` separately pass. There is no A27 "stop for approval" fork — the plan never depended on Rapid gaining native override. A heavier full tokenizer/config-replacement overlay still needs separate approval.

### Phase 10 — Spawn Wizard IA completion, Pro view, screenshot harness, and model-introspection-only sampling defaults

- **State:** In progress (reconciled 2026-08-07 against the 2026-08-06 spawn-wizard redesign, which shipped in `ec0f813b` but only delivered ~40% of its own scope; ledger updated 2026-08-07)
- **Budget:** 170k (reallocated across the five packets below; original single-IA framing is retired)
- **Depends on:** Phases 7–9
- **Read:** D7–D10/D16; A16/A28/A32–A33/A38/A50; Phase 10 (original); `docs/archive/rapid-mlx/20260806-spawn_wizard_uiux_redesign.md` (the shipped redesign's source doc — **required reading**, especially §4 "Option B — Pro" wireframe/scenarios, §6 full control inventory, §7 recommendation, §8 implementation notes); UI matrix and screenshot rules; `docs/reference/spawn-wizard.md`.
- **User directive locking this scope (2026-08-07, verbatim, non-negotiable):** finish Option A's "cheap-but-load-bearing" half; build Option B ("Pro"/"Advanced") in full — it was explicitly in scope for the 2026-08-06 rework and was dropped, which the user called out directly; fix the screenshot capture harness for the spawn wizard (correct per-shot filenames, no duplicates, no half-screen/broken captures); ensure llama.cpp/GGUF and Rapid-MLX feature parity wherever possible. A fifth item was discovered mid-investigation and is equally load-bearing: **eliminate all filename/name/repo-id substring-matching heuristics used anywhere in the codebase to infer model properties, replacing every one with real GGUF/MLX-config introspection.** User's exact words: "there should never ever be any filename checking at all for anything in our codebase around models" and "this should only be introspection."

#### 10a — Finish Option A ("Guided") — the cheap-but-load-bearing half

**Milestone 2 status: verified complete** (M2-A `57c8462e`, M2-B `3693af05`, M2-C `3ee360a1`). The `tier` field is retired across all CONTROLS (46 entries) and GROUPS (13 entries). Engine reads `critical`; Quick-profile disable uses `disableOnQuick`; Advanced badge uses `!group.critical`. Zero `.tier` references remain.

**Milestone 3 status: complete** (M3-A `43c16b30`, M3-B `3f5b45a7`). Sticky header includes inline VRAM bar with status badge (Comfortable/Tight/Over); KV dtype controls show "auto · [use-case]"/"you" provenance chips that flip on user interaction.

**Milestone 4 status: complete** (625f6c0). "All settings (23)" button with collapsible drawer for all non-card controls.

**Phase 10a status: COMPLETE.** All four milestones delivered: M2 (tier retirement + critical/view), M3 (sticky header + provenance), M4 (all-settings drawer). See commits `57c8462e`, `3693af05`, `3ee360a1`, `43c16b30`, `3f5b45a7`, `625f6c0`.

**Phase 10b status: COMPLETE (2026-08-09).** M5-A (`fdf6b954`, `ad8add9`): Pro toggle + left rail. M5-B (`32d3e16`): Pro renderer. M5-C (`a3b50302`): ⌘K quick filter (Cmd/Ctrl+K shortcut), "Modified only" toggle, "Reset all" button. Current release-built receipts verify the shared-state Pro surface for llama.cpp and Rapid-MLX.

The following bullets are the historical M5 scope checklist; all items are now closed by the archived Guided/Pro completion packet and the ledger rows below:
- Decision cards for the small number of choices that actually drive downstream config (per §3 wireframe — not every field, only the ones with real branching consequence).
- The "All settings (N)" collapsed drawer for everything else, so Guided stays uncluttered without silently hiding controls that exist.
- Sticky context bar + locked effective-value rows (shows the resolved value even when a field is left at "server default"/blank, so users aren't guessing what will actually be sent).
- Provenance chips on every effective value: is this the model's native default, a qualified `TemplateRelease`/sampling-catalog preset, or an explicit user override? (Ties into 10e below — provenance must originate from real introspection, never a filename guess.)
- ~~Retire the single `tier` field~~ **(done, Milestone 2)** — split into two independent registry axes in `spawn-wizard-groups.js` / `spawn-wizard-llama-ia.js` / `spawn-wizard-mlx-ia.js`:
  - `critical` — can this be safely touched without expert knowledge (drives whether it's editable inline vs. requires the drawer/Pro view).
  - `view` — which view(s) show this control at all (Guided vs. Pro vs. both).
  This is the prerequisite for 10b — Pro view can't cleanly reuse the same `CONTROLS` registry until this split exists. ~~(pending)~~ **(done)**.

**Execution approach (added 2026-08-07):** `tier` is not just registry data — it's wired directly into the disclosure engine (`spawn-wizard-ia.js`'s `createWizardIA()`, `isOpenForProfile()`, `applyTierVisibility()`), both IA files' `GROUPS` arrays, and the profile-switch handler in `spawn-wizard.js`. Retiring it correctly is a real engine refactor, not a data-only change, so 10a proceeds as four checkpointed milestones rather than one pass — each verified (build + a live capture) before starting the next:
1. **Registry split (done 2026-08-07)** — added `critical`/`view` to every `CONTROLS` entry in `spawn-wizard-groups.js`, additive alongside the still-present `tier` (not yet consumed by the engine). `critical: true` for old quick/balanced (safely editable without expert knowledge), `false` for old advanced. `view: 'card'` for the five controls backing the archived doc's §3 four always-open decision cards (`spawn-context-size`, `spawn-cache-type-k`, `spawn-cache-type-v`, `hw-mmproj-select`, `hw-use-mtp`, plus MLX's `spawn-rapid-reasoning-mode` for the "thinking" card), `'both'` for everything else — nothing is Guided-hidden per I1. Added a small `controlsForView(loader, view)` helper for Milestone 4's card/drawer wiring. Verified via `node -c` + a clean `cargo build --release`.
2. **Engine refactor — verified complete (M2-A/B/C, done 2026-08-07)**: see execution approach below this section.

    **Background (scoping note, 2026-08-07):** there are three distinct `tier`-consumers, not one, and they operate at different granularities:
   - `spawn-wizard-ia.js`'s `isOpenForProfile(groupTier, profile)` — keyed on **`GROUPS[].tier`** (group-level, one value per drawer section), decides a `<details>`'s default open/closed state. Group membership itself (which controls land in the drawer at all) is fixed by each group's hardcoded `controls: [...]` array, independent of any individual control's tier — e.g. `batching-threads` mixes `spawn-batch-size` (quick) and `spawn-n-cpu-moe` (advanced) in one group. So `critical`/`view`, defined per-`CONTROLS`-entry, don't map 1:1 onto this axis; **`GROUPS` needs its own `critical`/`view`-equivalent fields** (or the group's fields must be homogeneous enough to derive one), separate from Milestone 1's per-control split.
   - `spawn-wizard.js`'s `applyProfileVisibility()` (`static/js/features/spawn-wizard.js:1998-1999`) — keyed on **`CONTROLS[].tier === 'quick'`** directly (control-level), drives the Quick-profile disable-and-write-`quickValue` behavior (I2). This one maps cleanly onto Milestone 1's `critical` (quick was already folded into `critical:true`), so `control.tier !== 'quick'` → `!control.critical` is a safe, mechanical swap — but note `critical:true` is broader (includes old `balanced` too), so this line also needs a second check to isolate just the old-quick subset, or a dedicated `disableOnQuick` flag if quick's specific behavior must stay distinguishable from balanced's.
   - `wizardState.profile` (`quick`/`balanced`/`advanced`) is the wizard's existing three-way expertise selector — a different axis entirely from the Guided/Pro view toggle Milestone 4 introduces. Decide whether `view` should be derived from `wizardState.profile` (reuse the existing selector, no new UI toggle) or become fully independent (new toggle, doc's §7 recommendation) before writing Milestone 2 — this determines whether `isOpenForProfile`'s `profile` parameter is retired or just reinterpreted.

    **Execution approach (confirmed 2026-08-07):** Milestone 2 splits into three sequential Builder checkpoints. Each verified (build + live capture) before starting the next:

    - **M2-Step A (~30k) — Add GROUPS-level fields [DONE `57c8462e`]:** mechanically add `critical`/`view` to every `GROUPS` entry in both `spawn-wizard-llama-ia.js` and `spawn-wizard-mlx-ia.js`. Derive from existing `tier`/group composition. Zero engine changes.
    - **M2-Step B (~50k) — Swap engine reads [DONE `3693af05`]:** rewired `isOpenForProfile()` to boolean-first semantics (`critical:true` → open everywhere; `critical:false` → Advanced only). Updated `buildGroup()`, `relocatePrebuiltGroup()`, `applyTierVisibility()` to read/write `data-mlx-wiz-critical`. Removed dead `TIER_RANK`/`PROFILE_RANK`.
    - **M2-Step C (~10k) — Delete `tier` [DONE `3ee360a1`]:** removed `tier` from 46 CONTROLS + 13 GROUPS entries. Added `disableOnQuick` flag for Quick-profile disable behavior (spawn-gpu-layers, spawn-rapid-reasoning-mode). Badge check uses `!group.critical`. Removed dead `tierOf()` helper. Zero `.tier` references remain.

    Outcome: `tier` fully retired. Disclosure engine reads `critical`; Quick-profile disable uses `disableOnQuick`; Advanced badge uses `!group.critical`. Rollback plan preserved but not needed.
3. **Sticky context bar + provenance chips** — smaller UI addition; extends the existing `applyEffectiveLocks()`/chip pattern already shipped in `spawn-wizard-groups.js` rather than inventing a new mechanism.
4. **Decision cards + "All settings (N)" drawer** — the largest net-new UI piece (context size, KV precision, vision, speed boost cards; drawer wraps every `view:'pro'`-or-drawer-only control per the archived doc's disclosure table in §3).

#### 10b — Build Option B ("Pro"/"Advanced") — currently zero code, must be built from scratch

Per the archived doc §4 (wireframe, "nothing collapsed" principle, §4.3 "how it avoids being a wall of fields") and §7 (recommendation: ship both, Guided as default, with a `View: Guided ⇄ Pro` toggle):
- Left-rail settings navigator grouped by the same categories the `CONTROLS` registry already uses — no new taxonomy, reuse what 10a produces.
- ⌘K / Ctrl+K quick filter across all controls by label/key, jumping the rail + main pane to the match.
- "Modified only" toggle — hides every control still at its resolved default, surfacing only what the user has actually changed from either the model-native or qualified-preset value.
- Dense multi-column layout appropriate to viewport width (see §4 wireframe for the column-collapse breakpoints).
- Guided ⇄ Pro toggle, persisted per-session (not per-model), switching the same underlying `wizardState` and `CONTROLS` registry between the two renderers — no separate state model, no duplicated field bindings.
- Read §4's scenario walkthroughs and build against them directly; they are the acceptance criteria for "did this actually help a power user go fast."

#### 10c — Fix the spawn-wizard screenshot capture harness (done 2026-08-07)

Root cause: `static/css/spawn-wizard.css` lines 241–249 — `.wizard-body { flex:1; min-height:0; overflow-y:auto; overflow-x:hidden; }` sits inside the fixed-viewport `#spawn-wizard-overlay` modal (`position: fixed`, from the shared `.modal-overlay` base class in `components.css`). Puppeteer's `page.screenshot({ fullPage: true })` (the prior default in `captureShot()`) captures based on `document`/`body` scrollHeight, which a `position: fixed` element never contributes to no matter how tall its content grows — so `fullPage: true` only ever captured whatever was visible in the fixed modal viewport at the last-set scroll position. This was the concrete cause of the "half the screen" / seemingly-broken captures the user found, and of near-duplicate-looking screenshots (same crop dimensions, different scroll offsets).

First fix attempt (abandoned): flattening `.wizard-body`/`.spawn-wizard-modal`'s `overflow`/`height`/`max-height`/`flex` inline styles to force the full scrollable content into one tall layout box, then capturing via `elementHandle.screenshot({ captureBeyondViewport: true })`. This was root-caused correctly (confirmed via `getBoundingClientRect()` that the modal really did expand to e.g. 3800–5000px) but produced unusably large images — explicitly rejected by the user ("i dont ever want gigantic 5000px images... please keep things realistic for typical browsers").

Actual fix: `captureShot()`'s `expandSelector` option now scrolls the target container (`.wizard-body`) to its natural top position (`scrollTo({ top: 0 })`) and takes a normal, viewport-sized (`fullPage: false`) screenshot — i.e. capture exactly what a real browser shows at rest, not the full scrollable content flattened into one image. Applied to every wizard-step capture in `tests/ui/capture/scenarios/wizard-llamacpp/*.mjs` and `tests/ui/capture/scenarios/wizard-rapidmlx/*.mjs` (the latter left as dashboard/chat/settings captures that were never part of the fixed-modal problem).
- Along the way, discovered and fixed a real product bug this debugging surfaced: `spawn-wizard-vram-display.js`'s `renderLlamaCppScenarioCards()`/`renderMlxScenarioCards()` cleared `dom.vramScenarios` synchronously but appended cards only after an `await Promise.all(fetch...)` — two overlapping calls (e.g. two VRAM-update triggers firing close together) could each append their own set of context-fit cards, visibly doubling the "Reliable agents / More context / Full precision" grid in the live app, not just in screenshots. Fixed with a module-level render-generation token that makes a stale call's clear/append a no-op.
- Fixed a stale `--help` description: `tests/ui/capture/index.mjs` described `spawn-wizard` as "Steps 1–6" despite the Option A collapse to 3 steps; corrected to "3-step wizard: model & profiles, hardware & VRAM, launch summary/spawn". (In-code `// 6 steps → 3` comments elsewhere are accurate historical context about the collapse, not stale claims, and were left as-is.)
- Re-ran `spawn-wizard`, `spawn-wizard-engines`, `spawn-wizard-hf-download`, and `spawn-wizard-tier-matrix` against a fresh release build: all scenarios captured with zero errors, all screenshots at realistic dimensions (1440×900 / 1280×900 / 1280×1400 / 430×900 for narrow), visually spot-checked — no crops, no duplicated card grids, filenames match content.

#### 10d — llama.cpp/GGUF ⇄ Rapid-MLX feature parity

Audit the `CONTROLS` registry and both IA views (Guided + Pro) for controls that exist for one backend but have a real, meaningful equivalent for the other and are simply missing — not every llama.cpp flag has a Rapid-MLX analog and vice versa, so this is a parity audit against actual capability, not forced 1:1 mirroring. Close gaps found. Record backend-only controls explicitly (labeled, not silently hidden) rather than pretending both backends are identical.
- **Phase 7 carryover (folded in here):** Phase 7 exit gate found the Web UI group (Auto/On/Off, config JSON, static path) has a UI selector in the preset editor but not the spawn wizard — backend is wired, wizard frontend is missing it. Close this as part of the same parity pass.

#### 10e — Eliminate filename/name-based model-property heuristics; introspection-only, everywhere — VERIFIED COMPLETE (2026-08-09)

**Closure note:** The investigation bullets below are retained as historical routing context. Current active wizard behavior and final validation are governed by the archived Guided/Pro packet: metadata provenance is introspection-backed, unavailable/degraded states are explicit, the filename-derived MTP advisor fallback is removed, and the remaining `model_defaults.rs` compatibility path is isolated rather than an active wizard inference source.

Discovered while root-causing a live bug: a real Qwen3.6 community finetune, streamed (not yet downloaded) from HF, showed the generic `universal_modes()` sampling-preset fallback instead of the real Qwen3.6-family presets, with no error surfaced. Root cause: `static/js/features/spawn-wizard-hf-browse.js:1090` sets `wizardState.model.path = ''` for `delivery: 'stream_hf'`, so `tryIntrospectModel()`/`doIntrospect()` in `spawn-wizard.js` (which only run against a local disk path) never fire — `wizardState.arch.ggufArch`/`family` stay empty for the life of HF browsing, forcing `src/llama/sampling_catalog.rs`'s `modes_for_model()` detection cascade onto its filename/repo-name substring-matching fallback branch, which failed for this model's actual filename.

The fix is not "patch the Qwen3.6 branch" — per direct user mandate this is a whole-codebase constraint: **no model property (architecture, family, param count, MoE-ness, MTP-ness, sampling defaults, vision capability, etc.) may ever be inferred from a filename, repo name, or tag substring match. The only acceptable sources of truth are real GGUF header introspection (`read_gguf_metadata`/`read_gguf_metadata_from_bytes` in `src/llama/gguf_meta.rs` for local files, `fetch_gguf_header_metadata` in `src/hf/mod.rs:748` for not-yet-downloaded HF files via progressive range-fetch) or the MLX equivalent (`fetch_mlx_config`/`fetch_mlx_config_revision_aware` in `src/hf/mod.rs`) for MLX repos.** `fetch_gguf_header_metadata` already exists and is already proven working in `src/web/api/vram.rs` (VRAM-estimate HF branch) and `src/hf/qualify.rs` — it is not yet wired into the sampling-defaults path.

Known call sites requiring remediation (audit for more before closing this packet — this list is what was found during initial investigation, not guaranteed complete):
- `src/llama/sampling_catalog.rs::modes_for_model()` — filename/repo-name substring branch in the detection cascade; must only ever receive real `gguf_arch`/`arch_family`, never fall back to guessing from `name_or_repo`.
- `/api/model-defaults` (`src/web/api/benchmark.rs::api_model_defaults`) — currently only accepts a local `model_name_or_repo`; needs an HF-aware branch (mirroring `vram.rs`'s HF branch) that calls `fetch_gguf_header_metadata`/`fetch_mlx_config` when the model is HF-streamed and not yet downloaded, so real `gguf_arch`/`arch_family` reach `modes_for_model()` before it ever needs a fallback.
- `static/js/features/spawn-wizard.js` — `inferParamBFromName()`, `parseMoeSuffix()`, `detectMtpFromName()` — all filename-substring heuristics; must be replaced by consuming the real introspected fields (extend the HF-aware introspection above to return param count / MoE / MTP the same way local `doIntrospect()` already does for local files) or, if introspection is genuinely unavailable (range-fetch fails), the field must show as unknown/pending rather than guessing.
- `static/js/features/spawn-wizard-hf-browse.js:1090` (and the second `model.path = ''` at line 182) — the actual trigger: HF-streamed models must still get an introspection call (the new HF-aware backend path above), just not through the local-disk-only `doIntrospect()`.
- `src/web/api/vram.rs` — the existing fallback `ModelArch::from_name_and_params` (filename heuristic) used only when `fetch_gguf_header_metadata` genuinely fails; per the mandate this must become a "real value unavailable, don't guess" state rather than a silent filename-based guess, unless the user narrows the mandate to allow degraded-but-labeled estimates as a last resort — flag this specific case for explicit confirmation before removing it, since VRAM estimation (unlike sampling defaults) may have a legitimate need for a best-effort number rather than a blank field.
- `src/llama/model_defaults.rs` — has parallel family-detection logic to `sampling_catalog.rs`; relationship (duplicate/legacy/different consumer) is unresolved and must be established first, then treated the same way if it has its own filename heuristic.
- Any other site turned up during implementation (this is a whole-codebase mandate, not scoped to the files above).

**Completion proof (all five sub-packets):** 10a — `tier` fully retired in favor of `critical`/`view`; decision cards, drawer, sticky context bar, and provenance chips all present in Guided view; parity precedes reorder. 10b — Pro view exists, is reachable via a persisted Guided⇄Pro toggle, passes the archived doc's §4 scenario walkthroughs, and shares `wizardState`/`CONTROLS` with Guided (no duplicated state model). 10c — every wizard capture scenario re-run and visually confirmed free of crops/near-duplicates, every filename matches its content. 10d — Web UI group parity gap from Phase 7 closed; other genuine llama.cpp⇄Rapid-MLX control gaps closed or explicitly labeled backend-only. 10e — zero filename/name/repo-id substring matching remains in any of the enumerated call sites (or their replacement is explicitly user-approved as a labeled best-effort fallback, per the VRAM-estimator carve-out above); a real HF-streamed Qwen3.6 finetune shows correct family-specific sampling presets with no manual download required; `model_defaults.rs` vs `sampling_catalog.rs` relationship is documented and resolved.

### Phase 11 — Diagnostics, metrics, and storage

- **State:** In progress — remote-agent idle-gating packet implemented (2026-08-09); Phase 11 metrics/Doctor packet implemented (2026-08-10), storage and export packets remain.
- **Milestone (2026-08-09):** The Windows-installed remote agent now keeps GPU and system metric workers idle until an authenticated master request is received. Authenticated requests refresh a shared activity timestamp; the gate expires after 180 seconds without activity, with a bounded five-second idle check. Unauthenticated `/health` remains available and does not wake polling. This preserves remote reachability while avoiding unnecessary system polling and thread work when no master is connected. Unit coverage verifies inactive, active-through-timeout, expiry, and clock-skew-safe behavior.
- **Budget:** 170k
- **Depends on:** Phases 3 and 5–7
- **Read:** cache telemetry Sections 6.1/6.2/6.5; A9/A12/A23–A24/A31/A37/A41/A48; Phase 11; diagnostics/security/client matrices.
- **Primary output:** effective-policy and capability diagnostics; cache/queue/TTFT/context/MTP metrics; bounded privacy; disk-state visibility and approved cleanup only; and the **cross-backend Doctor** (E11) — grow the existing rapid-mlx-focused Doctor to cover llama.cpp too (drawing on the Phase 3 llama capability snapshot), as a release-gating teaching + troubleshooting pillar. Each check traces to a real failure mode (same defect→test rigor), gives condition + explanation + remediation + a "why this happens" teaching note, at dual reading levels (novice + power-user) from one detection engine reusing the `[decide-once]` educational copy. Ship the already-surfaced checks: KV < q8_0 for tool-enabled llama, tool-call-loop template mismatch, invalid `--tool-call-parser` argv, stale/incompatible rapid-mlx update.
- **Completion proof:** no content telemetry; no raw/stable fingerprint leaves ephemeral process memory; local aggregate-only shadow telemetry (if built) is absent from exports/backups/network paths; zero differs from absent; MTP activation/fallback visible; schema drift degrades safely; storage operations remain bounded/authenticated; every Doctor check is anchored to a real failure mode (no speculative checks), covers both backends where the failure applies, and renders both reading levels from one detection engine with concrete remediation text.

### Phase 12 — Security, dependencies, and watchlist

- **State:** Verified complete (2026-08-10) — authenticated routes, dependencies, probes, managed paths, remote-code consent, updater provenance, and watchlist policy audited; release validation remains in Phase 14.
- **Milestone (2026-08-10):** Authenticated route coverage passed (`cargo test --test auth_routing`: 39 tests) and path-focused library coverage passed (47 tests). `cargo tree -d` was reviewed; duplicate versions are transitive dependency families (Warp/Rustls/HF-Xet/crypto), not an unreviewed direct dependency addition. No dependency upgrade or security exception was introduced in this packet.
- **Milestone (2026-08-10, dependency refresh):** Integrated validated upstream updates: DOMPurify 3.4.13 (security), rusqlite 0.40.2, and clap 4.6.6. The npm audit also found and repaired the high-severity development-only `brace-expansion` advisory by locking 5.0.9; production and full npm audits now report zero vulnerabilities. Marked/puppeteer/Playwright updates remain deferred because they are non-security test/tooling churn and require a separate harness validation packet; release-please and the Rapid-MLX feature PR are not dependency updates.
- **Milestone (2026-08-10, main synchronization):** Fetched the current upstream `origin/main` (20 commits ahead at refresh), merged it into this branch, and resolved the dependency conflicts while preserving Rapid-MLX additions. The branch now contains `origin/main` as an ancestor; post-merge clippy, tests, release build, JS validation/lint, npm audit, and diff checks pass.
- **Milestone (2026-08-10, path review):** Closed a Phase 12 path-traversal gap in the authenticated discussion-template installer. Discussion-derived release names are now restricted to a bounded single filename component (no `..`, separators, or absolute paths), with regression tests for traversal and valid names. The authenticated `GET /api/chat-template/read` route is now constrained to canonical `.jinja` files below the managed `~/.config/llama-monitor/chat-templates` root; symlink escapes and arbitrary absolute-file reads are rejected with explicit invalid/outside/not-found responses and focused tests.
- **Milestone (2026-08-10, remote-code/probe audit):** Reviewed Rapid-MLX launch consent and updater provenance. `trust_remote_code` remains fail-closed and revision-scoped for the primary model and MTP companion; managed activation requires a validated managed release, immutable environment-relative executable, completion marker, SHA-256 match, and bounded on-device validation. Rapid-MLX and llama.cpp capability probes use `kill_on_drop`, bounded stdout/stderr, and short timeouts (22 compatibility tests, 2 timeout-bound tests, 32 updater tests pass). No new remote-code, executable-origin, probe-limit, or secret-logging gap was found.
- **Completion (2026-08-10):** Phase 12 hard gates are closed. Full validation passed after the security fix: 2,292 Rust tests (26 ignored), 39 auth-routing tests, compatibility/updater/path-focused tests, `cargo clippy -- -D warnings`, release build, JavaScript validation/lint, and `git diff --check`. Existing npm audit receipts remain zero-vulnerability; the sandbox could not refresh the registry audit because DNS access is unavailable. Phase 14 owns the final independent release gate.
- **Budget:** 120k
- **Depends on:** Phases 3 and 8–11
- **Read:** gaps 3.10/3.11; D12/D13/D17/D21/D26; A11–A14/A18/A24/A27/A34/A44; Phase 12; security matrix/evidence.
- **Primary output:** remote-code posture; upstream dependency-contract/resolved-receipt/rollback policy with evidence-based overrides only; source consent; probe/storage/template/Web-UI threat review; explicit waybarrios watchlist.
- **Completion proof:** no blanket remote-code warning/consent, path escape, unauthenticated route, secret leak, unbounded probe, or waybarrios flag leakage; ordinary data-only repositories remain low-friction while actual custom-code use is evidence-bearing and revision-scoped; llama MCP proxy remains loopback-scoped/explicit and no `--agent` shortcut bypasses per-tool review.

### Phase 13 — Convergence and documentation

- **State:** Not started
- **Budget:** 130k
- **Depends on:** Phases 5–12
- **Read:** all resolved decisions; Phase 13; completion ledger; surface/client matrices; reference-doc requirements.
- **Primary output:** one vocabulary across all surfaces; completed preset migrations against the D32 version/migration contract (not ad-hoc per-field defaults); complete user/reference/client/cache/troubleshooting documentation.
- **Completion proof:** no evidence grade is hidden; docs match code; all preset-shape migrations reference the single D32 contract and round-trip; OpenCode/Hermes/OpenClaw/SillyTavern setup is explicit; promoted screenshots are referenced.

### Phase 14 — Full release validation

- **State:** Not started
- **Budget:** 120k
- **Depends on:** Phases 1–13
- **Read:** Phase 14; all validation matrices; revalidation and completion ledgers; repository mandatory checks.
- **Primary output:** final independent validation evidence, closed traceability, clean intended worktree. This is the **one and only** release checkpoint (single cutover, no intermediate release, B3 resolved): dead/unwired code between phase gates was expected; the "releasable" check applies only here.
- **Completion proof:** mandatory checks in exact order; isolated full Playwright; sequential screenshots; security/platform review; representative E2E matrix; no P0 remains; the "releasable" check holds (no half-wired user-visible control, no partial read-path migration); the **dual-audience UX release bar** is met — novice safe-default/progressive-disclosure/educational-copy path AND power-user full-tweakability path both verified — and the cross-backend Doctor teaching pillar is present (release-gating, not cosmetic).

### Phase 14.5 — Rapid-MLX VLM/vision revisit (2026-08-07)

- **State:** Not started. Explicitly added so this is not lost among the Phase 10-14 backlog.
- **Depends on:** Phase 14 (or later, at user's discretion) — a deliberate last-in-list item, not a
  blocker for anything else.
- **Why this is separate from the vision work already in the plan:** `[[project_rapidmlx_vision_off_the_table]]`
  recorded Rapid-MLX vision as broken/deferred as of the last check, and 8B3's mmproj-gating fix
  (2026-08-07) hid the mmproj UI control for the Rapid-MLX backend on that basis — but that fix
  intentionally did NOT remove any vision-related code, detection logic, or plan/doc content
  (`[[project_vision_detection_decoupled_from_runtime]]`: detection is deliberately decoupled from
  runtime support so it stays ready for other MLX loaders). The user asked explicitly to revisit
  whether the Rapid-MLX vision runtime itself is now fixed and functional, specifically on
  Qwen3.6-27B and Qwen3.6-35B-A3B, before the followups plan is considered closed.
- **Work:** Re-test Rapid-MLX vision end-to-end on Qwen3.6-27B and Qwen3.6-35B-A3B (both were
  previously reported broken/unqualified — reconfirm against the currently installed rapid-mlx
  version, since this drifts release to release). If vision now works on either or both, remove
  the mmproj-gating restriction added in 8B3 for that backend/model combination (or replace it with
  a proper MLX-VLM component picker per the original 8B3 brief: "Rapid hides mmproj, shows only
  actual MLX-VLM components"), and update `[[project_rapidmlx_vision_off_the_table]]` and any other
  memory/docs that currently say vision is off the table. If still broken, record the current
  failure mode with fresh evidence and leave the gate in place.
- **Dependency/profile gate:** run the matrix only after confirming the managed
  runtime manifest records the requested `guided` + `vision` extras. Capture
  the exact Rapid-MLX version, `mlx-vlm` version, model/revision, and effective
  MLLM arguments. A missing extra must be reported separately from a hybrid
  `ArraysCache`/scheduler failure; do not treat either result as general model
  capability evidence.
- **Required matrix:** repeat the prior Qwen3.6-27B and Qwen3.6-35B-A3B image
  requests with the new profile, plus a known-good VLM positive control. Retain
  full server stderr and the HTTP/error-event outcome, including empty-200
  responses, before changing any Vision UI availability claim.

## 6. Decision Gate Router

The full decision text is comprehensive Section 8. This table tells the Coordinator when to stop.

| Before phase | Confirm these decision families |
|---:|---|
| 1 | Rapid remote code; unknown-model policy; unsupported platforms; hidden no-op controls; llama MCP proxy; interim unlimited-cache behavior |
| 2 | Request-default scope; legacy source migration; canonical workloads; sampler precedence; Coding default |
| 3 | Managed dependency authority; alias/finetune confidence; extras; capability-cache lifetime; MTP qualification |
| 4 | Companion ownership and evidence representation |
| 5 | Advisory planning-context scope versus actual runtime ceiling; active/retained totals; GPU utilization; TurboQuant; calibration; llama guaranteed/elastic context; MTP single-stream policy; quant-fit meaning |
| 6 | Hybrid byte budget; response-cache placement/trial; cache telemetry privacy; automatic recommendation authority; mixed-client policy |
| 7 | Advanced control placement; workload-profile UX; endpoint presentation; MTP/slot behavior |
| 8 | HF qualification/cache/credential behavior; unknown finetunes; library hierarchy |
| 9 | (Template architecture is resolved by E1 — no route/overlay decision to stop on; Rapid applies templates by file-placement, llama by flag.) SillyTavern Text/Chat behavior |
| 10 | Final preset categories and wizard order; whether shared IA reorganizes llama.cpp |
| 11 | Telemetry retention/hashing; disk ownership/cleanup; automatic tuning prohibition |
| 12 | Final remote-code/dependency/export/import/watchlist policies |
| 13 | All decisions recorded and reflected in docs/migrations |
| 14 | No unresolved decision or conditional hard gate |

If an answer changes a consequential design, present the best two approaches and a recommendation before proceeding.

### 6.1 Remaining authority gates

These are the only known consequential choices still open. They are deliberately deferred until the listed evidence exists; Phase 0 must not force premature answers. Everything else in Section 8 is accepted/frozen, measurement-pending under an accepted policy, or explicitly out of scope.

| Gate | Current safe baseline | Stop only when | Owning phase |
|---|---|---|---:|
| Foreground/background runtime architecture (Section 8.2 item 6) | One runtime, one active generation where policy requires it, queue rare overlap | Evidence justifies app-owned priority scheduling or a separate background runtime/port/credential lifecycle | 5/7/13 |
| llama-server built-in tools (A44/D26) | Web UI controls allowed; MCP proxy Off; built-in tools absent | A concrete allowlist/threat model/network design proposes enabling tools | 7/12/13 |

The former "Rapid template escalation (A10/A27/D11)" authority gate is **resolved by E1** and is no longer open: do not build native Rapid override or pause for approval. Rapid applies revision-pinned templates by file-placement into an llama-monitor-owned copy/overlay; llama uses its flags; the work is driven by the §3.9 tool-call-reliability defect and gated by a tool-call smoke test plus one M5 Max device check. Only a heavier full tokenizer/config-replacement overlay (beyond the sanctioned template-file copy) would need its own approval/threat model.

Implementation-time calibration, exact client protocols, prefix stability, package/model qualification, and source inspection are evidence gates—not invitations for a Builder to reopen product decisions. Unknown evidence resolves to the already documented conservative behavior.

Phase 10 screenshot review is a standard development acceptance gate, not an unresolved product-choice gate. A16/A28/A50 already freeze the seven-category Preset Editor and six-step intent-first Wizard direction. The Coordinator must present the real completed-control captures for ordinary UI/UX validation and adjustment, but must not reopen the accepted architecture or present competing approaches unless those screenshots reveal a genuinely consequential flaw.

Mechanical A-ID status ledger:

| Status | Decision IDs | Coordinator behavior |
|---|---|---|
| Accepted/frozen and ready | A1–A3, A7–A9 (A9 default inverted by E9: explicit trial ships; HMAC shadow observer deferred), A10 (resolved by E1), A11, A14, A16–A21, A23, A25, A27 (resolved by E1), A28, A30–A34, A36, A38–A40, A45, A47, A49–A51, A53, A55–A57 | Do not reopen unless new evidence contradicts the accepted policy |
| Accepted/frozen; implementation evidence or numeric calibration pending | A4–A6, A15, A22, A24, A26, A29, A35, A37, A41–A43, A46, A48, A52, A54, A58 (measurement-blocked A4/A5/A6/A22/A35/A41/A42/A48/A54/A58 form the §8.3 `[escalate→device]` M5 Max envelope) | Use the documented conservative/default behavior until the owning phase proves a stronger recommendation |
| Conditional user-authority gate | A44 | Only the specific expansion described in the authority-gate table is open; the recorded baseline remains frozen |
| Explicitly deferred/out of parity scope | A12–A13 | Preserve as a watchlist/deferred item; do not implement implicitly |

Section 8.2 item 6 is the additional non-A-ID foreground/background authority gate. Other Section 8.2 items are measurements under accepted policies.

## 7. Coordinator Brief Templates

### 7.1 Builder brief

```text
You are the Builder for Phase <N><Part>.

**CRITICAL: You do not have compression. DO NOT read the entire plan — it is ~2300 lines and you will blow context.**
Read ONLY the exact line ranges specified below.

File paths (use these EXACT paths):
- Authoritative plan: /Users/nick/SCRIPTS/CLAUDE/llama-monitor/docs/plans/20260718-final_rapidmlx_followups.md
- Execution companion: /Users/nick/SCRIPTS/CLAUDE/llama-monitor/docs/plans/20260718-final_rapidmlx_followups_execution.md
- Repository rules: /Users/nick/SCRIPTS/CLAUDE/llama-monitor/AGENTS.md

Read completely (exact ranges ONLY):
- AGENTS.md: /Users/nick/SCRIPTS/CLAUDE/llama-monitor/AGENTS.md (full file)
- Plan Section 9 (Coordinator/Builder/Verifier protocol): /Users/nick/SCRIPTS/CLAUDE/llama-monitor/docs/plans/20260718-final_rapidmlx_followups.md lines ~1422-1512
- Plan Phase <N> specific builder items ONLY: /Users/nick/SCRIPTS/CLAUDE/llama-monitor/docs/plans/20260718-final_rapidmlx_followups.md lines <start>-<end>
- Routed supporting sections ONLY: e.g. "/Users/nick/SCRIPTS/CLAUDE/llama-monitor/docs/plans/20260718-final_rapidmlx_followups.md lines 351-362 (gap 3.8), lines 627-638 (D13)"

Frozen state:
- branch:
- HEAD:
- dirty user changes to preserve:
- prior verified phase:
- resolved decisions:
- verified dependency phase commits:
- evidence snapshot dates/hashes:
- required user approvals:
- assigned packet (if Phase 5/7/8):

Allowed scope/files:
Objective/user outcome:
Ordered requirements:
Required external evidence revalidation:
Required tests/captures:
Hard gates:
Non-goals:
Stop/escalation conditions:
Context ceiling:

Do not commit, push, open a PR, make unresolved product decisions, or work on another phase.
Return the structured handoff required by comprehensive Section 9.3.
```

### 7.2 Verifier brief

```text
You are the fresh independent Verifier for Phase <N><Part>.

**CRITICAL: You do not have compression. DO NOT read the entire plan — ~2300 lines will blow context.**
Read ONLY exact line ranges specified below.

File paths (use these EXACT paths):
- Authoritative plan: /Users/nick/SCRIPTS/CLAUDE/llama-monitor/docs/plans/20260718-final_rapidmlx_followups.md
- Execution companion: /Users/nick/SCRIPTS/CLAUDE/llama-monitor/docs/plans/20260718-final_rapidmlx_followups_execution.md
- Repository rules: /Users/nick/SCRIPTS/CLAUDE/llama-monitor/AGENTS.md

Read completely (exact ranges ONLY):
- AGENTS.md: /Users/nick/SCRIPTS/CLAUDE/llama-monitor/AGENTS.md (full file)
- Plan Section 9 (Verifier protocol): /Users/nick/SCRIPTS/CLAUDE/llama-monitor/docs/plans/20260718-final_rapidmlx_followups.md lines ~1422-1512
- Plan Phase <N> hard gates: /Users/nick/SCRIPTS/CLAUDE/llama-monitor/docs/plans/20260718-final_rapidmlx_followups.md lines <start>-<end>
- Routed sections: /Users/nick/SCRIPTS/CLAUDE/llama-monitor/docs/plans/20260718-final_rapidmlx_followups.md lines <ranges>

Frozen pre-phase HEAD:
Builder handoff:
Actual diff/status:
Verified dependency phase commits:
Evidence snapshot dates/hashes:
Required user approvals:

Independently map every phase requirement and hard gate to code/test/runtime/screenshot evidence.
Check correctness, negative paths, migrations, security, API compatibility, cross-platform degradation,
UI/accessibility, documentation, test quality, and unrelated regressions.
Do not accept Builder claims without inspecting or rerunning proportionate evidence.
Do not redesign or commit.

Return PASS, PASS WITH EXPLICIT CONDITIONS, or FAIL using comprehensive Section 9.4. Every condition must include a stable ID, owner/phase, evidence required, expiry or revalidation trigger, and whether it blocks Phase 14.
```

### 7.3 Remediation brief

```text
Remediate only these independently verified Phase <N> failures:
- <stable finding ID, severity, requirement/hard gate, evidence>

Allowed files:
Required regression tests:
Hard gates to rerun:
Non-goals:
Context ceiling:

Return a focused Builder handoff. A fresh verification pass will follow.
```

## 8. Phase Checkpoint Ledger

Only the Coordinator updates this table after independent verification.

**Last updated:** 2026-08-09 by Coordinator. Phase 5a/5b implementation remains verified complete with runtime-calibration closeout tracked separately; Phase 6 is verified complete at its documented reduced scope. Phase 7–8 rows were rechecked against current `HEAD`; 8B3 screenshot and behavior gates are now closed. The archived Spawn Wizard Guided/Pro packet supplies current release-built evidence for Phase 10 and supersedes stale 2026-08-03 status labels. Broader Phases 11–14.5 remain open.

Phase 9 was reconciled 2026-08-03 by audit of `spawn_wizard.rs` and is now split into ordered parts: 9a, 9-RapidMLX, 9b, 9c, 9d, and 9e are verified complete. Phase 9f is verified complete for llama.cpp through live SillyTavern round-trips; Rapid-MLX structured chat/tool-call remains covered by 9b, while its raw-completion path remains an explicit lowest-priority limitation rather than an unqualified universal claim. The Rapid-MLX overlay approach remains the documented file-placement contract.

**Amended 2026-07-30.** Phase 6.5 had no ledger row at all, so its state lived only in a separate working-handoff doc and a coordinator reading this table alone would have seen phase 6 → 7 and missed it. Rows for 6.5a and 6.5b are added below. That handoff is retired: its live state and open items are folded into the Phase 6.5 section of `20260718-final_rapidmlx_followups.md`, its harness prerequisites into §12.3a of `docs/reference/rapid-mlx-mtp-evidence.md`, and the document itself is **deleted** — recoverable from git history at `396644b`. Nothing outside this repo's plan/evidence pair is current MTP state. It was briefly archived with a stale-claims banner; keeping a corrected copy of a document whose content had already been absorbed was redundant, so the copy went rather than the corrections.

| Phase | State | Builder handoff | Verifier verdict | Commit/checkpoint | Remaining condition |
|---:|---|---|---|---|---|
| 0 | Verified complete | handoff.md | PASS (0 gaps) | phase-0/ | None |
| 1 | Verified complete | handoff.md | PASS WITH NOTES (2 gaps→remediated) | phase-1/ | None |
| 2 | Verified complete | handoff.md | PASS (1 condition: fmt pre-existing) | phase-2/ | None |
| 3A | Verified complete | handoff.md | PASS WITH CONDITIONS (3, 1 blocks P14) | phase-3a/ | COND-P3A-T1 (finetune/alias test → Phase 8) |
| 3B | Verified complete | handoff.md | PASS WITH CONDITION (1, none block P14) | phase-3b/ | COND-P3B-R1 (CriticalFail UX → Phase 7) |
| 3C | Verified complete | handoff.md | PASS (condition C-P3C-RAPID-HASH remediated inline) | phase-3c/ | None |
| 4 | Verified complete | handoffs for A/B/C | PASS (all 3 parts) | ae42537 | None |
| 5a | Verified complete | handoffs for P1-P5 | PASS; cross-surface equality gate passed | `791635e` | Runtime calibration evidence is tracked separately below; it does not reopen estimator implementation |
| 5b | Verified complete | handoffs for A-C | PASS | `6a14cc7` | Fresh integration regression and evidence-tier closeout active |
| 6 | Verified complete — reduced scope (2026-08-05) | Coordinator audit | PASS for shipped CacheMode/effective-launch wiring; reduced-scope boundary recorded in Phase 6 section | `CacheMode` implementation and follow-up fixes through 2026-08-05 | Full workload-scenario Auto/refusal engine and broader recommendation telemetry remain intentionally unbuilt; do not present them as shipped |
| 6.5a | Closed — upstream limitation, not a local defect (confirmed by user 2026-08-07) | — | — | `ca3cffb`, `209f6bd`, `27243c7` (upstream-independent wiring only) | Rapid-MLX only supports MTP on greedy requests with no logits processor — useless for normal sampled/constrained-tool use, confirmed by the user as the current real-world state, not a stale finding. Re-verified against the repo 2026-08-07: `docs/reference/rapid-mlx-mtp-evidence.md` unchanged since 2026-07-30, requalification lane (`scripts/rapid-mlx-requalify-spec-decode.mjs`) still does not exit `0`. This is not fixable in this codebase; closed until upstream Rapid-MLX lifts the greedy-only restriction. Re-open by re-running the requalification lane against a newer rapid-mlx release |
| 6.5b | Closed — parked behind 6.5a, which is itself closed on upstream | — | — | Item 14 discharged early (`ca3cffb`) | Items 12, 13, 15 stay parked; they require a 6.5a PASS that cannot happen locally. Item 14 (refuse the corrupting in-trunk sidecar layout) remains taken and does not unpark the sub-phase. No further local action until upstream changes |
| 7A1 | Verified complete | Coordinator, 2026-07-30 | PASS — catalog reachability defect discharged 2026-07-30. Both carried items re-examined and found deliberate: `ValidationContext`'s populated-but-unread fields are documented as awaiting a decision on whether capability- or workload-dependent rules are warranted, and the catalog-vs-API prefix-cache disagreement is an explicitly listed exception in a drift-guard test that still catches new divergence. 1041 tests | catalog wiring 2026-07-30 | Nothing under `static/` fetches `/api/rapid-mlx/settings` — recorded under 7B1 |
| 7A2 | Verified complete | Coordinator, 2026-07-30 | PASS — reachability clean; two defects found and fixed: the preview kept a second config→argv mapping that dropped 12 flags and invented 3, and capability probing read the wrong stream so the endpoint failed against every real runtime. Live-checked against installed 0.11.1 with no capabilities override. 1030 tests | 774b611 + HEAD | `prefill_step_size` clamp keeps the documented 4096 value unreachable (recorded, deliberate) |
| 7A3 | Verified complete | Coordinator, 2026-07-30 | PASS — route/auth reachable, live 401 without token; v2→v3 migration verified live on a planted pre-Phase-7 preset. One defect found and fixed: a single unreadable preset failed the whole-file parse and the loader then wrote defaults over the file, silently destroying every other preset. Now parsed entry by entry; a partial read never writes back; an unparseable file is preserved rather than overwritten. Reproduced and re-verified live. 1033 tests | 774b611 + HEAD | Earlier 7A2 note about the `4096` clamp was wrong; corrected in place |
| 7B1 | Verified complete | Coordinator, 2026-07-30 | FAIL-with-fixes — four defects found live, all invisible to the suite. (1) `pflash_policy` never reached any deserialized config, so llama-monitor shipped rapid-mlx's `always` default on Qwen3.5/3.6 against a measured 0–40% recall collapse; (2) `check_mutual_exclusions` fired on any single participant, so `reasoning_mode=on` alone reported a conflict with an unsubmitted setting; (3) `effective_policy`/`requested_vs_effective` echoed the requested KV dtype instead of the int8 `--reasoning` pins; (4) two 7B2-removal leftovers in the wizard (dead `workload_scenario` in the spawn payload, Review row using a vocabulary the wizard never produces). All fixed and re-verified against a running binary. 1040 tests | 31af56b + `a1f77fe` | Frontend never calls `/api/rapid-mlx/command-preview` and never reads `requested_vs_effective` or `effective_policy`, so the Prompt storage selector is a visible no-op while the backend honestly reports `k8v4 → none`; 19 of 57 config fields have no UI at all (see section) |
| 7B2 | Verified complete — rescoped | Coordinator, 2026-07-30 | PASS with one defect fixed. The row's promised UI (5 profiles, editable assumptions, required confirmation checkbox) no longer exists: the step-3 picker was deleted as redundant with the page-1 use-case cards, and the broken confirmation gate — the trigger for the whole UNVERIFIED flag — was removed rather than repaired. The surviving path is live-verified: three use-case cards map onto the estimator and move the number (29.6 GB no scenario / 44.4 agentic / 33.3 general / 38.5 roleplay at 27B mxfp8, 32K). Defect: the preset editor kept a Workload Scenario dropdown that serde dropped on save, since `RapidMlxConfig` has no such field; reproduced live and removed. 1040 tests, eslint and validate-js clean | 5d00ee0 + `0a0a9a0` | `/api/vram-estimate` silently ignores an unknown scenario rather than rejecting it |
| 7B3 | Verified complete — feature removed | Coordinator, 2026-07-30 | Corrected, not re-verified. The roleplay teaching panel rendered inside the step-3 workload picker; deleting that step made it unreachable and `58cfa42` removed it with the rest of `WORKLOAD_PROFILES`, along with its three tests. No `roleplay-teaching` identifier survives in `static/` or `tests/ui/`; the removal is complete | 91468fb, removed by 712c261 / 58cfa42 | The teaching content has no home and is release-gating under the teaching pillar — see the gap register |
| 7B4 | Verified complete — feature removed, backend fixed | Coordinator, 2026-07-30 | UI removed with 7B3 (4 D25 cards, endpoint compatibility, 5 tests); orphan `#pe-mtp-concurrency-teaching` container left behind. Backend intact and live-verified: `/api/vram-estimate` returns a full `mtp_admission` with warnings and fallthroughs, and no frontend reads it. Defect fixed: `parallel_slots` defaulted to 1 and never consulted the scenario, putting the D25 multi-slot warning out of reach; the existing unit test passed because it supplies the slot count itself. 1041 tests, 29 UI tests, clippy clean | 3b96564, removed by 712c261 / 58cfa42, backend reconciled by `f1e323e` | `mtp_admission` has no UI consumer; no use-case card maps to a multi-slot scenario |
| 8A | Verified complete — fully consumed (re-verified 2026-08-07) | Coordinator, 2026-07-30; re-verified against `HEAD` 2026-08-07 | The 2026-07-30 "largely unconsumed" finding is fully stale — every endpoint it flagged now has a live consumer. `CommunitySourceCatalog`: `258e0175`/`3b7668b5` wired `/api/hf/community-sources[/entry\|/reset]` end to end; called from `static/js/features/models.js:2396-2459` and `hf-browse.js:83`. `/api/hf/qualify`: called from `hf-browse.js:323` and `spawn-wizard-hf-browse.js:567`. `/api/hf/identity`: called from `hf-browse.js:324`. `/api/models/mlx-introspect`: called from `models.js:492`, `spawn-wizard-chat-template.js:82,125`, and `spawn-wizard-hf-browse.js:561`. The 8A3 "MLX local introspection" claim also checks out: `resolve_mlx_recursive_size` and `read_mlx_local_config` are real functions in `src/inference/rapid_mlx/info_query.rs:603,621`, feeding the `mlx-introspect` handler (`src/web/api/models.rs:1501-1520`) — the ledger's literal function-name expectation didn't match, but the described capability exists and is wired. | 0fe6105 + rework in `258e0175`, `3b7668b5` | None outstanding |
| 8B1 | Verified complete — defects since resolved by rework (re-verified 2026-08-07) | Coordinator, 2026-07-30; re-verified against `HEAD` 2026-08-07 | All three 2026-07-30 defects are fixed by later commits, confirmed by direct file read against current `HEAD`, not by re-running the original Verifier: (1) sort — `b6d256b6` split HF-side pool fetch (`resolveSortParam`, `hf-browse.js:405-419`) from a real client-side `compareModels`/`sizeRank` comparator using `last_modified`/`model_size_bytes`; Name/Size now actually sort by name/size. (2) workload wiring — `f3e34644` deleted `sessionState.workloadProfile` outright rather than wiring it; `models.js:3313-3320` documents why the Library estimate intentionally has no workload input (that only exists in the spawn wizard). (3) curated authors — `258e0175`/`3b7668b5` replaced `KNOWN_CONVERTER_PATTERNS` with `resolveAuthorRole()` (`hf-browse.js:137+`) reading the live `CommunitySourceCatalog` via `ensureCommunitySourceCatalog()`. `HF_SCOPE` (`hf-browse.js:11-16`) has no `AUTO` value by design — additive MLX/GGUF/All toggling replaced the documented 4-way scheme, not a bug. | f290273; rework in `b6d256b6`, `f3e34644`, `258e0175`, `3b7668b5` | None outstanding from the original three; scope UX (additive toggles) already landed, so 8B3's "scope UX fix" dependency is satisfied |
| 8B2 | Verified complete — defects since resolved by rework (re-verified 2026-08-07) | Coordinator, 2026-07-30; re-verified against `HEAD` 2026-08-07 | Both 2026-07-30 defects are fixed: (1) lineage — `e5dba350` added a real `download_provenance: Option<DownloadProvenanceView>` field to `ModelInventoryEntry` (`src/models/library.rs:71-105`); the dead `hf_repo_id`/`originRepo`/etc. read in `models.js` was replaced with `m.download_provenance` (comment at `models.js:415-421` explicitly notes the old fields were never real). (2) revision — `3b7668b5` added `pub revision: String` to `SimpleModelInfo` (`src/hf/mod.rs:275-304`); `/api/hf/qualify` is called revision-pinned from `hf-browse.js:323` (`openHfEvidence`) and `spawn-wizard-hf-browse.js:567`. | 7011f5c+0f0b575; rework in `e5dba350`, `3b7668b5` | None outstanding from the original two; 8B3's "8B2 verified" dependency is satisfied |
| 8B3 | Verified complete (2026-08-07) | Coordinator re-verification against `HEAD` | PASS — quant-switch/context-KV UX, MLX-only and mmproj gating fixes, additive scope UX, and named screenshot gates captured | `docs/screenshots/artifacts/models/` gates; current `HEAD` | None for the 8B3 scope; retain the fresh screenshot/manifest evidence as the source of truth |
| 9a | Verified complete (2026-08-03) | Reconciled by Coordinator audit | PASS — revision pinning, retained history, rollback, History UI all present in HEAD; full gate passed (1084 tests, clippy/lint/build/fmt clean) | present in HEAD | Rapid-MLX wiring pending (9-RapidMLX part) |
| 9-RapidMLX | Verified complete (2026-08-03) | Coordinator | PASS — overlay wiring implemented, 1091 tests, clippy/lint/build/fmt clean | committed and pushed | None |
| 9b | Verified complete (2026-08-03) | Coordinator | PASS — smoke-test endpoint implemented, 630 lines, backend only; frontend activate-gate deferred to Phase 13 | committed and pushed | Frontend integration (Phase 13) |
| 9c | Verified complete (2026-08-03) | Coordinator | PASS — Google official template + provenance labeling; registry supports multiple candidates per family; UI shows provenance labels in force-family dropdown and installed status | committed and pushed | None |
| 9d | Verified complete (2026-08-03) | Coordinator | PASS — deterministic froggeric `-opencode-v3` transform, install/rollback wiring, and 9b smoke-test qualification | `e9568ff` and current `HEAD` | None |
| 9e | Verified complete (2026-08-03) | Coordinator | PASS — HF discussion activity feed, create-fix lifecycle, auto-routing, and UI evidence | Current `HEAD` and Phase 9e evidence | None |
| 9f | Verified complete for llama.cpp (2026-08-07) | Coordinator live device round-trips | PASS — SillyTavern Structured Chat, raw `/completion`, and legacy `/v1/completions` paths; tool-call fixture passed without repetition loop | M5 Max live `llama-server` round-trips documented in Phase 9f | Rapid-MLX structured chat/tool-call remains covered by 9b; raw-completion path was not independently live-tested because it has no distinct Rapid-MLX template path and remains lowest-priority follow-up |
| 10a | Verified complete (2026-08-09) | Coordinator; current Spawn Wizard completion packet | PASS — legacy tier axis retired, sticky context/provenance, decision cards, and complete-settings drawer all release-built and captured | `0e7df60`, `d52e12f`, `dbe1029`, and archived Guided/Pro evidence | None |
| 10b | Verified complete (2026-08-09) | Coordinator; current Spawn Wizard completion packet | PASS — persisted Guided⇄Pro toggle, shared canonical state, seven-category Pro surface, search/modified-only/reset, llama.cpp and Rapid-MLX panes | `b56c828`, `a1d3c93`, current release-built Pro receipts | None |
| 10c | Verified complete (2026-08-09) | Coordinator; current Spawn Wizard completion packet | PASS — fixed-viewport capture contract re-ran all wizard scenarios with strict manifest-backed receipts | `879af46`, `62be7de` evidence archive | None |
| 10d | Verified complete (2026-08-09) | Coordinator; current Spawn Wizard completion packet | PASS — llama.cpp/Rapid-MLX Guided and Pro parity audited; backend-only differences are explicitly labeled and unsupported controls remain unavailable | `a1d3c93`, `bde808b`, archived Guided/Pro evidence and launch receipts | None for the Spawn Wizard parity slice |
| 10e | Verified complete (2026-08-09) | Coordinator; metadata provenance cleanup and final validation | PASS — active wizard recommendations use introspection/effective metadata with truthful unavailable/degraded states; filename-derived MTP advisor fallback removed | `bde808b`, `879af46`, `62be7de` evidence archive | Legacy `src/llama/model_defaults.rs` compatibility path remains isolated/documented; do not treat it as active wizard inference |
| 11 | Not started | — | — | — | Phases 3, 5–7 |
| 12 | Not started | — | — | — | Phases 3, 8–11 |
| 13 | Not started | — | — | — | Phases 5–12 |
| 14 | Not started | — | — | — | Phases 1–13 |

### 8.1 Finding F-DEADCODE-RAPID — unwired Rapid-MLX capability surface (historical finding; reconciled 2026-08-09)

The original finding predates the Phase 6 reduced-scope closeout and the Phase 7–8 reconciliation. References below to “Phase 6 Not started” are historical discovery labels; the checkpoint ledger is authoritative for current state. Remaining unwired helpers still belong to their explicitly named later phases and keep Phase 14 open.

**Severity:** medium. **Owner:** Phase 7 (largest share), Phase 6, Phase 8. **Blocks Phase 14: yes**
(Phase 14 requires that Rapid estimates and controls agree with actual launch policy; code with no
caller cannot agree with anything).

Established by removing every `#[allow(dead_code)]` under `src/inference/rapid_mlx/` and letting
the compiler decide, rather than by reading the attributes. Result: **29 items are genuinely
unreachable from the running binary**, and the remaining suppressions were stale — the code they
covered had been wired up later and nobody removed the attribute, which is why the raw attribute
count overstated the problem. `src/lib.rs` re-exports these modules `pub`, so the lib target never
warns; `src/main.rs` declares `mod inference` privately, so only the **binary** target exposes the
gap. Anything reachable only from a unit test therefore looks alive in `cargo test` and is dead in
the shipped program.

| Group | Items | Status |
|---|---|---|
| `settings.rs` — the whole Rapid setting catalog (`RapidMlxSetting` + 7 methods, `all_settings`, `ValidationContext`, `EffectivePolicyExplanation`) | 12 | **Resolved 2026-07-30.** Wired via `GET /api/rapid-mlx/settings` and `POST /api/rapid-mlx/settings/validate`; 11 of 12 suppressions removed. Only `ValidationContext`'s unread fields remain, because `validate` ignores its context — see Phase 7A1 above |
| `mlx_meta.rs` — older metadata API (`MlxMetadata`, `read_mlx_config`, `read_mlx_metadata`, `metadata_from_config`, `finish_metadata`, `SafetensorsIndexInfo`, `parse_safetensors_index`, `infer_weight_components_from_safetensors`, `to_arch`, `MlxMetaEvidence`) | 10 | Superseded in practice by `read_mlx_model_profile` / `parse_mlx_config_to_profile`, which is what `/api/vram` calls. **Wire-or-delete decision open** |
| `info_query.rs` — `rapid-mlx models` listing (`fetch_model_list`, `parse_model_list`, `parse_list_line`, `ModelListEntry`, `Eligibility::is_eligible`/`is_ineligible`, `MODELS_TIMEOUT`) | 7 | Parsed and tested, never called; discovery reads the filesystem and the HF API instead. Phase 8 decides |
| `capabilities.rs` — cache guidance (`PrefixCacheGuidanceParams`, `supports_max_cache_blocks`, `CacheDiagnosticParams::snapshot`) | 3 | Phase 6 input; Phase 6 is Not started |
| `capabilities.rs` — `MtpConcurrencyState::label`, `CapabilitySnapshot::fingerprint` | 2 | `label` renders a state nothing acts on (see Phase 6.5 open items); `fingerprint` is snapshot identity for upgrade invalidation, no caller |
| `model_resolver.rs` — `empty`, `is_valid`, `validated_alias`; `mod.rs` — `model_source_view` method | 4 | Unused helpers; `presets.rs` assigns the `model_source_view` field directly and `command.rs` validates aliases through the full resolver |
| `updater.rs` (whole module) | — | Suppressed at the `pub mod` declaration; wiring belongs to Phase 12 |

Each surviving suppression now carries a comment naming the reason and the owning phase, so the
next reader does not have to re-derive this. **Do not bulk-delete this code**: the `settings.rs`
group is the Phase 7A1 deliverable that a previous Verifier marked PASS, and its unreachability is
the same class of defect as the Phase 7B2 workload-profile bug that caused the 2026-07-25
UNVERIFIED flag. Treat this table as evidence for the Phase 7/8 reconciliation, not as cleanup.

Allowed states:

- `Not started`
- `Builder active`
- `Awaiting verification`
- `Remediation active`
- `Blocked — <decision/evidence>`
- `Verified complete`

Never mark `Verified complete` from Builder completion alone.

State transitions are Coordinator-only and require durable evidence:

- `Not started -> Builder active`: dependencies are `Verified complete`, start HEAD/worktree preservation is recorded, decisions/evidence are current, and the Builder brief is stored.
- `Builder active -> Awaiting verification`: Builder handoff, actual diff, tests, and packet checkpoint are stored; Builder completion is not acceptance.
- `Awaiting verification -> Remediation active`: every failure has a stable finding ID and mapped requirement/hard gate.
- `Awaiting verification -> Verified complete`: fresh Verifier PASS, all conditions closed or explicitly non-blocking, Coordinator acceptance, and a phase checkpoint commit.
- Any state `-> Blocked`: exact authority/evidence gate, owner, and resume condition are recorded.

## 9. Per-Phase Working Record

Use the deterministic directory `docs/plans/handoffs/20260718-final-rapidmlx-followups/phase-<N>/`. The Coordinator creates and links these durable Markdown records from the checkpoint ledger:

- `coordinator-start.md` — branch/HEAD/worktree, preserved user changes, dependencies, decisions, evidence snapshot, approvals, scope, brief;
- `builder-<packet>.md` — structured Builder handoff, actual diff/checkpoint, commands and outcomes;
- `verifier-<iteration>.md` — independent mapping, rerun evidence, verdict, and conditions;
- `findings.md` — stable finding IDs with severity, requirement/hard gate, owner, status, and closure evidence;
- `remediation-<iteration>.md` — focused remediation handoff and regression proof;
- `evidence.md` — durable source hashes/URLs, runtime receipts, screenshots, and large-log locations.

Each record begins with:

```text
Phase:
Coordinator start date:
Start HEAD:
Upstream evidence date:
Decisions used:
Builder agent/run:
Builder result:
Verifier agent/run:
Verifier verdict:
Remediation iterations:
Commands/tests:
Screenshot artifacts:
Security/platform notes:
Final checkpoint/commit:
Open external limitations:
```

Do not paste full command logs into the plan. Store concise outcomes and paths/URLs to durable evidence.

PASS WITH EXPLICIT CONDITIONS is permitted only when no hard gate is violated. Record each condition in `findings.md` with its owner, required evidence, expiry/revalidation trigger, and `Blocks Phase 14: yes/no`. Phase 14 must close every blocking condition.

## 10. Completion Definition

The project is not complete merely because all phase rows contain commits. Completion requires:

- all phases independently verified;
- every comprehensive Section 15 row closed;
- all A1–A58 decisions resolved, measurement-pending under an accepted conservative policy, or explicitly deferred without violating a hard gate;
- upstream evidence refreshed at the final supported versions;
- no P0/P1 defect reproducible;
- all canonical workloads qualified on their supported routes;
- Rapid and llama estimates agree with their actual launch policies;
- mandatory repository checks and full isolated Playwright pass;
- final screenshots reviewed;
- security, authentication, path, privacy, storage, and platform gates pass;
- clean intended worktree and Coordinator sign-off.

If any condition is missing, record the exact blocker and owning phase. Do not replace evidence with confidence language.
