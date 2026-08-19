# Phase 6 — Rapid-MLX Long-Context Cache Benchmarking
## Evidence-first benchmark and policy plan
 
## 2026-07-26 workspace calibration evidence

All rows below use the Rapid-MLX 0.11.0 source build with its upstream active
KV quantization fix, a 512-token text prefill step, one sequence, PFlash off,
`--hybrid-cache-entries 16`, and `--kv-disk-checkpoint-interval 0`.

| Workspace target | Active KV | Retained cap | Cold TTFT | Fork TTFT | Saved on fork | Entries / retained bytes | Metal peak |
|---:|---|---:|---:|---:|---:|---:|---:|
| 200K (203,533-token fork) | INT4 | 16 GiB | 317.85 s | 3.14 s | 202,997 tokens | 2 / 8.16 GiB | 33.47 GB |
| 160K (161,971-token fork) | INT4 | 16 GiB | 216.78 s | 2.07 s | 161,434 tokens | 2 / 6.61 GiB | 31.03 GB |
| 131K (134,741-token fork) | INT4 | 16 GiB | 117.88 s | 1.60 s | 134,205 tokens | 2 / 5.66 GiB | 29.43 GB |

The cold/repeat/follow-up operations remain in the receipt because they build
a real multi-turn agent history. The decisive branch operation is the fork:
it restores the usable shared history and reduces first-token latency by about
101x at 200K, 105x at 160K, and 74x at 131K. All final forks had no
evictions; the 131K run left about 9.47 GiB OS free.
Receipt: `tests/fixtures/calibration/rapid-mlx-receipts/unsloth-qwen36-35b-workspace-cache-200k-16g-int4-v1/00-00-cache-200000-ram-16384-int4.json`.

### 2026-07-27 retained-capacity matrix

The same real multi-turn/fork workload was repeated at 160K and 200K for both
active KV dtypes and 8/16 GiB retained-cache caps. Fork TTFT and saved-token
totals stayed effectively constant; the cap determines how many older
boundaries survive, not whether the newest fork is fast.

| Context | Active KV | Cap | Fork TTFT | Fork saved | Final evictions | Retained at fork | Metal peak |
|---:|---|---:|---:|---:|---:|---:|---:|
| 160K | INT8 | 8 GiB | 1.89 s | 161,951 | 5 | 6.83 GiB | 34.27 GB |
| 160K | INT8 | 16 GiB | 1.95 s | 161,951 | 0 | 12.03 GiB | 36.20 GB |
| 160K | INT4 | 8 GiB | 1.84 s | 161,434 | 0 | 6.61 GiB | 31.03 GB |
| 160K | INT4 | 16 GiB | 1.82 s | 161,434 | 0 | 6.61 GiB | 31.03 GB |
| 200K | INT8 | 8 GiB | 2.34 s | 203,905 | 6 | 6.40 GiB | 34.87 GB |
| 200K | INT8 | 16 GiB | 2.21 s | 203,905 | 0 | 15.00 GiB | 39.53 GB |
| 200K | INT4 | 8 GiB | 2.29 s | 202,997 | 3 | 6.91 GiB | 33.47 GB |
| 200K | INT4 | 16 GiB | 2.17 s | 202,997 | 0 | 8.16 GiB | 33.47 GB |

Recommendation from this workload: make 8 GiB the evidence-backed starting
cache budget. It preserves the newest fork at every tested point, but it
evicts older branches for INT8 at 160K/200K and INT4 at 200K. Offer 16 GiB as
the branch-retention option, not as a speed preset; it produced no meaningful
newest-fork TTFT win and costs additional unified memory. The receipt set is
`tests/fixtures/calibration/rapid-mlx-receipts/unsloth-qwen36-35b-workspace-cache-capacity-160k-200k-v1/`.

### 2026-07-27 disk-checkpoint write cost

The isolated 200K INT4/8 GiB workspace workload compared the performance-lane
control (`--kv-disk-checkpoint-interval 0`) with `8192`. Both reached the
same final fork behavior: 203,533 prompt tokens, 402,839 cumulative saved
tokens, 3 evictions, and 7.41 GiB retained. The interval therefore did not
create a new reusable disk tier or improve branch reuse.

| Interval | Cold TTFT | Repeat TTFT | Follow-up TTFT | Fork TTFT | Metal peak |
|---:|---:|---:|---:|---:|---:|
| 0 | 231.36 s | 287.83 s | 307.49 s | 2.29 s | 33.47 GB |
| 8192 | 287.85 s | 321.83 s | 319.12 s | 2.88 s | 33.80 GB |

At this context, automatic checkpoint writes added 56.5 s (24%) to cold TTFT
and also slowed repeat/follow-up. Keep interval `0` for all interactive
performance/cache recommendations. The `8192` setting remains a separate
snapshot-write/persistence capability, pending the manual export → restart →
import → follow-up qualification; it must never be described as automatic
disk-backed eviction recovery. Receipts:
`tests/fixtures/calibration/rapid-mlx-receipts/unsloth-qwen36-35b-workspace-cache-disk-200k-int4-8g-v1/`.

### 2026-07-27 llama.cpp comparison boundary

The companion Qwen 3.6 35B Q4_K_M GGUF one-slot experiment found 0.548/0.673/
0.784 s forks at 131K/160K/200K with `-cram 0`, versus 0.541/0.637/0.699 s
with `-cram 8192`. It confirms excellent live-slot reuse in both cases, but
does not qualify `-cram 8192`: no branch was displaced, so the optional host
cache never had to restore one. Keep unified-memory one-user llama Auto=`0`
and treat multi-branch cache pressure as a later server-workload experiment.
See [`cache-benchmark-results.md`](../reference/cache-benchmark-results.md) for the complete cross-backend
evidence and policy.
**Prepared:** July 26, 2026  
**Target runtime:** Rapid-MLX `0.11.0+git.5fc6556c` source build for initial evidence; repeat the compact qualification set when a tagged release contains the live-KV fix.  
**Target hardware:** Apple Silicon MacBook Pro M5 Max, 64 GB unified memory  
**Primary workload:** One OpenCode client using an OpenAI-compatible endpoint for long-running coding-agent sessions  
**Existing benchmark coverage:** TG, PP, TTFT, total latency, and RAM for BF16 / INT8 / INT4 KV at approximately 32K, 65K, 131K, 160K, and 200K contexts

**Candidate models:**

1. Qwen3.6-27B, 4-bit or MXFP4 weights
2. Qwen3.6-35B-A3B, 4-bit or MXFP4 weights
3. Gemma 4 31B, preferably a QAT-derived MLX quant
4. Gemma 4 26B-A4B, preferably a QAT-derived MLX quant

Gemma 4 31B QAT is deliberately deferred until the cache protocol is operational. The initial Phase 6 model track uses the paired Gemma 4 26B-A4B LM/VLM conversions; the 31B joins only when it can receive matched cold and cache evidence.

---

# 1. Purpose

Extend the existing standardized inference benchmark suite so it can evaluate **Rapid-MLX prefix-cache behavior**, not only cold prompt processing and decode performance.

The resulting dataset should support an application-level recommendation engine that can answer:

- Will a selected model and KV dtype fit on a machine with a given amount of unified memory?
- At a requested context length, can at least one useful reusable prefix checkpoint fit?
- How many useful entries or message boundaries can remain resident?
- Does Rapid-MLX avoid a full prompt-processing scan during a growing OpenCode conversation?
- What cache-memory allocation balances reuse, live-generation headroom, and macOS memory pressure?
- Is INT8 or INT4 KV required to make long-context caching practical?
- Does a recurrent hybrid model need different cache controls from a local/global-attention Transformer?
- When does caching improve TTFT without damaging TG through pressure, compression, or swap?
- Which runtime configuration should be recommended for a specific machine, model, KV dtype, and context target?

This phase should establish a trustworthy cache baseline before speculative decoding is added. It does not authorize an automatic cache recommendation, cache UI, or estimator formula until the measured gates below pass.

---

# 2. Scope and Phases

## Phase A — Cache correctness and capacity

Run with speculative decoding disabled. Validate:

- exact prefix-cache hits;
- growing-prefix reuse;
- message-boundary reuse;
- cache memory accounting;
- LRU eviction;
- hybrid/non-trimmable entry behavior;
- cold versus cached TTFT;
- peak and steady-state RAM;
- TG stability under retained-cache pressure.

## Phase B — Real OpenCode request behavior

Replay or generate OpenAI-compatible multi-turn coding-agent traffic with stable system/tool prefixes, assistant messages, tool calls, and tool results.

## Phase C — Speculative decoding

Only after Phase A and B pass, add Rapid-MLX speculative modes and compare against the cache-only baseline.

The llama.cpp baseline often combines:

- a model-trained MTP draft/head; and
- `ngram-mod` speculative decoding.

Rapid-MLX has model/family-dependent MTP and suffix/tree-style mechanisms. Do not assume they are behaviorally identical to llama.cpp. They need their own matrix after caching is validated.

---

# 3. Existing Benchmark Assets and What They Gain Us

The current suite already records, for each model × context × KV dtype:

- prompt-processing throughput (`PP`, tokens/s);
- token-generation throughput (`TG`, tokens/s);
- time to first token (`TTFT`);
- total request latency;
- RAM or unified-memory measurements;
- context levels near 32K, 65K, 131K, 160K, and 200K;
- BF16, INT8, and INT4 KV modes.

This is the **cold-path reference dataset**. Cache testing adds a warm path and compares it to the cold path.

For a prompt containing `P` cached-prefix tokens with an appended suffix of `S` tokens:

```text
cold_prefill_seconds ≈ (P + S) / measured_cold_PP

ideal_cached_prefill_seconds ≈ S / measured_cold_PP
```

Actual cached TTFT also includes:

- HTTP/request parsing;
- tokenization;
- radix lookup;
- cache extraction/restoration/deep-copy work;
- MLX synchronization;
- scheduler insertion;
- first-token setup.

Therefore, cache restoration does not have to be constant-time to be successful. It only has to remain dramatically smaller than a full prompt scan.

Use three independent checks:

1. Rapid-MLX telemetry reports a hit and tokens saved.
2. TTFT resembles suffix-only processing rather than full-context processing.
3. RAM behavior is consistent with a retained checkpoint plus a live request.

Do not trust only one signal.

---

# 4. Rapid-MLX Cache Architecture Relevant to the Benchmark

This section describes source-level hypotheses that must be verified on the exact running server. Source inspection can guide a test, but only emitted runtime data is a benchmark contract.

- Repository: <https://github.com/raullenchai/Rapid-MLX>
- Source snapshot surfaced by GitHub search: `5fc6556c9b9fbf63c56f69a71a0fd6482ece26e4`
- Record the actual installed package version and commit because the project is moving quickly.

## 4.1 Continuous batching

Rapid-MLX uses `mlx-lm`'s `BatchGenerator` and a continuous-batching scheduler.

Relevant controls:

```text
--max-num-seqs
--max-concurrent-requests
```

For one OpenCode client and a llama.cpp-like single-slot baseline:

```text
--max-num-seqs 1
--max-concurrent-requests 1
```

These are distinct:

- `max-num-seqs=1` allows only one active generation sequence.
- `max-concurrent-requests=1` prevents another request from being admitted or queued concurrently.

The suite should record whether a particular release exposes a continuous-batching disable flag, but the primary baseline should constrain sequence and admission counts rather than depend on disabling the scheduler.

## 4.2 Prefix cache

Relevant controls include:

```text
--enable-prefix-cache
--prefix-cache-index radix|hash
--cache-memory-mb N
```

The source supports:

- exact token-sequence matches;
- a cached shorter prefix followed by suffix processing;
- reuse of a longer cached entry after trimming, when all cache layers are trimmable;
- memory-aware/LRU eviction under a byte budget;
- statistics including hits, misses, tokens saved, and evictions;
- prompt and message-boundary snapshots intended for growing multi-turn conversations.

Cache keys are token sequences. Semantically equivalent but token-different prompts stop matching at the first divergent token.

## 4.3 Radix versus hash

Use this as the primary mode:

```text
--prefix-cache-index radix
```

Rapid-MLX describes radix as the default, with `O(prefix_len)` token-trie lookup aimed at large shared-prefix workloads, including Cursor/Claude-Code-style traffic.

Use `hash` only for:

- an A/B regression test;
- suspected radix correctness problems;
- profiling index overhead;
- exact-match-only or mostly unrelated traffic.

KV/recurrent tensors, not radix metadata, should dominate memory.

## 4.4 Cache-memory budget

`--cache-memory-mb` controls the **retained reusable prefix-cache budget**, not total runtime memory.

Memory outside it can include:

- quantized model weights;
- active/live request KV or recurrent state;
- activations and temporary buffers;
- MLX/Metal allocator reserve;
- restored/copied cache state during a hit;
- tokenizer/Python memory;
- macOS and other applications.

Rapid-MLX can derive a default from a percentage of currently available physical RAM. This is unsuitable for controlled benchmarking because it varies by startup state. Always set an explicit value during tests.

Do **not** assume 8 GB is sufficient. The existing measured data may show that one retained 160K–200K state needs much more.

## 4.5 `--hybrid-cache-entries`

```text
--hybrid-cache-entries N
```

This is **not** a general equivalent of llama.cpp's checkpoint-count setting.

It governs a bounded pool used for cache states that cannot safely be trimmed backward, such as recurrent or linear-attention state.

Values:

- `0`: special trim-free hybrid reuse disabled;
- `1`: retain at most one useful non-trimmable entry;
- `2`: allow the newest state plus another useful boundary/state;
- `4`: allow additional branches/boundaries/sessions, with more memory cost.

For the representative multi-turn/fork workload, use `16` during
byte-capacity comparisons so this independent count limit cannot conceal the
effect of `--cache-memory-mb`. A smaller, workload-derived entry limit can be
recommended only after the cache working set is measured. A row that reaches
the configured entry limit is not valid evidence about a larger byte cap.

## 4.6 Message-boundary snapshots

Rapid-MLX contains scheduler logic to snapshot cache state at stable multi-turn boundaries.

This matters because one request often ends with an assistant-generation sentinel, while the next request replaces that tail with the actual assistant response, a tool result, a new user message, and another generation sentinel.

A whole-prompt cache entry may not be a strict prefix of the next rendered prompt. Boundary snapshots are intended to preserve reuse across this pattern.

Therefore, exact replay is not enough. The suite must include a real OpenAI `messages` / chat-template track.

## 4.7 PFlash

Disable PFlash during cache validation:

```text
--pflash off
```

PFlash may select/compress long-prompt blocks rather than process the entire original token sequence. Rapid-MLX deliberately excludes PFlash-compressed sequences from normal shared prefix-cache fetch/store because they are not positionally faithful.

Leaving PFlash on could make low TTFT look like a cache hit when it is prompt reduction.

## 4.8 Disk checkpointing and persistence

Rapid-MLX includes disk-backed KV checkpoint/persistence mechanisms. Do not mix them into the first in-memory cache benchmark.

Initial policy:

- set `--kv-disk-checkpoint-interval 0` for every in-memory performance row;
- restart the server between cold-series groups;
- make in-memory cache state explicit;
- test persistence separately later.

The current source build's automatic interval (default `256`) writes active-KV
snapshots but does not automatically reload evicted entries or hydrate a new
server. It can also contaminate TTFT because its write hook may run before the
first-token timestamp. Treat it as a write/persistence mechanism, not a
transparent disk cache. A later isolated lane compares `0` with `8192`, then
qualifies manual prefix-cache export → restart → import → follow-up.

---

# 5. Candidate Model Classification

The four models need separate cache tracks.

## 5.1 Qwen3.6-27B

Official architecture summary:

- 27B dense model;
- 64 layers;
- 16 repeated blocks of 3 Gated DeltaNet layers plus 1 gated attention layer;
- Gated DeltaNet: 48 V heads, 16 QK heads, head dimension 128;
- attention: 24 Q heads, 4 KV heads, head dimension 256;
- native context: 262,144 tokens;
- trained with multi-step prediction (MTP).

Source: <https://huggingface.co/Qwen/Qwen3.6-27B>

### Cache implication

This is a **recurrent hybrid model**, not a conventional all-attention Transformer. Its Gated DeltaNet state cannot necessarily be trimmed like ordinary KV cache.

Use the hybrid-entry track:

```text
--hybrid-cache-entries 1
--hybrid-cache-entries 2
--hybrid-cache-entries 4
```

Verify the installed MLX implementation's actual cache class names and trimmability.

### Memory implication

Weight memory is for a dense 27B model, but only a subset of layers have full-attention KV that grows with context. Existing measured RAM is more authoritative than an all-layer KV formula.

## 5.2 Qwen3.6-35B-A3B

Official architecture summary:

- 35B total parameters, about 3B activated;
- 40 layers;
- 10 repeated blocks of 3 Gated DeltaNet layers plus 1 gated attention layer;
- Gated DeltaNet: 32 V heads, 16 QK heads, head dimension 128;
- attention: 16 Q heads, 2 KV heads, head dimension 256;
- 256 experts, 8 routed plus 1 shared active;
- native context: 262,144 tokens;
- trained with multi-step prediction.

Source: <https://huggingface.co/Qwen/Qwen3.6-35B-A3B>

### Cache implication

This is also recurrent hybrid. Use a non-binding `--hybrid-cache-entries 16`
for byte-capacity calibration, then derive a smaller operational default from
the observed number of useful boundaries.

Because only 10 of 40 layers are full attention and those layers have only 2 KV heads, context-growth memory may be much lower than the 35B label suggests. Recurrent snapshots and runtime representation still matter.

### Memory/performance implication

Do not infer weight residency from active parameter count. Total stored parameters drive weight memory; active parameters mainly influence per-token compute.

## 5.3 Gemma 4 31B

Official architecture summary:

- dense model, approximately 30.7B core parameters;
- 60 decoder layers;
- interleaved local sliding-window and global attention;
- 1,024-token sliding window;
- 256K context;
- QAT variants and matching draft/assistant assets are officially available for the Gemma 4 family.

Sources:

- <https://huggingface.co/google/gemma-4-31B>
- <https://ai.google.dev/gemma/docs/core>

### Cache implication

Gemma 4 is hybrid in the generic local/global-attention sense, but it is not recurrent like Qwen's Gated DeltaNet.

Treat it as an **attention-cache track first**:

- radix prefix cache;
- memory-aware cache;
- no `--hybrid-cache-entries` in the initial baseline.

Then inspect runtime cache objects:

- cache class names;
- `is_trimmable()` results;
- local/sliding-window cache behavior;
- whether Rapid-MLX routes any Gemma layers into its bounded hybrid path.

If runtime evidence shows non-trimmable layers, add the hybrid-entry sweep as an observed exception.

## 5.4 Gemma 4 26B-A4B

Official architecture summary:

- approximately 25.2B total parameters;
- approximately 3.8B active;
- 30 layers;
- local/global attention interleaving;
- 1,024-token sliding window;
- 256K context;
- 128 routed experts, 8 active, plus one shared expert;
- official QAT-family assets exist.

Sources:

- <https://huggingface.co/google/gemma-4-26B-A4B-it>
- <https://ai.google.dev/gemma/docs/core>

### Cache implication

Like Gemma 4 31B, start in the attention-cache track and inspect actual cache classes/trimmability before adding `--hybrid-cache-entries`.

Its lower layer count and sparse activation may make it a strong 64 GB candidate with a QAT-derived 4-bit MLX conversion.

---

# 6. Weight Quantization Versus KV Quantization

The model's 4-bit/MXFP4 weights and runtime KV dtype are separate variables.

Example:

```text
weights = 4-bit or MXFP4
KV/recurrent cache = BF16, INT8, or INT4
```

Never label a run only as “4-bit.” Store both independently.

Required metadata:

```text
weight_quant_family
weight_bits_effective
weight_format
weight_model_id
kv_dtype
kv_quantization_mode
kv_quantization_bits
kv_group_size
turboquant_mode
```

For QAT-derived weights also store:

```text
is_qat_source
qat_source_model_id
conversion_tool
conversion_commit
```

QAT weight quality does not automatically imply anything about KV quantization quality.

---

# 7. Baseline Rapid-MLX Commands

Read the CLI from the installed v0.11.0 build first. Archive:

```bash
rapid-mlx --version
rapid-mlx serve --help
```

These templates express intent; map them to the exact local flag spelling.

## 7.1 Attention-cache baseline (Gemma first)

```bash
rapid-mlx serve MODEL_ID \
  --host 127.0.0.1 \
  --port 8000 \
  --max-num-seqs 1 \
  --max-concurrent-requests 1 \
  --enable-prefix-cache \
  --prefix-cache-index radix \
  --cache-memory-mb CACHE_MB \
  --pflash off
```

Add the selected runtime KV dtype flags.

## 7.2 Recurrent-hybrid baseline (Qwen3.6)

```bash
rapid-mlx serve MODEL_ID \
  --host 127.0.0.1 \
  --port 8000 \
  --max-num-seqs 1 \
  --max-concurrent-requests 1 \
  --enable-prefix-cache \
  --prefix-cache-index radix \
  --hybrid-cache-entries HYBRID_ENTRIES \
  --cache-memory-mb CACHE_MB \
  --pflash off
```

For the capacity benchmark, set `HYBRID_ENTRIES=16`; lower values are a
separate branch-retention policy experiment, not a cache-RAM comparison.

## 7.3 Feature exclusions for cache isolation

```text
speculative decoding = off
PFlash = off
disk cache restore = off or isolated
response cache = off
multiple concurrent requests = off
background load = off
```

Use deterministic sampling where practical. Cache correctness should not depend on sampling, but deterministic output simplifies replay.

---

# 8. Cache-Memory Budget Strategy

A universal 8 GB budget is inappropriate. Generate budgets from the existing measured model/context/KV dataset.

## 8.1 Estimate one retained state

For each cold benchmark row:

```text
model_loaded_ram = steady RAM after model load
active_context_ram = steady/peak RAM during context run
estimated_context_state_delta = active_context_ram - model_loaded_ram - known_non_cache_overhead
```

If non-cache overhead is uncertain, preserve both:

- raw active-context delta;
- adjusted estimate with a documented method.

A retained entry may differ from active-context delta because:

- active buffers include temporary allocations;
- cache arrays may have capacity padding;
- stored cache may be compressed differently;
- recurrent and KV layers have different shapes;
- restoring a hit may duplicate state transiently.

Use estimates to choose the sweep, then measure actual retained-cache bytes.

## 8.2 Candidate budget generator

For each model × context × KV dtype:

```text
0.75 × estimated_entry_mb
1.00 × estimated_entry_mb
1.25 × estimated_entry_mb
1.75 × estimated_entry_mb
2.25 × estimated_entry_mb
```

Round to 512 MB or 1,024 MB increments.

Also include practical candidates where machine-safe:

```text
4096
8192
12288
16384
20480
24576
```

Prune using model-loaded RAM and safe headroom.

## 8.3 Machine-headroom policy

For a 64 GB Mac, define a configurable reserve. Initial heuristic:

```text
system_reserve_mb = max(8192, 0.15 × physical_ram_mb)
```

Then:

```text
max_safe_cache_budget_mb =
    physical_ram_mb
    - model_loaded_steady_mb
    - expected_live_request_peak_delta_mb
    - system_reserve_mb
    - other_runtime_reserve_mb
```

This is a heuristic, not a guarantee. macOS compression, swap, and unified-memory behavior must be measured.

Mark a configuration unsafe if any occur:

- sustained yellow/red memory pressure;
- meaningful swap growth;
- TG degradation beyond threshold;
- allocation failure or process termination;
- poor system responsiveness;
- restore/copy transient peak beyond safe headroom.

## 8.4 Capacity goals

Test independently:

1. **One-entry viability:** latest long prefix remains cached.
2. **Two-boundary viability:** stable boundary plus latest state coexist.
3. **Multi-turn viability:** at least three sequential turns hit without eviction rescans.
4. **Branch/retry viability:** an earlier checkpoint survives one divergent retry/tool path.

Do not claim “cache supported” because one exact replay hits once.

---

# 9. Required Test Matrix

Use a staged matrix rather than a full Cartesian product.

## 9.1 Stage 0 — Runtime and telemetry contract discovery

**Initial source-build control (2026-07-26):** Gemma 4 26B-A4B LM revision `c6e1fec9d99b6af346dcae14bc0ed29cf55cd7e2`, `rapid-mlx 0.11.0+git.5fc6556c`, one persistent PFlash-off/int8 server with prefix cache enabled (`cache-memory-mb=4096`, `hybrid-cache-entries=4`), has completed an 8K `cold → exact replay → extension` control. The request-level raw metrics are observable and valid: cold was 8,030 prompt tokens / 2,830.76 ms TTFT; exact replay was 8,030 tokens / 3,146.95 ms TTFT and emitted `rapid_mlx_prefix_cache_hits_total +1` plus `rapid_mlx_prefix_cache_tokens_saved_total +8030`; the existing word-count extension was 9,362 tokens (+1,332 measured tokens) / 3,810.35 ms TTFT and emitted a prefix-cache miss with no observed saved-token increment. Cache storage metrics also moved (`current_bytes` 599,777,280 after cold, +628,424,320 on extension; radix nodes/entries and checkpoint metrics emitted). Do **not** call replay a speedup: this one replay was 316.19 ms slower despite the hit. Do **not** call extension a partial-prefix failure: its suffix was word-counted, not the required exact +512-token control. Receipt: `tests/fixtures/calibration/rapid-mlx-receipts/ailexleon-gemma4-26b-lm-source-5fc6556c-cache-stage0/`.

**Qwen3.6-35B-A3B exact-token control (2026-07-26; partial-prefix portion superseded):** source-build revision `6700c3e5bdeb050a379c8d2a4133f43f3647f20f` completed a persistent PFlash-off/int8/cache-enabled single-message control. The exact replay is valid mechanical evidence: 7,700 prompt tokens, `rapid_mlx_prefix_cache_hits_total +1`, and `tokens_saved +7700`; TTFT was essentially unchanged (3,269.49 → 3,256.96 ms), so performance benefit remains unproven and cache restore/copy overhead is a live hypothesis. The appended +512 user-content suffix rendered at 8,213 tokens (+513) but is **not a valid partial-prefix test**: altering the sole user message moves chat-template boundary tokens, and Rapid's hybrid message-boundary path intentionally returns no reusable boundary when that user message is index 0. Its no-additional-savings result must not be interpreted as a partial-cache miss. Replace it with a real multi-turn control: base user request → captured assistant response → new user turn, followed by a fork/replay sharing the full prior-message prefix. Receipt retained as diagnostic evidence: `tests/fixtures/calibration/rapid-mlx-receipts/unsloth-qwen36-35b-source-5fc6556c-cache-stage0-exact512-v2/`.

**Qwen3.6-35B-A3B valid multi-turn anchors (2026-07-26):** corrected `cold → exact replay → first follow-up → fork` controls establish the relevant agent boundary. At 32K: cold 31,769 tokens/15.60 s TTFT; replay hit but 17.53 s; first follow-up miss at 18.66 s; fork hit saving 32,218 tokens at 785.52 ms. At 65K: cold 63,435/40.14 s; replay hit but 47.09 s; first follow-up miss at 64.17 s; fork hit saving 64,261 tokens at 1.425 s. Thus exact cache lookup alone has no demonstrated speed benefit, while a warmed multi-turn fork has large, repeated TTFT benefit. Receipts: `...cache-multiturn-32k/` and `...cache-multiturn-65k/`.

Record:

```text
timestamp
machine model
SoC
physical RAM
macOS version
Rapid-MLX version and commit
mlx version
mlx-lm version
Python version
model ID and revision
weight format and quantization
chat-template hash
tokenizer hash
server command
environment variables
```

Archive:

```text
rapid-mlx --version
rapid-mlx serve --help
package snapshot
model config.json
generation_config.json
tokenizer_config.json
chat template / processor config
```

Before defining normalized cache fields, run one small persistent-server control sequence with PFlash and speculative decoding off:

```text
cold → exact replay → +512 extension
```

For every request, retain raw `/metrics` snapshots before and after, the effective argv, server logs, OpenAI usage, PP/TG/TTFT, and memory observations. A metric may enter the normalized schema only when it is emitted by the exact runtime and changes consistently under this control. Do not infer a metric from a source symbol, a documentation claim, or a similarly named metric in another Rapid-MLX revision.

Classify every proposed cache field as one of:

```text
observed_runtime_metric
derived_measurement (formula and inputs recorded)
unavailable (null; no proxy field)
```

## 9.2 Stage 1 — Cold parity

For each candidate model/KV dtype already covered, rerun:

```text
32K
65K
131K
160K
200K
```

Compare against the existing dataset to detect runtime drift.

Initial investigate thresholds:

```text
PP deviation > 10%
TG deviation > 10%
TTFT deviation > 15%
RAM deviation > 10%
```

## 9.3 Stage 2 — Exact replay

Sequence:

1. Cold prompt `A`.
2. Send identical prompt `A` again.
3. Send it a third time.

Capture:

- first/second/third TTFT;
- hits/misses;
- tokens saved;
- cache bytes and entry count;
- evictions;
- RAM before/after;
- restore-overhead trend.

Purpose:

- prove exact-match caching;
- establish minimum restore overhead;
- detect first-hit versus repeated-hit differences.

## 9.4 Stage 3 — Prefix extension

Append exact token-count suffixes:

```text
+128
+512
+2,048
+8,192
```

Generate target counts with the model tokenizer, not approximate characters.

Expected:

```text
cached_TTFT ≈ fixed_restore_overhead + suffix_tokens / cold_PP
```

Derived fields:

```text
cold_predicted_prefill_seconds
suffix_predicted_prefill_seconds
observed_cached_ttft_seconds
ttft_speedup
estimated_fresh_prefill_tokens
estimated_reused_tokens
estimated_cache_hit_ratio
```

Treat token estimates as approximate because restore overhead is mixed in.

## 9.5 Stage 4 — Real multi-turn OpenAI messages

Construct a deterministic coding-agent transcript with:

- stable system message;
- stable tool schemas;
- user task;
- assistant content;
- tool call;
- tool result containing source code/command output;
- another assistant turn;
- at least five total turns.

Use the same endpoint and message serialization OpenCode uses.

Keep stable:

- tool ordering;
- JSON serialization;
- thinking mode;
- tool parser;
- chat-template settings.

Capture rendered token IDs or their hash for every turn.

Purpose:

- verify message-boundary snapshots;
- expose generation-sentinel divergence;
- detect tool-schema reorder instability;
- compare real OpenCode behavior with synthetic raw prompts.

## 9.6 Stage 5 — Controlled divergence

For a 200K prompt, alter content near token positions:

```text
1K
32K
65K
131K
160K
```

Use token-index-aware mutation.

Expected:

- reuse stops near the first divergence;
- uncached work approximates the divergent tail;
- radix finds the deepest valid prefix.

Use existing cold context anchors to sanity-check the remaining-tail cost.

## 9.7 Stage 6 — Compaction simulation

Construct:

```text
Before: system + tools + ~200K transcript
After:  system + tools + compacted summary + recent turns
```

Measure:

- reuse of stable system/tool prefix;
- fresh processing of the new summary;
- cache behavior on the next post-compaction turn.

Expected:

- one larger PP event at compaction;
- normal incremental reuse afterward.

## 9.8 Stage 7 — Eviction/capacity

Generate distinct prompts `A`, `B`, `C`, `D`.

For each budget:

1. Populate A.
2. Populate B.
3. Populate C.
4. Replay A.
5. Populate D.
6. Replay B and C.

Capture exact LRU behavior.

For Qwen, cross with:

```text
hybrid_cache_entries = 1, 2, 4
```

For Gemma, add this dimension only if cache inspection shows it is relevant.

## 9.9 Stage 8 — Long-lived soak

Run 20–50 deterministic incremental turns:

- begin near 131K;
- grow through 160K;
- approach 200K;
- compact;
- continue for another 10 turns.

Capture:

- hit continuity;
- gradual RAM growth;
- entry churn;
- TTFT outliers;
- TG drift;
- swap;
- cache corruption/recovery warnings;
- server stability.

---

# 10. Context and Model Prioritization

## Primary decision contexts

```text
131K
160K
200K
```

## Calibration contexts

```text
32K
65K
```

## Suggested initial model order

1. Qwen3.6-35B-A3B
2. Gemma 4 26B-A4B QAT-derived MLX
3. Qwen3.6-27B
4. Gemma 4 31B QAT-derived MLX

This starts with one recurrent MoE and one local/global-attention MoE, then tests the dense alternatives. Existing benchmark results should override this ordering if they point elsewhere.

---

# 11. Dataset Schema Extensions

Add a cache-focused result table rather than overloading one flat cold row.

## 11.1 Run identity

```text
run_id
suite_version
timestamp
git_commit
runtime_name
runtime_version
runtime_commit
model_id
model_revision
model_family
architecture_class
weight_quant_family
weight_bits_effective
is_qat_source
kv_dtype
context_target_tokens
actual_prompt_tokens
output_tokens_requested
```

## 11.2 Cache configuration

```text
prefix_cache_enabled
prefix_cache_index
cache_memory_limit_mb
hybrid_cache_entries
pflash_enabled
disk_checkpoint_enabled
spec_decode_mode
max_num_seqs
max_concurrent_requests
```

## 11.3 Request relationship

```text
scenario_type
sequence_id
turn_index
parent_run_id
base_prefix_tokens
new_suffix_tokens
divergence_token_index
prefix_boundary_tokens
compaction_event
prompt_token_hash
stable_prefix_hash
chat_template_hash
tool_schema_hash
```

Suggested scenario enum:

```text
cold
exact_replay
prefix_extension
multi_turn
controlled_divergence
compaction
eviction_fill
eviction_probe
soak
```

## 11.4 Performance

```text
pp_tokens_per_second
tg_tokens_per_second
ttft_ms
total_latency_ms
decode_latency_ms
non_decode_latency_ms
queue_latency_ms
tokenization_latency_ms
```

## 11.5 Cache telemetry normalization

```text
cache_hit
cache_match_type
cache_hits_total
cache_misses_total
cache_hit_rate_total
cache_tokens_saved
cache_entries_before
cache_entries_after
cache_bytes_before
cache_bytes_after
cache_evictions_delta
cached_prefix_tokens_reported
fresh_prefill_tokens_reported
cache_restore_ms
cache_lookup_ms
```

This list is a desired analytical vocabulary, not a claim that the current runtime exports every field. Keep unavailable fields nullable. Preserve the raw metric snapshots and map only empirically observed series into these normalized names. Derived values must record their formula and source inputs; per-request hit type, restored-token counts, and restore timing remain unavailable unless the exact runtime exposes or the benchmark explicitly instruments them.

## 11.6 Memory/system state

```text
process_rss_before_mb
process_rss_after_mb
process_peak_rss_mb
metal_active_before_mb
metal_active_peak_mb
metal_cache_mb
system_used_memory_mb
system_available_memory_mb
compressed_memory_mb
swap_used_before_mb
swap_used_after_mb
memory_pressure_state
thermal_state
power_mode
```

On macOS, RSS alone is insufficient. Prefer multiple sources:

- `psutil`;
- `vm_stat`;
- `memory_pressure`;
- `sysctl`;
- MLX memory APIs;
- process sampling;
- optional `powermetrics` where permissions allow.

## 11.7 Derived values

```text
cold_reference_run_id
cold_reference_ttft_ms
cold_reference_pp
ttft_speedup
ttft_reduction_percent
estimated_fresh_prefill_tokens
estimated_cached_tokens
estimated_cache_hit_ratio
cache_bytes_per_cached_token
incremental_ram_per_turn_mb
tg_change_percent_vs_cold
swap_growth_mb
recommendation_status
failure_reason
```

---

# 12. Acceptance Criteria

## Exact hit

Pass when:

- telemetry reports a hit;
- tokens saved are close to prompt length;
- TTFT is dramatically below cold TTFT;
- result is reproducible;
- no corruption/retry warning;
- TG remains within tolerance.

## Prefix extension

Pass when:

- cached prefix is reported or inferred correctly;
- TTFT scales mainly with suffix size;
- 128 → 512 → 2,048 suffixes produce a monotonic suffix-cost curve;
- total context length has far less effect than cold PP predicts.

## Multi-turn viability

Pass when:

- at least five realistic OpenAI-message turns reuse the stable prefix;
- no unexpected full PP scan occurs before compaction;
- tool call/result turns do not break reuse early due to serialization drift;
- cache remains resident under the selected budget.

## Memory safety

Pass when:

- no meaningful swap growth;
- no sustained critical memory pressure;
- no OOM/allocation failure;
- TG does not regress beyond threshold;
- system remains responsive;
- restore transient peaks stay within reserve.

## Recommendation labels

```text
recommended
recommended_with_int8_kv
recommended_with_int4_kv
cache_budget_constrained
single_checkpoint_only
multi_turn_cache_unreliable
unsafe_memory_pressure
unsupported_runtime_cache_type
insufficient_data
```

---

# 13. Decision Matrix Logic for the Application

Inputs:

```text
machine_ram_mb
model_id
model_weight_memory_mb
kv_dtype
context_tokens
desired_parallelism
desired_cached_sessions
workload_type
```

Current workload defaults:

```text
parallelism = 1
cached_sessions = 1
workload_type = long_running_coding_agent
```

## Feasibility stages

1. **Model residency:** model plus minimum system reserve must fit.
2. **Live context:** selected model/KV/context must run cold without unsafe pressure.
3. **One reusable checkpoint:** enough safe memory remains for one effective retained state.
4. **Multi-turn boundary reserve:** enough budget remains for the model's cache strategy.
5. **Performance:** compare cold/warm TTFT, speedup, TG, and total latency.
6. **Ranking:** combine reliability, TTFT, TG, safe headroom, model quality, eviction, and instability.

Example objective:

```text
score =
    cache_reliability_weight × cache_hit_continuity
  + ttft_weight × normalized_ttft_improvement
  + tg_weight × normalized_tg
  + memory_weight × safe_headroom
  + quality_weight × model_quality_score
  - swap_penalty
  - eviction_penalty
  - instability_penalty
```

Keep raw facts separate from policy weights so recommendations are explainable.

Example user-facing output:

```text
Qwen3.6-35B-A3B, 4-bit weights, INT8 KV, 160K context

Status: Recommended for one coding-agent session
Measured model + live context peak: X GB
Recommended prefix-cache budget: Y GB
Expected retained useful entries: 1–2
Median cold TTFT: A s
Median cached-turn TTFT: B s
Observed cache continuity: 20/20 turns
Swap growth: 0 MB
Notes: recurrent hybrid; radix; hybrid-cache-entries=2
```

Do not pretend entry count is exact when ordinary cache capacity is byte-budgeted and LRU-managed.

---

# 14. Instrumentation Work for Codex

Codex should inspect the current codebase and integrate rather than rewrite blindly.

1. Locate and extend the existing benchmark data model, serializers, server lifecycle, fresh-server receipts, and cold/repeat/extension runner; do not replace them.
2. Add a scenario/sequence abstraction so requests share one live server.
3. Record the server version/help and raw metric snapshots for the Stage 0 control before committing a telemetry mapping.
4. Normalize only observed telemetry. Use an explicitly documented derived value where possible; use log parsing only as a labeled fallback.
5. Add prompt token hashing for whole prompt, stable prefix, tool schema, and rendered template.
6. Add deterministic exact-token-count prompt generation.
7. Implement the sequence scenarios in Section 9.
8. Add macOS unified-memory collection.
9. Join warm results to exact cold-reference rows.
10. Add assertions and failure classifications.
11. Export JSON/CSV plus a human-readable report.
12. Add resume support because the matrix will be long.
13. Record restarts and cache resets explicitly.
14. Prevent accidental PFlash/speculative activation in Phase A.
15. Preserve raw logs and metrics snapshots.

---

# 15. Avoiding False Conclusions

## “TTFT was low, so cache worked”

Could instead be PFlash, response cache, truncation, wrong token count, or disk restore. Verify tokens, telemetry, and flags.

## “TTFT grew with context, so cache failed”

Restore/deep-copy cost may scale with state size while still avoiding full PP. Compare against cold TTFT.

## “INT4 weights mean INT4 KV”

Store weight and KV formats independently.

## “hybrid-cache-entries is checkpoint count”

It is a bounded policy for special non-trimmable entries, not general capacity.

## “MoE active parameters define memory”

Total quantized parameters drive weight residency; active parameters mainly affect compute.

## “Exact replay proves OpenCode caching”

Real requests mutate the prompt tail and message/tool structure. Multi-turn testing is mandatory.

## “Same text means same prefix”

Template, tokenizer, thinking mode, tool order, and JSON serialization can change token IDs early.

---

# 16. Speculative-Decoding Follow-Up

Do not combine this with the first cache implementation unless separation is trivial.

After cache validation, add modes one at a time:

```text
none
MTP only
suffix/ngram-like mode only
supported combined mode, if Rapid-MLX explicitly allows it
```

Rapid-MLX has a unified speculative configuration framework and family-specific MTP injection paths. Detect support from the installed build rather than assume it.

Candidate implications:

- Qwen3.6 advertises trained MTP, but Rapid-MLX support must be verified for the exact `model_type` and conversion.
- Gemma 4 has official matching assistant/drafter assets, including QAT-family assistants, but exact Rapid-MLX/MLX compatibility must be verified.
- MTP gains may depend on batch size, sampling, model family, and acceptance.
- Speculative decoding adds memory that may reduce prefix-cache capacity.

Combined memory must include:

- target model;
- draft/assistant model or MTP head;
- live context;
- retained prefix cache;
- speculative buffers;
- macOS reserve.

Required speculative metrics:

```text
draft_tokens_proposed
draft_tokens_accepted
acceptance_rate
accepted_tokens_per_step
target_forward_calls
draft_forward_calls
TG
TTFT
total_latency
extra_RAM
cache_hit_continuity
```

Rerun cache scenarios after enabling speculative decoding because its memory overhead may cause new evictions.

---

# 17. Implementation Milestones

## Milestone 1 — Harness plumbing

- server lifecycle;
- sequence scenarios;
- telemetry;
- macOS memory;
- cold/warm joins.

## Milestone 2 — Synthetic cache validation

- exact replay;
- suffix extension;
- divergence;
- eviction.

## Milestone 3 — Model-aware tracks

- Qwen recurrent hybrid entry sweep;
- Gemma cache-class/trimmability inspection;
- QAT/weight metadata.

## Milestone 4 — OpenCode transcript replay

- stable tools;
- tool calls/results;
- multi-turn boundary reuse;
- compaction.

## Milestone 5 — Decision engine

- safe budget prediction;
- empirical override;
- recommendation labels;
- explainable output.

## Milestone 6 — Speculative decoding

- MTP;
- suffix/tree/ngram-equivalent options;
- combined memory/performance matrix.

---

# 18. Minimum Useful First Run

## Models

```text
Qwen3.6-35B-A3B 4-bit/MXFP4
Gemma 4 26B-A4B QAT-derived 4-bit MLX
```

## KV

```text
INT8
INT4
```

Add BF16 only where existing data says 160K–200K remains safe.

## Contexts

```text
131K
160K
200K
```

## Scenarios

```text
cold
exact replay
+512 suffix
+2048 suffix
five-turn OpenAI messages
compaction
eviction probe
```

## Cache budgets

Generate around measured one-entry size:

```text
below predicted fit
near predicted fit
25% above predicted fit
two-entry target, when safe
```

## Qwen hybrid entries

```text
1
2
4
```

## Gemma hybrid entries

```text
not set initially
```

This should reveal enough to refine the full matrix.

---

# 19. Questions Codex Must Resolve Locally

1. What exact flags does installed Rapid-MLX v0.11.0 expose?
2. What is the CLI default KV dtype for each profile?
3. Does `--cache-memory-mb` apply only to retained entries, and what telemetry exposes actual bytes?
4. What metrics names are available at `/metrics`?
5. Is there a supported cache-clear API without restarting?
6. Are cache lookup/restore times exposed separately?
7. What cache classes does MLX instantiate for each model?
8. Which layers report `is_trimmable()`?
9. Does Gemma 4 use the special hybrid-entry path in practice?
10. Does Rapid-MLX auto-select a nonzero hybrid count for known models?
11. Is PFlash automatically enabled by any chosen alias?
12. What disk checkpoint defaults are active?
13. Which speculative methods are accepted for Qwen3.6 and Gemma 4?
14. Can MTP and suffix decoding be combined, or are they mutually exclusive?
15. Does the OpenAI response report cached-token usage?
16. Can the benchmark capture exact server-rendered token IDs?
17. Are multimodal processors loaded for text-only runs, and what is their memory cost?
18. Which MLX quant repositories/revisions will be canonical?

Unknowns should become metadata or test results, not hard-coded assumptions.

---

# 20. Source References

## Rapid-MLX

- Repository: <https://github.com/raullenchai/Rapid-MLX>
- CLI reference: <https://github.com/raullenchai/Rapid-MLX/blob/main/docs/reference/cli.md>
- Configuration reference: <https://github.com/raullenchai/Rapid-MLX/blob/main/docs/reference/configuration.md>
- Continuous batching: <https://github.com/raullenchai/Rapid-MLX/blob/main/docs/guides/continuous-batching.md>
- Scheduler: <https://github.com/raullenchai/Rapid-MLX/blob/main/vllm_mlx/scheduler.py>
- Prefix cache: <https://github.com/raullenchai/Rapid-MLX/blob/main/vllm_mlx/prefix_cache.py>
- Memory-aware cache: <https://github.com/raullenchai/Rapid-MLX/blob/main/vllm_mlx/memory_cache.py>
- Radix index: <https://github.com/raullenchai/Rapid-MLX/blob/main/vllm_mlx/runtime/radix_index.py>
- Metrics: <https://github.com/raullenchai/Rapid-MLX/blob/main/vllm_mlx/routes/metrics.py>
- Speculative config: <https://github.com/raullenchai/Rapid-MLX/blob/main/vllm_mlx/spec_decode/config.py>
- MTP detection: <https://github.com/raullenchai/Rapid-MLX/blob/main/vllm_mlx/spec_decode/mtp/detect.py>
- Suffix eligibility: <https://github.com/raullenchai/Rapid-MLX/blob/main/docs/suffix_decoding_eligibility.md>

## Models

- Qwen3.6-27B: <https://huggingface.co/Qwen/Qwen3.6-27B>
- Qwen3.6-35B-A3B: <https://huggingface.co/Qwen/Qwen3.6-35B-A3B>
- Gemma 4 31B: <https://huggingface.co/google/gemma-4-31B>
- Gemma 4 26B-A4B IT: <https://huggingface.co/google/gemma-4-26B-A4B-it>
- Gemma 4 overview/QAT/drafters: <https://ai.google.dev/gemma/docs/core>
- Hybrid prefix-caching background: <https://arxiv.org/abs/2411.19379>

---

# 21. Final Direction to Codex

Treat the existing cold benchmark dataset as the source of truth for model-, KV-, and context-specific PP/TG/RAM behavior.

Do not hard-code an 8 GB cache budget.

Build the extension so it:

1. derives test budgets from measured context-memory data;
2. independently validates telemetry, TTFT, and RAM;
3. separates recurrent Qwen behavior from Gemma local/global attention;
4. tests real OpenCode-style multi-turn messages;
5. measures compaction and eviction;
6. produces explainable machine/model/KV/context recommendations;
7. preserves a clean cache-only baseline before speculative decoding.

The central product question is:

> For a specific machine, model, weight quantization, KV dtype, context target, and workload, can Rapid-MLX retain enough reusable state to avoid repeated full prompt scans without causing memory pressure or reducing decode performance?

Every schema and decision rule should support answering that question with measured evidence.
