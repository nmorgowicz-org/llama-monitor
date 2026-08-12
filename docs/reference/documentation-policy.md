# Documentation policy for the 2.0 rebrand

Current operator and developer documentation uses Local LLM Foundry, the
canonical executable `local-llm-foundry`, and the canonical application roots.
Compatibility identifiers are shown in a clearly labeled 2.x note when a user
needs them (`llama-monitor`, the legacy roots, stable API fields, or the
`llama_monitor` Rust library namespace).

Historical material is intentionally immutable. This includes changelogs,
release notes, benchmark receipts, calibration fixtures, dated implementation
plans, host-gate reports, and third-party product names. A historical filename
or path is not evidence that a current surface is still branded incorrectly.

Before publishing documentation changes:

1. Check links from the changed file and ensure commands match current
   `local-llm-foundry --help` output.
2. Preserve technical names (`llama.cpp`, `llama-server`, GGUF, MLX,
   Rapid-MLX, LHM) and internal compatibility identifiers.
3. Run `bash scripts/check-unused-screenshots.sh`; promote only fresh,
   release-built screenshots that are referenced by current docs.
4. Review current-name scan results against this policy instead of applying a
   bulk rewrite to historical evidence.
