# Windows native calibration evidence

Validated 2026-08-17 on native Windows x86_64 with an NVIDIA GeForce RTX 5090 and the managed CUDA 13.3 llama.cpp runtime (`b10470`). The disposable profile contained the Qwen3.5 GGUF and Froggeric v22.1 template; the user’s port 8001 and active profile were not used.

## Gates

- Managed CUDA 13.3 server, llama-bench, and fit-params binaries installed and fingerprinted.
- Quick calibration: complete; baseline and verification trials passed.
- Balanced calibration: complete; selected `balanced-l9-r04` (batch 1024, ubatch 512, context 57344).
- Apply/rollback: passed with exact preset fingerprint restoration.
- Cancellation cleanup: passed; cancelled jobs are intentionally not resumable.
- Native real-server qualification: passed after removing the invalid abstract `gpu` draft-device emission and adding one bounded MTP loopback retry.
- Qualification tracks: `latency_memory`, `mtp`, and `ngram` completed; no diagnostics.
- Release-built preset UI capture group: community sources, evidence drawer, and preset editor completed; Rapid-MLX/discussions scenarios correctly skipped on Windows by platform guard. Calibration capture remains opt-in because it must not run a benchmark implicitly.

## Final qualification

- Job: `ccd322b59f7aac51cff72094`
- Fixture: `windows-qwen35-phase7-mtp`
- Model: Qwen3.5 9B GGUF, 7,087,550,080 bytes
- Runtime server SHA-256: `aa6b7907d3901f2e24892838e6f15243a47b22ad792eaccbdc0e2a4bccfd5283`
- llama-bench SHA-256: `23947ddff87fe418e2db0e49d6fb1b79f2f66c142cf7d5c614d0f4c870e05c4b`
- Baseline qualification: `latency_memory` completed
- Candidate qualification: `latency_memory`, `mtp`, `ngram` completed

The full disposable receipts remain outside the repository profile; this file records the reproducible gate results without tokens or machine-local paths.
