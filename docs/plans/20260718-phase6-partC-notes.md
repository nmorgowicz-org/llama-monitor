Phase 6 Part C — Cache Cleanup Policy + Diagnostic Panel Wiring

Summary:
- Diagnostic panel now detects cache misconfigurations and memory-pressure reclaim needs via GET /api/doctor/findings.
- Findings are additive to existing DoctorFinding format; new Cache finding_type.
- No automatic mutations; all findings are recommendations only.

Files changed:

src/state.rs:
- Added Cache variant to DoctorFindingType
- Added FixAction variants: DisablePrefixCache, AdjustPrefixCacheBudget(u64), DisableMaxCacheBlocks, SetPrefixCacheBudget(u64), ReclaimBackendAllocatorCache

src/inference/rapid_mlx/capabilities.rs:
- Added generate_snapshot_from_discovery() helper
- Added CacheDiagnosticParams struct
- Added PrefixCacheDiagnosticFindings and CacheDiagnosticFinding structs
- Added CapabilitySnapshot::compute_prefix_cache_findings() method detecting:
  - CACHE_BUDGET_EXCEEDS_CEILING (error): budget > configured_ceiling_bytes
  - CACHE_BLOCKS_UNSUPPORTED (warning): max_cache_blocks set but --max-cache-blocks missing
  - CACHE_ENABLED_NO_BUDGET_LOW_HEADROOM (warning): enabled with zero budget and <30% headroom
- Added CapabilitySnapshot::supports_max_cache_blocks() helper
- Added bytes_to_human() helper

src/web/api/doctor.rs (new):
- GET /api/doctor/findings endpoint (api-token auth)
- collect_cache_findings(): reads active session's RapidMlxConfig + capability snapshot + memory snapshot, runs compute_prefix_cache_findings()
- collect_reclaim_findings(): when state ∈ {Unsafe, AfterClosingApps}, suggests reclaim via compute_reclaim_guidance()
- 5 tests: budget_exceeds_ceiling, blocks_unsupported, enabled_no_budget_low_headroom, no_issues_when_within_limits, supports_max_cache_blocks_detects_flag

src/web/api/mod.rs:
- Added doctor module and wired doctor::routes into api_routes

src/web/api/sessions.rs:
- Extended apply-fix match to handle new FixAction variants

Findings produced:

1) CACHE_BUDGET_EXCEEDS_CEILING (Issue/error)
   - prefix_cache.budget_bytes > MemoryAvailabilitySnapshot.configured_ceiling_bytes
   - fixable: true; fix_action: AdjustPrefixCacheBudget(recommended)

2) CACHE_BLOCKS_UNSUPPORTED (Warning)
   - max_cache_blocks set in config but capability snapshot lacks --max-cache-blocks
   - fixable: true; fix_action: DisableMaxCacheBlocks

3) CACHE_ENABLED_NO_BUDGET_LOW_HEADROOM (Warning)
   - prefix_cache_enabled=true, budget_bytes=Some(0), headroom < 30% of ceiling
   - fixable: true; fix_action: SetPrefixCacheBudget(recommended)

4) Memory pressure reclaim suggestions (Issue/Warning)
   - From compute_reclaim_guidance() when state ∈ {Unsafe, AfterClosingApps}
   - fixable: true; fix_action: ReclaimBackendAllocatorCache (diagnostic-only; actual reclaim via system_tools)

Tests:
- cargo test --lib doctor: 13 passed (includes new cache_finding tests)
- New tests: cache_finding_budget_exceeds_ceiling, cache_finding_blocks_unsupported,
  cache_finding_enabled_no_budget_low_headroom, cache_finding_no_issues_when_within_limits,
  supports_max_cache_blocks_detects_flag

Build:
- cargo build --release: OK
- cargo clippy -- -D warnings: OK
- cargo fmt --check: OK
- cargo test --lib: 794 passed, 17 ignored
- cargo check --target x86_64-pc-windows-gnu: OK (pre-existing macos-gated errors only, no new platform issues)

Hard gates verified:
- No automatic cache mutations; findings are recommendations only
- User explicit values preserved; fix_action is advisory, never auto-applied

Notes for Verifier:
- Confirm finding codes/messages match spec wording
- Confirm fix_action shape is suitable for Phase 7 UI wiring
- Confirm no unintended coupling to reclaim execution (only compute_reclaim_guidance() results exposed; actual reclaim remains via system_tools endpoint)
- Confirm DoctorFindingType::Cache is additive (no breaking changes to existing findings)