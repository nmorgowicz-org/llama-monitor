// ── Chat Rendering ────────────────────────────────────────────────────────────
// Rendering functions for chat tabs, messages, compaction markers, and actions.
// Calls rendering functions via window.* to avoid circular imports.

// ── Markdown rendering ────────────────────────────────────────────────────────

function renderMd(src) {
    if (typeof marked !== 'undefined') {
        try { return marked.parse(src); } catch(_) {}
    }
    return src.replace(/&/g,'&amp;').replace(/</g,'&lt;').replace(/\n/g,'<br>');
}

function renderMdStreaming(src) {
    if (typeof marked !== 'undefined') {
        try { return marked.parse(src, { gfm: true, breaks: true, renderer: new marked.Renderer() }); } catch(_) {}
    }
    return src.replace(/&/g,'&amp;').replace(/</g,'&lt;').replace(/\n/g,'<br>');
}

// ── Scroll ────────────────────────────────────────────────────────────────────

function chatScroll(force = false) {
    const c = document.getElementById('chat-messages');
    if (!c) return;
    const distFromBottom = c.scrollHeight - c.scrollTop - c.clientHeight;
    if (force || distFromBottom < 80) {
        c.scrollTop = c.scrollHeight;
    }
    if (force) {
        window.unreadChatCount = 0;
        const badge = document.getElementById('chat-scroll-badge');
        if (badge) badge.style.display = 'none';
    }
}

function initChatScrollButton() {
    const container = document.getElementById('chat-messages');
    const btn = document.getElementById('chat-scroll-bottom');
    if (!container || !btn) return;

    const checkScroll = () => {
        const distFromBottom = container.scrollHeight - container.scrollTop - container.clientHeight;
        btn.classList.toggle('visible', distFromBottom > 100);
    };

    container.addEventListener('scroll', checkScroll, { passive: true });
    requestAnimationFrame(() => requestAnimationFrame(checkScroll));
}

function incrementUnreadCount() {
    const container = document.getElementById('chat-messages');
    if (!container) return;
    const distFromBottom = container.scrollHeight - container.scrollTop - container.clientHeight;
    if (distFromBottom > 80) {
        window.unreadChatCount++;
        const badge = document.getElementById('chat-scroll-badge');
        if (badge) {
            badge.textContent = window.unreadChatCount;
            badge.style.display = 'flex';
        }
    }
}

// ── Tab rendering ─────────────────────────────────────────────────────────────

function renderChatTabs() {
    const bar = document.getElementById('chat-tab-bar');
    const addBtn = bar.querySelector('.chat-tab-add');
    bar.querySelectorAll('.chat-tab').forEach(el => el.remove());

    for (const tab of window.chatTabs) {
        const el = document.createElement('div');
        const msgCount = tab.messages.filter(m => m.role !== 'system').length;
        let extraClasses = '';
        if (msgCount > 50) extraClasses = ' tab-hot';
        else if (msgCount > 20) extraClasses = ' tab-warm';
        el.className = 'chat-tab' + (tab.id === window.activeChatTabId ? ' active' : '') + extraClasses;
        el.dataset.tabId = tab.id;
        el.dataset.msgCount = msgCount;
        el.innerHTML = `
          <span class="chat-tab-name" data-chat-tab-rename="${tab.id}">${window.escapeHtml(tab.name)}</span>
          <span class="chat-tab-count">${tab.messages.filter(m => m.role !== 'system').length || ''}</span>
          ${window.chatTabs.length > 1
            ? `<button class="chat-tab-close" data-chat-tab-close="${tab.id}" title="Close tab">×</button>`
            : ''}
        `;
        el.addEventListener('click', e => {
            const closeBtn = e.target.closest('.chat-tab-close');
            if (closeBtn) return;
            if (e.target.classList.contains('chat-tab-name') && e.detail === 2) return;
            window.switchChatTab(tab.id);
        });
        bar.insertBefore(el, addBtn);
    }
    updateTabBarOverflowMask();
}

function updateTabBarOverflowMask() {
    const bar = document.getElementById('chat-tab-bar');
    if (!bar) return;
    bar.classList.toggle('no-overflow', bar.scrollWidth <= bar.clientWidth);
}

// ── Message rendering ─────────────────────────────────────────────────────────

function renderChatMessages() {
    const container = document.getElementById('chat-messages');
    const tab = window.activeChatTab();

    if (!tab || tab.messages.filter(m => m.role !== 'system').length === 0) {
        const prompts = [
            { icon: '💡', text: 'Explain a complex topic simply', label: 'Learn something' },
            { icon: '✍️', text: 'Help me write an email about...', label: 'Write something' },
            { icon: '🔍', text: 'Compare the pros and cons of...', label: 'Analyze something' },
            { icon: '🎨', text: 'Give me creative ideas for...', label: 'Brainstorm' },
        ];
        const promptCards = prompts.map((p, i) => `
            <button class="chat-empty-prompt" style="animation-delay:${i * 60}ms"
                    data-prompt-text="${window.escapeHtml(p.text)}">
                <span class="chat-empty-prompt-icon">${p.icon}</span>
                <span class="chat-empty-prompt-text">${p.text}</span>
            </button>`).join('');

        const aiName = tab?.ai_name || 'Assistant';
        const modelName = window.lastLlamaMetrics?.model_name
            ? ` (${window.lastLlamaMetrics.model_name.split('/').pop().replace(/\.gguf$/i, '')})`
            : '';

        container.innerHTML = `
          <div class="chat-empty">
            <div class="chat-empty-icon">
              <svg width="48" height="48" viewBox="0 0 24 24" fill="none"
                   stroke="currentColor" stroke-width="1.2" opacity="0.25">
                <path d="M21 15a2 2 0 01-2 2H7l-4 4V5a2 2 0 012-2h14a2 2 0 012 2z"/>
              </svg>
            </div>
            <p class="chat-empty-title">${window.escapeHtml(aiName)}${window.escapeHtml(modelName)} is ready</p>
            <p class="chat-empty-hint">Ask anything, or try a suggestion below</p>
            <div class="chat-empty-prompts">${promptCards}</div>
          </div>`;
        return;
    }

    const allMessages = tab.messages.filter(m => m.role !== 'system' || m.compaction_marker);
    const limit = tab.visible_message_limit || 15;
    const isPaginated = allMessages.length > limit;
    const visibleMessages = isPaginated ? allMessages.slice(-limit) : allMessages;

    container.innerHTML = '';

    // Add "Load More" button if paginated
    if (isPaginated) {
        const loadMoreBtn = document.createElement('button');
        loadMoreBtn.className = 'chat-load-more';
        const olderCount = allMessages.length - limit;
        loadMoreBtn.innerHTML = `
            <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <path d="M12 5v14M5 12l7 7 7-7"/>
            </svg>
            Load ${Math.min(limit, olderCount)} older messages
        `;
        loadMoreBtn.onclick = () => loadMoreMessages(tab, limit);
        container.appendChild(loadMoreBtn);
    }

    let idx = 0;
    for (const msg of visibleMessages) {
        const el = buildMessageElement(msg, idx, tab.messages);
        const realIdx = tab.messages.indexOf(msg);
        if (realIdx >= 0) el.dataset.msgIdx = realIdx;
        container.appendChild(el);
        idx++;
    }
    chatScroll(true);
    window.syncCompactSettingsUI(activeChatTab());
}

function loadMoreMessages(tab, currentLimit) {
    const allMessages = tab.messages.filter(m => m.role !== 'system' || m.compaction_marker);
    tab.visible_message_limit = Math.min(currentLimit * 2, allMessages.length);
    renderChatMessages();
    const container = document.getElementById('chat-messages');
    if (container) container.scrollTop = 0;
}

function buildMessageElement(msg, idx, allMessages) {
    const isUser = msg.role === 'user';
    const tab = window.activeChatTab();
    const wrapper = document.createElement('div');

    // Render compaction tombstone as a divider
    if (msg.compaction_marker) {
        wrapper.className = 'chat-message chat-compact-marker' + (msg.summarized ? ' compact-marker-summarized' : ' compact-marker-truncated');
        wrapper.dataset.compactState = 'final';
        wrapper.dataset.expanded = 'false';

        const isSummarized = !!msg.summarized;
        const droppedCount = msg.dropped_count || 0;
        const ctxBefore = msg.ctx_pct_before || 0;

        let statsHtml = `${droppedCount} messages removed`;
        if (ctxBefore > 0) statsHtml += ` · was ${ctxBefore}% ctx`;

        const labelText = isSummarized ? 'Context summarized' : 'Context trimmed';
        const iconPath = isSummarized
            ? '<path d="M9 12h6M9 16h6M9 8h6M5 4h14a2 2 0 012 2v14a2 2 0 01-2 2H5a2 2 0 01-2-2V6a2 2 0 012-2z"/>'
            : '<path d="M8 6h13M8 12h13M8 18h13M3 6h.01M3 12h.01M3 18h.01"/>';

        let bodyHtml = '';
        if (isSummarized) {
            const summaryText = msg.content.replace(/^\[Context compacted[^\]]*\]\s*/i, '').trim();
            bodyHtml = summaryText ? renderMd(summaryText) : '';
        } else if (msg.dropped_preview && msg.dropped_preview.length > 0) {
            const rows = msg.dropped_preview.map(p => {
                const label = p.role === 'user' ? 'You' : 'AI';
                return `<div class="compact-peek-row"><span class="compact-peek-role">${label}</span><span class="compact-peek-snippet">${window.escapeHtml(p.snippet)}${p.snippet.length >= 80 ? '…' : ''}</span></div>`;
            }).join('');
            bodyHtml = `<div class="compact-peek-list">${rows}</div>`;
        }

        wrapper.innerHTML = `
          <div class="compact-marker-content">
            <div class="compact-marker-rule compact-marker-rule-left"></div>
            <div class="compact-marker-pill" data-compact-toggle="true">
              <svg class="compact-marker-icon" width="11" height="11" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">${iconPath}</svg>
              <span class="compact-marker-label">${labelText}</span>
              <span class="compact-marker-stats">${statsHtml}</span>
              <svg class="compact-marker-chevron" width="10" height="10" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5"><path d="M6 9l6 6 6-6"/></svg>
            </div>
            <div class="compact-marker-rule compact-marker-rule-right"></div>
          </div>
          <div class="compact-marker-body" style="display:none;">${bodyHtml}</div>`;

        return wrapper;
    }

    wrapper.className = `chat-message chat-message-${msg.role}`;

    const ts = msg.timestamp_ms
        ? new Date(msg.timestamp_ms).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })
        : '';
    const aiLabel = tab?.ai_name || 'AI';
    const userLabel = tab?.user_name || 'You';

    let metaHtml = '';
    if (!isUser) {
        const parts = [];
        if (msg.input_tokens > 0) parts.push(`↓${formatTokenCount(msg.input_tokens)}`);
        if (msg.output_tokens > 0) parts.push(`↑${formatTokenCount(msg.output_tokens)}`);
        let cumInput = 0, cumOutput = 0;
        for (let i = 0; i <= idx; i++) {
            const m = allMessages[i];
            if (m.role === 'assistant') {
                cumInput += m.input_tokens || 0;
                cumOutput += m.output_tokens || 0;
            }
        }
        const cumTotal = cumInput + cumOutput;
        if (cumTotal > 0) parts.push(`R${formatTokenCount(cumTotal)}`);
        const capacity = window.lastLlamaMetrics?.context_capacity_tokens || 0;
        const ctxPct = capacity > 0 ? Math.round((cumTotal / capacity) * 100) : 0;
        if (ctxPct > 0) parts.push(`${ctxPct}% ctx`);
        const modelName = msg.model_name || window.lastLlamaMetrics?.model_name || '';
        if (modelName) parts.push(modelName);
        if (parts.length > 0) {
            metaHtml = `<span class="chat-msg-meta-sep">·</span><span class="chat-msg-meta-model" title="↓ = prompt tokens in · ↑ = tokens generated · R = running total · ctx = % of context window used">${parts.join(' · ')}</span>`;
        }
    }

    wrapper.innerHTML = `
      <div class="chat-avatar">${isUser ? userLabel : aiLabel}</div>
      <div class="chat-bubble">
        <div class="chat-msg-body">${isUser ? window.escapeHtml(msg.content).replace(/\n/g, '<br>') : renderMd(msg.content)}</div>
        <div class="chat-msg-footer">
          <span class="chat-msg-time">${ts}</span>
          ${metaHtml}
          <div class="chat-msg-actions">
            <button class="chat-action-btn" data-chat-action="copy" title="Copy">
              <svg width="13" height="13" viewBox="0 0 24 24" fill="none"
                   stroke="currentColor" stroke-width="2">
                <rect x="9" y="9" width="13" height="13" rx="2"/><path d="M5 15H4a2 2 0 01-2-2V4a2 2 0 012-2h9a2 2 0 012 2v1"/>
              </svg>
            </button>
            ${!isUser ? (() => {
                const variants = msg._variants || [];
                const curIdx = msg._variantIndex || 0;
                const total = variants.length || 1;
                const canGoLeft = variants.length > 1 && curIdx > 0;
                const canGoRight = variants.length > 1 ? curIdx < variants.length - 1 : true;
                return `
            <button class="chat-action-btn" data-chat-action="nav-variant" data-variant-dir="-1" title="Previous response" ${canGoLeft ? '' : 'disabled'}>
              <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <path d="M15 18l-6-6 6-6"/>
              </svg>
            </button>
            <span class="chat-variant-badge">${curIdx+1}/${total}</span>
            <button class="chat-action-btn" data-chat-action="nav-variant" data-variant-dir="1" title="${canGoRight && variants.length <= 1 ? 'Regenerate' : 'Next response'}" ${canGoRight ? '' : 'disabled'}>
              <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <path d="M9 18l6-6-6-6"/>
              </svg>
            </button>`;
            })() : ''}
            <button class="chat-action-btn" data-chat-action="edit" title="Edit">
              <svg width="13" height="13" viewBox="0 0 24 24" fill="none"
                   stroke="currentColor" stroke-width="2">
                <path d="M11 4H4a2 2 0 00-2 2v14a2 2 0 002 2h14a2 2 0 002-2v-7"/>
                <path d="M18.5 2.5a2.121 2.121 0 013 3L12 15l-4 1 1-4 9.5-9.5z"/>
              </svg>
            </button>
            <button class="chat-action-btn chat-action-btn-delete" data-chat-action="delete" title="Delete">
              <svg width="13" height="13" viewBox="0 0 24 24" fill="none"
                   stroke="currentColor" stroke-width="2">
                <path d="M3 6h18M8 6V4a2 2 0 012-2h4a2 2 0 012 2v2M19 6l-1 14a2 2 0 01-2 2H8a2 2 0 01-2-2L5 6"/>
              </svg>
            </button>
          </div>
        </div>
      </div>`;

    return wrapper;
}

function formatTokenCount(n) {
    if (n >= 1000000) return (n / 1000000).toFixed(1) + 'M';
    if (n >= 1000) return (n / 1000).toFixed(1) + 'k';
    return String(n);
}

// ── Streaming helpers ─────────────────────────────────────────────────────────

function appendAssistantPlaceholder() {
    const container = document.getElementById('chat-messages');
    const tab = window.activeChatTab();
    const aiLabel = tab?.ai_name || 'AI';
    const wrapper = document.createElement('div');
    wrapper.className = 'chat-message chat-message-assistant chat-message-streaming';
    wrapper.innerHTML = `
      <div class="chat-avatar">${aiLabel}</div>
      <div class="chat-bubble">
        <div class="chat-msg-body"><span class="chat-cursor">▋</span></div>
        <div class="chat-msg-footer">
          <span class="chat-msg-time"></span>
          <span class="chat-msg-meta-sep">·</span>
          <span class="chat-msg-meta-model"></span>
          <div class="chat-msg-actions"></div>
        </div>
      </div>`;
    container.appendChild(wrapper);
    chatScroll(true);
    return wrapper;
}

function appendThinkingBlock(afterEl) {
    const details = document.createElement('details');
    details.className = 'chat-thinking';
    details.innerHTML = `
      <summary class="chat-thinking-summary">
        <svg class="chat-thinking-icon" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M12 2l2.4 7.4H22l-6.2 4.5 2.4 7.4L12 16.8l-6.2 4.5 2.4-7.4L2 9.4h7.6z"/></svg>
        <span class="chat-thinking-label">Thinking</span>
        <span class="chat-thinking-dots"><span>.</span><span>.</span><span>.</span></span>
        <span class="chat-thinking-hint">(click to expand)</span>
      </summary>
      <div class="chat-thinking-body"></div>`;
    afterEl.parentElement.insertBefore(details, afterEl);
    return details;
}

function finalizeAssistantMessage(el, content, usage, tab) {
    el.classList.remove('chat-message-streaming');
    const body = el.querySelector('.chat-msg-body');
    if (content) {
        body.innerHTML = renderMd(content);
        if (typeof hljs !== 'undefined') {
            body.querySelectorAll('pre code:not(.hljs)').forEach(codeEl => {
                hljs.highlightElement(codeEl);
            });
        }
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
    }
    const time = el.querySelector('.chat-msg-time');
    if (time) {
        time.textContent = new Date().toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' });
    }
    const actions = el.querySelector('.chat-msg-actions');
    if (actions && content) {
        const tab = window.activeChatTab();
        const variants = tab?._pendingVariants || null;
        let msg = null;
        if (variants) {
            tab._pendingVariants = null;
            for (let i = tab.messages.length - 1; i >= 0; i--) {
                if (tab.messages[i].role === 'assistant') {
                    tab.messages[i]._variants = variants;
                    tab.messages[i]._variantIndex = variants.length - 1;
                    msg = tab.messages[i];
                    break;
                }
            }
        }
        if (!msg) {
            const allMsgs = Array.from(document.querySelectorAll('#chat-messages .chat-message'));
            const idx = allMsgs.indexOf(el);
            const firstVisibleIdx = tab?.messages.findIndex(m => m.role !== 'system');
            const msgIdx = firstVisibleIdx + idx;
            msg = tab?.messages[msgIdx] || null;
        }
        const hasVariants = msg && msg._variants && msg._variants.length > 1;
        const variantIdx = msg?._variantIndex || 0;

        actions.innerHTML = `
          <button class="chat-action-btn" data-chat-action="copy" title="Copy">
            <svg width="13" height="13" viewBox="0 0 24 24" fill="none"
                  stroke="currentColor" stroke-width="2">
              <rect x="9" y="9" width="13" height="13" rx="2"/>
              <path d="M5 15H4a2 2 0 01-2-2V4a2 2 0 012-2h9a2 2 0 012 2v1"/>
            </svg>
          </button>
          <button class="chat-action-btn" data-chat-action="regenerate" title="Regenerate">
            <svg width="13" height="13" viewBox="0 0 24 24" fill="none"
                  stroke="currentColor" stroke-width="2">
              <path d="M1 4v6h6M23 20v-6h-6"/>
              <path d="M20.5 9A9 9 0 005.6 5.6L1 10m22 4l-4.6 4.4A9 9 0 013.5 15"/>
            </svg>
          </button>
          ${hasVariants ? `
          <button class="chat-action-btn" data-chat-action="nav-variant" data-variant-dir="-1" title="Previous variant" ${variantIdx <= 0 ? 'disabled' : ''}>
            <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <path d="M15 18l-6-6 6-6"/>
            </svg>
          </button>
          <span class="chat-variant-badge">${variantIdx+1}/${msg._variants.length}</span>
          <button class="chat-action-btn" data-chat-action="nav-variant" data-variant-dir="1" title="Next variant" ${variantIdx >= msg._variants.length-1 ? 'disabled' : ''}>
            <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <path d="M9 18l6-6-6-6"/>
            </svg>
          </button>` : ''}
          <button class="chat-action-btn" data-chat-action="edit" title="Edit">
            <svg width="13" height="13" viewBox="0 0 24 24" fill="none"
                 stroke="currentColor" stroke-width="2">
              <path d="M11 4H4a2 2 0 00-2 2v14a2 2 0 002 2h14a2 2 0 002-2v-7"/>
              <path d="M18.5 2.5a2.121 2.121 0 013 3L12 15l-4 1 1-4 9.5-9.5z"/>
            </svg>
          </button>
          <button class="chat-action-btn chat-action-btn-delete" data-chat-action="delete" title="Delete">
            <svg width="13" height="13" viewBox="0 0 24 24" fill="none"
                 stroke="currentColor" stroke-width="2">
              <path d="M3 6h18M8 6V4a2 2 0 012-2h4a2 2 0 012 2v2M19 6l-1 14a2 2 0 01-2 2H8a2 2 0 01-2-2L5 6"/>
            </svg>
          </button>`;
    }

    // Populate footer metadata (single line)
    const footer = el.querySelector('.chat-msg-footer');
    if (footer) {
        const modelName = window.lastLlamaMetrics?.model_name || '';
        const inp = usage ? (usage.prompt_tokens ?? 0) : 0;
        const out = usage ? (usage.completion_tokens ?? 0) : 0;
        const totalInput = tab ? (tab.totalInputTokens || 0) : inp;
        const totalOutput = tab ? (tab.totalOutputTokens || 0) : out;
        const total = totalInput + totalOutput;
        const capacity = window.lastLlamaMetrics?.context_capacity_tokens || 0;
        const ctxPct = capacity > 0 ? Math.round((total / capacity) * 100) : 0;

        if (tab) tab.lastCtxPct = ctxPct;

        const parts = [];
        if (inp > 0) parts.push(`↓${formatTokenCount(inp)}`);
        if (out > 0) parts.push(`↑${formatTokenCount(out)}`);
        if (total > 0) parts.push(`R${formatTokenCount(total)}`);
        if (ctxPct > 0) parts.push(`${ctxPct}% ctx`);
        if (modelName) parts.push(modelName);

        const metaEl = footer.querySelector('.chat-msg-meta-model');
        if (metaEl) {
            metaEl.textContent = parts.join(' · ');
            metaEl.title = '↓ = prompt tokens in · ↑ = tokens generated · R = running total · ctx = % of context window used';
        }
        const sepEl = footer.querySelector('.chat-msg-meta-sep');
        if (sepEl) sepEl.style.display = parts.length > 0 ? '' : 'none';
    }
}

// ── Message actions ───────────────────────────────────────────────────────────

function copyMessageContent(btn) {
    const body = btn.closest('.chat-bubble').querySelector('.chat-msg-body');
    navigator.clipboard.writeText(body.innerText).then(() => {
        btn.classList.add('chat-action-btn-copied');
        setTimeout(() => btn.classList.remove('chat-action-btn-copied'), 1500);
    });
}

function navigateVariant(btn, direction) {
    const msgEl = btn.closest('.chat-message');
    const msgIdx = parseInt(msgEl.dataset.msgIdx);
    const tab = window.activeChatTab();
    if (!tab || isNaN(msgIdx)) return;

    const msg = tab.messages[msgIdx];
    if (!msg || msg.role !== 'assistant') return;

    const variants = msg._variants || [];
    const curIdx = msg._variantIndex || 0;

    // Going right on the last variant (or only variant) → regenerate
    if (direction === 1 && (variants.length <= 1 || curIdx >= variants.length - 1)) {
        if (window.chatBusy) return;

        let newVariants = variants.length > 0 ? [...variants, msg.content] : [msg.content];

        // Find the last user message before this assistant message
        const lastUser = [...tab.messages].reverse().find(m => m.role === 'user');
        if (!lastUser) return;
        const userMsgIdx = tab.messages.indexOf(lastUser);

        // Truncate to include the user message, remove all subsequent
        tab.messages = tab.messages.slice(0, userMsgIdx + 1);
        tab.updated_at = Date.now();

        tab._pendingVariants = newVariants;
        window.scheduleChatPersist();

        // User message is already in tab.messages — use sendChatResend
        window.sendChatResend(tab);
        return;
    }

    if (!variants || variants.length <= 1) return;

    msg._variantIndex = Math.max(0, Math.min(variants.length - 1, curIdx + direction));
    msg.content = msg._variants[msg._variantIndex];
    tab.updated_at = Date.now();

    renderChatMessages();
    window.scheduleChatPersist();
}

function regenerateFromMessage(btn) {
    const msgEl = btn.closest('.chat-message');
    const msgIdx = parseInt(msgEl.dataset.msgIdx);
    const tab = window.activeChatTab();
    if (!tab || isNaN(msgIdx)) return;

    const msg = tab.messages[msgIdx];
    if (!msg || msg.role !== 'assistant') return;

    // Find the last user message before this assistant message
    const lastUser = [...tab.messages].reverse().find(m => m.role === 'user');
    if (!lastUser) return;
    const userMsgIdx = tab.messages.indexOf(lastUser);

    // Truncate to include the user message, remove all subsequent
    tab.messages = tab.messages.slice(0, userMsgIdx + 1);
    tab.updated_at = Date.now();
    window.scheduleChatPersist();

    // User message is already in tab.messages — use sendChatResend
    window.sendChatResend(tab);
}

function editMessageContent(btn) {
    const msgEl = btn.closest('.chat-message');
    const body = msgEl.querySelector('.chat-msg-body');
    const msgIdx = parseInt(msgEl.dataset.msgIdx);
    const tab = window.activeChatTab();
    if (!tab || isNaN(msgIdx)) return;

    const msg = tab.messages[msgIdx];
    if (!msg) return;

    const isLastUserMsg = msg.role === 'user' &&
        tab.messages.slice(msgIdx + 1).every(m => m.role !== 'user');

    const resendBtn = isLastUserMsg
        ? `<button class="chat-edit-btn chat-edit-btn-resend" data-chat-edit="resend">Resend</button>`
        : '';
    body.innerHTML = `<textarea class="chat-msg-edit-area" rows="6">${window.escapeHtml(msg.content)}</textarea>
      <div class="chat-msg-edit-actions">
        ${resendBtn}
        <button class="chat-edit-btn chat-edit-btn-save" data-chat-edit="save">Save</button>
        <button class="chat-edit-btn chat-edit-btn-cancel" data-chat-edit="cancel">Cancel</button>
      </div>`;
    const textarea = body.querySelector('.chat-msg-edit-area');
    textarea.focus();
    textarea.selectionStart = textarea.value.length;
}

function resendMessageEdit(btn) {
    const msgEl = btn.closest('.chat-message');
    const body = msgEl.querySelector('.chat-msg-body');
    const textarea = body.querySelector('.chat-msg-edit-area');
    const msgIdx = parseInt(msgEl.dataset.msgIdx);
    const tab = window.activeChatTab();
    if (!tab || !textarea || isNaN(msgIdx)) return;

    const msg = tab.messages[msgIdx];
    if (!msg || msg.role !== 'user') return;

    const newContent = textarea.value.trim();
    if (!newContent) return;

    msg.content = newContent;
    tab.updated_at = Date.now();

    // Truncate to include the user message, remove all subsequent messages
    tab.messages = tab.messages.slice(0, msgIdx + 1);
    window.scheduleChatPersist();

    // Use sendChatResend — the user message is already in tab.messages
    window.sendChatResend(tab);
}

function saveMessageEdit(btn) {
    const msgEl = btn.closest('.chat-message');
    const body = msgEl.querySelector('.chat-msg-body');
    const textarea = body.querySelector('.chat-msg-edit-area');
    const msgIdx = parseInt(msgEl.dataset.msgIdx);
    const tab = window.activeChatTab();
    if (!tab || !textarea || isNaN(msgIdx)) return;

    const msg = tab.messages[msgIdx];
    if (!msg) return;

    const newContent = textarea.value.trim();
    if (newContent !== msg.content) {
        msg.content = newContent;
        tab.updated_at = Date.now();
        window.scheduleChatPersist();
    }
    renderChatMessages();
}

function cancelMessageEdit(btn) {
    renderChatMessages();
}

function deleteMessage(btn) {
    if (!confirm('Delete this message?')) return;
    const msgEl = btn.closest('.chat-message');
    const msgIdx = parseInt(msgEl.dataset.msgIdx);
    const tab = window.activeChatTab();
    if (!tab || isNaN(msgIdx) || msgIdx < 0 || msgIdx >= tab.messages.length) return;

    tab.messages.splice(msgIdx, 1);
    tab.updated_at = Date.now();
    renderChatMessages();
    window.scheduleChatPersist();
}

// ── Export / Import ───────────────────────────────────────────────────────────

function exportChatTab() {
    const tab = window.activeChatTab();
    if (!tab) return;
    const md = tab.messages
        .filter(m => m.role !== 'system')
        .map(m => `**${m.role === 'user' ? 'You' : 'Assistant'}**\n\n${m.content}`)
        .join('\n\n---\n\n');
    const blob = new Blob([md], { type: 'text/markdown' });
    const a = document.createElement('a');
    a.href = URL.createObjectURL(blob);
    a.download = `${tab.name.replace(/[^a-z0-9]/gi, '-').toLowerCase()}.md`;
    a.click();
    URL.revokeObjectURL(a.href);
}

function importChatTab() {
    const input = document.createElement('input');
    input.type = 'file';
    input.accept = '.json,.md';
    input.onchange = e => {
        const file = e.target.files[0];
        if (!file) return;
        const reader = new FileReader();
        reader.onload = ev => {
            try {
                if (file.name.endsWith('.json')) {
                    const data = JSON.parse(ev.target.result);
                    if (Array.isArray(data) && data.length > 0) {
                        const newTab = data[0];
                        newTab.id = crypto.randomUUID();
                        newTab.created_at = Date.now();
                        newTab.updated_at = Date.now();
                        window.chatTabs.push(newTab);
                        window.switchChatTab(newTab.id);
                        window.scheduleChatPersist();
                        window.showToast('Conversation imported', 'success');
                    }
                } else {
                    const lines = ev.target.result.split(/\n---\n/);
                    const messages = [];
                    for (const block of lines) {
                        const match = block.match(/\*\*(You|Assistant)\*\*\s*\n\n([\s\S]+)/);
                        if (match) {
                            messages.push({
                                role: match[1] === 'You' ? 'user' : 'assistant',
                                content: match[2].trim(),
                                timestamp_ms: Date.now(),
                            });
                        }
                    }
                    if (messages.length > 0) {
                        const tab = window.activeChatTab();
                        tab.messages = [...tab.messages, ...messages];
                        tab.updated_at = Date.now();
                        renderChatMessages();
                        window.scheduleChatPersist();
                        window.showToast(`Imported ${messages.length} messages`, 'success');
                    }
                }
            } catch (err) {
                window.showToast('Import failed: ' + err.message, 'error');
            }
        };
        reader.readAsText(file);
    };
    input.click();
}

// ── Tab rename ────────────────────────────────────────────────────────────────

function startRenameTab(id) {
    const tabEl = document.querySelector(`.chat-tab[data-tab-id="${id}"] .chat-tab-name`);
    if (!tabEl) return;
    const orig = tabEl.textContent;
    tabEl.contentEditable = 'true';
    tabEl.focus();
    const range = document.createRange();
    range.selectNodeContents(tabEl);
    window.getSelection().removeAllRanges();
    window.getSelection().addRange(range);
    const finish = () => {
        tabEl.contentEditable = 'false';
        window.renameChatTab(id, tabEl.textContent || orig);
    };
    tabEl.addEventListener('blur', finish, { once: true });
    tabEl.addEventListener('keydown', e => {
        if (e.key === 'Enter') { e.preventDefault(); tabEl.blur(); }
        if (e.key === 'Escape') { tabEl.textContent = orig; tabEl.blur(); }
    }, { once: true });
}

// ── Badge ─────────────────────────────────────────────────────────────────────

function updateChatTabBadge() {
    const tab = window.activeChatTab();
    const count = tab ? tab.messages.filter(m => m.role !== 'system').length : 0;
    const badge = document.getElementById('sidebar-badge-chat');
    if (badge) badge.textContent = count > 0 ? count : '';
}

// ── Public API ────────────────────────────────────────────────────────────────

export function initChatRender() {
    // Call setup functions that bind DOM event listeners
    initChatScrollButton();

    // Event delegation for chat tab close buttons
    document.getElementById('chat-tab-bar')?.addEventListener('click', (e) => {
        const closeBtn = e.target.closest('.chat-tab-close');
        if (closeBtn) {
            window.closeChatTab(closeBtn.dataset.chatTabClose);
        }
    });

    // Event delegation for chat tab rename (dblclick)
    document.getElementById('chat-tab-bar')?.addEventListener('dblclick', (e) => {
        const renameEl = e.target.closest('[data-chat-tab-rename]');
        if (renameEl) {
            window.startRenameTab(renameEl.dataset.chatTabRename);
        }
    });

    // Event delegation for chat message action buttons
    document.getElementById('chat-messages')?.addEventListener('click', (e) => {
        const actionBtn = e.target.closest('[data-chat-action]');
        if (!actionBtn) return;
        const action = actionBtn.dataset.chatAction;
        if (action === 'copy') copyMessageContent(actionBtn);
        else if (action === 'regenerate') regenerateFromMessage(actionBtn);
        else if (action === 'nav-variant') navigateVariant(actionBtn, +actionBtn.dataset.variantDir);
        else if (action === 'edit') editMessageContent(actionBtn);
        else if (action === 'delete') deleteMessage(actionBtn);
    });

    // Event delegation for chat message edit buttons
    document.getElementById('chat-messages')?.addEventListener('click', (e) => {
        const editBtn = e.target.closest('[data-chat-edit]');
        if (!editBtn) return;
        const editAction = editBtn.dataset.chatEdit;
        if (editAction === 'resend') resendMessageEdit(editBtn);
        else if (editAction === 'save') saveMessageEdit(editBtn);
        else if (editAction === 'cancel') cancelMessageEdit(editBtn);
    });

    // Event delegation for suggested prompt buttons
    document.getElementById('chat-messages')?.addEventListener('click', (e) => {
        const promptBtn = e.target.closest('[data-prompt-text]');
        if (promptBtn) {
            window.sendSuggestedPrompt(promptBtn.dataset.promptText);
        }
    });

    // Event delegation for compact marker toggle
    document.getElementById('chat-messages')?.addEventListener('click', (e) => {
        const pill = e.target.closest('[data-compact-toggle]');
        if (!pill) return;
        const marker = pill.closest('.chat-compact-marker');
        if (!marker) return;
        const body = marker.querySelector('.compact-marker-body');
        if (!body) return;
        const isExpanded = marker.dataset.expanded === 'true';
        body.style.display = isExpanded ? 'none' : 'block';
        marker.dataset.expanded = isExpanded ? 'false' : 'true';
    });

    // Register functions on window for cross-module calls
    window.renderMd = renderMd;
    window.renderMdStreaming = renderMdStreaming;
    window.chatScroll = chatScroll;
    window.incrementUnreadCount = incrementUnreadCount;
    window.renderChatTabs = renderChatTabs;
    window.updateTabBarOverflowMask = updateTabBarOverflowMask;
    window.renderChatMessages = renderChatMessages;
    window.appendAssistantPlaceholder = appendAssistantPlaceholder;
    window.appendThinkingBlock = appendThinkingBlock;
    window.finalizeAssistantMessage = finalizeAssistantMessage;
    window.exportChatTab = exportChatTab;
    window.updateChatTabBadge = updateChatTabBadge;
}
