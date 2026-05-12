// ── Context Notes (Stretchable Bar) ──────────────────────────────────────────
// Stretchable bar on right side of chat messages for persistent context notes.

import { activeChatTab, persistChatTabs } from './chat-state.js';
import { escapeHtml } from '../core/format.js';
import { showToast } from './toast.js';

const SIDEBAR_STORAGE_KEY = 'llama_monitor_sidebar_width';
const SIDEBAR_INTRO_HIDDEN_KEY = 'llama_monitor_context_notes_intro_hidden';
const DEFAULT_WIDTH = 280;
const MIN_WIDTH = 240;
const MAX_WIDTH = 600;

const PREDEFINED_SECTIONS = [
    { id: 'character', name: 'Character', icon: '👤', placeholder: 'e.g. "Kira, 28, cynical detective with a dry wit. Secretly writes poetry."' },
    { id: 'setting', name: 'Setting', icon: '🌍', placeholder: 'e.g. "Neo-Tokyo, 2087. Acid rain, neon signs, the Yakuza run everything."' },
    { id: 'plot', name: 'Plot', icon: '📖', placeholder: 'e.g. "The murder victim was Kira\'s mentor. The suspect list includes her partner."' },
    { id: 'tone', name: 'Tone', icon: '🎭', placeholder: 'e.g. "Noir atmosphere. Wry humor. Short punchy sentences. No melodrama."' },
];

let sidebarResizing = false;
let sidebarState = {
    expanded: false,
    activeSection: null,
    editingNoteIndex: null,
    composerSection: null,
    composerDrafts: {},
};

function setSvgIcon(button, paths) {
    if (!button) return;
    const svgNs = 'http://www.w3.org/2000/svg';
    const svg = document.createElementNS(svgNs, 'svg');
    svg.setAttribute('width', '12');
    svg.setAttribute('height', '12');
    svg.setAttribute('viewBox', '0 0 24 24');
    svg.setAttribute('fill', 'none');
    svg.setAttribute('stroke', 'currentColor');
    svg.setAttribute('stroke-width', '2.2');
    svg.setAttribute('stroke-linecap', 'round');
    svg.setAttribute('stroke-linejoin', 'round');

    paths.forEach((d) => {
        const path = document.createElementNS(svgNs, 'path');
        path.setAttribute('d', d);
        svg.appendChild(path);
    });

    button.replaceChildren(svg);
}

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
    const messages = document.getElementById('chat-messages');
    const countBadge = document.getElementById('context-sidebar-count');
    const collapseBtn = document.getElementById('context-sidebar-collapse');
    const subtitle = document.getElementById('chat-sidebar-subtitle');
    const introToggle = document.getElementById('chat-sidebar-intro-toggle');
    const tab = activeChatTab();

    if (!sidebar || !contextBar || !toggleBtn || !messages || !tab) return;

    // Update width from tab state or localStorage
    const savedWidth = localStorage.getItem(SIDEBAR_STORAGE_KEY);
    const width = tab.sidebar_width ?? (savedWidth ? parseInt(savedWidth, 10) : DEFAULT_WIDTH);
    messages.style.setProperty('--chat-sidebar-current-width', sidebarState.expanded ? `${width}px` : '36px');

    const notesCount = (tab.context_notes || []).filter(note => note.content?.trim()).length;
    const introHidden = localStorage.getItem(SIDEBAR_INTRO_HIDDEN_KEY) === 'true';

    if (countBadge) {
        if (notesCount > 0) {
            countBadge.hidden = false;
            countBadge.textContent = String(notesCount);
        } else {
            countBadge.hidden = true;
        }
    }
    if (subtitle) {
        subtitle.hidden = introHidden;
    }
    if (introToggle) {
        introToggle.classList.toggle('is-hidden-state', introHidden);
        introToggle.setAttribute('title', introHidden ? 'Show context notes description' : 'Hide context notes description');
        introToggle.setAttribute('aria-label', introHidden ? 'Show context notes description' : 'Hide context notes description');
        setSvgIcon(
            introToggle,
            introHidden
                ? ['M3 12s3.5-7 9-7 9 7 9 7-3.5 7-9 7-9-7-9-7z', 'M12 9a3 3 0 100 6 3 3 0 000-6z']
                : ['M18 6L6 18', 'M6 6l12 12']
        );
    }

    // Update expanded state
    if (sidebarState.expanded) {
        sidebar.classList.add('sidebar-expanded');
        contextBar.classList.add('expanded');
        toggleBtn.classList.add('active');
        toggleBtn.setAttribute('aria-expanded', 'true');
        toggleBtn.setAttribute('aria-label', 'Close context notes');
        collapseBtn?.classList.add('visible');
    } else {
        sidebar.classList.remove('sidebar-expanded');
        contextBar.classList.remove('expanded');
        toggleBtn.classList.remove('active');
        toggleBtn.setAttribute('aria-expanded', 'false');
        toggleBtn.setAttribute('aria-label', 'Open context notes');
        collapseBtn?.classList.remove('visible');
    }

    renderNotesList();
}

// ── Notes List Rendering ─────────────────────────────────────────────────────

function renderNotesList() {
    const container = document.getElementById('sidebar-notes-list');
    const tab = activeChatTab();

    if (!container || !tab) return;

    const notes = (tab.context_notes || []).filter(note => note.content?.trim());
    const customSections = [
        ...new Set([
            ...(tab.context_custom_sections || []),
            ...notes
                .filter(n => !PREDEFINED_SECTIONS.some(s => s.name === n.section))
                .map(n => n.section),
        ].filter(Boolean)),
    ];

    // Render all predefined sections
    // eslint-disable-next-line no-unsanitized/property
    container.innerHTML = PREDEFINED_SECTIONS.map(sectionDef => {
        const sectionNotes = notes.filter(n => n.section === sectionDef.name);
        const hasNotes = sectionNotes.length > 0;
        const isComposing = sidebarState.composerSection === sectionDef.name;
        const draft = sidebarState.composerDrafts[sectionDef.name] || '';

        return `
            <div class="sidebar-section-wrapper" data-section="${escapeHtml(sectionDef.name)}">
                <div class="sidebar-section-header">
                    <div class="sidebar-section-title">
                        <span class="sidebar-section-icon">${sectionDef.icon}</span>
                        ${escapeHtml(sectionDef.name)}
                    </div>
                    <div class="sidebar-section-actions">
                        <button class="sidebar-add-note-btn" data-section="${escapeHtml(sectionDef.name)}" title="Add note to ${sectionDef.name}">${isComposing ? 'Writing…' : '+ Add Note'}</button>
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
                    ${isComposing ? `
                        <div class="sidebar-add-note-form" data-section-form="${escapeHtml(sectionDef.name)}">
                            <div class="sidebar-form-group">
                                <label class="sidebar-form-label">New ${escapeHtml(sectionDef.name)} note</label>
                                <textarea class="sidebar-form-textarea sidebar-note-compose-input" data-section="${escapeHtml(sectionDef.name)}" placeholder="${escapeHtml(sectionDef.placeholder)}">${escapeHtml(draft)}</textarea>
                            </div>
                            <div class="sidebar-note-compose-chips">
                                <button type="button" class="sidebar-note-chip" data-section-chip="${escapeHtml(sectionDef.name)}" data-template="${escapeHtml(sectionDef.placeholder)}">Use example</button>
                                <button type="button" class="sidebar-note-chip" data-section-chip="${escapeHtml(sectionDef.name)}" data-template="Keep this concise and high-signal.">Concise</button>
                            </div>
                            <div class="sidebar-note-compose-actions">
                                <button class="sidebar-form-btn sidebar-note-compose-save" data-section-save="${escapeHtml(sectionDef.name)}">Save Note</button>
                                <button class="sidebar-note-btn sidebar-note-btn-cancel" data-section-cancel="${escapeHtml(sectionDef.name)}">Cancel</button>
                            </div>
                        </div>
                    ` : ''}
                </div>
            </div>
        `;
    }).join('');

    // Add custom sections if any
    customSections.forEach(sectionName => {
        const sectionNotes = notes.filter(n => n.section === sectionName);
        const isComposing = sidebarState.composerSection === sectionName;
        const draft = sidebarState.composerDrafts[sectionName] || '';
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
                    <button class="sidebar-add-note-btn" data-section="${escapeHtml(sectionName)}" title="Add note to ${sectionName}">${isComposing ? 'Writing…' : '+ Add Note'}</button>
                </div>
            </div>
            <div class="sidebar-section-notes">
                ${sectionNotes.length ? sectionNotes.map((note, i) => {
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
                }).join('') : `<div class="sidebar-section-empty">Capture reusable details for this section.</div>`}
                ${isComposing ? `
                    <div class="sidebar-add-note-form" data-section-form="${escapeHtml(sectionName)}">
                        <div class="sidebar-form-group">
                            <label class="sidebar-form-label">New note</label>
                            <textarea class="sidebar-form-textarea sidebar-note-compose-input" data-section="${escapeHtml(sectionName)}" placeholder="Add note...">${escapeHtml(draft)}</textarea>
                        </div>
                        <div class="sidebar-note-compose-actions">
                            <button class="sidebar-form-btn sidebar-note-compose-save" data-section-save="${escapeHtml(sectionName)}">Save Note</button>
                            <button class="sidebar-note-btn sidebar-note-btn-cancel" data-section-cancel="${escapeHtml(sectionName)}">Cancel</button>
                        </div>
                    </div>
                ` : ''}
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

    container.querySelectorAll('.sidebar-note-compose-input').forEach(textarea => {
        textarea.addEventListener('input', () => {
            sidebarState.composerDrafts[textarea.dataset.section] = textarea.value;
        });
        textarea.addEventListener('keydown', (e) => {
            if (e.key === 'Enter' && (e.metaKey || e.ctrlKey)) {
                e.preventDefault();
                submitComposerSection(textarea.dataset.section);
            } else if (e.key === 'Escape') {
                e.preventDefault();
                cancelComposerSection(textarea.dataset.section);
            }
        });
    });

    container.querySelectorAll('[data-section-save]').forEach(btn => {
        btn.addEventListener('click', () => submitComposerSection(btn.dataset.sectionSave));
    });

    container.querySelectorAll('[data-section-cancel]').forEach(btn => {
        btn.addEventListener('click', () => cancelComposerSection(btn.dataset.sectionCancel));
    });

    container.querySelectorAll('[data-section-chip]').forEach(btn => {
        btn.addEventListener('click', () => {
            const section = btn.dataset.sectionChip;
            const input = container.querySelector(`.sidebar-note-compose-input[data-section="${CSS.escape(section)}"]`);
            if (!input) return;
            input.value = btn.dataset.template || '';
            sidebarState.composerDrafts[section] = input.value;
            input.focus();
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
    sidebarState.composerSection = section;
    sidebarState.composerDrafts[section] = sidebarState.composerDrafts[section] || '';
    renderNotesList();
    requestAnimationFrame(() => {
        const textarea = document.querySelector(`.sidebar-note-compose-input[data-section="${CSS.escape(section)}"]`);
        textarea?.focus();
    });
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

function submitComposerSection(section) {
    const content = (sidebarState.composerDrafts[section] || '').trim();
    if (!content) {
        showToast('Note content cannot be empty', 'error');
        return;
    }

    const saved = addNote(section, content);
    if (!saved) return;
    sidebarState.composerDrafts[section] = '';
    sidebarState.composerSection = null;
}

function cancelComposerSection(section) {
    sidebarState.composerSection = null;
    sidebarState.composerDrafts[section] = '';
    renderNotesList();
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
    const messages = document.getElementById('chat-messages');
    const handle = document.getElementById('chat-context-bar-resize');

    if (!messages || !handle) return;

    handle.addEventListener('mousedown', (e) => {
        sidebarResizing = true;
        handle.classList.add('active');
        e.preventDefault();
        document.body.style.cursor = 'col-resize';
        document.body.style.userSelect = 'none';

        const tab = activeChatTab();
        const startWidth = tab?.sidebar_width ?? DEFAULT_WIDTH;
        const startX = e.clientX;

        const onMouseMove = (moveEvent) => {
            const delta = startX - moveEvent.clientX;
            const newWidth = Math.max(MIN_WIDTH, Math.min(MAX_WIDTH, startWidth + delta));
            messages.style.setProperty('--chat-sidebar-current-width', `${newWidth}px`);
        };

        const onMouseUp = () => {
            sidebarResizing = false;
            handle.classList.remove('active');
            document.body.style.cursor = '';
            document.body.style.userSelect = '';

            // Save width to tab and localStorage
            const width = parseInt(getComputedStyle(messages).getPropertyValue('--chat-sidebar-current-width'), 10);
            const activeTab = activeChatTab();
            if (activeTab) {
                activeTab.sidebar_width = width;
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
            const customSections = tab?.context_custom_sections || [];
            
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

    tab.context_custom_sections = tab.context_custom_sections || [];
    if (!tab.context_custom_sections.includes(sectionName)) {
        tab.context_custom_sections.push(sectionName);
    }
    tab.updated_at = Date.now();
    sidebarState.composerSection = sectionName;
    sidebarState.composerDrafts[sectionName] = '';

    persistChatTabs().then(() => {
        renderNotesList();
        updateContextInjection();
    }).catch(err => {
        showToast(`Failed to add section: ${err.message}`, 'error');
    });
}

// ── Sidebar Close Handler ────────────────────────────────────────────────────

function setupSidebarCloseHandler() {
    const collapseBtn = document.getElementById('context-sidebar-collapse');
    if (collapseBtn) {
        collapseBtn.addEventListener('click', (e) => {
            e.stopPropagation();
            sidebarState.expanded = false;
            updateSidebarUI();
        });
    }
}

function setupSidebarIntroToggle() {
    const introToggle = document.getElementById('chat-sidebar-intro-toggle');
    if (!introToggle) return;

    introToggle.addEventListener('click', (e) => {
        e.stopPropagation();
        const introHidden = localStorage.getItem(SIDEBAR_INTRO_HIDDEN_KEY) === 'true';
        localStorage.setItem(SIDEBAR_INTRO_HIDDEN_KEY, introHidden ? 'false' : 'true');
        updateSidebarUI();
    });
}

// ── Initialization ───────────────────────────────────────────────────────────

export function initContextSidebar() {
    setupResizeHandle();
    setupAddSectionHandler();
    setupSidebarCloseHandler();
    setupSidebarIntroToggle();
    updateSidebarUI();

    // Listen for tab switches
    window.addEventListener('activeTabChanged', () => {
        updateSidebarUI();
    });
}
