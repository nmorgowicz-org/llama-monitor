# Phase 3 inference inventory

This is the Phase 3A audit baseline. It classifies model-property inference sites before repair;
the entries below are not treated as completion evidence.

| Source | Current behavior | Classification | Required disposition |
|---|---|---|---|
| `static/js/features/spawn-wizard.js::_inferFamilyFromName` | Maps repo/file strings to family labels; used by mmproj/search helpers | Unsafe model-property inference | Replace with server-owned metadata/profile family; preserve only artifact search labeling if it cannot feed defaults/capabilities |
| `static/js/features/spawn-wizard.js::detectMtpFromName` | Detects MTP from filename substrings | Unsafe capability inference | Remove from enable/default paths; use GGUF/profile capability evidence |
| `static/js/features/spawn-wizard.js::showStep` | Defaults speculation to `draft-mtp` when selected name contains `mtp` | Unsafe automatic default | Remove; only qualified introspection may recommend it |
| `static/js/features/spawn-wizard.js::buildSpawnPayload` | Falls back to name-derived MTP/architecture values in estimator input | Unsafe fallback | Preserve explicit user values; represent unknown/degraded metadata without guessing |
| `static/js/features/spawn-wizard-mmproj.js` | Family fallback from HF repo/filename | Unsafe capability/family inference | Consume server metadata/profile; keep artifact lookup separate |
| `src/llama/model_defaults.rs` | Family-specific defaults from caller-supplied name/family | Transitional authority gap | Require typed metadata/provenance at API boundary; universal defaults on unknown |
| `src/web/api/benchmark.rs` | Uses `from_name_and_params` when architecture metadata absent | Heuristic fallback | Mark degraded and constrain fallback to estimator-only, never capability/default claims |
| `src/web/api/vram.rs` | Uses name/parameter heuristic when no GGUF metadata exists | Estimator fallback | Keep only as explicitly degraded estimate with provenance; never feed MTP/vision/runtime claims |
| `src/web/api/models.rs` | Builds architecture from GGUF architecture plus name/params | Mixed | Ensure real GGUF metadata wins and name fallback cannot populate capability claims |

Next repair packet must add source-backed tests for local GGUF, streamed-HF GGUF, and MLX config
metadata, then remove each unsafe recommendation/capability fallback from the closure set.
