// Chat-panel interaction and wait helpers used by chat-driving scenarios.
// Extracted from tests/ui/capture.mjs (Phase A1).
import { switchTab } from './browser.mjs';
import { SCREENSHOT_TAB_PREFIX, sleep } from './paths.mjs';
import { cleanupScreenshotTabs } from './shot.mjs';

export async function clearExistingChats(page) {
    await page.evaluate(() => {
        document.getElementById('btn-clear')?.click();
    });
    await sleep(200);
}

export async function createFreshChat(page) {
    await switchTab(page, 'chat');
    await sleep(500);

    await cleanupScreenshotTabs(page);
    await page.evaluate(async () => {
        const { addChatTab } = await import('/js/features/chat-state.js');
        const { renderChatTabs, renderChatMessages } = await import('/js/features/chat-render.js');
        addChatTab();
        renderChatTabs();
        renderChatMessages();
    });
    await sleep(300);

    await page.evaluate(async prefix => {
        const { chat } = await import('/js/core/app-state.js');
        const { persistChatTabs } = await import('/js/features/chat-state.js');
        const { renderChatTabs, renderChatMessages } = await import('/js/features/chat-render.js');

        const activeTab = chat.tabs.find(tab => tab.id === chat.activeTabId);
        if (!activeTab) return;
        activeTab.name = `${prefix} Chat`;
        activeTab.messages = [];
        activeTab.updated_at = Date.now();
        renderChatTabs();
        renderChatMessages();
        await persistChatTabs();
    }, SCREENSHOT_TAB_PREFIX);

    await clearExistingChats(page);
    await sleep(300);
}

export async function activateScreenshotChat(page) {
    await page.evaluate(prefix => {
        const items = Array.from(document.querySelectorAll('.csp-item'));
        const target = items.find(el => {
            const name = el.querySelector('.csp-item-name')?.textContent || '';
            return name.includes(prefix);
        });
        target?.click();
    }, SCREENSHOT_TAB_PREFIX);
    await sleep(500);
}

export async function sendChatPrompt(page, prompt) {
    // Check if chat input is visible
    const chatInput = await page.$('#chat-input');
    if (!chatInput) {
        console.log('[CAPTURE] Chat input not found!');
        throw new Error('Chat input not found');
    }
    const inputVisible = await chatInput.evaluate(el => getComputedStyle(el).display !== 'none');
    console.log('[CAPTURE] Chat input visible:', inputVisible);
    if (!inputVisible) {
        console.log('[CAPTURE] Chat input is not visible, trying to scroll into view...');
        await chatInput.evaluate(el => el.scrollIntoView({ behavior: 'instant', block: 'center' }));
        await sleep(500);
    }
    await page.$eval('#chat-input', (input, text) => {
        input.value = text;
        input.dispatchEvent(new Event('input', { bubbles: true }));
        input.dispatchEvent(new Event('change', { bubbles: true }));
    }, prompt);
    // Use JavaScript to click the send button (puppeteer click may fail without proper event handling)
    await page.evaluate(() => {
        const sendBtn = document.getElementById('btn-send');
        if (sendBtn) sendBtn.click();
    });
    await page.waitForFunction(() => {
        return document.querySelectorAll('#chat-messages .chat-message-user').length > 0;
    }, { timeout: 10000 });
}

export async function logChatState(page, label) {
    const state = await page.evaluate(() => {
        const { chat } = window;
        const streaming = document.querySelector('#chat-messages .chat-message-streaming');
        const assistantMessages = document.querySelectorAll('#chat-messages .chat-message-assistant');
        const lastAssistant = assistantMessages[assistantMessages.length - 1];
        const lastBody = lastAssistant?.querySelector('.chat-msg-body');
        const sendBtn = document.getElementById('btn-send');
        return {
            chatBusy: chat?.busy,
            streamingElement: !!streaming,
            assistantCount: assistantMessages.length,
            lastMessageLength: lastBody?.textContent?.length ?? 0,
            sendBtnClass: sendBtn?.className ?? null,
        };
    });
    console.log(`[CAPTURE] ${label}:`, JSON.stringify(state));
    return state;
}

export async function waitForChatResponse(page, timeoutMs = 300000) {
    const start = Date.now();
    console.log('[CAPTURE] waitForChatResponse: waiting for chat response...');
    
    await logChatState(page, 'waitForChatResponse:BEFORE');
    
    await page.waitForFunction(() => {
        // Check chat.busy state directly - this is the authoritative source
        const { chat } = window;
        if (chat?.busy) return false;
        
        // Also check UI state as fallback
        const streaming = document.querySelector('#chat-messages .chat-message-streaming');
        if (streaming) return false;
        const sendBtn = document.getElementById('btn-send');
        if (sendBtn && sendBtn.classList.contains('btn-chat-send-stop')) return false;
        const assistantMessages = Array.from(document.querySelectorAll('#chat-messages .chat-message-assistant'));
        return assistantMessages.length > 0;
    }, { timeout: timeoutMs });
    
    // Increased buffer to ensure AI has fully completed
    await sleep(10000);
    
    await logChatState(page, 'waitForChatResponse:AFTER');
    
    const elapsed = Date.now() - start;
    console.log(`[CAPTURE] waitForChatResponse: completed in ${elapsed}ms`);
}

export async function waitForChatIdle(page, timeoutMs = 120000) {
    const start = Date.now();
    console.log('[CAPTURE] waitForChatIdle: waiting for chat to become idle...');
    
    await logChatState(page, 'waitForChatIdle:BEFORE');
    
    // Wait for chat to become idle (no streaming, send button not in stop mode)
    await page.waitForFunction(() => {
        // Check chat.busy state directly - this is the authoritative source
        const { chat } = window;
        if (chat?.busy) return false;
        
        const streaming = document.querySelector('#chat-messages .chat-message-streaming');
        if (streaming) return false;
        const sendBtn = document.getElementById('btn-send');
        if (sendBtn && sendBtn.classList.contains('btn-chat-send-stop')) return false;
        return true;
    }, { timeout: timeoutMs });
    
    // Verify idle state is stable (wait 5s to ensure no new streaming starts)
    for (let i = 0; i < 5; i++) {
        await sleep(1000);
        const isIdle = await page.evaluate(() => {
            const { chat } = window;
            if (chat?.busy) return false;
            const streaming = document.querySelector('#chat-messages .chat-message-streaming');
            if (streaming) return false;
            const sendBtn = document.getElementById('btn-send');
            if (sendBtn && sendBtn.classList.contains('btn-chat-send-stop')) return false;
            return true;
        });
        if (!isIdle) {
            console.log('[CAPTURE] waitForChatIdle: chat became busy again, waiting...');
            i = -1; // Reset counter and start over
        }
    }
    
    await logChatState(page, 'waitForChatIdle:AFTER');
    
    const elapsed = Date.now() - start;
    console.log(`[CAPTURE] waitForChatIdle: completed in ${elapsed}ms`);
}

export async function waitForChatComplete(page, timeoutMs = 300000) {
    const start = Date.now();
    console.log('[CAPTURE] waitForChatComplete: waiting for chat to complete...');
    
    await logChatState(page, 'waitForChatComplete:BEFORE');
    
    // Wait for streaming to stop and assistant message to appear
    await waitForChatResponse(page, timeoutMs);

    // Retry until no [stopped] text in the last assistant message (max 3 retries)
    for (let i = 0; i < 3; i++) {
        const hasStopped = await page.evaluate(() => {
            const assistantMessages = document.querySelectorAll('#chat-messages .chat-message-assistant');
            if (assistantMessages.length === 0) return false;
            const lastMessage = assistantMessages[assistantMessages.length - 1];
            const body = lastMessage.querySelector('.chat-msg-body');
            return body && body.textContent.includes('[stopped]');
        });

        if (!hasStopped) break;

        console.log(`[CAPTURE] Detected [stopped] response, waiting longer (attempt ${i + 1}/3)...`);
        await sleep(10000);
    }

    // Final check
    const stillStopped = await page.evaluate(() => {
        const assistantMessages = document.querySelectorAll('#chat-messages .chat-message-assistant');
        if (assistantMessages.length === 0) return false;
        const lastMessage = assistantMessages[assistantMessages.length - 1];
        const body = lastMessage.querySelector('.chat-msg-body');
        return body && body.textContent.includes('[stopped]');
    });

    if (stillStopped) {
        console.log('[CAPTURE] WARNING: [stopped] response persists after retries, may need manual review');
    }
    
    await logChatState(page, 'waitForChatComplete:AFTER');
    
    const elapsed = Date.now() - start;
    console.log(`[CAPTURE] waitForChatComplete: completed in ${elapsed}ms`);
}

export async function waitForChatSettledOrError(page, timeoutMs = 300000) {
    await page.waitForFunction(() => {
        const streaming = document.querySelector('#chat-messages .chat-message-streaming');
        if (streaming) return false;
        const error = document.querySelector('#chat-messages .chat-error');
        if (error) return true;
        const assistantMessages = Array.from(document.querySelectorAll('#chat-messages .chat-message-assistant'));
        return assistantMessages.length > 1;
    }, { timeout: timeoutMs });
}

export async function waitForSuggestionsSettled(page, timeoutMs = 300000) {
    await page.waitForFunction(() => {
        const dropdown = document.getElementById('suggestions-dropdown');
        const list = document.getElementById('suggestions-list');
        if (!dropdown || !list) return false;
        const isLoading = list.querySelector('.suggestions-loading');
        if (isLoading) return false;
        const hasItems = list.querySelectorAll('.suggestion-item').length > 0;
        const hasEmpty = !!list.querySelector('.suggestions-empty-state');
        const collapsed = dropdown.classList.contains('setup-collapsed');
        return hasItems || hasEmpty || !collapsed;
    }, { timeout: timeoutMs });
}

export async function describeSuggestionsPanel(page) {
    return page.evaluate(() => {
        const dropdown = document.getElementById('suggestions-dropdown');
        const list = document.getElementById('suggestions-list');
        const toggle = document.getElementById('suggestions-view-toggle');
        const generate = document.getElementById('suggestions-generate-btn');
        const tagCloud = dropdown?.querySelector('.suggestions-tag-cloud');
        return {
            expanded: dropdown?.classList.contains('dropdown-expanded') ?? false,
            setupCollapsed: dropdown?.classList.contains('setup-collapsed') ?? false,
            toggleLabel: toggle?.textContent?.trim() ?? null,
            generateVisible: !!generate && getComputedStyle(generate).display !== 'none',
            tagCloudVisible: !!tagCloud && getComputedStyle(tagCloud).display !== 'none',
            suggestionCount: list?.querySelectorAll('.suggestion-item').length ?? 0,
            emptyStateText: list?.querySelector('.suggestions-empty-state p')?.textContent?.trim() ?? null,
            loading: !!list?.querySelector('.suggestions-loading'),
        };
    });
}

export function attachSuggestionsResponseLogger(page) {
    const handler = async response => {
        if (!response.url().includes('/api/chat/suggestions')) return;
        try {
            const payload = await response.text();
            const condensed = payload.replace(/\s+/g, ' ').slice(0, 3000);
            console.log(`[CAPTURE] Suggestions API ${response.status()}: ${condensed}`);
        } catch (error) {
            console.log(`[CAPTURE] Suggestions API response logging failed: ${error.message}`);
        }
    };
    page.on('response', handler);
    return () => page.off('response', handler);
}
