# Spawn Wizard

The Spawn Wizard is the guided flow for creating a model server. It provides:

- Profile and use-case selection (agentic / general / roleplay)
- Engine selection between llama.cpp and Rapid-MLX
- Model source input (local GGUF, Hugging Face, or import)
- Architecture-aware VRAM breakdown and context fit modes
- Auto-size recommendations and MoE tuning
- Per-backend settings isolation

## Steps

| Step | Purpose |
|------|---------|
| 1. How it works | Choose profile (speed / balanced / quality) and use case |
| 2. Choose model | Select engine, choose model source, set model-specific options |
| 3. Hardware & memory | Tune context, offload, batching, speculative decoding, VRAM |
| 4. Settings | Review network, security, and advanced launch flags |
| 5. Review settings | Summary of the configuration before launch |
| 6. Start server | Launch and monitor start-up |

## Engine selection

The wizard supports two inference backends:

- llama.cpp — native for GGUF models
- Rapid-MLX — optimized for MLX-ecosystem models on Apple Silicon

Engine selection appears on Step 2 (Choose model) as two cards. The wizard:

- Prefers llama.cpp by default.
- Automatically recommends Rapid-MLX when the chosen model source is native to it.
- Allows the user to override the recommendation (choice is preserved).

### When Rapid-MLX is recommended

The wizard calls `/api/rapid-mlx/recommend` after:

- a model source or file is selected,
- a HF repo is entered (with explicit Rapid-MLX engine),
- the engine is changed.

The endpoint uses `recommend_backend()` (src/inference/backend.rs) which makes a recommendation based on:

- the classified artifact type (see below),
- whether Apple Silicon is detected locally,
- whether a compatible Rapid-MLX runtime is available.

Recommendation outcomes:

- GGUF file or GGUF inventory
  - Recommended: llama.cpp. Reason: "GGUF runs natively with llama.cpp."
- MLX directory, authoritative Safetensors, Rapid-MLX HF repository, Rapid-MLX alias:
  - Not Apple Silicon:
    - State: platform_unavailable
    - Rapid-MLX card becomes visually "unavailable"; user can still attach a remote Rapid-MLX endpoint.
  - Apple Silicon, runtime not installed:
    - State: runtime_required
    - Wizard blocks next step; message instructs user to install from Settings.
  - Apple Silicon, runtime available:
    - Recommended: Rapid-MLX.
    - If the user hasn't explicitly chosen an engine, Rapid-MLX is auto-selected.
- Unknown source:
  - State: manual_selection
  - User must pick an engine after defining the model source.

### Artifact classification

The wizard classifies the selected artifact (spawn-wizard.js:classifyWizardArtifact):

- gguf:
  - path or hfFile ends with .gguf, or quant file list contains a .gguf file.
- authoritative_safetensors:
  - model source kind is "authoritative_safetensors" (from a typed library entry).
- rapid_mlx_alias:
  - model source kind is "alias" (e.g., HF-style alias name resolved by Rapid-MLX).
- rapid_mlx_hf_repository:
  - model source kind is "hugging_face_repo" (Rapid-MLX managed HF repository reference).
- mlx_directory:
  - model source kind indicates MLX directory.
- unknown:
  - none of the above.

The classification is used both by the UI (to show appropriate hints) and by the recommendation endpoint.

## Rapid-MLX wizard UX

When Rapid-MLX is selected, the wizard adapts the Step 2 and Step 3 UI:

- Model source description:
  - Switches to "Choose a validated MLX directory or a Rapid-MLX Hugging Face repository."
- Local model card:
  - Label changes to "Select local MLX model".
  - Description: "Browse to a validated MLX model directory."
  - Browse button switches to directory mode instead of GGUF-only.
- HF source card:
  - Description: "Enter a Rapid-MLX-compatible Hugging Face repository ID."
  - For Rapid-MLX, entering a repo ID is sufficient (no GGUF file picker).
- Import source card:
  - Hidden when Rapid-MLX is selected (Rapid-MLX does not support the import path).
- Hardware step:
  - llama.cpp-specific controls (GPU layers, KV cache types, MoE offload, mlock,
    threads, speculative decoding, MTP, mmproj) are hidden.
  - A Rapid-MLX-specific panel (rapid-hardware-panel) is shown for backend-specific
    configuration, keeping its settings isolated from llama.cpp flags.
- Launch guard:
  - Step 2 validation:
    - Blocks if Rapid-MLX is selected but not Apple Silicon.
    - Blocks if Rapid-MLX is recommended-ready but a GGUF was chosen under it;
      instructs switching engines or choosing a validated MLX source.
    - Blocks if a Rapid-MLX-specific model source (alias, HF repository, MLX directory)
      is used under llama.cpp; instructs switching to Rapid-MLX engine.

## Runtime install and upgrade

The Rapid-MLX runtime is managed by Llama Monitor. The wizard does not ship its own installer;
it relies on the runtime management APIs documented in rapid-mlx-runtime.md.

Wizard behavior tied to runtime state:

- On open:
  - Calls `/api/rapid-mlx/runtime/status` and platform-info.
  - If Apple Silicon and runtime is active, the Rapid-MLX card shows "Runtime ready".
  - If Apple Silicon but runtime is missing, it shows "Runtime setup required".
  - On non-Apple Silicon, the card is marked "Local launch · Apple Silicon only".
- Step 2 validation:
  - If Rapid-MLX is selected but runtime_required:
    - User cannot proceed; hint points to Settings → Rapid-MLX to install a version.
- Engine badge:
  - Displays one of:
    - "Runtime ready"
    - "Runtime setup required"
    - "Local launch · Apple Silicon only"

The user installs or upgrades the runtime from Settings, using the managed runtime
UI (version picker, channel selection, job polling). After a successful install,
the wizard reflects the new runtime-ready state.

## HF alias support

Rapid-MLX integrates HF-style aliases. These are human-readable model names (for
example, "Qwen2.5-0.5B-Instruct") that Rapid-MLX can resolve to the correct source
repository and revision.

Wizard behavior:

- When a Rapid-MLX model source has kind "alias" in `rapidMlxSource` or
  `localMeta.model_source`, `classifyWizardArtifact()` classifies it as `rapid_mlx_alias`.
- The recommendation endpoint treats this as native to Rapid-MLX:
  - If runtime is compatible and platform supports it, it auto-selects Rapid-MLX.
  - If the user attempts llama.cpp, the validation step blocks with:
    "This typed model source requires Rapid-MLX. Switch engines to continue."
- Alias-based models behave the same as other Rapid-MLX-native sources for VRAM
  estimation, hardware panel rendering, and launch.

## VRAM estimator

The spawn wizard uses the backend VRAM estimator as the single source of truth; there
are no local VRAM formulas.

- The wizard sends requests to `/api/vram-estimate` via `scheduleEstimate()`
  in vram-estimate.js.
- `buildEstimateBody()` sets:
  - `backend: "rapid_mlx"` when Rapid-MLX is selected.
  - `backend: "llama_cpp"` by default.
- The backend returns a normalized breakdown (weights, KV cache, overhead, free)
  for the selected backend.

Behavior per backend:

- llama.cpp:
  - Uses GGUF-introspected architecture, layer counts, quantization, MoE settings.
  - Reflects GPU layers, context size, KV cache type, speculation, mmproj, etc.
- Rapid-MLX:
  - Uses Rapid-MLX-specific memory modeling based on the selected model.
  - Reflects backend-specific overhead and any Rapid-MLX-native memory considerations.

The VRAM bar and side panel always use the same visual layout regardless of engine,
but the underlying numbers differ because the `backend` field is respected server-side.

## Backend-aware settings

The wizard isolates settings per backend:

- llama.cpp-only settings (hidden under Rapid-MLX):
  - GPU layers (-ngl)
  - KV cache types (ctk/ctv)
  - MoE CPU offload (-n-cpu-moe)
  - mlock, threads, threads-batch
  - Speculative decoding and MTP controls
  - mmproj projector selection
  - Flash attention, fit-to-memory, priority
- Rapid-MLX-only settings:
  - Exposed via a dedicated Rapid-MLX hardware panel
  - No llama.cpp flags; Rapid-MLX models do not send llama.cpp args.

This ensures generated launch commands only include parameters valid for the chosen backend.
