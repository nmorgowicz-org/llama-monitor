# Character Card Import (SillyTavern / Pyre-style)

Date: 2026-06-15
Status: Analysis / proposal

Goal: Understand our current chat/persona system, study how SillyTavern and Pyre handle character cards, and propose a practical import path so users can import characters from chub.ai, BotBooru, SillyTavern backups, etc. — instead of hand-crafting personas. Extend this into:

- A "character card mode" for roleplay with imported characters.
- A user persona system (so the AI knows about the user).
- An optional lorebook/memory system (static + auto-generated) for persistent context.

## 1. Current Chat / Persona System

### 1.1 High-level

- Persona/character is defined per chat tab as a system_prompt.
- The backend (Rust) is a thin relay: it forwards whatever messages array the frontend sends directly to llama.cpp.
- All prompt assembly, template wiring, and "guided generation" logic lives client-side in JS.

Implication: To support character cards, we mainly need to:
- Parse an external card (JSON or PNG-embedded JSON).
- Map its fields into tab-level data (system_prompt, ai_name, user_name, context_notes, etc.).
- Let existing _doSendChat flow handle the rest.

### 1.2 Data model (Rust)

- DB schema (src/chat_storage.rs, lines 15–40):
  - tabs.system_prompt TEXT NOT NULL DEFAULT ''
  - tabs.ai_name TEXT
  - tabs.user_name TEXT
  - tabs.explicit_level INTEGER
  - tabs.active_template_id TEXT (links to persona template)
  - tabs.ai_gender TEXT
  - tabs.context_notes TEXT (JSON) (structured lore/notes)
- ChatTabRow (src/chat_storage.rs, lines 138–170):
  - system_prompt, ai_name, user_name, ai_gender, active_template_id, context_notes

These fields are the direct hooks for a character card.

### 1.3 Persona templates

- Built-in personas defined client-side:
  - static/js/features/chat-templates.js:
    - SYSTEM_PROMPT_TEMPLATES (lines 10–102): simple label/value.
    - DEFAULT_TEMPLATES (lines 106–675+): rich templates with name, description, prompt, explicit_policies.
- User-defined templates stored server-side:
  - src/presets/mod.rs:
    - SystemPromptTemplate { id, name, prompt, explicit_policies } (lines 243–251)
  - Persistence: load_templates / save_templates (lines 263–299)
  - API:
    - GET /api/templates (api_get_templates, api.rs 6885–6902)
    - POST /api/templates (api_create_template, 6906–6930)
    - PUT /api/templates/:id (api_update_template, 6932–6964)
    - DELETE /api/templates/:id (api_delete_template, 6966–6994)

A character card import could:
- Create (or update) a persona template in /api/templates.
- Or directly produce a tab config (system_prompt, ai_name, etc.).

### 1.4 Prompt assembly (JS)

All prompt construction happens in:
- static/js/features/chat-transport.js, _doSendChat(tab), lines 430–873

Steps (key parts):

1) Start:
   - messages = []
   - systemParts = []

2) Base system prompt:
   - systemPrompt = substituteNames(tab.system_prompt, tab.ai_name, tab.user_name, tab.ai_gender)
     - Defined in static/js/features/chat-state.js, substituteNames, lines 657–664
     - Replaces:
       - {{char}} -> aiName || "AI"
       - {{user}} -> userName || "User"
       - {{gender}} -> gender || "neutral"

3) Explicit-mode policies (if tab.explicit_level > 0):
   - Uses active template's explicit_policies from resolveActiveTemplate(tab.active_template_id).

4) Context notes:
   - If tab.context_notes present and non-empty:
     - Inserted as ### <SECTION> NOTES ### blocks.

5) Quick guide (guided generation):
   - If quick_guide_instruction:
     - Inserted as ### QUICK GUIDE ###

6) Armed story beat:
   - If armedBeat?.instruction:
     - Inserted as ### ARMED STORY BEAT ###

7) Role boundary:
   - buildRoleBoundaryInstruction(tab) (lines 143–146)
   - Controls whether assistant can speak for user.

8) Compacted memory:
   - For messages with compaction_marker:
     - Injected as ### COMPACTED MEMORY ### with summaries.

9) Final system message:
   - If systemParts non-empty:
     - messages[0] = { role: "system", content: systemParts.join("\n\n") }

10) Chat history:
    - Only user/assistant messages (no system or compaction_marker).

11) Transient user prompt (for guided suggestions).

Then:

- POST /api/chat with messages array.
- Backend (api.rs, fn api_chat, lines 7747–7829):
  - Authenticates via api-token.
  - Derives llama.cpp URL.
  - Forwards request body verbatim (no prompt engineering).
  - Streams SSE response back.

Key conclusion: Any persona or character definition can be expressed as:
- A system_prompt (possibly including {{char}}, {{user}}, {{gender}})
- Tab metadata (ai_name, user_name, ai_gender, context_notes)
- And, optionally, initial messages for greeting/examples

The existing system is a clean host for SillyTavern-style cards.

## 2. Chat Architecture: Two Modes

We currently have one "mode" of chatting: simple system prompts with explicit/guided generation helpers. Adding character cards into that same flow is possible but risky: we'd risk overloading the current system with roleplay-specific logic (lorebooks, post-history instructions, etc.) that doesn't always fit a normal assistant chat.

Better: support two chat "modes," each with its own prompt assembly path, while sharing the same underlying DB and relay.

### 2.1 Assistant Mode (existing, unchanged)

Behavior:
- User selects a persona template (coder, roleplay companion, etc.) or writes a custom system prompt.
- Prompt assembly uses existing _doSendChat logic:
  - system_prompt
  - explicit policies
  - context_notes (if present)
  - quick guide / story beats
  - role boundary
  - compacted memory

Design intent:
- Concise, task-oriented, or lightly flavored chat.
- No requirement for deep character continuity, lore, or post-history instructions.
- Existing flows should behave exactly as they do today.

### 2.2 Character Card Mode (new)

Behavior:
- Triggered when a chat tab is bound to an imported character card.
- Uses a different prompt assembly path optimized for roleplay:
  - Character definition (description, personality, scenario)
  - User persona (so the AI knows the user's traits)
  - Character-specific lorebook/entries
  - Example dialogues
  - Post-history instructions (if card defines them)
- Still uses same /api/chat relay, same DB (system_prompt is just richer).

Design intent:
- Deep continuity, structured character knowledge.
- Compatible with SillyTavern chara_card_v2 conventions.
- Toggle-able: users choose when to use it.

Implementation approach:
- New tab field: chat_mode TEXT:
  - "assistant" (default, current behavior)
  - "character" (character card mode)
- When chat_mode == "character", prompt assembly diverges:
  - Uses a "character prompt builder" that:
    - Respects character card fields
    - Injects user persona
    - Injects lorebook entries
- Existing assistant mode is unaffected.

This keeps our current system stable and gives roleplay/character use its own tailored pipeline.

## 3. SillyTavern Character Card Format

### 3.1 V1 (Tavern) fields

Core fields (all strings, all optional/"" default):

- name: Character name.
- description: Full character/world description.
- personality: Short personality summary.
- scenario: Current situation/context.
- first_mes: Opening message (greeting).
- mes_example: Example dialogues:
  - Uses <START> between example exchanges.
  - Uses {{user}} for user, {{char}} for character.

These are injected into the "story string" (SillyTavern's system prompt template):
- description, personality, scenario, and mes_example all end up in instructions.
- first_mes is used as the first bot message.

### 3.2 V2 (chara_card_v2)

Canonical spec:
- https://github.com/malfoyslastname/character-card-spec-v2/blob/main/spec_v2.md

V2 wraps the data in:

{
  "spec": "chara_card_v2",
  "spec_version": "2.0",
  "data": {
    // Core V1 fields:
    "name": "string",
    "description": "string",
    "personality": "string",
    "scenario": "string",
    "first_mes": "string",
    "mes_example": "string",

    // V2 additions:
    "creator_notes": "string",
    "system_prompt": "string",
    "post_history_instructions": "string",
    "alternate_greetings": ["string"],
    "character_book": { ... },

    "tags": ["string"],
    "creator": "string",
    "character_version": "string",
    "extensions": { }
  }
}

Field usage:

- name: Character name.
- description: Main lore/description — goes into instructions.
- personality: Short personality — goes into instructions.
- scenario: Current scenario — goes into instructions.
- first_mes: Used as first assistant message.
- mes_example: Example dialogues used early in context.
- system_prompt:
  - Replaces/overrides global system prompt.
  - Supports {{original}} to merge with existing system prompt.
- post_history_instructions:
  - "Jailbreak" / post-history instruction appended after chat history.
- alternate_greetings:
  - Alternate first messages (for "swipes").
- character_book:
  - Lorebook with entries keyed by keywords.
  - Each entry:
    - id, name, keys, content
    - enabled, constant, priority, position, etc.
- tags: Classification (e.g. nsfw); not used in prompts.
- creator, creator_notes, character_version: Metadata, not used in prompts.
- extensions: Tool-specific metadata (SillyTavern/Risu data, depth_prompt, etc.)

### 3.3 PNG-embedded cards

SillyTavern cards are commonly distributed as PNG images with embedded JSON metadata.

- Storage:
  - PNG tEXt chunk with keyword "chara".
  - Value: V2 JSON serialized as a string, then base64-encoded.
- On import:
  - Read tEXt chunks.
  - Take text for keyword "chara".
  - base64-decode → JSON.

SillyTavern also supports a newer "ccv3" chunk (spec: "chara_card_v3") that takes precedence when present.

No zlib in the JSON blob itself — just:
JSON → UTF-8 bytes → base64 → tEXt chunk.

We don't need to fully replicate SillyTavern's template engine; we just need to:
- Parse these cards (JSON and PNG).
- Map fields into our system prompt / tab model.

### 3.4 BYAF / ZIP cards (optional)

SillyTavern also supports ZIP-based "BYAF" cards from Backyard AI.
Pyre can import full SillyTavern backups (.zip).
For a first pass, supporting:
- Plain JSON cards
- PNG-embedded v2 cards
- URL import from chub.ai (see below)
is sufficient. We can add ZIP/bulk import later.

Sources:
- SillyTavern repo: https://github.com/SillyTavern/SillyTavern
- Character card parser: src/character-card-parser.js
- Context template docs: https://docs.sillytavern.app/usage/prompts/context-template/
- BYAF parser: src/byaf.js

## 4. Chub.ai / CharacterHub URL Import

Chub.ai is a major character hosting platform; SillyTavern can import directly from URLs without downloading files. We should support this, since users expect it.

### 4.1 How SillyTavern does it

From SillyTavern's content-manager.js:

- Detects Chub URL via host:
  - chub.ai or characterhub.org
- Parses the URL to get:
  - character type vs lorebook
  - id as "creatorName/projectName"
- For characters, calls:
  - GET https://api.chub.ai/api/characters/{creatorName}/{projectName}?full=true
- From the JSON:
  - Reads metadata.node.definition fields:
    - name, personality, tavern_personality, scenario
    - first_message, example_dialogs
    - creator_notes (from description)
    - system_prompt, post_history_instructions, alternate_greetings
    - embedded_lorebook, extensions
  - Also reads topics (tags).
- Constructs a chara_card_v2-style JSON:
  - spec: "chara_card_v2"
  - spec_version: "2.0"
- Downloads the avatar from metadata.node.max_res_url.
- Writes JSON into the PNG using its card parser → imports as standard PNG card.

So the flow is:
User pastes Chub URL → SillyTavern talks to Chub API → builds V2 card → imports.

### 4.2 URL patterns

Canonical character URL:
- https://chub.ai/characters/{creatorName}/{projectName}

Alternative (legacy):
- https://characterhub.org/characters/{creatorName}/{projectName}

Lorebooks:
- https://chub.ai/lorebooks/{creatorName}/{projectName}

SillyTavern's parser is flexible and tolerates partial/short forms; for us, supporting the canonical URL shapes is enough.

### 4.3 How we should handle it

We have two viable approaches:

1) Frontend direct (simpler, no backend proxy needed):
   - User pastes a Chub URL into import dialog.
   - Frontend:
     - Validates shape.
     - Calls Chub API (CORS may be an issue; needs testing).
     - Normalizes response into chara_card_v2-style JSON.
     - Sends to our /api/characters/import.

2) Backend proxy (cleaner, avoids CORS):
   - User pastes URL into import dialog.
   - Frontend calls:
     - POST /api/characters/import-url
       - body: { url: "https://chub.ai/characters/creator/project" }
   - Backend:
     - Calls Chub API.
     - Normalizes into chara_card_v2 structure.
     - Saves character.

Recommendation:
- Start with backend proxy (avoids CORS headaches).
- If Chub disallows or rate-limits, fallback to user-provided JSON/PNG.

## 5. SillyTavern Persona (User Character)

SillyTavern defines "persona" as the user's own identity in chat, not the AI character. This is important for roleplay: the AI should know about the user (appearance, personality, preferences) so it can respond in-character and consistently.

### 5.1 What SillyTavern supports

- Persona:
  - A managed profile for the user:
    - Name (used as {{user}})
    - Avatar (visual only)
    - Description (free-text about the user)
    - Title (display-only, not in prompts)
- How it's used:
  - {{user}} resolves to the Persona name.
  - {{persona}} resolves to the Persona description.
  - The persona description is injected into the prompt (story string / system prompt), so the AI knows:
    - "User has a dragon tattoo."
    - "User prefers dark humor."
    - "User is an architect."
- Not part of chara_card_v2:
  - SillyTavern persona is SillyTavern-specific; not portable via character card import.

### 5.2 Why we need this

For character card mode, users will care that:
- The AI knows who "they" are.
- The AI can reference their traits, history, preferences.
- Importing a character doesn't mean the user is a blank slate.

We should:
- Add a User Persona feature, separate from AI characters.
- Let the user:
  - Define a persona (or multiple) with:
    - Name
    - Description (appearance, personality, backstory, preferences, etc.)
    - Optional avatar
  - Assign it to a chat (or globally).
- In character card mode, inject user persona into the prompt (similar to {{persona}}), either:
  - As a "User Profile" block in the system prompt, or
  - As part of the character prompt builder.

This fits naturally into our existing model:
- tab.user_name maps to {{user}}.
- A new field/tab setting for "user persona description" maps to {{persona}}.

We do NOT need to support chara_card_v2 for the user persona (since no spec exists), but we should make its fields structured enough to be clear and editable.

## 6. Lorebook / Memory System

Both SillyTavern and related extensions use "lorebooks" — sets of keyword-triggered text entries that get injected into context when relevant. We should support this for character card mode and, optionally, for regular chats.

There are three important references:
1) SillyTavern's built-in lorebooks.
2) SillyTavern-MemoryBooks: AI-assisted memory generation from chat.
3) SillyTavern-LoreManager: template-driven, rule-based, automatic lore extraction.

### 6.1 SillyTavern built-in lorebooks

Core idea:
- Collection of entries with:
  - id, name
  - keys (activation keywords)
  - content (text injected into context)
  - enabled, constant (always-on), priority
  - position (before/after character, etc.)
  - cooldown, probability, sticky, etc.
- On each prompt assembly:
  - SillyTavern scans recent chat for keywords.
  - Matching entries are injected into the context, up to a budget/limit.

For llama-monitor:
- Character cards can include character_book (V2 spec).
- We should support:
  - Basic keyword-triggered entries.
  - Constant entries (always included).
  - A context budget (e.g., up to X% of context window).

### 6.2 SillyTavern-MemoryBooks (AI-based memory generation)

Repo: https://github.com/aikohanasaki/SillyTavern-MemoryBooks

Problem:
- Long chats lose important details; manual lore is tedious.
- STMB automates memory: watches chat, generates structured "memory" entries via the AI.

Core workflow:
- Scene selection:
  - Manual: user marks a range with ►/◀ on messages.
  - Auto: after X messages since last processed, treats that range as a scene.
- Prompting:
  - Sends that range to the model with a prompt asking it to produce:
    - title
    - content: concise summary of events, facts, relationships
    - keywords: activation keywords
- Storage:
  - Each memory is a lorebook entry with metadata:
    - stmemorybooks flag
    - start/end message indices
    - position, order, vectorized/constant flags
- Coexistence:
  - STMB memories live in the same lorebook as hand-written entries.
  - Mixed use: static lore + dynamically generated memories.

Relevance:
- This approach is directly useful for llama-monitor:
  - For character card chats: auto-capture key story beats, character revelations.
  - For normal chats: auto-capture project decisions, design agreements, requirements.
- Integration idea:
  - After N new turns in a chat, run a background summarization pass.
  - Take messages since last processed.
  - Prompt the model to extract:
    - "Important facts, decisions, constraints, relationships."
    - Return JSON: { title, content, keywords[] }
  - Store as a "memory entry."
  - On prompt assembly, inject:
    - All constant entries.
    - Entries whose keywords match recent messages.
  - Make this:
    - Optional.
    - Per-chat or global toggle.
    - Configurable (every 20/30 messages, etc.)
  - Reuse our existing compaction/summarization infrastructure.

### 6.3 SillyTavern-LoreManager (rule-based dynamic lore)

Repo: https://gitgud.io/Monblant/sillytavern-loremanager

Core concept:
- Defines "groups" with input/output schemas (templates, regex).
- Every message is matched against input patterns.
- When matched: data is written/updated into lorebook entries automatically (no model call required).

Example:
- An "Inventory" group:
  - Input schema: "{{item}} added to inventory"
  - Output schema: appends item to inventory entry.
- An "Event" group:
  - Input schema: "{{event}} happened in {{location}}"
  - Output schema: creates/updates lore entry for event.

Update policies:
- REPLACE: overwrite fields.
- APPEND: grow lists (inventories, events).
- SIMPLE_APPEND: append raw lines.
- JSON_PATCH: apply JSON Patch to complex entries.

Key ideas relevant to us:
- Deterministic, pattern-based lore extraction complements AI-based memory:
  - AI: good for unstructured summary ("They decided X, Y, Z").
  - LoreManager: good for structured tracking (inventories, flags, ongoing plots).
- For llama-monitor, we could:
  - Start simple:
    - Basic lorebook: keyword entries (from character_book).
    - MemoryBooks-style: AI-generated scene memories (optional).
  - Later:
    - Let users define "extraction rules" (LoreManager-style):
      - "If model says 'You gained [item]', add to inventory entry."
      - "If 'Scene: [scene_name]', create/update scene entry."
- Not required for day one, but the design should allow it.

### 6.4 Proposed Lorebook / Memory Design (llama-monitor)

Start simple, design for extensibility.

- New concept: "Lorebook"
  - Collection of entries (structured or prose) that can be:
    - Attached to a character (imported from character_book).
    - Attached to a chat (auto-generated memories).
    - Global (user's reusable world info).

- Entry fields:
  - id
  - name (display name)
  - keys (activation keywords: ["sword", "Elias"])
  - content (text injected into prompt when triggered)
  - constant (bool; if true, always included)
  - priority (int)
  - position (enum; where to inject: e.g. "before_char", "after_scenario")
  - source (enum; "imported" | "ai_memory" | "manual" | "rule")

- Prompt integration (character card mode):
  - On _doSendChat:
    - Scan recent messages for keywords.
    - Select matching entries (up to context budget).
    - Inject into system prompt at designated positions.
  - For "assistant" mode:
    - Optional: allow using lorebooks for persistent project memory.

- Auto-memory (MemoryBooks-style):
  - New optional feature:
    - Trigger: every N turns.
    - Action:
      - Take last K messages.
      - Prompt model to summarize important facts as JSON.
      - Insert as new memory entries.
  - Configurable:
    - On/off globally.
    - Per-chat override.
    - Interval / message count.

## 7. Proposed Character Import (Updated)

### 7.1 Conceptual goals

- Let users:
  - Import character cards (JSON, PNG, or Chub URL) as AI characters.
  - Define their own user persona (for roleplay continuity).
  - Use imported characters + user persona + lorebooks in "character card mode."
- Retain existing "assistant mode" behavior unchanged.
- Support:
  - SillyTavern chara_card_v2
  - Chub.ai exports and URL imports
  - BotBooru / general PNG imports
  - Round-trip export (back to chara_card_v2 JSON)

### 7.2 Character model

We should introduce a canonical "Character" entity, separate from assistant-mode SystemPromptTemplate, though stored using the same infra.

Fields:
- id (String)
- name (String)
- description (String)
- personality (String)
- scenario (String)
- first_mes (String) (greeting)
- mes_example (String)
- system_prompt (String) (from card; used in character mode)
- post_history_instructions (String)
- character_book (JSON, raw from card)
- tags (String[])
- creator (String)
- character_version (String)
- extensions (JSON)
- source (String): "sillytavern" | "chub" | "manual" | "pyre" | "other"

Storage:
- For simplicity: stored as a SystemPromptTemplate with extended metadata, OR:
- Dedicated "characters" section in templates DB with these fields.

On import:
- Backend normalizes card into this model.
- Stores raw card fields (for round-trip/export).
- Composes a system_prompt for immediate use (or relies on character-mode prompt builder to assemble dynamically).

### 7.3 User persona model

New top-level concept: "UserPersona"

Fields:
- id
- name (String)
- description (String) (appearance, personality, backstory, preferences)
- avatar_url (String, optional)

Behavior:
- Assigned per chat or globally:
  - tab.user_persona_id (new field).
- In character card mode, injected into prompt as:
  - "User Profile: {description}"
  - {{user}} uses the name as usual.
- In assistant mode:
  - Optional: still useful for continuity ("AI knows the user is a developer named Alex").

Not chara_card_v2-based; our own format.

### 7.4 Prompt composition (Character Card Mode)

In "character" mode, prompt assembly diverges from current _doSendChat into a "character prompt builder."

High-level composition:

- System message includes:
  - Card's system_prompt (if set), or:
    - Character name/role
    - Character description
    - Personality
    - Scenario
    - Example dialogues (mes_example, cleaned)
  - Active lorebook entries:
    - All constant entries
    - Keyword-matched entries from recent messages
  - User persona (if set):
    - A "User Profile" block so AI knows who they're talking to.
  - Existing llama-monitor elements:
    - Context notes
    - Role boundary
    - Compacted memory
- After chat history (post_history_instructions):
  - If card defines post_history_instructions:
    - Injected after messages, as an additional directive (often in assistant mode as a final "user" or system block).

Notes:
- This mode's prompt is richer and more structured than assistant mode.
- Existing assistant mode remains untouched.

### 7.5 Example mapping (concrete)

Given a chara_card_v2:

{
  "spec": "chara_card_v2",
  "data": {
    "name": "Seraphina",
    "description": "Seraphina is an elven mage...",
    "personality": "Kind, curious, protective, hides a tragic past.",
    "scenario": "A traveler has just arrived.",
    "first_mes": "*Seraphina looks up.* \"Oh — hello there...\"",
    "mes_example": "<START>\n{{user}}: Where am I?\n{{char}}: You're in Willowmere.\n\n<START>\n{{user}}: Do you live here alone?\n{{char}}: For now."
  }
}

We'd store:

- character:
  - name: "Seraphina"
  - description: "Seraphina is an elven mage..."
  - personality: "Kind, curious, protective, hides a tragic past."
  - scenario: "A traveler has just arrived."
  - first_mes: "*Seraphina looks up.* \"Oh — hello there...\""
  - mes_example: "<START>..."

On chat start (character card mode):

- tab.chat_mode: "character"
- tab.ai_name: "Seraphina"
- tab.user_name: from UserPersona or default "User"
- Initial assistant message: first_mes

Prompt assembly:

You are Seraphina.

Character Description:
Seraphina is an elven mage...

Personality:
Kind, curious, protective, hides a tragic past.

Scenario:
A traveler has just arrived.

Example Conversations:
<START>
User: Where am I?
Seraphina: You're in Willowmere.

<START>
User: Do you live here alone?
Seraphina: For now.

(User Profile, if set)
User Profile:
Alex is a 32-year-old architect with a dragon tattoo on their left forearm...

(Lorebook entries, if active)

### 7.6 UI / flow (frontend)

Proposed UX:

- Persona menu enhancements:
  - Groups:
    - "Your Personas" (for AI)
    - "Characters" (imported character cards)
    - "Built-in" (existing templates)
  - New:
    - "Import Character Card" button:
      - Options:
        - Drag-and-drop PNG
        - Drag-and-drop JSON file
        - Paste URL (Chub / external)
        - Paste raw JSON
    - "User Persona" section:
      - Manage one or more user profiles.
      - Assign to current chat or globally.

Flow for import:

- PNG/JSON:
  - Frontend:
    - PNG → parse tEXt "chara" chunk → base64-decode → JSON.
    - JSON → parse directly.
  - Normalize:
    - Detect v1/v2.
  - Call backend:
    - POST /api/characters/import
      - Body: normalized character JSON.
  - Backend:
    - Validate.
    - Store character.
    - Return id + name.
  - Frontend:
    - Insert into "Characters" group.
    - Prompt: "Start a chat with this character?"
      - Creates new tab in "character" mode.

Flow for URL import:

- User pastes URL.
- Frontend:
  - POST /api/characters/import-url
    - body: { url: "https://chub.ai/characters/creator/project" }
- Backend:
  - Calls Chub API.
  - Normalizes response into chara_card_v2.
  - Stores and returns character.

### 7.7 Backend changes (Rust)

Required:

- New character import endpoint:
  - POST /api/characters/import
    - Auth: api-token.
    - Input: normalized card JSON.
    - Behavior:
      - Validate required fields (name).
      - Store as character.
      - Return id/name.
- New URL import endpoint:
  - POST /api/characters/import-url
    - Auth: api-token.
    - Input: { url: string }
    - Behavior:
      - Detect source (Chub).
      - Call external API.
      - Normalize and store.
- Extend DB:
  - Add tab.chat_mode (assistant/character) or equivalent discriminator.
  - Add tab.character_id (which character card is active).
  - Add tab.user_persona_id (user persona in use).

Optional (phase 2/3):

- Dedicated endpoints:
  - GET /api/characters/:id
  - PUT /api/characters/:id
  - DELETE /api/characters/:id
  - GET /api/characters/:id/export (chara_card_v2 JSON)
- User persona endpoints:
  - CRUD for user personas.
- Lorebook endpoints:
  - CRUD for lorebook entries.
  - Support auto-memory triggers (if implemented).

### 7.8 Frontend changes (JS)

Key changes:

- static/js/features/chat-templates.js:
  - Extend persona management:
    - New "Characters" group.
    - "Import Character Card" UI.
  - Add PNG parser utility:
    - Read file as ArrayBuffer.
    - Locate PNG tEXt chunk.
    - Extract chara keyword.
    - Base64-decode → JSON.

- static/js/features/chat-params.js:
  - Wire import buttons and user persona UI.

- static/js/features/chat-transport.js:
  - Update _doSendChat:
    - If tab.chat_mode == "character", branch into character-mode prompt builder:
      - Use character card fields.
      - Inject user persona.
      - Inject lorebook entries.
      - Inject post_history_instructions after history.

### 7.9 Handling character edits vs existing chats

From Pyre: on chat creation, freeze a copy of the character.

For llama-monitor:
- We already store system_prompt per tab.
- When using a character:
  - Copy relevant fields into the tab at creation time.
  - Future edits to the character don't change existing tabs.
- Provide:
  - "Update this chat with latest character version" as an optional action.

## 8. Implementation Plan (Proposed, Updated)

Phase 1 (core import + character mode):

- Backend:
  - Add character import:
    - POST /api/characters/import (JSON card).
    - POST /api/characters/import-url (Chub URL support).
  - Extend DB:
    - tab.chat_mode (assistant/character).
    - tab.character_id.
  - Store characters as structured templates or new "characters" collection.
- Frontend:
  - Add "Import Character Card" button and PNG parser.
  - Add "Characters" group to persona menu.
  - On import success, offer to create a chat in character mode.
- Prompt:
  - Implement character-mode prompt builder:
    - Use character description/personality/scenario/mes_example.
    - Use user persona (if set).
    - Integrate with existing _doSendChat routing.

Phase 2 (user persona + deeper character support):

- User persona:
  - CRUD for user personas.
  - Integrate into character-mode prompts.
- Character card fields:
  - Full support for post_history_instructions (inject after history).
  - Handle system_prompt from card (with {{original}}).
- Export:
  - Export characters as chara_card_v2 JSON for compatibility.

Phase 3 (lorebooks + auto-memory):

- Lorebook:
  - Import character_book from chara_card_v2.
  - Basic keyword-triggered injection in character mode.
  - Global/user-managed lorebooks for persistent world info.
- Auto-memory (MemoryBooks-style):
  - Optional per-chat feature.
  - Every N messages:
    - Summarize key facts into lorebook entries.
  - Configurable: on/off, interval, include prior memories or not.

Phase 4 (advanced lore + ecosystem):

- SillyTavern backup import:
  - Parse .zip, extract characters, bulk-import.
- BotBooru integration (optional):
  - Open BotBooru URL, user selects character, we import PNG.
- Rule-based lore extraction (LoreManager-style):
  - Allow users to define templates/regex for automatic lore updates from chat.

## 9. References

- SillyTavern:
  - Repo: https://github.com/SillyTavern/SillyTavern
  - Character card parser: src/character-card-parser.js
  - Content manager (Chub import): src/content-manager.js
  - Context template docs: https://docs.sillytavern.app/usage/prompts/context-template/
  - BYAF parser: src/byaf.js
- Character card spec:
  - V2: https://github.com/malfoyslastname/character-card-spec-v2/blob/main/spec_v2.md
  - V1: https://github.com/malfoyslastname/character-card-spec-v2/blob/main/spec_v1.md
- Pyre:
  - Repo: https://github.com/devemberteam-ops/Pyre
  - card_import.dart (mapping from chara_card_v2 to Character model)
  - st_backup_import.dart (SillyTavern bulk import)
- SillyTavern-MemoryBooks:
  - Repo: https://github.com/aikohanasaki/SillyTavern-MemoryBooks
  - AI-based scene memories as lorebook entries.
- SillyTavern-LoreManager:
  - Repo: https://gitgud.io/Monblant/sillytavern-loremanager
  - Template-driven, rule-based lore extraction and updates.
- llama-monitor internal:
  - Prompt assembly: static/js/features/chat-transport.js, _doSendChat, 430–873
  - Persona templates: static/js/features/chat-templates.js
  - Backend chat relay: src/web/api.rs, api_chat, 7747–7829
  - DB schema: src/chat_storage.rs, 15–40
  - Templates: src/presets/mod.rs, 243–299

## 10. UI/UX and AI-Automation Design

Purpose: define where character-card features fit in our existing, premium-style UI so:
- Users find them naturally.
- Flows feel native to llama-monitor.
- We lean on the model (remote or local) to reduce manual work.

All placements below are based on current UI structure (static/index.html and JS modules) and existing patterns (header toolbar buttons, Behavior panel, File menu, Context Notes sidebar).

### 10.1 Current UI map (short)

Relevant existing areas:

- Left sidebar:
  - Conversations list: #chat-sessions-panel
- Chat header toolbar:
  - Left: Behavior (#btn-behavior), Model (#btn-model-params), Style (#btn-chat-style), Compact (#btn-compact), AI name / You name pills, Explicit toggle.
  - Right: font size, Hide chat, File dropdown (#chat-file-btn), History Q&A, Focus Mode.
- Behavior panel (#chat-behavior-panel):
  - Active Persona row with name + "Manage" (#chat-open-template-mgr)
  - Character Gender
  - Role Boundary (collapsible)
  - Explicit toggle
- Model/Params panel (#chat-params-panel):
  - Sampling params, context/memory settings, auto-compact.
- Context Notes sidebar (#chat-sidebar):
  - Per-chat structured notes; right of message list.
- File dropdown (#chat-file-menu):
  - Save as Markdown / Save as JSON / Import conversation
- Template Manager modal (#template-manager-modal):
  - Left: template list
  - Right: preview/editor
- Input row (#chat-input-row):
  - Textarea, Send button, Suggestions, Guide AI

Key modules:

- static/js/features/chat-templates.js
- static/js/features/chat-params.js
- static/js/features/chat-state.js
- static/js/features/chat-transport.js
- static/js/features/chat-notes.js
- static/js/features/chat-sessions-sidebar.js

New character-card UI should:
- Use the same header-panel pattern.
- Avoid heavy global modals except where necessary (e.g., import dialog).
- Feel consistent with "Behavior", "Model", and "Style" panels.

### 10.2 Mode switch: Assistant vs Character Card

We need a clear, per-chat way to select mode, not a confusing global toggle.

Recommended placement:
- Behavior panel (#chat-behavior-panel), as the first row:
  - Add a small segmented/pill control:
    - [ Assistant ] [ Character ]
  - Label: "Mode" (or "Chat Mode") with a small hovercard tooltip.

Behavior:
- Default: "Assistant" for all existing and new chats.
- Switching to "Character":
  - Enables:
    - Character selection row (see below).
    - User Persona row (see below).
    - Lorebook/Memory row (see below).
  - Disables or hides:
    - Some assistant-only items (e.g., "Role Boundary" may become less relevant).
- Mode is stored per-tab: tab.chat_mode.

UX rationale:
- Lives where "Active Persona" already lives.
- Keeps mode tightly coupled to identity/personality.
- No new header button clutter.

### 10.3 Import Character Card

We want 3 ways to import, all discoverable:

1) From Behavior panel (primary)
2) From File menu (secondary, consistent with "Import conversation")
3) From New Chat flow (discovery-friendly)

1) Behavior panel (primary):
- In Behavior panel:
  - Add "Characters" row below Active Persona:
    - Label: "Characters"
    - Right-aligned button: "Import" (icon + label)
  - Click "Import" opens "Import Character Card" modal.

2) File menu (secondary):
- In #chat-file-menu:
  - Add "Import Character Card..." under existing "Import conversation".
  - Reuses the same modal as above.

3) New Chat / Conversations sidebar:
- In #chat-sessions-panel:
  - Near #csp-new-btn ("New Chat"):
    - Add a small "+" or icon button that offers:
      - New Chat (Assistant)
      - New Chat with Character Card
      - Import Character Card

Import Character Card modal:

- Style: same as Template Manager modal (#template-manager-modal) — centered, glassy, split pane.
- Left area: import options:
  - Drag-and-drop zone:
    - Label: "Drop PNG/JSON card" with subtle dashed border.
  - "Import from URL" input:
    - Text input + "Go" button.
    - Tooltip: "Chub.ai, CharacterHub, or direct card URL."
  - "Paste JSON" button:
    - Expands to a small textarea.
- Right area:
  - Live preview:
    - On parse success:
      - Character name
      - Avatar (from PNG if present)
      - Short summary (first 2–3 lines of description)
      - Tags
- Footer:
  - "Import as Character" button (primary)
  - "Start Chat" checkbox:
    - "Start a new chat with this character" (enabled by default).

AI-assisted UX hooks:
- When importing:
  - If card is long or messy, optionally ask model:
    - "Summarize this character's personality and style in 3–5 bullet points."
    - Show in the preview panel to help the user decide.
  - This uses the currently active model; no extra setup.

Implementation notes:
- Frontend:
  - PNG parser utility (to read tEXt "chara" chunk).
  - On import success, call:
    - POST /api/characters/import (PNG/JSON)
    - POST /api/characters/import-url (URL)
- Backend:
  - Normalize and store character.
  - Return id/name/short summary.

### 10.4 Character selection in Behavior panel

Once characters are imported, user should be able to assign one to a chat easily.

In Behavior panel, when in "Character" mode:
- Add a row:
  - Label: "Active Character"
  - Pill showing character name.
  - Button: "Select" (or drop-down).
- "Select" opens a list/modal:
  - Left:
    - Grouped list of characters:
      - "All"
      - "Recently used"
      - "Favorites" (later)
  - Right:
    - Quick preview:
      - Name, short description.
- On selection:
  - Set tab.chat_mode = "character"
  - Set tab.character_id
  - If chat is empty:
    - Insert first_mes as initial assistant message.
  - Update AI name pill (#chat-ai-name).

UX details:
- Behavior panel should show:
  - A subtle badge when character is active:
    - e.g., "🎭 Character: Seraphina" in the Active Character row.
- Hovercards or small "?" tooltips explain:
  - "This character defines how the AI behaves and speaks."

### 10.5 User Persona management

The user's persona should be simple, elegant, and optional.

Placement:
- In Behavior panel, under Active Character row (only in "Character" mode, or optionally visible in Assistant mode as a subtle row):
  - Row:
    - Label: "Your Persona"
    - Current: name or "None"
    - Button: "Manage"
- "Manage" opens:
  - A slim modal or right-drawer panel styled like Behavior panel:
    - List of user personas (if multiple).
    - "+ New Persona" button.
    - Selected persona's:
      - Name input.
      - Description textarea:
        - Placeholder: "Describe who you are (appearance, personality, preferences, backstory, etc.)"
      - Optional avatar upload (future).

AI-assisted UX hook:
- On "+ New Persona", optionally offer:
  - "Let AI help me write my persona."
  - Simple inline form:
    - A few quick prompts:
      - "Describe yourself in a few sentences."
      - Or select options:
        - Tone, style, key traits.
    - The model (current endpoint/local) generates a polished persona description.
- This keeps it premium and hands-off.

Behavior:
- Selected persona ID stored globally (in settings) and optionally per-tab (tab.user_persona_id).
- In Character mode:
  - Inject persona into prompt as:
    - "User Profile: {description}"
- In Assistant mode:
  - Optional: still inject, if enabled by user, as a general continuity aid.

### 10.6 Lorebook / Memory controls

Lorebook and memory features must be:
- Discoverable for RP users.
- Non-intrusive for assistant users.
- Heavily automation-friendly.

Best fit:
- Extend the existing Context Notes sidebar (#chat-sidebar):
  - Currently: user-authored notes.
  - Enhanced: "Notes + Memory" bar, with tabs or sections:
    - "Notes" (existing)
    - "Memory" (new; AI-generated and imported lore)
    - "Lore" (optional; character_book entries)

UX:

- In header or sidebar toggle:
  - Keep current "Context" toggle button.
  - In Character mode:
    - Label may become "Context & Memory".
- In sidebar:
  - Memory section:
    - List of memory entries:
      - Each shows:
        - Short title (e.g., "Elias revealed betrayal")
        - Timestamp / turn range (tiny)
        - Optional keywords (hover to reveal).
    - Actions:
      - Click entry: expand inline.
      - Right-click or small menu:
        - Edit
        - Delete
        - Mark as "Always On"
  - Lore section (if character_book present):
    - Entries imported from the card.
    - Similar UI as Memory, but clearly labeled.

AI-assisted UX:
- Auto-memory (MemoryBooks-style) should be mostly hands-off:
  - In Model/Params panel (#chat-params-panel), under existing "Context & memory" section:
    - New control: "Auto Memory"
    - Toggle: On/Off
    - Setting: "Generate after every N messages" (default: 20–30)
    - Checkbox: "Include previous memories as context"
  - When it triggers:
    - Subtle inline indicator:
      - e.g., small banner near the context bar or a message chip: "Updating memory..."
    - No interruptions; runs quietly using the active model.
  - User can:
    - Review generated memories in the sidebar.
    - Edit/delete as desired.
    - Turn it off if they don't want it.

For assistant mode:
- Same Auto Memory toggle, repurposed as:
  - "Persistent memory for this chat."
  - AI summarizes key decisions, constraints, references — same mechanism, same benefit.

### 10.7 AI-assisted prompt and character optimization

We should leverage the active llama-server (remote or local) to reduce friction:

1) On character import:
- After parsing card:
  - Optional step:
    - "Optimize prompt for this model"
    - Uses current model to:
      - Condense verbose cards into a tight system prompt.
      - Fix conflicting instructions.
    - UX:
      - Checkbox: "Optimize prompt with AI" (enabled by default).
      - Shows:
        - Before/After or just "Optimized" with collapsible diff.

2) On chat start:
- If user has character but no user persona:
  - Inline hint (small, one-time):
    - "Define your persona so {Character} can interact more realistically."
  - Click:
    - Opens the User Persona management with "Let AI help" option.

3) During chat:
- For memory entries:
  - Use the current model to generate concise, keyword-rich memory.
- For prompt tuning:
  - In Model/Params panel or Behavior panel:
    - Optional button:
      - "Refine character prompt"
    - The model:
      - Analyzes character + early conversation.
      - Suggests small improvements (tone, clarity).
      - Applied via a modal with diff-style preview.

All of this is "premium UX": invisible automation, user in control, no manual prompt engineering.

### 10.8 SillyTavern-style templates: Context Templates, Instruct Templates, and model formats

This is critical: SillyTavern users expect model-specific prompting behavior (e.g., Mistral Tekken, ChatML, Alpaca) and structured placement of:
- Character card fields
- Scenario
- Example dialogues
- Worldbook / lorebook
- Post-history instructions

Our system must map these properly, or roleplay quality will be bad and power users will complain.

Short answer:
- For OpenAI-style / Chat Completion APIs (which llama-monitor primarily uses), we do NOT need "Instruct Mode" at all.
- We DO need:
  - A robust "Context Template" (story string) that wires all character card and worldbook fields into a clean prompt.
  - Proper handling of post-history instructions after chat history.
  - Model-aware prompt composition (e.g., for ChatML/Alpaca-style models when used via raw text completion).

Below is how to translate SillyTavern concepts into our architecture.

#### 10.8.1 SillyTavern's two layers

SillyTavern separates:
1) Context Template (Story String):
   - Defines how character card fields, persona, worldbook, etc. are assembled.
   - Uses Handlebars-style macros:
     - {{description}}, {{scenario}}, {{personality}}
     - {{system}} (system prompt or character main prompt)
     - {{persona}} (user persona description)
     - {{char}}, {{user}}
     - {{anchorBefore}}, {{anchorAfter}}
       - Used for injecting author notes, summaries, STScript, web search, etc.
     - {{wiBefore}} / {{loreBefore}}: combined activated lorebook entries
2) Instruct Template:
   - Defines how the final prompt is wrapped for a model (Alpaca, Llama3, ChatML, Mistral Tekken).
   - Applies only in Text Completion Mode (not needed for Chat Completion APIs).
   - Specifies:
     - System / User / Assistant message prefixes and suffixes
     - Stop sequences
     - Role-specific wrappers

#### 10.8.2 How this maps to llama-monitor (Chat Completion APIs)

We currently:
- Use Chat Completion-style endpoints (OpenAI-compatible) via /api/chat.
- Send:
  - messages[0] = { role: "system", content: systemPrompt }
  - followed by user/assistant messages.

For Chat Completion APIs:
- The "Instruct Template" is handled automatically by llama.cpp / vLLM / API.
- Our job is to make sure:
  - The system message is built like a proper Context Template.
  - Character card fields, lorebook entries, and user persona are included.
  - Post-history instructions are injected in a way the model understands.

Implementation:

- In Character Card Mode, use a "character prompt builder" that constructs a structured system message:

You are {{char}}.

CHARACTER DESCRIPTION:
{{description}}

PERSONALITY:
{{personality}}

SCENARIO:
{{scenario}}

EXAMPLE DIALOGUES:
{{mes_example}}

USER PROFILE:
{{persona}}

LORE / WORLD INFO:
[lorebook entries, if active]

ROLE BOUNDARY AND BEHAVIOR:
[our existing role boundary]

NOTES / CONTEXT:
[context_notes / quick_guide / story beats]

COMPACTED MEMORY:
[existing memory blocks]

- Replace macros:
  - {{char}} → character.name
  - {{user}} → user persona name or "User"
  - {{persona}} → user persona description (or empty)
  - {{description}}, {{scenario}}, {{personality}}, {{mes_example}} → from character card

This is the core "Context Template" for our system. It:
- Matches the spirit of SillyTavern's story string.
- Is readable and debuggable.
- Keeps our existing assistant mode unchanged.

For post_history_instructions:
- SillyTavern appends these after chat history, often as an additional message or appended text.
- We can:
  - If using Chat Completion APIs:
    - Add an additional system or user message at the end:
      - e.g., { role: "system", content: post_history_instructions } after history
      - Or include it in the last user's message if that's safer for a given model.
  - This needs per-model testing, but is standard for "jailbreak" / instruction prompts.

#### 10.8.3 Text Completion APIs and instruct templates

If we later:
- Support raw text completion backends (Kobold, Horde, local text completion)
- Or expose advanced prompting for power users

Then we'd need to add:
- Instruct templates:
  - A selection UI (Model/Params panel or Behavior panel) for:
    - Alpaca, ChatML, Llama3, Mistral / Mistral Tekken, etc.
  - Each template defines:
    - How system / story string is wrapped
    - How user/assistant messages are wrapped
    - Stop sequences
- Integration:
  - The "Context Template" (system message) is first assembled.
  - Then instruct template wraps the full prompt into text before sending.

This is:
- Not required for initial character card support.
- Should be designed for (separate "Prompt Builder" module) so we can add it later cleanly.

#### 10.8.4 Worldbook / lorebook / scenario integration

In SillyTavern:
- Worldbook entries are:
  - Keyword-triggered or vectorized.
  - Injected into the story string at anchor points: {{anchorBefore}}, {{anchorAfter}}, or {{wiBefore}}/{{loreBefore}}.

For llama-monitor (Character Mode):

- We should:
  - Import character_book entries into our lorebook.
  - On each message:
    - Scan recent text for keyword matches.
    - Respect context budget (e.g., up to 60–80% of context).
    - Insert matched entries into system prompt at a dedicated "LORE / WORLD INFO" section (our {{wiBefore}} equivalent).

UX note:
- Keep this optional and well-hidden from users who don't need it.
- For advanced users, expose it under Context Notes sidebar as "Lore" section.

#### 10.8.5 Summary of SillyTavern translation

Concretely:

- Context Template:
  - YES — we must implement a structured system message builder that:
    - Combines character card fields, user persona, lorebook entries, and our existing guided-generation/context helpers.
    - Supports {{char}}, {{user}}, {{persona}} macros.
- Instruct Template:
  - Not required at first (Chat Completion APIs handle wrapping).
  - Design the prompt builder so that, if needed, we can add instruct templates later for raw text completion backends.
- Worldbook / Lorebook:
  - Keyword-triggered injection into system prompt.
  - Stored in the same lorebook system as AI-generated memories.
- Post-history instructions:
  - Injected as a final system/user message after chat history in Character Mode.

#### 10.8.6 Prompt Builder architecture (concrete)

To avoid tangled code and to support both Chat Completion and (eventually) raw Text Completion, we should design the prompt builder with a clear separation:

- Content layer: what data goes into the prompt.
- Wrapping layer: how that data is formatted for the target API/model.

Conceptually:

- buildPrompt(tab, character, userPersona, lorebookEntries):
    - storyString = buildStoryString(character, userPersona, lorebookEntries, tab)
    - history = tab.messages
    - postHistory = character.post_history_instructions || ""

    if (mode == "chat-completion") {
        systemPrompt = storyString
        messages = [
            { role: "system", content: systemPrompt },
            ...history,
            ...(postHistory ? [{ role: "system", content: postHistory }] : [])
        ]
        return { type: "chat-completion", messages }
    } else if (mode == "text-completion") {
        fullPrompt = applyInstructTemplate(storyString, history, postHistory, template)
        return { type: "text-completion", prompt: fullPrompt, stop: template.stopSequences }
    }

This design:
- Makes storyString a single source of truth.
- Keeps instruct templates isolated (swap-able, importable like SillyTavern).
- Allows Chat Completion and Text Completion backends to coexist cleanly.

#### 10.8.7 Story String builder (concrete)

buildStoryString should be deterministic and structured, similar to SillyTavern's context templates, but simplified.

Example (Chat Completion-style; our default):

function buildStoryString(character, userPersona, lorebookEntries, tab) {
    let parts = []

    // Core identity
    parts.push(`You are ${character.name}.`)

    // Character definition
    if (character.description) {
        parts.push(`CHARACTER DESCRIPTION:\n${character.description}`)
    }
    if (character.personality) {
        parts.push(`PERSONALITY:\n${character.personality}`)
    }
    if (character.scenario) {
        parts.push(`SCENARIO:\n${character.scenario}`)
    }

    // Example dialogues
    if (character.mes_example) {
        parts.push(`EXAMPLE DIALOGUES:\n${normalizeExampleDialogue(character.mes_example, character.name, userPersona?.name || "User")}`)
    }

    // User persona
    if (userPersona?.description) {
        parts.push(`USER PROFILE:\n${userPersona.description}`)
    }

    // Lorebook / world info
    if (lorebookEntries && lorebookEntries.length) {
        parts.push(`LORE / WORLD INFO:\n${lorebookEntries.map(e => e.content).join("\n\n")}`)
    }

    // Role boundary (our existing logic)
    parts.push(buildRoleBoundaryInstruction(tab))

    // Context notes / guided generation
    if (tab.context_notes) {
        parts.push(`NOTES:\n${renderContextNotes(tab.context_notes)}`)
    }

    // Quick guide / story beats / explicit policies
    if (tab.quick_guide_active) {
        parts.push(`QUICK GUIDE:\n${tab.quick_guide_active}`)
    }

    // Compacted memory
    if (tab.compacted_memory) {
        parts.push(`COMPACTED MEMORY:\n${tab.compacted_memory}`)
    }

    return parts.filter(Boolean).join("\n\n")
}

Notes:
- normalizeExampleDialogue should:
  - Replace {{char}}/{{user}} with actual names.
  - Optionally normalize common variants from other platforms: {char}/{user}, <BOT>/<USER>.
- All of this can live in JS (e.g., static/js/features/chat-character-prompt.js).

#### 10.8.8 Instruct templates (for text completion; design-forward)

For text completion (your primary SillyTavern experience), each instruct template defines:
- How story string / system prompt is wrapped.
- How user/assistant messages are wrapped.
- Stop sequences.

Examples (simplified):

- Alpaca-style:
  - System/story:
    - "Below is an instruction describing a character and scenario.\n\nWrite an appropriate response."
  - User:
    - "### Instruction:\n{message}"
  - Assistant:
    - "### Response:\n{message}"
  - Stop: ["### Instruction:\n"]

- ChatML-style:
  - User:
    - "<|im_start|>user\n{message}<|im_end|>\n"
  - Assistant:
    - "<|im_start|>assistant\n{message}<|im_end|>\n"
  - System/story:
    - "<|im_start|>system\n{storyString}<|im_end|>\n"
  - Final prompt ends with:
    - "<|im_start|>assistant\n"
  - Stop: ["<|im_end|>"]

- Mistral (Mistral Tekken):
  - User:
    - "[INST] {storyPrefixIfNeeded}\n{message} [/INST]"
  - Assistant:
    - " {message}"
  - Final prompt ends after:
    - " [/INST]"
  - Stop: ["[/INST]", "[/inst]"]

For llama-monitor:
- We should:
  - Define a simple JSON/in-memory structure per instruct template:
    - systemWrap(storyString)
    - userWrap(msg)
    - assistantWrap(msg)
    - firstAssistantPrefix(msg)
    - lastAssistantPrefix(msg)
    - stopSequences[]
  - Store built-in templates for major families.
  - Allow:
    - Loading from JSON (like SillyTavern presets).
    - User edits, if desired.

Integration:

- buildPrompt(tab, ..., mode == "text-completion"):
    - storyString = buildStoryString(...)
    - sysBlock = template.systemWrap(storyString)
    - historyParts = history.map(m =>
        m.role === "user" ? template.userWrap(m.content)
        : template.assistantWrap(m.content)
      )
    - postHistoryBlock = template.userWrap(postHistory) // often as user instruction
    - fullPrompt = sysBlock + historyParts.join("") + postHistoryBlock + template.lastAssistantPrefix("")
    - return fullPrompt

This exactly matches how SillyTavern's instruct templates work, so users familiar with those concepts will map cleanly.

#### 10.8.9 Summary of SillyTavern translation (final)

Concretely:

- Context Template:
  - YES — we must implement a structured system message builder that:
    - Combines character card fields, user persona, lorebook entries, and our existing guided-generation/context helpers.
    - Supports {{char}}, {{user}}, {{persona}} macros.
- Instruct Template:
  - Not required at first (Chat Completion APIs handle wrapping).
  - Design the prompt builder so that, if needed, we can add instruct templates later for raw text completion backends.
- Worldbook / Lorebook:
  - Keyword-triggered injection into system prompt.
  - Stored in the same lorebook system as AI-generated memories.
- Post-history instructions:
  - Injected as a final system/user message after chat history in Character Mode.

