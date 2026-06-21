# Speed-Bench Integration for Speculative Decoding

**Date:** 2026-06-19
**Status:** Proposed / Exploratory
**Priority:** Medium
**Scope:** Benchmarking tooling, speculative decoding helpers, model-specific guidance

This is an exploratory/feasibility plan, not a full implementation spec. The goal is to:

- Assess whether llama.cpp's speed-bench tool can be integrated into llama-monitor.
- Understand how easy/automated it could be for users to compare speculative decoding methods.
- Surface the complexity for different model families (Qwen3.6 MTP, Gemma4, EAGLE3).

## Background

llama.cpp PR #23869 introduced `tools/server/bench/speed-bench/`:

- `speed_bench.py` — runs a SPEED-Bench split against a running `llama-server` via its OpenAI-compatible API, prints per-category summary (throughput, latency, accept rate), optionally saves JSON.
- `speed_bench_compare.py` — compares two JSON runs (baseline vs speculative) to show decode and latency speedups.

Key properties:

- Does not launch llama-server; it talks to whatever is already running.
- Uses the SPEED-Bench dataset (nvidia/SPEED-Bench) via the Hugging Face `datasets` library.
- Python dependencies: `datasets`, `requests`, `tqdm` (very light).
- Fully independent of llama.cpp source tree — only requires a llama-server endpoint.

This means:

- We can run it using the llama.cpp binary we already manage.
- We do not need to bundle llama.cpp source.
- We can treat it as an external benchmark harness that we optionally integrate and wrap.

## Why This Matters

Speculative decoding (SD) is becoming a first-class knob for performance, but:

- Its benefit depends heavily on:
  - Model (MTP heads vs draft model vs n-gram).
  - Data type (coding, math, long context, chit-chat).
  - System constraints (batch, ubatch, context, GPU).
- Right now:
  - Users rely on anecdotal feels or a single short prompt.
  - Our current `/api/benchmark` is a single-shot test that is useful for basic tuning, but not for evaluating SD trade-offs across domains.
- We want:
  - A structured way for users to "try" different SD methods on their setup.
  - Per-category visibility so they can see where SD wins/loses.
  - A frictionless baseline vs spec comparison.

## Feasibility

### Can We Use speed-bench As-Is?

Yes, in principle:

- It:
  - Loads samples from SPEED-Bench via Hugging Face.
  - Sends them as chat completions requests to a given URL.
  - Reads `timings` fields from llama.cpp responses (draft_n, draft_n_accepted, prompt_ms, predicted_ms, etc.).
- We already:
  - Control llama-server launch and config.
  - Provide an OpenAI-compatible endpoint.
  - Know about speculative decoding flags (MTP, EAGLE3, draft models, n-gram, etc.).

So conceptually:

1. User starts a model.
2. We run speed-bench against it as baseline.
3. We modify the speculative config (or prompt the user to).
4. We run speed-bench again.
5. We compare.

Obstacles/considerations:

- Python runtime:
  - speed-bench requires Python plus a couple of libraries.
  - On many systems (especially macOS, Linux, dev machines), Python is available; on consumer setups or Windows without Python, it's not.
  - Options:
    - Require Python:
      - Pros: reuse official tool, easy to keep in sync.
      - Cons: extra dependency, harder to market as "1-click".
    - Ship a bundled Python runtime:
      - Pros: self-contained, consistent environment.
      - Cons: heavier packaging and maintenance.
    - Rewrite core logic in Rust:
      - Pros: aligns with our stack, no Python required.
      - Cons: more work; we must reimplement dataset loading and concurrency.
- Dataset loading:
  - Uses `datasets` library from Hugging Face, downloads SPEED-Bench on first run.
  - This is fine for most users, but:
    - Behind strict firewalls or in offline environments, this can fail.
    - We should handle dataset download failure gracefully (fallback, explanation).
- Runtime length:
  - Full runs can be minutes (many samples, many tokens).
  - Our current `/api/benchmark` is a fast, 1-shot test.
  - speed-bench is not a drop-in replacement; it is:
    - More expensive.
    - Much more informative for SD decisions.
  - We should position it clearly as a heavier, optional, specialized benchmark.

### High-Level Conclusion

- Integration is feasible.
- It can run independently against our managed llama-server.
- The main challenge is not correctness, but:
  - User experience (Python requirement).
  - Framing this as a deliberate, structured process, not something that runs accidentally.

## Desired UX (Exploratory)

We don't want users to run Python commands in their terminal. We want something like:

1. User is in Server or Tuning view with a model loaded.
2. They open a "Speculative Decoding" or "Benchmark" area.
3. They see:
   - Current config (SD method, draft model, etc.).
   - A "Run SPEED-Bench Comparison" button (or similar).
4. Tool does:
   - Checks: Python available? Dataset accessible? Server running?
   - Runs baseline:
     - Starts speed-bench against current config (no or minimal speculative decoding).
   - Optionally switches config:
     - For MTP models: enables/specifies MTP config.
     - For EAGLE3/draft models: uses configured draft model.
   - Runs speculative run:
     - Uses same samples/categories.
   - Compares:
     - Uses speed_bench_compare.py.
     - Shows per-category table:
       - Decode speedup.
       - Latency speedup.
       - Accept rate.
5. Output:
   - Clear table: "SD helps a lot for coding/math, hurts for writing" style.
   - Recommendation:
     - "MTP is worthwhile on this model and workload."
     - "Avoid this method for your primary use case."

We should not over-automate:

- For now, keep some user control:
  - Which SD method to test.
  - Which categories to run (or limit to a quick subset).
  - Whether to auto-restart llama-server between configs.

## Model Family Considerations

This is critical for guiding users.

### Qwen3.6 (MTP heads)

- Qwen3.6 models include built-in MTP (Multi-Token Prediction) heads.
- llama.cpp supports:
  - Enabling MTP-based speculative decoding.
  - Tuning draft parameters (n-max, probability thresholds).
- For us:
  - We can detect when an active model is Qwen3.6 and has MTP support.
  - Then:
    - Offer to compare:
      - Baseline (no speculative decoding).
      - MTP enabled.
    - Optionally sweep different MTP params to see:
      - How n-max choice affects speedup vs overhead.

Automated level:

- High:
  - We know the model is Qwen3.6.
  - We know MTP is built-in.
  - We know which flags to add.
- This is a prime candidate for "Run MTP comparison" button.

### Gemma4 (Separate MTP draft models)

- Gemma4 does not include built-in MTP heads; uses separate draft models.
- llama.cpp supports:
  - `-md <draft_model.gguf>` with MTP-style speculative decoding.
- For us:
  - Harder to fully automate:
    - We must:
      - Know which draft model to use for a given Gemma4 model.
      - Ensure it is available on disk.
      - Optionally help the user find or download it.
- Possible:
  - Use Hugging Face to:
    - Search for known draft models for the main model.
    - Show candidate draft models in the UI.
    - Let user choose and optionally download.
  - Then run:
    - Baseline: without draft model.
    - Speculative: with chosen draft model and appropriate flags.

Automated level:

- Medium:
  - We can assist discovery, but user must confirm which draft model to use.

### EAGLE3

- EAGLE3 is brand new and uses:
  - Dedicated EAGLE3 models specific to each base model.
  - A distinct `--spec-type draft-eagle3` in llama.cpp.
- For us:
  - Even harder to automate:
    - We need:
      - A mapping of base model → appropriate EAGLE3 model.
      - Confidence that the model is compatible.
- Possible:
  - Start with a curated list:
    - Known EAGLE3 models for major base models.
    - Provide links and descriptions.
  - Let the user:
    - Enable EAGLE3.
    - Select or provide the model path.
    - Optionally run speed-bench comparison.

Automated level:

- Low/Medium:
  - For now: UI guidance + optional curated mapping, but user must select.
  - Later: we can improve mapping as more models appear.

### N-gram

- N-gram based speculative decoding:
  - Model-agnostic.
  - Tuned via flags (`--spec-type ngram-mod` and related).
- For us:
  - Easy to test:
    - No draft model required.
    - Only requires adjusting flags.
  - Ideal to offer as a quick comparison:
    - "Test n-gram on this model" button.

Automated level:

- High:
  - We can offer this as a simple toggle/comparison.

## Integration Options (Conceptual)

Three options, from least to most ambitious:

### Option A: Thin Wrapper (Recommended Initial Step)

We add:

- A new API endpoint or command under `/api/benchmark` or `/api/speed-bench` to:
  - Check Python availability.
  - Ensure required dependencies.
  - Invoke `speed_bench.py` with given parameters.
  - Optionally invoke `speed_bench_compare.py` and return comparison.
- UI:
  - A "Speed-Bench" section in Tuning or Benchmark.
  - Fields:
    - Bench type (qualitative, throughput_1k, etc.).
    - Categories (all or subset).
    - OSL (max_tokens).
    - Concurrency/limit.
    - Mode: baseline-only, or baseline+spec comparison.
- Behavior:
  - For a comparison:
    - Optionally restart llama-server with the specified speculative config.
    - Run second pass.
    - Show comparison table.

Pros:

- Direct use of upstream tool; easy to update.
- Limited new code; mostly orchestration.
- Fits within existing Tuning/Benchmark workflow.

Cons:

- Python dependency.
- User must be aware this is "heavy".

### Option B: Bundled Python Runtime

We ship a small, self-contained Python environment specifically for speed-bench.

Pros:

- No user-side Python requirements.
- Fully controlled.

Cons:

- Adds packaging complexity and size.
- Slightly overkill for one benchmark harness.

### Option C: Rust Rewrite

We reimplement the core in Rust:

Pros:

- Fits our stack.
- No Python.
- Easier to integrate, stream results, add auth/limits.

Cons:

- More upfront work.
- We must mirror dataset fetching, concurrency, and compare logic.

Given our current priorities, Option A is the natural first step, with Option C as the eventual target if this proves valuable.

## Open Questions

- Should we:
  - Always require explicit user confirmation before running (recommended).
  - Provide a "quick" preset (e.g., `qualitative` with `--limit 5–10`) for faster runs.
  - Gate behind a setting so it's not visible to users not interested in SD tuning.
- How to present results:
  - Inline table in Tuning panel.
  - Per-category color chips (good/bad/neutral).
  - Recommendation summary (e.g., "MTP: recommended for this workload").
- How to coordinate with existing `/api/benchmark`:
  - Keep `/api/benchmark` as a fast, single-shot test.
  - Treat speed-bench as an advanced, multi-sample comparison tool.

## Next Steps (Exploratory)

- Validate:
  - Can speed-bench run against our current llama-server version with required fields (`timings`, draft stats, etc.) present.
  - Confirm latency and token counts are accurate with our server config.
- Prototype:
  - Implement a basic endpoint that:
    - Runs speed-bench with user-specified parameters.
    - Streams or logs progress.
    - Returns JSON summary.
- Add:
  - UI wiring in a Tuning/Benchmark section.
  - Model-aware suggestions:
    - For Qwen3.6: "Test MTP speculative decoding".
    - For Gemma4: "Find and compare a MTP draft model".
    - For EAGLE3: "Try EAGLE3 if a compatible draft model is available".
- Decide:
  - Whether we commit to Option A long-term or start Option C.
