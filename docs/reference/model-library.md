# Model library

Llama Monitor keeps every app-managed model under the configured `models_dir`. The
default is `~/.config/llama-monitor/models/`. The directory is backend-neutral:

```text
models/
├── gguf/                       # llama.cpp models and companions
├── mlx/
│   ├── native/                 # local native MLX models
│   └── converted/              # validated official mlx-lm conversions
├── transformers/              # complete safetensors conversion sources
├── rapid-mlx/
│   ├── imports/               # experimental recovered FP16 caches
│   └── requantized/           # experimental MLX quantization caches
├── cache/huggingface/
│   ├── hub/                    # app-scoped Hugging Face cache
│   └── xet/                    # app-scoped Xet cache
└── .staging/                   # incomplete downloads and conversions
```

Files directly under `models_dir` remain discoverable for compatibility. The library
migration moves complete GGUF files to `gguf/`, incomplete `.part` files to
`.staging/downloads/`, and explicitly selected app-owned Hugging Face repositories to
the app cache. It uses same-filesystem renames, records a restartable journal, refuses
collisions and symlink escapes, and rewrites preset, session, draft, mmproj, and
path-keyed tag references. Files with other extensions, including chat-template
`.jinja` files, are not moved.

Migration is never automatic. Preview the exact plan with an API token, then execute
that same `plan_id` with the database-admin token and the explicit confirmation value.

## Inventory metadata and backend awareness

The inventory is backend-neutral: it scans `models_dir` once and classifies each entry by
format (`gguf`, `mlx`, `transformers`, `unknown`) and by which backends it supports
(`llama_cpp`, `rapid_mlx`). A model can be launchable under llama.cpp, Rapid-MLX, or
neither. The inventory does not run either backend.

Every model card receives typed metadata from the backend rather than guessing solely
from its filename:

- `format`: `gguf`, `mlx`, `transformers`, or `unknown`
- `source`: `local`, `hugging_face`, `official_conversion`, `recovered_gguf`,
  `requantized_mlx`, `legacy`, or `unknown`
- `lifecycle`: `ready`, `incomplete`, `converting`, `invalid`, or `unknown`
- `compatibility`: `verified`, `experimental`, `provisional`, `unsupported`, or
  `unknown`
- `supported_backends`: `llama_cpp`, `rapid_mlx`, or an empty list
- `companion_kind`: `mmproj` or `draft` when applicable

Unknown and incomplete entries remain visible with explicit badges but cannot launch.
Ready Rapid-MLX entries create a typed Rapid-MLX preset; GGUF entries continue through
the llama.cpp wizard. GGUF is not presented as a native Rapid-MLX input.

Recovered and re-quantized caches appear as first-class MLX cards with their source,
recipe, and `Experimental` badges. They deliberately have no supported backend and no
launch action. Visibility is not a promotion: only a future architecture gate can make
one launchable.

## Experimental GGUF Import Lab

The compatibility inspector evaluates a GGUF for an experimental MLX recovery
without converting weights, launching a runtime, or using the network. It reads at most
64 MiB of GGUF metadata and tensor-directory data in a bounded blocking worker and
returns a versioned report containing:

- canonical source identity, file size, modification time, and a SHA-256 identity of
  the bounded GGUF header (not a full-file content hash);
- authoritative `general.architecture`, tensor count, and per-quant-type inventory;
- observed config, tokenizer, chat-template, MTP, and multimodal asset metadata;
- `verified`, `experimental`, or `unsupported` compatibility, exact missing profile
  fields/assets, resource tier, warnings, and remediation.

The inspector never infers architecture from the filename and never falls back to a
Llama profile. Initial Llama, Qwen 2, and Mistral text-only sources can be reported only
as experimental candidates; no R1 profile is verified for conversion. Qwen 3.5/3.6
hybrid/MoE, Gemma 4, MTP/NextN, multimodal projector inputs, unknown architectures,
unknown tensor formats, and Q3/Q2/IQ3-or-lower tiers fail closed until dedicated
profiles and runtime/parity evidence exist. Q4/Q5 inputs show a compounded-loss warning,
and IQ4 inputs require a separate importance-aware profile.

The Models manager's Import Lab tab combines that report with current disk/RAM
headroom, the llama.cpp fallback, and an exact-profile recovery action. Recovery is
available only for the pinned SmolLM2 135M R2 profile and creates a separate FP16 cache;
it never edits or replaces the GGUF. One bounded worker runs at a time, with at most 32
retained in-memory job records. Progress, cancellation, bounded diagnostics, terminal
cleanup, and failures are app-native. Browser dialogs are not used.

Resource estimates use a fail-fast pool of two blocking workers. Unsafe path syntax is
rejected before pool admission, saturation returns `429`, and unknown disk headroom is
shown as unknown rather than treated as available. Public job failures are stable,
path-free diagnostics; raw worker errors and local filesystem paths are not exposed.

Only library-relative paths resolving to regular `.gguf` files inside the configured
`models_dir` are accepted. Traversal, absolute or root-relative input, symlinked path
components, incomplete headers, and oversized metadata are rejected. Inspection never
modifies the original GGUF and does not create a cache or other large output.

Local Rapid-MLX execution is available only on Apple Silicon (`macOS` + `aarch64`).
Other platforms still inventory, identify, and allow copying or migration of MLX and
Transformers models, but show an explicit Apple Silicon requirement instead of a local
Rapid-MLX configure action. The Rust launch boundary enforces the same restriction
before runtime discovery, downloads, or conversion can begin. Remote attachment remains
separate from local execution. On unsupported platforms the report and model-management
surfaces remain available, while the local recovery action stays disabled.

## Rapid-MLX model sources

Rapid-MLX presets store a tagged `rapid_mlx.model_source`. Supported inputs are:

- A validated local MLX directory
- A revision-pinned Hugging Face repository
- A runtime alias (a known name mapped by the launcher to a specific repository)
- Complete authoritative safetensors

The VRAM estimator's Rapid-MLX path also accepts an HF-repo-style alias as `model_path`
(e.g. `"mlx-community/Qwen3-30B-A3B-4bit"`): it detects the `"org/repo"` shape and
treats it as an `hf_repo_id`, fetching the `config.json` directly from Hugging Face.

Legacy `rapid_mlx.model_path` values are migrated at resolution time.

Authoritative safetensors conversion uses the managed runtime's exact
`mlx-lm==0.31.3`, stages output below `.staging/conversions/`, performs a real MLX load
check, writes a source/tool/recipe/hash manifest, and atomically promotes the result to
`mlx/converted/`. A `.complete` marker alone is insufficient: every cached file is
verified against the manifest before reuse.

Recovered and re-quantized experimental caches receive first-class inventory badges
only after strict validation of their non-symlink cache root, zero-byte `.complete`
marker, bounded typed manifest, exact profile/worker/environment identities,
validation-report hash, and complete recursive published-file hash closure. Invalid or
incomplete caches are omitted. The inventory emits sanitized lineage fields and never
trusts arbitrary manifest provenance; every such model remains non-launchable pending
an explicit profile-promotion phase.

App-launched Hugging Face operations receive `HF_HUB_CACHE` and `HF_XET_CACHE` inside
the model library. `HF_TOKEN` is passed only through the child environment and is not
stored in source metadata, conversion manifests, command arguments, or diagnostics.

Platform information is fetched once through shared frontend state. This phase has no
runtime-install or platform-changing action, so there is no mutation that needs to
invalidate it yet; the explicit refresh mechanism is reserved for the installation
workflow.
