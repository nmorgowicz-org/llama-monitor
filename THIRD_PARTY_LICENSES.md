# Third-party notices

This file records third-party works used as sources for Local LLM Foundry.
Pinned commits and the exact reviewed files are recorded in
`docs/plans/evidence/20260811-local-llm-foundry/phase-11/calibration-upstream-contract-ledger.md`.

## `bigattichouse/llama-optimize`

- Repository: <https://github.com/bigattichouse/llama-optimize>
- Pinned commit: `1d9e7d7fc2c94675362673983ea4fa1e756e0a0a`
- Author: bigattichouse
- License: MIT
- Use: methodology and contract reference for the native Rust Calibration
  design. The Python application is not imported or executed by Foundry.

If source code from this repository is copied or substantially derived into a
Foundry module, preserve the following notice in that module and retain this
license text:

```text
MIT License

Copyright (c) 2026 bigattichouse

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
```

## `bigattichouse/robust`

- Repository: <https://github.com/bigattichouse/robust>
- Pinned commit: `a457b7f7f4a7a06b183fd55be4b8aced5d7f2541`
- Author: bigattichouse
- License: CC0 1.0 Universal
- Use: Taguchi/Morris design and analysis reference. The bounded prime-field
  orthogonal-array construction is implemented natively in
  `src/calibration/design.rs`; the native C library is not linked or shipped by
  Foundry. The Rust module identifies the source commit and its adaptation in
  its header.

CC0 does not require attribution, but Foundry credits the author and keeps the
license reference here as a courtesy. The full legal text is available at
<https://creativecommons.org/publicdomain/zero/1.0/> and was verified in the
pinned checkout. Any future copied source must record its exact path, commit,
modification, and fixture parity in the Calibration evidence ledger.

## Update procedure

Do not follow upstream `main` automatically. To update a source:

1. pin a new commit and re-check its license;
2. regenerate the source/hash and oracle-fixture manifest;
3. review contract and security changes line by line;
4. run native Rust parity tests on every supported platform; and
5. update this file and the Calibration plan in the same change.
