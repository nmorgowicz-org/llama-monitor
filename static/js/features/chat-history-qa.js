// ── Chat History Q&A ──────────────────────────────────────────────────────────
// Slide-in panel for asking natural language questions about the active
// tab's conversation history. Thread state is ephemeral (per session, per tab)
// and is never persisted to the server.

import { activeChatTab } from './chat-state.js';
import { renderMd, renderMdStreaming } from './chat-render.js';
import { escapeHtml } from '../core/format.js';

// ── Constants ─────────────────────────────────────────────────────────────────

const PANEL_ID        = 'chat-history-qa-panel';
const BTN_ID          = 'chat-history-qa-btn';
const THREAD_ID       = 'chqa-thread';
const EMPTY_ID        = 'chqa-empty';
const INPUT_ID        = 'chqa-input';
const SEND_BTN_ID     = 'chqa-send-btn';
const STOP_BTN_ID     = 'chqa-stop-btn';
const CLOSE_BTN_ID    = 'chqa-close-btn';
const CLEAR_BTN_ID    = 'chqa-clear-btn';
const CONTEXT_LBL_ID  = 'chqa-context-label';

const MAX_TRANSCRIPT_CHARS = 120_000;

const SUGGESTIONS = [
    'What has happened in this conversation so far?',
    'Who are the main characters and what are their goals?',
    'What important decisions or events have taken place?',
    'What storylines or questions are still unresolved?',
];

// ── Module state ──────────────────────────────────────────────────────────────

// Ephemeral Q&A thread storage: Map<tabId, QAEntry[]>
// QAEntry: { id, question, answer, streaming, error }
const threads = new Map();

let abortController = null;
let streaming = false;
let renderedTabId = null;

// ── Public API ────────────────────────────────────────────────────────────────

export function initChatHistoryQA() {
    document.getElementById(BTN_ID)?.addEventListener('click', toggleHistoryQAPanel);
    document.getElementById(CLOSE_BTN_ID)?.addEventListener('click', closeHistoryQAPanel);
    document.getElementById(CLEAR_BTN_ID)?.addEventListener('click', clearThread);
    document.getElementById(SEND_BTN_ID)?.addEventListener('click', handleSend);
    document.getElementById(STOP_BTN_ID)?.addEventListener('click', handleStop);

    const input = document.getElementById(INPUT_ID);
    if (input) {
        input.addEventListener('keydown', e => {
            if (e.key === 'Enter' && !e.shiftKey) {
                e.preventDefault();
                handleSend();
            }
        });
        input.addEventListener('input', () => autoResizeInput(input));
    }

    renderSuggestions();
}

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
    const btn = document.getElementById(BTN_ID);
    if (!panel) return;

    // Close conflicting panels
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
    const btn = document.getElementById(BTN_ID);
    panel?.classList.remove('open');
    btn?.classList.remove('active');
    btn?.setAttribute('aria-pressed', 'false');
}

// ── Tab refresh ───────────────────────────────────────────────────────────────

function refreshForCurrentTab() {
    const tab = activeChatTab();
    const tabId = tab?.id ?? null;

    const msgCount = countVisibleMessages(tab);
    const labelEl = document.getElementById(CONTEXT_LBL_ID);
    if (labelEl) {
        labelEl.textContent = msgCount > 0 ? `${msgCount} msg${msgCount !== 1 ? 's' : ''}` : 'no history';
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

// ── Thread rendering ──────────────────────────────────────────────────────────

function renderThread(tabId) {
    const threadEl = document.getElementById(THREAD_ID);
    const emptyEl  = document.getElementById(EMPTY_ID);
    if (!threadEl) return;

    // Remove existing entry elements, keep the static empty-state node
    threadEl.querySelectorAll('.chqa-entry').forEach(el => el.remove());

    const entries = tabId ? (threads.get(tabId) ?? []) : [];
    if (entries.length === 0) {
        emptyEl?.classList.remove('chqa-hidden');
    } else {
        emptyEl?.classList.add('chqa-hidden');
        for (const entry of entries) {
            threadEl.appendChild(buildEntryEl(entry));
        }
        scrollThreadToBottom(threadEl);
    }
}

function buildEntryEl(entry) {
    const div = document.createElement('div');
    div.className = 'chqa-entry';
    div.dataset.entryId = entry.id;

    const qEl = document.createElement('div');
    qEl.className = 'chqa-question';
    qEl.textContent = entry.question;
    div.appendChild(qEl);

    const aEl = document.createElement('div');
    aEl.className = 'chqa-answer' + (entry.streaming ? ' streaming' : '');
    const bodyEl = document.createElement('div');
    bodyEl.className = 'chqa-answer-body';
    setAnswerBody(bodyEl, entry);
    aEl.appendChild(bodyEl);
    div.appendChild(aEl);

    return div;
}

function setAnswerBody(bodyEl, entry) {
    if (entry.streaming && !entry.answer) {
        bodyEl.innerHTML = '<div class="chqa-thinking-dots"><span></span><span></span><span></span></div>';
        return;
    }
    if (entry.error) {
        bodyEl.innerHTML = `<span class="chqa-answer-error">${escapeHtml(entry.answer || 'Something went wrong.')}</span>`;
        return;
    }
    if (entry.answer) {
        // eslint-disable-next-line no-unsanitized/property -- answer from local LLM, rendered via marked+DOMPurify
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
        // eslint-disable-next-line no-unsanitized/property -- local LLM, rendered via marked+DOMPurify
        bodyEl.innerHTML = renderMd(answer);
    } else if (answer) {
        // eslint-disable-next-line no-unsanitized/property -- local LLM, rendered via marked+DOMPurify
        bodyEl.innerHTML = renderMdStreaming(answer);
    }

    const threadEl = document.getElementById(THREAD_ID);
    if (threadEl) scrollThreadToBottom(threadEl);
}

function scrollThreadToBottom(threadEl) {
    threadEl.scrollTop = threadEl.scrollHeight;
}

// ── Input helpers ─────────────────────────────────────────────────────────────

function autoResizeInput(el) {
    el.style.height = 'auto';
    el.style.height = Math.min(el.scrollHeight, 120) + 'px';
}

function renderSuggestions() {
    const container = document.getElementById('chqa-suggestions');
    if (!container) return;
    container.innerHTML = '';
    for (const text of SUGGESTIONS) {
        const btn = document.createElement('button');
        btn.className = 'chqa-suggestion-btn';
        btn.type = 'button';
        btn.textContent = text;
        btn.addEventListener('click', () => {
            const input = document.getElementById(INPUT_ID);
            if (input && !streaming) {
                input.value = text;
                autoResizeInput(input);
                handleSend();
            }
        });
        container.appendChild(btn);
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
    autoResizeInput(input);

    const visibleCount = countVisibleMessages(tab);
    const hasCompaction = (tab.messages || []).some(m => m.compaction_marker && m.content?.trim());

    if (visibleCount === 0 && !hasCompaction) {
        addEntry(tab.id, {
            question: text,
            answer: 'This conversation has no history yet. Start chatting first, then come back to ask questions about what happened.',
            streaming: false,
            error: false,
        });
        return;
    }

    await submitQuestion(tab, text);
}

function handleStop() {
    if (abortController) {
        abortController.abort();
        abortController = null;
    }
}

// ── Thread state management ───────────────────────────────────────────────────

function addEntry(tabId, entryData) {
    if (!threads.has(tabId)) threads.set(tabId, []);
    const entries = threads.get(tabId);
    const id = `chqa-${Date.now()}-${Math.random().toString(36).slice(2, 6)}`;
    const entry = { id, ...entryData };
    entries.push(entry);

    const threadEl = document.getElementById(THREAD_ID);
    const emptyEl  = document.getElementById(EMPTY_ID);
    if (!threadEl) return entry;

    emptyEl?.classList.add('chqa-hidden');
    threadEl.appendChild(buildEntryEl(entry));
    scrollThreadToBottom(threadEl);
    return entry;
}

function clearThread() {
    const tab = activeChatTab();
    if (!tab) return;
    threads.delete(tab.id);
    renderedTabId = null;
    refreshForCurrentTab();
}

// ── Streaming Q&A call ────────────────────────────────────────────────────────

async function submitQuestion(tab, question) {
    const entry = addEntry(tab.id, { question, answer: '', streaming: true, error: false });
    setStreamingUI(true);

    const transcript = buildHistoryTranscript(tab);

    const systemPrompt =
        'You are a precise conversation analyst. You have been given the full transcript of an ongoing chat.\n\n' +
        'Answer the user\'s questions about what happened in this conversation — events, characters, decisions, ' +
        'plot points, or any other details. Be accurate and specific, citing relevant exchanges when helpful. ' +
        'If something didn\'t happen or isn\'t in the transcript, say so clearly.\n\n' +
        'Keep answers focused and concise. Do not continue the story, roleplay, or generate new content — ' +
        'only report on what is already in the transcript.';

    const messages = [
        { role: 'system', content: systemPrompt },
        {
            role: 'user',
            content: `Here is the conversation transcript:\n\n${transcript}\n\n---\n\nQuestion: ${question}`,
        },
    ];

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
            if (!answer) {
                answer = '[Stopped]';
                failed = true;
            }
        } else {
            answer = `Error: ${err.message}`;
            failed = true;
        }
    }

    // Finalize
    entry.answer    = answer;
    entry.streaming = false;
    entry.error     = failed && answer !== '[Stopped]';
    updateEntryDOM(entry.id, answer, true, entry.error);

    abortController = null;
    setStreamingUI(false);
    document.getElementById(INPUT_ID)?.focus();
}

function setStreamingUI(isStreaming) {
    streaming = isStreaming;
    const sendBtn = document.getElementById(SEND_BTN_ID);
    const stopBtn = document.getElementById(STOP_BTN_ID);
    const input   = document.getElementById(INPUT_ID);
    if (sendBtn) sendBtn.disabled = isStreaming;
    if (stopBtn) stopBtn.classList.toggle('chqa-hidden', !isStreaming);
    if (input)   input.disabled  = isStreaming;
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
