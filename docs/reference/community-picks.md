# Community Picks

Purpose:
- Show a curated list of recommended local LLM models (by category) in the
  spawn wizard’s “Discover” area, based on community discussion.

How it works:
- The app expects a JSON file at:
  - ~/.config/llama-monitor/community-picks.json
- The backend serves it via GET /api/hf/community-picks.
- The frontend:
  - Fetches that endpoint once on load.
  - If data is present, shows an expandable “Community picks” panel with:
    - Category tabs
    - Model cards; clicking a model pre-fills the HF repo and loads files.
  - If the file is missing, invalid, or the endpoint fails:
    - The panel is hidden (no stuck loader).

JSON format (expected by the code):

{
  "generated_at": "2025-01-01T00:00:00Z",
  "categories": [
    {
      "id": "agentic",
      "label": "Agentic / Tool use",
      "description": "Good for agent workflows and tool calling.",
      "models": [
        {
          "hf_repo": "bartowski/Some-Model-GGUF",
          "display_name": "Some Model 70B",
          "param_b": 70,
          "is_moe": false,
          "why": "Strong agentic performance; reliable tool calls.",
          "quant_rec": "Q4_K_M",
          "mention_count": 5,
          "source_subreddits": ["LocalLLaMA", "SillyTavernAI"],
          "last_seen": "2025-01-01"
        }
      ]
    }
  ]
}

Key fields the UI uses:
- hf_repo: used to populate the HuggingFace repo field.
- display_name, why, param_b, quant_rec, is_moe, mention_count: used in the model card.
- Categories: used for tabs; only categories with at least one model should be
  included to avoid confusing empty sections.

How the file is supposed to be created:
- It is NOT bundled with the app; it is generated externally by a curation
  process that:
  - Scrapes communities (e.g., r/LocalLLaMA, r/SillyTavernAI)
  - Aggregates recurring recommendations
  - Applies thresholds (mention_count, param ranges)
  - Writes the resulting JSON to ~/.config/llama-monitor/community-picks.json
- A reference prompt describing this workflow lives in:
  - docs/reference/hermes-community-picks-cron.md
- There is currently no built-in automation (no GitHub Actions or internal cron).
  This is an out-of-band process; if it’s not running, the panel is hidden.

Development / testing:
- For UI screenshots and tests, an example fixture is used:
  - docs/reference/community-picks-example.json
- In CI/test harness (tests/ui/capture.mjs), if no real file exists, it copies
  this example into the temp config so the feature is visible in screenshots.
- When developing locally:
  - Copy that example into your config dir as community-picks.json, or
  - Create your own minimal version using the schema above.

If you are unsure whether to change this:
- If you are editing the community picks data, follow the external curation
  process (Hermes prompt). Don’t hardcode picks directly into app code.
- If you are editing the UI/backend:
  - Assume the file may be missing or malformed.
  - The panel must gracefully hide instead of showing dead UI or stuck loaders.
