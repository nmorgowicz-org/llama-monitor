# P0/P1 Requirement Traceability Table

Source: Comprehensive plan Section 3 (Critical Gaps), mapped to Phase ownership from Section 11.

| ID | Severity | Gap | Own Phase | Responsible | Verification Test |
|----|----------|-----|-----------|-------------|-------------------|
| 3.1 | P0 | Invalid tool-parser launch arguments (--tool-call-parser bare, --auto-tool-choice) | 1 | Builder | argv test: no bare --tool-call-parser; no --auto-tool-choice emitted; explicit --enable-auto-tool-choice only with capability |
| 3.2 | P0 | Typed model-source split brain (RapidMlxModelSource ignored by frontend) | 2 | Builder | typed fixture never shows "No model configured"; all surfaces use shared codec; legacy path read-only migration |
| 3.3 | P0 | Visible controls that do nothing (speculative, MLLM, embedding) | 1 | Builder | search proves hidden controls cannot enter launch payload; read-only eligibility shown per A30 |
| 3.4 | P0 | Rapid preset sampling persisted but not applied | 2 | Builder | request_from_preset() maps to Rapid launch; explicit client values win; no double application |
| 3.5 | P0 | MLX memory metadata not architecture-complete (nested text_config, hybrid layers, Gemma4 KV) | 4 | Builder | six pinned fixtures assert correct layer groups, KV heads, recurrent state, context ceiling |
| 3.6 | P0 | Rapid memory controls/estimates use llama.cpp vocabulary (ctk/ctv) | 5 | Builder | no Rapid estimate accepts/displays llama KV vocabulary; requested/effective policies distinct |
| 3.7 | P1 | Estimator data-source and arithmetic defects (*8 conversion, HF pagination, local MLX) | 4/5 | Builder | *8 conversion tested; HF revision-aware/paginated; local MLX parses config/index |
| 3.8 | P1 | Upstream info/extras assumptions false (info ≠ verified alias) | 3 | Builder | capability state: arbitrary finetunes distinct from verified aliases; source each claim independently |
| 3.9 | P0/P1 | Stock chat templates tool-call-unreliable; no revision-pinned template substitution | 9 | Builder | tool-call smoke-test matrix; Rapid applier never mutates canonical dir; llama via --chat-template-file |
| 3.10 | P1 | Security and dependency gaps (trust_remote_code, [guided] extra, broken mlx-vlm) | 3/12 | Builder | data-only MLX repos no blanket warning; custom-code consent revision-scoped; [guided] probed separately |
| 3.11 | P0/P1 | Cross-backend agent-workload gaps (cache -1, MTP single-stream, ctx-size semantics) | 1/5 | Builder | unified-memory Auto resolves cache to 0; -1 enabled/unlimited sentinel test; ctx-size total KV pool |

Phase mapping:
- Phase 1: 3.1, 3.3, 3.10 (partial), 3.11 (partial)
- Phase 2: 3.2, 3.4
- Phase 3: 3.8, 3.10 (partial)
- Phase 4: 3.5, 3.7 (partial)
- Phase 5: 3.6, 3.7 (partial), 3.11 (partial)
- Phase 9: 3.9

No orphan P0/P1 findings. Every gap mapped to a concrete phase and verification test.
