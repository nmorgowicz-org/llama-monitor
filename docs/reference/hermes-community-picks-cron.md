# Hermes Cron: Community Model Picks for llama-monitor

This document is the canonical skill prompt for the Hermes bot cron job that
scrapes community model recommendations and produces `community-picks.json` for
the llama-monitor app.

---

## Purpose

Scan r/LocalLLaMA and r/SillyTavernAI for model recommendations, then write a
structured JSON file to `~/.config/llama-monitor/community-picks.json`. The
llama-monitor app reads this file and surfaces the picks inside the HF download
wizard under a "Community picks" panel.

---

## Prompt (feed this to Hermes)

```
You are a model-recommendation researcher. Your job is to scan recent Reddit
discussions and identify the most-mentioned local LLM models in specific
categories, then output a structured JSON file.

## Data sources (use your MCP tools to fetch these)

1. r/SillyTavernAI — search for the weekly "Best Models" megathread.
   These threads are stickied or have "megathread" or "best model" in the title.
   Look for the most recent one (within the last 60 days). Parse the comments for
   model names, sizes, and user opinions.

2. r/LocalLLaMA — search for posts in the last 30 days that discuss:
   - "best model for roleplay", "best RP model", "best ERP model"
   - "best model for agents", "function calling", "tool use"
   - "best MoE", "best offload", "best for limited VRAM"
   Focus on top-voted posts and their top comments.

3. Also check any r/SillyTavernAI posts about "model tier list" or "model
   recommendations" from the last 30 days.

## Size constraints

- **RP/ERP category**: Only include models with 20B–70B parameters. Skip anything
  below 20B (too small for quality RP) or above 70B (requires more than 2× 24GB
  GPU for Q4). For MoE models, use total parameter count for the size filter.

- **MoE / Offload category**: Include models with 20B–130B *total* parameters
  where partial GPU offload is realistic (model has sparse MoE architecture, so
  effective active params are much lower). Common examples: Mixtral 8x7B (47B
  total, 13B active), DeepSeek MoE variants, Qwen MoE variants.

- **Agentic category**: 7B–70B range; focus on models praised for function
  calling, multi-step reasoning, tool use, or coding. No size restriction below
  70B because smaller models can be surprisingly capable agents.

## Extraction rules

For each mentioned model:
- Find the canonical HuggingFace repo ID of the *base* GGUF quantizer (prefer
  bartowski, unsloth, or lmstudio-community; prefer bartowski for most models).
  Format: `author/RepoName-GGUF`
- Count how many distinct users/comments mention it positively (mention_count).
  If a single power-user post generates 50 upvotes but only 1 person mentioned
  it, mention_count = 1. Count unique recommenders, not upvotes.
- Extract the "why" from their words: a single sentence capturing the most
  commonly stated reason. Keep it under 100 characters. Paraphrase, don't quote.
- Set quant_rec to the most commonly recommended quantization, or "Q4_K_M" if
  not specified.
- Set is_moe = true for MoE architectures (Mixtral, Qwen MoE, DeepSeek MoE,
  Gemma 4 MoE variants, etc.).

## Minimum threshold

Only include a model if it has mention_count >= 2 (at least 2 different people
recommended it in the scanned period). Exception: if a category has fewer than
3 qualifying models, include the top 1–2 even at mention_count = 1.

## Output format

Write ONLY the JSON below — no markdown, no explanation. Write it to the file
path shown at the end.

```json
{
  "schema_version": "1",
  "generated_at": "<ISO-8601 timestamp of now>",
  "generated_by": "hermes-cron",
  "categories": [
    {
      "id": "roleplay_erp",
      "label": "RP / ERP",
      "description": "Community-recommended roleplay and ERP models (20B–70B, local-first)",
      "models": [
        {
          "hf_repo": "bartowski/MODEL-NAME-GGUF",
          "display_name": "Human-readable model name",
          "param_b": 70.0,
          "is_moe": false,
          "why": "One sentence. Why community recommends it. Under 100 chars.",
          "quant_rec": "Q4_K_M",
          "mention_count": 7,
          "source_subreddits": ["SillyTavernAI", "LocalLLaMA"],
          "last_seen": "YYYY-MM-DD"
        }
      ]
    },
    {
      "id": "moe_offload",
      "label": "MoE / Offload",
      "description": "Mixture-of-experts models viable with partial GPU offload (20B–130B total)",
      "models": []
    },
    {
      "id": "agentic",
      "label": "Agentic",
      "description": "Models praised for tool use, function calling, multi-step reasoning",
      "models": []
    }
  ]
}
```

Sort models within each category by mention_count descending.
If a model qualifies for multiple categories, include it in each.

## Output destination

Write the JSON to: ~/.config/llama-monitor/community-picks.json

Overwrite if it already exists.
```

---

## Schema reference (for llama-monitor integration)

The app reads `community-picks.json` from `$config_dir/community-picks.json`
(default: `~/.config/llama-monitor/community-picks.json`) via `GET /api/hf/community-picks`.

### Top-level fields

| Field | Type | Description |
|---|---|---|
| `schema_version` | `"1"` | Always `"1"` for this format |
| `generated_at` | ISO-8601 string | When the file was produced |
| `generated_by` | string | Free-form provenance string |
| `categories` | array | Ordered list of category objects |

### Category object

| Field | Type | Description |
|---|---|---|
| `id` | string | Machine identifier (`roleplay_erp`, `moe_offload`, `agentic`) |
| `label` | string | Display name shown in the tab bar |
| `description` | string | Tooltip / subtitle |
| `models` | array | Ordered list of model objects (desc by mention_count) |

### Model object

| Field | Type | Required | Description |
|---|---|---|---|
| `hf_repo` | string | yes | Full HF repo ID, e.g. `bartowski/Llama-3.3-70B-Instruct-GGUF` |
| `display_name` | string | yes | Human-readable name shown in UI |
| `param_b` | number | yes | Parameter count in billions (total, not active) |
| `is_moe` | bool | yes | True for MoE architectures |
| `why` | string | no | Why community recommends it (≤100 chars) |
| `quant_rec` | string | no | Recommended quantization level |
| `mention_count` | int | no | Number of distinct recommenders found |
| `source_subreddits` | string[] | no | Which subreddits were the source |
| `last_seen` | `YYYY-MM-DD` | no | Most recent date a recommendation was found |

---

## Cron schedule suggestion

Run weekly (or after each SillyTavern megathread is posted, typically Fridays).
The file overwrites the previous one; the app reflects changes immediately on
the next time the HF download panel is opened.

Example crontab (runs every Sunday at 03:00):
```
0 3 * * 0  hermes run community-picks-cron
```
