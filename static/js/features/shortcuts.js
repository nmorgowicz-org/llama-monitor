// ── Shortcuts ─────────────────────────────────────────────────────────────────
// Global keyboard shortcuts: modal toggle, chat tab switching.

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
            if (window.chatTabs[idx]) window.switchChatTab(window.chatTabs[idx].id);
        }
        if ((e.ctrlKey || e.metaKey) && e.shiftKey && e.key === 'ArrowRight') {
            e.preventDefault();
            const idx = window.chatTabs.findIndex(t => t.id === window.activeChatTabId);
            const next = window.chatTabs[(idx + 1) % window.chatTabs.length];
            if (next) window.switchChatTab(next.id);
        }
        if ((e.ctrlKey || e.metaKey) && e.shiftKey && e.key === 'ArrowLeft') {
            e.preventDefault();
            const idx = window.chatTabs.findIndex(t => t.id === window.activeChatTabId);
            const prev = window.chatTabs[(idx - 1 + window.chatTabs.length) % window.chatTabs.length];
            if (prev) window.switchChatTab(prev.id);
        }
    });
    window.addEventListener('resize', () => {
        window.updateTabBarOverflowMask();
    });
}

// ── Public API ────────────────────────────────────────────────────────────────

export function initShortcuts() {
    window.openKeyboardShortcutsModal = openKeyboardShortcutsModal;
    window.closeKeyboardShortcutsModal = closeKeyboardShortcutsModal;
    window.initChatKeyboardShortcuts = initChatKeyboardShortcuts;
}
