# Phase 3 inference inventory

This is the Phase 3A audit baseline. It classifies model-property inference sites before repair;
the entries below are not treated as completion evidence.

| Source | Current behavior | Classification | Required disposition |
|---|---|---|---|
| `static/js/features/spawn-wizard.js::_inferFamilyFromName` | Removed; family is populated only from persisted typed metadata or authoritative `gguf_arch` | Closed | No repository/filename family fallback remains |
| `static/js/features/spawn-wizard.js::detectMtpFromName` | Removed; MTP capability/defaults remain introspection/profile driven | Closed | No filename-based MTP capability inference remains |
| `static/js/features/spawn-wizard.js::showStep` | Defaults speculation to `draft-mtp` when selected name contains `mtp` | Unsafe automatic default | Remove; only qualified introspection may recommend it |
| `static/js/features/spawn-wizard.js::buildSpawnPayload` | Falls back to name-derived MTP/architecture values in estimator input | Unsafe fallback | Preserve explicit user values; represent unknown/degraded metadata without guessing |
| `static/js/features/spawn-wizard-mmproj.js` | Uses typed `wizardState.model.family`; filename remains only for artifact stem matching | Repaired | Consume server metadata/profile; keep artifact lookup separate; fresh selector capture proves recommendation |
| `static/js/features/spawn-wizard.js::getEffectiveArch/buildHeuristicArch` | Effective architecture previously merged filename/parameter heuristics | Repaired | Effective arch now returns introspection fields only; compatibility helper is inert and unknown/degraded state remains explicit |
| `src/llama/model_defaults.rs` | Family-specific defaults from caller-supplied name/family | Transitional authority gap | Require typed metadata/provenance at API boundary; universal defaults on unknown |
| `src/llama/sampling_catalog.rs` | Family mode selection previously inspected model/repo filename | Repaired | Family-specific modes now require `gguf_arch` or typed family; arbitrary names use universal modes |
| `src/web/api/benchmark.rs` | Uses `from_name_and_params` when architecture metadata absent | Heuristic fallback | Mark degraded and constrain fallback to estimator-only, never capability/default claims |
| `src/web/api/vram.rs` | Uses name/parameter heuristic when no GGUF metadata exists | Estimator fallback | Keep only as explicitly degraded estimate with provenance; never feed MTP/vision/runtime claims |
| `src/web/api/models.rs` | Builds architecture from GGUF architecture plus name/params | Mixed | Ensure real GGUF metadata wins and name fallback cannot populate capability claims |

Next repair packet must add source-backed tests for local GGUF, streamed-HF GGUF, and MLX config
metadata, then remove each unsafe recommendation/capability fallback from the closure set.
