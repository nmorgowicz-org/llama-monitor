# Windows capability return marker

This marker is intentionally not a claim of native Windows execution. The
macOS checkout has completed the GNU cross-check; native managed-binary help
and one real Windows calibration receipt must be collected on the Windows host
before Phase 10 platform qualification is closed.

Run from the repository root on Windows after the managed llama.cpp bundle is
available:

```powershell
cargo check --target x86_64-pc-windows-gnu
cargo test calibration::
.\target\release\local-llm-foundry.exe --help
```

Record, without absolute user paths or secrets:

- `llama-server.exe`, `llama-bench.exe`, and optional
  `llama-fit-params.exe` SHA-256 hashes;
- bounded `--help`/`--version` stdout and stderr hashes;
- supported factor flags and any missing optional capability;
- one tiny local-GGUF Quick receipt, cancellation receipt, and restart/recovery
  receipt;
- child-process cleanup and receipt/application-home paths.

The native Windows receipt belongs in the Phase 10 evidence directory and
must not be backfilled from macOS or GNU cross-compilation.
