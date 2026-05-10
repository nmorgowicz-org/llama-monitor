// ── Context Notes (Stretchable Bar) ──────────────────────────────────────────
// Stretchable bar on right side of chat messages for persistent context notes.

import { activeChatTab, persistChatTabs } from './chat-state.js';
import { escapeHtml } from '../core/format.js';
import { showToast } from './toast.js';

const SIDEBAR_STORAGE_KEY = 'llama_monitor_sidebar_width';
const DEFAULT_WIDTH = 320;
const MIN_WIDTH = 240;
const MAX_WIDTH = 600;

const PREDEFINED_SECTIONS = [
    { id: 'character', name: 'Character', icon: '👤', placeholder: 'Add character details...' },
    { id: 'setting', name: 'Setting', icon: '🌍', placeholder: 'Add setting info...' },
    { id: 'plot', name: 'Plot', icon: '📖', placeholder: 'Add plot points...' },
    { id: 'tone', name: 'Tone', icon: '🎭', placeholder: 'Add tone/style notes...' },
];

let sidebarResizing = false;
let sidebarState = {
    expanded: false,
    activeSection: null,
    editingNoteIndex: null,
};

// ── Sidebar Toggle ────────────────────────────────────────────────────────────

export function toggleContextSidebar() {
    const settings = JSON.parse(localStorage.getItem('llama_monitor_settings') || '{}');
    if (settings.enabled_context_notes === false) return;

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
    const contextBar = document.getElementById('chat-context-bar');
    const toggleBtn = document.getElementById('context-sidebar-toggle');
    const tab = activeChatTab();

    if (!sidebar || !contextBar || !toggleBtn || !tab) return;

    // Update width from tab state or localStorage
    const savedWidth = localStorage.getItem(SIDEBAR_STORAGE_KEY);
    const width = tab.sidebar_width ?? (savedWidth ? parseInt(savedWidth, 10) : DEFAULT_WIDTH);
    contextBar.style.width = sidebarState.expanded ? `${width}px` : '24px';

    // Update expanded state
    if (sidebarState.expanded) {
        sidebar.classList.add('sidebar-expanded');
        contextBar.classList.add('expanded');
        toggleBtn.classList.add('active');
        toggleBtn.setAttribute('aria-expanded', 'true');
    } else {
        sidebar.classList.remove('sidebar-expanded');
        contextBar.classList.remove('expanded');
        toggleBtn.classList.remove('active');
        toggleBtn.setAttribute('aria-expanded', 'false');
    }

    renderNotesList();
}

// ── Notes List Rendering ─────────────────────────────────────────────────────

function renderNotesList() {
    const container = document.getElementById('sidebar-notes-list');
    const tab = activeChatTab();

    if (!container || !tab) return;

    const notes = tab.context_notes || [];

    // Render all predefined sections
    // eslint-disable-next-line no-unsanitized/property
    container.innerHTML = PREDEFINED_SECTIONS.map(sectionDef => {
        const sectionNotes = notes.filter(n => n.section === sectionDef.name);
        const hasNotes = sectionNotes.length > 0;

        return `
            <div class="sidebar-section-wrapper" data-section="${escapeHtml(sectionDef.name)}">
                <div class="sidebar-section-header">
                    <div class="sidebar-section-title">
                        <span class="sidebar-section-icon">${sectionDef.icon}</span>
                        ${escapeHtml(sectionDef.name)}
                    </div>
                    <div class="sidebar-section-actions">
                        <button class="sidebar-add-note-btn" data-section="${escapeHtml(sectionDef.name)}" title="Add note to ${sectionDef.name}">+ Add Note</button>
                    </div>
                </div>
                <div class="sidebar-section-notes">
                    ${hasNotes ? sectionNotes.map((note, i) => {
                        const originalIndex = notes.indexOf(note);
                        const isEditing = sidebarState.editingNoteIndex === originalIndex;
                        return `
                            <div class="sidebar-note-item ${isEditing ? 'sidebar-note-item-editing' : ''}" data-index="${originalIndex}">
                                <div class="sidebar-note-content">
                                    ${isEditing ?
                                        `<textarea class="sidebar-note-content-edit" data-index="${originalIndex}" placeholder="${escapeHtml(sectionDef.placeholder)}">${escapeHtml(note.content)}</textarea>` :
                                        escapeHtml(note.content)
                                    }
                                </div>
                                <div class="sidebar-note-actions">
                                    ${isEditing ?
                                        `<button class="sidebar-note-btn sidebar-note-btn-save" data-index="${originalIndex}" title="Save">✓ Save</button>
                                         <button class="sidebar-note-btn sidebar-note-btn-cancel" data-index="${originalIndex}" title="Cancel">✕ Cancel</button>` :
                                        `<button class="sidebar-note-btn sidebar-note-btn-delete" data-index="${originalIndex}" title="Delete">✕ Delete</button>`
                                    }
                                </div>
                            </div>
                        `;
                    }).join('') :
                        `<div class="sidebar-section-empty">${escapeHtml(sectionDef.placeholder)}</div>`
                    }
                </div>
            </div>
        `;
    }).join('');

    // Add custom sections if any
    const customSections = [...new Set(notes.filter(n => !PREDEFINED_SECTIONS.some(s => s.name === n.section)).map(n => n.section))];
    customSections.forEach(sectionName => {
        const sectionNotes = notes.filter(n => n.section === sectionName);
        const sectionWrapper = document.createElement('div');
        sectionWrapper.className = 'sidebar-section-wrapper';
        sectionWrapper.dataset.section = escapeHtml(sectionName);
        // eslint-disable-next-line no-unsanitized/property
        sectionWrapper.innerHTML = `
            <div class="sidebar-section-header">
                <div class="sidebar-section-title">
                    <span class="sidebar-section-icon">📝</span>
                    ${escapeHtml(sectionName)}
                </div>
                <div class="sidebar-section-actions">
                    <button class="sidebar-add-note-btn" data-section="${escapeHtml(sectionName)}" title="Add note to ${sectionName}">+ Add Note</button>
                </div>
            </div>
            <div class="sidebar-section-notes">
                ${sectionNotes.map((note, i) => {
                    const originalIndex = notes.indexOf(note);
                    const isEditing = sidebarState.editingNoteIndex === originalIndex;
                    return `
                        <div class="sidebar-note-item ${isEditing ? 'sidebar-note-item-editing' : ''}" data-index="${originalIndex}">
                            <div class="sidebar-note-content">
                                ${isEditing ?
                                    `<textarea class="sidebar-note-content-edit" data-index="${originalIndex}" placeholder="Add note...">${escapeHtml(note.content)}</textarea>` :
                                    escapeHtml(note.content)
                                }
                            </div>
                            <div class="sidebar-note-actions">
                                ${isEditing ?
                                    `<button class="sidebar-note-btn sidebar-note-btn-save" data-index="${originalIndex}" title="Save">✓ Save</button>
                                     <button class="sidebar-note-btn sidebar-note-btn-cancel" data-index="${originalIndex}" title="Cancel">✕ Cancel</button>` :
                                    `<button class="sidebar-note-btn sidebar-note-btn-delete" data-index="${originalIndex}" title="Delete">✕ Delete</button>`
                                }
                            </div>
                        </div>
                    `;
                }).join('')}
            </div>
        `;
        container.appendChild(sectionWrapper);
    });

    setupNoteHandlers();
}

function setupNoteHandlers() {
    const container = document.getElementById('sidebar-notes-list');
    if (!container) return;

    // Add note button handlers
    container.querySelectorAll('.sidebar-add-note-btn').forEach(btn => {
        btn.addEventListener('click', () => {
            const section = btn.dataset.section;
            addNoteForSection(section);
        });
    });

    // Note click handlers (inline edit)
    container.querySelectorAll('.sidebar-note-item').forEach(item => {
        item.addEventListener('click', (e) => {
            if (e.target.classList.contains('sidebar-note-btn')) return;
            const index = parseInt(item.dataset.index, 10);
            startEditingNote(index);
        });
    });

    // Delete handlers
    container.querySelectorAll('.sidebar-note-btn-delete').forEach(btn => {
        btn.addEventListener('click', (e) => {
            e.stopPropagation();
            const index = parseInt(btn.dataset.index, 10);
            deleteNote(index);
        });
    });

    // Save handlers
    container.querySelectorAll('.sidebar-note-btn-save').forEach(btn => {
        btn.addEventListener('click', (e) => {
            e.stopPropagation();
            const index = parseInt(btn.dataset.index, 10);
            saveEditingNote(index);
        });
    });

    // Cancel handlers
    container.querySelectorAll('.sidebar-note-btn-cancel').forEach(btn => {
        btn.addEventListener('click', (e) => {
            e.stopPropagation();
            const index = parseInt(btn.dataset.index, 10);
            cancelEditingNote(index);
        });
    });

    // Textarea enter key handler
    container.querySelectorAll('.sidebar-note-content-edit').forEach(textarea => {
        textarea.addEventListener('keydown', (e) => {
            if (e.key === 'Enter' && !e.shiftKey) {
                e.preventDefault();
                const index = parseInt(textarea.dataset.index, 10);
                saveEditingNote(index);
            }
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

function addNoteForSection(section) {
    const sectionDef = PREDEFINED_SECTIONS.find(s => s.name === section);
    const placeholder = sectionDef ? sectionDef.placeholder : 'Add note...';
    const content = prompt(`Add ${section} note:`, '');
    if (content && content.trim()) {
        addNote(section, content);
    }
}

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
        tab.context_notes.pop();
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

function startEditingNote(index) {
    sidebarState.editingNoteIndex = index;
    renderNotesList();
    
    // Auto-focus the textarea
    requestAnimationFrame(() => {
        const textarea = document.querySelector(`.sidebar-note-content-edit[data-index="${index}"]`);
        if (textarea) {
            textarea.focus();
            textarea.setSelectionRange(textarea.value.length, textarea.value.length);
        }
    });
}

function saveEditingNote(index) {
    const textarea = document.querySelector(`.sidebar-note-content-edit[data-index="${index}"]`);
    if (!textarea) {
        sidebarState.editingNoteIndex = null;
        renderNotesList();
        return;
    }

    const content = textarea.value.trim();
    if (!content) {
        showToast('Note content cannot be empty', 'error');
        return;
    }

    const tab = activeChatTab();
    if (!tab || !tab.context_notes || index < 0 || index >= tab.context_notes.length) {
        sidebarState.editingNoteIndex = null;
        renderNotesList();
        return;
    }

    tab.context_notes[index] = {
        ...tab.context_notes[index],
        content: content,
    };
    tab.updated_at = Date.now();

    persistChatTabs().then(() => {
        sidebarState.editingNoteIndex = null;
        renderNotesList();
        updateContextInjection();
    }).catch(err => {
        showToast(`Failed to save note: ${err.message}`, 'error');
    });
}

function cancelEditingNote(index) {
    sidebarState.editingNoteIndex = null;
    renderNotesList();
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
    const contextBar = document.getElementById('chat-context-bar');
    const handle = document.getElementById('chat-context-bar-resize');

    if (!contextBar || !handle) return;

    handle.addEventListener('mousedown', (e) => {
        sidebarResizing = true;
        e.preventDefault();
        document.body.style.cursor = 'col-resize';
        document.body.style.userSelect = 'none';

        const startWidth = parseInt(contextBar.style.width || DEFAULT_WIDTH, 10);
        const startX = e.clientX;

        const onMouseMove = (moveEvent) => {
            const delta = startX - moveEvent.clientX;
            const newWidth = Math.max(MIN_WIDTH, Math.min(MAX_WIDTH, startWidth + delta));
            contextBar.style.width = `${newWidth}px`;
        };

        const onMouseUp = () => {
            sidebarResizing = false;
            document.body.style.cursor = '';
            document.body.style.userSelect = '';

            // Save width to tab and localStorage
            const width = parseInt(contextBar.style.width, 10);
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

// ── Add Section Handler ──────────────────────────────────────────────────────

function setupAddSectionHandler() {
    const triggerBtn = document.getElementById('sidebar-add-section-trigger');
    const inputContainer = document.getElementById('sidebar-add-section-input');
    const nameInput = document.getElementById('sidebar-section-name-input');
    const confirmBtn = document.getElementById('sidebar-section-confirm-btn');
    const cancelBtn = document.getElementById('sidebar-section-cancel-btn');

    if (!triggerBtn || !inputContainer || !nameInput) return;

    // Show input
    triggerBtn.addEventListener('click', () => {
        inputContainer.classList.remove('hidden');
        triggerBtn.classList.add('hidden');
        nameInput.value = '';
        nameInput.focus();
    });

    // Confirm new section
    confirmBtn.addEventListener('click', () => {
        const sectionName = nameInput.value.trim();
        if (sectionName) {
            // Check if section already exists
            const existingSections = PREDEFINED_SECTIONS.map(s => s.name);
            const tab = activeChatTab();
            const customSections = tab?.context_notes?.filter(n => !existingSections.includes(n.section)).map(n => n.section) || [];
            
            if (existingSections.includes(sectionName) || customSections.includes(sectionName)) {
                showToast('Section already exists', 'error');
                return;
            }
            
            addCustomSection(sectionName);
            inputContainer.classList.add('hidden');
            triggerBtn.classList.remove('hidden');
        }
    });

    // Cancel
    cancelBtn.addEventListener('click', () => {
        inputContainer.classList.add('hidden');
        triggerBtn.classList.remove('hidden');
        nameInput.value = '';
    });

    // Enter key to confirm
    nameInput.addEventListener('keydown', (e) => {
        if (e.key === 'Enter') {
            confirmBtn.click();
        } else if (e.key === 'Escape') {
            cancelBtn.click();
        }
    });
}

function addCustomSection(sectionName) {
    const tab = activeChatTab();
    if (!tab) return;

    tab.context_notes = tab.context_notes || [];
    tab.context_notes.push({
        section: sectionName,
        content: '',
        created_at: Date.now(),
    });
    tab.updated_at = Date.now();

    persistChatTabs().then(() => {
        renderNotesList();
        updateContextInjection();
    }).catch(err => {
        showToast(`Failed to add section: ${err.message}`, 'error');
    });
}

// ── Sidebar Close Handler ────────────────────────────────────────────────────

function setupSidebarCloseHandler() {
    const closeBtn = document.getElementById('chat-sidebar-close');
    if (!closeBtn) return;

    closeBtn.addEventListener('click', () => {
        sidebarState.expanded = false;
        updateSidebarUI();
    });
}

// ── Initialization ───────────────────────────────────────────────────────────

export function initContextSidebar() {
    setupResizeHandle();
    setupAddSectionHandler();
    setupSidebarCloseHandler();
    updateSidebarUI();

    // Listen for tab switches
    window.addEventListener('activeTabChanged', () => {
        updateSidebarUI();
    });
}
