# New Inference Metrics Implementation Guide

Date: 2026-04-21

This guide is a handoff for implementing the next generation of inference metrics in Llama Monitor. It summarizes what was learned from polling a live `llama-server`, what data is actually exposed, and how to turn that data into premium, modern, high-impact visuals without overstating metric accuracy.

## Context

The live endpoint tested was:

```text
http://192.168.2.16:8001
```

The relevant endpoints were:

```text
GET /
GET /slots
GET /metrics
```

Observed `/slots` data included:

- `id`
- `n_ctx`
- `speculative`
- `is_processing`
- `id_task`
- `params`
- `params.n_predict`
- `params.max_tokens`
- `params.samplers`
- `params.speculative.*`
- `next_token[0].n_remain`
- `next_token[0].n_decoded`
- `next_token[0].has_next_token`
- `next_token[0].has_new_line`

Observed `/metrics` data included useful Prometheus values such as:

- `llamacpp:prompt_tokens_seconds`
- `llamacpp:predicted_tokens_seconds`
- `llamacpp:n_tokens_max`

The major limitation is that llama-server HTTP did not expose live current context usage for this setup. The server console logs show values such as `n_tokens`, `n_past`, prompt eval timing, and generation eval timing, but `/slots` did not expose those same live context fields.

## Naming Rules

Use names that describe what the UI truly knows.

Preferred labels:

- `Output tokens`
- `Output tokens remaining`
- `Output budget`
- `Prompt throughput`
- `Generation throughput`
- `Live generation estimate`
- `Context capacity`
- `Peak observed`
- `Live usage not exposed`
- `Speculative decoding`
- `Slot activity`

Avoid misleading labels:

- Do not call `next_token.n_decoded` total context usage.
- Do not call `llamacpp:n_tokens_max` live context usage.
- Do not call retained Prometheus averages live throughput unless they are actively updating.
- Do not call output-limit progress context-window progress.

## Visual Direction

The UI should feel modern, premium, and alive, but it should stay lightweight in the browser.

Use:

- CSS transforms and opacity transitions instead of layout-heavy animation.
- Inline SVG sparklines for lightweight charts.
- CSS gradients, masks, and subtle glow states for active inference.
- Small status chips with precise wording.
- Motion only during active requests.
- Reduced-motion fallbacks using `prefers-reduced-motion`.
- Compact diagnostic popovers instead of always-visible explanatory text.

Avoid:

- Heavy chart libraries unless the app already depends on one.
- Constant full-page animations while idle.
- Large canvas/WebGL effects for basic metrics.
- Repeating the same unavailable state in multiple places.
- Making inference-only remote endpoints show host System/GPU panels.

## 1. Request Activity Timeline

### Data Sources

Use:

- `/slots[].is_processing`
- `/slots[].id_task`
- Optional request counters from `/metrics` if present.

### Visual

Add a thin activity rail beneath the inference cards. It should show:

- idle gaps
- prompt/setup windows when a task begins
- active generation windows
- task completion markers

Design:

- Horizontal segmented timeline.
- Active segments glow softly.
- Completed requests leave faint marks for the last 1-5 minutes.
- Hovering or focusing a segment shows task id, duration, and output tokens when known.

### Implementation Notes

Maintain a small in-memory ring buffer:

```js
window.requestActivity = [
  { taskId, startedAtMs, endedAtMs, state, outputTokens }
];
```

Create a new segment when `id_task` changes or `is_processing` transitions from false to true.

## 2. Prompt vs Generation Phase Indicator

### Data Sources

Use:

- `/slots[].is_processing`
- `/slots[].next_token[0].n_decoded`
- `/metrics` prompt/generation throughput gauges

### Visual

Add a two-stage phase indicator:

```text
Prompt ingest | Output generation
```

Design:

- Segmented pill.
- Prompt segment pulses briefly when a new task starts and `n_decoded` is still near zero.
- Generation segment becomes active once `n_decoded` increases.
- Idle state should be subdued, not blank.

### Accuracy Guardrail

This is an inferred phase indicator unless llama-server exposes a direct phase field. Label it internally and in tooltips as inferred.

## 3. Live Output Velocity Sparkline

### Data Sources

Use:

- `/slots[].next_token[0].n_decoded`
- local polling timestamps

### Visual

Add a live sparkline for generation velocity.

Design:

- Inline SVG, 60-120 recent samples.
- Bright cyan/green line while active.
- Soft fill beneath the line.
- Peak dot for highest recent rate.
- Fade to retained state when generation stops.

### Implementation Notes

Compute live rate from deltas:

```js
const deltaTokens = currentDecoded - previousDecoded;
const deltaSeconds = (nowMs - previousMs) / 1000;
const liveOutputTokensPerSec = deltaSeconds > 0 ? deltaTokens / deltaSeconds : 0;
```

Reset the series when `id_task` changes.

This should be prioritized because it is more live than the Prometheus generation throughput gauge.

## 4. Completion Progress Ring

### Data Sources

Use:

- `/slots[].next_token[0].n_decoded`
- `/slots[].next_token[0].n_remain`
- `/slots[].params.n_predict`
- `/slots[].params.max_tokens`

### Visual

Replace or enhance the current generation progress bar with an output-budget progress ring.

Design:

- Circular progress ring around output token count.
- Center text: `234 output tokens`
- Secondary text: `31,766 remaining`
- Small task id below.
- Ring animates only while active.

### Accuracy Guardrail

Label this as output budget progress. It is not context usage.

## 5. Slot Occupancy Grid

### Data Sources

Use:

- `/slots[]`
- `id`
- `is_processing`
- `id_task`
- `n_ctx`
- `next_token[0].n_decoded`
- `next_token[0].n_remain`

### Visual

Add a compact slot grid. For a one-slot server it should still look intentional, but it becomes more useful for multi-slot servers.

Design:

- One tile per slot.
- Active tile has a subtle animated border.
- Idle tile is matte and quiet.
- Tile content:
  - slot id
  - task id
  - active/idle state
  - output tokens
  - context capacity

### Implementation Notes

Keep this below or beside the main inference cards. Do not let it dominate the page when there is only one slot.

## 6. Speculative Decoding Badge And Efficiency Panel

### Data Sources

Use:

- `/slots[].speculative`
- `/slots[].params["speculative.n_max"]`
- `/slots[].params["speculative.n_min"]`
- `/slots[].params["speculative.p_min"]`
- `/slots[].params["speculative.type"]`
- `/slots[].params["speculative.ngram_size_n"]`
- `/slots[].params["speculative.ngram_size_m"]`
- Optional `/metrics` draft/accepted counters if exposed in future.

### Visual

Add a compact speculative decoding chip.

Example:

```text
Speculative · ngram_map_k · n_max 48
```

Expanded panel:

- type
- n_max
- p_min
- ngram sizes
- acceptance rate if available

### Accuracy Guardrail

If accepted draft counters are not exposed over HTTP, show configuration only. Do not infer acceptance rate from console-only data.

## 7. Sampler Stack Visualization

### Data Sources

Use:

- `/slots[].params.samplers`
- sampler-related params such as `top_k`, `top_p`, `min_p`, `temperature`, `dry_*`, `xtc_*`

Observed sampler list:

```json
["penalties", "dry", "top_n_sigma", "top_k", "typ_p", "top_p", "min_p", "xtc", "temperature"]
```

### Visual

Add a compact sampler pipeline strip.

Design:

- Small ordered chips connected by a thin line.
- Active request gives the strip a subtle traveling highlight.
- Hover/focus opens a popover with important values.

### UX Notes

Keep it secondary. This is useful context, not the primary performance metric.

## 8. Context Capacity Gauge With Honesty Layer

### Data Sources

Use:

- `/slots[].n_ctx` for capacity.
- `/metrics` `llamacpp:n_tokens_max` for largest observed token count, if present.
- future live fields only if llama-server exposes them.

### Visual

Keep a layered context gauge:

- Full rail: context capacity.
- Historic fill: peak observed.
- Live fill: shown only when live usage is actually available.

Preferred visible wording when live usage is missing:

```text
Live usage: not exposed by llama-server
Peak observed only
capacity 212,992 · peak 121,713
```

### Accuracy Guardrail

`llamacpp:n_tokens_max` is historic peak observed, not current live usage.

## 9. Recent Task Summary

### Data Sources

Use local tracking from `/slots` transitions:

- `id_task`
- `is_processing`
- `next_token[0].n_decoded`
- local start timestamp
- local end timestamp

### Visual

Add a recent completion card or strip.

Example:

```text
Last task 2667 · 182 output tokens · ~4.1s · ~44.4 t/s estimated
```

Design:

- Small horizontal card under generation metrics.
- Slides in when a task completes.
- Uses retained/faded styling after a few seconds.

### Implementation Notes

This bridges the gap between the llama-server console timing block and what HTTP can expose.

The duration and rate are monitor-estimated from polling, so label the rate as estimated or live estimate.

## 10. Health And Capability Matrix

### Data Sources

Use existing backend state:

- inference available
- host metrics available
- remote agent connected
- remote agent health reachable
- slots available
- metrics available
- generation progress available
- context live usage available

### Visual

Add a diagnostic popover behind the endpoint/status chip.

Example content:

```text
Inference: live
Slots: live
Generation progress: live
Throughput: retained average + live estimate
Context capacity: live
Context usage: not exposed
Host metrics: unavailable
Remote agent: disconnected
```

Design:

- Small glassy popover.
- Capability rows with tiny status LEDs.
- No large explanatory text in the main dashboard.

### UX Notes

This is the best place to explain why System/GPU panels are hidden for inference-only remote endpoints.

## Backend/API Recommendations

Add or refine fields in the internal API so the frontend does not need to infer too much:

```rust
generation_live_tokens_per_sec_estimate
generation_live_tokens_per_sec_estimate_available
slot_generation_tokens
slot_generation_remaining
slot_generation_limit
slot_generation_active
active_task_id
last_task_id
context_capacity_tokens
context_high_water_tokens
context_live_tokens
context_live_tokens_available
speculative_enabled
speculative_type
host_metrics_available
slots_available
metrics_available
```

Keep legacy fields only for compatibility, and avoid serializing misleading aliases where possible.

## Frontend State Recommendations

Track these lightweight client-side buffers:

```js
window.slotSnapshots = new Map();
window.liveOutputSeries = [];
window.requestActivity = [];
window.recentTasks = [];
window.metricCapabilities = {};
```

Keep buffers bounded:

- sparklines: 60-120 samples
- activity timeline: 5-10 minutes or 100 segments
- recent tasks: 5-10 tasks

## Priority Order

1. Live output velocity sparkline from `n_decoded` deltas.
2. Recent task summary from task start/end transitions.
3. Completion progress ring for output budget.
4. Context capacity gauge refinement.
5. Capability matrix popover.
6. Slot occupancy grid.
7. Prompt vs generation phase indicator.
8. Request activity timeline.
9. Speculative decoding badge/panel.
10. Sampler stack visualization.

## Validation Checklist

Before shipping:

- Fresh page load with no endpoint attached should not show live data.
- Inference-only remote endpoint should not show System/GPU sections.
- Remote endpoint with no agent should clearly show inference-only status.
- Remote endpoint with agent should show host metrics only after `host_metrics_available` is true.
- Context card must not imply live context usage unless `context_live_tokens_available` is true.
- Output token visuals must not be labeled as context usage.
- Retained throughput must show a last-updated age.
- Live output estimate must reset on task id changes.
- Animations must stop or quiet down while idle.
- `prefers-reduced-motion` must disable nonessential animation.

## Final Product Goal

The finished experience should feel like a serious 2026 local AI operations dashboard:

- visually alive during inference
- calm and readable while idle
- honest about missing llama-server data
- precise about output tokens vs context tokens
- compact enough for continuous monitoring
- polished without being browser-heavy

