# Spawn Llama-Server V2 — Frontend Architecture and Wizard UI

- **Branch:** `feature/spawn-llama-server-v2`
- **Date:** 2026-05-29
- **Author:** Iris (via Hermes)

This is the frontend implementation reference for the Spawn Llama-Server V2 feature.
It defines the wizard architecture, UI flows, data/API contracts, error handling, and testing strategy.

For a concise, self-contained reference (optimized for fresh AI agents), see:
- `docs/plans/20260529-spawn-v2-reference.md`

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
  - Defines steps (Profile → Model → Hardware → Summary → Spawn).
  - Manages step state and transitions.
  - Integrates with VRAM estimation, MoE tuning, HF model selection, and health checks.
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
     - "Quick": auto-tune based on hardware and model; minimal user input.
     - "Balanced": guided tuning with sensible defaults and inline help.
     - "Advanced": full control over all parameters.
   - Inline help: short description of each profile.
   - Integration:
     - Optionally pre-select based on user history (e.g., always uses Advanced).

2. **Model:**
   - Select model via one of:
     - Local directory (file browser; GGUF filter).
     - HuggingFace:
       - Input HF repo ID.
       - List available GGUF files (via `/api/models/download` or HF-related endpoints).
       - Optionally download or use `-hf` if supported.
     - Third-party import (Ollama, LM Studio, etc.):
       - Provide hints/paths; user selects existing GGUF.
   - Inline help: short description of each option.

3. **Hardware:**
   - GPU layers:
     - Auto / All / Manual.
   - Context size, batch sizes, parallel slots.
   - KV cache quantization options (if VRAM is tight).
   - VRAM estimator feedback inline:
     - Call `/api/estimate-vram` with current settings.
     - Show Fit/Tight/Risk/Won’t fit.
   - MoE tuning assistance (if applicable):
     - Expose `--n-cpu-moe` or equivalent.
   - Inline help: short description of each field.

4. **Summary:**
   - Review all settings in a compact, readable view.
   - "Save as Preset" option:
     - Capture current config as a new preset.
   - "Run Health Check" button:
     - Call `/api/benchmark` on the target server.
   - Inline help: short description of each field.

5. **Spawn:**
   - "Spawn Server" button:
     - Sends final config to `/api/sessions/spawn` or `/api/start` depending on context.
   - Progress bar and status messages:
     - Show spawn progress and key events.
   - Inline help: short description of each step.

### State Management

The wizard maintains a state object:

- `profile`: "quick" | "balanced" | "advanced"
- `model`:
  - `path`: local model path (if local).
  - `hfRepo`: HF repo ID (if HF-based).
  - `name`, `size`, `quant`.
- `hardware`:
  - `gpuLayers`, `contextSize`, `batchSize`, `ubatchSize`
  - `parallelSlots`
  - `cacheTypeK`, `cacheTypeV`
  - `moeTuning` (e.g., `n_cpu_moe`)
- `safety`:
  - `maxTokens`
  - `apiKey`
- `multimodal`:
  - `mmproj`
- `summary`:
  - `presetName`
  - `healthCheckResults`
- `spawn`:
  - `status`, `progress`, `error`

This state is:

- Used to drive the wizard UI.
- Mapped to a `ServerConfig`-compatible payload and sent to `/api/sessions/spawn` or `/api/start`.
- Optionally persisted in localStorage for quick recovery.

### Data and API Contract

The wizard must integrate with the following backend endpoints (see backend doc for details):

- Model selection:
  - `/api/models` and `/api/models/refresh` for local models.
  - `/api/browse?path=...&filter=gguf` for file browser.
- HF integration:
  - Endpoints under `/api/models/download` and HF-related endpoints for listing/downloading GGUF.
- VRAM estimation:
  - `POST /api/estimate-vram` in Step 3 to validate configuration.
- Health check:
  - `POST /api/benchmark` in Step 4 to run a short benchmark.
- Spawn:
  - `POST /api/sessions/spawn` (db-admin-token) or `POST /api/start` (api-token) in Step 5 with the final config.

Rules:
- Never hard-code internal backend fields; rely on documented API schemas.
- Use `authHeaders()` utility for token-based auth.
- On auth failure:
  - Show toast and optionally redirect to login.

### Integration with Existing Modals

The wizard integrates with existing modals:

- **File browser:**
  - Used for model selection in Step 2.
- **Preset selection:**
  - Used for preset selection in Step 1 and Step 4 ("Save as Preset").
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

## UI/UX Design Guidelines (Critical)

This section defines how the Spawn V2 wizard and related flows should look and behave.
These rules are mandatory. A fresh agent must follow them to ensure the UI is modern, premium, and consistent.

### 1. Design philosophy

- This is a 2026-class desktop app:
  - Premium, focused, and fast.
  - No boilerplate SaaS wizard look.
  - No clutter, no generic cards, no noisy labels.
- Principles:
  - Calm by default; powerful when needed.
  - Auto-configure everything we can; ask only when it matters.
  - Explain in 1–2 lines; never lecture.
  - Security and safety are silent by design: present only when user must act.

### 2. Visual style (must match/exceed existing app)

- Use and extend existing patterns:
  - Glassmorphism modals (agent-modal.css, modal-premium.css).
  - Subtle radial/linear gradients, blur, multi-shadow, top-line accent, border glow.
  - Step badges, hero cards, and premium buttons.
- Layout:
  - Modal width:
    - Large enough for clarity (e.g., 900–1100px), but not full-screen.
  - Two-column layout for complex steps:
    - Left: primary controls and fields.
    - Right: contextual help, hints, or live feedback (VRAM estimate, recommendations).
  - Use:
    - Clear section headings.
    - Compact form fields with inline labels.
    - Tooltips for advanced or optional fields.
- Typography:
  - Short labels; no long paragraphs.
  - Use 1–2 line descriptions max; deeper info via tooltips or “?” hints.
- Motion:
  - Subtle transitions between steps (fade/slide).
  - Respect prefers-reduced-motion:
    - Disable or reduce animations for users who prefer it.

### 3. Wizard structure and progressive disclosure

The wizard must feel guided, not overwhelming.

- Step 1: Profile
  - Show 3 options as compact cards:
    - Quick: “Fully auto-tuned. Best for most users.”
    - Balanced: “Guided tuning with sensible defaults.”
    - Advanced: “Full control over all parameters.”
  - Defaults:
    - First-time: Balanced.
    - Returning: persist last choice in localStorage.
  - Behavior:
    - Quick: hide advanced fields; auto-tune everything.
    - Balanced: show core fields (model, context, GPU layers, batch sizes).
    - Advanced: show all, with clear grouping and tooltips.

- Step 2: Model
  - Provide three clean options:
    - “Select local model”:
      - Opens existing file browser (GGUF filter).
    - “Use HuggingFace repo”:
      - Single input: repo ID.
      - On blur:
        - Call backend to fetch available GGUF files.
        - Show a compact list with:
          - Filename
          - Size
          - Quant label (if obvious)
        - Highlight recommended quant (e.g., Q4_K_M or Q8_0) based on VRAM.
    - “Import from another tool”:
      - Short hints (e.g., “Already using Ollama or LM Studio? We can use your existing models.”).
      - Opens file browser pointed to common directories (configurable).
  - UX rules:
    - No large walls of text.
    - If gated:
      - Show a small warning chip: “Gated model – requires access.”
      - Provide a short link and 1-line instruction.

- Step 3: Hardware
  - Auto-fill:
    - GPU layers: auto/all by default.
    - Context size: model-native or safe default.
    - Batch sizes: tuned for platform.
  - UI:
    - Show a compact “Hardware summary” card:
      - Detected GPU(s).
      - Estimated free VRAM.
    - Inline VRAM feedback:
      - Use a small status pill:
        - “Fit” (green)
        - “Tight” (yellow)
        - “Risk” (orange)
        - “Won’t fit” (red)
      - On hover: 1–2 lines explaining why and what to adjust.
  - Advanced-only:
    - KV cache quantization.
    - MoE tuning sliders.
    - Multi-GPU split controls.

- Step 4: Summary
  - Show a clean, scannable summary:
    - Model
    - Key settings (context, GPU layers, batch)
    - VRAM estimate
    - Any active warnings
  - Actions:
    - “Save as Preset” (secondary).
    - “Run Health Check” (tertiary, only if a server is available).
  - UX:
    - No repetition; no redundant labels.
    - Use icons or small tags for status (e.g., “MoE”, “Vision”, “Gated”).

- Step 5: Spawn
  - Primary: “Spawn Server”
  - Behavior:
    - Show a progress area with:
      - Live status (“Starting llama-server…”).
      - Estimated time if relevant.
    - On success:
      - Short confirmation (“Server running”).
      - Auto-switch to monitor/chat view.
    - On failure:
      - Clear, short message.
      - One actionable suggestion if possible.

### 4. Automation and simplification (non-negotiable)

We must aggressively reduce user effort.

- Auto-detect and auto-configure:
  - GPU type and VRAM:
    - Recommend backend (CUDA/ROCm/Metal/Vulkan).
    - Recommend GPU layers and batch sizes.
  - Model metadata:
    - Use introspection and HF metadata to:
      - Set context size.
      - Enable MoE tuning UI when MoE is detected.
      - Suggest mmproj when multimodal is detected.
  - Use model-specific generation defaults:
    - Auto-fill temperature, top_p, etc. based on model family.

- Smart defaults:
  - If VRAM is tight:
    - Suggest a smaller context size or KV quantization (Balanced/Advanced).
  - If model is MoE:
    - Pre-set a reasonable --n-cpu-moe.
  - If user selects HF repo:
    - Pre-select a recommended quant based on VRAM.

- Minimal friction:
  - No unnecessary confirmations.
  - No “Are you sure?” for non-destructive actions.
  - Destructive actions (e.g., deleting a preset, restoring DB) must:
    - Use a short confirmation dialog with a clear label.

### 5. Error handling and messaging

Rules:

- Never:
  - Show raw stack traces.
  - Leak tokens, keys, or internal paths.
  - Use vague messages like “Something went wrong.”
- Always:
  - Use short, human-readable messages.
  - Offer 1 concrete action when possible.

Patterns:

- Network errors:
  - “Connection error. Checking your network and trying again.”
- Auth failures:
  - “Authentication failed. Please check your token or log in.”
- VRAM issues:
  - “Not enough VRAM for this configuration. Try reducing context size or using a smaller quantization.”
- Gated model:
  - “This model is gated. Visit it on HuggingFace to request access, then provide your HF_TOKEN.”
- Corrupted/invalid model:
  - “Model file appears corrupted. Try downloading it again or selecting another model.”

### 6. Security-aware UI/UX

The UI must reinforce, not weaken, our security posture.

- Tokens and keys:
  - Never log or echo full tokens.
  - Mask HF_TOKEN and API keys in UI (e.g., “hf_****”).
- Auth flows:
  - If an operation requires db-admin-token:
    - If user is not authenticated:
      - Prompt for auth in a clean modal.
    - If auth fails:
      - Show a short, neutral error; never reveal internal token details.
- Path safety:
  - File browser:
    - Only show allowed roots.
    - No raw path inputs where a controlled selector exists.
- Spawn safety:
  - Do not allow arbitrary command input in Quick/Balanced.
  - Advanced mode:
    - Allow extra_args, but:
      - Show a small warning: “Custom flags can affect stability and security.”

### 7. Accessibility and polish

- Accessibility:
  - Keyboard navigation:
    - All wizard steps and controls must be reachable via Tab.
  - Reduced motion:
    - Provide @media (prefers-reduced-motion) overrides.
- Polish:
  - Consistent button hierarchy:
    - Primary: main action (e.g., “Next”, “Spawn Server”).
    - Secondary: “Save as Preset”, “Edit”.
    - Tertiary: “Run Health Check”, “Advanced settings”.
  - Micro-interactions:
    - Subtle hover lifts and glows on interactive elements.
    - Smooth transitions between steps.

These guidelines must be treated as first-class requirements.
If an implementation choice conflicts with “premium, automated, secure,” prefer:
- Simpler for the user.
- Stronger security.
- Higher visual quality.

---

## Testing Strategy

### Playwright E2E Tests

- Tests for:
  - Wizard steps.
  - Model selection.
  - Error states.
  - Spawn flow end-to-end.

Details: `docs/plans/20260529-spawn-v2-backend.md` (Testing).
