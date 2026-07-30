# Apple-silicon MTP runtime comparison

This reference records the 2026-07-29 source audit of Rapid-MLX 0.11.1 and
MTPLX 2.3.0, with llama.cpp as the behavioral reference. It answers a narrow
product question: can each runtime provide speculative decoding for normal
OpenAI-compatible coding-agent traffic while retaining sampling, tools,
caching, and multimodal behavior?

The answer is not a single winner:

- **Rapid-MLX remains useful for its broader loader and cache integration, but
  its current MTP path cannot serve normal sampled/tool-constrained agent
  requests.**
- **MTPLX has a real sampled Qwen MTP implementation, Qwen vision, and a much
  richer cache, but is a distinct runtime with deliberate sampler-policy,
  concurrency, model-family, and artifact constraints.**
- **llama.cpp has the broadest sampler compatibility and is the operational
  reference, but its MTP implementation and MLX-native runtimes do not share
  artifact formats or identical vision behavior.**

## Audited snapshots and evidence boundary

| Runtime | Audited source | Evidence boundary |
|---|---|---|
| Rapid-MLX | 0.11.1, commit [`206ed0e`](https://github.com/raullenchai/Rapid-MLX/tree/206ed0e03b7b6fc7b3b2e6f68a7b60467f6e5abe) | Installed source plus local served receipts. See [Rapid-MLX MTP evidence](rapid-mlx-mtp-evidence.md). |
| MTPLX | 2.3.0, commit [`97fb388`](https://github.com/youssofal/MTPLX/tree/97fb388a88ff2ab4084712ab76a198a8c73f431e) | Source audit and checked-in evidence; not yet live-qualified on this Mac. |
| llama.cpp | commit [`afeebe1`](https://github.com/ggml-org/llama.cpp/tree/afeebe103bd99cda8f5dfaefcabadf890db7fda7) | Source comparison plus the user's separate RTX 5090 operational evidence. |

“Supported in source” below does not mean “qualified on this machine.” MTPLX
throughput, memory, cache restore, vision correctness, and long-context rows
still require local receipts before llama-monitor may make product claims.
Similarly, the user's 200k llama.cpp result is useful operational evidence but
is not a transferable proof that an MLX runtime supports the same workload.

## Practical capability matrix

| Capability | Rapid-MLX 0.11.1 | MTPLX 2.3.0 | llama.cpp reference |
|---|---|---|---|
| Sampled MTP | **No.** Scheduler permits MTP only at temperature 0. | **Yes.** Stochastic proposal, target verification, `min(1,p/q)` acceptance, and residual correction. | **Yes.** Target tokens pass through the normal sampler chain; draft matches are accepted. |
| Coding-agent tools | MTP is bypassed when a logits processor is installed; normal constrained tools install one. | Tool prompting and strict grammar remain available with MTP, subject to the sampling caveat below. | Normal grammar and sampler state participate in target sampling. |
| OpenCode sampler values | OpenCode values reach the OpenAI endpoint, then the MTP eligibility guards usually disable MTP. | Generated OpenCode configuration marks temperature unsupported and identifies the client; server policy intentionally owns sampling. | Accepted request controls apply through the normal server sampler surface. |
| Sampling controls with MTP | Greedy only. | Temperature, top-p, top-k, presence/frequency penalties, and seed. No broad llama.cpp sampler parity. | Broad llama.cpp sampler chain, including grammar and runtime sampler options. |
| Draft-depth policy | Requested K is clamped to **K=1** for Qwen3.5/3.6 hybrid recurrent caches. | Qwen depth is capped at **3**. No draft-confidence `p_min` control. | `--spec-draft-n-max`; `--spec-draft-p-min` stops low-confidence drafting. |
| Prefix/session cache | Strong Rapid retained-prefix cache; independent of MTP qualification. | RAM session bank, optional SSD tier, prefix/boundary restore, target logits/hidden state, and committed MTP history. | Slot/prompt caching and optional host cache, with different ownership and persistence contracts. |
| Qwen3.5/3.6 vision | Current llama-monitor qualification found the Rapid VLM path non-functional for the target Qwen models. | Integrated `image_url` path, Qwen3-VL tower, embedding splice, MTP history integration, and vision cache. | Target VLM support exists; MTP source still has an image-embedding limitation, so draft efficiency/state near images needs direct validation. |
| Gemma 4 vision | Not qualified. | **No Gemma vision tower.** | Treat as model/build-specific and qualify directly. |
| Gemma 4 text MTP | Not qualified. | Dense 31B target plus the official four-layer assistant only. | Embedded or external draft paths where the model/build supports them. |
| Gemma 4 MoE/QAT | Not qualified. | 26B-A4B and other MoE/A4B shapes are rejected; QAT offshoots are not established compatibility. | Artifact-specific; GGUF support does not transfer to MLX. |
| KV quantization | Rapid cache policy is separately useful; active-generation KV qualification is tracked elsewhere. | Qwen paged KV supports q8/q4; Gemma path does not. | Broad GGUF KV types, including the user's q8_0 configuration. |
| Context evidence | Higher-context MTP rows intentionally deferred. | Declares 262,144 for Qwen; checked-in hardware evidence reaches 128k, not 200k. | User has separately run Qwen3.6-27B at 200k on RTX 5090. |
| Concurrency | Rapid scheduler supports broader serving, but MTP eligibility is the blocker here. | MTP is principally serialized; optional concurrency falls back to batched autoregressive decoding. | Server/slot policy is configurable; speculative modes may impose their own constraints. |
| GGUF | No; not expected. | No. | Native artifact format. |

## Why the MTP algorithms behave differently

### Rapid-MLX

Rapid's lower-level generator contains the pieces needed for stochastic
speculative decoding, including proposal and target distributions, acceptance,
and residual correction. The scheduler does not expose that path for sampled
requests. It checks the request before entering MTP:

1. temperature must be exactly zero;
2. no logits processor may be installed; and
3. hybrid Qwen cache state forces effective K=1.

Normal Rapid tool requests install a stateful constrained-grammar processor.
Consequently, `temperature=0` plus
`RAPID_MLX_CONSTRAIN_TOOLS=0` is only a diagnostic bypass: it changes the
client's sampling semantics and removes structural tool-call enforcement.

The scheduler mapping and guard are visible in the pinned
[`scheduler.py`](https://github.com/raullenchai/Rapid-MLX/blob/206ed0e03b7b6fc7b3b2e6f68a7b60467f6e5abe/vllm_mlx/scheduler.py#L2626-L2637).
The local repeated greedy run proves that the head can accelerate that narrow
envelope; it does not qualify real agent traffic.

### MTPLX

MTPLX's Qwen path implements standard stochastic speculative decoding. It
samples a draft token from distribution `q`, evaluates it under target
distribution `p`, accepts with `min(1, p/q)`, and on rejection samples from
normalized `(p-q)+`. The implementation also trims/restores target and
MTP-history caches as the accepted prefix changes.

The core probability operations are in pinned
[`sampling.py`](https://github.com/youssofal/MTPLX/blob/97fb388a88ff2ab4084712ab76a198a8c73f431e/mtplx/sampling.py);
generation and cache commit/rollback are in
[`generation.py`](https://github.com/youssofal/MTPLX/blob/97fb388a88ff2ab4084712ab76a198a8c73f431e/mtplx/generation.py).

This is the important functional gap from Rapid: nonzero temperature does not
turn MTP off. It is still not sampler-equivalent to llama.cpp. The audited
configuration covers temperature, top-p, top-k, presence/frequency penalties,
and seeds, but not min-p, typical-p, mirostat, logit bias, or an arbitrary
processor chain. The OpenAI request model allows extra fields, so a client can
send an unsupported field without proving that it affected generation.

MTPLX keeps strict grammar enabled during MTP, but its source notes that
top-k/top-p truncation can make tail correction inexact. This is a qualification
item, not a reason to equate its current implementation with llama.cpp's full
sampler/grammar semantics.

### llama.cpp

The audited llama.cpp Qwen path drafts greedily, target-samples each verification
row through the normal sampler chain, and accepts draft tokens only while they
exactly match the target-sampled tokens. That design naturally inherits the
target sampler's grammar and stateful processor behavior. See pinned
[`common/speculative.cpp`](https://github.com/ggml-org/llama.cpp/blob/afeebe103bd99cda8f5dfaefcabadf890db7fda7/common/speculative.cpp).

`--spec-draft-p-min 0.60` is a **draft confidence cutoff**, not a required
acceptance ratio. With `p_min=0`, drafting continues until another stop
condition such as `n_max`. MTPLX has no equivalent confidence knob in the
audited snapshot.

## OpenCode: the practical MTPLX caveat

MTPLX can keep sampled MTP active for OpenCode, but it does so by owning the
sampler policy rather than honoring ordinary per-request sampler changes.

The generated OpenCode configuration:

- sends `x-mtplx-client: opencode`;
- marks `temperature` as unavailable to the client; and
- ignores the builder's temperature/top-p/top-k arguments.

See pinned
[`opencode.py`](https://github.com/youssofal/MTPLX/blob/97fb388a88ff2ab4084712ab76a198a8c73f431e/mtplx/opencode.py).
Generic clients can opt into client controls with
`x-mtplx-allow-client-controls: 1`, but recognized OpenCode traffic is denied
those controls before the generic opt-in is considered.

This is usable if the operator accepts MTPLX's server-owned policy. It is not
drop-in semantic parity with an OpenAI endpoint that applies every OpenCode
request override. A local qualification must verify both output behavior and
the effective sampler values reported by the runtime.

## Cache and multimodal detail

MTPLX's cache is substantive, not a prompt-string memo. The
[`session bank`](https://github.com/youssofal/MTPLX/blob/97fb388a88ff2ab4084712ab76a198a8c73f431e/mtplx/session_bank.py)
stores exact/near prefix state, target logits and hidden state, cache snapshots,
and committed MTP history. The
[`cold tier`](https://github.com/youssofal/MTPLX/blob/97fb388a88ff2ab4084712ab76a198a8c73f431e/mtplx/cache_bank/cold_tier.py)
adds optional SSD restore. These are attractive source-level capabilities, but
must be measured for real multi-turn OpenCode histories, eviction, restart, and
memory pressure on the target Mac.

For Qwen vision, MTPLX accepts OpenAI `image_url` content, runs its
[`Qwen3-VL tower`](https://github.com/youssofal/MTPLX/blob/97fb388a88ff2ab4084712ab76a198a8c73f431e/mtplx/vision/qwen3_vl_tower.py),
and splices embeddings into both target and MTP history. The audited official
Qwen3.6-27B and 35B-A3B Speed artifacts include the expected vision-tower
tensors. Content-keyed session/vision caching avoids treating equal text
placeholders with different images as the same prompt.

That finding is specific to those artifacts. Custom QAT, AWQ, NVFP4, or
converted checkpoints must preserve both MTP and vision tensors. Name/shape
similarity is not enough.

MTPLX's Gemma support is much narrower: dense Gemma 4 31B text generation with
the official four-layer assistant, no Gemma vision tower, no Gemma KV
quantization, and no accepted MoE/A4B shapes. It does not currently satisfy the
requested Gemma4 26B-A4B multimodal use case.

## Product decision for llama-monitor

Rapid-MLX MTP qualification is **suspended, not abandoned**:

- retain the committed benchmark harness and existing receipts;
- do not spend more time on qx64/MXFP8 repeats, natural-lane, 65k/131k,
  expanded completion, or tool-call MTP matrices now;
- continue Rapid loader, cache, runtime, and unrelated integration work;
- do not advertise, auto-enable, or treat Rapid MTP as qualified;
- do not begin MTP-specific managed artifact/UI work that consumes a
  qualification result.

Reopen the Rapid matrix only when a pinned upstream build passes all of these
cheap gates:

1. a nonzero-temperature request records nonzero speculative attempts;
2. a normal constrained-tool request records nonzero speculative attempts;
3. sampled and tool-constrained outputs pass parity/fidelity checks;
4. requested/effective draft depth and fallback reasons remain observable.

Only then run the saved repeated matrix, including the deferred higher-context
rows. A future K>1 hybrid-cache implementation is desirable, but sampled and
tool-constrained correctness is the minimum reopening condition.

MTPLX should be evaluated as a separate candidate runtime, not as a sidecar fix
for Rapid. A first qualification should cover Qwen3.6-27B and 35B-A3B only:
OpenCode effective sampling, sampled MTP correctness/acceptance, tool grammar,
warm/follow-up/fork cache behavior, image correctness with cache reuse, q8/q4
KV, serialized request behavior, 32k/65k/128k, and only then a 200k stress gate.
Gemma and custom QAT artifacts remain out of scope until their source-level
model/vision gaps change.
