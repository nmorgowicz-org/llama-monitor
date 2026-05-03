// ── Chat Transport & Streaming ────────────────────────────────────────────────
// /api/chat calls, streaming decode, abort controller, summarization requests.

import { chat, lastLlamaMetrics } from '../core/app-state.js';
import {
    activeChatTab,
    substituteNames,
    scheduleChatPersist,
    setChatBusyUI,
    setTransportGetter,
    getChatViewBindings,
} from './chat-state.js';
import {
    renderChatMessages,
    appendAssistantPlaceholder,
    appendThinkingBlock,
    finalizeAssistantMessage,
    incrementUnreadCount,
    chatScroll,
    renderMd,
    renderMdStreaming,
    updateChatTabBadge,
    setChatTransportGetter,
} from './chat-render.js';
import { escapeHtml, formatMetricNumber } from '../core/format.js';
import { autoResizeChatInput } from './chat-state.js';
import { getExplicitModePolicy } from './chat-templates.js';
import { showToast, showToastWithActions } from './toast.js';

// ── Summarization ──────────────────────────────────────────────────────────────

export async function fetchSummary(messages) {
    // Derive transcript budget from the model's context window so the summarization
    // request never overflows (small models) and doesn't under-use capacity (large models).
    // 65% of context for the transcript; 35% reserved for system prompt + summary output.
    // 3.5 chars/token is a conservative estimate for mixed English/roleplay content.
    const capacityTokens =
        lastLlamaMetrics?.context_capacity_tokens ||
        lastLlamaMetrics?.kv_cache_max ||
        0;
    const MAX_TRANSCRIPT_CHARS = capacityTokens > 0
        ? Math.min(Math.floor(capacityTokens * 3.5 * 0.65), 500_000)
        : 140_000;

    const conversational = messages.filter(m => !m.compaction_marker);
    const rawTranscript = conversational
        .map(m => {
            const label = m.role === 'user' ? 'User' : 'Assistant';
            return `${label}: ${m.content}`;
        })
        .join('\n\n');

    // Smart truncation: preserve the first 25% (setup/persona/initial context) and the last 75%
    // (recent events). Slicing only the beginning discards the most important recent turns.
    let transcript = rawTranscript;
    if (rawTranscript.length > MAX_TRANSCRIPT_CHARS) {
        const headChars = Math.floor(MAX_TRANSCRIPT_CHARS * 0.25);
        const tailChars = MAX_TRANSCRIPT_CHARS - headChars;
        transcript =
            rawTranscript.slice(0, headChars) +
            '\n\n[... middle of conversation omitted for length ...]\n\n' +
            rawTranscript.slice(-tailChars);
    }

    const summaryMessages = [
        {
            role: 'system',
            content: 'You are a precise context-preservation assistant. Your job is to write a dense, structured summary of a conversation that will be injected as memory when the conversation resumes. Be specific — names, numbers, decisions, and unresolved threads matter. Output only the summary with no preamble or commentary.',
        },
        {
            role: 'user',
            content: `${transcript}\n\n---\n\nWrite a comprehensive memory summary of the conversation above. This summary will replace the conversation history, so it must contain everything needed to continue naturally.\n\nStructure your summary with these sections (omit any that don't apply):\n\n**Participants & Personas**\n- Names, roles, and established character traits (especially important for roleplay or character conversations)\n- Relationship dynamics between participants\n- Any user preferences, communication style, or stated constraints\n\n**Established Facts & Context**\n- Key facts, figures, technical details, or domain knowledge introduced\n- Decisions or conclusions that were agreed upon\n- Any world-building, setting, or scenario details (for creative/roleplay contexts)\n\n**Conversation Arc**\n- What was being discussed, created, or built\n- Major milestones or turning points in the conversation\n- How the most recent exchange ended (tone, content, where things stand)\n\n**Open Threads**\n- Unanswered questions or unresolved topics\n- In-progress tasks or ongoing narrative threads\n- Next steps or things the user said they'd do\n\nBe thorough. Err on the side of too much detail rather than too little — this summary is the only memory the assistant will have.`,
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
    if (chat.busy || chat.compactionInProgress) return;
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

    if (typeof renderChatMessages === 'function') renderChatMessages();

    _doSendChat(tab);
}

// Send a message that is already in tab.messages (for resend/regenerate — no duplicate push)
export async function sendChatResend(tab) {
    if (chat.busy || chat.compactionInProgress) return;

    if (typeof renderChatMessages === 'function') renderChatMessages();

    _doSendChat(tab);
}

export async function sendChat() {
    if (chat.busy || chat.compactionInProgress) return;
    const tab = activeChatTab();
    if (!tab) return;

    const input = document.getElementById('chat-input');
    const text = input.value.trim();
    if (!text) return;
    input.value = '';
    if (typeof autoResizeChatInput === 'function') autoResizeChatInput();

    const userMsg = {
        role: 'user',
        content: text,
        timestamp_ms: Date.now(),
    };
    tab.messages.push(userMsg);
    tab.updated_at = Date.now();

    if (typeof renderChatMessages === 'function') renderChatMessages();

    await _doSendChat(tab);
}

export async function _doSendChat(tab) {
    // Pre-send overflow guard: estimate token usage against current model capacity.
    // Uses the same formula as ctx%: cumulative output tokens + last input tokens.
    const capacity = lastLlamaMetrics?.context_capacity_tokens || lastLlamaMetrics?.kv_cache_max || 0;
    if (capacity > 0) {
        const asstMsgs = (tab.messages || []).filter(m => m.role === 'assistant' && !m.compaction_marker);
        const totalOutput = asstMsgs.reduce((sum, m) => sum + (m.output_tokens || 0), 0);
        const lastInput = asstMsgs.at(-1)?.input_tokens || 0;
        const estimatedTokens = totalOutput + lastInput;
        if (estimatedTokens > capacity) {
            const pct = Math.round((estimatedTokens / capacity) * 100);
            // Restore the user's message before showing the toast
            const lastMsg = tab.messages.at(-1);
            if (lastMsg?.role === 'user') {
                tab.messages.pop();
                const input = document.getElementById('chat-input');
                if (input) input.value = lastMsg.content;
                if (typeof autoResizeChatInput === 'function') autoResizeChatInput();
            }
            chat.busy = false;
            setChatBusyUI(false);
            const toast = showToastWithActions(
                'Context overflow',
                'warning',
                `Chat is ~${pct}% of the ${formatMetricNumber(capacity)}-token window. Compact first.`,
                [{
                    id: 'compact',
                    label: 'Compact now',
                    primary: true,
                    handler: () => {
                        document.getElementById('btn-compact')?.click();
                        toast?.remove();
                    },
                }]
            );
            return;
        }
    }

    const params = tab.model_params;
    const messages = [];
    let systemPrompt = tab.system_prompt ? substituteNames(tab.system_prompt, tab.ai_name, tab.user_name) : '';
    if (tab.explicit_mode) {
        const explicitPolicy = typeof getExplicitModePolicy === 'function'
            ? getExplicitModePolicy() : '';
        if (explicitPolicy) {
            systemPrompt += `\n\n${explicitPolicy}`;
        }
    }
    if (systemPrompt) {
        messages.push({ role: 'system', content: systemPrompt });
    }
    messages.push(...tab.messages.map(m => ({ role: m.role, content: m.content })));

    chat.busy = true;
    setChatBusyUI(true);
    chat.abortController = new AbortController();

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
            signal: chat.abortController.signal,
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
                chat.abortController.abort();
                if (!msgEl && typeof appendAssistantPlaceholder === 'function') {
                    msgEl = appendAssistantPlaceholder();
                }
                if (msgEl) {
                    // eslint-disable-next-line no-unsanitized/property -- LLM output rendered via marked.js in trusted local context; fallback span is hardcoded
                    msgEl.querySelector('.chat-msg-body').innerHTML =
                        msgContent ? (typeof renderMd === 'function' ? renderMd(msgContent) : msgContent)
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
                        if (!msgEl && typeof appendAssistantPlaceholder === 'function') {
                            msgEl = appendAssistantPlaceholder();
                        }
                        if (msgEl && !thinkEl && typeof appendThinkingBlock === 'function') {
                            thinkEl = appendThinkingBlock(msgEl);
                        }
                        if (thinkEl) {
                            thinkEl.querySelector('.chat-thinking-body').textContent = thinkContent;
                        }
                    }

                    const c = delta.content ?? '';
                    if (c) {
                        msgContent += c;
                        lastContentTime = Date.now();
                        const isFirstToken = !msgEl;
                        if (!msgEl && typeof appendAssistantPlaceholder === 'function') {
                            msgEl = appendAssistantPlaceholder();
                        }
                        if (msgEl) {
                            // eslint-disable-next-line no-unsanitized/property -- LLM output rendered via marked.js in trusted local context
                            msgEl.querySelector('.chat-msg-body').innerHTML =
                                typeof renderMdStreaming === 'function'
                                    ? renderMdStreaming(msgContent)
                                    : msgContent;
                        }
                        // Increment once per response (not per token) so badge = unread message count
                        if (isFirstToken && typeof incrementUnreadCount === 'function') incrementUnreadCount();
                    }
                } catch { /* malformed chunk — skip */ }
            }
            if (typeof chatScroll === 'function') chatScroll();
        }

    } catch (err) {
        if (!msgEl && typeof appendAssistantPlaceholder === 'function') {
            msgEl = appendAssistantPlaceholder();
        }
        if (msgEl) {
            const body = msgEl.querySelector('.chat-msg-body');
            if (err.name === 'AbortError') {
                // eslint-disable-next-line no-unsanitized/property -- LLM output rendered via marked.js in trusted local context; fallback span is hardcoded
                body.innerHTML = msgContent
                    ? (typeof renderMd === 'function' ? renderMd(msgContent) : msgContent)
                    : '<span class="chat-stopped">[stopped]</span>';
            } else {
                body.innerHTML = `<span class="chat-error">[error] ${escapeHtml(err.message)}</span>`;
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
    if (typeof finalizeAssistantMessage === 'function') {
        finalizeAssistantMessage(msgEl, msgContent, tokenUsage, tab);
    }
    setChatBusyUI(false);
    chat.busy = false;
    chat.abortController = null;
    if (typeof updateChatTabBadge === 'function') updateChatTabBadge();

    // Trigger auto-compact if the tab has it enabled and the threshold was hit.
    // Runs after busy is cleared so compaction can proceed without being blocked.
    getChatViewBindings().checkAutoCompact?.(tab);
}

// ── Stop Chat ──────────────────────────────────────────────────────────────────

export function stopChat() {
    if (chat.abortController) {
        chat.abortController.abort();
        chat.abortController = null;
    }
    chat.busy = false;
    setChatBusyUI(false);
}

// ── Init ───────────────────────────────────────────────────────────────────────

export function initChatTransport() {
    // Wire up transport getter for chat-state and chat-render (avoids circular import)
    const transport = () => ({ sendChat, sendChatResend, sendSuggestedPrompt, stopChat });
    setTransportGetter(transport);
    setChatTransportGetter(transport);
}
