# Calibration managed-binary capability receipt

Date: 2026-08-13  
Host evidence class: macOS Apple Silicon; native Windows evidence remains a
Phase 12 return marker.  Paths are intentionally described as the configured
managed bundle, not as a product-root literal.

## Binary identities

| Sibling | SHA-256 | Version/help result |
|---|---|---|
| `llama-server` | `02723fc39fbeebd9849ce4c9ca3799649df3cf91f101c2cd56b8756e1db54d28` | `--version` passed: `10310 (cb26014d9)`, AppleClang 21, Darwin arm64 |
| `llama-bench` | `641a7ccc957ddb0fdcb304a02ec1809aa18ac88d7b498a43c7dedd5ba75290c3` | `--help` passed; `--version` is unsupported and returns its bounded usage error |
| `llama-fit-params` | `192e6d0ac3575ff8868a2658bd5d6043162e276ed4d6cf4b8cecb9bd89ad243a` | `--version` passed: `10310 (cb26014d9)`, AppleClang 21, Darwin arm64; `--help` passed |

## Required factor evidence

The managed help output contains the exact flags required by the bounded
llama.cpp candidate catalog:

```text
llama-bench: -p, -n, -d, -b, -ub, -ctk, -ctv, -t, -ngl, -ncmoe,
             -nkvo, -fa, -o/--output, -fitt/--fit-target, -fitc/--fit-ctx
llama-server: -c, -b, -ub, -ctk, -ctv, -t, -tb, -ngl, -ncmoe,
              -nkvo, -fa, --fit, -fitt, -fitc, --host, --port
llama-fit-params: --help, --version, -c, -b, -ub, -ctk, -ctv, -ngl,
                  -ncmoe, --fit, -fitt, -fitc
```

`llama-fit-params` is present and therefore predictive pruning may be enabled
only when its typed output is successfully parsed. Missing or malformed output
must degrade to measured execution. `llama-bench` has no standalone version
flag in this bundle; its binary hash and help contract are the evidence keys.

## Gate result

**Pass for the available macOS managed bundle.** The sibling set resolves from
the configured `llama-server` directory, the required Quick factor flags are
present, and the optional fit helper is available. This receipt does not close
the native Windows capability or real-GGUF Calibration gates.
