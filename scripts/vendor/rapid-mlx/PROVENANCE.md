# Vendored: `extract_mtp_weights.py`

Upstream's MTP weight extractor, vendored **verbatim** and never edited. Tensor
extraction is upstream's job and its details change; `scripts/build-mtp-head.py`
wraps this file and owns only sidecar *placement*, never the math. A newer
upstream version is dropped in by replacing this one file.

## What is pinned

| | |
|---|---|
| File | `extract_mtp_weights.py` |
| sha256 | `0776ecb720de1b2c6228cd2d6f37abad26ce8c198cde07e1babc7db05616d5a8` |
| Size | 8988 bytes |
| Vendored | 2026-07-29 |
| Upstream revision | **Not recorded at vendor time.** See below. |

The exact upstream commit is **unknown** and is deliberately written down as
unknown rather than guessed. What *is* established about it:

- It **postdates** rapid-mlx `5fc6556` ("fix(kv-cache): degrade to bf16 when
  mlx-lm lacks BatchGenerator._make_new_cache", #1231). Proven, not assumed: the
  copy in a `5fc6556` checkout contains no `pre_fc_norm` handling at all, while
  this copy does.
- It contains upstream's RMSNorm-shift fix (matches any 1-D norm weight instead
  of a hardcoded suffix list, so `pre_fc_norm_embedding` / `pre_fc_norm_hidden`
  are shifted) and fused-MoE expert conversion.

Record the revision when this file is next re-pulled. Absence of the field
downgrades what can be claimed about the pin; it does not invalidate the pin.

## Why this file matters more than it looks

The **stale** version of this extractor shifted only a hardcoded list of norm
suffixes and missed the MTP-specific `pre_fc_norm_*`, leaving the draft head's
fc-input normalization inverted. Symptom: **~0% draft acceptance, no error,
backbone fine, tool calls fine.** That read as a capability limit rather than a
bug for weeks. Details in `docs/reference/rapid-mlx-mtp-evidence.md` §1.

⚠️ **Older checkouts of rapid-mlx on this machine still contain the defective
extractor** — e.g. `/private/tmp/rapid-mlx-build` at `5fc6556`. `build-mtp-head.py`
accepts `--extractor <path>`, so pointing it at one of those silently rebuilds a
dead head. `verify_extractor()` refuses any extractor that does not mention
`pre_fc_norm`, and reports (without refusing) a sha256 that differs from the
value above, since a newer upstream copy is a legitimate override.

## Verifying / updating

```sh
shasum -a 256 scripts/vendor/rapid-mlx/extract_mtp_weights.py
```

When re-pulling: replace the file, update the sha256 here **and**
`VENDORED_EXTRACTOR_SHA256` in `scripts/build-mtp-head.py`, record the upstream
revision this time, and confirm `pre_fc_norm` still appears. Do not edit the
vendored file to fix anything — fix it upstream, or fix it in the wrapper.
