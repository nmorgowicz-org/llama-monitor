// ── Fix Last Response ────────────────────────────────────────────────────────
// Regenerate last assistant message with correction instruction.

import { activeChatTab, saveChatTabs } from './chat-state.js';
import { showToast } from './toast.js';
import { sendChatMessage } from './chat-transport.js';

let fixLastState = {
    isOpen: false,
    instruction: '',
};

// ── Modal Control ────────────────────────────────────────────────────────────

export function openFixLastModal() {
    const modal = document.getElementById('fix-last-modal');
    const input = document.getElementById('fix-last-instruction');

    if (!modal || !input) return;

    // Check if there's a last assistant message to fix
    const tab = activeChatTab();
    if (!tab || tab.messages.length === 0) {
        showToast('No messages to fix', 'error');
        return;
    }

    const lastMsg = tab.messages[tab.messages.length - 1];
    if (lastMsg.role !== 'assistant') {
        showToast('Last message is not from assistant', 'error');
        return;
    }

    fixLastState.isOpen = true;
    fixLastState.instruction = '';
    input.value = '';

    modal.removeAttribute('aria-hidden');
    modal.inert = false;
    modal.classList.add('open');
    input.focus();
}

export function closeFixLastModal() {
    const modal = document.getElementById('fix-last-modal');
    if (!modal) return;

    fixLastState.isOpen = false;
    modal.classList.add('closing');
    setTimeout(() => {
        modal.classList.remove('open', 'closing');
        modal.setAttribute('aria-hidden', 'true');
        modal.inert = true;
    }, 260);
}

// ── Regenerate with Fix ──────────────────────────────────────────────────────

async function regenerateWithFix() {
    const instruction = fixLastState.instruction.trim();
    if (!instruction) {
        showToast('Please enter a correction', 'error');
        return;
    }

    const tab = activeChatTab();
    if (!tab) return;

    // Remove last assistant message
    const lastMsg = tab.messages.pop();
    if (!lastMsg || lastMsg.role !== 'assistant') {
        showToast('No assistant message to fix', 'error');
        return;
    }

    // Store the original user message that prompted this response
    const userMsgIndex = tab.messages.length - 1;
    if (tab.messages[userMsgIndex]?.role !== 'user') {
        // Restore and show error
        tab.messages.push(lastMsg);
        showToast('Could not find original user message', 'error');
        return;
    }

    // Add instruction as a system message
    const instructionMsg = {
        role: 'system',
        content: `### CORRECTION ###\n\nPlease regenerate your previous response with this correction: ${instruction}`,
        timestamp_ms: Date.now(),
        _fix_instruction: instruction,
    };

    // Temporarily add instruction message
    tab.messages.push(instructionMsg);

    // Send the user message again with the correction context
    const userMsg = tab.messages[userMsgIndex];
    const input = document.getElementById('chat-input');

    // Clear input and show loading
    if (input) input.value = '';

    // Mark instruction as used so it's removed after regeneration
    instructionMsg._pending_fix = true;

    // Save and regenerate
    await saveChatTabs();

    // Trigger regeneration by "sending" the user message again
    // This will include the correction instruction in the context
    const originalContent = userMsg.content;
    userMsg.content = `[With correction: ${instruction}] ${originalContent}`;

    // Use chat transport to send
    await sendChatMessage();

    // Cleanup: remove the temporary instruction message
    tab.messages.pop(); // Remove instructionMsg
    // Restore original user message content
    tab.messages[userMsgIndex].content = originalContent;

    await saveChatTabs();

    closeFixLastModal();
    showToast('Regenerating with correction...', 'success');
}

// ── Event Handlers ───────────────────────────────────────────────────────────

function setupFixLastButton() {
    const btn = document.getElementById('btn-fix-last');
    if (!btn) return;

    btn.addEventListener('click', openFixLastModal);
}

function setupModalClose() {
    const closeBtn = document.getElementById('fix-last-modal-close');
    const cancelBtn = document.getElementById('fix-last-cancel');

    closeBtn?.addEventListener('click', closeFixLastModal);
    cancelBtn?.addEventListener('click', closeFixLastModal);

    // Regenerate button
    const regenBtn = document.getElementById('fix-last-regenerate');
    regenBtn?.addEventListener('click', regenerateWithFix);

    // Input change tracking
    const input = document.getElementById('fix-last-instruction');
    input?.addEventListener('input', (e) => {
        fixLastState.instruction = e.target.value;
    });

    // Keyboard shortcuts
    input?.addEventListener('keydown', (e) => {
        if (e.key === 'Enter' && e.ctrlKey) {
            e.preventDefault();
            regenerateWithFix();
        } else if (e.key === 'Escape') {
            closeFixLastModal();
        }
    });
}

// ── Tab Switch Handler ───────────────────────────────────────────────────────

function setupTabSwitchHandler() {
    window.addEventListener('activeTabChanged', () => {
        const tab = activeChatTab();
        const btn = document.getElementById('btn-fix-last');

        if (!btn || !tab) return;

        // Show button only if there's a last assistant message
        const lastMsg = tab.messages[tab.messages.length - 1];
        if (lastMsg && lastMsg.role === 'assistant') {
            btn.style.display = 'flex';
        } else {
            btn.style.display = 'none';
        }
    });
}

// ── Initialization ───────────────────────────────────────────────────────────

export function initFixLastResponse() {
    setupFixLastButton();
    setupModalClose();
    setupTabSwitchHandler();

    // Initial check
    const tab = activeChatTab();
    const btn = document.getElementById('btn-fix-last');
    if (btn && tab && tab.messages.length > 0) {
        const lastMsg = tab.messages[tab.messages.length - 1];
        btn.style.display = lastMsg.role === 'assistant' ? 'flex' : 'none';
    } else if (btn) {
        btn.style.display = 'none';
    }
}
