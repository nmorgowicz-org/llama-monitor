# Chat UI/UX Improvements — 2026-04-28

## Current State

The chat tab (`#page-chat`, class `.chat-page`) is a multi-panel layout stacked vertically: tab bar → header → optional panels → message thread → input row. All relevant CSS lives in `static/css/chat.css`.

**What is already implemented:**
- Multi-tab system: `#chat-tab-bar` rendered by `renderChatTabs()` in `app.js:6349`. Tabs have double-click rename, close buttons, per-tab message persistence via debounced `scheduleChatPersist()`.
- System prompt panel (`#chat-system-panel`): collapsible via `toggleSystemPromptPanel()`, with template select (`#chat-template-select`), template manager modal, and copy-settings-from-tab button.
- Model params panel (`#chat-params-panel`): basic params (temperature, top_p) always visible; advanced params (top_k, min_p, repeat_penalty, max_tokens) behind `.chat-params-advanced` toggle.
- Message rendering: `buildMessageElement()` (app.js:6411) uses `renderMd()` (app.js:6093), which calls `marked.parse()` if the `marked` CDN script is loaded. No syntax highlighting library is loaded.
- Streaming: SSE chunks update `msgEl.querySelector('.chat-msg-body').innerHTML = renderMd(msgContent)` on every chunk (app.js:6698).
- Inline thinking dots displayed while waiting for first token (app.js:6627 — overwrites the body with `.chat-thinking-inline` spans).
- Separate `.chat-typing` element (`#chat-typing`) exists in the HTML (index.html:783–788) but is **never shown or hidden by `setChatBusyUI()`** — it is dead HTML.
- Empty state: injected by `renderChatMessages()` (app.js:6369) when `tab.messages` has no non-system messages; renders `.chat-empty` with `.chat-empty-prompts` grid populated from a hardcoded `prompts` array.
- Scroll-to-bottom button: `.chat-scroll-bottom` / `#chat-scroll-bottom` (chat.css:293, index.html:2075). Visibility toggled by `initChatScrollButton()` scroll listener. `chatScroll()` always force-scrolls unconditionally (app.js:6324–6326) — it does not respect "user scrolled up" state.
- Send button: `#btn-send`, `.btn-chat-send` — disabled by `setChatBusyUI(busy)` (app.js:6313). Has a breathing animation while idle and a box-shadow on hover.
- Char count: `#chat-char-count` / `.chat-char-count` (chat.css:1254) — updated by `autoResizeChatInput()`. Positioned `absolute bottom:6px right:10px` inside `.chat-textarea-wrap`.
- Chat styles: `.chat-page[data-chat-style="..."]` CSS variable approach — 4 styles: rounded (default), compact, minimal, bubbly.
- Font size controls: `#chat-font-value`, adjust via `adjustChatFont()`.
- Copy message: per-bubble `.chat-action-btn` with `copyMessageContent()`.
- No syntax highlighting library is loaded. `marked` is the only external script (`https://cdn.jsdelivr.net/npm/marked/marked.min.js`).
- Mobile: one `@media (max-width: 600px)` block in chat.css (line 928) covers only `.chat-message max-width` and `.chat-messages` padding.

**Obvious gaps and rough edges:**
1. The `.chat-typing` element is wired up in HTML but never toggled — dead.
2. `chatScroll()` force-scrolls even when the user has scrolled up to read history.
3. No syntax highlighting on fenced code blocks.
4. No per-code-block copy button (only per-message copy exists).
5. The `pre` block inside `.chat-message-assistant .chat-msg-body` has no language label or copy affordance.
6. On mobile, the header (`chat-name-inputs`, font controls, explicit toggle, export) overflows and wraps badly; no responsive stacking.
7. `.chat-system-panel` and `.chat-params-panel` use `display:none` / `display:block` toggled by inline style. The CSS transitions on `max-height` and `opacity` only trigger when `style` contains the literal string `"block"` (selector `[style*="block"]`), making the collapse animation brittle and only working on open, not close.
8. `autoResizeChatInput()` resets `height` to `'auto'` first — on every keystroke this causes a brief layout thrash on slower machines.
9. Suggested prompts grid uses `repeat(auto-fit, minmax(180px, 1fr))` which collapses to one column on narrow viewports and still forces full-width cards that feel like they belong on desktop.
10. Token metadata in `.chat-msg-meta-model` uses terse symbols (`↓`, `↑`, `R`) with no tooltip explaining them.
11. The `chatScroll()` function is called on every SSE chunk (app.js:6702 inside the `while(true)` reader loop), causing a scroll jump on every chunk regardless of user position.

---

## Fix List (ordered quickest → largest)

---

### [S] Fix dead `.chat-typing` element — wire it to `setChatBusyUI`

**What:** The `#chat-typing` div (index.html:783) is never shown; the thinking indicator is only the inline dots injected into the assistant bubble body, which disappears the moment the first token arrives and leaves no "model is loading" state visible for slow first-token latency.

**Where:**
- `static/app.js`, function `setChatBusyUI` (line 6313)
- `static/index.html`, `#chat-typing` (line 783)

**How:** In `setChatBusyUI(busy)`, add:
```js
function setChatBusyUI(busy) {
    document.getElementById('btn-send').disabled = busy;
    const stopBtn = document.getElementById('btn-stop');
    if (stopBtn) stopBtn.style.display = busy ? 'flex' : 'none';
    const input = document.getElementById('chat-input');
    if (input) input.disabled = busy;
    // ADD THIS:
    const typing = document.getElementById('chat-typing');
    if (typing) typing.style.display = busy ? 'flex' : 'none';
}
```
Then remove the inline-dot injection from `sendChat()` at app.js ~line 6627:
```js
// REMOVE this line (the one that overwrites the body with inline dots):
msgEl.querySelector('.chat-msg-body').innerHTML = '<span class="chat-thinking-inline">...</span>';
```
The `.chat-typing` element already has `.chat-typing-avatar` + `.chat-typing-dots` with a polished bounce animation (chat.css:752–798). This shows below the messages thread and disappears when the first token starts streaming, at which point the cursor `▋` in the placeholder bubble takes over.

---

### [S] Fix `chatScroll()` force-scrolling when user has scrolled up

**What:** `chatScroll()` (app.js:6324) unconditionally sets `c.scrollTop = c.scrollHeight`, hijacking the user's scroll position mid-read on every SSE chunk.

**Where:** `static/app.js`, `chatScroll()` (line 6324), and every call site.

**How:** Replace `chatScroll()` with a smart version that only scrolls when the user is already near the bottom:
```js
function chatScroll(force = false) {
    const c = document.getElementById('chat-messages');
    if (!c) return;
    const distFromBottom = c.scrollHeight - c.scrollTop - c.clientHeight;
    if (force || distFromBottom < 80) {
        c.scrollTop = c.scrollHeight;
    }
}
```
Update the two call sites that must always scroll (new user message sent, tab switch):
- `renderChatMessages()` (line 6408): `chatScroll(true)`
- `appendAssistantPlaceholder()` (line 6501): `chatScroll(true)`

Leave the streaming call at line 6702 as `chatScroll()` (no `force`) so it respects user position.

---

### [S] Fix panel open/close animation — replace `display:none` toggle with class-based approach

**What:** `toggleSystemPromptPanel()` and `toggleModelParamsPanel()` toggle `element.style.display = 'block'/'none'`, which makes the CSS `max-height` transition (chat.css:1332–1345) only animate on open (because `[style*="block"]` selector fires on open), never on close — the panel snaps shut instantly.

**Where:**
- `static/css/chat.css` lines 1332–1345
- `static/app.js` — functions `toggleSystemPromptPanel` and `toggleModelParamsPanel` (search for these names)

**How:** Replace the CSS approach with a `.open` class:

In `chat.css`, remove the `[style*="block"]` selectors and replace with:
```css
.chat-system-panel,
.chat-params-panel {
  max-height: 0;
  overflow: hidden;
  opacity: 0;
  padding-top: 0;
  padding-bottom: 0;
  transition: max-height 0.3s cubic-bezier(0.16, 1, 0.3, 1),
              padding 0.3s cubic-bezier(0.16, 1, 0.3, 1),
              opacity 0.2s ease;
}
.chat-system-panel.open,
.chat-params-panel.open {
  max-height: 500px;
  opacity: 1;
  padding: var(--gap-sm) var(--gap-md);
}
```

Remove the existing `style="display:none;"` from both panels in `index.html` (lines ~691 and ~722).

In `app.js`, update the toggle functions to use `.classList.toggle('open')` and remove any `style.display` assignment on those panels.

---

### [S] Make char count show tokens, not characters, and add a warning color

**What:** `#chat-char-count` shows `"N chars"` but the model operates on tokens (roughly chars/4). The count is also always gray — no color change when approaching a large message size.

**Where:**
- `static/app.js`, `autoResizeChatInput()` (line 7274)
- `static/css/chat.css`, `.chat-char-count` (line 1254)

**How:** Replace the count display logic in `autoResizeChatInput()`:
```js
const countEl = document.getElementById('chat-char-count');
if (countEl) {
    const len = ta.value.length;
    const approxTokens = Math.round(len / 4);
    if (len === 0) {
        countEl.textContent = '';
        countEl.style.opacity = '0';
        countEl.style.color = '';
    } else {
        countEl.textContent = approxTokens >= 1000
            ? `~${(approxTokens / 1000).toFixed(1)}k tok`
            : `~${approxTokens} tok`;
        countEl.style.opacity = '1';
        // Warn at 800+ tokens, danger at 1500+
        countEl.style.color = approxTokens > 1500
            ? 'var(--color-error)'
            : approxTokens > 800
                ? 'var(--color-warning)'
                : '';
    }
}
```
Add a tooltip via title attribute in the HTML on `#chat-char-count`:
```html
<span class="chat-char-count" id="chat-char-count" title="Approximate token count (chars ÷ 4)"></span>
```

---

### [S] Add tooltips to token metadata symbols in `.chat-msg-meta-model`

**What:** The per-message footer shows terse symbols `↓Ntok · ↑Ntok · RNtok · N% ctx · model-name` with no explanation. `↓`, `↑`, and `R` are opaque to new users.

**Where:**
- `static/app.js`, `finalizeAssistantMessage()` (line ~6524) and `buildMessageElement()` (line ~6411)

**How:** Wrap the meta string in a `<span title="...">` or switch to individual labeled chips. Simplest: change the `parts.join(' · ')` to include an explanatory title on the containing element. In both `buildMessageElement` and `finalizeAssistantMessage`, where `metaModel.textContent = parts.join(' · ')` is set, instead:
```js
metaModel.title = '↓ = prompt tokens in · ↑ = tokens generated · R = running total · ctx = % of context window used';
metaModel.textContent = parts.join(' · ');
```

---

### [M] Textarea auto-resize: fix layout thrash and add smooth height transition

**What:** `autoResizeChatInput()` sets `ta.style.height = 'auto'` on every `oninput` event before measuring `scrollHeight`. On every keystroke this reflows the layout twice. Additionally there is no transition on height so growth/shrink jumps.

**Where:**
- `static/app.js`, `autoResizeChatInput()` (line 7274)
- `static/css/chat.css`, `.chat-input-row textarea` (line 816)

**How:** Add a CSS transition for height to chat.css:
```css
.chat-input-row textarea {
  /* existing styles ... */
  transition: border-color 0.2s, box-shadow 0.2s, height 0.1s ease;
}
```
And in `autoResizeChatInput()`, avoid the double-reflow:
```js
function autoResizeChatInput() {
    const ta = document.getElementById('chat-input');
    if (!ta) return;
    // Temporarily disable transition so the reset doesn't animate
    ta.style.transition = 'none';
    ta.style.height = 'auto';
    const newH = Math.min(ta.scrollHeight, 200);
    // Re-enable transition before setting final height
    requestAnimationFrame(() => {
        ta.style.transition = '';
        ta.style.height = newH + 'px';
    });
    // char/token count update (see fix above)
    const countEl = document.getElementById('chat-char-count');
    // ... token count logic ...
}
```
This keeps the grow/shrink smooth without janking the layout.

---

### [M] Add syntax highlighting to fenced code blocks in `renderMd()`

**What:** `renderMd()` (app.js:6093) calls `marked.parse(src)` which wraps fenced code blocks in `<pre><code class="language-X">` but there is no syntax highlighter loaded, so all code renders as plain text in a dark box.

**Where:**
- `static/index.html` — `<head>` section (line 25, after the `marked` script tag)
- `static/app.js`, `renderMd()` (line 6093) and the streaming chunk handler (line 6698)

**How:** Add highlight.js via CDN. In `index.html` after line 25:
```html
<script src="https://cdn.jsdelivr.net/npm/marked/marked.min.js"></script>
<link rel="stylesheet" href="https://cdn.jsdelivr.net/npm/highlight.js@11.9.0/styles/atom-one-dark.min.css">
<script src="https://cdn.jsdelivr.net/npm/highlight.js@11.9.0/highlight.min.js"></script>
```

Update the `marked` initialization block in `app.js` (line 6087):
```js
if (typeof marked !== 'undefined') {
    marked.setOptions({
        breaks: true,
        gfm: true,
        highlight: (typeof hljs !== 'undefined')
            ? (code, lang) => {
                const language = hljs.getLanguage(lang) ? lang : 'plaintext';
                return hljs.highlight(code, { language }).value;
              }
            : null,
    });
}
```

Note: `marked` v5+ removed the `highlight` option; if the CDN serves v5+, use a custom renderer instead:
```js
if (typeof marked !== 'undefined' && typeof hljs !== 'undefined') {
    const renderer = new marked.Renderer();
    renderer.code = (code, lang) => {
        const language = hljs.getLanguage(lang) ? lang : 'plaintext';
        const highlighted = hljs.highlight(code, { language }).value;
        return `<pre><code class="hljs language-${language}">${highlighted}</code></pre>`;
    };
    marked.setOptions({ breaks: true, gfm: true, renderer });
}
```

The `atom-one-dark` stylesheet uses its own background (`#282c34`) which is close to the app's `rgba(0,0,0,0.3)` on `pre` (chat.css:938). You may want to override it to stay on-brand:
```css
/* In chat.css, after the existing .chat-message-assistant .chat-msg-body pre rule */
.chat-message-assistant .chat-msg-body pre code.hljs {
  background: transparent;
  padding: 0;
}
```

**Important caveat:** During streaming, `renderMd(msgContent)` is called on every SSE chunk (app.js:6698). Running `hljs.highlight()` on every chunk is fine for small responses but will be noticeable on 4k+ token responses. Defer highlighting to `finalizeAssistantMessage()` only, and during streaming render the `pre` blocks without highlighting:
```js
// In the SSE loop, replace:
msgEl.querySelector('.chat-msg-body').innerHTML = renderMd(msgContent);
// With:
msgEl.querySelector('.chat-msg-body').innerHTML = renderMdStreaming(msgContent);
```
Add a `renderMdStreaming()` function that uses `marked.parse()` but without the custom renderer (no hljs). Then in `finalizeAssistantMessage()`, after `body.innerHTML = renderMd(content)`, add:
```js
if (typeof hljs !== 'undefined') {
    body.querySelectorAll('pre code').forEach(el => hljs.highlightElement(el));
}
```

---

### [M] Add per-code-block copy button

**What:** There is no way to copy a code block without selecting text manually. The per-message copy button copies all message text via `body.innerText`, which strips formatting.

**Where:**
- `static/app.js`, `renderMd()` custom renderer (new, see previous fix) or a post-processing step in `finalizeAssistantMessage()` (line ~6521)
- `static/css/chat.css` — add new CSS for `.chat-code-copy-btn`

**How:** In `finalizeAssistantMessage()`, after setting `body.innerHTML = renderMd(content)`, run a DOM decoration pass:
```js
body.querySelectorAll('pre').forEach(pre => {
    if (pre.querySelector('.chat-code-copy-btn')) return; // already decorated
    const btn = document.createElement('button');
    btn.className = 'chat-code-copy-btn';
    btn.title = 'Copy code';
    btn.innerHTML = '<svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><rect x="9" y="9" width="13" height="13" rx="2"/><path d="M5 15H4a2 2 0 01-2-2V4a2 2 0 012-2h9a2 2 0 012 2v1"/></svg>';
    btn.addEventListener('click', () => {
        const code = pre.querySelector('code')?.innerText ?? pre.innerText;
        navigator.clipboard.writeText(code).then(() => {
            btn.classList.add('copied');
            setTimeout(() => btn.classList.remove('copied'), 1500);
        });
    });
    pre.style.position = 'relative';
    pre.appendChild(btn);
});
```

Add to `chat.css`:
```css
.chat-code-copy-btn {
  position: absolute;
  top: 6px;
  right: 6px;
  width: 26px;
  height: 26px;
  display: flex;
  align-items: center;
  justify-content: center;
  border: 1px solid rgba(255, 255, 255, 0.1);
  border-radius: 6px;
  background: rgba(255, 255, 255, 0.04);
  color: var(--text-muted);
  cursor: pointer;
  opacity: 0;
  transition: opacity 0.15s, background 0.15s, color 0.15s;
  padding: 0;
}
.chat-message-assistant .chat-msg-body pre:hover .chat-code-copy-btn {
  opacity: 1;
}
.chat-code-copy-btn.copied {
  background: rgba(16, 185, 129, 0.15);
  color: #10b981;
  border-color: rgba(16, 185, 129, 0.3);
}
```

Also add a language label to fenced code blocks. In the custom renderer, wrap the `<pre>` in a `<div class="chat-code-block">` and emit a `<span class="chat-code-lang">` for the language:
```js
renderer.code = (code, lang) => {
    const language = hljs.getLanguage(lang) ? lang : 'plaintext';
    const highlighted = hljs.highlight(code, { language }).value;
    const langLabel = lang ? `<span class="chat-code-lang">${escapeHtml(lang)}</span>` : '';
    return `<div class="chat-code-block"><pre>${langLabel}<code class="hljs language-${language}">${highlighted}</code></pre></div>`;
};
```

Add to `chat.css`:
```css
.chat-code-block { position: relative; }
.chat-code-lang {
  position: absolute;
  top: 6px;
  left: 10px;
  font-size: 10px;
  font-weight: 600;
  color: var(--text-muted);
  opacity: 0.6;
  font-family: var(--font-mono);
  text-transform: lowercase;
  pointer-events: none;
}
```

---

### [M] Fix suggested prompts layout — stagger animation and improve hover feel

**What:** The `.chat-empty-prompts` grid (chat.css:433) uses `auto-fit, minmax(180px, 1fr)` which on wide viewports creates 3–4 cards per row but on 400–600px screens creates one awkwardly tall card per row. The cards also all appear at once — no stagger.

**Where:**
- `static/css/chat.css`, `.chat-empty-prompts` (line 433) and `.chat-empty-prompt` (line 441)
- `static/app.js`, `renderChatMessages()` (line 6369) — the `promptCards` template literal

**How:** Update the grid to cap at 2 columns on medium, and fix min-width:
```css
.chat-empty-prompts {
  display: grid;
  grid-template-columns: repeat(2, 1fr);
  gap: var(--gap-sm);
  width: 100%;
  max-width: 640px;
  margin-top: 8px;
}
@media (max-width: 500px) {
  .chat-empty-prompts {
    grid-template-columns: 1fr;
  }
}
```

Add staggered entry animation. In `renderChatMessages()`, update the `promptCards` map to include an inline `animation-delay`:
```js
const promptCards = prompts.map((p, i) => `
    <button class="chat-empty-prompt" style="animation-delay:${i * 60}ms"
            onclick="sendSuggestedPrompt('${escapeHtml(p.text)}')">
        <span class="chat-empty-prompt-icon">${p.icon}</span>
        <span class="chat-empty-prompt-text">${p.text}</span>
    </button>`).join('');
```

In `chat.css`, update `.chat-empty-prompt`:
```css
.chat-empty-prompt {
  /* existing styles */
  opacity: 0;
  animation: chat-fade-in 0.3s ease forwards;
}
```
This reuses the existing `@keyframes chat-fade-in` (chat.css:418).

Also add a left accent bar on hover to make interaction more tactile:
```css
.chat-empty-prompt:hover {
  /* existing hover */
  border-left: 3px solid rgba(99, 102, 241, 0.6);
  padding-left: 13px; /* 16px - 3px border */
}
```

---

### [M] Mobile/narrow viewport: collapse chat header controls

**What:** The `.chat-header` (index.html:575) contains `.chat-header-left` with five interactive elements side by side (Behavior, Settings, Style, AI/You name inputs, explicit toggle) plus `.chat-header-right` with font controls and Export. On viewports below 768px all of this wraps or overflows.

**Where:**
- `static/css/chat.css`, add new `@media` block
- `static/index.html`, optionally restructure `.chat-header` — but CSS-only approach preferred

**How:** Add a mobile breakpoint to `chat.css`:
```css
@media (max-width: 768px) {
  .chat-header {
    flex-wrap: wrap;
    gap: 6px;
    padding: 6px var(--gap-sm);
  }
  .chat-header-left,
  .chat-header-right {
    gap: 4px;
    flex-wrap: wrap;
  }
  .chat-name-inputs {
    display: none; /* hide AI/You name pills on mobile — accessible via system prompt */
  }
  .chat-font-controls {
    display: none; /* hide font size widget on mobile */
  }
  .chat-header-label {
    display: none; /* icon-only buttons on mobile */
  }
  .chat-tab-name {
    max-width: 80px;
  }
  .chat-empty-title {
    font-size: var(--text-base);
  }
  .chat-messages {
    padding: var(--gap-sm) var(--gap-sm) 60px;
  }
  .chat-input-row {
    padding: var(--gap-xs) var(--gap-sm);
    gap: 6px;
  }
}
```

Also update the existing `@media (max-width: 600px)` block (chat.css:928) to add missing rules:
```css
@media (max-width: 600px) {
  .chat-message { max-width: 96%; }
  .chat-messages { padding: var(--gap-sm); }
  .chat-params-grid { grid-template-columns: 1fr 1fr; }
  /* ADD: */
  .chat-scroll-bottom {
    bottom: 70px; /* above taller mobile input row */
  }
  .btn-chat-send {
    width: 38px;
    height: 38px;
  }
}
```

---

### [M] Tab bar: add overflow fade mask and keyboard navigation

**What:** The `.chat-tab-bar` (chat.css:25) hides its scrollbar with `scrollbar-width: none` but gives no visual indication that more tabs exist off-screen. There is also no keyboard way to switch tabs.

**Where:**
- `static/css/chat.css`, `.chat-tab-bar` (line 25)
- `static/app.js`, keyboard event handler — search for `keydown` event listener in the chat init block

**How:** Add a right-edge fade mask to `.chat-tab-bar` in `chat.css`:
```css
.chat-tab-bar {
  /* existing styles */
  -webkit-mask-image: linear-gradient(to right, black calc(100% - 40px), transparent 100%);
  mask-image: linear-gradient(to right, black calc(100% - 40px), transparent 100%);
}
/* Remove mask when no overflow (JS will add a class): */
.chat-tab-bar.no-overflow {
  -webkit-mask-image: none;
  mask-image: none;
}
```

In `app.js`, after `renderChatTabs()`, add an overflow check:
```js
function updateTabBarOverflowMask() {
    const bar = document.getElementById('chat-tab-bar');
    if (!bar) return;
    bar.classList.toggle('no-overflow', bar.scrollWidth <= bar.clientWidth);
}
```
Call `updateTabBarOverflowMask()` at the end of `renderChatTabs()` and on window resize.

For keyboard tab switching, add to the `DOMContentLoaded` initialization block:
```js
document.addEventListener('keydown', e => {
    if (!document.getElementById('page-chat').classList.contains('active')) return;
    if ((e.ctrlKey || e.metaKey) && e.key >= '1' && e.key <= '9') {
        e.preventDefault();
        const idx = parseInt(e.key) - 1;
        if (chatTabs[idx]) switchChatTab(chatTabs[idx].id);
    }
    if ((e.ctrlKey || e.metaKey) && e.shiftKey && e.key === 'ArrowRight') {
        e.preventDefault();
        const idx = chatTabs.findIndex(t => t.id === activeChatTabId);
        const next = chatTabs[(idx + 1) % chatTabs.length];
        if (next) switchChatTab(next.id);
    }
    if ((e.ctrlKey || e.metaKey) && e.shiftKey && e.key === 'ArrowLeft') {
        e.preventDefault();
        const idx = chatTabs.findIndex(t => t.id === activeChatTabId);
        const prev = chatTabs[(idx - 1 + chatTabs.length) % chatTabs.length];
        if (prev) switchChatTab(prev.id);
    }
});
```

---

### [M] Improve empty state: personalize with model name and animate icon

**What:** The `.chat-empty-icon` (chat.css:422) is a static SVG at 25% opacity. The title is generic "How can I help you today?" regardless of which AI name or model is loaded.

**Where:**
- `static/app.js`, `renderChatMessages()` (line 6369) — the `container.innerHTML = ...` template
- `static/css/chat.css`, `.chat-empty-icon` (line 422) and `.chat-empty-title` (line 423)

**How:** In `renderChatMessages()`, use the tab's `ai_name` and the global `lastLlamaMetrics?.model_name`:
```js
const tab = activeChatTab();
const aiName = tab?.ai_name || 'Assistant';
const modelName = lastLlamaMetrics?.model_name
    ? ` (${lastLlamaMetrics.model_name.split('/').pop().replace(/\.gguf$/i, '')})`
    : '';

container.innerHTML = `
  <div class="chat-empty">
    <div class="chat-empty-icon">
      <svg .../>  <!-- existing SVG -->
    </div>
    <p class="chat-empty-title">${escapeHtml(aiName)}${escapeHtml(modelName)} is ready</p>
    <p class="chat-empty-hint">Ask anything, or try a suggestion below</p>
    <div class="chat-empty-prompts">${promptCards}</div>
  </div>`;
```

Add a subtle float animation to the icon in `chat.css`:
```css
.chat-empty-icon {
  opacity: 0.3;
  margin-bottom: 4px;
  animation: chat-icon-float 3s ease-in-out infinite;
}
@keyframes chat-icon-float {
  0%, 100% { transform: translateY(0); }
  50% { transform: translateY(-5px); }
}
```

---

### [M] Send button: show spinner icon during busy state, not just disabled opacity

**What:** When `chatBusy` is true, `#btn-send` is disabled and fades to 45% opacity (chat.css:907). There is no indication that generation is in progress beyond the typing dots and the stop button appearing.

**Where:**
- `static/css/chat.css`, `.btn-chat-send:disabled` (line 907)
- `static/app.js`, `setChatBusyUI()` (line 6313)

**How:** In `setChatBusyUI(busy)`, swap the send icon for a spinner SVG:
```js
const sendBtn = document.getElementById('btn-send');
sendBtn.disabled = busy;
sendBtn.innerHTML = busy
    ? `<svg class="chat-send-spinner" width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
         <path d="M12 2v4M12 18v4M4.93 4.93l2.83 2.83M16.24 16.24l2.83 2.83M2 12h4M18 12h4M4.93 19.07l2.83-2.83M16.24 7.76l2.83-2.83"/>
       </svg>`
    : `<svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
         <path d="M22 2L11 13M22 2l-7 20-4-9-9-4 20-7z"/>
       </svg>`;
```

Add the spinner animation to `chat.css`:
```css
.chat-send-spinner {
  animation: chat-send-spin 0.9s linear infinite;
}
@keyframes chat-send-spin {
  to { transform: rotate(360deg); }
}
.btn-chat-send:disabled {
  opacity: 0.7; /* Less dim since spinner communicates state */
  cursor: not-allowed;
  box-shadow: none;
  animation: none;
}
```

---

### [M] System prompt & params panel: show "active" indicator on header buttons

**What:** When a system prompt is set, `#system-prompt-indicator` (a `.chat-header-badge` with "S") is shown (index.html:581). But there is no equivalent indicator for when non-default model params are active. Both indicators are visually quiet — a small badge doesn't stand out against the muted button style.

**Where:**
- `static/app.js` — `onParamChange()` and `resetParamsToDefaults()` functions (search for these names)
- `static/css/chat.css`, `.chat-header-btn` (line 136), `.chat-header-badge` (line 197)

**How:** In `onParamChange()`, after updating `tab.model_params`, check if any param differs from defaults and toggle a class on `#btn-model-params`:
```js
function updateParamsDirtyIndicator() {
    const tab = activeChatTab();
    if (!tab) return;
    const p = tab.model_params;
    const isDirty = p.temperature !== 0.7 || p.top_p !== 0.9
        || p.top_k !== 40 || p.min_p !== 0.01
        || p.repeat_penalty !== 1.0 || (p.max_tokens && p.max_tokens !== 0);
    const btn = document.getElementById('btn-model-params');
    if (btn) btn.classList.toggle('has-active-params', isDirty);
}
```
Call `updateParamsDirtyIndicator()` at the end of `onParamChange()` and `resetParamsToDefaults()`.

In `chat.css`, add a highlight state for the button:
```css
#btn-model-params.has-active-params {
  border-color: rgba(99, 102, 241, 0.25);
  background: rgba(99, 102, 241, 0.08);
  color: var(--color-primary-light);
}
#btn-model-params.has-active-params::before {
  content: '';
  position: absolute;
  top: 4px;
  right: 4px;
  width: 5px;
  height: 5px;
  border-radius: 50%;
  background: var(--color-primary-light);
}
```

---

### [L] Add per-language code block headers with a language name label and line count

**What:** The `pre` blocks in `.chat-message-assistant .chat-msg-body` (chat.css:937) have no header — just a dark box. Developers expect a language label and ideally a line count.

**Where:**
- `static/app.js` — the `marked` renderer setup and `finalizeAssistantMessage()` DOM decoration pass
- `static/css/chat.css` — new `.chat-code-header` rules

**How:** This builds on the code-block copy button fix above. Extend the DOM decoration in `finalizeAssistantMessage()`:
```js
body.querySelectorAll('pre').forEach(pre => {
    if (pre.parentElement?.classList.contains('chat-code-block')) return;
    const code = pre.querySelector('code');
    const lang = (code?.className.match(/language-(\w+)/) || [])[1] || '';
    const lineCount = (code?.innerText.match(/\n/g) || []).length + 1;

    const wrapper = document.createElement('div');
    wrapper.className = 'chat-code-block';

    const header = document.createElement('div');
    header.className = 'chat-code-header';
    header.innerHTML = `
        <span class="chat-code-lang">${lang || 'code'}</span>
        <span class="chat-code-lines">${lineCount} line${lineCount !== 1 ? 's' : ''}</span>
        <button class="chat-code-copy-btn" title="Copy code">
          <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <rect x="9" y="9" width="13" height="13" rx="2"/><path d="M5 15H4a2 2 0 01-2-2V4a2 2 0 012-2h9a2 2 0 012 2v1"/>
          </svg>
          Copy
        </button>`;

    header.querySelector('.chat-code-copy-btn').addEventListener('click', function() {
        navigator.clipboard.writeText(code?.innerText ?? pre.innerText).then(() => {
            this.classList.add('copied');
            this.innerHTML = '<svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><polyline points="20 6 9 17 4 12"/></svg> Copied';
            setTimeout(() => {
                this.classList.remove('copied');
                this.innerHTML = '<svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><rect x="9" y="9" width="13" height="13" rx="2"/><path d="M5 15H4a2 2 0 01-2-2V4a2 2 0 012-2h9a2 2 0 012 2v1"/></svg> Copy';
            }, 1500);
        });
    });

    pre.parentElement.insertBefore(wrapper, pre);
    wrapper.appendChild(header);
    wrapper.appendChild(pre);
});
```

Add to `chat.css`:
```css
.chat-code-block {
  margin: 0.5em 0;
  border-radius: 8px;
  overflow: hidden;
  border: 1px solid rgba(255, 255, 255, 0.08);
}
.chat-code-header {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 5px 12px;
  background: rgba(255, 255, 255, 0.04);
  border-bottom: 1px solid rgba(255, 255, 255, 0.06);
  font-size: 11px;
  font-family: var(--font-mono);
}
.chat-code-lang {
  color: var(--color-primary-light);
  font-weight: 600;
  text-transform: lowercase;
}
.chat-code-lines {
  color: var(--text-muted);
  opacity: 0.5;
  flex: 1;
}
.chat-code-copy-btn {
  display: flex;
  align-items: center;
  gap: 4px;
  padding: 2px 8px;
  border: 1px solid rgba(255, 255, 255, 0.08);
  border-radius: 5px;
  background: rgba(255, 255, 255, 0.04);
  color: var(--text-muted);
  cursor: pointer;
  font-size: 11px;
  font-family: var(--font-mono);
  transition: all 0.15s;
}
.chat-code-copy-btn:hover {
  background: rgba(255, 255, 255, 0.08);
  color: var(--text-primary);
}
.chat-code-copy-btn.copied {
  background: rgba(16, 185, 129, 0.15);
  color: #10b981;
  border-color: rgba(16, 185, 129, 0.3);
}
/* Remove old pre border-radius since the wrapper handles it */
.chat-message-assistant .chat-msg-body pre {
  background: rgba(0, 0, 0, 0.3);
  border-radius: 0 0 8px 8px; /* top corners now belong to .chat-code-block */
  padding: 10px 14px;
  overflow-x: auto;
  font-size: var(--text-sm);
  margin: 0;
}
```

---

### [L] Color and contrast polish: align to actual Nord palette

**What:** The token CSS (`tokens.css:10`) uses `#0f1115` for `--color-bg` and `#6366f1` for `--color-primary`, which are indigo-based. The Nord palette calls for `#2e3440` (bg), `#3b4252` (surface), `#88c0d0` (accent). The current theme is close to a dark indigo dashboard, not Nord. Several specific issues:

1. `--color-bg: #0f1115` is near-black, darker than Nord's polar night `#2e3440`.
2. Inline code in `.chat-msg-body code:not(pre code)` uses `rgba(0,0,0,0.25)` which has WCAG contrast ratio under 4.5:1 against `#eceff4` text — marginal.
3. Blockquotes use `rgba(99, 102, 241, 0.4)` border — indigo, not Nord's `#88c0d0`.
4. `.chat-message-user .chat-avatar` and `.chat-message-assistant .chat-avatar` colors are indigo/emerald — consider Nord's `#5e81ac` (blue) and `#a3be8c` (green) for Nord authenticity.

**Where:**
- `static/css/tokens.css` — root variable block (lines 10–35)
- `static/css/chat.css` — lines 561, 566, 957, 959

**How (selective — don't break the whole app):**

In `chat.css`, update blockquote border to Nord frost blue:
```css
/* Line 957 — replace rgba(99, 102, 241, 0.4) with Nord #88c0d0 */
.chat-message-assistant .chat-msg-body blockquote {
  border-left: 3px solid rgba(136, 192, 208, 0.45);
  padding-left: 10px;
  margin-left: 0;
  color: var(--text-secondary);
}
```

Improve inline code contrast — darken the background slightly:
```css
/* Line 945 */
.chat-message-assistant .chat-msg-body code:not(pre code) {
  background: rgba(0, 0, 0, 0.35);
  border: 1px solid rgba(255, 255, 255, 0.08); /* ADD this */
  border-radius: 4px;
  padding: 1px 5px;
  font-size: 0.9em;
}
```

Update avatars to Nord-adjacent colors in `chat.css`:
```css
/* Line 560 */
.chat-message-user .chat-avatar {
  background: rgba(94, 129, 172, 0.25); /* Nord #5e81ac */
  color: #7bafd4;
}
/* Line 564 */
.chat-message-assistant .chat-avatar {
  background: rgba(163, 190, 140, 0.2); /* Nord #a3be8c */
  color: #a3be8c;
}
```

Update the streaming border highlight to Nord accent:
```css
/* Line 594 */
.chat-message-streaming .chat-msg-body {
  border-color: rgba(136, 192, 208, 0.25); /* #88c0d0 at low alpha */
}
```

---

### [L] Add message fade-in per-chunk animation during streaming and "wow factor" features

**What:** New messages animate in with `chat-msg-in` (chat.css:477) but during streaming the text just appears character-by-character with no visual rhythm. Additionally, some low-cost "wow factor" CSS-only effects are missing.

**Where:**
- `static/css/chat.css` — new rules
- `static/app.js` — `appendAssistantPlaceholder()` and streaming chunk handler

**Wow factor item 1 — Gradient streaming border.** While a message is streaming (`.chat-message-streaming`), animate the assistant bubble border with a moving gradient:
```css
@keyframes streaming-border-pulse {
  0%   { border-color: rgba(136, 192, 208, 0.1); }
  50%  { border-color: rgba(136, 192, 208, 0.4); box-shadow: 0 0 12px rgba(136, 192, 208, 0.1); }
  100% { border-color: rgba(136, 192, 208, 0.1); }
}
.chat-message-streaming .chat-msg-body {
  border-color: rgba(136, 192, 208, 0.2);
  animation: streaming-border-pulse 2s ease-in-out infinite;
}
```

**Wow factor item 2 — Scroll-to-bottom button shows unread count.** When the scroll-to-bottom button is visible, count how many messages arrived below the fold and show a badge. In `initChatScrollButton()` in `app.js`, track the count when new messages arrive while the user is scrolled up, and update a `<span>` badge inside `#chat-scroll-bottom`. In `index.html` line 2075, update:
```html
<button class="chat-scroll-bottom" id="chat-scroll-bottom" onclick="chatScroll(true)" title="Scroll to bottom">
  <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5">
    <path d="M12 5v14M5 12l7 7 7-7"/>
  </svg>
  <span class="chat-scroll-badge" id="chat-scroll-badge" style="display:none;"></span>
</button>
```
Add to `chat.css`:
```css
.chat-scroll-badge {
  position: absolute;
  top: -5px;
  right: -5px;
  min-width: 16px;
  height: 16px;
  padding: 0 4px;
  border-radius: 999px;
  background: var(--color-primary);
  color: white;
  font-size: 9px;
  font-weight: 700;
  display: flex;
  align-items: center;
  justify-content: center;
}
```
In `app.js`, increment a `let unreadChatCount = 0` counter each time `appendAssistantPlaceholder()` or a streaming chunk arrives while `distFromBottom > 80`. Reset to 0 and hide the badge when `chatScroll(true)` is called.

**Wow factor item 3 — Input glow on focus with Nord accent.** Replace the current indigo focus ring with Nord frost:
```css
/* In chat.css line 1327 — replace existing */
.chat-input-row textarea:focus {
  outline: none;
  border-color: rgba(136, 192, 208, 0.5);
  box-shadow: 0 0 0 3px rgba(136, 192, 208, 0.08), 0 2px 8px rgba(0, 0, 0, 0.15);
}
```

**Wow factor item 4 — Tab active underline color matches message count.** When a tab has 20+ messages, tint its `::before` underline to amber; 50+ messages tints it to indigo (signals "long conversation"). Add via JS in `renderChatTabs()`, adding `data-msg-count="${tab.messages.length}"` to the tab `el`, then in CSS:
```css
.chat-tab[data-msg-count]:not([data-msg-count="0"]) { /* cosmetic progression */ }
/* Handled via: if > 20, add class .tab-warm; if > 50, add class .tab-hot */
.chat-tab.tab-warm::before { background: var(--color-warning); }
.chat-tab.tab-hot::before { background: var(--color-error); }
```

**Wow factor item 5 — Hover-reveal message timestamp.** Currently `.chat-msg-time` is always at 60% opacity. Make it more discoverable by starting hidden and revealing on bubble hover:
```css
.chat-msg-time {
  font-size: 11px;
  color: var(--text-muted);
  opacity: 0;
  transition: opacity 0.2s;
}
.chat-message:hover .chat-msg-time {
  opacity: 0.6;
}
```
This is a one-line change to the opacity value at chat.css line 639.

---

### [XL] Chat History Pagination — Limit Visible Messages

**What:** Long conversations with hundreds of messages cause browser memory bloat and slow rendering. The DOM accumulates all message elements indefinitely. Implement a message limit that shows only the most recent N messages (default: 15) with a "Load More" button to reveal older batches.

**Why:** Reduces DOM node count, lowers memory usage, improves scroll performance, and prevents browser tab crashes on very long sessions.

**Where:**
- `static/app.js` — `renderChatMessages()` (line 6369), `chatScroll()` (line 6324)
- `static/index.html` — add "Load More" button above message thread
- `static/css/chat.css` — add `.chat-load-more` styles

**How:**

Add a `visible_message_limit` field to the `ChatTab` interface (stored in `chat-tabs.json`). Default: `15`.

In `renderChatMessages()`, slice the messages array before rendering:

```js
function renderChatMessages() {
    const tab = activeChatTab();
    if (!tab) return;
    const container = document.getElementById('chat-messages');
    if (!container) return;

    const limit = tab.visible_message_limit || 15;
    const totalMessages = tab.messages.filter(m => m.role !== 'system').length;
    const isPaginated = totalMessages > limit;

    // Show only the most recent N messages
    const allMessages = tab.messages.filter(m => m.role !== 'system');
    const visibleMessages = isPaginated
        ? allMessages.slice(-limit)
        : allMessages;

    // Render messages (existing logic, but use visibleMessages instead of tab.messages)
    container.innerHTML = '';
    if (visibleMessages.length === 0) {
        // ... existing empty state logic ...
    } else {
        visibleMessages.forEach(m => {
            const el = buildMessageElement(m, tab);
            container.appendChild(el);
        });
    }

    // Add "Load More" button if paginated
    if (isPaginated) {
        const loadMoreBtn = document.createElement('button');
        loadMoreBtn.className = 'chat-load-more';
        loadMoreBtn.innerHTML = `
            <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <path d="M12 5v14M5 12l7 7 7-7"/>
            </svg>
            Load ${Math.min(limit, totalMessages - limit)} older messages
        `;
        loadMoreBtn.onclick = () => loadMoreMessages(tab, limit);
        container.insertBefore(loadMoreBtn, container.firstChild);
    }
}
```

Add the `loadMoreMessages()` function:

```js
function loadMoreMessages(tab, limit) {
    const allMessages = tab.messages.filter(m => m.role !== 'system');
    const currentVisible = allMessages.slice(-limit);
    const olderMessages = allMessages.slice(0, -limit);

    // Prepend older messages to the visible set
    // Re-render with expanded limit (double it each time, up to total)
    tab.visible_message_limit = Math.min(tab.visible_message_limit * 2, allMessages.length);
    renderChatMessages();

    // Scroll to maintain position (don't jump to bottom)
    const container = document.getElementById('chat-messages');
    if (container) {
        container.scrollTop = 0; // Stay at top where load-more button was
    }
}
```

Add a settings control in the system prompt panel to adjust the limit:

```html
<!-- In static/index.html, inside #chat-system-panel -->
<label class="modal-field">
    Visible message limit
    <input type="number" id="chat-msg-limit" min="5" max="200" step="5"
           value="15" oninput="onMessageLimitChange(+this.value)">
    <span class="modal-help">Show only the most recent N messages. Older messages are hidden to improve performance.</span>
</label>
```

Add CSS for the load-more button:

```css
.chat-load-more {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 6px;
    margin: 0 auto 12px;
    padding: 6px 14px;
    border-radius: 8px;
    background: rgba(255, 255, 255, 0.04);
    border: 1px solid rgba(255, 255, 255, 0.08);
    color: var(--text-muted);
    font-size: 11px;
    font-weight: 500;
    cursor: pointer;
    transition: background 0.2s, border-color 0.2s, color 0.2s;
}
.chat-load-more:hover {
    background: rgba(255, 255, 255, 0.08);
    border-color: rgba(255, 255, 255, 0.15);
    color: var(--text-secondary);
}
```

**Important:** The full message history is still preserved in `tab.messages` and persisted to `chat-tabs.json`. Only the rendering is limited. Export, search, and API access still see all messages.

---

## Implementation Status

| # | Fix | Status | Notes |
|---|---|---|---|
| 1 | Wire `#chat-typing` to `setChatBusyUI` | ✅ Done | Removed inline-dot injection |
| 2 | Fix `chatScroll()` force-scroll | ✅ Done | Smart scroll with `distFromBottom < 80` |
| 3 | Panel open/close animation (class-based) | ✅ Done | `.open` class replaces `display:none`; `style="display:none;"` removed from HTML |
| 4 | Char count → token count with color warning | ✅ Done | Warning at 800+, error at 1500+ |
| 5 | Token metadata symbol tooltips | ✅ Done | `title` attr set on meta elements in `buildMessageElement` and `finalizeAssistantMessage` |
| 6 | Textarea height transition + layout thrash fix | ✅ Done | `requestAnimationFrame` separates reset from final height; CSS `transition: height 0.1s` added |
| 7 | Syntax highlighting (highlight.js) | ✅ Done | hljs CDN loaded; `renderMdStreaming()` passes `new marked.Renderer()` to bypass the global hljs renderer during streaming; `finalizeAssistantMessage()` runs `hljs.highlightElement()` on finalized blocks |
| 8 | Per-code-block copy button | ✅ Done | DOM decoration in `finalizeAssistantMessage()` |
| 9 | Suggested prompts layout + stagger animation | ✅ Done | 2-column grid, per-card `animation-delay`, hover left accent bar |
| 10 | Mobile header collapse | ✅ Done | `@media (max-width: 768px)` hides `.chat-name-inputs` and `.chat-font-controls` |
| 11 | Tab bar overflow mask + keyboard shortcuts | ✅ Done | CSS `mask-image` fade + `updateTabBarOverflowMask()`; Ctrl+1–9 and Ctrl+Shift+Arrow tab switching |
| 12 | Empty state personalization + icon float | ✅ Done | Uses `tab.ai_name` + `lastLlamaMetrics.model_name`; `@keyframes chat-icon-float` added |
| 13 | Send button spinner | ✅ Done | `setChatBusyUI()` swaps SVG icon; `@keyframes chat-send-spin` added |
| 14 | Model params dirty indicator | ✅ Done | `updateParamsDirtyIndicator()` toggles `.has-active-params` on `#btn-model-params`; dot indicator via `::before` |
| 15 | Code block header (lang + lines + copy) | ✅ Done | DOM decoration in `finalizeAssistantMessage()` wraps `<pre>` in `.chat-code-block` with `.chat-code-header` |
| 16 | Nord color palette alignment | ✅ Done | Blockquotes → `rgba(136,192,208,0.45)`; avatars → Nord blue/green; streaming border → `#88c0d0` |
| 17 | Streaming border pulse + scroll badge + wow extras | ✅ Done | `@keyframes streaming-border-pulse`; `#chat-scroll-badge` unread counter; hover-reveal timestamps; Nord focus glow on textarea |
| 18 | Chat history pagination | ✅ Done | `tab.visible_message_limit` (default 15) persisted per-tab; `loadMoreMessages()` doubles limit; `onMessageLimitChange()` settings input; `.chat-load-more` button |

### Implementation Notes

**Item 7 — `renderMdStreaming` and hljs**: The spec described creating a separate non-highlighting render path for streaming. In the implementation, `marked.setOptions({ renderer })` sets the hljs renderer globally. Passing `{ gfm: true, breaks: true }` alone to `marked.parse()` does **not** override the global renderer in marked v4 — the renderer merges with global state. The fix is to explicitly pass `renderer: new marked.Renderer()` in `renderMdStreaming()` to reset to the default renderer for streaming calls only. This was corrected during code review.

**Item 18 — Pagination state**: `visible_message_limit` is stored on the tab object and persisted to `chat-tabs.json` via `scheduleChatPersist()`. Switching tabs preserves each tab's own limit independently. The full `tab.messages` array is always intact — only rendering is windowed.
