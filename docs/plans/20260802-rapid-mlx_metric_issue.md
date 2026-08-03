# Rapid-MLX bug report: `/v1/status` always reports `prompt_tps`/`generation_tps` as `0.0` for standard (non-MLLM) text models

Status: confirmed reproducible, confirmed still present on latest upstream `main` (commit `a73cf8cdd38fed0028319357c838032bbe63cdd1`, 2026-08-02) as of this writing. Ready to file against https://github.com/raullenchai/Rapid-MLX using the `bug_report.yml` template.

## Summary

For any model served **without** `--enable-mllm` / without a vision scheduler active (i.e. the normal, default text-generation path — which is what `--no-mllm` explicitly selects, and what non-vision models use by default), `/v1/status` permanently reports:

```json
"generation_tps": 0.0,
"prompt_tps": 0.0
```

...even while a request is actively generating and `steps_executed` / `total_completion_tokens` are climbing normally. This makes `/v1/status` unusable for any external monitoring/dashboard tool (Grafana, custom dashboards, `llama-monitor`, etc.) that wants live throughput for a standard text-serving deployment — which is the overwhelming majority of Rapid-MLX use cases.

The MLLM (vision) serving path does **not** have this bug — it computes and reports real `prompt_tps`/`generation_tps` values correctly.

## Why this happens (root cause, traced in source)

`GET /v1/status` (`vllm_mlx/routes/health.py`) builds its response from `cfg.engine.get_stats()`, reading `generation_tps`/`prompt_tps` out of a `batch_generator` sub-dict:

```python
stats = cfg.engine.get_stats()
bg = stats.get("batch_generator")
if not isinstance(bg, dict):
    bg = {}

def _tps(key: str) -> float:
    v = bg.get(key)
    return 0.0 if v is None else v

return {
    ...
    "generation_tps": _tps("generation_tps"),
    "prompt_tps": _tps("prompt_tps"),
    ...
}
```

`BatchedEngine.get_stats()` (`vllm_mlx/engine/batched.py`) only ever populates a `"batch_generator"` key in the `stats` dict when an **MLLM scheduler** is active:

```python
def get_stats(self) -> dict[str, Any]:
    stats = {...}
    if self._mllm_scheduler:
        mllm_stats = self._mllm_scheduler.get_stats()
        stats["mllm_scheduler"] = mllm_stats
        # Promote Metal memory stats + batch_generator throughput to
        # top-level for /v1/status. Without "batch_generator" forwarded,
        # generation_tps/prompt_tps stay invisible to monitoring even
        # though the underlying counters are populated.
        for key in (
            "metal_active_memory_gb",
            "metal_peak_memory_gb",
            "metal_cache_memory_gb",
            "batch_generator",
        ):
            if key in mllm_stats:
                stats[key] = mllm_stats[key]
    elif self._engine:
        stats.update(self._engine.get_stats())   # <-- text path, no batch_generator forwarding at all

    return stats
```

The `elif self._engine:` branch — the one used for every standard (non-MLLM) text model — calls straight into `Scheduler.get_stats()` (`vllm_mlx/scheduler.py`), whose returned dict never includes a `batch_generator` key, `prompt_tps`, or `generation_tps` in any form (confirmed by reading the full method body).

The deeper reason the text path has nothing to forward: the MLLM path wraps its own `MLLMBatchStats` tracker (`vllm_mlx/mllm_batch_generator.py`) which explicitly accumulates `prompt_tokens`/`prompt_time`/`generation_tokens`/`generation_time` and exposes `prompt_tps`/`generation_tps` as computed properties. The **standard text path's** `self.batch_generator` is not a Rapid-MLX class at all — it's imported directly from upstream `mlx_lm`:

```python
# vllm_mlx/scheduler.py
from mlx_lm.generate import BatchGenerator  # noqa: E402
...
self.batch_generator: BatchGenerator | None = None
```

`mlx_lm.generate.BatchGenerator` (third-party, not Rapid-MLX code) carries no timing/throughput tracking whatsoever — no `prompt_tps`, `generation_tps`, or elapsed-time fields exist on it. So even if `batched.py`'s `elif self._engine:` branch were fixed to forward a `batch_generator` key, there is currently no underlying data source on the text path to forward — the scheduler itself doesn't track prompt/generation wall-clock time or compute tps anywhere in the non-MLLM path today. The fix has two parts:
1. Add timing/tps tracking to the standard scheduler (`Scheduler` in `scheduler.py`), similar to what `MLLMBatchStats` already does for the MLLM path.
2. Forward it into `get_stats()`'s top-level `batch_generator` key so `/v1/status` picks it up, matching the existing MLLM branch's pattern.

## Confirmed still present on latest upstream

- Locally installed version: `rapid-mlx 0.11.1` (via `uv tool install`, `vllm_mlx-0.11.1.dist-info`)
- Latest released version: `v0.11.9` (released 2026-08-02T07:13:25Z, GitHub Releases)
- Latest `main` commit: `a73cf8cdd38fed0028319357c838032bbe63cdd1` (pushed 2026-08-03T01:52:44Z)
- Fetched `vllm_mlx/routes/health.py`, `vllm_mlx/engine/batched.py`, `vllm_mlx/scheduler.py`, `vllm_mlx/mllm_batch_generator.py` directly from `main` via `raw.githubusercontent.com` and diffed the relevant methods against the locally installed 0.11.1 copies — **identical logic**, byte-for-byte matching comments included (e.g. the "Without `batch_generator` forwarded, generation_tps/prompt_tps stay invisible to monitoring" comment is present verbatim in both). The bug has not been touched between 0.11.1 and current `main`.
- No existing GitHub issue found for this (searched `prompt_tps`, `generation_tps`, `throughput status` across all issues, open and closed — only unrelated hits: #1258 "mixed 8-bit base + 4-bit Qwen3.6 sidecar is slower than baseline" and closed #476 "KV Cache Export/Import HTTP API").

## Reproduction

**Environment:**
- Rapid-MLX version: `0.11.1` (bug independently confirmed present on `main` @ `a73cf8cdd38fed0028319357c838032bbe63cdd1` / latest release `v0.11.9`)
- Hardware: MacBook, Apple M5 Max, 64 GB
- macOS: 26.5.1 (build 25F80)
- Python: 3.11.15
- Model: `nightmedia/Qwen3.5-9B-DS9-USS-Defiant-1M-q8-hi-mlx` (also separately reproduced with `mlx-community/Qwen3-0.6B-4bit` — not model-specific)

**Serve command:**
```
rapid-mlx serve nightmedia/Qwen3.5-9B-DS9-USS-Defiant-1M-q8-hi-mlx \
  --served-model-name nightmedia/Qwen3.5-9B-DS9-USS-Defiant-1M-q8-hi-mlx \
  --port 19322 --host 127.0.0.1 \
  --max-num-seqs 1 --max-concurrent-requests 1 \
  --enable-prefix-cache --cache-memory-mb 4096 --hybrid-cache-entries 4 \
  --pflash off --prefill-step-size 512 --max-tokens 32768 \
  --kv-cache-dtype int8 --kv-cache-turboquant none --no-mllm \
  --gpu-memory-utilization 0.88 \
  --tool-call-parser hermes --enable-auto-tool-choice --reasoning-parser qwen3 --no-hybrid \
  --log-level INFO
```

(Also reproduced with plain `rapid-mlx serve <model> --port <port>` and no other flags — `--no-mllm` is not required to trigger this; any model that doesn't load an MLLM/vision scheduler hits the same `elif self._engine:` branch.)

**Steps:**
1. Start the server with the command above; wait for `"Application startup complete."`
2. Send a chat completion that will generate for several seconds:
   ```bash
   curl -s http://127.0.0.1:19322/v1/chat/completions \
     -H "Content-Type: application/json" \
     -d '{
       "model": "nightmedia/Qwen3.5-9B-DS9-USS-Defiant-1M-q8-hi-mlx",
       "messages": [{"role": "user", "content": "Write a very long detailed essay (at least 1500 words) about the history and future of lighthouse engineering."}],
       "max_tokens": 1400,
       "temperature": 0.7
     }' &
   ```
3. While that request is in flight, poll `/v1/status` repeatedly (e.g. every 0.5–2s):
   ```bash
   curl -s http://127.0.0.1:19322/v1/status | python3 -m json.tool
   ```
4. Observe: `status` correctly reports `"generating"`, `steps_executed` and `total_completion_tokens` climb normally between polls (e.g. `steps_executed` rose from 154 → 1111 across ~7s of polling in one capture), but `generation_tps` and `prompt_tps` are `0.0` on every single poll, including polls taken mid-burst and immediately after the request completed.

**Actual response** (representative poll taken mid-generation):
```json
{
  "status": "generating",
  "model": "nightmedia/Qwen3.5-9B-DS9-USS-Defiant-1M-q8-hi-mlx",
  "uptime_s": 9.1,
  "steps_executed": 1111,
  "num_running": 1,
  "num_waiting": 0,
  "total_requests_processed": 2,
  "total_prompt_tokens": 49,
  "total_completion_tokens": 899,
  "generation_tps": 0.0,
  "prompt_tps": 0.0,
  "metal": { "active_memory_gb": 10.2, "peak_memory_gb": 10.2, "cache_memory_gb": 0.0 },
  "cache": { "...": "..." },
  "requests": []
}
```

**Expected:** `generation_tps` and `prompt_tps` should reflect real, non-zero instantaneous or windowed throughput while a request is actively generating on the standard (non-MLLM) engine path — matching what the MLLM/vision scheduler path already correctly reports.

## Streaming/non-streaming

Reproduced with non-streaming (`stream` unset/`false`) requests. Not yet tested with `stream: true`, but the bug is in the `/v1/status` polling endpoint, not the completion response itself, so the streaming flag on the generation request is not expected to matter — the same `get_stats()` → `/v1/status` code path is hit regardless of how the underlying completion is being streamed to the client.

## Impact

Any external tool that monitors Rapid-MLX via `/v1/status` (dashboards, Grafana via `/metrics` — same underlying gap likely applies there, since `/metrics` also has no rate/tps-style gauge for the non-MLLM path, only cumulative token counters) cannot show live or historical throughput for the default text-serving deployment. This affects the vast majority of Rapid-MLX deployments, since MLLM/vision serving is the minority use case.

## Suggested fix direction

Add basic wall-clock timing to the standard `Scheduler` class in `vllm_mlx/scheduler.py` (prompt-processing start/end and generation start/end timestamps per step or per request, accumulated similarly to `MLLMBatchStats`), compute `prompt_tps`/`generation_tps` from those accumulators, and forward them into `get_stats()`'s returned dict under a `batch_generator` key (or any key `routes/health.py`'s `/v1/status` handler is updated to read) — mirroring the pattern the MLLM path already uses in `BatchedEngine.get_stats()`. This does not require touching `mlx_lm.generate.BatchGenerator` itself; the tracking can live entirely in Rapid-MLX's own `Scheduler` wrapper around it.

## Files referenced (as of `main` @ `a73cf8cdd38fed0028319357c838032bbe63cdd1`)

- `vllm_mlx/routes/health.py` — `/v1/status` route handler, lines ~265–310
- `vllm_mlx/engine/batched.py` — `BatchedEngine.get_stats()`, lines ~2710–2737
- `vllm_mlx/scheduler.py` — `Scheduler.get_stats()` (no `batch_generator`/tps forwarding), `BatchGenerator` import from `mlx_lm.generate` at module level
- `vllm_mlx/mllm_batch_generator.py` — `MLLMBatchStats` (the tracker that exists for MLLM but has no text-path equivalent), lines ~253–288
