# Phase 0 Builder Handoff Summary

## Completed Tasks

1. ✅ Recorded state: feat/rapid-mlx-integration, HEAD 72ba538, clean worktree
2. ✅ Inventoried installed packages: rapid-mlx 0.10.12 (active), mlx 0.32.0, mlx-lm 0.31.3, transformers 5.12.1 (prev: 0.10.10)
3. ✅ Re-fetched upstream evidence: v0.10.12 intact; 75b1fe3 intact; llama.cpp 571d0d5 intact; main advanced to 73034c5
4. ✅ Resolved 6 HF model URLs to commits; configs downloaded with SHA-256 checksums
5. ✅ Captured CLI help + metadata: serve --help, version, pip show/list saved to cli/
6. ✅ Built P0/P1 traceability table: 10 gaps mapped to phases 1–9
7. ✅ Unresolved decisions as packets: 2 packets (runtime architecture, llama tools)
8. ✅ Platform expectations: macOS=required, Linux/Windows=graceful unavailability
9. ✅ [E1] Template applier fact-pin: NO --chat-template flag exists (file:line evidence recorded)
10. ✅ [E3] KV-dtype fact-pin: --kv-cache-dtype {bf16,int8,int4} default int4; --reasoning pins int8; TurboQuant {v4,k8v4,none} (file:line evidence recorded)

## Artifacts Created

```
docs/plans/handoffs/20260718-final-rapidmlx-followups/phase-0/
├── coordinator-start.md
├── evidence-drift.md
├── hf-fixtures-manifest.md
├── e1-template-pin.md
├── e3-kv-dtype-pin.md
├── traceability-table.md
├── unresolved-decisions.md
├── platform-expectations.md
├── coordinator-handoff.md (this file)
└── cli/
    ├── serve-help.txt
    ├── version.txt
    ├── pip-show.txt
    └── pip-list.txt

tests/fixtures/rapid_mlx/configs/
├── mlx-community_Qwen3.6-27B-4bit.config.json
├── mlx-community_Qwen3.6-35B-A3B-4bit.config.json
├── mlx-community_gemma-4-31b-it-4bit.config.json
├── mlx-community_gemma-4-26b-a4b-it-4bit.config.json
├── mlx-community_Qwen3-0.6B-4bit.config.json
└── mlx-community_Qwen3-30B-A3B-4bit.config.json
```

## Findings Requiring Human Review

1. ✅ RESOLVED: Runtime version gap closed — active managed runtime is now v0.10.12 (matches audit). Previous v0.10.10 retained in environments/ for rollback.
2. Two unresolved decision packets (runtime architecture, llama tools) per §6.1 — Coordinator must present to user.
3. Upstream main (73034c5) has advanced beyond audited commit (75b1fe3). Evidence pins to audited; main tracked separately.

## Blockers for Phase 1 Builder

NONE. All Phase 0 items completed:
- Evidence frozen and pinned to immutable commits.
- Fact-pins (E1, E3) recorded with file:line evidence.
- Fixtures captured with checksums.
- Traceability has no orphan P0/P1.
- No application code changed. No Cargo.lock mutated.
- Phase 1 Builder can proceed using v0.10.12 audited source + installed v0.10.10 CLI as evidence.

## Constraints Maintained

- No application code changes
- No Cargo.lock mutations
- No Cargo.toml changes
- All A-items accepted/frozen unless gated by undev'd code or measurements
- Evidence pinned to commits, not mutable URLs
