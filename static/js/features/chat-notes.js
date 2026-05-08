// ── Context Notes (Sidebar) ─────────────────────────────────────────────────
// Right sidebar for persistent context notes (character, setting, plot details).

import { activeChatTab, persistChatTabs } from './chat-state.js';
import { escapeHtml } from '../core/format.js';
import { showToast } from './toast.js';

const SIDEBAR_STORAGE_KEY = 'llama_monitor_sidebar_width';
const DEFAULT_WIDTH = 280;

let sidebarResizing = false;
let sidebarState = {
    expanded: false,
    activeSection: null,
};

// ── Sidebar Toggle ────────────────────────────────────────────────────────────

export function toggleContextSidebar() {
    const settings = JSON.parse(localStorage.getItem('llama_monitor_settings') || '{}');
    if (!settings.enabled_context_notes) return;

    sidebarState.expanded = !sidebarState.expanded;
    updateSidebarUI();
}

export function isContextSidebarEnabled() {
    const settings = JSON.parse(localStorage.getItem('llama_monitor_settings') || '{}');
    return settings.enabled_context_notes !== false;
}

export function getContextSidebarState() {
    return sidebarState;
}

// ── Sidebar UI Updates ────────────────────────────────────────────────────────

function updateSidebarUI() {
    const sidebar = document.getElementById('chat-sidebar');
    const toggleBtn = document.getElementById('context-sidebar-toggle');
    const tab = activeChatTab();

    if (!sidebar || !toggleBtn || !tab) return;

    // Update width from tab state or localStorage
    const savedWidth = localStorage.getItem(SIDEBAR_STORAGE_KEY);
    const width = tab.sidebar_width ?? (savedWidth ? parseInt(savedWidth, 10) : DEFAULT_WIDTH);
    sidebar.style.width = `${width}px`;

    // Update expanded state
    if (sidebarState.expanded) {
        sidebar.classList.add('sidebar-expanded');
        toggleBtn.classList.add('active');
        toggleBtn.setAttribute('aria-expanded', 'true');
        toggleBtn.innerHTML = '✕';
    } else {
        sidebar.classList.remove('sidebar-expanded');
        toggleBtn.classList.remove('active');
        toggleBtn.setAttribute('aria-expanded', 'false');
        toggleBtn.innerHTML = '📋';
    }

    renderNotesList();
}

// ── Notes List Rendering ─────────────────────────────────────────────────────

function renderNotesList() {
    const container = document.getElementById('sidebar-notes-list');
    const tab = activeChatTab();

    if (!container || !tab) return;

    const notes = tab.context_notes || [];

    if (notes.length === 0) {
        container.innerHTML = `
            <div class="sidebar-empty-state">
                <p>No context notes yet</p>
                <p class="text-sm">Add notes to help the model remember character details, setting info, and plot points.</p>
            </div>
        `;
        return;
    }

    // eslint-disable-next-line no-unsanitized/property -- All user content escaped via escapeHtml()
    container.innerHTML = notes.map((note, index) => `
        <div class="sidebar-note-item" data-index="${index}">
            <div class="sidebar-note-header">
                <span class="sidebar-note-section">${escapeHtml(note.section)}</span>
                <button class="sidebar-note-btn sidebar-note-btn-delete" title="Delete note">✕</button>
            </div>
            <div class="sidebar-note-content">${escapeHtml(note.content)}</div>
            <div class="sidebar-note-meta">
                ${formatNoteTime(note.created_at)}
            </div>
        </div>
    `).join('');

    // Attach delete handlers
    container.querySelectorAll('.sidebar-note-btn-delete').forEach(btn => {
        btn.addEventListener('click', (e) => {
            e.stopPropagation();
            const item = e.target.closest('.sidebar-note-item');
            const index = parseInt(item.dataset.index, 10);
            deleteNote(index);
        });
    });
}

function formatNoteTime(timestamp) {
    const date = new Date(timestamp);
    const now = new Date();
    const diffMs = now - date;
    const diffMins = Math.floor(diffMs / 60000);
    const diffHours = Math.floor(diffMs / 3600000);
    const diffDays = Math.floor(diffMs / 86400000);

    if (diffMins < 1) return 'Just now';
    if (diffMins < 60) return `${diffMins}m ago`;
    if (diffHours < 24) return `${diffHours}h ago`;
    if (diffDays < 7) return `${diffDays}d ago`;
    return date.toLocaleDateString();
}

// ── Note CRUD Operations ─────────────────────────────────────────────────────

export function addNote(section, content) {
    const tab = activeChatTab();
    if (!tab || !content.trim()) return false;

    const note = {
        section: section.trim() || 'General',
        content: content.trim(),
        created_at: Date.now(),
    };

    tab.context_notes = tab.context_notes || [];
    tab.context_notes.push(note);
    tab.updated_at = Date.now();

    persistChatTabs().then(() => {
        renderNotesList();
        updateContextInjection();
    }).catch(err => {
        showToast(`Failed to save note: ${err.message}`, 'error');
        tab.context_notes.pop(); // Revert on error
    });

    return true;
}

function deleteNote(index) {
    const tab = activeChatTab();
    if (!tab || !tab.context_notes || index < 0 || index >= tab.context_notes.length) return;

    tab.context_notes.splice(index, 1);
    tab.updated_at = Date.now();

    persistChatTabs().then(() => {
        renderNotesList();
        updateContextInjection();
    }).catch(err => {
        showToast(`Failed to delete note: ${err.message}`, 'error');
    });
}

export function updateNote(index, section, content) {
    const tab = activeChatTab();
    if (!tab || !tab.context_notes || index < 0 || index >= tab.context_notes.length) return false;

    tab.context_notes[index] = {
        ...tab.context_notes[index],
        section: section.trim() || 'General',
        content: content.trim(),
    };
    tab.updated_at = Date.now();

    persistChatTabs().then(() => {
        renderNotesList();
        updateContextInjection();
    }).catch(err => {
        showToast(`Failed to update note: ${err.message}`, 'error');
    });

    return true;
}

// ── Context Injection ────────────────────────────────────────────────────────

function updateContextInjection() {
    // Signal to chat-transport to rebuild messages with new notes
    window.dispatchEvent(new CustomEvent('contextNotesUpdated', {
        detail: { tabId: activeChatTab()?.id },
    }));
}

// ── Sidebar Resize ───────────────────────────────────────────────────────────

function setupResizeHandle() {
    const sidebar = document.getElementById('chat-sidebar');
    const handle = document.getElementById('sidebar-resize-handle');

    if (!sidebar || !handle) return;

    handle.addEventListener('mousedown', (e) => {
        sidebarResizing = true;
        e.preventDefault();
        document.body.style.cursor = 'col-resize';
        document.body.style.userSelect = 'none';

        const onMouseMove = (moveEvent) => {
            const newWidth = Math.max(200, Math.min(500, moveEvent.clientX - sidebar.offsetWidth + parseInt(sidebar.style.width || DEFAULT_WIDTH, 10)));
            sidebar.style.width = `${newWidth}px`;
        };

        const onMouseUp = () => {
            sidebarResizing = false;
            document.body.style.cursor = '';
            document.body.style.userSelect = '';

            // Save width to tab and localStorage
            const width = parseInt(sidebar.style.width, 10);
            const tab = activeChatTab();
            if (tab) {
                tab.sidebar_width = width;
                persistChatTabs().catch(() => {});
            }
            localStorage.setItem(SIDEBAR_STORAGE_KEY, width.toString());

            document.removeEventListener('mousemove', onMouseMove);
            document.removeEventListener('mouseup', onMouseUp);
        };

        document.addEventListener('mousemove', onMouseMove);
        document.addEventListener('mouseup', onMouseUp);
    });
}

// ── Add Note Form ────────────────────────────────────────────────────────────

function setupAddNoteForm() {
    const form = document.getElementById('sidebar-add-note-form');
    const sectionInput = document.getElementById('sidebar-note-section');
    const contentInput = document.getElementById('sidebar-note-content');

    if (!form) return;

    form.addEventListener('submit', (e) => {
        e.preventDefault();
        const section = sectionInput?.value || '';
        const content = contentInput?.value || '';

        if (addNote(section, content)) {
            sectionInput.value = '';
            contentInput.value = '';
            contentInput.focus();
        }
    });
}

// ── Initialization ───────────────────────────────────────────────────────────

export function initContextSidebar() {
    setupResizeHandle();
    setupAddNoteForm();
    updateSidebarUI();

    // Listen for tab switches
    window.addEventListener('activeTabChanged', () => {
        updateSidebarUI();
    });
}
