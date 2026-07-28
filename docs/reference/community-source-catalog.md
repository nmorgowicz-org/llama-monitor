# Community Source Catalog

> **Status: Backend module (Phase 8A).** Not yet exposed via HTTP endpoints.

The Community Source Catalog is a user-editable, role-based catalog of HuggingFace model sources. It replaces the GGUF-only `KnownQuantizer` concept with a unified system that supports multiple role types per username and evidence-bearing claims.

## Purpose

When browsing HuggingFace models, users need to distinguish between:
- The original author who created the weights
- The quantizer who produced GGUF/MLX variants
- The curator who selects and organizes collections
- The merger/distiller who combines multiple models

The catalog encodes this distinction so the UI can show accurate lineage badges on model cards.

## Design constraints

- Curated/heretic/uncensored/discovery preferences are separate from technical qualification
- Original author is never conflated with converter roles
- All roles are evidence-bearing, not name-only claims
- User-editable with disk persistence

## Data model

### Catalog structure

```jsonc
{
  "entries": [...],     // All source entries (bundled + user-added)
  "preferences": {...}, // User preferences for discovery/filtering
  "version": 1          // Schema version for future migrations
}
```

### Entry fields

| Field | Type | Description |
|-------|------|-------------|
| `username` | string | HF username or org ID (e.g. `"bartowski"`, `"unsloth"`) |
| `display_name` | string | UI display name |
| `description` | string | Brief description of their contribution style |
| `role` | enum | Primary role (see below) |
| `also_known_for` | `enum[]` | Additional roles this entity performs |
| `categories` | `string[]` | Discovery categories (heretic, uncensored, etc.) |
| `note` | string? | Optional note explaining quirks or special behavior |
| `bundled` | bool | `true` = vendor-bundled, `false` = user-added |

### Roles

| Role | Label | Description |
|------|-------|-------------|
| `OriginalAuthor` | Original author | Created the original model weights or first fine-tune |
| `DatasetAuthor` | Dataset author | Created or curated the training dataset |
| `GgufQuantizer` | GGUF quantizer | Produced GGUF quantized weights |
| `MlxConverter` | MLX converter | Converted or produced native MLX weights |
| `Curator` | Curator | Selects, organizes, or publishes model collections |
| `MergerDistiller` | Merger / distiller | Merged or distilled from multiple models |
| `Custom` | Custom | User-defined role |

### User preferences

Separate from technical qualification — these are editorial choices:

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `prefer_heretic` | bool | `false` | Show heretic/abliterated variants prominently |
| `prefer_uncensored` | bool | `false` | Show uncensored variants prominently |
| `prefer_updated_finetune` | bool | `false` | Show updated-dataset finetunes/distillations prominently |
| `trusted_sources` | `string[]` | `[]` | Custom usernames/orgs the user trusts and wants featured |
| `custom_categories` | `string[]` | `[]` | Custom categories the user recognizes |

## Bundled entries

The catalog ships with 10 pre-populated entries:

| Username | Role | Also known for | Categories |
|----------|------|----------------|------------|
| `bartowski` | GgufQuantizer | — | — |
| `mradermacher` | GgufQuantizer | — | Note: i1-* files are imatrix quants |
| `unsloth` | OriginalAuthor | GgufQuantizer | updated-finetune |
| `lmstudio-community` | GgufQuantizer | — | — |
| `llmfan46` | GgufQuantizer | — | — |
| `DavidAU` | OriginalAuthor | MergerDistiller, GgufQuantizer | heretic, uncensored |
| `mudler` | Curator | GgufQuantizer | — |
| `Jackrong` | GgufQuantizer | — | — |
| `prithivMLmods` | GgufQuantizer | — | — |
| `mlx-community` | MlxConverter | — | — |

## File location and persistence

**File:** `{config_dir}/community-source-catalog.json`

**Legacy file:** `{config_dir}/hf-quantizers.json` (auto-migrated on first load)

The catalog loads in this priority order:
1. `community-source-catalog.json` — load directly if present
2. `hf-quantizers.json` — migrate legacy entries to new format
3. Default bundled catalog — if neither file exists

Writes are atomic (temp file + rename).

## Migration from legacy format

The `migrate_from_user_quantizers()` function converts old `hf-quantizers.json` entries:

- `"unsloth"` → `OriginalAuthor` (with `also_known_for: [GgufQuantizer]`, category: `updated-finetune`)
- `"mudler"` → `Curator`
- `"davidau"` → `OriginalAuthor` (with `also_known_for: [MergerDistiller, GgufQuantizer]`, categories: `heretic, uncensored`)
- Everything else → `GgufQuantizer`

Migrated entries have `bundled: false`.

## Hard gates

The catalog enforces role separation to prevent conflating creator with converter:

1. **Cannot add converter role for OriginalAuthor:** If a username already has `OriginalAuthor`, `upsert_entry()` rejects adding `GgufQuantizer` or `MlxConverter` for the same username.

2. **Cannot add OriginalAuthor when converter exists:** If a username already has `GgufQuantizer` or `MlxConverter`, `upsert_entry()` rejects adding `OriginalAuthor`.

3. **Bundled entries cannot be removed:** `remove_entry()` skips entries with `bundled: true`.

## How the catalog is used

### `/api/hf/identity` endpoint

The `hf_resolve_identity()` function in `src/hf/qualify.rs:1017` consumes the catalog to resolve author/converter roles for any HuggingFace repo:

1. Loads the cached catalog (`CATALOG_CACHE` in `hf/qualify.rs:18-32`)
2. Uses `entries_for_username()` to look up catalog entries for the repo owner
3. Resolves converter role based on format (`GgufQuantizer` for GGUF, `MlxConverter` for MLX)
4. Cross-references catalog `OriginalAuthor` entries when base models are present in tags
5. Returns `HfIdentity` with resolved roles

The catalog entry's `display_name` is used as the artifact publisher name when available; otherwise falls back to the raw username.

### Caching

The catalog is loaded once and cached in a `LazyLock<Mutex<Option<CommunitySourceCatalog>>>`. Subsequent API calls reuse the cached instance.

### No direct frontend usage

The catalog is entirely backend-driven. Frontend consumers receive its data indirectly through the `/api/hf/identity` API response, which includes resolved `roles` and `converter_role` fields.

## CRUD operations

These functions exist in the module but are **not yet exposed via HTTP endpoints** (Phase 8A):

| Function | Description |
|----------|-------------|
| `upsert_entry()` | Add or update an entry (enforces hard gates) |
| `remove_entry()` | Remove a user-added entry by username + role |
| `save_catalog()` | Write catalog atomically to disk |
| `reset_catalog()` | Reset to defaults, preserving user preferences |
| `entries_for_role()` | Get entries filtered by role |
| `entries_for_username()` | Get entries for a username across all roles |

## Key source files

| File | Lines | Role |
|------|-------|------|
| `src/models/community_source_catalog.rs` | 1-620 | Main module — all types, functions, tests |
| `src/models/mod.rs` | 5 | Module export |
| `src/hf/qualify.rs` | 14, 18-32, 1028, 1087, 1142-1208 | Catalog cache, identity resolution |
| `src/hf/mod.rs` | 354-393 | Legacy `UserQuantizer` + migration source |
| `src/web/api/hf.rs` | 895-931 | Route for `/api/hf/identity` |
| `docs/reference/hf-model-library.md` | 58-65 | Documentation reference |
