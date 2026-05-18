# Chat

The chat tab provides multi-conversation streaming chat against the connected llama.cpp server, with per-conversation prompts, parameters, guided-generation tools, and live telemetry.

## Tab Management

- **Multi-tab conversations** — Each tab keeps its own message history, system prompt, persona, explicit level, model parameters, context notes, and guided-generation state
- **Pinned tabs** — Pinned conversations stay grouped at the front in both the top tab strip and the conversation sidebar
- **Drag reorder** — Drag tabs within the pinned or unpinned section to change order
- **Keyboard switching** — `Ctrl+1` through `Ctrl+9` jump by tab position; `Ctrl+Shift+Left/Right` cycles
- **Rename** — Double-click the top tab label or use the sidebar context menu
- **Delete with undo** — Closing a tab moves it into an in-memory trash bin with an Undo toast; the trash list is not persisted across reloads

## Conversation Sidebar

The left conversation sidebar is the main organizer for chat sessions.

![Conversation Sidebar](../screenshots/sidebar-sidebar-expanded.png)
![Conversation Sidebar Collapsed](../screenshots/sidebar-sidebar-collapsed.png)

- **Recency groups** — Conversations are grouped into `Pinned`, `Today`, `Yesterday`, `This Week`, and `Older`
- **Per-conversation status** — Each row shows the conversation name, persona label, explicit-mode badge, message count, and a context-pressure bar derived from the last known context percentage
- **Collapse/expand** — The collapsed state persists in `localStorage` and is restored when the page is reopened
- **Title filter** — The inline filter narrows the sidebar list by conversation names and visible persona labels only
- **Message search entry point** — A dedicated `Search Messages` button sits under the title filter so full-text search is visible without hunting for a header icon
- **Context menu** — Rename, pin/unpin, export JSON, export Markdown, duplicate, and delete are available from the `...` menu

![Conversation Context Menu](../screenshots/chat-context-menu.png)

### Cross-Conversation Message Search

The sidebar's `Search Messages` button opens a larger flyout beside the conversation rail and searches message bodies across stored conversations.

![Conversation Search](../screenshots/sidebar-fts-search-active.png)

- **Message search** — Searches stored message content, not just conversation names
- **Wider results surface** — Matches render in a dedicated flyout instead of replacing the narrow sidebar list
- **Result paging** — Results load in pages so long histories stay scannable even when a query returns dozens of matches
- **Prefix and punctuation tolerant** — Short fragments and punctuation-heavy queries are normalized before matching
- **Collapse-safe** — If the sidebar was collapsed before search, closing search restores that state
- **Jump to match** — Clicking a result switches to the matching tab, scrolls to the stored message row, and briefly highlights it

## Messaging

- **Streaming** — Real-time SSE streaming from `/v1/chat/completions`
- **Markdown rendering** — Assistant output is rendered with Markdown, syntax-highlighted code blocks, and per-block copy controls
- **Thinking blocks** — If the upstream model sends `reasoning_content`, the UI renders it in an expandable thinking block during the active browser session
- **Token estimates** — The composer shows a rough `~N tok` estimate with warning colors at higher counts
- **Smart scroll** — Auto-scroll stays on only while you are near the bottom; scrolling upward during generation disables follow mode until you jump back down
- **Unread badge** — New assistant replies increment a scroll-to-bottom unread badge when you are reading older content
- **History pagination** — Long conversations render only the newest messages first (default 15) and expose older history through `Load More`
- **RP dialogue highlighting** — Quoted dialogue is colorized even when Markdown formatting splits the text across inline tags

### Upstream Busy Handling

- **Monitor-side serialization** — llama-monitor now serializes its own chat-adjacent inference requests so it does not issue overlapping requests from the same app session.
- **Single-slot upstream protection** — When the active llama.cpp server is already occupied, the chat transport waits briefly for the current request to finish before attempting a new one.
- **Explicit transport errors** — If the upstream stays busy, the transport now surfaces a busy response instead of a generic `500`. Offline or dropped-upstream cases are also reported explicitly.
- **Shared behavior across tools** — The same admission logic applies to normal chat sends, guided suggestions, quick-guide rewrites, keyword generation, and context-note analysis because they all share the same upstream chat-completions transport.

### Message Actions

| Action | Description |
|--------|-------------|
| **Edit** | Edit a user message and regenerate from that point |
| **Regenerate** | Re-run from a prior user turn to get a different assistant reply |
| **Copy** | Copy message text to the clipboard |
| **Export JSON** | Download the active tab as a single-item JSON array containing the current in-memory tab object |
| **Export Markdown** | Download only the visible conversation transcript as `**You**` / `**Assistant**` blocks separated by `---` |
| **Import JSON** | Create a new tab from the first element of the JSON array |
| **Import Markdown** | Parse `**You**` and `**Assistant**` blocks and append them to the active tab as new messages |

## Personas & Template Manager

The template manager is the central place for chat personas.

![Persona Manager](../screenshots/guided-gen-persona-modal.png)

### Template List Sections

| Section | Description |
|---------|-------------|
| **Active** | The persona currently applied to this tab |
| **Custom** | User-created personas |
| **Built-in** | Personas shipped with llama-monitor |

### Applying a Persona

- Click the persona chip in the chat header to open quick-select
- Applying a persona stores its `active_template_id` on the current tab
- Use the pencil action on the chip to open the full manager

### Per-Persona Explicit Policies

Each persona stores separate Level 1 and Level 2 explicit-policy text. When explicit mode is enabled, the active persona's policy text is appended to the system prompt for that tab.

### Token Substitution

System prompts support:

| Token | Replaced With |
|-------|---------------|
| `{{char}}` | The AI name for the tab |
| `{{user}}` | The user name for the tab |
| `{{gender}}` | The AI gender (`male`, `female`, or `neutral`) |

### Custom Role Boundary

Each tab can override the default role-boundary instruction with `role_boundary_custom`. If blank, the app generates a default boundary from the current AI and user names.

## Behavior Panel

The behavior panel provides fast access to the active persona prompt, role-boundary controls, AI gender, and explicit-mode settings for the current tab.

![Behavior Settings](../screenshots/panels-behavior-settings.png)

## Model Parameters

Per-tab controls for generation behavior. A dot indicator appears when the active tab differs from defaults.

| Parameter | Default | Description |
|-----------|---------|-------------|
| Temperature | `0.7` | Randomness |
| `top_p` | `0.9` | Nucleus sampling threshold |
| `top_k` | `40` | Top-k sampling |
| `min_p` | `0.01` | Minimum probability threshold |
| `repeat_penalty` | `1.0` | Repetition penalty |
| `max_tokens` | `4096` | Reply length cap |
| `stream_timeout` | `120s` | Abort if no content arrives within this interval |

![Response Settings](../screenshots/panels-model-settings.png)

## Context Compaction

Compaction turns older history into a memory/tombstone entry so the conversation can continue inside the model context window.

### Compact Confirmation Modal

- **Stats preview** — Shows message counts, estimated tokens freed, current context %, and model capacity
- **Summary preview** — When summarization is enabled, the generated summary appears before you confirm and can be edited
- **Exit animation** — The modal fades out after a completed compaction

### Compaction Modes

| Mode | Behavior |
|------|----------|
| **Percent** | Triggers once context usage passes the configured threshold |
| **Optimized** | Triggers when the remaining context budget drops below a fixed reserve |

- **Auto-summarize** — Uses the model to summarize dropped history instead of only trimming it
- **Threshold slider** — Per-tab auto-compact threshold
- **Rolling memory aware** — Existing compaction markers are folded back into later requests as `COMPACTED MEMORY`

## Prompt Debug Inspector

The debug inspector shows the exact outbound request shape used for the next reply. Open it with the `{...}` button in the chat input toolbar.

![Prompt Debug Inspector](../screenshots/panels-prompt-debug.png)

### What It Shows

| Section | Description |
|---------|-------------|
| **System slices** | The assembled system prompt split into base prompt, context notes, quick guide, armed story beat, role boundary, and compacted memory sections when present |
| **History** | The non-system message history that will actually be sent upstream |
| **Totals** | Rough token estimates versus the model context capacity |
| **Timing** | Prompt and generation timings once a reply finishes |
| **Model params** | The exact sampling parameters used for the request |

## Chat Telemetry

Real-time metrics for the active chat tab, accessible from the telemetry toggle in the chat header.

### Summary Rail

- **State chip** — Idle, prompting, or generating
- **Stage indicator** — Prompt vs generation phase
- **Throughput bars** — Prompt and generation speeds
- **Live rate** — Current generation tokens per second
- **Context ring** — Last known context pressure for the tab

### Expanded Detail Panel

- **Throughput grid**
- **Sparkline**
- **Task metadata**
- **Slot tiles**
- **Activity timeline**

The panel can float as a popover or be pinned inline below the toolbar.

![Chat Telemetry](../screenshots/chat-chat-telemetry.png)
![Chat Telemetry Pinned](../screenshots/chat-chat-telemetry-pinned.png)

## Chat Style

The style panel changes message presentation for the current browser.

| Style | Description |
|-------|-------------|
| **Rounded** | Rounded bubbles with shadows |
| **Compact** | Tighter spacing and lighter chrome |
| **Minimal** | Flat layout with minimal decoration |
| **Bubbly** | Larger bubbles with stronger visual treatment |

Style selection persists in `localStorage` under `llama-monitor-chat-style`.

### Font Scaling

Message font size can be adjusted from 70% to 150% via the style panel.

### Date Format

Message timestamps follow the per-user appearance setting:

| Format | Example |
|--------|---------|
| `MM/DD/YY` | `05/06/26` |
| `DD/MM/YY` | `06/05/26` |
| `YYYY-MM-DD` | `2026-05-06` |
| `locale` | Browser locale |

### Enter Behavior

Enter-to-send is stored as a browser preference. When disabled, `Enter` inserts a newline and `Ctrl+Enter` sends.

## Guided Generation

Guided-generation features shape the next assistant reply without forcing you to rewrite the base persona or model settings for the whole tab.

### Context Notes Sidebar

The right-side context notes panel stores structured notes on the active tab and injects them into the system prompt as grouped `### SECTION NOTES ###` blocks.

![Context Notes Sidebar](../screenshots/guided-gen-context-notes-expanded.png)

#### Built-In Sections

| Section | Purpose |
|---------|---------|
| Character | Character traits, motivations, voice, and relationships |
| Setting | Places, world rules, atmosphere |
| Plot/Scenario | Current beats, stakes, and scenario facts |
| Tone | Mood, pacing, and stylistic guardrails |

#### Behavior

- **Per-tab notes** — Notes live on the tab, not globally
- **Multiple notes per section** — A section can contain several entries; they are concatenated when injected
- **Custom sections** — You can create additional section names, which are also persisted with the tab
- **Resizable width** — Width is stored on the tab and also mirrored in `localStorage` for UI restore
- **Expanded/collapsed state** — The open state and intro visibility are browser-local `localStorage` preferences

#### AI Review

The `Analyze` action calls `POST /api/context-notes/analyze`.

- **Default scan depth** — The initial review uses the last 20 messages
- **Per-section full-context rerun** — Individual sections can be re-analyzed against the full conversation when needed
- **Statuses** — `new`, `current`, and `stale`
- **Actions** — Add the suggested note, replace your existing note, keep your version, skip, or delete the current note

### Suggestions Dropdown

Suggestions generate user-side next-step ideas from the current conversation context.

![Suggestions Dropdown](../screenshots/guided-gen-suggestions-dropdown.png)

The browser sends recent messages, the current system prompt, non-empty context notes, and the active quick-guide instruction (if one is currently active) as suggestion context.

![Tag Cloud](../screenshots/guided-gen-suggestions-tag-cloud.png)
![Search Filter](../screenshots/guided-gen-suggestions-search-filter.png)
![Suggestions Results](../screenshots/guided-gen-suggestions-results.png)

#### Focus Keywords

The setup panel can auto-generate focus keywords through `POST /api/keywords/generate`. That request disables model thinking for a fast keyword-only result.

#### Suggestion Draft Rewrite

`Edit Draft` opens a workspace that turns a suggestion into a fuller user-side message. The rewrite pass tries to match recent user voice and point of view before dropping the result into the main composer.

#### Custom Categories

Custom categories appear alongside built-in ones and persist to `~/.config/llama-monitor/suggestion-categories.json`.

### Manage Categories

![Manage Categories](../screenshots/guided-gen-manage-categories.png)

- **Built-in prompts** — Editable, reorderable, and individually disableable
- **Custom categories** — Add your own groups and prompt lists
- **Per-prompt edits** — Name, description, and prompt text

### Quick Guide

Quick Guide is the inline steering surface for one-off reply direction.

| Mode | Description |
|------|-------------|
| **Quick** | Applies a direct instruction to the next guided reply, then clears it |
| **Director** | Expands one directing note into four continuation options |
| **Surprise** | Arms a hidden future beat that lands on a later assistant reply |

![Quick Guide](../screenshots/guided-gen-quick-guide-dropdown.png)
![Director Mode](../screenshots/guided-gen-director-options.png)
![Director Results](../screenshots/guided-gen-director-applied.png)

#### Quick Mode Details

- **Draft persistence** — The unsent quick-guide draft is stored on the tab
- **Immediate guided follow-up** — Submitting a quick guide triggers a guided reply flow instead of only changing future defaults
- **Restore previous guide** — If the last quick-guide reply is restorable, the app removes that assistant reply and reopens the instruction for editing

#### Surprise Mode Details

- **Delayed beats** — Each surprise stores `kind`, normalized instruction text, and `remaining_turns`
- **Countdown** — A beat fires when `remaining_turns` reaches `0`; other armed beats decrement after assistant replies complete
- **Per-tab queue** — Armed surprises are part of tab state

![Surprise Mode Armed](../screenshots/guided-gen-surprise-armed.png)

## Explicit Mode

Explicit mode is a three-level content filter layered on top of the active persona.

| Level | Icon | Description |
|-------|------|-------------|
| **Off** | 🔒 | Default filtering |
| **Unlocked** | 🔓 | Level 1 persona policy |
| **Unrestricted** | 🔥 | Level 2 persona policy |

![Explicit Unlocked](../screenshots/guided-gen-explicit-unlocked.png)
![Explicit Unrestricted](../screenshots/guided-gen-explicit-unrestricted.png)
![Explicit Locked](../screenshots/guided-gen-explicit-locked.png)

### Controls

- **Footer toggle** — Fast level switch in the composer footer
- **Behavior panel** — Full explicit controls for the current tab

## Message Management

| Feature | Description |
|---------|-------------|
| **Message limit** | Controls how many messages are rendered at once (default 15) |
| **Copy settings** | Copies prompt and parameter settings from another tab into the current one |
| **AI/You names** | Per-tab display names used in the UI and prompt token substitution |
| **Tab trash** | Deleted tabs are restorable from the in-memory trash menu until the page reloads or the trash entry ages out |

## Export & Import

### Markdown Export

- Exports only non-system messages
- Uses `**You**` and `**Assistant**` headings plus `---` separators
- Does **not** include timestamps, token counts, personas, system prompts, notes, or model parameters

### JSON Export

- Exports the active tab as a one-element array
- Includes the current in-memory tab object, including messages and tab-level settings that exist in the browser state at export time
- This is the only built-in export that carries personas, model params, notes, explicit level, quick-guide drafts, and similar tab metadata

### Import Behavior

| Format | Behavior |
|--------|----------|
| **Markdown** | Appends parsed user/assistant blocks to the active tab with fresh import timestamps |
| **JSON** | Creates a new tab from the first array element in the file |

### Reasoning / Thinking Content

Thinking blocks are currently a live-session UI feature, not a durable storage feature.

- Assistant `thinking_content` can appear in the browser while a reply streams
- JSON export can include `thinking_content` if it is still present in the in-memory tab object
- The current SQLite tab/message schema does not store `thinking_content`, so those blocks are not restored after a reload from `chat.db`

## Data Flow

```text
User message -> /v1/chat/completions (SSE stream) -> Browser renders tokens live
                                                   -> Chat telemetry updates from live metrics
```

## Persistence

Chat persistence is backed by SQLite, not by the old flat JSON store.

- **Primary store** — `~/.config/llama-monitor/chat.db`
- **Schema** — Conversations live in `tabs` and `messages` tables, with full-text search on message content
- **Legacy migration** — If `~/.config/llama-monitor/chat-tabs.json` exists at startup, the app imports it into `chat.db` and renames the old file to `chat-tabs.json.bak`
- **Write pattern** — The browser saves whole-tab updates through `/api/chat/tabs/:id` and flushes pending tab data on page unload
- **Search backing** — Cross-conversation message search reads from the database-backed FTS index, not from transient browser-only state
