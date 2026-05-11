// ── Quick Guide (Inline Input) ──────────────────────────────────────────────
// Collapsible inline input for an active reply guide that persists until changed.

import { activeChatTab, getChatViewBindings, scheduleChatPersist } from './chat-state.js';
import { showToast } from './toast.js';

let quickGuideState = {
    expanded: false,
    lastUsedInstruction: null,
};

// ── Toggle ───────────────────────────────────────────────────────────────────

export function toggleQuickGuide() {
    const settings = JSON.parse(localStorage.getItem('llama_monitor_settings') || '{}');
    if (settings.enabled_quick_guide === false) return;

    quickGuideState.expanded = !quickGuideState.expanded;
    updateQuickGuideUI();
}

export function isQuickGuideEnabled() {
    const settings = JSON.parse(localStorage.getItem('llama_monitor_settings') || '{}');
    return settings.enabled_quick_guide !== false;
}

export function getQuickGuideState() {
    return quickGuideState;
}

// ── UI Updates ───────────────────────────────────────────────────────────────

function updateQuickGuideUI() {
    const container = document.getElementById('quick-guide-container');
    const toggleBtn = document.getElementById('quick-guide-toggle');
    const wrapper = toggleBtn?.closest('.guided-tool');
    const input = document.getElementById('quick-guide-input');
    const status = document.getElementById('quick-guide-status');
    const restoreBtn = document.getElementById('quick-guide-restore-btn');
    const tab = activeChatTab();
    const draft = tab?.quick_guide_draft || '';
    const activeGuide = tab?._quickGuideInFlight ? (tab.quick_guide_active || draft) : '';
    const lastRevision = tab?._quickGuideLastRun || null;

    if (!container || !toggleBtn) return;

    if (quickGuideState.expanded) {
        container.classList.add('quick-guide-expanded');
        toggleBtn.classList.add('active');
        toggleBtn.setAttribute('aria-expanded', 'true');
        wrapper?.classList.add('is-open');

        if (input) {
            input.focus();
            input.value = draft;
        }
    } else {
        container.classList.remove('quick-guide-expanded');
        toggleBtn.classList.remove('active');
        toggleBtn.setAttribute('aria-expanded', 'false');
        wrapper?.classList.remove('is-open');
    }

    toggleBtn.classList.toggle('guided-action-btn-attentive', !!activeGuide || (!!draft && quickGuideState.expanded));

    if (status) {
        status.textContent = activeGuide ? 'Applying' : draft ? 'Draft' : 'Idle';
        status.hidden = !activeGuide && !quickGuideState.expanded;
    }

    if (restoreBtn) {
        restoreBtn.disabled = !lastRevision;
        restoreBtn.hidden = !lastRevision;
    }

    updateLastUsedDisplay();
}

function truncate(value, maxLength) {
    if (!value) return '';
    return value.length > maxLength ? `${value.slice(0, maxLength)}...` : value;
}

function updateLastUsedDisplay() {
    const lastUsed = document.getElementById('quick-guide-last-used');
    const tab = activeChatTab();
    const lastInstruction = tab?._quickGuideLastRun?.instruction || quickGuideState.lastUsedInstruction;
    if (!lastUsed) return;

    if (lastInstruction) {
        lastUsed.textContent = `Last applied: ${truncate(lastInstruction, 60)}`;
        lastUsed.style.display = 'block';
        return;
    }

    lastUsed.style.display = 'none';
}

// ── Input Handling ───────────────────────────────────────────────────────────

function setupInputHandler() {
    const input = document.getElementById('quick-guide-input');
    if (!input) return;

    input.addEventListener('input', (e) => {
        const tab = activeChatTab();
        if (tab) {
            tab.quick_guide_draft = e.target.value;
            scheduleChatPersist();
        }
    });

    input.addEventListener('keydown', (e) => {
        if (e.key === 'Enter' && !e.shiftKey) {
            e.preventDefault();
            submitQuickGuide();
        } else if (e.key === 'Escape') {
            quickGuideState.expanded = false;
            updateQuickGuideUI();
        }
    });
}

// ── Submit ───────────────────────────────────────────────────────────────────

function submitQuickGuide() {
    const tab = activeChatTab();
    const instruction = (tab?.quick_guide_draft || '').trim();
    if (!tab) return;

    quickGuideState.lastUsedInstruction = instruction || null;
    window.dispatchEvent(new CustomEvent('quickGuideSubmitted', {
        detail: { instruction },
    }));

    tab.quick_guide_draft = '';
    quickGuideState.expanded = false;
    updateQuickGuideUI();

    showToast(instruction ? 'Reply guide applied' : 'Reply guide cleared', 'success');
}

function restorePreviousInstructionForEdit() {
    const tab = activeChatTab();
    const input = document.getElementById('quick-guide-input');
    const lastRun = tab?._quickGuideLastRun;
    if (!tab || !lastRun?.instruction) return;

    const targetIndex = typeof lastRun.targetIndex === 'number' ? lastRun.targetIndex : -1;
    const targetMsg = targetIndex >= 0 ? tab.messages[targetIndex] : null;
    if (targetMsg && targetMsg.role === (lastRun.targetRole || 'assistant')) {
        tab.messages.splice(targetIndex, 1);
    } else {
        for (let i = tab.messages.length - 1; i >= 0; i -= 1) {
            if (tab.messages[i]?.role === (lastRun.targetRole || 'assistant')) {
                tab.messages.splice(i, 1);
                break;
            }
        }
    }

    tab.quick_guide_draft = lastRun.instruction;
    tab.quick_guide_active = '';
    tab.updated_at = Date.now();
    quickGuideState.expanded = true;
    getChatViewBindings().renderChatMessages?.();
    scheduleChatPersist();
    updateQuickGuideUI();
    if (input) {
        input.value = lastRun.instruction;
        input.focus();
        input.setSelectionRange(input.value.length, input.value.length);
    }

    showToast('Previous guided reply removed. Edit and apply again.', 'info');
}

// ── Submit Button ────────────────────────────────────────────────────────────

function setupSubmitButton() {
    const submitBtn = document.getElementById('quick-guide-submit-btn');
    const restoreBtn = document.getElementById('quick-guide-restore-btn');
    if (!submitBtn) return;

    submitBtn.addEventListener('click', submitQuickGuide);
    restoreBtn?.addEventListener('click', restorePreviousInstructionForEdit);
}

// ── Last Used Display ───────────────────────────────────────────────────────

function setupLastUsedDisplay() {
    updateLastUsedDisplay();
}

// ── Click Outside to Close ──────────────────────────────────────────────────

function setupClickOutside() {
    document.addEventListener('click', (e) => {
        const container = document.getElementById('quick-guide-container');
        const toggleBtn = document.getElementById('quick-guide-toggle');

        if (!container || !toggleBtn) return;

        const isClickInside = container.contains(e.target) || toggleBtn.contains(e.target);

        if (!isClickInside && quickGuideState.expanded) {
            quickGuideState.expanded = false;
            updateQuickGuideUI();
        }
    });
}

// ── Initialization ───────────────────────────────────────────────────────────

export function initQuickGuide() {
    setupInputHandler();
    setupSubmitButton();
    setupLastUsedDisplay();
    setupClickOutside();
    window.addEventListener('activeTabChanged', updateQuickGuideUI);
    window.addEventListener('quickGuideStateChanged', updateQuickGuideUI);
    updateQuickGuideUI();
}
