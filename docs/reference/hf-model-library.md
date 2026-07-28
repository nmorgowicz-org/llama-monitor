# Hugging Face Model Library

> **Status: Shipped.** Phase 8A-B complete. Full HF model discovery, MLX quant detection, CommunitySourceCatalog integration, and model library with lineage badges.

The Hugging Face model library provides a rich model discovery and selection interface integrated into the Spawn Wizard (Step 1) and the models inventory view. It replaces the simple repo ID input with full search, filtering, sorting, and model card rendering.

## Discovery scopes

Users can filter models by scope using additive toggle buttons:

- **MLX** — shows models with MLX-compatible formats (safetensors, .mlx files)
- **GGUF** — shows GGUF quantized models
- **All** — shows all models (union of MLX + GGUF + others)

The scope toggle is additive: clicking MLX then GGUF shows both. Clicking All shows everything. Defaults are platform-smart: macOS shows MLX+GGUF+All, other platforms show GGUF+All.

## Search and filtering

- **Repo ID input**: Enter a HuggingFace repo ID to search
- **Quants only toggle**: Filters to quantized models only
- **Official HF apps filter**: Uses HuggingFace's built-in app/category filters for MLX/GGUF
- **Quantized-only search**: Leverages HF's tag-based quantized filtering

## Sorting and categories

- **Sorting**: Models can be sorted by downloads, likes, or creation date
- **Categories**: Models are categorized (text-generation, image-text-to-text, etc.)
- **Author roles**: Models show author, uploader, and quantizer information

## Model cards

Each model is rendered as a card with:

- **VRAM bar**: Shows estimated VRAM based on the selected quantization
- **Context pills**: Show the native context ceiling; selecting a pill triggers VRAM recalculation
- **Format badge**: MLX, GGUF, or other format indicator
- **Quantization pills**: Available quantization levels with purple MLX-themed styling
- **Download button**: Appears on MLX models; initiates download and validates the directory

## Lineage cards (Phase 8B2)

Model cards display lineage information showing the model's ancestry:

- **Base model**: The original model the quantization is based on
- **Quantization source**: Which quantizer produced this version
- **MLX lineage**: For MLX models, shows the conversion source

Cards are grouped by base model name and sorted by downloads within each group. Groups are collapsible.

## Qualification badges (Phase 8B2)

Models receive badges based on verification status:

- **Verified quantizer**: The quantizer is verified by the model author or community
- **Community-verified**: The model has been verified by the community
- **CommunitySourceCatalog**: Integrated qualification data from Phase 8A

## CommunitySourceCatalog (Phase 8A)

The CommunitySourceCatalog provides structured information about model sources:

- **HF qualification/identity APIs**: Endpoint `/api/hf/qualify` and `/api/hf/identity` provide qualification data
- **Author roles**: Distinguishes model authors, uploaders, and quantizers
- **Quantizer verification**: Shows whether a quantizer is verified
- **Community-qualified models**: Models with community verification badges

### API endpoints

| Endpoint | Method | Auth | Description |
|----------|--------|------|-------------|
| `/api/hf/search` | GET | api-token | Search HuggingFace models |
| `/api/hf/models/:repo_id` | GET | api-token | Get model details |
| `/api/hf/models/:repo_id/profile` | GET | api-token | Get MLX profile from HF |
| `/api/hf/qualify` | POST | api-token | Qualify a model |
| `/api/hf/identity` | GET | api-token | Get author/quantizer identity |
| `/api/hf/repo/size` | GET | api-token | Get repository size |
| `/api/hf/models/:repo_id/safetensors` | GET | api-token | List safetensors files |
| `/api/hf/download` | POST | db-admin-token | Download a model |

### Model source kinds

The system classifies model sources into typed kinds:

- `mlx_directory` — Local MLX model directory
- `hugging_face_repo` — HuggingFace repository reference
- `alias` — Rapid-MLX alias (e.g., "Qwen2.5-0.5B-Instruct")
- `authoritative_safetensors` — Authoritative Safetensors source
- `gguf` — GGUF file (llama.cpp only)

### Format detection

Model format is detected from multiple signals:

1. **HF tags**: Models with `mlx` tag are classified as MLX
2. **File extension**: `.gguf` files are classified as GGUF
3. **Repository analysis**: The system fetches the repo tree to determine format
4. **CommunitySourceCatalog**: Qualified models have format data from the catalog

## MLX VRAM estimates

When browsing models, the system shows VRAM estimates for MLX models:

- **Pre-download estimation**: The backend fetches the model's metadata from HuggingFace (config.json for MLX, GGUF header for GGUF) without downloading the full model
- **VRAM estimation**: Uses the `/api/vram-estimate` endpoint with the model's real architecture
- **Quantization-aware**: VRAM estimates are calculated per quantization level
- **Context-aware**: VRAM recalculates when the user selects a different context size

## HF browse component

The `hf-browse.js` module handles HuggingFace model browsing:

- `initHfDownloadTab()` — Initializes the HF tab in the spawn wizard
- `hfSearch()` — Performs model search with scope filtering and grouping
- `createGroupVariant()` — Creates variant rows within a model group
- `extractBaseModelName()` — Extracts base model name for grouping
- `createSearchResult()` — Creates a single model result card

## Integration with spawn wizard

The HF browse component is integrated into the Spawn Wizard's Step 1 (Model):

- When Rapid-MLX is selected, the HF tab shows MLX-compatible models
- When llama.cpp is selected, the HF tab shows GGUF-compatible models
- Selecting a model populates the wizard's hardware settings
- VRAM estimation is triggered automatically on model selection
- Context pills and quantization pills are interactive and trigger recalculation
