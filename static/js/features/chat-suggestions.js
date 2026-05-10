// ── Suggestions (Dropdown) ──────────────────────────────────────────────────
// Dropdown menu with AI-generated suggestions (General, Plot Twist, New Character).

import { activeChatTab, persistChatTabs } from './chat-state.js';
import { escapeHtml } from '../core/format.js';
import { showToast, showToastWithActions } from './toast.js';
import { toggleExplicitMode } from './chat-templates.js';

let suggestionsState = {
    expanded: false,
    currentCategory: 'general',
    isLoading: false,
    suggestions: [],
    recentSuggestions: [],
    customCategories: new Map(),
    retryCount: 0,
    maxRetries: 3,
    lastError: null,
    isOffline: false,
};

const DEBOUNCE_DELAY = 300;
let categorySwitchTimeout = null;

// ── Dropdown Toggle ──────────────────────────────────────────────────────────

export function toggleSuggestionsDropdown() {
    const settings = JSON.parse(localStorage.getItem('llama_monitor_settings') || '{}');
    if (settings.enabled_suggestions === false) return;

    suggestionsState.expanded = !suggestionsState.expanded;
    updateDropdownUI();
}

export function isSuggestionsEnabled() {
    const settings = JSON.parse(localStorage.getItem('llama_monitor_settings') || '{}');
    return settings.enabled_suggestions !== false;
}

export function getSuggestionsState() {
    return suggestionsState;
}

// ── Dropdown UI Updates ──────────────────────────────────────────────────────

function updateDropdownUI() {
    const dropdown = document.getElementById('suggestions-dropdown');
    const toggleBtn = document.getElementById('suggestions-toggle');
    const categoryBtns = document.querySelectorAll('.suggestion-category-btn');
    const listContainer = document.getElementById('suggestions-list');
    const explicitGroup = document.getElementById('suggestions-explicit-group');

    if (!dropdown || !toggleBtn) return;

    if (suggestionsState.expanded) {
        dropdown.classList.add('dropdown-expanded');
        toggleBtn.classList.add('active');
        toggleBtn.setAttribute('aria-expanded', 'true');
        toggleBtn.innerHTML = '▼';
    } else {
        dropdown.classList.remove('dropdown-expanded');
        toggleBtn.classList.remove('active');
        toggleBtn.setAttribute('aria-expanded', 'false');
        toggleBtn.innerHTML = '💡';
    }

    // Toggle explicit group visibility based on explicit_level
    if (explicitGroup) {
        const tab = activeChatTab();
        const explicitEnabled = (tab?.explicit_level ?? 0) > 0;
        explicitGroup.classList.toggle('explicit-enabled', explicitEnabled);
    }

    // Apply search filter
    applySearchFilter();

    // Update category buttons
    categoryBtns.forEach(btn => {
        const category = btn.dataset.category;
        btn.classList.toggle('active', category === suggestionsState.currentCategory);
    });

    if (listContainer) {
        renderSuggestionsList();
    }

    // Always render recent suggestions
    renderRecentSuggestions();
}

function applySearchFilter() {
    const searchInput = document.getElementById('suggestion-search-input');
    if (!searchInput) return;

    const query = searchInput.value.toLowerCase().trim();
    const chips = document.querySelectorAll('.suggestion-category-btn');
    const groups = document.querySelectorAll('.category-group');

    chips.forEach(chip => {
        const text = chip.textContent.toLowerCase();
        const visible = !query || text.includes(query);
        chip.style.display = visible ? '' : 'none';
    });

    groups.forEach(group => {
        const visibleChips = group.querySelectorAll('.suggestion-category-btn:not([style*="display: none"])');
        const hasVisible = visibleChips.length > 0;
        group.style.display = hasVisible ? '' : 'none';
    });
}

// ── Category Switching ───────────────────────────────────────────────────────

export function setSuggestionCategory(category) {
    if (category === 'manage') {
        manageCategories();
        return;
    }

    // Silently prevent explicit category access when explicit mode is off
    if (category === 'explicit') {
        const tab = activeChatTab();
        const explicitMode = (tab?.explicit_level ?? 0) > 0;
        if (!explicitMode) {
            import('./chat-templates.js').then(({ enableExplicitMode }) => {
                enableExplicitMode();
            });
        }
    }

    clearTimeout(categorySwitchTimeout);
    categorySwitchTimeout = setTimeout(() => {
        suggestionsState.currentCategory = category;
        suggestionsState.suggestions = [];
        updateDropdownUI();
        fetchSuggestions();
    }, DEBOUNCE_DELAY);
}

// ── Suggestions List Rendering ───────────────────────────────────────────────

function renderSuggestionsList() {
    const container = document.getElementById('suggestions-list');
    if (!container) return;

    if (suggestionsState.isLoading) {
        container.innerHTML = `
            <div class="suggestions-loading" role="status" aria-live="polite">
                <div class="spinner" aria-hidden="true"></div>
                <p>Generating suggestions...</p>
            </div>
        `;
        return;
    }

    const suggestions = suggestionsState.suggestions;

    if (suggestions.length === 0) {
        container.innerHTML = `
            <div class="suggestions-empty-state" role="status" aria-live="polite">
                <p>No suggestions yet</p>
                <p class="text-sm">Select a category above, then hit **Generate** to ask the AI for writing prompts tailored to your conversation.</p>
            </div>
        `;
        return;
    }

    // eslint-disable-next-line no-unsanitized/property -- User content escaped via escapeHtml()
    container.innerHTML = suggestions.map((suggestion, index) => {
        // Parse Pathweaver format: "TITLE\nDESCRIPTION"
        const parts = suggestion.split('\n');
        const title = parts[0] || suggestion;
        const description = parts.slice(1).join('\n') || '';

        return `
        <div class="suggestion-item" data-index="${index}" role="option" aria-selected="false" tabindex="0">
            ${description ? `<div class="suggestion-title">${escapeHtml(title)}</div>` : ''}
            <div class="suggestion-content">${escapeHtml(description || title)}</div>
            <button class="suggestion-btn suggestion-btn-use" title="Insert this prompt into the chat input" aria-label="Use suggestion: ${escapeHtml(title)}">Use</button>
        </div>
    `;
    }).join('');

    // Attach use handlers
    container.querySelectorAll('.suggestion-btn-use').forEach(btn => {
        btn.addEventListener('click', (e) => {
            e.stopPropagation();
            const item = e.target.closest('.suggestion-item');
            const index = parseInt(item.dataset.index, 10);
            useSuggestion(index);
        });
    });
}

function renderRecentSuggestions() {
    const container = document.getElementById('suggestions-recent');
    const list = document.getElementById('suggestions-recent-list');
    const tab = activeChatTab();

    if (!container || !list || !tab) return;

    const recent = tab._suggestion_history || [];

    if (recent.length === 0) {
        container.style.display = 'none';
        return;
    }

    container.style.display = 'block';
    // eslint-disable-next-line no-unsanitized/property -- User content escaped via escapeHtml()
    list.innerHTML = recent.map((suggestion, index) => `
        <div class="suggestion-item" data-recent-index="${index}" role="option" aria-selected="false" tabindex="0">
            <div class="suggestion-content">${escapeHtml(suggestion)}</div>
            <button class="suggestion-btn suggestion-btn-use" title="Reuse this suggestion" aria-label="Reuse suggestion: ${escapeHtml(suggestion)}">Use</button>
        </div>
    `).join('');

    // Attach reuse handlers
    list.querySelectorAll('.suggestion-btn-use').forEach(btn => {
        btn.addEventListener('click', (e) => {
            e.stopPropagation();
            const item = e.target.closest('.suggestion-item');
            const index = parseInt(item.dataset.recentIndex, 10);
            reuseRecentSuggestion(index);
        });
    });
}

function addRecentSuggestion(suggestion) {
    const tab = activeChatTab();
    if (!tab) return;

    tab._suggestion_history = tab._suggestion_history || [];

    // Remove if already exists to avoid duplicates
    tab._suggestion_history = tab._suggestion_history.filter(s => s !== suggestion);

    // Add to beginning
    tab._suggestion_history.unshift(suggestion);

    // Limit to 10
    if (tab._suggestion_history.length > 10) {
        tab._suggestion_history = tab._suggestion_history.slice(0, 10);
    }

    // Save
    persistChatTabs().catch(() => {});
    renderRecentSuggestions();
}

function reuseRecentSuggestion(index) {
    const tab = activeChatTab();
    if (!tab || !tab._suggestion_history || index < 0 || index >= tab._suggestion_history.length) return;

    const suggestion = tab._suggestion_history[index];

    // Dispatch event for chat-input to handle
    window.dispatchEvent(new CustomEvent('suggestionSelected', {
        detail: { text: suggestion },
    }));

    // Close dropdown
    suggestionsState.expanded = false;
    updateDropdownUI();
}

function clearRecentSuggestions() {
    const tab = activeChatTab();
    if (!tab) return;

    tab._suggestion_history = [];
    persistChatTabs().catch(() => {});
    renderRecentSuggestions();
}

// ── Fetch Suggestions from API ───────────────────────────────────────────────

function getSettingsValue() {
    try {
        return JSON.parse(localStorage.getItem('llama_monitor_settings') || '{}');
    } catch {
        return {};
    }
}

async function fetchSuggestions() {
    const tab = activeChatTab();
    if (!tab) return;

    if (suggestionsState.isOffline) {
        showToast('Offline: suggestions unavailable until connection restored', 'error');
        return;
    }

    suggestionsState.isLoading = true;
    suggestionsState.retryCount = 0;
    updateDropdownUI();

    const settings = getSettingsValue();
    const contextDepth = settings.context_depth ?? 10;
    const prompts = settings.suggestion_prompts ?? {};

    let prompt = prompts[suggestionsState.currentCategory];

    try {
        const response = await fetch('/api/chat/suggestions', {
            method: 'POST',
            headers: {
                'Content-Type': 'application/json',
            },
            body: JSON.stringify({
                tab_id: tab.id,
                category: suggestionsState.currentCategory,
                context_depth: contextDepth,
                prompt: prompt,
            }),
        });

        if (!response.ok) {
            throw new Error(`HTTP ${response.status}: ${response.statusText}`);
        }

        const data = await response.json();
        suggestionsState.suggestions = data.suggestions || [];
        suggestionsState.isLoading = false;
        suggestionsState.lastError = null;
        updateDropdownUI();
    } catch (error) {
        suggestionsState.lastError = error;
        if (suggestionsState.retryCount < suggestionsState.maxRetries) {
            suggestionsState.retryCount++;
            const delay = 1000 * suggestionsState.retryCount;
            setTimeout(() => {
                fetchSuggestions();
            }, delay);
            showToast(`Retrying... (${suggestionsState.retryCount}/${suggestionsState.maxRetries})`, 'warning');
        } else {
            suggestionsState.isLoading = false;
            suggestionsState.suggestions = [];
            updateDropdownUI();
            showToast(`Failed to fetch suggestions: ${error.message}`, 'error');
        }
    }
}

// ── Use Suggestion ───────────────────────────────────────────────────────────

function useSuggestion(index) {
    const suggestion = suggestionsState.suggestions[index];
    if (!suggestion) return;

    // Track in history
    addRecentSuggestion(suggestion);

    // Dispatch event for chat-input to handle
    window.dispatchEvent(new CustomEvent('suggestionSelected', {
        detail: { text: suggestion },
    }));

    // Close dropdown
    suggestionsState.expanded = false;
    updateDropdownUI();
}

// ── Category Management ──────────────────────────────────────────────────────

function manageCategories() {
    const modal = document.getElementById('manage-categories-modal');
    if (!modal) return;

    modal.removeAttribute('aria-hidden');
    modal.inert = false;
    modal.classList.add('open');

    renderCategoriesList();
}

function renderCategoriesList() {
    const list = document.getElementById('categories-list');
    if (!list) return;

    const defaults = {
        general: document.getElementById('settings-prompt-general')?.value || '',
        'plot-twist': document.getElementById('settings-prompt-plot-twist')?.value || '',
        'new-character': document.getElementById('settings-prompt-new-character')?.value || '',
    };

    const categories = [
        { name: 'General', key: 'general', prompt: defaults.general, isDefault: true },
        { name: 'Plot Twist', key: 'plot-twist', prompt: defaults['plot-twist'], isDefault: true },
        { name: 'New Character', key: 'new-character', prompt: defaults['new-character'], isDefault: true },
        ...Array.from(suggestionsState.customCategories.entries()).map(([key, prompt]) => ({
            name: key.charAt(0).toUpperCase() + key.slice(1),
            key,
            prompt,
            isDefault: false,
        })),
    ];

    // eslint-disable-next-line no-unsanitized/property -- User content escaped via escapeHtml()
    list.innerHTML = categories.map(cat => `
        <div class="category-item" data-key="${escapeHtml(cat.key)}" style="display:flex;align-items:center;justify-content:space-between;padding:12px;border:1px solid var(--border-color);border-radius:6px;margin-bottom:8px;background:var(--bg-secondary);">
            <div>
                <strong class="category-name">${escapeHtml(cat.name)}${cat.isDefault ? ' (Default)' : ''}</strong>
                <div class="category-prompt" style="font-size:12px;color:var(--text-muted);margin-top:4px;max-width:400px;overflow:hidden;text-overflow:ellipsis;white-space:nowrap;">${escapeHtml(cat.prompt.substring(0, 100))}${cat.prompt.length > 100 ? '...' : ''}</div>
            </div>
            <div style="display:flex;gap:8px;">
                <button class="btn-sm btn-preset category-action-btn" data-action="edit" data-key="${escapeHtml(cat.key)}" title="Edit prompt">Edit</button>
                ${!cat.isDefault ? `<button class="btn-sm btn-secondary category-action-btn" data-action="delete" data-key="${escapeHtml(cat.key)}" title="Remove category">Remove</button>` : ''}
            </div>
        </div>
    `).join('');

    list.querySelectorAll('.category-action-btn').forEach(btn => {
        btn.addEventListener('click', (e) => {
            const action = e.target.dataset.action;
            const key = e.target.dataset.key;
            if (action === 'edit') {
                editCategoryPrompt(key);
            } else if (action === 'delete') {
                removeCategory(key);
            }
        });
    });
}

function addCategory(name, prompt) {
    if (!name || !prompt) {
        showToast('Please provide both a name and prompt', 'error');
        return;
    }

    const key = name.toLowerCase().replace(/\s+/g, '-');
    suggestionsState.customCategories.set(key, prompt);
    saveCustomCategories();
    renderCategoriesList();
    showToast(`Category "${name}" added`, 'success');
}

function removeCategory(key) {
    if (suggestionsState.customCategories.delete(key)) {
        saveCustomCategories();
        renderCategoriesList();
        showToast('Category removed', 'success');
    }
}

function editCategoryPrompt(key) {
    const existingPrompt = suggestionsState.customCategories.get(key) ||
        document.getElementById(`settings-prompt-${key}`)?.value || '';

    const newPrompt = prompt('Edit prompt for category:', existingPrompt);
    if (newPrompt !== null) {
        if (key === 'general' || key === 'plot-twist' || key === 'new-character') {
            const el = document.getElementById(`settings-prompt-${key}`);
            if (el) el.value = newPrompt;
        } else {
            suggestionsState.customCategories.set(key, newPrompt);
            saveCustomCategories();
        }
        renderCategoriesList();
        showToast('Category updated', 'success');
    }
}

function saveCustomCategories() {
    try {
        const data = Object.fromEntries(suggestionsState.customCategories);
        localStorage.setItem('suggestions_custom_categories', JSON.stringify(data));
    } catch (e) {
        console.error('Failed to save custom categories:', e);
    }
}

function loadCustomCategories() {
    try {
        const data = JSON.parse(localStorage.getItem('suggestions_custom_categories') || '{}');
        suggestionsState.customCategories = new Map(Object.entries(data));
    } catch (e) {
        console.error('Failed to load custom categories:', e);
    }
}

// ── Generate Button ──────────────────────────────────────────────────────────

function setupGenerateButton() {
    const generateBtn = document.getElementById('suggestions-generate-btn');
    if (!generateBtn) return;

    generateBtn.addEventListener('click', () => {
        fetchSuggestions();
    });
}

// ── Category Buttons ─────────────────────────────────────────────────────────

function setupCategoryButtons() {
    document.querySelectorAll('.suggestion-category-btn').forEach(btn => {
        btn.addEventListener('click', (e) => {
            e.stopPropagation();
            const category = btn.dataset.category;
            setSuggestionCategory(category);
        });
    });

    // Manage categories modal
    const manageModal = document.getElementById('manage-categories-modal');
    const manageClose = document.getElementById('manage-categories-close');
    const addCategoryBtn = document.getElementById('add-category-btn');

    if (manageClose) {
        manageClose.addEventListener('click', () => {
            manageModal.classList.remove('open');
            manageModal.setAttribute('aria-hidden', 'true');
            manageModal.inert = true;
        });
    }

    if (addCategoryBtn) {
        addCategoryBtn.addEventListener('click', () => {
            const nameInput = document.getElementById('new-category-name');
            const promptInput = document.getElementById('new-category-prompt');
            const name = nameInput?.value.trim();
            const prompt = promptInput?.value.trim();

            if (name && prompt) {
                addCategory(name, prompt);
                nameInput.value = '';
                promptInput.value = '';
            }
        });
    }
}

// ── Clear Recent Button ─────────────────────────────────────────────────────

function setupClearRecentButton() {
    const clearBtn = document.getElementById('suggestions-clear-recent');
    if (!clearBtn) return;

    clearBtn.addEventListener('click', (e) => {
        e.stopPropagation();
        clearRecentSuggestions();
    });
}

// ── Keyboard Navigation ─────────────────────────────────────────────────────

function setupKeyboardNav() {
    document.addEventListener('keydown', (e) => {
        if (!suggestionsState.expanded) return;

        const items = document.querySelectorAll('.suggestion-item');
        const activeItem = document.querySelector('.suggestion-item.active');
        let currentIndex = activeItem ? parseInt(activeItem.dataset.index, 10) : -1;

        if (e.key === 'ArrowDown') {
            e.preventDefault();
            items.forEach(item => {
                item.classList.remove('active');
                item.setAttribute('aria-selected', 'false');
            });
            const nextIndex = Math.min(currentIndex + 1, items.length - 1);
            if (nextIndex >= 0) {
                items[nextIndex].classList.add('active');
                items[nextIndex].setAttribute('aria-selected', 'true');
                items[nextIndex].focus();
                items[nextIndex].scrollIntoView({ block: 'nearest' });
            }
        } else if (e.key === 'ArrowUp') {
            e.preventDefault();
            items.forEach(item => {
                item.classList.remove('active');
                item.setAttribute('aria-selected', 'false');
            });
            const prevIndex = Math.max(currentIndex - 1, 0);
            if (prevIndex >= 0) {
                items[prevIndex].classList.add('active');
                items[prevIndex].setAttribute('aria-selected', 'true');
                items[prevIndex].focus();
                items[prevIndex].scrollIntoView({ block: 'nearest' });
            }
        } else if (e.key === 'Enter' && activeItem) {
            e.preventDefault();
            const index = activeItem.dataset.index || activeItem.dataset.recentIndex;
            useSuggestion(parseInt(index, 10));
        } else if (e.key === 'Tab' && activeItem) {
            e.preventDefault();
            const index = activeItem.dataset.index || activeItem.dataset.recentIndex;
            useSuggestion(parseInt(index, 10));
        } else if (e.key === 'Escape') {
            suggestionsState.expanded = false;
            updateDropdownUI();
            document.getElementById('suggestions-toggle')?.focus();
        }
    });
}

// ── Offline Detection ───────────────────────────────────────────────────────

function setupOfflineDetection() {
    window.addEventListener('online', () => {
        suggestionsState.isOffline = false;
        if (suggestionsState.expanded) {
            showToast('Back online!', 'success');
        }
    });

    window.addEventListener('offline', () => {
        suggestionsState.isOffline = true;
        showToast('You are offline', 'warning');
    });
}

// ── Click Outside to Close ──────────────────────────────────────────────────

function setupClickOutside() {
    document.addEventListener('click', (e) => {
        const dropdown = document.getElementById('suggestions-dropdown');
        const toggleBtn = document.getElementById('suggestions-toggle');

        if (!dropdown || !toggleBtn) return;

        const isClickInside = dropdown.contains(e.target) || toggleBtn.contains(e.target);

        if (!isClickInside && suggestionsState.expanded) {
            suggestionsState.expanded = false;
            updateDropdownUI();
        }
    });
}

// ── Initialization ───────────────────────────────────────────────────────────

function setupTagCloudUI() {
    const searchInput = document.getElementById('suggestion-search-input');
    if (searchInput) {
        searchInput.addEventListener('input', () => {
            applySearchFilter();
        });
    }

    document.querySelectorAll('.category-group-header').forEach(header => {
        header.addEventListener('click', () => {
            const expanded = header.getAttribute('aria-expanded') === 'true';
            header.setAttribute('aria-expanded', String(!expanded));
        });
    });
}

export function initSuggestionsDropdown() {
    loadCustomCategories();
    setupGenerateButton();
    setupCategoryButtons();
    setupClearRecentButton();
    setupKeyboardNav();
    setupClickOutside();
    setupOfflineDetection();
    setupTagCloudUI();
    updateDropdownUI();
    renderRecentSuggestions();
}
