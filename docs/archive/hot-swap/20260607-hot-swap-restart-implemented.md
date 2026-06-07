# Hot-Swap / Binary Update — Implemented Design

Original plan: docs/plans/hot_swap_restart.md (moved here after implementation).

Goal: Allow users to install a new llama-server binary and restart their running
preset with it, preserving configuration.

Implemented behavior (summary):

- The existing llama.cpp pill in the nav header already:
  - Shows the current build (e.g. "llama.cpp · b9550")
  - Highlights when a newer build is available ("↑ b9600")
  - Opens a version picker modal with recent releases and install buttons.
- After installing a new binary:
  - If a local llama-server is running (Spawn session), the UI now shows a
    "Restart server" button.
  - This calls POST /api/llama/restart, which:
    - Uses the saved ServerConfig (captured before stop)
    - Calls stop_server() (kills existing process, clears child/metrics)
    - Calls start_server() with the same config; because the update rewrites
      llama_server_path in-place, the restart immediately uses the new binary.
- No new AppState tracking structs are used; existing llama-binary endpoints
  and the new restart endpoint are sufficient.

Key API endpoints involved:

- GET  /api/llama-binary/version   — current installed build
- GET  /api/llama-binary/latest    — latest available build
- GET  /api/llama-binary/releases  — list recent releases
- POST /api/llama-binary/update    — install a specific tag
- POST /api/llama/restart          — restart running server with current binary

Notes (vs. original plan):

- No LlamaCppUpdateStatus struct: the previous attempt added unnecessary tracking;
  the existing llama-binary endpoints already handle version info and installs.
- No separate version-check endpoint: --print-model-metadata misuse was removed.
- Restart behavior:
  - Does not rebuild config from preset; it preserves the exact runtime config.
  - Intentionally simple: kill → sleep(1s) → restart.
- Chat and session persistence unaffected: sessions.json and chat.db are never touched.
