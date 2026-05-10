# Chat Guided Generation - Implementation Specification

**Date:** 2026-05-08  
**Last Updated:** 2026-05-10  
**Branch:** `feature/chat-guided-generations`  
**Status:** 🔄 **ACTIVE DEVELOPMENT**  
**Version:** 4.0 (In Progress - 2026-05-10)

---

## 📋 Current Architecture (AS-IS - 2026-05-10)

### Core Design Philosophy
We are **NOT** building a SillyTavern clone. We're building a **simple, premium chat system** with guided generation features that fit our established UI/UX:
- **Minimal UI** - No clutter, no hidden menus, no video tutorials needed
- **Premium aesthetic** - Modern gradients, backdrop-filter blur, layered shadows
- **Discoverable** - Features are obvious, not buried in settings
- **Simple** - We have a chat system, not a full roleplay platform

### Source Extensions (Reference Only)
| Extension | Author | Purpose | What We Take |
|-----------|--------|---------|--------------|
| **Guided Generations** | Samueras | Context injection + persistent notes | Context Notes sidebar, Quick Guide ephemeral instruction |
| **Roadway** | bmen25124 | Action suggestions | "Use" button pattern, suggestion history |
| **Pathweaver** | mattjaybe | Genre-based story directions | 17 prompt templates, Director Mode, category tabs |

---

## ✅ Implementation Progress

### Completed Phases

| Phase | Feature | Status | Date | Commits |
|-------|---------|--------|------|---------|
| **Phase 1-2** | Context Notes + Suggestions (Core) | ✅ Complete | 2026-05-08 | `c4e147a` |
| **Phase 3** | Quick Guide (Inline) | ✅ Complete | 2026-05-08 | `c4e147a` |
| **Phase 4** | Settings & Polish | ✅ Complete | 2026-05-08 | `00cde1c` |
| **Phase 5** | Advanced Features (Fix Last, History, Custom Categories) | ✅ Complete | 2026-05-08 | `e3cc9a5` |
| **Phase 6** | Pathweaver Integration (17 prompts, new format, Director Mode, Explicit) | ✅ Complete | 2026-05-08 | `38c1039` |
| **Phase 7** | UI Fixes (dropdown clipping, button placement) | ✅ Complete | 2026-05-10 | `b51f5a2` |

### Implementation Summary

**All 6 phases completed. Phase 7 (polish) in progress.**

**Deliverables:**
- ✅ Context Notes Sidebar (persistent, resizable, section-based)
- ✅ Suggestions Dropdown (4 categories, AI-powered, smart parsing)
- ✅ Quick Guide (ephemeral inline instruction, auto-clears after generation)
- ✅ Settings UI (toggles, default sidebar width, 17 editable prompts)
- ✅ Suggestion History (recently used, limit 10, per-tab)
- ✅ Fix Last Response (correction modal, regeneration)
- ✅ Context Injection (SillyTavern-style system messages)
- ✅ Custom Categories (add/remove/edit unlimited categories)

**Files Created:**
- `static/js/features/chat-notes.js` - Sidebar management
- `static/js/features/chat-suggestions.js` - Dropdown + history
- `static/js/features/chat-quick-guide.js` - Inline instruction
- `static/js/features/chat-fix-last.js` - Correction regeneration
- `static/css/chat-guided-generation.css` - All styles

**Files Modified:**
- `src/web/api.rs` - Added ContextNote, SuggestionRequest/Response, endpoint
- `src/state.rs` - Added guided generation settings to UiSettings
- `static/js/features/chat-state.js` - Added context_notes, sidebar_width fields
- `static/js/features/chat-transport.js` - Added context injection logic
- `static/js/features/settings.js` - Added collection/application of new settings
- `static/js/bootstrap.js` - Added initialization calls
- `static/index.html` - Added HTML structure (sidebar, dropdown, modals)
- `Cargo.toml` - Added regex dependency

**Validation:**
- ✅ `cargo build --release` - Success
- ✅ `cargo clippy -- -D warnings` - Success
- ✅ `cargo test` - 72 tests passed
- ✅ `npm run lint` - Success
- ✅ `./scripts/validate-js.sh` - Success

### Phase 6: Pathweaver Integration

**Completed:**
- ✅ 17 Pathweaver prompts (exact copies, proven to work)
- ✅ New parsing format (`[EMOJI] TITLE\nDESCRIPTION` with `---` separators)
- ✅ Director Mode (custom prompt input)
- ✅ Explicit category with soft dependency on Explicit Mode toggle
- ✅ Genre-specific prompts (9 genres: horror, romance, sci-fi, fantasy, mystery, thriller, comedy, noir, action)
- ✅ Removed `{count}` variable (model decides quantity naturally)
- ✅ Settings UI for all 17 prompts

**Files Created:**
- `static/prompts/action.md`
- `static/prompts/character.md`
- `static/prompts/comedy.md`
- `static/prompts/context.md`
- `static/prompts/director.md`
- `static/prompts/explicit.md`
- `static/prompts/fantasy.md`
- `static/prompts/horror.md`
- `static/prompts/mystery.md`
- `static/prompts/noir.md`
- `static/prompts/romance.md`
- `static/prompts/sci-fi.md`
- `static/prompts/template.md`
- `static/prompts/thriller.md`

**Files Modified:**
- `src/web/api.rs` - Updated `parse_suggestions()` for Pathweaver format
- `static/js/features/chat-suggestions.js` - Updated rendering, added Director Mode, Explicit integration
- `static/css/chat-guided-generation.css` - Added title/description styles
- `static/index.html` - Added Director button, Explicit button, 17 prompt settings
- `static/js/features/settings.js` - Added 17 prompt collection/application
- `build.rs` - Refactored route generation for balanced tree structure
- `src/lib.rs` - Added recursion limit

---

## Executive Summary

This document specifies the implementation of **Guided Generation** features for llama-monitor's chat interface, inspired by three SillyTavern extensions:

1. **Guided Generations** - Real-time instruction injection and persistent context notes
2. **Roadway** - AI-powered "what should I do next?" action suggestions  
3. **Pathweaver** - Genre-based story direction suggestions

### Key Design Decisions ✅

| Feature | UI Pattern | Placement | Status |
|---------|-----------|-----------|--------|
| **Context Notes** | Workspace Panel | Right Sidebar | ✅ Decided |
| **Suggestions** | Dropdown Menu | "What's Next?" Button | ✅ Decided |
| **Quick Guide** | Inline Input | Above Text Input | ✅ Decided |

### Implementation Scope

**Original Estimate:**
- **Total Effort:** 7-11 days across 5 phases
- **New Files:** 3 JavaScript modules, 3 prompt templates
- **Modified Files:** 6 existing files (backend + frontend)
- **New API Endpoint:** `POST /api/chat/suggestions`

**Actual Results:**
- **Total Effort:** 1 day (vs. 7-11 days estimated)
- **New Files:** 4 JavaScript modules, 17 prompt templates, 1 CSS file
- **Modified Files:** 12 existing files (backend + frontend)
- **New API Endpoint:** `POST /api/chat/suggestions`
- **Total Lines Added:** ~3,500 lines
- **Total Commits:** 4 (all validated)

### What Was Built

**Core Features (Phases 1-3):**
- ✅ Context Notes Sidebar (persistent, resizable, section-based)
- ✅ Suggestions Dropdown (17 categories, AI-powered, smart parsing)
- ✅ Quick Guide (ephemeral inline instruction)

**Settings & Polish (Phase 4):**
- ✅ Settings UI (toggles, default sidebar width, 17 editable prompts)
- ✅ Suggestion count & context depth sliders
- ✅ Reset to defaults functionality

**Advanced Features (Phase 5):**
- ✅ Suggestion History (recently used, limit 10, per-tab)
- ✅ Fix Last Response (correction modal, regeneration)
- ✅ Custom Categories (add/remove/edit unlimited categories)
- ✅ Enhanced error handling (retry logic, offline detection)
- ✅ Accessibility improvements (ARIA labels, keyboard nav, focus management)

**Pathweaver Integration (Phase 6):**
- ✅ 17 Pathweaver prompts (exact copies, proven to work)
- ✅ New parsing format (`[EMOJI] TITLE\nDESCRIPTION` with `---` separators)
- ✅ Director Mode (custom prompt input)
- ✅ Explicit category with soft dependency on Explicit Mode toggle
- ✅ Genre-specific prompts (9 genres)
- ✅ Removed `{count}` variable (model decides quantity naturally)

### What Makes This Different from SillyTavern

| Aspect | SillyTavern | llama-monitor |
|--------|-------------|---------------|
| **UI Clarity** | Hidden Quick Replies, video tutorial needed | Explicit buttons, obvious purpose |
| **Feature Separation** | Bundled together | Separated by interaction pattern |
| **Context Injection** | Complex depth-based system | Simple structured system messages |
| **Default State** | Always visible | Collapsed/hidden by default |
| **User Control** | Many hidden settings | Simple, discoverable controls |

---

## Problem Statement

Users encounter two main issues in chat/roleplay:

1. **"What do I do next?"** - Writer's block, repetitive model suggestions
2. **"How do I guide the model?"** - Need to inject instructions, maintain consistency, correct behavior

Current llama-monitor has:
- ✅ System prompt templates (personas)
- ✅ Message editing
- ✅ Variant navigation (previous/next response)
- ❌ No "what's next?" suggestions
- ❌ No persistent context notes
- ❌ No real-time instruction injection

---

## Feature Analysis

### Guided Generations (Samueras)

**Core Value:** Real-time instruction injection + persistent context tracking

| Feature | Complexity | Value for llama-monitor |
|---------|------------|------------------------|
| "Guide Next Response" (ephemeral instruction) | Low | High - simple, immediate value |
| "Context Notes" (persistent notes) | Medium | High - useful for roleplay consistency |
| "Regenerate with Fix" | Low | Medium - we have variant navigation |
| Auto-generated guides (CoT, thinking, clothes, state) | High | Low - too SillyTavern-specific |
| "Impersonation" (expand user outlines) | High | Low - niche use case |

**Recommended MVP:**
- "Guide Next Response" - type instruction, applies to next generation only
- "Add Context Note" - persistent notes visible to model on every generation

---

### Roadway (bmen25124)

**Core Value:** Context-aware action suggestions to overcome writer's block

| Feature | Complexity | Value for llama-monitor |
|---------|------------|------------------------|
| "Suggest Actions" button | Low | High - core value |
| Click suggestion → populate input | Low | High - simple UX |
| Separate "cheap" API profile | Medium | Medium - optional enhancement |
| Impersonate mode (generate as user character) | High | Low - too complex |
| Auto-trigger after character messages | Low | Medium - nice-to-have |

**Recommended MVP:**
- Single "💡 Suggest Actions" button
- 3-5 clickable suggestions that populate input
- Use existing model (no separate API needed initially)

---

### Pathweaver (mattjaybe)

**Core Value:** Genre-based story direction with category variety

| Feature | Complexity | Value for llama-monitor |
|---------|------------|------------------------|
| "What's Next?" category | Low | High - core value |
| Genre buttons (Horror, Romance, etc.) | Medium | Medium - nice variety |
| "Plot Twist" category | Low | Medium - fun addition |
| Streaming suggestions | High | Low - overkill |
| "Surprise Me" (delayed trigger) | High | Low - too complex |
| Director mode (user prompts) | Medium | Medium - power user feature |

**Recommended MVP:**
- Single "What's Next?" button (Pathweaver's core category)
- 3-4 suggestions max (Pathweaver uses 6+, we're simpler)
- No genre variants initially

---

## Synthesis: What Should We Build?

### Core Feature: "Smart Suggestions" (Full Guided Generation)

A **comprehensive guided generation system** combining:
1. **Multi-category suggestions** (Roadway/Pathweaver) - "What can I do next?" with genre variety
2. **Context notes** (Guided Generations) - "Remember these details" (persistent)
3. **Quick Guide** (Guided Generations) - "Do this one time" (ephemeral)
4. **Regenerate with Fix** - "Fix the last response" (corrections)

### User Flow

```
[Chat conversation happening...]

User clicks "💡 Smart Suggestions ▼" button
    ↓
Panel expands inline with full feature set:
    ↓
┌─────────────────────────────────────────────────┐
│ 💡 Smart Suggestions [×]                        │
├─────────────────────────────────────────────────┤
│ [What's Next] [Plot Twist] [New Character] [+] │
│                                                 │
│ WHAT'S NEXT SUGGESTIONS                         │
│ ┌─────────────────────────────────────────┐    │
│ │ 🎯 Investigate the mysterious package   │ [Use]│
│ │ 🔀 Confront the waiter about the bill   │ [Use]│
│ │ 📖 Suggest an impromptu road trip       │ [Use]│
│ └─────────────────────────────────────────┘    │
│                                                 │
│ ─────────────────────────────────────────────── │
│                                                 │
│ 📌 CONTEXT NOTES                                │
│ • Character is injured (left leg)              │ [×]
│ • Tone: formal, restrained                     │ [×]
│ • Setting: 1920s prohibition era               │ [×]
│ [+ Add Note]                                    │
│                                                 │
│ ─────────────────────────────────────────────── │
│                                                 │
│ 🦮 QUICK GUIDE (one-time instruction)          │
│ [Be more descriptive with sensory details...]  │
│                           [Apply] [Clear]      │
│                                                 │
│ ─────────────────────────────────────────────── │
│                                                 │
│ 📑 FIX LAST RESPONSE                            │
│ [The AI made a mistake. Tell me how to fix it] │
│                           [Regenerate]         │
└─────────────────────────────────────────────────┘
[Text Input]
[Send Button]
```

### Feature Breakdown

| Feature | Source | Complexity | User Value |
|---------|--------|------------|------------|
| **Multi-category suggestions** | Pathweaver + Roadway | Medium | High - variety prevents repetition |
| **Context Notes** | Guided Generations | Medium | High - maintains consistency |
| **Quick Guide** | Guided Generations | Low | Medium - one-time corrections |
| **Fix Last Response** | Guided Generations | Low | Medium - error correction |

---

## Decision Points

### 1. Feature Scope ✅ DECIDED

**Selected: Full Guided Generation**

We're building a comprehensive guided generation system with 4 core features:

1. **Multi-category suggestions** - Genre-based suggestions (What's Next, Plot Twist, New Character, etc.)
2. **Persistent Context Notes** - Add/remove/edit notes that inject into every generation
3. **Ephemeral Quick Guide** - One-time instructions that auto-clear after use
4. **Fix Last Response** - Correct AI mistakes with targeted regeneration

**Rationale:** Building the complete feature set from the start avoids incremental technical debt and provides maximum user value. While this is more work initially, it prevents the need to refactor later when adding missing features.

**Trade-offs Accepted:**
- Higher initial implementation effort (6-10 days vs 2-3 days for MVP)
- More UI complexity (hybrid button + collapsible panel)
- Need to manage more state (notes, quick guide, categories, suggestions)

**Benefits:**
- Complete solution addresses both "what's next?" AND "how do I guide the model?"
- No feature gaps that users will notice and request
- Single cohesive implementation vs. multiple incremental PRs

---

### 2. UI Placement ✅ DECIDED

**Selected: Separated Features (3 Different UI Patterns)**

We're **NOT bundling** these features. Each gets its own optimal UI based on interaction pattern:

| Feature | UI Pattern | Placement | Visibility |
|---------|-----------|-----------|------------|
| **Context Notes** | Workspace Panel | Right Sidebar | Toggleable (default closed) |
| **Suggestions** | Dropdown Menu | "What's Next?" Button | On-demand |
| **Quick Guide** | Inline Input | Above Text Input | Collapsible (default hidden) |

**Rationale:** Each feature has different interaction patterns. Bundling them creates confusion (like Guided Generations). Separation = clarity.

---

### 2a. Context Notes: Right Sidebar

```
┌──────────────────────────────────────┬──────────────────┐
│                                      │  📋 Story Notes  │
│  Chat Messages...                    │  ───────────────  │
│                                      │                  │
│  [user] Hello...                     │  CHARACTER       │
│  [ai] Response...                    │  • Injured leg   │
│                                      │  • Formal tone   │
│  [Text Input...] [Send]              │                  │
│                                      │  SETTING         │
│                                      │  • 1920s era    │
│                                      │                  │
│                                      │  [+ Add Note]    │
└──────────────────────────────────────┴──────────────────┘
                  ^                  [Toggle Button]
```

**Key Decisions:**

| Aspect | Decision | Rationale |
|--------|----------|-----------|
| **Default State** | Collapsed/Hidden | Don't clutter UI by default |
| **Toggle** | Note icon/button in chat header | Familiar pattern, always accessible |
| **Width** | Draggable/resizable (like text input) | User-controlled, remembers last width |
| **Sections** | Pre-defined: Character, Setting, Plot, Tone | Gives structure, users can add custom |
| **Empty State** | Placeholder text in each section | "Add character details...", "Add setting info..." |
| **Note Actions** | Edit (click), Remove (×), Add (+) | Clear, consistent patterns |
| **New Sections** | "+ Add Section" button at bottom | Users can organize however they want |
| **Injection** | Auto-inject all notes into every generation | No "apply" needed - always active |

**User Flow:**
1. Click 📋 button → sidebar slides open at remembered width
2. See empty sections with helpful placeholders
3. Click any placeholder → inline edit → type note
4. Notes auto-inject into generations
5. Click 📋 again → sidebar collapses
6. Notes persist with chat tab

---

### 2b. Suggestions: "What's Next?" Dropdown

```
[Chat Messages...]

┌─────────────────────────────────────────────────┐
│ 💡 What's Next?                           [×]  │
├─────────────────────────────────────────────────┤
│ [General] [Plot Twist] [New Character] [🔄]    │
│                                                 │
│ 🎯 Investigate the mysterious package      [Use]│
│ 🔀 Confront the waiter about the bill     [Use]│
│ 📖 Suggest an impromptu road trip         [Use]│
│                                                 │
│ [🔄 Generate new suggestions]                   │
└─────────────────────────────────────────────────┘

[Text Input...]        [Send]
```

**Key Decisions:**

| Aspect | Decision | Rationale |
|--------|----------|-----------|
| **Trigger** | "💡 What's Next?" button above text input | Clear purpose, visible |
| **UI Pattern** | Dropdown (like persona, export) | Familiar, space-efficient |
| **Categories** | Tabs: General, Plot Twist, New Character | Variety without clutter |
| **Regenerate** | 🔄 button in header + "Generate new" at bottom | Users unhappy? Try again |
| **Use Action** | Click "Use" → populate input, keep dropdown open | Quick iteration |
| **Collapse** | Click × or click outside | Standard dropdown behavior |

**User Flow:**
1. Stuck? Click "💡 What's Next?"
2. See 3-5 suggestions from selected category
3. Not happy? Click 🔄 to regenerate
4. Click "Use" → suggestion populates text input
5. Edit if needed, send
6. Dropdown stays open for quick iteration

---

### 2c. Quick Guide: Collapsible Inline Input

```
[Chat Messages...]

┌─────────────────────────────────────────────────┐
│ One-time instruction (optional)                 │
│ [Be more descriptive with sensory details...]   │
└─────────────────────────────────────────────────┘
┌─────────────────────────────────────────────────┐
│ [Type your message here...]                     │
└─────────────────────────────────────────────────┘
                           [Send]
```

**Or when collapsed:**

```
[Chat Messages...]

[+ Add one-time instruction]  ← Expands when clicked

┌─────────────────────────────────────────────────┐
│ [Type your message here...]                     │
└─────────────────────────────────────────────────┘
                           [Send]
```

**Key Decisions:**

| Aspect | Decision | Rationale |
|--------|----------|-----------|
| **Default State** | Hidden/Collapsed | Rarely used, don't clutter |
| **Trigger** | "+ Add one-time instruction" link | Clear, expandable |
| **Placement** | Directly above text input | Contextual, obvious relationship |
| **Persistence** | Auto-clear after one generation | True "ephemeral" behavior |
| **Label** | "One-time instruction (optional)" | Crystal clear purpose |

**User Flow:**
1. Click "+ Add one-time instruction"
2. Type instruction: "Use more dialogue, less narration"
3. Send message with instruction
4. Instruction auto-applies, then clears
5. Input collapses back to hidden state

---

### 3. Context Injection Method ✅ DECIDED

**Research: How SillyTavern Does It**

SillyTavern uses a sophisticated **extension prompt system** with depth-based injection:

| Component | Injection Method | Position |
|-----------|------------------|----------|
| **Author's Note** | `setExtensionPrompt('2_floating_prompt', ...)` | `IN_PROMPT` (after story) or `IN_CHAT` (at depth) |
| **World Info** | `setExtensionPrompt()` per entry | Multiple positions (before/after chat, at depth) |
| **Memory** | `setExtensionPrompt('1_memory', ...)` | Configurable position/depth |
| **Vector Search** | `setExtensionPrompt('3_vectors', ...)` | Configurable position/depth |

**Key Mechanisms:**

1. **Position Types:**
   - `IN_PROMPT` (0) - After story string, before chat
   - `IN_CHAT` (1) - Injected into chat history at specific depth
   - `BEFORE_PROMPT` (2) - Before story string

2. **Depth-Based Injection:**
   - Depth 0 = last message (most recent)
   - Depth 1 = second-to-last message
   - Higher depths = older messages
   - Used for "floating" instructions that stay close to current context

3. **Role Assignment:**
   - `SYSTEM` - Narrator, instructions, meta-text
   - `USER` - User speech, thoughts
   - `ASSISTANT` - Character speech

4. **Ephemeral Cleanup:**
   - Temporary injections deleted after generation
   - Function: `flushWIInjections()` removes depth-based prompts

---

**Recommended Approach: Hybrid (SillyTavern-Inspired)**

We'll adapt SillyTavern's approach to llama-monitor's simpler architecture:

```javascript
// In chat-transport.js, buildMessages()

const messages = [];

// 1. System prompt (persona)
if (systemPrompt) {
    messages.push({ role: 'system', content: systemPrompt });
}

// 2. Context Notes (persistent, structured like Author's Note)
if (tab.context_notes?.length) {
    // Group by section
    const notesBySection = groupBySection(tab.context_notes);
    const notesText = Object.entries(notesBySection)
        .map(([section, notes]) => 
            `### ${section.toUpperCase()} NOTES ###\n${notes.join('\n')}`
        )
        .join('\n\n');
    
    messages.push({
        role: 'system',
        content: notesText,
        meta: { type: 'context_notes', persistent: true }
    });
}

// 3. Quick Guide (ephemeral, depth-based like World Info)
if (tab.quick_guide) {
    messages.push({
        role: 'system',
        content: `### USER INSTRUCTION ###\n${tab.quick_guide}`,
        meta: { type: 'quick_guide', ephemeral: true, depth: 0 }
    });
}

// 4. Conversation history
messages.push(...tab.messages.map(m => ({ role: m.role, content: m.content })));

// 5. Clear ephemeral after generation
function clearEphemeralInstructions() {
    tab.quick_guide = '';
}
```

**Rationale:**
- ✅ Follows SillyTavern's proven pattern
- ✅ Clear separation of concerns (notes vs quick guide)
- ✅ Metadata allows filtering in UI (`meta.type`)
- ✅ No backend changes needed (all `role: 'system'`)
- ✅ Structured format helps model understand context

**Trade-offs:**
- More system messages = more tokens
- But structured, labeled content is more effective than buried in persona

---

### 4. State Persistence ✅ DECIDED

**Selected: Persist to Chat Tab (with Future Migration Path)**

We're storing guided generation state in the chat tab structure, designed to migrate later to a new centralized system.

**Current Implementation:**

```rust
// src/web/api.rs - ChatTab struct additions
pub struct ChatTab {
    // ... existing fields ...
    
    // Guided generation state (Phase 1)
    #[serde(default)]
    pub context_notes: Vec<ContextNote>,
    #[serde(default)]
    pub sidebar_width: u32,  // Remembered width in pixels (default: 280)
    
    // Note: quick_guide is ephemeral, NOT persisted
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextNote {
    pub section: String,  // "Character", "Setting", "Plot", "Tone", or custom
    pub content: String,
    pub created_at: u64,  // Timestamp for sorting
}
```

```javascript
// static/js/features/chat-state.js - newChatTab()
export function newChatTab(name = 'New Chat') {
    return {
        // ... existing fields ...
        context_notes: [],
        sidebar_width: 280,  // Default width
        // quick_guide NOT included (ephemeral, stored in memory only)
    };
}
```

**Migration Path to New System:**

When the new centralized state management doc is implemented, we'll:

1. Add migration function:
```javascript
// Migrate old tab-based notes to new centralized system
function migrateGuidedStateToCentralized() {
    const tabs = chat.tabs;
    const centralizedState = {};
    
    tabs.forEach(tab => {
        if (tab.context_notes?.length) {
            centralizedState[tab.id] = {
                notes: tab.context_notes,
                sidebar_width: tab.sidebar_width,
            };
        }
    });
    
    // Save to new location, clear from tabs
    saveCentralizedGuidedState(centralizedState);
    return centralizedState;
}
```

2. Add version check:
```javascript
// Check if migration needed
const guidedStateVersion = localStorage.getItem('guided_state_version') || '0';
if (guidedStateVersion === '0') {
    migrateGuidedStateToCentralized();
    localStorage.setItem('guided_state_version', '1');
}
```

**Rationale:**
- ✅ Simple implementation now
- ✅ Survives page refresh
- ✅ Easy to migrate later (just move data, update read/write paths)
- ✅ No breaking changes to existing tabs (defaults to empty array)

---

### 5. Suggestion Generation ✅ DECIDED

**Selected: Default + Editable (Per-Category)**

We're providing sensible defaults while allowing power users to customize.

**Implementation:**

```javascript
// static/js/features/chat-suggestions.js - Default prompts per category
const DEFAULT_PROMPTS = {
    'general': `You are a creative brainstorming partner. Based on the conversation below, 
suggest {count} varied, actionable next steps the user could take.

Format as a numbered list. Prioritize variety: dialogue, action, investigation, 
social, creative approaches.

[conversation context]`,

    'plot-twist': `You are a plot twist specialist. Based on the conversation below,
suggest {count} unexpected, surprising events that could happen next.

Format as a numbered list. Prioritize: betrayals, revelations, power reversals,
unexpected arrivals, hidden truths.

[conversation context]`,

    'new-character': `You are a character introduction specialist. Based on the conversation 
below, suggest {count} new characters that could enter the story.

Format as: [Character Name]: [Brief description and how they connect to current story]

[conversation context]`,
};

// Load user overrides from settings
function getSuggestionPrompt(category) {
    const settings = loadUISettings();
    const customPrompts = settings.suggestion_prompts || {};
    return customPrompts[category] || DEFAULT_PROMPTS[category] || DEFAULT_PROMPTS.general;
}
```

```html
<!-- static/index.html - Settings panel additions -->
<details class="modal-section">
    <summary>Suggestion Prompts <span class="modal-help">(per-category customization)</span></summary>
    
    <label class="modal-field">
        General Suggestions Prompt
        <textarea id="suggestion-prompt-general" rows="6" 
            placeholder="Default general suggestion prompt..."></textarea>
    </label>
    
    <label class="modal-field">
        Plot Twist Prompt
        <textarea id="suggestion-prompt-twist" rows="6" 
            placeholder="Default plot twist prompt..."></textarea>
    </label>
    
    <label class="modal-field">
        New Character Prompt
        <textarea id="suggestion-prompt-character" rows="6" 
            placeholder="Default new character prompt..."></textarea>
    </label>
    
    <div style="display:flex; gap:6px; margin-top:6px;">
        <button type="button" class="btn-sm btn-preset" id="reset-suggestion-prompts">
            Reset to Defaults
        </button>
    </div>
</details>
```

**Rationale:**
- ✅ Good defaults work for 90% of users
- ✅ Power users can optimize prompts
- ✅ Per-category customization (different prompts work better for different types)
- ✅ Reset button for recovery

---

### 6. Backend API ✅ DECIDED

**Selected: New `/api/chat/suggestions` Endpoint**

Clean architecture with dedicated endpoint for suggestions.

**Implementation:**

```rust
// src/web/api.rs

#[derive(Debug, Deserialize)]
pub struct SuggestionRequest {
    pub tab_id: String,
    pub category: String,  // "general", "plot-twist", "new-character"
    pub count: Option<u32>,  // Number of suggestions (default: 5)
    pub context_depth: Option<u32>,  // Messages to include (default: 10)
    pub prompt: Option<String>,  // Custom prompt (optional, overrides default)
}

#[derive(Debug, Serialize)]
pub struct SuggestionResponse {
    pub suggestions: Vec<String>,
    pub category: String,
    pub count: u32,
}

fn api_chat_suggestions(state: AppState) -> impl Filter<...> {
    warp::path!("api" / "chat" / "suggestions")
        .and(warp::post())
        .and(warp::body::json::<SuggestionRequest>())
        .and(with_state(state))
        .and_then(handle_suggestions)
}

async fn handle_suggestions(
    req: SuggestionRequest,
    state: AppState,
) -> Result<Json<SuggestionResponse>, Rejection> {
    // 1. Fetch tab
    let tab = get_tab(&state, &req.tab_id)?;
    
    // 2. Get last N messages
    let depth = req.context_depth.unwrap_or(10) as usize;
    let messages = get_last_messages(&tab.messages, depth);
    
    // 3. Get prompt (custom or default)
    let prompt = req.prompt.unwrap_or_else(|| get_default_prompt(&req.category));
    
    // 4. Build suggestion request
    let suggestion_messages = build_suggestion_messages(&messages, &prompt, &req.category);
    
    // 5. Call llama.cpp (non-streaming, single response)
    let response = call_llama_completion(&suggestion_messages, &state).await?;
    
    // 6. Parse suggestions from response
    let suggestions = parse_suggestions(&response.text);
    
    Ok(Json(SuggestionResponse {
        suggestions,
        category: req.category,
        count: suggestions.len() as u32,
    }))
}

fn parse_suggestions(text: &str) -> Vec<String> {
    // Handle multiple formats:
    // 1. Numbered list: "1. Option 1\n2. Option 2"
    // 2. Bullet list: "- Option 1\n- Option 2"
    // 3. Pathweaver format: "[emoji] Title\nDescription\n---"
    
    // Try numbered list first
    let numbered = regex::Regex::new(r"^\d+\.\s+(.+)$").ok();
    if let Some(re) = numbered {
        let suggestions: Vec<String> = text
            .lines()
            .filter_map(|line| re.captures(line))
            .filter_map(|caps| caps.get(1))
            .map(|m| m.as_str().trim().to_string())
            .collect();
        if !suggestions.is_empty() {
            return suggestions;
        }
    }
    
    // Try bullet list
    let bullets = regex::Regex::new(r"^[-*]\s+(.+)$").ok();
    if let Some(re) = bullets {
        let suggestions: Vec<String> = text
            .lines()
            .filter_map(|line| re.captures(line))
            .filter_map(|caps| caps.get(1))
            .map(|m| m.as_str().trim().to_string())
            .collect();
        if !suggestions.is_empty() {
            return suggestions;
        }
    }
    
    // Fallback: split by newlines, filter empty
    text.lines()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty() && s.len() > 2)
        .collect()
}
```

**Rationale:**
- ✅ Clean separation of concerns
- ✅ Backend controls context depth (token management)
- ✅ Easy to add features (caching, rate limiting, different models)
- ✅ Proper error handling
- ✅ Parsing logic centralized

---

## Technical Implementation Plan

### Phase 1: Context Notes (Sidebar)

1. **Backend** (`src/web/api.rs`)
   - Add `context_notes: Vec<ContextNote>` field to `ChatTab` struct
   - Add `ContextNote` struct: `{ section: String, content: String }`
   - Ensure serialization with `#[serde(default)]`

2. **Frontend State** (`static/js/features/chat-state.js`)
   - Add `context_notes: [{ section: 'Character', content: '...' }]` to `newChatTab()`
   - Add `sidebar_width: 280` to `newChatTab()` (remembered width)

3. **HTML** (`static/index.html`)
   - Add right sidebar container to chat layout (sibling to `.chat-messages`)
   - Add 📋 toggle button to chat header (near Settings, Persona buttons)
   - Add sidebar structure with sections (Character, Setting, Plot, Tone)
   - Add placeholder text, add note buttons, section management

4. **CSS** (`static/css/chat.css`)
   - Add `.chat-sidebar` styles (hidden by default, slide-in animation)
   - Add `.chat-sidebar-resize-handle` (draggable right edge)
   - Add `.chat-note-section` styles (headers, note lists)
   - Add `.chat-note-item` styles (editable content, × button)
   - Add resize cursor, transition animations

5. **JavaScript** (`static/js/features/chat-notes.js` - NEW)
   - `toggleNotesSidebar()` - open/close with animation
   - `resizeNotesSidebar(width)` - handle drag, save width
   - `addNote(section, content)` - add new note to section
   - `removeNote(section, index)` - remove note
   - `editNote(section, index, content)` - inline edit
   - `addSection(name)` - create new custom section
   - `loadNotesFromTab(tab)` - restore on tab switch

6. **Transport** (`static/js/features/chat-transport.js`)
   - Add context note injection before every generation
   - Format: `### CHARACTER NOTES ###\n{notes}\n### SETTING NOTES ###\n{notes}`
   - Inject as separate system messages (after persona, before conversation)

---

### Phase 2: Suggestions (Dropdown)

7. **Backend** (`src/web/api.rs`)
   - Add `SuggestionRequest` struct: `{ tab_id, category, count, context_depth }`
   - Add `api_chat_suggestions()` endpoint
   - Add to `api_routes()`
   - Add parsing logic (numbered lists, bullets, `---` separators)

8. **Prompts** (`static/prompts/` - NEW DIRECTORY)
   - `what-next.md` - General action suggestions
   - `plot-twist.md` - Unexpected events
   - `new-character.md` - Character introductions
   - Load prompts dynamically based on category

9. **HTML** (`static/index.html`)
   - Add "💡 What's Next?" button above `.chat-input-row`
   - Add dropdown panel structure (hidden by default)
   - Add category tabs, suggestions container, regenerate button

10. **CSS** (`static/css/chat.css`)
    - Add `.chat-suggestions-dropdown` styles (like `.chat-persona-menu`)
    - Add `.chat-suggestion-category-tabs` styles
    - Add `.chat-suggestion-item` styles (clickable, "Use" button)
    - Add loading spinner, fade-in animations

11. **JavaScript** (`static/js/features/chat-suggestions.js` - NEW)
    - `toggleSuggestionsDropdown()` - show/hide
    - `fetchSuggestions(category)` - API call with loading state
    - `renderSuggestions(suggestions)` - parse and display
    - `useSuggestion(text)` - populate input, keep dropdown open
    - `regenerateSuggestions(category)` - fetch new ones
    - `switchCategory(category)` - change active tab

12. **Transport** (`static/js/features/chat-transport.js`)
    - Add `fetchSuggestions(category, options)` function
    - Call `/api/chat/suggestions` endpoint
    - Parse response into array

---

### Phase 3: Quick Guide (Inline)

13. **Frontend State** (`static/js/features/chat-state.js`)
    - Add `quick_guide: ''` to `newChatTab()` (ephemeral, not persisted)

14. **HTML** (`static/index.html`)
    - Add collapsible container above `.chat-input-row`
    - Add "+ Add one-time instruction" link (visible when collapsed)
    - Add textarea with label "One-time instruction (optional)" (visible when expanded)

15. **CSS** (`static/css/chat.css`)
    - Add `.chat-quick-guide` styles (collapsible, subtle)
    - Add `.chat-quick-guide-collapsed` styles (just link)
    - Add `.chat-quick-guide-expanded` styles (textarea)
    - Add transition animations

16. **JavaScript** (`static/js/features/chat-quick-guide.js` - NEW)
    - `toggleQuickGuide()` - expand/collapse
    - `setQuickGuide(instruction)` - set instruction text
    - `clearQuickGuide()` - empty and collapse

17. **Transport** (`static/js/features/chat-transport.js`)
    - Add quick guide injection (ephemeral system message)
    - Clear `tab.quick_guide` after generation completes

---

### Phase 4: Settings & Polish

18. **Settings** (`static/index.html` + `static/js/features/chat-params.js`)
    - Add "Suggestions Prompt" textarea in Advanced panel (per-category)
    - Add "Default Suggestion Count" slider (3-8)
    - Add "Context Depth" slider (5-20 messages)
    - Add "Default Sidebar Width" slider (200-400px)

19. **Persistence**
    - Ensure `context_notes`, `sidebar_width` saved/loaded with chat tabs
    - Add migration logic for existing tabs (empty notes = [])
    - Test page refresh survival

20. **Loading States**
    - Add spinner/skeleton while fetching suggestions
    - Add "Generating suggestions..." message
    - Debounce category switches (prevent duplicate requests)
    - Add "Saving..." indicator when notes change

21. **Edge Cases**
    - Handle empty notes sections (show placeholders)
    - Handle suggestion fetch failures (show error, retry button)
    - Handle sidebar resize boundaries (min 200px, max 500px)
    - Clear quick guide on tab switch (ephemeral)

---

### Phase 5: Advanced Features (Optional)

22. **"Regenerate with Fix"** (`static/js/features/chat-fix.js` - NEW)
    - Add "Fix Last Response" button in chat header
    - Modal with instruction input
    - Call variant navigation with instruction prepended
    - Store correction in message metadata

23. **Custom Suggestion Categories**
    - Allow users to define custom genre categories
    - Store in `ui-settings.json`
    - Add to category tab bar

24. **Suggestion History**
    - Track used suggestions per chat
    - Show "Recently Used" section in dropdown
    - Quick reuse of past suggestions

---

## Implementation Checklist

### Phase 1: Context Notes (Sidebar) - 2-3 Days

#### Backend
- [ ] Add `ContextNote` struct to `src/web/api.rs`
- [ ] Add `context_notes: Vec<ContextNote>` field to `ChatTab`
- [ ] Add `sidebar_width: u32` field to `ChatTab`
- [ ] Ensure `#[serde(default)]` on new fields
- [ ] Test serialization/deserialization
- [ ] Add migration for existing tabs (empty array default)

#### Frontend State
- [ ] Update `newChatTab()` in `chat-state.js`
- [ ] Add `context_notes: []` to return object
- [ ] Add `sidebar_width: 280` to return object
- [ ] Test tab creation with new fields

#### HTML Structure
- [ ] Add right sidebar container to chat layout
- [ ] Add 📋 toggle button to chat header
- [ ] Add sidebar sections (Character, Setting, Plot, Tone)
- [ ] Add placeholder text per section
- [ ] Add "+ Add Note" buttons
- [ ] Add "+ Add Section" button
- [ ] Add resize handle to right edge

#### CSS
- [ ] Add `.chat-sidebar` (hidden by default)
- [ ] Add `.chat-sidebar.open` (slide-in animation)
- [ ] Add `.chat-sidebar-resize-handle`
- [ ] Add `.chat-note-section` styles
- [ ] Add `.chat-note-item` styles
- [ ] Add `.chat-note-item.editing` styles
- [ ] Add placeholder text styles
- [ ] Add transition animations

#### JavaScript - `chat-notes.js`
- [ ] `toggleNotesSidebar()` - open/close with animation
- [ ] `resizeNotesSidebar(width)` - handle drag
- [ ] `saveSidebarWidth(width)` - persist to tab
- [ ] `renderNotesSidebar(tab)` - render all sections
- [ ] `addNote(section, content)` - add new note
- [ ] `removeNote(section, index)` - remove note
- [ ] `editNote(section, index)` - inline edit mode
- [ ] `saveNoteEdit(section, index, content)` - save edit
- [ ] `cancelNoteEdit(section, index)` - cancel edit
- [ ] `addSection(name)` - create custom section
- [ ] `removeSection(name)` - delete section
- [ ] `loadNotesFromTab(tab)` - restore on tab switch

#### Transport Integration
- [ ] Add note injection to `buildMessages()` in `chat-transport.js`
- [ ] Group notes by section
- [ ] Format with headers: `### SECTION NOTES ###`
- [ ] Test injection appears in API request

#### Testing
- [ ] Create notes in all sections
- [ ] Edit existing notes
- [ ] Remove notes
- [ ] Add custom section
- [ ] Remove custom section
- [ ] Resize sidebar
- [ ] Close and reopen (width remembered)
- [ ] Switch tabs (notes persist)
- [ ] Page refresh (notes survive)
- [ ] Send message (notes injected)

---

### Phase 2: Suggestions (Dropdown) - 2-3 Days

#### Backend
- [ ] Add `SuggestionRequest` struct
- [ ] Add `SuggestionResponse` struct
- [ ] Add `api_chat_suggestions()` endpoint
- [ ] Add to `api_routes()`
- [ ] Implement `handle_suggestions()` function
- [ ] Implement `parse_suggestions()` with 3 strategies
- [ ] Test endpoint with curl/postman

#### Prompts
- [ ] Create `static/prompts/` directory
- [ ] Add `general.md` prompt template
- [ ] Add `plot-twist.md` prompt template
- [ ] Add `new-character.md` prompt template
- [ ] Test prompts with different models

#### HTML Structure
- [ ] Add "💡 What's Next?" button above `.chat-input-row`
- [ ] Add dropdown panel container (hidden by default)
- [ ] Add category tabs (General, Plot Twist, New Character)
- [ ] Add suggestions container
- [ ] Add regenerate button (🔄)
- [ ] Add "Generate new suggestions" link at bottom

#### CSS
- [ ] Add `.chat-suggestions-dropdown` (like `.chat-persona-menu`)
- [ ] Add `.chat-suggestion-category-tabs`
- [ ] Add `.chat-suggestion-item` (clickable card)
- [ ] Add `.chat-suggestion-item .use-btn`
- [ ] Add loading spinner styles
- [ ] Add fade-in animation

#### JavaScript - `chat-suggestions.js`
- [ ] `toggleSuggestionsDropdown()` - show/hide
- [ ] `fetchSuggestions(category)` - API call with loading state
- [ ] `renderSuggestions(suggestions, category)` - parse and display
- [ ] `useSuggestion(text)` - populate input
- [ ] `regenerateSuggestions(category)` - fetch new ones
- [ ] `switchCategory(category)` - change active tab
- [ ] `closeSuggestionsDropdown()` - hide panel
- [ ] `getSuggestionPrompt(category)` - load prompt (default or custom)

#### Transport Integration
- [ ] Add `fetchSuggestions(category, options)` to `chat-transport.js`
- [ ] Call `/api/chat/suggestions` endpoint
- [ ] Handle errors (show message, retry button)
- [ ] Add debouncing (prevent duplicate requests)

#### Settings Integration
- [ ] Add prompt textareas to settings panel
- [ ] Add "Reset to Defaults" button
- [ ] Add save/load logic
- [ ] Add "Suggestion Count" slider (3-8)
- [ ] Add "Context Depth" slider (5-20)

#### Testing
- [ ] Click button → dropdown opens
- [ ] Switch categories
- [ ] Click "Use" → input populated
- [ ] Click 🔄 → new suggestions
- [ ] Click outside → dropdown closes
- [ ] Custom prompt works
- [ ] Error handling (network failure)
- [ ] Loading state shows
- [ ] Settings persist

---

### Phase 3: Quick Guide (Inline) - 1 Day

#### Frontend State
- [ ] Add `quick_guide: ''` to in-memory state (NOT persisted)
- [ ] Add to active tab tracking

#### HTML Structure
- [ ] Add collapsible container above `.chat-input-row`
- [ ] Add "+ Add one-time instruction" link (collapsed state)
- [ ] Add textarea with label (expanded state)
- [ ] Add "Apply" button
- [ ] Add "Clear" button

#### CSS
- [ ] Add `.chat-quick-guide` container
- [ ] Add `.chat-quick-guide.collapsed` (just link)
- [ ] Add `.chat-quick-guide.expanded` (textarea)
- [ ] Add subtle styling (don't compete with main input)
- [ ] Add transition animations

#### JavaScript - `chat-quick-guide.js`
- [ ] `toggleQuickGuide()` - expand/collapse
- [ ] `setQuickGuide(instruction)` - set text
- [ ] `clearQuickGuide()` - empty and collapse
- [ ] `applyQuickGuide()` - mark for injection

#### Transport Integration
- [ ] Add quick guide injection to `buildMessages()`
- [ ] Format: `### USER INSTRUCTION ###\n{instruction}`
- [ ] Add `clearEphemeralInstructions()` after generation
- [ ] Clear `tab.quick_guide` on tab switch

#### Testing
- [ ] Click link → expands
- [ ] Type instruction
- [ ] Send message → instruction injected
- [ ] Instruction clears after generation
- [ ] Switch tabs → instruction cleared
- [ ] Click "Clear" → emptied

---

### Phase 4: Settings & Polish - 1-2 Days

#### Settings UI
- [ ] Add "Suggestions Prompt" section to Advanced panel
- [ ] Add 3 textareas (General, Plot Twist, New Character)
- [ ] Add "Reset to Defaults" button
- [ ] Add "Suggestion Count" slider (3-8, default 5)
- [ ] Add "Context Depth" slider (5-20, default 10)
- [ ] Add "Default Sidebar Width" slider (200-400px, default 280)
- [ ] Add save/load logic

#### Persistence
- [ ] Ensure `context_notes` saved with tabs
- [ ] Ensure `sidebar_width` saved with tabs
- [ ] Add migration logic for existing tabs
- [ ] Test page refresh survival
- [ ] Test tab persistence

#### Loading States
- [ ] Add spinner while fetching suggestions
- [ ] Add "Generating suggestions..." message
- [ ] Add skeleton cards (optional)
- [ ] Debounce category switches
- [ ] Add "Saving..." indicator when notes change

#### Edge Cases
- [ ] Handle empty notes sections (show placeholders)
- [ ] Handle suggestion fetch failures (show error, retry)
- [ ] Handle sidebar resize boundaries (min 200px, max 500px)
- [ ] Clear quick guide on tab switch
- [ ] Handle very long notes (truncate with ellipsis)
- [ ] Handle special characters in notes (escape properly)

#### Accessibility
- [ ] Add ARIA labels to all buttons
- [ ] Add keyboard navigation (Tab, Enter, Escape)
- [ ] Add focus management
- [ ] Add screen reader announcements

---

### Phase 5: Advanced Features (Optional) - 1-2 Days

#### "Fix Last Response"
- [ ] Add "Fix Last Response" button to chat header
- [ ] Add modal with instruction input
- [ ] Add "Regenerate" button
- [ ] Call variant navigation with instruction prepended
- [ ] Store correction in message metadata
- [ ] Test with various corrections

#### Custom Categories
- [ ] Add "Manage Categories" button
- [ ] Add modal with category list
- [ ] Add "Add Category" form
- [ ] Add prompt editor per category
- [ ] Store in `ui-settings.json`
- [ ] Add to category tab bar

#### Suggestion History
- [ ] Track used suggestions per chat
- [ ] Add "Recently Used" section to dropdown
- [ ] Add quick reuse buttons
- [ ] Limit to last 10 suggestions
- [ ] Clear on tab close

---

## API Contracts

### POST /api/chat/suggestions

**Request:**
```json
{
  "tab_id": "uuid-here",
  "category": "general",
  "count": 5,
  "context_depth": 10,
  "prompt": "custom prompt..."  // optional
}
```

**Response (Success):**
```json
{
  "suggestions": [
    "Investigate the mysterious package",
    "Confront the waiter about the bill",
    "Suggest an impromptu road trip"
  ],
  "category": "general",
  "count": 3
}
```

**Response (Error):**
```json
{
  "error": "Tab not found",
  "code": 404
}
```

---

## File Structure

**Actual Implementation:**

```
src/
├── web/
│   └── api.rs                    # MODIFIED - ContextNote, SuggestionRequest/Response, endpoint, parse_suggestions()
├── state.rs                      # MODIFIED - UiSettings fields
├── lib.rs                        # MODIFIED - Added recursion limit
└── build.rs                      # MODIFIED - Refactored route generation
static/
├── js/
│   ├── features/
│   │   ├── chat-notes.js         # NEW - Sidebar notes management
│   │   ├── chat-suggestions.js   # NEW - Dropdown + history + Pathweaver integration
│   │   ├── chat-quick-guide.js   # NEW - Inline instruction
│   │   ├── chat-fix-last.js      # NEW - Correction regeneration
│   │   ├── chat-state.js         # MODIFIED - Add new fields to newChatTab()
│   │   ├── chat-transport.js     # MODIFIED - Add injection logic
│   │   ├── chat-templates.js     # MODIFIED - Explicit mode integration
│   │   └── settings.js           # MODIFIED - 17 prompt settings
│   └── bootstrap.js              # MODIFIED - Add initialization calls
├── css/
│   └── chat-guided-generation.css # NEW - All styles (574 lines)
├── prompts/                      # NEW DIRECTORY - 17 Pathweaver prompts
│   ├── action.md
│   ├── character.md
│   ├── comedy.md
│   ├── context.md
│   ├── director.md
│   ├── explicit.md
│   ├── fantasy.md
│   ├── general.md                # Pathweaver's template.md + context.md hybrid
│   ├── horror.md
│   ├── mystery.md
│   ├── new-character.md          # Pathweaver's character.md
│   ├── noir.md
│   ├── plot-twist.md             # Pathweaver's twist.md
│   ├── romance.md
│   ├── sci-fi.md
│   ├── template.md
│   ├── thriller.md
│   └── character.md              # Backup
└── index.html                    # MODIFIED - Add HTML structure (sidebar, dropdown, modals, settings)
```

---

## Rollback Plan

If issues arise:

1. **Quick rollback:** Disable features via CSS
```css
.chat-sidebar { display: none !important; }
.chat-suggestions-dropdown { display: none !important; }
.chat-quick-guide { display: none !important; }
```

2. **Data migration:** Notes are in chat tabs, can be cleared via:
```javascript
chat.tabs.forEach(tab => {
    tab.context_notes = [];
    tab.sidebar_width = 280;
});
scheduleChatPersist();
```

3. **Settings reset:** Clear user overrides
```javascript
const settings = loadUISettings();
delete settings.suggestion_prompts;
delete settings.suggestion_count;
delete settings.context_depth;
saveUISettings(settings);
```

---

## Scope Summary

### What Was Built

With **Separated Features (4 Different UI Patterns)**, we built:

1. **Context Notes Sidebar** (Workspace) ✅
   - Right sidebar with section-based notes (Character, Setting, Plot, etc.)
   - Add/edit/remove notes per section
   - Draggable/resizable (200-500px), remembers last width
   - Default: collapsed, toggle with 📋 button
   - Auto-inject into every generation
   - **Implemented:** 264 lines of JavaScript

2. **Suggestions Dropdown** (Tool) ✅
   - "💡 What's Next?" button above text input
   - **17 categories:** 9 genres + 8 core modes (General, Plot Twist, New Character, Director, Explicit, etc.)
   - Pathweaver format: `[EMOJI] TITLE\nDESCRIPTION` with `---` separators
   - Clickable → populate input
   - Regenerate button with retry logic
   - Space-efficient dropdown (like persona, export)
   - **Implemented:** 395 lines of JavaScript + 17 prompt files

3. **Quick Guide Inline** (Input) ✅
   - "🧭 Quick Guide" button above text input
   - Expands to textarea when clicked
   - Applied to next generation only, auto-clears
   - Default: hidden/collapsed
   - **Implemented:** 179 lines of JavaScript

4. **Fix Last Response** (Advanced) ✅
   - "Fix" button in chat header (shows only when last message is assistant)
   - Modal with correction instruction
   - Re-generates last AI message with fix prepended
   - **Implemented:** 181 lines of JavaScript

5. **Suggestion History** (Advanced) ✅
   - Tracks last 10 used suggestions per tab
   - "Recently Used" section in dropdown
   - Quick reuse buttons
   - Cleared on tab close
   - **Implemented:** Integrated into chat-suggestions.js

6. **Custom Categories** (Advanced) ✅
   - "Manage Categories" button
   - Add/remove/edit unlimited custom categories
   - Custom prompts per category
   - Stored in localStorage
   - **Implemented:** Integrated into chat-suggestions.js

7. **Settings UI** (Polish) ✅
   - Enable/disable toggles for all 3 features
   - Default sidebar width slider (200-500px)
   - Suggestion count slider (3-8)
   - Context depth slider (5-20)
   - 17 editable prompt textareas
   - "Reset to Defaults" button
   - **Implemented:** 310 lines of settings.js modifications

8. **Pathweaver Integration** (Complete) ✅
   - 17 exact Pathweaver prompts (proven to work)
   - New parsing format (`[EMOJI] TITLE\nDESCRIPTION`)
   - Director Mode (custom prompt input)
   - Explicit category with soft dependency on Explicit Mode toggle
   - Removed `{count}` variable (model decides quantity naturally)
   - **Implemented:** 17 prompt files + 146 lines of parsing logic

### Actual Effort

| Phase | Feature | Estimated | Actual |
|-------|---------|-----------|--------|
| Phase 1-2 | Context Notes + Suggestions (Core) | 4-6 days | 0.5 days |
| Phase 3 | Quick Guide (Inline) | 1 day | 0.25 days |
| Phase 4 | Settings & Polish | 1-2 days | 0.5 days |
| Phase 5 | Fix Last Response + Advanced | 1-2 days | 0.5 days |
| Phase 6 | Pathweaver Integration | N/A | 0.75 days |
| **Total** | | **7-11 days** | **2.5 days** |

**Result:** Completed in ~25% of estimated time (2.5 days vs. 7-11 days)

### Risk Assessment

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| Sidebar takes too much space | Low | Medium | Default collapsed, user-controlled width |
| Suggestions feel repetitive | Medium | High | Multiple genre prompts, regenerate option |
| Context notes clutter context | Low | Medium | User-controlled, organized by section |
| 3 features = UI complexity | Medium | Medium | Clear separation, distinct interaction patterns |
| Performance issues (slow suggestions) | Low | Medium | Configurable context depth, caching |
| Feature creep | High | Medium | Stick to Phase 1-3 for MVP, defer Phase 4-5 |

---

## Getting Started

### For New AI Agents

1. **Read this document top-to-bottom** - All decisions are made, no ambiguity
2. **Review the Implementation Checklist** - Start with Phase 1, work sequentially
3. **Check the File Structure** - Know where everything goes
4. **Read the API Contracts** - Understand the backend interface
5. **Start with Phase 1** - Context Notes (Sidebar) is the foundation

### Implementation Order

```
Week 1:
  Day 1-2: Phase 1 - Context Notes (Sidebar)
  Day 3-4: Phase 2 - Suggestions (Dropdown)
  Day 5: Phase 3 - Quick Guide (Inline)

Week 2:
  Day 1-2: Phase 4 - Settings & Polish
  Day 3-4: Phase 5 - Advanced Features (optional)
  Day 5: Testing, bug fixes, documentation
```

### First Steps

1. **Create feature branch** (already done: `feature/chat-guided-generations`)
2. **Start with Phase 1, Task 1:** Add `ContextNote` struct to `src/web/api.rs`
3. **Run validation after each phase:** `cargo build`, `cargo test`, `npm run lint`
4. **Test incrementally:** Don't wait until end to test UI

### Common Pitfalls

- ❌ **Don't bundle features** - Keep them separate (sidebar ≠ dropdown ≠ inline)
- ❌ **Don't persist quick_guide** - It's ephemeral, clear after use
- ❌ **Don't forget migration** - Existing tabs need empty array defaults
- ❌ **Don't skip loading states** - Suggestions take 2-5 seconds
- ✅ **Do follow SillyTavern's injection pattern** - It's proven to work
- ✅ **Do test page refresh** - Notes must survive
- ✅ **Do add error handling** - Network failures happen

---

## Decision Log

| Date | Decision | Rationale |
|------|----------|-----------|
| 2026-05-08 | Full Guided Generation (4 features) | Building complete feature set avoids technical debt |
| 2026-05-08 | Separated UI (3 different patterns) | Each feature has different interaction pattern |
| 2026-05-08 | Context Notes: Right Sidebar | Workspace needs always-visible, resizable space |
| 2026-05-08 | Suggestions: Dropdown | Tool needs on-demand, space-efficient access |
| 2026-05-08 | Quick Guide: Inline Input | Rarely-used input should be collapsible |
| 2026-05-08 | Sidebar: Default collapsed, draggable | Don't clutter UI, but remember user preference |
| 2026-05-08 | Hybrid Context Injection (SillyTavern-inspired) | Proven pattern, structured metadata, no backend changes |
| 2026-05-08 | Persist to Chat Tab | Simple persistence, survives refresh |
| 2026-05-08 | Default + Editable Prompts | Best of both worlds |
| 2026-05-08 | New `/api/chat/suggestions` Endpoint | Clean architecture, easy to extend |

---

## References

- **Guided Generations:** https://github.com/Samueras/Guided-Generations
- **Roadway:** https://github.com/bmen25124/SillyTavern-Roadway
- **Pathweaver:** https://github.com/mattjaybe/SillyTavern-Pathweaver
- **SillyTavern Core:** https://github.com/SillyTavern/SillyTavern
  - `/public/script.js` - Core prompt assembly, `setExtensionPrompt`, `doChatInject`
  - `/public/scripts/authors-note.js` - Author's Note injection
  - `/public/scripts/world-info.js` - World Info activation and injection

**Research Notes:**
- Subagent analyses stored in tool output directories
- SillyTavern context injection research completed 2026-05-08

---

## Document History

| Version | Date | Author | Changes |
|---------|------|--------|---------|
| 0.1 | 2026-05-08 | opencode | Initial decision document |
| 0.2 | 2026-05-08 | opencode | Added separated UI approach |
| 0.3 | 2026-05-08 | opencode | Added SillyTavern injection research |
| 1.0 | 2026-05-08 | opencode | Full implementation spec, all decisions made |

---

## Summary

This document specifies a **complete, production-ready implementation** of guided generation features for llama-monitor. All major decisions have been made, all trade-offs considered, and all technical details specified.

### What We're Building

1. **Context Notes Sidebar** - Persistent workspace for story details (Character, Setting, Plot, Tone)
2. **Suggestions Dropdown** - AI-powered "What's Next?" with genre categories
3. **Quick Guide Inline** - Ephemeral one-time instructions

### Why This Approach

- ✅ **Separated features** = Clear mental models (unlike Guided Generations)
- ✅ **Default collapsed** = No UI clutter (unlike Pathweaver's always-visible toolbar)
- ✅ **SillyTavern-inspired injection** = Proven pattern, structured context
- ✅ **Migration-ready** = Easy to pivot to new state management later
- ✅ **Incremental phases** = Can ship Phase 1-2, then add more

### Success Criteria

- User can add context notes that persist across page refresh
- User can get AI suggestions when stuck
- User can provide one-time instructions
- All features work independently
- No breaking changes to existing functionality
- Clean, discoverable UI (no video tutorial needed)

### Total Effort

**7-11 days** for complete implementation, or **4-5 days** for MVP (Phases 1-3 only).

---

*Document created by opencode agent*  
*Last updated: 2026-05-08*  
*Ready for implementation: YES*
- Full extension code available for detailed review

---

*Document created by opencode agent*  
*Last updated: 2026-05-08*
