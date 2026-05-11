// ── Quick Guide (Inline Input) ──────────────────────────────────────────────
// Collapsible inline input for an active reply guide that persists until changed.

import { activeChatTab, scheduleChatPersist } from './chat-state.js';
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
    const pending = document.getElementById('quick-guide-pending');
    const tab = activeChatTab();
    const draft = tab?.quick_guide_draft || '';
    const activeGuide = tab?.quick_guide_active || tab?.quick_guide_pending || '';

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

    toggleBtn.classList.toggle('guided-action-btn-attentive', !!activeGuide);

    if (status) {
        status.textContent = activeGuide ? 'Active' : draft ? 'Draft' : 'Idle';
        status.hidden = !activeGuide && !quickGuideState.expanded;
    }

    if (pending) {
        pending.textContent = activeGuide
            ? `Active reply guide: ${truncate(activeGuide, 96)}`
            : 'No active reply guide.';
        pending.classList.toggle('is-active', !!activeGuide);
    }

    updateLastUsedDisplay();
}

function truncate(value, maxLength) {
    if (!value) return '';
    return value.length > maxLength ? `${value.slice(0, maxLength)}...` : value;
}

function updateLastUsedDisplay() {
    const lastUsed = document.getElementById('quick-guide-last-used');
    if (!lastUsed) return;

    if (quickGuideState.lastUsedInstruction) {
        lastUsed.textContent = `Last applied: ${truncate(quickGuideState.lastUsedInstruction, 60)}`;
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

// ── Submit Button ────────────────────────────────────────────────────────────

function setupSubmitButton() {
    const submitBtn = document.getElementById('quick-guide-submit-btn');
    if (!submitBtn) return;

    submitBtn.addEventListener('click', submitQuickGuide);
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
