# Calibration upstream contract ledger

Date: 2026-08-13  
Purpose: immutable source and capability evidence for the bounded llama.cpp
Calibration v1 release gate. This receipt records the upstream source used as
an oracle; it does not authorize importing the Python implementation or its
console output as a product contract.

## Pinned sources

| Repository | Commit | License | License SHA-256 |
|---|---|---|---|
| `bigattichouse/llama-optimize` | `1d9e7d7fc2c94675362673983ea4fa1e756e0a0a` | MIT | `6b9bedaa81ffa7a661d080ddd40c15b2994bee4659c1a3b72d036c30ae090e86` |
| `bigattichouse/robust` (git submodule `robust`) | `a457b7f7f4a7a06b183fd55be4b8aced5d7f2541` | CC0-1.0 | `a5649436406916897fb924b3e3bd7422d6df82fdfa904e153aeb426a9b7a99e8` |

The parent repository records the exact `robust` submodule object:

```text
160000 commit a457b7f7f4a7a06b183fd55be4b8aced5d7f2541 robust
```

## Reviewed contract files

The following files were read from the pinned checkout and are the only
upstream references currently authorized for contract extraction:

```text
README.md
ROADMAP.md
docs/CONDITIONAL-FACTORS.md
docs/DESIGN.md
docs/measurement-validity.md
docs/multi-gpu-design.md
llama-optimize.py
robust/README.md
robust/DESIGN.md
robust/LICENSE
```

SHA-256 manifest for the reviewed files:

```text
64b174222e723a7a7b83009da713e35220af06be3bd1a6239aad44b7cbcbecba  README.md
ed73cbd78749e18c77d2ed5ff8e97622504c8da61218df95c5e815f601b083ed  ROADMAP.md
e022efc6e8b9af2a19b9b8b5403d77d05519e2cd24aeba4aaea79c0e29b56a1a  docs/CONDITIONAL-FACTORS.md
af0bbed2bc0025ebacdd623590a1e4e1914259f76c23d74a80356ce3cb6e854b  docs/DESIGN.md
703adafaa7bfa726ea843336e3af2289ef7a0829ec4c25f1e7f012be7dc15293  docs/measurement-validity.md
4dbc3ad9fae596e0a949e90c0add93ec5d13bf4f0be2b5b94432d1f843f675c8  docs/multi-gpu-design.md
3a96b72569a4ba4dc44f03acb785ea094e60b3ebb4cb0a6a614a71d4b29effd5  llama-optimize.py
4a5a4819fc86225ae44bc08765915a4f59c3c94bc4d9ffce306abab6196abdf6  robust/README.md
9d1ee922ded6f29806f18f0f0286dfcb9960253f70cbac2ea03a1c022400f67b  robust/DESIGN.md
```

## Offline capability checks

Commands run from the pinned `llama-optimize` checkout:

```text
python3 llama-optimize.py --selftest
  selftest: all checks passed

make -C robust all
  PASS (C99 build; Taguchi, Morris, Robust, Pareto, regression, UQ, OFAT,
  grid, report, RSM, and desirability tools built)
```

The complete upstream C test target was also attempted:

```text
make -C robust test
  FAIL: core security test `test_space_parse_file_paths`
  assertion: strstr(err, "limit") != NULL
```

This is an upstream portability-test defect on macOS: opening a directory and
calling `fseek` returns `cannot seek` before the parser reaches its documented
oversized-file guard, while the test assumes Linux directory `ftell` behavior.
No upstream source was modified. The failure is retained as a caveat and the
full upstream test gate remains open until the contract is either reproduced
on the reference platform or the source maintainer supplies a portable fix.

Windows may reject the directory at `fopen`, `fseek`, or `ftell` with a
different platform error. Therefore the Local LLM Foundry implementation must
not match these human strings. Its cross-platform invariant is:

1. inspect the path metadata before reading;
2. reject directories, symlinks, and other non-regular files;
3. return one stable typed error such as `not_regular_file`; and
4. test that invariant with macOS/Linux/Windows fixtures.

This keeps a platform-specific upstream parser behavior from becoming a
product compatibility contract.

## CLI contract snapshot

`python3 llama-optimize.py --help` was captured successfully. The integration
must not parse its human-readable output. The allowed product evidence is the
typed capability/argv contract described in the pinned source regions:

- `llama-optimize.py:47-171` — binary discovery and preflight;
- `llama-optimize.py:216-756` — hardware/GGUF introspection;
- `llama-optimize.py:757-1015` — configuration and factor selection;
- `llama-optimize.py:1075-1403` — factor registry and command construction;
- `llama-optimize.py:1404-1880` — validity checks and benchmark drivers;
- `llama-optimize.py:1881-2456` — Pareto/picks/reporting;
- `llama-optimize.py:2457-2604` — context probe and pick verification;
- `llama-optimize.py:4070-5035` — Morris, CLI, persistence, resume, and crash journal.

## Gate result

**Partial pass.** Commit/license pins, source manifest, offline self-test, and
native build evidence are recorded. The upstream full-test portability defect,
the cross-platform regular-file invariant, managed-binary capability receipts,
and the native llama.cpp contract matrix remain open. No source or binary from
either upstream repository is shipped by this receipt.
