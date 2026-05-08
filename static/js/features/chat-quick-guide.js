// ── Quick Guide (Inline Input) ──────────────────────────────────────────────
// Collapsible inline input for ephemeral instructions (one-time context injection).

import { activeChatTab } from './chat-state.js';
import { escapeHtml } from '../core/format.js';
import { showToast } from './toast.js';

let quickGuideState = {
    expanded: false,
    currentValue: '',
    lastUsedInstruction: null,
};

// ── Toggle ───────────────────────────────────────────────────────────────────

export function toggleQuickGuide() {
    const settings = JSON.parse(localStorage.getItem('llama_monitor_settings') || '{}');
    if (!settings.enabled_quick_guide) return;

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
    const input = document.getElementById('quick-guide-input');

    if (!container || !toggleBtn) return;

    if (quickGuideState.expanded) {
        container.classList.add('quick-guide-expanded');
        toggleBtn.classList.add('active');
        toggleBtn.setAttribute('aria-expanded', 'true');
        toggleBtn.innerHTML = '▼';

        if (input) {
            input.focus();
            input.value = quickGuideState.currentValue;
        }
    } else {
        container.classList.remove('quick-guide-expanded');
        toggleBtn.classList.remove('active');
        toggleBtn.setAttribute('aria-expanded', 'false');
        toggleBtn.innerHTML = '🧭';
    }
}

// ── Input Handling ───────────────────────────────────────────────────────────

function setupInputHandler() {
    const input = document.getElementById('quick-guide-input');
    if (!input) return;

    input.addEventListener('input', (e) => {
        quickGuideState.currentValue = e.target.value;
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
    const instruction = quickGuideState.currentValue.trim();
    if (!instruction) return;

    // Save as last used
    quickGuideState.lastUsedInstruction = instruction;

    // Dispatch event for chat-transport to handle
    window.dispatchEvent(new CustomEvent('quickGuideSubmitted', {
        detail: { instruction },
    }));

    // Clear and collapse
    quickGuideState.currentValue = '';
    quickGuideState.expanded = false;
    updateQuickGuideUI();

    // Show confirmation
    showToast('Quick guide instruction added to context', 'success');
}

// ── Submit Button ────────────────────────────────────────────────────────────

function setupSubmitButton() {
    const submitBtn = document.getElementById('quick-guide-submit-btn');
    if (!submitBtn) return;

    submitBtn.addEventListener('click', submitQuickGuide);
}

// ── Clear Button ─────────────────────────────────────────────────────────────

function setupClearButton() {
    const clearBtn = document.getElementById('quick-guide-clear-btn');
    if (!clearBtn) return;

    clearBtn.addEventListener('click', () => {
        quickGuideState.currentValue = '';
        quickGuideState.lastUsedInstruction = null;
        const input = document.getElementById('quick-guide-input');
        if (input) input.value = '';
        updateQuickGuideUI();
    });
}

// ── Last Used Display ───────────────────────────────────────────────────────

function setupLastUsedDisplay() {
    const lastUsed = document.getElementById('quick-guide-last-used');
    if (!lastUsed) return;

    function updateLastUsed() {
        if (quickGuideState.lastUsedInstruction) {
            lastUsed.textContent = `Last: ${escapeHtml(quickGuideState.lastUsedInstruction.substring(0, 50))}${quickGuideState.lastUsedInstruction.length > 50 ? '...' : ''}`;
            lastUsed.style.display = 'block';
        } else {
            lastUsed.style.display = 'none';
        }
    }

    updateLastUsed();
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
    setupClearButton();
    setupLastUsedDisplay();
    setupClickOutside();
    updateQuickGuideUI();
}
