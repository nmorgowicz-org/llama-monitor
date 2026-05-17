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
let _searchCount = null;
let _collapsedBeforeSearch = false;

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

    const icon = document.createElement('span');
    icon.className = 'csp-search-input-icon';
    icon.setAttribute('aria-hidden', 'true');
    icon.innerHTML =
        `<svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.3">
            <circle cx="11" cy="11" r="7"/>
            <path d="M20 20l-3.5-3.5"/>
        </svg>`;

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

    wrap.appendChild(icon);
    wrap.appendChild(_searchInput);
    wrap.appendChild(closeBtn);
    header.appendChild(wrap);

    // Search header (e.g. "Search conversations")
    _searchHeader = document.createElement('div');
    _searchHeader.className = 'csp-search-header';
    _searchHeader.style.display = 'none';
    _searchHeader.innerHTML =
        `<div class="csp-search-header-copy">
            <div class="csp-search-header-title">Message Search</div>
            <div class="csp-search-header-subtitle">Across all conversations</div>
         </div>
         <div class="csp-search-header-meta">
            <span class="csp-search-count">Type 2+ letters</span>
         </div>
         <button class="csp-search-back-btn" type="button">Back</button>`;
    _searchHeader.querySelector('.csp-search-back-btn').addEventListener('click', closeSearch);
    _searchCount = _searchHeader.querySelector('.csp-search-count');

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
    _collapsedBeforeSearch = panel.classList.contains('collapsed')
        || localStorage.getItem('csp-collapsed') === 'true';
    if (!panel.classList.contains('visible')) {
        panel.classList.add('visible');
    }
    panel.classList.remove('collapsed');

    _searchActive = true;
    panel.classList.add('search-active');

    const wrap = _searchInput?.closest('.csp-search-input-wrap');
    if (wrap) wrap.style.display = 'flex';
    _searchInput?.focus();
    _searchInput?.select();

    const list = document.getElementById('csp-list');
    if (list) list.style.display = 'none';

    if (_searchHeader) _searchHeader.style.display = 'flex';
    if (_searchList) _searchList.style.display = 'flex';
    renderIdleState();
}

export function closeSearch() {
    if (!_searchActive) return;
    _searchActive = false;

    const panel = document.getElementById('chat-sessions-panel');
    panel?.classList.remove('search-active');

    const wrap = _searchInput?.closest('.csp-search-input-wrap');
    if (wrap) wrap.style.display = 'none';

    _searchInput.value = '';

    if (_searchList) _searchList.innerHTML = '';
    if (_searchList) _searchList.style.display = 'none';
    updateSearchCount('Type 2+ letters');

    if (_searchHeader) _searchHeader.style.display = 'none';

    const list = document.getElementById('csp-list');
    if (list) list.style.display = '';

    clearTimeout(_searchTimer);

    if (_collapsedBeforeSearch) {
        panel?.classList.add('collapsed');
    }
    _collapsedBeforeSearch = false;
}

async function onSearchInput() {
    const q = _searchInput?.value?.trim();
    clearTimeout(_searchTimer);

    if (!q || q.length < 2) {
        renderIdleState();
        return;
    }

    _searchTimer = setTimeout(async () => {
        try {
            renderLoadingState(q);
            const resp = await fetch(`/api/chat/search?q=${encodeURIComponent(q)}&limit=50`);
            const results = await resp.json();
            renderResults(results, q);
        } catch (e) {
            console.error('[chat-search] search failed:', e);
            renderErrorState();
        }
    }, 300);
}

function renderResults(results, query) {
    if (!_searchList) return;

    if (!results || results.length === 0) {
        updateSearchCount('0 matches');
        _searchList.innerHTML =
            `<div class="csp-search-empty">
                <div class="csp-search-empty-title">No matches found</div>
                <div class="csp-search-empty-body">Try a broader phrase or a shorter fragment than “${escapeHtml(query)}”.</div>
             </div>`;
        return;
    }

    updateSearchCount(`${results.length} ${results.length === 1 ? 'match' : 'matches'}`);
    const frag = document.createDocumentFragment();

    for (const r of results) {
        const card = document.createElement('div');
        card.className = 'csp-search-result';

        const roleLabel = r.role === 'assistant' ? 'AI' : 'You';
        const rawSnippet = r.snippet || '';
        const snippet = (typeof window.DOMPurify !== 'undefined')
            ? window.DOMPurify.sanitize(rawSnippet, { ALLOWED_TAGS: ['mark'] })
            : rawSnippet;

        // eslint-disable-next-line no-unsanitized/property
        card.innerHTML =
            `<div class="csp-search-result-header">
                <div class="csp-search-result-heading">
                    <span class="csp-search-result-tab">${escapeHtml(r.tab_name)}</span>
                    <span class="csp-search-result-role">${roleLabel}</span>
                </div>
                <span class="csp-search-result-jump">Jump</span>
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

function renderIdleState() {
    if (!_searchList) return;
    updateSearchCount('Type 2+ letters');
    _searchList.innerHTML =
        `<div class="csp-search-empty csp-search-empty-idle">
            <div class="csp-search-empty-title">Search across messages</div>
            <div class="csp-search-empty-body">Find exact moments, themes, or fragments from any chat tab.</div>
         </div>`;
}

function renderLoadingState(query) {
    if (!_searchList) return;
    updateSearchCount('Searching…');
    _searchList.innerHTML =
        `<div class="csp-search-empty csp-search-empty-loading">
            <div class="csp-search-empty-title">Searching</div>
            <div class="csp-search-empty-body">Looking for “${escapeHtml(query)}” across your conversation history.</div>
         </div>`;
}

function renderErrorState() {
    if (!_searchList) return;
    updateSearchCount('Search unavailable');
    _searchList.innerHTML =
        `<div class="csp-search-empty csp-search-empty-error">
            <div class="csp-search-empty-title">Search unavailable</div>
            <div class="csp-search-empty-body">The message index could not be queried right now. Try again in a moment.</div>
         </div>`;
}

function updateSearchCount(text) {
    if (_searchCount) {
        _searchCount.textContent = text;
    }
}
