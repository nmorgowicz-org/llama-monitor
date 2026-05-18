// Chat Sessions Sidebar
// Renders and manages the left session panel inside #page-chat.
// Activated when the Chat nav item is selected; hidden otherwise.

import { chat } from '../core/app-state.js';
import { switchChatTab, closeChatTab, addChatTab, renameChatTab,
         togglePinTab, activeChatTab } from './chat-state.js';

const CSP_COLLAPSED_KEY = 'csp-collapsed';

// Lifecycle

export function initChatSessionsSidebar() {
    const newBtn    = document.getElementById('csp-new-btn');
    const collapseBtn = document.getElementById('csp-collapse-btn');
    const searchEl  = document.getElementById('csp-search');
    const strip = document.getElementById('csp-collapsed-strip');

    newBtn?.addEventListener('click', () => addChatTab());
    collapseBtn?.addEventListener('click', toggleSessionPanelCollapse);

    // Make whole strip clickable to expand
    strip?.addEventListener('click', (e) => {
        e.stopPropagation();
        expandSessionPanel();
    });

    searchEl?.addEventListener('input', () => {
        const q = searchEl.value.trim().toLowerCase();
        _applySearchFilter(q);
    });

    document.addEventListener('click', (e) => {
        if (!e.target.closest('.csp-context-menu')) _dismissContextMenu();
    });
    document.addEventListener('keydown', (e) => {
        if (e.key === 'Escape') _dismissContextMenu();
    });

    if (localStorage.getItem(CSP_COLLAPSED_KEY) === 'true') {
        const panel = document.getElementById('chat-sessions-panel');
        panel?.classList.add('collapsed');
    }

    updateCollapsedLabel();
}

// Show / Hide (called from nav.js)

export function showSessionPanel() {
    const panel = document.getElementById('chat-sessions-panel');
    if (!panel) return;
    panel.classList.add('visible');
    // Respect user's last collapse preference
    const shouldStayCollapsed = localStorage.getItem(CSP_COLLAPSED_KEY) === 'true';
    if (!shouldStayCollapsed) {
        panel.classList.remove('collapsed');
    }
}

export function hideSessionPanel() {
    const panel = document.getElementById('chat-sessions-panel');
    if (!panel) return;
    panel.classList.remove('visible');
}

export function expandSessionPanel() {
    const panel = document.getElementById('chat-sessions-panel');
    if (!panel) return;
    panel.classList.add('visible');
    panel.classList.remove('collapsed');
    localStorage.setItem(CSP_COLLAPSED_KEY, 'false');
}

export function toggleSessionPanelCollapse() {
    const panel = document.getElementById('chat-sessions-panel');
    if (!panel) return;
    const collapsed = panel.classList.toggle('collapsed');
    localStorage.setItem(CSP_COLLAPSED_KEY, collapsed.toString());

    const icon = document.querySelector('#csp-collapse-btn svg');
    if (icon) {
        icon.style.transform = collapsed ? 'rotate(180deg)' : '';
    }

    updateCollapsedLabel();
}

function updateCollapsedLabel() {
    const label = document.getElementById('csp-collapsed-label');
    if (!label) return;
    const tab = (chat.tabs || []).find(t => t.id === chat.activeTabId);
    label.textContent = tab?.name || 'Conversations';
}

// Render

export function renderChatSessionsSidebar() {
    const list = document.getElementById('csp-list');
    if (!list) return;

    const groups = _groupTabsByRecency(chat.tabs || []);
    const activeId = chat.activeTabId;

    const sections = [
        { key: 'pinned',    label: 'Pinned' },
        { key: 'today',     label: 'Today' },
        { key: 'yesterday', label: 'Yesterday' },
        { key: 'week',      label: 'This Week' },
        { key: 'older',     label: 'Older' },
    ];

    const frag = document.createDocumentFragment();

    for (const { key, label } of sections) {
        const tabs = groups[key];
        if (!tabs || tabs.length === 0) continue;

        const hdr = document.createElement('div');
        hdr.className = 'csp-section-header';
        hdr.textContent = label;
        frag.appendChild(hdr);

        for (const tab of tabs) {
            frag.appendChild(_buildSessionItem(tab, tab.id === activeId));
        }
    }

    list.innerHTML = '';
    list.appendChild(frag);

    _applySearchFilter(document.getElementById('csp-search')?.value.trim().toLowerCase() || '');
    updateCollapsedLabel();
}

export function updateSessionItem(tabId) {
    const list = document.getElementById('csp-list');
    const existing = list?.querySelector(`.csp-item[data-tab-id="${tabId}"]`);
    if (!existing) return;

    const tab = (chat.tabs || []).find(t => t.id === tabId);
    if (!tab) { existing.remove(); return; }

    const isActive = tab.id === chat.activeTabId;
    const fresh = _buildSessionItem(tab, isActive);
    existing.replaceWith(fresh);
}

// Item builder

function _buildSessionItem(tab, isActive) {
    const el = document.createElement('div');
    const ctxPct = tab.lastCtxPct || 0;
    const ctxLevel = ctxPct >= 90 ? 'critical' : ctxPct >= 75 ? 'high' : ctxPct >= 50 ? 'medium' : 'low';
    const msgCount = (tab.messages || []).filter(m => m.role !== 'system').length;
    const initial = (tab.name || '?').charAt(0).toUpperCase();
    const hue = _avatarHue(tab.id);

    el.className = 'csp-item' + (isActive ? ' active' : '');
    el.dataset.tabId = tab.id;
    el.dataset.ctx = ctxLevel;
    el.setAttribute('role', 'button');
    el.setAttribute('tabindex', '0');
    el.setAttribute('aria-current', isActive ? 'true' : 'false');

    // Build inner HTML with fully static markup; dynamic values via DOM APIs.
    el.innerHTML =
        `<div class="csp-item-avatar"><span></span></div>` +
        `<div class="csp-item-body">` +
            `<div class="csp-item-name"></div>` +
            `<div class="csp-item-meta">` +
                `<span class="csp-item-persona"></span>` +
                `<span class="csp-item-explicit"></span>` +
                `<span class="csp-item-count"></span>` +
            `</div>` +
            `<div class="csp-item-ctx-bar">` +
                `<div class="csp-item-ctx-fill"></div>` +
            `</div>` +
        `</div>` +
        `<div class="csp-item-actions">` +
            `<button class="csp-item-action-btn" data-action="pin"></button>` +
            `<button class="csp-item-action-btn" data-action="more">\u22EF</button>` +
        `</div>`;

    // Avatar initial
    const avatarSpan = el.querySelector('.csp-item-avatar span');
    if (avatarSpan) avatarSpan.textContent = initial;

    // Name
    const nameEl = el.querySelector('.csp-item-name');
    if (nameEl) nameEl.textContent = tab.name || 'Untitled';

    // Persona
    const personaEl = el.querySelector('.csp-item-persona');
    if (personaEl) {
        if (tab.active_template_id) {
            personaEl.dataset.templateId = tab.active_template_id;
            personaEl.textContent = '\u2026';
        } else {
            personaEl.textContent = 'Default';
        }
    }

    // Explicit level
    const explicitEl = el.querySelector('.csp-item-explicit');
    if (explicitEl) {
        explicitEl.dataset.level = String(tab.explicit_level || 0);
    }

    // Message count
    const countEl = el.querySelector('.csp-item-count');
    if (countEl) {
        if (msgCount) {
            countEl.textContent = msgCount + ' msg' + (msgCount !== 1 ? 's' : '');
        } else {
            countEl.style.display = 'none';
        }
    }

    // Pin button
    const pinBtn = el.querySelector('button[data-action="pin"]');
    if (pinBtn) {
        pinBtn.textContent = tab.pinned ? '\u{1F4CC}' : '\u2299';
        pinBtn.title = tab.pinned ? 'Unpin' : 'Pin';
    }

    // Set dynamic styles via DOM API to keep innerHTML safe
    const avatar = el.querySelector('.csp-item-avatar');
    if (avatar) {
        avatar.style.setProperty('--avatar-hue', String(hue));
    }

    const ctxFill = el.querySelector('.csp-item-ctx-fill');
    if (ctxFill) {
        ctxFill.style.width = ctxPct.toFixed(1) + '%';
    }

    if (tab.active_template_id) {
        _resolvePersonaLabel(el, tab.active_template_id);
    }

    el.addEventListener('click', (e) => {
        const actionBtn = e.target.closest('[data-action]');
        if (actionBtn) {
            e.stopPropagation();
            const action = actionBtn.dataset.action;
            if (action === 'pin') {
                togglePinTab(tab.id);
                renderChatSessionsSidebar();
            } else if (action === 'more') {
                _showContextMenu(tab, actionBtn);
            }
            return;
        }
        switchChatTab(tab.id);
        renderChatSessionsSidebar();
    });

    el.addEventListener('keydown', (e) => {
        if (e.key === 'Enter' || e.key === ' ') {
            e.preventDefault();
            switchChatTab(tab.id);
            renderChatSessionsSidebar();
        }
    });

    return el;
}

// Context menu

let _activeMenu = null;

function _showContextMenu(tab, anchorEl) {
    _dismissContextMenu();

    const menu = document.createElement('div');
    menu.className = 'csp-context-menu';
    menu.setAttribute('role', 'menu');
    menu.setAttribute('tabindex', '-1');

    const items = [
        { label: 'Rename',          action: 'rename' },
        { label: tab.pinned ? 'Unpin' : 'Pin', action: 'pin' },
        { label: 'Export JSON',     action: 'export-json' },
        { label: 'Export Markdown', action: 'export-md' },
        { label: 'Duplicate',       action: 'duplicate' },
        { separator: true },
        { label: 'Delete',          action: 'delete', danger: true },
    ];

    for (const item of items) {
        if (item.separator) {
            const sep = document.createElement('div');
            sep.className = 'csp-context-menu-separator';
            menu.appendChild(sep);
            continue;
        }
        const el = document.createElement('div');
        el.className = 'csp-context-menu-item' + (item.danger ? ' danger' : '');
        el.textContent = item.label;
        el.setAttribute('role', 'menuitem');
        el.addEventListener('click', (e) => {
            e.stopPropagation();
            _dismissContextMenu();
            _handleContextAction(tab, item.action);
        });
        menu.appendChild(el);
    }

    document.body.appendChild(menu);
    _activeMenu = menu;

    const rect = anchorEl.getBoundingClientRect();
    const menuW = 170;
    const left = Math.min(rect.right + 4, window.innerWidth - menuW - 8);
    menu.style.left = left + 'px';
    menu.style.top  = rect.top + 'px';
    menu.focus();
}

function _dismissContextMenu() {
    _activeMenu?.remove();
    _activeMenu = null;
}

function _handleContextAction(tab, action) {
    switch (action) {
        case 'rename': {
            const newName = prompt('Rename conversation:', tab.name);
            if (newName && newName.trim()) {
                renameChatTab(tab.id, newName.trim());
                renderChatSessionsSidebar();
            }
            break;
        }
        case 'pin':
            togglePinTab(tab.id);
            renderChatSessionsSidebar();
            break;
        case 'export-json':
            // Delegate to existing export handler on window
            window.exportChatTab?.('json');
            break;
        case 'export-md':
            window.exportChatTab?.('md');
            break;
        case 'duplicate': {
            const copy = {
                ...tab,
                id: crypto.randomUUID(),
                name: tab.name + ' (copy)',
                messages: [...(tab.messages || [])],
                created_at: Date.now(),
                updated_at: Date.now(),
            };
            chat.tabs.push(copy);
            switchChatTab(copy.id);
            renderChatSessionsSidebar();
            // Trigger persistence via chat-state
            const t = activeChatTab();
            if (t) {
                t._dirty = true;
                clearTimeout(t._persistTimer);
                t._persistTimer = setTimeout(() => {
                    if (!t._dirty) return;
                    t._dirty = false;
                    // For now use existing scheduleChatPersist via import
                    import('./chat-state.js').then(m => m.scheduleChatPersist?.(t));
                }, 500);
            }
            break;
        }
        case 'delete':
            closeChatTab(tab.id);
            renderChatSessionsSidebar();
            break;
    }
}

// Search filter

function _applySearchFilter(q) {
    const list = document.getElementById('csp-list');
    if (!list) return;

    list.querySelectorAll('.csp-item').forEach(el => {
        if (!q) { el.style.display = ''; return; }
        const name = el.querySelector('.csp-item-name')?.textContent.toLowerCase() || '';
        const persona = el.querySelector('.csp-item-persona')?.textContent.toLowerCase() || '';
        el.style.display = (name.includes(q) || persona.includes(q)) ? '' : 'none';
    });

    list.querySelectorAll('.csp-section-header').forEach(hdr => {
        let next = hdr.nextElementSibling;
        let allHidden = true;
        while (next && !next.classList.contains('csp-section-header')) {
            if (next.classList.contains('csp-item') && next.style.display !== 'none') {
                allHidden = false; break;
            }
            next = next.nextElementSibling;
        }
        hdr.style.display = allHidden ? 'none' : '';
    });
}

// Grouping & utilities

function _groupTabsByRecency(tabs) {
    const now = Date.now();
    const d  = (ms) => Math.floor(ms / 86400000);
    const today = d(now);

    const groups = { pinned: [], today: [], yesterday: [], week: [], older: [] };
    for (const tab of tabs) {
        if (tab.pinned) { groups.pinned.push(tab); continue; }
        const dayDiff = today - d(tab.updated_at || tab.created_at || now);
        if (dayDiff === 0)      groups.today.push(tab);
        else if (dayDiff === 1) groups.yesterday.push(tab);
        else if (dayDiff <= 7)  groups.week.push(tab);
        else                    groups.older.push(tab);
    }
    return groups;
}

function _avatarHue(id) {
    let h = 0;
    for (let i = 0; i < id.length; i++) h = (h * 31 + id.charCodeAt(i)) & 0xFFFF;
    return h % 360;
}

async function _resolvePersonaLabel(itemEl, templateId) {
    const span = itemEl.querySelector('.csp-item-persona');
    if (!span) return;
    const templates = await window.loadTemplates?.();
    const tmpl = templates?.find(t => t.id === templateId);
    span.textContent = tmpl?.name || '';
}

function escapeHtml(s) {
    const div = document.createElement('div');
    div.textContent = s;
    return div.innerHTML;
}

function escapeAttr(s) {
    return String(s).replace(/"/g, '&quot;').replace(/'/g, '&#39;');
}
