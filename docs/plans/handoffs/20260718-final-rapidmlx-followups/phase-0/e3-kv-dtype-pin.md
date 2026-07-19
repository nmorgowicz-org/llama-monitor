# [E3] KV-Dtype Fact-Pin

Tag: [local-verifiable]
Phase: 0 (Phase 5 prerequisite)

## Question

What are the factual KV-cache flags, choices, and defaults in Rapid-MLX (v0.10.10 installed, v0.10.12 audited)?

## Answer

Installed v0.10.10 evidence from vllm_mlx/cli.py:

### Primary Flags

1. `--kv-cache-dtype` (cli.py:6833)
   - Type: str
   - Choices: {bf16, int8, int4}
   - Default: int4
   - Help: "KV cache dtype (R15 #300, default: int4). Apple Silicon decode is memory-bandwidth-bound; int4 yields ~4× less bandwidth per decode step with 97-98% quality retention. Sliding-window (Gemma 3, GPT-OSS) and MLA (DeepSeek V3+, Kimi K2.5) models auto-downgrade to bf16."

2. `--reasoning` (cli.py:6846)
   - Type: action=store_true, default=False
   - Effect: Pins `--kv-cache-dtype` to int8 regardless of dtype flag
   - Help: "Reasoning profile: pins --kv-cache-dtype to int8 regardless of the dtype flag (sub-4-bit drops -20pt on AIME-class math for Qwen3 thinking variants)."

### Legacy Quantization Flags

3. `--kv-cache-quantization` (cli.py:6858)
   - Type: action=store_true
   - Status: [deprecated alias of --kv-cache-dtype int8]
   - Help: "Quantize stored KV caches to reduce memory (8-bit by default). When both flags are passed, this one wins for backwards compatibility."

4. `--kv-cache-quantization-bits` (cli.py:6865)
   - Type: int, default=8, choices={4, 8}
   - Bit width for KV cache quantization

### TurboQuant Flags

5. `--kv-cache-turboquant` (cli.py:6833+ in server.py:1920)
   - Choices: {v4, k8v4, none}
   - Default: None (alias-driven per model profile)
   - v4 = V-only 3-4 bit Lloyd-Max with K in FP16
   - k8v4 = K at 8-bit Walsh-Hadamard + V at 4-bit Lloyd-Max
   - none = explicit off-switch
   - Mutually exclusive with --kv-cache-quantization

6. `--kv-cache-turboquant-bits` (cli.py/server.py)
   - Type: int, choices={3, 4}
   - Auto-select by head_dim: 3-bit for head_dim>=96, 4-bit for head_dim=64

7. `--kv-cache-turboquant-group-size` (cli.py/server.py)
   - Type: int, default=32

## Evidence Files

- cli.py:6833-6870 (argument definitions)
- cli.py:2837-2890 (resolution logic: --kv-cache-dtype + --reasoning + safelist)
- server.py:1918-1931 (TurboQuant flag definitions)
- server.py:2274-2294 (TurboQuant resolution)

## Explicit Non-Recommendation

This fact-pin records ONLY what the parser contains. NO recommended tool-calling KV value is asserted for MLX. That value is unknown and belongs to §8.3 `[escalate→device]` measurement envelope.

Separate llama.cpp heuristic (for Phase 1 encoding): tool-enabled runtimes below q8_0 KV are unreliable (§3.6).

## CHECK

PASS iff: KV-cache flags recorded with file:line evidence; no recommended MLX KV value asserted.
Status: PASS
