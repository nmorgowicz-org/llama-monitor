# Model-Specific Sampling Presets Plan

Date: 2026-06-03

Goal:
- Provide model-family-specific sampling presets (wizard + preset editor).
- Bundle thinking/preserve_thinking/reasoning defaults logically.
- Make all values editable.
- Give users educational guidance without forcing a single “right” config.

Key principles:
- We suggest; users decide.
- No hidden overrides; everything reflected in visible fields.
- Backwards-compatible: new fields are optional with safe defaults.

High-level behavior:
- /api/model-defaults returns:
  - A "defaults" object (from primary preset).
  - A "presets" array (model-specific or generic fallback).
- Wizard (Step 4) and preset editor:
  - Show Mode pills for presets (e.g. "Agentic / Coding (thinking)").
  - Selecting a preset sets all matching sampler/loader fields.
  - All fields remain editable and savable.

New fields (ModelDefaults, backend):
- reasoning: bool (default false)
- reasoning_budget: Option<u64> (default None)
- reasoning_budget_message: Option<String> (default None)
- preserve_thinking: Option<bool> (default None; used notably for Qwen3.6)

Logic when thinking is enabled:
- For Qwen3.6:
  - Default presets should set:
    - enable_thinking: true
    - preserve_thinking: true
    - reasoning: true
    - reasoning_budget: 16384
    - reasoning_budget_message: "\nFinal Answer:"
- Still fully customizable.

Qwen3.6 presets:

1) Agentic / Coding (thinking) — recommended default
- temperature: 1.0
- top_p: 0.95
- top_k: 20
- min_p: 0.0
- repeat_penalty: 1.0
- presence_penalty: 0.0
- enable_thinking: true
- preserve_thinking: true
- reasoning: true
- reasoning_budget: 16384
- reasoning_budget_message: "\nFinal Answer:"

2) Creative / Roleplay (thinking)
- temperature: 1.0
- top_p: 0.95
- top_k: 20
- min_p: 0.0
- repeat_penalty: 1.0
- presence_penalty: 1.5
- enable_thinking: true
- preserve_thinking: true
- reasoning: true
- reasoning_budget: 16384
- reasoning_budget_message: "\nFinal Answer:"

3) Precise coding (thinking)
- temperature: 0.6
- top_p: 0.95
- top_k: 20
- min_p: 0.0
- repeat_penalty: 1.0
- presence_penalty: 0.0
- enable_thinking: true
- preserve_thinking: true
- reasoning: true
- reasoning_budget: 16384
- reasoning_budget_message: "\nFinal Answer:"

4) Non-thinking general
- temperature: 0.7
- top_p: 0.8
- top_k: 20
- min_p: 0.0
- repeat_penalty: 1.0
- presence_penalty: 1.5
- enable_thinking: false
- preserve_thinking: false
- reasoning: false

5) Non-thinking reasoning
- temperature: 1.0
- top_p: 0.95
- top_k: 20
- min_p: 0.0
- repeat_penalty: 1.0
- presence_penalty: 1.5
- enable_thinking: false
- preserve_thinking: false
- reasoning: false

Gemma 4 presets:

1) General
- temperature: 1.0
- top_p: 0.95
- top_k: 64
- min_p: 0.0
- repeat_penalty: 1.0
- presence_penalty: 0.0
- enable_thinking: true (if supported)

2) Creative / Roleplay
- temperature: 1.0
- top_p: 0.97
- top_k: 64
- min_p: 0.0
- repeat_penalty: 1.0
- presence_penalty: 0.0

3) Precise / Agentic
- temperature: 0.7
- top_p: 0.95
- top_k: 64
- min_p: 0.0
- repeat_penalty: 1.0
- presence_penalty: 0.0–0.5

Generic fallback (for unknown/older/RP/finetunes):

1) General
- temperature: 0.9
- top_p: 0.95
- top_k: 64
- min_p: 0.03
- repeat_penalty: 1.05
- presence_penalty: 0.0

2) Creative / Roleplay
- temperature: 1.0
- top_p: 0.97
- top_k: 100
- min_p: 0.02
- repeat_penalty: 1.1
- presence_penalty: 0.1

Implementation steps:

- Backend:
  - Extend ModelDefaults fields (reasoning, reasoning_budget, reasoning_budget_message, preserve_thinking).
  - Implement per-family presets in get_model_presets.
  - Ensure /api/model-defaults exposes all new fields.

- Wizard (Step 4):
  - Use /api/model-defaults to populate:
    - Temperature, Top-P, etc.
    - Thinking, Preserve Thinking, Reasoning, Reasoning Budget fields.
  - Show Mode pills using preset names and short descriptions.
  - Make all fields editable.

- Preset editor:
  - Call /api/model-defaults when model is known.
  - Show same Mode pills in the Sampling section.
  - Allow users to select a preset, then edit and save as their own.
  - Keep all fields individually editable.

This document is canonical for implementation of model sampling presets and reasoning integration.
