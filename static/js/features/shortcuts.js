// ── Shortcuts ─────────────────────────────────────────────────────────────────
// Global keyboard shortcuts: modal toggle, chat tab switching.

import { chat } from '../core/app-state.js';
import { switchChatTab } from './chat-state.js';
import { updateTabBarOverflowMask } from './chat-render.js';

// ── Keyboard Shortcuts Modal ──────────────────────────────────────────────────

function openKeyboardShortcutsModal() {
    document.getElementById('keyboard-shortcuts-modal').classList.add('open');
}

function closeKeyboardShortcutsModal() {
    document.getElementById('keyboard-shortcuts-modal').classList.remove('open');
}

// Show modal on Ctrl+/ (or Cmd+/ on Mac)
document.addEventListener('keydown', e => {
    if (e.key === '/' && (e.ctrlKey || e.metaKey) && !e.altKey) {
        e.preventDefault();
        openKeyboardShortcutsModal();
    }
});

// Close modal on Escape key
document.addEventListener('keydown', e => {
    if (e.key === 'Escape' && document.getElementById('keyboard-shortcuts-modal').classList.contains('open')) {
        closeKeyboardShortcutsModal();
    }
});

// ── Chat Keyboard Shortcuts ───────────────────────────────────────────────────

function initChatKeyboardShortcuts() {
    document.addEventListener('keydown', e => {
        if (!document.getElementById('page-chat')?.classList.contains('active')) return;
        if ((e.ctrlKey || e.metaKey) && e.key >= '1' && e.key <= '9') {
            e.preventDefault();
            const idx = parseInt(e.key) - 1;
            if (chat.tabs[idx]) switchChatTab(chat.tabs[idx].id);
        }
        if ((e.ctrlKey || e.metaKey) && e.shiftKey && e.key === 'ArrowRight') {
            e.preventDefault();
            const idx = chat.tabs.findIndex(t => t.id === chat.activeTabId);
            const next = chat.tabs[(idx + 1) % chat.tabs.length];
            if (next) switchChatTab(next.id);
        }
        if ((e.ctrlKey || e.metaKey) && e.shiftKey && e.key === 'ArrowLeft') {
            e.preventDefault();
            const idx = chat.tabs.findIndex(t => t.id === chat.activeTabId);
            const prev = chat.tabs[(idx - 1 + chat.tabs.length) % chat.tabs.length];
            if (prev) switchChatTab(prev.id);
        }
    });
    window.addEventListener('resize', () => {
        updateTabBarOverflowMask();
    });
}

// ── Public API ────────────────────────────────────────────────────────────────

export { openKeyboardShortcutsModal };

export function initShortcuts() {
    // Call setup functions that bind DOM event listeners
    initChatKeyboardShortcuts();

    // Bind shortcuts modal close button
    const closeBtn = document.getElementById('shortcuts-close-btn');
    if (closeBtn) {
        closeBtn.addEventListener('click', closeKeyboardShortcutsModal);
    }

}
