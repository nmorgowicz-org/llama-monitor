// ── Chat Transport & Streaming ────────────────────────────────────────────────
// /api/chat calls, streaming decode, abort controller, summarization requests.

import { activeChatTab, substituteNames, scheduleChatPersist, setChatBusyUI } from './chat-state.js';

// ── Summarization ──────────────────────────────────────────────────────────────

export async function fetchSummary(messages) {
    const MAX_TRANSCRIPT_CHARS = 100_000;
    let transcript = messages
        .filter(m => !m.compaction_marker)
        .map(m => {
            const label = m.role === 'user' ? 'User' : 'Assistant';
            return `${label}: ${m.content}`;
        })
        .join('\n\n');
    if (transcript.length > MAX_TRANSCRIPT_CHARS) {
        transcript = transcript.slice(0, MAX_TRANSCRIPT_CHARS) + '\n\n[transcript truncated for length]';
    }

    const summaryMessages = [
        {
            role: 'system',
            content: 'Your task is to create a detailed summary of the conversation so far, paying close attention to the user\'s explicit requests and your previous responses. You will be given the conversation to summarize, and you should output your response directly without any preamble.',
        },
        {
            role: 'user',
            content: `${transcript}\n\nPlease provide a detailed summary of our conversation above. The summary will be used to restore context when the conversation continues, so it must capture everything needed to pick up exactly where we left off.\n\nInclude:\n- The main topics discussed and the purpose of the conversation\n- Key facts, figures, decisions, or conclusions established\n- Specific requests made and whether/how they were fulfilled\n- Any open questions, unresolved issues, or next steps mentioned\n- Important context, constraints, or preferences expressed by either party\n- The current state of any ongoing task or project\n\nWrite in past tense. Be thorough — omit nothing that would affect how the conversation continues.`,
        },
    ];

    try {
        const resp = await fetch('/api/chat', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ messages: summaryMessages, stream: true }),
        });

        if (!resp.ok) return null;

        const reader = resp.body.getReader();
        const decoder = new TextDecoder();
        let buf = '';
        let summary = '';

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
                    const obj = JSON.parse(payload);
                    const delta = obj.choices?.[0]?.delta;
                    if (delta?.content) summary += delta.content;
                } catch { /* skip malformed chunks */ }
            }
        }

        return summary.trim() || null;
    } catch {
        return null;
    }
}

// ── Suggested Prompts ─────────────────────────────────────────────────────────

export async function sendSuggestedPrompt(text) {
    const input = document.getElementById('chat-input');
    if (input) input.value = text;
    sendChat();
}

// ── Send Chat ──────────────────────────────────────────────────────────────────

export async function sendChatWithContent(text) {
    if (window.chatBusy || window.compactionInProgress) return;
    const tab = activeChatTab();
    if (!tab) return;

    text = text.trim();
    if (!text) return;

    const userMsg = {
        role: 'user',
        content: text,
        timestamp_ms: Date.now(),
    };
    tab.messages.push(userMsg);
    tab.updated_at = Date.now();

    if (typeof window.renderChatMessages === 'function') window.renderChatMessages();

    _doSendChat(tab);
}

// Send a message that is already in tab.messages (for resend/regenerate — no duplicate push)
export async function sendChatResend(tab) {
    if (window.chatBusy || window.compactionInProgress) return;

    if (typeof window.renderChatMessages === 'function') window.renderChatMessages();

    _doSendChat(tab);
}

export async function sendChat() {
    if (window.chatBusy || window.compactionInProgress) return;
    const tab = activeChatTab();
    if (!tab) return;

    const input = document.getElementById('chat-input');
    const text = input.value.trim();
    if (!text) return;
    input.value = '';
    if (typeof window.autoResizeChatInput === 'function') window.autoResizeChatInput();

    const userMsg = {
        role: 'user',
        content: text,
        timestamp_ms: Date.now(),
    };
    tab.messages.push(userMsg);
    tab.updated_at = Date.now();

    if (typeof window.renderChatMessages === 'function') window.renderChatMessages();

    await _doSendChat(tab);
}

export async function _doSendChat(tab) {
    const params = tab.model_params;
    const messages = [];
    let systemPrompt = tab.system_prompt ? substituteNames(tab.system_prompt, tab.ai_name, tab.user_name) : '';
    if (tab.explicit_mode) {
        const explicitPolicy = typeof window.getExplicitModePolicy === 'function'
            ? window.getExplicitModePolicy() : '';
        if (explicitPolicy) {
            systemPrompt += `\n\n${explicitPolicy}`;
        }
    }
    if (systemPrompt) {
        messages.push({ role: 'system', content: systemPrompt });
    }
    messages.push(...tab.messages.map(m => ({ role: m.role, content: m.content })));

    window.chatBusy = true;
    setChatBusyUI(true);
    window.chatAbortController = new AbortController();

    let thinkEl = null;
    let thinkContent = '';
    let msgEl = null;
    let msgContent = '';
    let tokenUsage = null;
    const streamTimeoutMs = (params.stream_timeout ?? 120) * 1000;
    let lastContentTime = Date.now();

    try {
        const chatResp = await fetch('/api/chat', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            signal: window.chatAbortController.signal,
            body: JSON.stringify({
                messages,
                stream: true,
                temperature: params.temperature,
                top_p: params.top_p,
                top_k: params.top_k,
                min_p: params.min_p,
                repeat_penalty: params.repeat_penalty,
                ...(params.max_tokens ? { max_tokens: params.max_tokens } : {}),
            }),
        });

        if (!chatResp.ok) {
            const errText = await chatResp.text().catch(() => '');
            throw new Error(`HTTP ${chatResp.status}: ${errText}`);
        }

        const reader = chatResp.body.getReader();
        const decoder = new TextDecoder();
        let buf = '';

        while (true) {
            if (streamTimeoutMs > 0 && Date.now() - lastContentTime > streamTimeoutMs) {
                window.chatAbortController.abort();
                if (!msgEl && typeof window.appendAssistantPlaceholder === 'function') {
                    msgEl = window.appendAssistantPlaceholder();
                }
                if (msgEl) {
                    msgEl.querySelector('.chat-msg-body').innerHTML =
                        msgContent ? (typeof window.renderMd === 'function' ? window.renderMd(msgContent) : msgContent)
                            : '<span class="chat-stopped">[timed out — no response for too long]</span>';
                }
                break;
            }

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
                    const obj = JSON.parse(payload);

                    if (obj.usage) {
                        tokenUsage = obj.usage;
                        continue;
                    }
                    if (obj.timings && obj.choices?.[0]?.finish_reason) {
                        tokenUsage = {
                            prompt_tokens: obj.timings.prompt_n || 0,
                            completion_tokens: obj.timings.predicted_n || 0,
                        };
                        continue;
                    }

                    const delta = obj.choices?.[0]?.delta;
                    if (!delta) continue;

                    const rc = delta.reasoning_content ?? '';
                    if (rc) {
                        thinkContent += rc;
                        if (!msgEl && typeof window.appendAssistantPlaceholder === 'function') {
                            msgEl = window.appendAssistantPlaceholder();
                        }
                        if (msgEl && !thinkEl && typeof window.appendThinkingBlock === 'function') {
                            thinkEl = window.appendThinkingBlock(msgEl);
                        }
                        if (thinkEl) {
                            thinkEl.querySelector('.chat-thinking-body').textContent = thinkContent;
                        }
                    }

                    const c = delta.content ?? '';
                    if (c) {
                        msgContent += c;
                        lastContentTime = Date.now();
                        if (!msgEl && typeof window.appendAssistantPlaceholder === 'function') {
                            msgEl = window.appendAssistantPlaceholder();
                        }
                        if (msgEl) {
                            msgEl.querySelector('.chat-msg-body').innerHTML =
                                typeof window.renderMdStreaming === 'function'
                                    ? window.renderMdStreaming(msgContent)
                                    : msgContent;
                        }
                        if (typeof window.incrementUnreadCount === 'function') window.incrementUnreadCount();
                    }
                } catch { /* malformed chunk — skip */ }
            }
            if (typeof window.chatScroll === 'function') window.chatScroll();
        }

    } catch (err) {
        if (!msgEl && typeof window.appendAssistantPlaceholder === 'function') {
            msgEl = window.appendAssistantPlaceholder();
        }
        if (msgEl) {
            const body = msgEl.querySelector('.chat-msg-body');
            if (err.name === 'AbortError') {
                body.innerHTML = msgContent
                    ? (typeof window.renderMd === 'function' ? window.renderMd(msgContent) : msgContent)
                    : '<span class="chat-stopped">[stopped]</span>';
            } else {
                body.innerHTML = `<span class="chat-error">[error] ${window.escapeHtml(err.message)}</span>`;
            }
        }
    }

    if (msgContent) {
        const inp = tokenUsage ? (tokenUsage.prompt_tokens ?? 0) : 0;
        const out = tokenUsage ? (tokenUsage.completion_tokens ?? 0) : 0;
        tab.totalInputTokens = (tab.totalInputTokens || 0) + inp;
        tab.totalOutputTokens = (tab.totalOutputTokens || 0) + out;
        tab.messages.push({
            role: 'assistant',
            content: msgContent,
            timestamp_ms: Date.now(),
            input_tokens: inp,
            output_tokens: out,
            cumulativeInputTokens: tab.totalInputTokens,
            cumulativeOutputTokens: tab.totalOutputTokens,
        });
        tab.updated_at = Date.now();
        scheduleChatPersist();
    } else if (!tab.messages.at(-1)?.content) {
        tab.messages.pop();
    }

    if (msgEl) {
        msgEl.dataset.msgIdx = tab.messages.length - 1;
    }
    if (typeof window.finalizeAssistantMessage === 'function') {
        window.finalizeAssistantMessage(msgEl, msgContent, tokenUsage, tab);
    }
    setChatBusyUI(false);
    window.chatBusy = false;
    window.chatAbortController = null;
    if (typeof window.updateChatTabBadge === 'function') window.updateChatTabBadge();
}

// ── Stop Chat ──────────────────────────────────────────────────────────────────

export function stopChat() {
    if (window.chatAbortController) {
        window.chatAbortController.abort();
        window.chatAbortController = null;
    }
    window.chatBusy = false;
    setChatBusyUI(false);
}

// ── Init ───────────────────────────────────────────────────────────────────────

export function initChatTransport() {
    // Put on window for cross-module calls
    window.sendChat = sendChat;
    window.sendChatResend = sendChatResend;
    window.sendSuggestedPrompt = sendSuggestedPrompt;
    window.stopChat = stopChat;
    window.fetchSummary = fetchSummary;
}
