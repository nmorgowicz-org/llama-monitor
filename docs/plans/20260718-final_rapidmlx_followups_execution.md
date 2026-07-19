# Rapid-MLX Final Follow-ups: Execution Companion

| Field | Value |
|---|---|
| Created | 2026-07-18 |
| Purpose | Low-context phase router and checkpoint ledger |
| Authoritative specification | [`20260718-final_rapidmlx_followups.md`](./20260718-final_rapidmlx_followups.md) |
| Intended reader | A context-free Coordinator agent |
| Execution model | Coordinator → Builder → fresh Verifier → focused remediation |
| Maximum phase context | 200k; stop and checkpoint before compaction |
| Product implementation status | Not started |

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
8. Brief one bounded Builder with the exact comprehensive-plan phase.
9. After the Builder handoff, inspect the actual diff and brief a fresh Verifier.
10. Update this ledger only after independent verification.

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

### 4.1 Context boundaries

- One phase per Builder context.
- One phase per Verifier context.
- Do not combine phases because an earlier phase was short.
- Stop at a compilable checkpoint before approaching 200k context.
- Spawn a fresh continuation agent with a structured handoff when a phase must be split.
- The final Verifier checks the complete phase, including work from continuation agents.

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

- **State:** Not started
- **Budget:** 140k
- **Depends on:** Phases 1–2
- **Read:** gaps 3.4/3.8–3.10; D13/D24/D25/D27; contract 7.5; A2/A14/A15/A17–A19/A26/A29/A48/A51–A52; Phase 3; runtime/client matrices; evidence ledger.
- **Primary output:** automatically generated exact-executable capability snapshots for Rapid and llama; upstream dependency-contract and resolved-receipt handling; a first-class **on-device, user-driven update-validation probe** `[escalate→device]` (modeled on the existing thin llama.cpp beta-update validation) — the only qualification the Phase 3 gate depends on; dependency/extras states; endpoint matrix; alias/finetune confidence; MTP concurrency qualification; per-field Rapid sampling-default CLI/cascade coverage. Any Nick-owned upstream-monitoring CI/manifest is **additive/optional** and must not gate Phase 3 (E6).
- **Completion proof:** no manual per-release certification treadmill; drift is handled by the on-device probe (near-daily rapid-mlx/dependency updates validated on the user's device, independent of llama-monitor releases), and the absence of any upstream CI never blocks this or a dependent phase; an unseen environment satisfying upstream constraints and passing the on-device baseline receives no global disclaimer; only concrete failures or indeterminate selected Advanced capabilities produce actionable per-feature notices; probes are bounded; managed installs retain a resolved receipt and rollback; Rapid MTP fallback and llama MTP build/model distinctions are represented.

### Phase 4 — Normalized MLX architecture metadata

- **State:** Not started
- **Budget:** 170k
- **Depends on:** Phase 0 fixtures and Phase 2 identity
- **Read:** gaps 3.5/3.7; accepted D1/D2; contract 7.2; A25/A53; Phase 4; architecture matrix and HF config evidence.
- **Primary output:** GGUF/MLX adapters into one evidence-bearing normalized geometry profile; full/local/linear/recurrent layer groups; nested config parsing; MoE/MTP/companions/context evidence; correct size math; no shared runtime math.
- **Completion proof:** six real pinned family fixtures with independent expected facts; Qwen3.6 and Gemma4 KV/recurrent geometry correct; degraded evidence is field-specific.

### Phase 5 — Execution policies and estimator

- **State:** Not started
- **Budget:** 190k
- **Depends on:** Phases 3–4
- **Read:** gaps 3.5–3.7/3.11; accepted D1–D4, D18–D25, D28, and D30–D31; contracts 7.2–7.4; A1/A3–A5/A21–A22/A42–A43/A46–A48/A53–A54/A58; existing RTX 5090/M5 Max calibration evidence; Phase 5; memory/llama/client matrices.
- **Primary output:** Rapid-native policy and estimator; corrected llama unified/partitioned context contract; active versus retained memory; typed Auto/Standard/K8V4/V-only retained-prefix policy with exact alias eligibility; one backend-owned live unified-memory snapshot with safe-now/reclaim/app-close/configured-cap scenarios; workload-fit quant guidance; explicit llama MTP single-stream mode; explicit Rapid MTP companion ownership plus memory-first one-active Auto and fully re-estimated Advanced overlap.
- **Completion proof:** all surfaces consume the same timestamped snapshot and agree; Rapid cannot inherit stale llama/HF memory caches; no total RAM or wired cap is mislabeled available; TurboQuant savings apply only to qualified retained conventional-KV portions, Standard is not mislabeled FP16, unknown finetunes do not inherit alias eligibility, and transient decompression peaks remain visible; recovery actions distinguish allocator cache, reusable state, runtime/app memory, and OS disk cache; recovery is conservative and measured before/after; process diagnostics redact commands and use honest footprint/RSS/backend labels; sysctl mutation is bounded/reversible/exactly verified/restart-aware; the user's verified reboot-persistent M5 Max path is preserved and its mechanism/version evidence recorded without untested cross-version generalization; Qualified/Calculated/Provisional and uncertainty boundaries are honest; existing 5090/M5 calibration does not regress; raw Rapid measurements reproduce; every embedded/external MTP companion and cache reservation is additive; Rapid single-active Auto protects near-capacity quant/context fit while overlap refits worst-admitted memory and context guarantees; llama command and estimator describe the same KV pool; no Rapid `ctk/ctv`; recommended quant satisfies workload policy.
- **Formal sub-phases (E5, comprehensive §Phase 5):** Phase 5 is two formal sub-phases, each with its **own hard gate and its own fresh Verifier pass** — not one Verifier over two packets. **5a** = execution policy + `MemoryBreakdown` + estimator core + cross-surface estimate equality (comprehensive Builder items 1–14), <=120k; it must reach `Verified complete` before 5b starts. **5b** = `MemoryAvailabilitySnapshot` + reclaim + wired-limit + acquisition-gap repairs (items 15–18), <=120k. Rationale: coherence-per-packet for the local model (not token fit), compounding with the §4.6 gate taxonomy. Track 5a and 5b as distinct checkpoint rows.

### Phase 6 — Cross-backend cache guidance

- **State:** Not started
- **Budget:** 170k
- **Depends on:** Phase 5
- **Read:** comprehensive Section 6 in full; D14/D15/D17–D20; A6–A9/A21/A23/A31–A37/A41; Phase 6; cache/client matrices; cache evidence ledger rows.
- **Primary output:** shared Reusable prompt state Auto/Off/Advanced Custom with backend-native effective behavior; Rapid hybrid and expert-only response-cache policies; bounded llama prompt-cache policy; educational workload profiles; recommendation/refusal logic. Cache-repeat observation ships as an **explicit opt-in trial by default** (E9); the ephemeral per-runtime HMAC-fingerprint shadow observer is **DEFERRED/optional**, since the cache is Off by default and a privacy-sensitive fingerprinting subsystem is not justified before demand is proven.
- **Completion proof:** response cache Off for normal agents/roleplay; Rapid Auto uses the smallest memory-safe working set for the dominant single-user loop, does not permanently provision for brief cron overlap, and resolves Off when ineligible/unbounded; llama unified-memory Auto defaults extra host states to `0` while ordinary common-prefix reuse remains active, and only confirmed evidence-backed surplus permits a bounded positive cap; no generic concurrency value becomes cache size; the default path is the explicit trial with no fingerprinting subsystem built; IF the shadow observer is later built, its fingerprints use a random per-runtime HMAC key, remain memory-only/TTL- and size-bounded, emit aggregates only, and cannot reach persistence/log/export/backup/network surfaces; telemetry never mutates or restarts automatically.

### Phase 7 — Critical settings and shared UI

- **State:** Not started
- **Budget:** 190k
- **Depends on:** Phases 2–3 and 5–6
- **Read:** capability priority Section 4; D6–D8/D16/D21–D26/D31; contracts 7.3/7.5; A5/A44/A49 and other relevant A decisions; Phase 7; control/UI/client matrices.
- **Primary output:** backend-owned semantic setting catalog with frontend-owned teaching/layout/custom interactions; shared wizard/editor controls; truthful capability states; workload profiles; endpoint compatibility; command preview; explicit MTP/concurrency behavior; accepted tiered Reusable prompt storage UI and diagnostics; Advanced llama-server Web UI availability/config/static-path group with proxy/tools security separation.
- **Completion proof:** every control completes the trace; wizard/editor parity; hidden backend values do not leak; workload assumptions remain inspectable; bundled Web UI can be opened/configured without enabling MCP proxy or built-in tools; Network & Access remains the sole authority for bind/auth/TLS/CORS.
- **Mandatory Builder packets:** 7A (Rust semantic catalog, config/command/API trace, migrations) <=120k; then 7B (shared Wizard/Editor UI, teaching, captures/tests) <=120k. Each packet returns its own handoff/checkpoint; one fresh Verifier evaluates the complete Phase 7 diff and both packets.

### Phase 8 — Hugging Face and Model Library

- **State:** Not started
- **Budget:** 190k
- **Depends on:** Phases 2–5
- **Read:** gaps 3.2/3.7/3.8; accepted D9/D10/D29; A15/A17/A25/A29/A45–A46/A51–A57; Phase 8; existing sorting/creator/Community Picks/quant-swap code; HF/library and workload matrices.
- **Primary output:** Auto/GGUF/MLX/All plus preserved explicit sorting/category/curated-author discovery; revision-bound qualification; user-editable community-source roles; first-class heretic/uncensored and updated finetune/distillation paths; native/converted MLX lineage; local MLX introspection; context/KV/concurrency-driven artifact switching; canonical association; fit/template/tool/roleplay evidence; clear card hierarchy.
- **Completion proof:** search is not qualification; every mature GGUF discovery/quant/mmproj behavior has a regression gate; original author and converter stay distinct; community finetunes reach Rapid through qualified native MLX or conversion; repo/revision/variant survives end to end; context/KV changes recompute but never silently switch model quant; Recommended means workload fit; public search remains tokenless.
- **Mandatory Builder packets:** 8A (qualification/identity/lineage/fit APIs and fixtures) <=120k; then 8B (HF/Library discovery, cards, quant-switch UX and captures) <=120k. Each packet returns its own handoff/checkpoint; one fresh Verifier evaluates the complete Phase 8 diff and both packets.

### Phase 9 — Formatting, endpoints, and revision-pinned template substitution

- **State:** Not started
- **Budget:** 120k
- **Depends on:** Phases 2–3; Phase 0 template-arg grep (item 9)
- **Read:** gap 3.9 (rewritten by E1); D11/D21/D22; A10/A27 (both resolved by E1)/A38–A40; Phase 9; template and external-client matrices; Rapid/MLX-LM/SillyTavern evidence.
- **Primary output (E1 — architecture is resolved, this is NOT a "native override investigation"):** ONE revision-pinned template-selection layer with two thin appliers — llama.cpp via `--chat-template`/`--chat-template-file`, Rapid via **file placement** into an llama-monitor-owned copy/overlay (never the canonical/HF-cache dir), or a template-path flag if Phase 0's grep found Rapid accepts one. The driving reason is the §3.9 **tool-call-reliability** defect (stock Qwen3.6/Gemma4 templates loop/fail on tool calls; Froggeric and the official Google Gemma template are the candidates to qualify). A tool-call smoke-test matrix gates activation; one retained `[escalate→device]` M5 Max check confirms the first real substitution loads and kills the observed loop. Preserve Froggeric SHA/update handling while adding immutable `TemplateRelease` records, alternatives, provenance-distinct official-Google/community candidates, comparison, stale/update state, bounded history, and rollback. Plus client-protocol qualification and SillyTavern raw Text / structured Chat paths.
- **Completion proof:** the Rapid applier never mutates the canonical/HF-cache dir (only an owned copy/overlay, reversible/re-download-safe); the applier is labeled honestly per backend (no false parity); no Jinja renderer, shim/proxy, fork, or unreleased pin; a candidate becomes active only after passing the tool-call smoke test, and a failed test leaves the active selection unchanged; the M5 Max device check passes; Froggeric behavior does not regress; mutable upstream updates install alongside rather than overwrite the active release; official Google templates and community forks are provenance-distinct; rollback reaches the model-provided or any retained pinned release; no double template; SillyTavern owns raw instruct prompts; Rapid `/v1/completions` and llama `/completion` separately pass. There is no A27 "stop for approval" fork — the plan never depended on Rapid gaining native override. A heavier full tokenizer/config-replacement overlay still needs separate approval.

### Phase 10 — Screenshot-driven IA

- **State:** Not started
- **Budget:** 170k
- **Depends on:** Phases 7–9
- **Read:** D7–D10/D16; A16/A28/A32–A33/A38/A50; Phase 10; UI matrix and screenshot rules.
- **Primary output:** the accepted stable seven-category IA rendered from the completed control inventory, sequential captures, user visual review, and approved accessible implementation; retain the documented five-category design only as a fallback if real screenshots invalidate the accepted hierarchy.
- **Completion proof:** parity precedes reorder; accepted direction and final visual approval are recorded; category order remains stable across backends; both backends and workload profiles pass dark/light/narrow/accessibility review.

### Phase 11 — Diagnostics, metrics, and storage

- **State:** Not started
- **Budget:** 170k
- **Depends on:** Phases 3 and 5–7
- **Read:** cache telemetry Sections 6.1/6.2/6.5; A9/A12/A23–A24/A31/A37/A41/A48; Phase 11; diagnostics/security/client matrices.
- **Primary output:** effective-policy and capability diagnostics; cache/queue/TTFT/context/MTP metrics; bounded privacy; disk-state visibility and approved cleanup only; and the **cross-backend Doctor** (E11) — grow the existing rapid-mlx-focused Doctor to cover llama.cpp too (drawing on the Phase 3 llama capability snapshot), as a release-gating teaching + troubleshooting pillar. Each check traces to a real failure mode (same defect→test rigor), gives condition + explanation + remediation + a "why this happens" teaching note, at dual reading levels (novice + power-user) from one detection engine reusing the `[decide-once]` educational copy. Ship the already-surfaced checks: KV < q8_0 for tool-enabled llama, tool-call-loop template mismatch, invalid `--tool-call-parser` argv, stale/incompatible rapid-mlx update.
- **Completion proof:** no content telemetry; no raw/stable fingerprint leaves ephemeral process memory; local aggregate-only shadow telemetry (if built) is absent from exports/backups/network paths; zero differs from absent; MTP activation/fallback visible; schema drift degrades safely; storage operations remain bounded/authenticated; every Doctor check is anchored to a real failure mode (no speculative checks), covers both backends where the failure applies, and renders both reading levels from one detection engine with concrete remediation text.

### Phase 12 — Security, dependencies, and watchlist

- **State:** Not started
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
You are the Builder for Phase <N>.

Authoritative plan:
docs/plans/20260718-final_rapidmlx_followups.md

Execution companion:
docs/plans/20260718-final_rapidmlx_followups_execution.md

Read completely (use targeted line ranges — DO NOT read the entire plan):
- repository AGENTS.md
- comprehensive plan Section 9 (~line 1422–1512)
- comprehensive Phase <N> (~line <start>–<end> from Section 11)
- routed supporting sections only: e.g. "gap 3.2 (~line 232–246), decision D5 (~line 1370–1380), contract 7.1 (~line 1198–1210)"
- NEVER say "read Phase N in full" without providing the exact line range; sub-agents will blow context

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
You are the fresh independent Verifier for Phase <N>.

Read completely:
- repository AGENTS.md
- comprehensive plan Section 9
- comprehensive Phase <N>
- these routed design/matrix/evidence sections: <list>

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

**Last updated:** 2026-07-19 by Coordinator (Phase 0 verified; Phase 1 re-verified via 4 targeted passes all PASS; Phase 2 verified + hard gate 3 remediated frontend JS model_source_view adoption; HEAD f425b6e; Phase 3 next)

| Phase | State | Builder handoff | Verifier verdict | Commit/checkpoint | Remaining condition |
|---:|---|---|---|---|---|
| 0 | Verified complete | handoff.md | PASS (0 gaps) | phase-0/ | None |
| 1 | Verified complete | handoff.md | PASS (re-verified: 4 targeted passes all PASS) | phase-1/ | None |
| 2 | Verified complete | handoff.md | PASS (hard gate 3→remediated: frontend JS) | phase-2/ | None |
| 3 | Not started | — | — | — | Phases 1–2 |
| 4 | Not started | — | — | — | Phase 0 fixtures, Phase 2 identity |
| 5a | Not started | — | — | — | Phases 3–4 (execution policy + estimator core, own gate + fresh Verifier) |
| 5b | Not started | — | — | — | Phase 5a Verified (memory-availability + reclaim + wired-limit + acquisition repairs) |
| 6 | Not started | — | — | — | Phase 5 (5a + 5b) |
| 7 | Not started | — | — | — | Phases 2–3, 5–6 |
| 8 | Not started | — | — | — | Phases 2–5 |
| 9 | Not started | — | — | — | Phases 2–3 |
| 10 | Not started | — | — | — | Phases 7–9 and user IA decision |
| 11 | Not started | — | — | — | Phases 3, 5–7 |
| 12 | Not started | — | — | — | Phases 3, 8–11 |
| 13 | Not started | — | — | — | Phases 5–12 |
| 14 | Not started | — | — | — | Phases 1–13 |

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
