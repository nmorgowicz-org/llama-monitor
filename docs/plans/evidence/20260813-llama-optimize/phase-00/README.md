# Calibration Phase 0 evidence

Date: 2026-08-13

This directory records the Phase 0 contract and authority gate for native
llama.cpp Calibration v1. The pinned upstream source review and managed-binary
receipts were completed in the existing release evidence set and are linked
below rather than copied into a second uncontrolled ledger.

## Pinned authority

- Upstream contract/license ledger: [`../../../20260811-local-llm-foundry/phase-11/calibration-upstream-contract-ledger.md`](../../../20260811-local-llm-foundry/phase-11/calibration-upstream-contract-ledger.md)
- macOS managed sibling-binary receipt: [`../../../20260811-local-llm-foundry/phase-11/calibration-managed-binaries-macos.md`](../../../20260811-local-llm-foundry/phase-11/calibration-managed-binaries-macos.md)
- Native regular-file portability receipt: [`../../../20260811-local-llm-foundry/phase-11/calibration-native-port-receipt.md`](../../../20260811-local-llm-foundry/phase-11/calibration-native-port-receipt.md)
- Attribution and update procedure: [`THIRD_PARTY_LICENSES.md`](../../../../../THIRD_PARTY_LICENSES.md)

The upstream ledger records the pinned commits, licenses, reviewed source
manifest, CLI contract boundary, self-test result, and the known portability
failure. Human-readable upstream console output is not an integration
contract.

## Phase 0 implementation evidence

- Backend dispatch is centralized in
  `classify_backend_benchmark_result`; Rapid-MLX routes to its informational
  classifier and cannot emit actionable llama.cpp fields.
- GGUF/HF advice uses introspected metadata or explicit caller-supplied
  architecture/profile data. Missing metadata is represented as
  `unknown/degraded`; filenames remain display labels only.
- Rust coverage includes Rapid-MLX non-actionability and renamed-GGUF
  architecture precedence. UI coverage verifies informational Rapid-MLX cards
  have no Apply control.
- Calibration API routes require `api-token` for reads/starts/cancellation and
  `db-admin-token` plus confirmation for apply/rollback. Route-level lifecycle
  coverage remains a Phase 1 gate and is intentionally not claimed here.

## Gate result

The source-side Phase 0 gate passes: backend dispatch, metadata provenance,
attribution, API authentication checks, and UI informational-only behavior are
covered by the implementation and focused tests. Native Windows managed-binary
help and real-runtime receipts remain an explicit Phase 10 return marker; they
must be collected on Windows and cannot be inferred from this macOS checkout.
