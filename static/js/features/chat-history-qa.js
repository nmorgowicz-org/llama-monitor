// ── Chat History Q&A ──────────────────────────────────────────────────────────
// Slide-in panel for asking natural language questions about the active
// tab's conversation history. Supports multi-turn Q&A, manual context
// injection, and AI-driven keyword search over in-memory messages.
//
// Thread state is ephemeral (per session, per tab) — never persisted.

import { activeChatTab, scheduleChatPersist } from './chat-state.js';
import { renderMd, renderMdStreaming, renderChatMessages } from './chat-render.js';
import { showToast } from './toast.js';
import { escapeHtml } from '../core/format.js';

// ── Constants ─────────────────────────────────────────────────────────────────

const PANEL_ID          = 'chat-history-qa-panel';
const BTN_ID            = 'chat-history-qa-btn';
const THREAD_ID         = 'chqa-thread';
const EMPTY_ID          = 'chqa-empty';
const INPUT_ID          = 'chqa-input';
const SEND_BTN_ID       = 'chqa-send-btn';
const STOP_BTN_ID       = 'chqa-stop-btn';
const CLOSE_BTN_ID      = 'chqa-close-btn';
const CLEAR_BTN_ID      = 'chqa-clear-btn';
const CONTEXT_LBL_ID    = 'chqa-context-label';
const STATUS_BAR_ID     = 'chqa-status-bar';
const STATUS_TEXT_ID    = 'chqa-status-text';
const INJECT_TOGGLE_ID  = 'chqa-inject-toggle';
const INJECT_PANEL_ID   = 'chqa-inject-panel';
const INJECT_INPUT_ID   = 'chqa-inject-input';
const INJECT_BADGE_ID   = 'chqa-inject-badge';

const INSERT_EDITOR_ID       = 'chqa-insert-editor';
const INSERT_TEXTAREA_ID     = 'chqa-insert-textarea';
const INSERT_ROLE_USER_ID    = 'chqa-insert-role-user';
const INSERT_ROLE_ASST_ID    = 'chqa-insert-role-asst';
const INSERT_CONFIRM_ID      = 'chqa-insert-confirm';
const INSERT_CANCEL_ID       = 'chqa-insert-cancel';
const INSERT_WRITE_BTN_ID    = 'chqa-write-scene-btn';

const MAX_TRANSCRIPT_CHARS  = 120_000;
const MAX_QA_HISTORY_TURNS  = 6;   // sliding window — older turns pruned first
const MAX_SEARCH_MATCHES    = 6;
const MAX_MATCH_CHARS       = 420; // per matched message in search block
const KEYWORD_TIMEOUT_MS    = 3_000;

const SYSTEM_PROMPT =
    'You are a precise conversation analyst. You have been given the full transcript of an ongoing chat.\n\n' +
    'Answer questions about what happened in this conversation — events, characters, decisions, plot points, ' +
    'or any other details. Be accurate and specific, citing relevant exchanges when helpful. ' +
    'If something is not in the transcript, say so clearly.\n\n' +
    'Keep answers focused and concise. Do not continue the story, roleplay, or generate new content — ' +
    'only report on what is already in the transcript.';

const SUGGESTIONS = [
    'What has happened in this conversation so far?',
    'Who are the main characters and what are their goals?',
    'What important decisions or events have taken place?',
    'What storylines or questions are still unresolved?',
];

// ── Stop words for client-side keyword fallback ───────────────────────────────
const STOP_WORDS = new Set([
    'the','a','an','is','are','was','were','what','who','when','where','how',
    'did','does','do','i','you','he','she','it','we','they','this','that',
    'these','those','in','on','at','to','for','of','with','by','from','about',
    'as','into','through','during','before','after','above','below','between',
    'and','or','but','if','then','because','while','although','though','since',
    'until','unless','not','no','nor','so','yet','be','been','being','have',
    'has','had','having','will','would','could','should','may','might','shall',
    'can','get','got','just','its','his','her','our','their','my','your',
]);

// ── Module state ──────────────────────────────────────────────────────────────

// Map<tabId, { entries: QAEntry[], apiHistory: Message[] }>
// apiHistory: first element is the transcript setup message, then alternating
//   user/assistant pairs. First turn bundles the transcript; follow-ups don't.
const threads = new Map();

let abortController = null;
let streaming = false;
let renderedTabId = null;

// ── Init ──────────────────────────────────────────────────────────────────────

export function initChatHistoryQA() {
    document.getElementById(BTN_ID)?.addEventListener('click', toggleHistoryQAPanel);
    document.getElementById(CLOSE_BTN_ID)?.addEventListener('click', closeHistoryQAPanel);
    document.getElementById(CLEAR_BTN_ID)?.addEventListener('click', clearThread);
    document.getElementById(SEND_BTN_ID)?.addEventListener('click', handleSend);
    document.getElementById(STOP_BTN_ID)?.addEventListener('click', handleStop);
    document.getElementById(INJECT_TOGGLE_ID)?.addEventListener('click', toggleInjectionPanel);
    document.getElementById(INSERT_WRITE_BTN_ID)?.addEventListener('click', () => openInsertEditor(''));
    document.getElementById(INSERT_CONFIRM_ID)?.addEventListener('click', submitInsert);
    document.getElementById(INSERT_CANCEL_ID)?.addEventListener('click', closeInsertEditor);

    const insertTextarea = document.getElementById(INSERT_TEXTAREA_ID);
    if (insertTextarea) {
        insertTextarea.addEventListener('input', () => autoResize(insertTextarea));
        insertTextarea.addEventListener('keydown', e => {
            if (e.key === 'Escape') { e.preventDefault(); closeInsertEditor(); }
        });
    }

    const input = document.getElementById(INPUT_ID);
    if (input) {
        input.addEventListener('keydown', e => {
            if (e.key === 'Enter' && !e.shiftKey) {
                e.preventDefault();
                handleSend();
            }
        });
        input.addEventListener('input', () => autoResize(input));
    }

    const injectInput = document.getElementById(INJECT_INPUT_ID);
    if (injectInput) {
        injectInput.addEventListener('input', updateInjectBadge);
    }

    renderSuggestions();
}

// ── Public panel controls ─────────────────────────────────────────────────────

export function toggleHistoryQAPanel() {
    const panel = document.getElementById(PANEL_ID);
    if (!panel) return;
    if (panel.classList.contains('open')) {
        closeHistoryQAPanel();
    } else {
        openHistoryQAPanel();
    }
}

export function openHistoryQAPanel() {
    const panel = document.getElementById(PANEL_ID);
    const btn   = document.getElementById(BTN_ID);
    if (!panel) return;

    // Close other panels
    document.getElementById('chat-behavior-panel')?.classList.remove('open');
    document.getElementById('chat-params-panel')?.classList.remove('open');
    document.getElementById('release-notes-panel')?.classList.remove('open');
    document.getElementById('btn-behavior')?.classList.remove('active');
    document.getElementById('btn-model-params')?.classList.remove('active');

    panel.classList.add('open');
    btn?.classList.add('active');
    btn?.setAttribute('aria-pressed', 'true');

    refreshForCurrentTab();
    setTimeout(() => document.getElementById(INPUT_ID)?.focus(), 60);
}

export function closeHistoryQAPanel() {
    const panel = document.getElementById(PANEL_ID);
    const btn   = document.getElementById(BTN_ID);
    panel?.classList.remove('open');
    btn?.classList.remove('active');
    btn?.setAttribute('aria-pressed', 'false');
}

// ── Tab refresh ───────────────────────────────────────────────────────────────

function refreshForCurrentTab() {
    const tab   = activeChatTab();
    const tabId = tab?.id ?? null;

    const msgCount = countVisibleMessages(tab);
    const labelEl  = document.getElementById(CONTEXT_LBL_ID);
    if (labelEl) {
        labelEl.textContent = msgCount > 0
            ? `${msgCount} msg${msgCount !== 1 ? 's' : ''}`
            : 'no history';
    }

    if (tabId !== renderedTabId) {
        renderedTabId = tabId;
        renderThread(tabId);
    }
}

function countVisibleMessages(tab) {
    if (!tab?.messages) return 0;
    return tab.messages.filter(m => !m.compaction_marker && m.role !== 'system').length;
}

// ── Thread helpers ────────────────────────────────────────────────────────────

function getThread(tabId) {
    if (!threads.has(tabId)) threads.set(tabId, { entries: [], apiHistory: [] });
    return threads.get(tabId);
}

function clearThread() {
    const tab = activeChatTab();
    if (!tab) return;
    threads.delete(tab.id);
    renderedTabId = null;
    refreshForCurrentTab();
}

// ── Thread rendering ──────────────────────────────────────────────────────────

function renderThread(tabId) {
    const threadEl = document.getElementById(THREAD_ID);
    const emptyEl  = document.getElementById(EMPTY_ID);
    if (!threadEl) return;

    threadEl.querySelectorAll('.chqa-entry').forEach(el => el.remove());

    const entries = tabId ? (getThread(tabId).entries) : [];
    if (entries.length === 0) {
        emptyEl?.classList.remove('chqa-hidden');
    } else {
        emptyEl?.classList.add('chqa-hidden');
        for (const entry of entries) threadEl.appendChild(buildEntryEl(entry));
        scrollBottom(threadEl);
    }
}

function buildEntryEl(entry) {
    const div  = document.createElement('div');
    div.className = 'chqa-entry';
    div.dataset.entryId = entry.id;

    const qEl  = document.createElement('div');
    qEl.className = 'chqa-question';
    qEl.textContent = entry.question;
    div.appendChild(qEl);

    const aEl  = document.createElement('div');
    aEl.className = 'chqa-answer' + (entry.streaming ? ' streaming' : '');
    const bodyEl = document.createElement('div');
    bodyEl.className = 'chqa-answer-body';
    setAnswerBodyContent(bodyEl, entry);
    aEl.appendChild(bodyEl);

    if (!entry.streaming && !entry.error && entry.answer) {
        aEl.appendChild(buildInsertActionBar(entry.answer));
    }

    div.appendChild(aEl);
    return div;
}

function buildInsertActionBar(answerText) {
    const bar = document.createElement('div');
    bar.className = 'chqa-answer-actions';

    const btn = document.createElement('button');
    btn.type = 'button';
    btn.className = 'chqa-insert-btn';
    btn.title = 'Insert this into the conversation history';
    btn.setAttribute('aria-label', 'Insert into conversation history');
    btn.innerHTML =
        '<svg width="11" height="11" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M12 5v14M5 12l7 7 7-7"/></svg>' +
        '<span>Insert into history</span>';
    btn.addEventListener('click', () => openInsertEditor(answerText));
    bar.appendChild(btn);
    return bar;
}

function setAnswerBodyContent(bodyEl, entry) {
    if (entry.streaming && !entry.answer) {
        bodyEl.innerHTML = '<div class="chqa-thinking-dots"><span></span><span></span><span></span></div>';
        return;
    }
    if (entry.error) {
        bodyEl.innerHTML = `<span class="chqa-answer-error">${escapeHtml(entry.answer || 'Something went wrong.')}</span>`;
        return;
    }
    if (entry.answer) {
        // eslint-disable-next-line no-unsanitized/property -- local LLM output, rendered via marked+DOMPurify
        bodyEl.innerHTML = renderMd(entry.answer);
    }
}

function updateEntryDOM(entryId, answer, done, error) {
    const entryEl = document.querySelector(`.chqa-entry[data-entry-id="${CSS.escape(entryId)}"]`);
    if (!entryEl) return;
    const aEl    = entryEl.querySelector('.chqa-answer');
    const bodyEl = entryEl.querySelector('.chqa-answer-body');
    if (!bodyEl) return;

    if (done || error) aEl?.classList.remove('streaming');

    if (error) {
        bodyEl.innerHTML = `<span class="chqa-answer-error">${escapeHtml(answer || 'Something went wrong.')}</span>`;
    } else if (done && answer) {
        // eslint-disable-next-line no-unsanitized/property -- local LLM output, rendered via marked+DOMPurify
        bodyEl.innerHTML = renderMd(answer);
        // Add insert action bar once streaming is complete
        if (!aEl.querySelector('.chqa-answer-actions')) {
            aEl.appendChild(buildInsertActionBar(answer));
        }
    } else if (answer) {
        // eslint-disable-next-line no-unsanitized/property -- local LLM output, rendered via marked+DOMPurify
        bodyEl.innerHTML = renderMdStreaming(answer);
    }

    const threadEl = document.getElementById(THREAD_ID);
    if (threadEl) scrollBottom(threadEl);
}

function scrollBottom(el) {
    el.scrollTop = el.scrollHeight;
}

// ── Input helpers ─────────────────────────────────────────────────────────────

function autoResize(el) {
    el.style.height = 'auto';
    el.style.height = Math.min(el.scrollHeight, 120) + 'px';
}

function renderSuggestions() {
    const container = document.getElementById('chqa-suggestions');
    if (!container) return;
    container.innerHTML = '';
    for (const text of SUGGESTIONS) {
        const btn = document.createElement('button');
        btn.className  = 'chqa-suggestion-btn';
        btn.type       = 'button';
        btn.textContent = text;
        btn.addEventListener('click', () => {
            const input = document.getElementById(INPUT_ID);
            if (input && !streaming) {
                input.value = text;
                autoResize(input);
                handleSend();
            }
        });
        container.appendChild(btn);
    }
}

// ── Injection panel ───────────────────────────────────────────────────────────

function toggleInjectionPanel() {
    const panel  = document.getElementById(INJECT_PANEL_ID);
    const toggle = document.getElementById(INJECT_TOGGLE_ID);
    if (!panel) return;
    const nowHidden = panel.classList.toggle('chqa-hidden');
    toggle?.setAttribute('aria-expanded', String(!nowHidden));
    if (!nowHidden) {
        document.getElementById(INJECT_INPUT_ID)?.focus();
    }
}

function updateInjectBadge() {
    const badge = document.getElementById(INJECT_BADGE_ID);
    const input = document.getElementById(INJECT_INPUT_ID);
    if (!badge || !input) return;
    const hasText = input.value.trim().length > 0;
    badge.classList.toggle('chqa-hidden', !hasText);
}

function consumeInjection() {
    const input = document.getElementById(INJECT_INPUT_ID);
    const text  = input?.value.trim() ?? '';
    if (input) {
        input.value = '';
        autoResize(input);
    }
    updateInjectBadge();
    // Close the panel after consuming
    document.getElementById(INJECT_PANEL_ID)?.classList.add('chqa-hidden');
    document.getElementById(INJECT_TOGGLE_ID)?.setAttribute('aria-expanded', 'false');
    return text;
}

// ── History insertion editor ──────────────────────────────────────────────────

function openInsertEditor(prefillText) {
    const editor   = document.getElementById(INSERT_EDITOR_ID);
    const textarea = document.getElementById(INSERT_TEXTAREA_ID);
    const roleAsst = document.getElementById(INSERT_ROLE_ASST_ID);
    if (!editor || !textarea) return;

    textarea.value = prefillText ?? '';
    if (roleAsst) roleAsst.checked = true;   // default: assistant
    editor.classList.remove('chqa-hidden');
    editor.setAttribute('aria-hidden', 'false');

    // Give the textarea time to appear before focusing
    requestAnimationFrame(() => {
        autoResize(textarea);
        textarea.focus();
        textarea.setSelectionRange(0, 0);
    });
}

function closeInsertEditor() {
    const editor = document.getElementById(INSERT_EDITOR_ID);
    if (!editor) return;
    editor.classList.add('chqa-hidden');
    editor.setAttribute('aria-hidden', 'true');
    const textarea = document.getElementById(INSERT_TEXTAREA_ID);
    if (textarea) textarea.value = '';
}

function submitInsert() {
    const tab      = activeChatTab();
    const textarea = document.getElementById(INSERT_TEXTAREA_ID);
    const roleUser = document.getElementById(INSERT_ROLE_USER_ID);
    if (!tab || !textarea) return;

    const content = textarea.value.trim();
    if (!content) {
        showToast('Nothing to insert', 'error', 'Write the scene content first.');
        return;
    }

    const role = roleUser?.checked ? 'user' : 'assistant';
    const newMsg = {
        role,
        content,
        timestamp_ms: Date.now(),
        compaction_marker: false,
        input_tokens: null,
        output_tokens: null,
    };

    tab.messages = [...tab.messages, newMsg];
    tab.updated_at = Date.now();
    renderChatMessages();
    scheduleChatPersist();

    closeInsertEditor();
    showToast(
        'Inserted into history',
        'success',
        `Added as ${role === 'user' ? 'User' : 'Assistant'} turn at end of conversation.`,
    );
}

// ── Status bar ────────────────────────────────────────────────────────────────

function setStatus(text) {
    const bar     = document.getElementById(STATUS_BAR_ID);
    const textEl  = document.getElementById(STATUS_TEXT_ID);
    if (!bar) return;
    if (text) {
        if (textEl) textEl.textContent = text;
        bar.classList.remove('chqa-hidden');
    } else {
        bar.classList.add('chqa-hidden');
    }
}

// ── Send / stop ───────────────────────────────────────────────────────────────

async function handleSend() {
    if (streaming) return;
    const input = document.getElementById(INPUT_ID);
    const text  = input?.value.trim();
    if (!text) return;

    const tab = activeChatTab();
    if (!tab) return;

    input.value = '';
    autoResize(input);

    const injectionText   = consumeInjection();
    const visibleCount    = countVisibleMessages(tab);
    const hasCompaction   = (tab.messages ?? []).some(m => m.compaction_marker && m.content?.trim());

    if (visibleCount === 0 && !hasCompaction) {
        addEntry(tab.id, {
            question: text,
            answer: 'This conversation has no history yet. Start chatting first, then come back to ask questions about what happened.',
            streaming: false,
            error: false,
        });
        return;
    }

    await submitQuestion(tab, text, injectionText);
}

function handleStop() {
    abortController?.abort();
    abortController = null;
}

// ── Entry management ──────────────────────────────────────────────────────────

function addEntry(tabId, entryData) {
    const thread  = getThread(tabId);
    const id      = `chqa-${Date.now()}-${Math.random().toString(36).slice(2, 6)}`;
    const entry   = { id, ...entryData };
    thread.entries.push(entry);

    const threadEl = document.getElementById(THREAD_ID);
    const emptyEl  = document.getElementById(EMPTY_ID);
    if (!threadEl) return entry;

    emptyEl?.classList.add('chqa-hidden');
    threadEl.appendChild(buildEntryEl(entry));
    scrollBottom(threadEl);
    return entry;
}

// ── Main submit ───────────────────────────────────────────────────────────────

async function submitQuestion(tab, question, injectionText) {
    const thread        = getThread(tab.id);
    const isFirstTurn   = thread.apiHistory.length === 0;

    const entry = addEntry(tab.id, { question, answer: '', streaming: true, error: false });
    setStreamingUI(true);

    // ── Step 1: AI keyword extraction + in-memory search ─────────────────────
    setStatus('Searching history…');
    const keywords    = await extractKeywords(question);
    const searchBlock = keywords ? searchMessages(tab.messages, keywords) : null;
    setStatus(null);

    // ── Step 2: Build the API messages array ──────────────────────────────────
    let messages;
    if (isFirstTurn) {
        // First question: bundle transcript into the opening user message
        let transcript = buildHistoryTranscript(tab);

        if (searchBlock) {
            transcript = `=== RELEVANT MESSAGES FOUND BY SEARCH ===\n${searchBlock}\n\n=== FULL TRANSCRIPT ===\n${transcript}`;
        }
        if (injectionText) {
            transcript = `=== ADDED CONTEXT ===\n${injectionText}\n\n${transcript}`;
        }

        const setupMsg = `Here is the conversation transcript:\n\n${transcript}\n\n---\n\nQuestion: ${question}`;
        messages = [
            { role: 'system', content: SYSTEM_PROMPT },
            { role: 'user',   content: setupMsg },
        ];
    } else {
        // Follow-up: transcript already in history, only send the new question
        let userContent = question;
        if (searchBlock) {
            userContent += `\n\n(Relevant passages found by search:\n${searchBlock})`;
        }
        if (injectionText) {
            userContent += `\n\n(Added context:\n${injectionText})`;
        }
        messages = [
            { role: 'system', content: SYSTEM_PROMPT },
            ...trimApiHistory(thread.apiHistory),
            { role: 'user',   content: userContent },
        ];
    }

    // ── Step 3: Stream the answer ─────────────────────────────────────────────
    abortController = new AbortController();
    let answer = '';
    let failed = false;

    try {
        const resp = await fetch('/api/chat', {
            method: 'POST',
            headers: window.authHeaders
                ? { ...window.authHeaders(), 'Content-Type': 'application/json' }
                : { 'Content-Type': 'application/json' },
            signal: abortController.signal,
            body: JSON.stringify({
                messages,
                stream: true,
                temperature: 0.3,
                thinking_budget_tokens: 0,
                chat_template_kwargs: { enable_thinking: false },
            }),
        });

        if (!resp.ok) throw new Error(`HTTP ${resp.status}`);

        const reader  = resp.body.getReader();
        const decoder = new TextDecoder();
        let buf = '';

        while (true) {
            const { done, value } = await reader.read();
            if (done) break;
            buf += decoder.decode(value, { stream: true });

            const lines = buf.split('\n');
            buf = lines.pop() ?? '';

            for (const line of lines) {
                if (!line.startsWith('data:')) continue;
                const payload = line.slice(5).trim();
                if (payload === '[DONE]') continue;
                try {
                    const obj   = JSON.parse(payload);
                    const delta = obj.choices?.[0]?.delta;
                    if (delta?.content) {
                        answer += delta.content;
                        updateEntryDOM(entry.id, answer, false, false);
                    }
                } catch { /* skip malformed chunk */ }
            }
        }

    } catch (err) {
        if (err.name === 'AbortError') {
            if (!answer) { answer = '[Stopped]'; failed = true; }
        } else {
            answer = `Error: ${err.message}`;
            failed = true;
        }
    }

    // ── Step 4: Finalize entry ────────────────────────────────────────────────
    entry.answer    = answer;
    entry.streaming = false;
    entry.error     = failed && answer !== '[Stopped]';
    updateEntryDOM(entry.id, answer, true, entry.error);

    // ── Step 5: Accumulate multi-turn history ─────────────────────────────────
    if (!failed || answer !== '[Stopped]') {
        const userMsg = isFirstTurn
            ? messages[1]                          // the full setup message (transcript + q)
            : messages[messages.length - 1];       // the plain follow-up question
        thread.apiHistory.push(userMsg);
        thread.apiHistory.push({ role: 'assistant', content: answer });
    }

    abortController = null;
    setStreamingUI(false);
    document.getElementById(INPUT_ID)?.focus();
}

// Keep apiHistory within the sliding window; always preserve index 0 (transcript setup).
function trimApiHistory(history) {
    if (history.length <= 1) return history;
    const [setup, ...rest] = history;
    const maxRest = MAX_QA_HISTORY_TURNS * 2; // user+assistant per turn
    return [setup, ...rest.slice(-maxRest)];
}

function setStreamingUI(active) {
    streaming = active;
    const sendBtn  = document.getElementById(SEND_BTN_ID);
    const stopBtn  = document.getElementById(STOP_BTN_ID);
    const input    = document.getElementById(INPUT_ID);
    if (sendBtn) sendBtn.disabled = active;
    if (stopBtn) stopBtn.classList.toggle('chqa-hidden', !active);
    if (input)   input.disabled   = active;
}

// ── AI keyword extraction ─────────────────────────────────────────────────────

async function extractKeywords(question) {
    const keywordMessages = [
        {
            role: 'system',
            content: 'Extract 3 to 5 search keywords from the user question. Return ONLY a comma-separated list of keywords. No other text.',
        },
        { role: 'user', content: question },
    ];

    const timeoutController = new AbortController();
    const timer = setTimeout(() => timeoutController.abort(), KEYWORD_TIMEOUT_MS);

    try {
        const resp = await fetch('/api/chat', {
            method: 'POST',
            headers: window.authHeaders
                ? { ...window.authHeaders(), 'Content-Type': 'application/json' }
                : { 'Content-Type': 'application/json' },
            signal: timeoutController.signal,
            body: JSON.stringify({
                messages: keywordMessages,
                stream: true,
                max_tokens: 40,
                temperature: 0,
                thinking_budget_tokens: 0,
                chat_template_kwargs: { enable_thinking: false },
            }),
        });

        if (!resp.ok) throw new Error('keyword extraction failed');

        const reader  = resp.body.getReader();
        const decoder = new TextDecoder();
        let buf = '';
        let raw = '';

        while (true) {
            const { done, value } = await reader.read();
            if (done) break;
            buf += decoder.decode(value, { stream: true });
            const lines = buf.split('\n');
            buf = lines.pop() ?? '';
            for (const line of lines) {
                if (!line.startsWith('data:')) continue;
                const payload = line.slice(5).trim();
                if (payload === '[DONE]') continue;
                try {
                    const delta = JSON.parse(payload).choices?.[0]?.delta;
                    if (delta?.content) raw += delta.content;
                } catch { /* skip */ }
            }
        }

        clearTimeout(timer);
        return raw.trim() || extractKeywordsFallback(question);

    } catch {
        clearTimeout(timer);
        return extractKeywordsFallback(question);
    }
}

function extractKeywordsFallback(question) {
    return question
        .toLowerCase()
        .replace(/[^a-z0-9\s]/g, ' ')
        .split(/\s+/)
        .filter(w => w.length > 2 && !STOP_WORDS.has(w))
        .slice(0, 5)
        .join(', ');
}

// ── In-memory message search ──────────────────────────────────────────────────

function searchMessages(messages, keywordsStr) {
    const keywords = keywordsStr
        .split(',')
        .map(k => k.trim().toLowerCase())
        .filter(k => k.length > 2);

    if (!keywords.length) return null;

    const candidates = (messages ?? []).filter(
        m => !m.compaction_marker && m.role !== 'system' && m.content?.trim()
    );

    const scored = candidates
        .map(m => {
            const lower  = m.content.toLowerCase();
            const hits   = keywords.filter(kw => lower.includes(kw)).length;
            return { m, hits };
        })
        .filter(({ hits }) => hits > 0)
        .sort((a, b) => b.hits - a.hits)
        .slice(0, MAX_SEARCH_MATCHES)
        .map(({ m }) => m);

    if (!scored.length) return null;

    return scored
        .map(m => {
            const label   = m.role === 'user' ? 'User' : 'Assistant';
            const content = m.content.length > MAX_MATCH_CHARS
                ? m.content.slice(0, MAX_MATCH_CHARS) + '…'
                : m.content;
            return `${label}: ${content}`;
        })
        .join('\n\n');
}

// ── Transcript builder ────────────────────────────────────────────────────────

function buildHistoryTranscript(tab) {
    const compactionMarkers = (tab.messages ?? []).filter(m => m.compaction_marker && m.content?.trim());
    const conversational    = (tab.messages ?? []).filter(m => !m.compaction_marker && m.role !== 'system');

    const parts = [];

    if (compactionMarkers.length > 0) {
        parts.push('=== COMPACTED HISTORY (earlier events summarized) ===');
        compactionMarkers.forEach((marker, i) => {
            parts.push(`Memory ${i + 1}:\n${marker.content.trim()}`);
        });
        parts.push('=== LIVE CONVERSATION ===');
    }

    const convoText = conversational
        .map(m => `${m.role === 'user' ? 'User' : 'Assistant'}: ${m.content}`)
        .join('\n\n');
    parts.push(convoText);

    let transcript = parts.join('\n\n');

    if (transcript.length > MAX_TRANSCRIPT_CHARS) {
        const headChars = Math.floor(MAX_TRANSCRIPT_CHARS * 0.25);
        const tailChars = MAX_TRANSCRIPT_CHARS - headChars;
        transcript =
            transcript.slice(0, headChars) +
            '\n\n[... middle of conversation omitted for length ...]\n\n' +
            transcript.slice(-tailChars);
    }

    return transcript;
}
