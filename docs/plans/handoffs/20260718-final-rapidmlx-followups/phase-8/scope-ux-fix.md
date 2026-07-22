# Phase 8B2 Scope UX Fix

**Date:** 2026-07-22
**Commit:** To be applied in Phase 8B3
**Source:** User review of Phase 8B2 screenshots

## Problem

Current Auto scope implementation is single-value radio button. On macOS, "Auto = MLX only" hides GGUF-only models (the ecosystem majority). Most finetuned/uncensored/distilled models are GGUF-only.

## Solution

Change HF_SCOPE from single-value radio to additive toggles:

- MLX and GGUF can BOTH be selected
- "All" button shows everything including NVFP4/unsupported — explicit power-user choice, not default
- Auto = platform default (macOS: MLX+GGUF, Win/Lin: GGUF)
- MLX tooltip: "Rapid-MLX native format. Faster on Apple Silicon. macOS only."
- On Windows, MLX-only models shown with macOS-only warning

## Files affected

- static/js/features/hf-browse.js — HF_SCOPE enum → multi-select flags
- static/js/features/models.js — wire new scope semantics
- static/index.html — MLX tooltip, scope button styling
- tests/ui/capture.mjs — scope UX screenshots

## Budget impact

Phase 8B3 budget increased from 50k → 60k to include this fix.

