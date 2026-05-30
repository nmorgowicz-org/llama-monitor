# Spawn Llama-Server V2 — Frontend Architecture and Wizard UI

- **Branch:** `feature/spawn-llama-server-v2`
- **Date:** 2026-05-29
- **Author:** Iris (via Hermes)

This is the frontend implementation reference for the Spawn Llama-Server V2 feature.
It defines the wizard architecture, UI flows, error handling, and testing strategy.

For the high-level overview and phased plan, see:
- `docs/plans/20260529-spawn-v2-overview.md`

For backend architecture, APIs, and data models, see:
- `docs/plans/20260529-spawn-v2-backend.md`

For llama.cpp tuning, third-party compatibility, and research, see:
- `docs/plans/20260529-spawn-v2-research.md`

---

## Current State (Summary)

Key existing UI patterns:

- **Premium modals:**
  - From: `agent-modal.css` + `modal-premium.css`
  - Glassmorphism, radial + linear gradients, blur, multi-shadow, top-line, border glow.
- **Step-by-step layout:**
  - From: `.agent-setup-section` + `.step-badge`
  - Numbered sections with conditional visibility, hover glow, staggered animation.
- **Hero intro card:**
  - From: `.agent-setup-hero`
  - Icon, title, description at top of wizard.
- **Progress bar and status:**
  - From: `.agent-setup-progress`, `.agent-setup-status`
  - Animated progress during spawn, status messages for success/error.
- **Button styles:**
  - From: `.btn-agent-*` variants
  - Gradient, shadow, hover-lift buttons for each step.
- **File browser:**
  - From: `file-browser.js` + `#file-browser-modal`
  - Used for model selection.
- **Preset selection:**
  - From: `#preset-select` and `#setup-preset-select`
  - Dropdown of presets, with link to edit in preset modal.
- **Spawn API integration:**
  - From: `sessions.js:saveSession()` + `attach-detach.js:doStart()`
  - `/api/sessions/spawn` or `/api/start` with enriched config.

Missing (must be built new):

- A dedicated spawn-wizard module that:
  - Defines steps (e.g., Profile → Model → Hardware → Summary → Spawn).
  - Manages step state and transitions.
  - Optionally pre-validates hardware (GPU, memory) before spawn.
- HTML for the Spawn V2 wizard modal (or enhanced session-modal spawn form).
- CSS for the Spawn V2 wizard (can extend `agent-modal.css`/`modal-premium.css` patterns).

---

## SpawnWizard Module Architecture

The SpawnWizard is a dedicated JS module (`static/js/features/spawn-wizard.js`) that:

- Defines steps (e.g., Profile → Model → Hardware → Summary → Spawn).
- Manages step state and transitions.
- Integrates with existing modals (sessions, presets, file-browser, attach-detach).
- Uses existing API endpoints and utilities (authHeaders, toast, showConnectingState).

### Steps

The wizard has 5 steps:

1. **Profile:**
   - Choose:
     - "Quick" (auto-tune based on hardware and model).
     - "Balanced" (guided tuning with sensible defaults).
     - "Advanced" (full control over all parameters).
   - Inline help: short description of each profile.

2. **Model:**
   - Select model:
     - From local directory (file browser).
     - From HuggingFace (model search).
     - From third-party import (Ollama, LM Studio, etc.).
   - Inline help: short description of each option.

3. **Hardware:**
   - GPU layers, context size, batch sizes.
   - VRAM estimator feedback inline.
   - MoE tuning assistance (if applicable).
   - Inline help: short description of each field.

4. **Summary:**
   - Review all settings.
   - "Save as Preset" option.
   - "Run Health Check" button (if applicable).
   - Inline help: short description of each field.

5. **Spawn:**
   - "Spawn Server" button.
   - Progress bar and status messages.
   - Inline help: short description of each step.

### State Management

The wizard maintains a state object:

- `profile`: "quick" | "balanced" | "advanced"
- `model`: { path, name, size, quant }
- `hardware`: { gpuLayers, contextSize, batchSize, ubatchSize, moeTuning }
- `summary`: { presetName, healthCheckResults }
- `spawn`: { status, progress, error }

This state is:

- Used to drive the wizard UI.
- Sent to the backend via `/api/sessions/spawn` or `/api/start`.
- Persisted in localStorage for quick recovery (optional).

### Integration with Existing Modals

The wizard integrates with existing modals:

- **File browser:**
  - Used for model selection in Step 2.
- **Preset selection:**
  - Used for preset selection in Step 1.
- **Session modal:**
  - Used for session management in Step 5.
- **Attach-detach:**
  - Used for spawn API integration in Step 5.

---

## UI Flows

### Welcome Screen

- Left card:
  - Attach to existing endpoint.
  - Recent endpoints (2-3).
  - Manual attach.
- Right card:
  - Spawn new llama-server (from preset or scratch).
  - "Get llama-server" if no binary detected.

First-time user:

- No recent endpoints.
- "Open Spawn Wizard" as primary CTA.

Returning user:

- Fast path: select preset + "Start Server".
- Secondary: "Open Spawn Wizard" for new config.

### Wizard Steps

Each step uses premium glassmorphism, clear CTAs, inline help.

- Step 1: Profile.
- Step 2: Model.
- Step 3: Hardware.
- Step 4: Summary.
- Step 5: Spawn.

### Error Handling

- Classification:
  - FATAL: cannot proceed (e.g., no model, no GPU).
  - CRITICAL: can proceed but with warnings (e.g., VRAM tight).
  - WARNING: can proceed but with notes (e.g., speculative decoding may hurt performance).
  - INFO: informational (e.g., "NVIDIA GPU detected → CUDA 13 recommended").

- UI patterns:
  - Toasts.
  - Inline alerts.
  - Modal errors.

Specifics:

- Network failures:
  - Toast: "Network error. Please check your connection and try again."
  - Retry with backoff.

- Auth failures:
  - Toast: "Authentication failed. Please check your token and try again."
  - Redirect to login if applicable.

- Model download failures:
  - Toast: "Model download failed. Please check your connection and try again."
  - Resume from last known position.

- Binary download failures:
  - Toast: "Binary download failed. Please check your connection and try again."
  - Retry with backoff.

- VRAM estimation failures:
  - Toast: "Could not read GPU details; VRAM estimate is approximate."
  - Continue with approximate estimate.

- Disk full:
  - Toast: "Disk full. Please free up space and try again."
  - Stop download and prompt user.

- GPU already in use:
  - Toast: "GPU already in use. Please close other applications or use a different GPU."
  - Prompt user to choose a different GPU or wait.

- Corrupted models:
  - Toast: "Model file appears corrupted. Please redownload or choose a different model."
  - Prompt user to redownload or choose a different model.

---

## Testing Strategy

### Playwright E2E Tests

- Tests for:
  - Wizard steps.
  - Model selection.
  - Error states.
  - Spawn flow end-to-end.

Details: `docs/plans/20260529-spawn-v2-backend.md` (Testing).
