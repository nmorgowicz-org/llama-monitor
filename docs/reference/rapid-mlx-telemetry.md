# Rapid-MLX Telemetry

> **Status: Shipped.** Phase 6 completed. Rapid-MLX telemetry is live on the dashboard with dynamic cards that replace static llama.cpp cards when a Rapid-MLX session is active.

Llama Monitor polls Rapid-MLX for live runtime metrics and renders them as a set of dynamic dashboard cards. The telemetry system replaces the static llama.cpp inference cards when a Rapid-MLX session is active, and restores the llama.cpp cards when the session ends.

## Polling architecture

The `RapidMlxPoller` (`src/inference/rapid_mlx/poller.rs`) performs three API calls per poll cycle, spaced 200ms apart:

1. `/health` — liveness probe (2s timeout). Returns success if the server process is alive.
2. `/v1/status` — comprehensive runtime state (3s timeout, 512KB body limit). Returns model identity, health status, throughput, memory, queue state, and cumulative totals.
3. `/v1/cache/stats` — prefix cache statistics (2s timeout, 256KB body limit). Returns hit rate, entry count, and per-tier cache metrics.

The poller is registered per-session in `src/llama/poller.rs`. It matches sessions by base URL and API key using constant-time comparison. A single reqwest client is reused across all polls within a poller instance.

## Telemetry fields

The poller aggregates data into an `InferenceMetricsSnapshot` (`src/inference/metrics.rs`) with these fields:

| Field | Source | Description |
|-------|--------|-------------|
| `backend` | `InferenceBackend::RapidMlx` | Always Rapid-MLX |
| `health` | `/v1/status` → `status` | `Ok` (idle/generating), `NotLoaded`, `Degraded` |
| `ready` | `/v1/status` → `status` | Boolean readiness |
| `model` | `/v1/status` → `model` | Model identity (max 512 chars) |
| `uptime_seconds` | `/v1/status` → `uptime_s` | Server uptime |
| `generation_tokens_per_second` | `/v1/status` → `generation_tps` | Generation throughput |
| `prompt_tokens_per_second` | `/v1/status` → `prompt_tps` | Prompt throughput |
| `running_requests` | `/v1/status` → `num_running` | Active requests |
| `waiting_requests` | `/v1/status` → `num_waiting` | Queued requests |
| `completed_requests_total` | `/v1/status` → `total_requests_processed` | Lifetime request count |
| `prompt_tokens_total` | `/v1/status` → `total_prompt_tokens` | Lifetime prompt tokens |
| `completion_tokens_total` | `/v1/status` → `total_completion_tokens` | Lifetime completion tokens |
| `steps_executed` | `/v1/status` → `steps_executed` | Generation steps |
| `global_cache_hit_rate` | `/v1/status` → `cache.hit_rate` | Prefix cache hit rate |
| `global_cache_entries` | `/v1/status` → `cache.entry_count` | Active cache entries |
| `active_memory_bytes` | `/v1/status` → `metal.active_memory_gb` | Current Metal memory usage |
| `peak_memory_bytes` | `/v1/status` → `metal.peak_memory_gb` | Peak Metal memory usage |
| `cache_memory_bytes` | `/v1/status` → `metal.cache_memory_gb` | Metal cache memory |
| `cache_metrics` | `/v1/cache/stats` | Per-tier cache breakdown |
| `active_requests` | `/v1/status` → `requests` | Sanitized active request list |
| `backend_details` | `/v1/status` → `status`, `progress` | Runtime status + progress |

## Dashboard cards

The frontend (`static/js/features/rapid-mlx-cards.js`) renders up to 8 card types from a registry (`CARD_REGISTRY`). Each card has an `available()` check and a `render()` function. Cards are shown/hidden based on data availability.

### Card types

| ID | Order | Title | Availability |
|----|-------|-------|-------------|
| `runtime` | 10 | Rapid-MLX runtime | Has model + (health or ready or uptime) |
| `throughput` | 20 | Inference throughput | Has generation or prompt t/s |
| `queue` | 30 | Request queue | Has running or waiting requests |
| `memory` | 40 | Metal runtime memory | Has active, peak, or cache memory |
| `cache` | 50 | Prefix & cache state | Has cache_metrics |
| `totals` | 60 | Cumulative totals | Has any lifetime metric |
| `activity` | 70 | Request activity | Has active request objects with id/request_id/status |
| `progress` | 80 | Live progress | Has backend_details.progress |

### Card rendering

Each card uses a shared `card(title, rows, state)` helper that produces a `.widget-card.rapid-telemetry-card` with:
- A `.metric-card-topline` containing the heading and a live/stale chip
- A `.rapid-metric-list` containing metric rows (`<div.rapid-metric>`)
- A live chip with state classes: `live`, `idle`, `active`, `degraded`, `not ready`

Progress cards use a special `.rapid-progress` element with an ARIA-compliant progress bar.

### Card synchronization

Cards are synchronized efficiently using DOM diffing:
- `syncCard()` compares existing and new card elements
- `syncCardBody()` diffs metric rows by label text
- `syncProgressRow()` updates progress bar fill width
- Cards that no longer meet availability are removed
- Cards that become available are inserted

### Stale detection

Cards track poll failure history:
- `STALE_POLLS = 3` — after 3 consecutive poll failures, a card is marked stale
- `cardHistory` map stores per-card sample + missing count
- Stale cards show a stale chip with "stale · 5s ago · 1/3" format
- Stale data is retained until 3 consecutive misses clear the history

### Session isolation

Cards are scoped per-session:
- `cardHistory.clear()` is called on session change
- `lastSessionId` tracks the current session
- `parkLlamaCards()` hides llama.cpp cards when Rapid-MLX is active
- `restoreLlamaCards()` shows llama.cpp cards when Rapid-MLX ends
