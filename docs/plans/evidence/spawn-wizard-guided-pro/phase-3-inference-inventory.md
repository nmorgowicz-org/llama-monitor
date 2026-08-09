# Phase 3 inference inventory

This is the Phase 3A audit baseline. It classifies model-property inference sites before repair;
the entries below are not treated as completion evidence.

| Source | Current behavior | Classification | Required disposition |
|---|---|---|---|
| `static/js/features/spawn-wizard.js::_inferFamilyFromName` | Removed; family is populated only from persisted typed metadata or authoritative `gguf_arch` | Closed | No repository/filename family fallback remains |
| `static/js/features/spawn-wizard.js::detectMtpFromName` | Removed; MTP capability/defaults remain introspection/profile driven | Closed | No filename-based MTP capability inference remains |
| `static/js/features/spawn-wizard.js::showStep` | Speculative decoding defaults are gated by resolved metadata and explicit user state | Closed | No filename-based MTP default remains |
| `static/js/features/spawn-wizard.js::buildSpawnPayload` | Uses introspected architecture and explicit controls; unknown/degraded metadata stays unknown | Closed | No name-derived MTP/architecture values enter payload or estimator state |
| `static/js/features/spawn-wizard-mmproj.js` | Uses typed `wizardState.model.family`; filename remains only for artifact stem matching | Repaired | Consume server metadata/profile; keep artifact lookup separate; fresh selector capture proves recommendation |
| `static/js/features/spawn-wizard.js::getEffectiveArch/buildHeuristicArch` | Effective architecture previously merged filename/parameter heuristics | Repaired | Effective arch now returns introspection fields only; compatibility helper is inert and unknown/degraded state remains explicit |
| `static/js/features/spawn-wizard.js::inferParamBFromName` | Filename parameter-count parser | Removed | Parameter counts now arrive from GGUF/HF metadata or remain unknown |
| `src/llama/model_defaults.rs` | Legacy compatibility helpers remain name-aware, but `/api/model-defaults` production authority is `SamplingCatalog` and passes typed architecture/profile metadata | Isolated compatibility surface | Retained for API/test compatibility; never used as wizard model-property authority |
| `src/llama/sampling_catalog.rs` | Family mode selection previously inspected model/repo filename | Repaired | Family-specific modes now require `gguf_arch` or typed family; arbitrary names use universal modes |
| `src/web/api/benchmark.rs` | Uses `from_name_and_params` when architecture metadata absent | Heuristic fallback | Mark degraded and constrain fallback to estimator-only, never capability/default claims |
| `src/web/api/vram.rs` | Uses name/parameter heuristic when no GGUF metadata exists | Estimator fallback | Keep only as explicitly degraded estimate with provenance; never feed MTP/vision/runtime claims |
| `src/web/api/models.rs` | Builds architecture from GGUF architecture plus name/params | Mixed | Ensure real GGUF metadata wins and name fallback cannot populate capability claims |
| `static/js/features/spawn-wizard-rapid-mlx.js` | Rapid profile/unified-profile responses drive parser, vision, and speculative recommendations | Repaired | Profile success/failure now updates shared metadata status/reason with safe degraded copy |

Active wizard repair packet is closed: source-backed local/streamed GGUF and MLX profile paths preserve authoritative metadata and explicit degraded state. The remaining `model_defaults.rs` name-aware helpers are isolated compatibility code and are not part of wizard authority.
