Phase: 0
Coordinator start date: 2026-07-19
Start HEAD: 72ba5385598d55c82e10d6e063b806f55923c38e
Branch: feat/rapid-mlx-integration
Worktree status: clean (no dirty files)
Upstream evidence date: 2026-07-19 (revalidated from 2026-07-18 audit)
Decisions used: A1–A58 per execution companion §6; all accepted/frozen per §6.1
Builder agent/run: Phase 0 Builder (context-free)
Builder result: evidence frozen, fixtures captured, fact-pins recorded
Verifier agent/run: pending
Verifier verdict: pending
Remediation iterations: none
Commands/tests: git rev-parse, uv pip list, curl fetches, grep evidence
Screenshot artifacts: none (Phase 0 has no UI work)
Security/platform notes: no secrets logged; runtime env inspected read-only
Final checkpoint/commit: pending Verifier sign-off
Open external limitations: upstream HEAD 73034c5 ahead of audited 75b1fe3; installed runtime v0.10.10 vs audit v0.10.12

## Preserved User Changes

None. Worktree was clean at Phase 0 start.

## Dependencies

- Phase 0 has no prerequisites.
- Phases 1–3 depend on this evidence freeze.

## Evidence Snapshot

### Installed Runtime

- Path: ~/.config/llama-monitor/runtimes/rapid-mlx/environments/0.10.10-abdd7d5421445014c012b23d/tool/rapid-mlx
- Executable: rapid-mlx 0.10.10
- Python: 3.14.5 (uv-managed)
- Key packages: rapid-mlx 0.10.10, mlx 0.32.0, mlx-lm 0.31.3, transformers 5.12.1

### Upstream Evidence

- Rapid-MLX latest tag: v0.10.12 (2026-07-17)
- Audited commit: 75b1fe3b3a8ab12967f64150524296f179dd9979 (1 commit ahead of release)
- Rapid-MLX main HEAD: 73034c55118e4300cfd3e26c610a8d159467532e (significantly ahead of audit)
- llama.cpp pinned baseline: 571d0d540df04f25298d0e159e520d9fc62ed121 (unchanged)

### HF Fixtures

Six configs downloaded to tests/fixtures/rapid_mlx/configs/ with SHA-256 checksums and main-branch commit SHAs.

## Scope

Phase 0: Evidence freeze, fact-pins, fixtures, traceability, unresolved decision packets.
NO application code changes. NO Cargo.lock mutations. NO Cargo.toml changes.

## Brief

Execute 10 Phase 0 items per comprehensive plan §11.1:
1. Record state (done)
2. Inventory installed packages (done)
3. Re-fetch upstream evidence + drift report (done)
4. Resolve HF URLs to commits + fixtures (done)
5. Capture CLI help + metadata (done)
6. Build P0/P1 traceability table (done)
7. Unresolved decisions as packets (done)
8. Platform expectations (done)
9. [E1] Template applier fact-pin (done)
10. [E3] KV-dtype fact-pin (done)

## Artifacts Created

See phase-0/ subdirectory:
- coordinator-start.md (this file)
- evidence-drift.md
- hf-fixtures-manifest.md
- e1-template-pin.md
- e3-kv-dtype-pin.md
- traceability-table.md
- unresolved-decisions.md
- platform-expectations.md
- coordinator-handoff.md
- cli/serve-help.txt, cli/version.txt, cli/pip-show.txt, cli/pip-list.txt
