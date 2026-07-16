# Third-party provenance

The R2 GGUF reader/dequantization seed was audited from:

- Project: `barrontang/gguf2mlx`
- Git commit: `6a0da6529f233df79362cbf62dd96221c895351f`
- Snapshot URL:
  `https://codeload.github.com/barrontang/gguf2mlx/tar.gz/6a0da6529f233df79362cbf62dd96221c895351f`
- Snapshot SHA-256:
  `a4d4bb8d9c673ebbec348cf32a40b95aa051ebccd5e761bbad107286d11006fc`
- Upstream `pyproject.toml` blob SHA-256:
  `71c3903cbf8040862a4d8299489ecec10ace73fc0293b6d884395b1c939a209c`
- Declared package version: `2.0.2` (not used as source identity)
- Upstream author: Barron Tang
- Upstream declared license: MIT

The audited Git tree does not contain a standalone `LICENSE` file. Its
`pyproject.toml` declares `license = {text = "MIT"}` and identifies Barron Tang as the
author. The declared MIT license text is reproduced here with attribution:

Copyright (c) Barron Tang

Permission is hereby granted, free of charge, to any person obtaining a copy of this
software and associated documentation files (the "Software"), to deal in the Software
without restriction, including without limitation the rights to use, copy, modify,
merge, publish, distribute, sublicense, and/or sell copies of the Software, and to
permit persons to whom the Software is furnished to do so, subject to the following
conditions:

The above copyright notice and this permission notice shall be included in all copies
or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR IMPLIED,
INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A
PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT
HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF
CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR
THE USE OR OTHER DEALINGS IN THE SOFTWARE.

This notice preserves the upstream attribution, declared license, and missing-license
provenance. The R2 fork remains experimental and must not be promoted as a production
dependency until independent review accepts this provenance.

The llama-monitor fork retains only the profile-scoped GGUF reader/dequantization idea
and Llama tensor-name mapping needed for the R2 corpus. It removes upstream architecture
guessing, config defaults, partial-success behavior, arbitrary output selection, free-form
CLI output, and network-facing features. Policy, staging, cancellation, closure checks,
and promotion remain llama-monitor-owned.
