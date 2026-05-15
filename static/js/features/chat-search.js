// chat-search.js — FTS cross-session search
// Triggered by a dedicated search button; replaces the session list with
// a clear "Search conversations" mode and structured results.
// The existing #csp-search in chat-sessions-sidebar.js handles name filtering.

import { switchChatTab } from './chat-state.js';

let _searchActive = false;
let _searchInput = null;
let _searchTimer = null;
let _searchList = null;
let _searchHeader = null;

export function initChatSearch() {
    const panel = document.getElementById('chat-sessions-panel');
    if (!panel) return;

    const header = document.querySelector('.csp-header');
    if (!header) return;

    // Search button
    const searchBtn = document.createElement('button');
    searchBtn.className = 'csp-search-btn';
    searchBtn.setAttribute('title', 'Search conversations');
    searchBtn.innerHTML =
        `<svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5">
            <circle cx="11" cy="11" r="8"/>
            <path d="M21 21l-4.35-4.35"/>
        </svg>`;
    searchBtn.addEventListener('click', openSearch);
    header.appendChild(searchBtn);

    // Search input wrap (hidden initially)
    const wrap = document.createElement('div');
    wrap.className = 'csp-search-input-wrap';
    wrap.style.display = 'none';

    _searchInput = document.createElement('input');
    _searchInput.id = 'csp-search-input';
    _searchInput.type = 'search';
    _searchInput.className = 'csp-search-input';
    _searchInput.placeholder = 'Search messages…';
    _searchInput.autocomplete = 'off';
    _searchInput.addEventListener('input', onSearchInput);
    _searchInput.addEventListener('blur', () => {
        setTimeout(closeSearch, 150);
    });

    const closeBtn = document.createElement('button');
    closeBtn.className = 'csp-search-close';
    closeBtn.setAttribute('title', 'Close search');
    closeBtn.innerHTML = '✕';
    closeBtn.addEventListener('click', closeSearch);

    wrap.appendChild(_searchInput);
    wrap.appendChild(closeBtn);
    header.appendChild(wrap);

    // Search header (e.g. "Search conversations")
    _searchHeader = document.createElement('div');
    _searchHeader.className = 'csp-search-header';
    _searchHeader.style.display = 'none';
    _searchHeader.innerHTML =
        `<div class="csp-search-header-title">Search conversations</div>
         <button class="csp-search-back-btn" type="button">Back</button>`;
    _searchHeader.querySelector('.csp-search-back-btn').addEventListener('click', closeSearch);

    const listEl = document.getElementById('csp-list');
    if (listEl) listEl.after(_searchHeader);

    // Results container (hidden initially)
    _searchList = document.createElement('div');
    _searchList.className = 'csp-search-results';
    _searchList.style.display = 'none';
    if (listEl) _searchHeader.after(_searchList);

    document.addEventListener('keydown', (e) => {
        if (e.key === 'Escape' && _searchActive) closeSearch();
    });
}

export function openSearch() {
    const panel = document.getElementById('chat-sessions-panel');
    if (!panel.classList.contains('visible')) {
        panel.classList.add('visible');
        panel.classList.remove('collapsed');
    }

    _searchActive = true;

    const wrap = _searchInput?.closest('.csp-search-input-wrap');
    if (wrap) wrap.style.display = 'flex';
    _searchInput?.focus();

    const list = document.getElementById('csp-list');
    if (list) list.style.display = 'none';

    if (_searchHeader) _searchHeader.style.display = 'flex';
    if (_searchList) _searchList.style.display = 'flex';
}

export function closeSearch() {
    if (!_searchActive) return;
    _searchActive = false;

    const wrap = _searchInput?.closest('.csp-search-input-wrap');
    if (wrap) wrap.style.display = 'none';

    _searchInput.value = '';

    if (_searchList) _searchList.innerHTML = '';
    if (_searchList) _searchList.style.display = 'none';

    if (_searchHeader) _searchHeader.style.display = 'none';

    const list = document.getElementById('csp-list');
    if (list) list.style.display = '';

    clearTimeout(_searchTimer);
}

async function onSearchInput() {
    const q = _searchInput?.value?.trim();
    clearTimeout(_searchTimer);

    if (!q || q.length < 2) {
        if (_searchList) _searchList.innerHTML = '';
        return;
    }

    _searchTimer = setTimeout(async () => {
        try {
            const resp = await fetch(`/api/chat/search?q=${encodeURIComponent(q)}&limit=50`);
            const results = await resp.json();
            renderResults(results);
        } catch (e) {
            console.error('[chat-search] search failed:', e);
        }
    }, 300);
}

function renderResults(results) {
    if (!_searchList) return;

    if (!results || results.length === 0) {
        _searchList.innerHTML =
            `<div class="csp-search-empty">
                No conversations found for “${escapeHtml(_searchInput.value)}”
             </div>`;
        return;
    }

    const frag = document.createDocumentFragment();

    for (const r of results) {
        const card = document.createElement('div');
        card.className = 'csp-search-result';

        const roleLabel = r.role === 'assistant' ? 'AI' : 'You';
        const snippet = r.snippet || '';

        // eslint-disable-next-line no-unsanitized/property
        card.innerHTML =
            `<div class="csp-search-result-header">
                <span class="csp-search-result-tab">${escapeHtml(r.tab_name)}</span>
                <span class="csp-search-result-role">${roleLabel}</span>
             </div>
             <div class="csp-search-result-snippet">${snippet}</div>`;

        card.addEventListener('click', () => {
            switchChatTab(r.tab_id).then(() => {
                setTimeout(() => {
                    const el = document.querySelector(`.chat-message[data-msg-id="${r.message_id}"]`);
                    if (el) {
                        el.scrollIntoView({ behavior: 'smooth', block: 'center' });
                        el.classList.add('chat-msg-highlight');
                        setTimeout(() => el.classList.remove('chat-msg-highlight'), 2000);
                    }
                }, 400);
            });
            closeSearch();
        });

        frag.appendChild(card);
    }

    _searchList.innerHTML = '';
    _searchList.appendChild(frag);
}

function escapeHtml(s) {
    const div = document.createElement('div');
    div.textContent = s;
    return div.innerHTML;
}
