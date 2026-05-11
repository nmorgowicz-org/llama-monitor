// ── Suggestions (Dropdown) ──────────────────────────────────────────────────
// Dropdown menu with AI-generated suggestions (General, Plot Twist, New Character).

import { activeChatTab, persistChatTabs } from './chat-state.js';
import { chat } from '../core/app-state.js';
import { escapeHtml } from '../core/format.js';
import { showToast, showToastWithActions } from './toast.js';
import { toggleExplicitMode } from './chat-templates.js';

const CATEGORY_META = {
    general: { label: 'General', description: 'Versatile next-step prompts that fit almost any conversation.' },
    'plot-twist': { label: 'Plot Twist', description: 'Unexpected reversals, reveals, and pressure spikes that raise the stakes.' },
    'new-character': { label: 'New Character', description: 'Fresh entrants who create conflict, chemistry, or new information.' },
    director: { label: 'Director', description: 'High-level scene direction, pacing shifts, and cinematic steering.' },
    action: { label: 'Action', description: 'Momentum, danger, movement, and immediate physical stakes.' },
    comedy: { label: 'Comedy', description: 'Humor beats, awkward pivots, and playful escalation.' },
    fantasy: { label: 'Fantasy', description: 'Magic, myth, wonder, and worldbuilding-driven next steps.' },
    horror: { label: 'Horror', description: 'Dread, menace, and unsettling turns that tighten the atmosphere.' },
    mystery: { label: 'Mystery', description: 'Clues, suspicion, revelations, and investigative momentum.' },
    noir: { label: 'Noir', description: 'Shadowy motives, sharp dialogue, and morally messy developments.' },
    romance: { label: 'Romance', description: 'Chemistry, vulnerability, longing, and emotional tension.' },
    'sci-fi': { label: 'Sci-Fi', description: 'Futuristic complications, speculative ideas, and tech-driven stakes.' },
    thriller: { label: 'Thriller', description: 'Urgency, pressure, danger, and escalating consequences.' },
    character: { label: 'Character', description: 'Choices and beats that reveal personality, desire, or conflict.' },
    explicit: { label: 'Explicit', description: 'Unfiltered prompts for adult-only scenes when explicit mode is enabled.' },
};

let suggestionsState = {
    expanded: false,
    currentCategory: 'general',
    setupCollapsed: false,
    isLoading: false,
    hasGenerated: false,
    suggestions: [],
    recentSuggestions: [],
    customCategories: new Map(),
    retryCount: 0,
    maxRetries: 3,
    lastError: null,
    isOffline: false,
    previewCategory: null,
};

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
    const wrapper = toggleBtn?.closest('.guided-tool');
    const categoryBtns = document.querySelectorAll('.suggestion-category-btn');
    const listContainer = document.getElementById('suggestions-list');
    const explicitGroup = document.getElementById('suggestions-explicit-group');
    const status = document.getElementById('suggestions-toggle-status');
    const description = document.getElementById('suggestions-category-description');
    const preview = document.getElementById('suggestions-category-preview');
    const setupToggle = document.getElementById('suggestions-view-toggle');
    const meta = CATEGORY_META[suggestionsState.currentCategory] || {
        label: suggestionsState.currentCategory,
        description: 'Generate suggestions for the current conversation.',
    };
    const previewMeta = CATEGORY_META[suggestionsState.previewCategory || suggestionsState.currentCategory] || meta;

    if (!dropdown || !toggleBtn) return;

    if (suggestionsState.expanded) {
        dropdown.classList.add('dropdown-expanded');
        toggleBtn.classList.add('active');
        toggleBtn.setAttribute('aria-expanded', 'true');
        wrapper?.classList.add('is-open');
    } else {
        dropdown.classList.remove('dropdown-expanded');
        toggleBtn.classList.remove('active');
        toggleBtn.setAttribute('aria-expanded', 'false');
        wrapper?.classList.remove('is-open');
    }

    dropdown.classList.toggle('setup-collapsed', suggestionsState.setupCollapsed);

    if (status) {
        status.textContent = suggestionsState.isLoading ? 'Loading' : meta.label;
        status.hidden = !suggestionsState.expanded;
    }
    if (description) {
        description.hidden = !suggestionsState.isLoading;
        description.textContent = suggestionsState.isLoading
            ? `Generating ${meta.label.toLowerCase()} ideas from the current conversation…`
            : '';
    }
    if (preview) {
        preview.textContent = `${previewMeta.label}: ${previewMeta.description}`;
    }
    if (setupToggle) {
        setupToggle.textContent = suggestionsState.setupCollapsed ? 'Show Setup' : 'Hide Setup';
        setupToggle.setAttribute('aria-pressed', suggestionsState.setupCollapsed ? 'true' : 'false');
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
        const categoryMeta = CATEGORY_META[category] || {
            label: btn.textContent.trim(),
            description: 'Generate suggestions for the current conversation.',
        };
        btn.classList.toggle('active', category === suggestionsState.currentCategory);
        btn.setAttribute('title', `${categoryMeta.label}: ${categoryMeta.description}`);
        btn.setAttribute('aria-label', `${categoryMeta.label}. ${categoryMeta.description}`);
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

    suggestionsState.currentCategory = category;
    suggestionsState.previewCategory = category;
    suggestionsState.hasGenerated = false;
    suggestionsState.suggestions = [];
    suggestionsState.setupCollapsed = false;
    updateDropdownUI();
}

// ── Suggestions List Rendering ───────────────────────────────────────────────

function renderSuggestionsList() {
    const container = document.getElementById('suggestions-list');
    if (!container) return;

    if (!suggestionsState.isLoading && !suggestionsState.hasGenerated) {
        container.innerHTML = '';
        return;
    }

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
                <p class="text-sm">Choose a category, then generate tailored prompts for this conversation.</p>
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
            <div class="suggestion-main">
                ${description ? `<div class="suggestion-title">${escapeHtml(title)}</div>` : ''}
                <div class="suggestion-content">${escapeHtml(description || title)}</div>
            </div>
            <div class="suggestion-actions">
                <button class="suggestion-btn suggestion-btn-append" data-mode="append" aria-label="Append suggestion: ${escapeHtml(title)}">Append</button>
                <button class="suggestion-btn suggestion-btn-use" data-mode="replace" aria-label="Replace input with suggestion: ${escapeHtml(title)}">Replace</button>
            </div>
        </div>
    `;
    }).join('');

    // Attach use handlers
    container.querySelectorAll('.suggestion-btn').forEach(btn => {
        btn.addEventListener('click', (e) => {
            e.stopPropagation();
            const item = e.target.closest('.suggestion-item');
            const index = parseInt(item.dataset.index, 10);
            useSuggestion(index, btn.dataset.mode || 'replace');
        });
    });
}

function renderRecentSuggestions() {
    const container = document.getElementById('suggestions-recent');
    const list = document.getElementById('suggestions-recent-list');
    const tab = activeChatTab();

    if (!container || !list || !tab) return;

    const recent = tab._suggestion_history || [];

    if (!suggestionsState.hasGenerated || recent.length === 0) {
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
        detail: { text: suggestion, mode: 'replace' },
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

function buildSuggestionContext(tab) {
    return {
        messages: (tab.messages || []).map(message => ({
            role: message.role,
            content: message.content,
        })),
        system_prompt: tab.system_prompt || '',
        context_notes: (tab.context_notes || [])
            .filter(note => note?.content?.trim())
            .map(note => ({
                section: note.section || 'context',
                content: note.content,
                created_at: note.created_at || 0,
            })),
        quick_guide_active: (tab.quick_guide_active || '').trim(),
    };
}

async function fetchSuggestions() {
    const tab = activeChatTab();
    if (!tab) return;

    if (chat.busy) {
        showToast('Wait for the current reply to finish before generating suggestions', 'warning');
        return;
    }

    if (suggestionsState.isOffline) {
        showToast('Offline: suggestions unavailable until connection restored', 'error');
        return;
    }

    suggestionsState.isLoading = true;
    suggestionsState.retryCount = 0;
    suggestionsState.hasGenerated = false;
    suggestionsState.setupCollapsed = true;
    updateDropdownUI();

    // Suggestions API currently reconstructs context from persisted tab state,
    // so flush first to avoid missing the latest assistant turn or mode changes.
    try {
        await persistChatTabs();
    } catch {
        suggestionsState.isLoading = false;
        suggestionsState.setupCollapsed = false;
        updateDropdownUI();
        showToast('Failed to save the latest chat state before generating suggestions', 'error');
        return;
    }

    const settings = getSettingsValue();
    const contextDepth = settings.context_depth ?? 10;
    const suggestionCount = settings.suggestion_count ?? 5;
    const prompts = settings.suggestion_prompts ?? {};
    const promptValue = prompts[suggestionsState.currentCategory];
    const prompt = typeof promptValue === 'string' && promptValue.trim()
        ? promptValue
        : null;

    await requestSuggestions({
        tabId: tab.id,
        category: suggestionsState.currentCategory,
        contextDepth,
        suggestionCount,
        prompt,
        context: buildSuggestionContext(tab),
        retryAttempt: 0,
    });
}

async function requestSuggestions({ tabId, category, contextDepth, suggestionCount, prompt, context, retryAttempt }) {
    suggestionsState.retryCount = retryAttempt;

    try {
        const response = await fetch('/api/chat/suggestions', {
            method: 'POST',
            headers: {
                'Content-Type': 'application/json',
            },
            body: JSON.stringify({
                tab_id: tabId,
                category,
                context_depth: contextDepth,
                count: suggestionCount,
                ...context,
                ...(prompt ? { prompt } : {}),
            }),
        });

        if (!response.ok) {
            throw new Error(`HTTP ${response.status}: ${response.statusText}`);
        }

        const data = await response.json();
        suggestionsState.suggestions = data.suggestions || [];
        suggestionsState.isLoading = false;
        suggestionsState.hasGenerated = true;
        suggestionsState.lastError = null;
        updateDropdownUI();
    } catch (error) {
        suggestionsState.lastError = error;
        if (retryAttempt < suggestionsState.maxRetries) {
            const nextAttempt = retryAttempt + 1;
            suggestionsState.retryCount = nextAttempt;
            const delay = 1000 * nextAttempt;
            setTimeout(() => {
                requestSuggestions({
                    tabId,
                    category,
                    contextDepth,
                    suggestionCount,
                    prompt,
                    context,
                    retryAttempt: nextAttempt,
                });
            }, delay);
            showToast(`Retrying... (${nextAttempt}/${suggestionsState.maxRetries})`, 'warning');
        } else {
            suggestionsState.isLoading = false;
            suggestionsState.hasGenerated = false;
            suggestionsState.setupCollapsed = false;
            suggestionsState.suggestions = [];
            updateDropdownUI();
            showToast(`Failed to fetch suggestions: ${error.message}`, 'error');
        }
    }
}

// ── Use Suggestion ───────────────────────────────────────────────────────────

function useSuggestion(index, mode = 'replace') {
    const suggestion = suggestionsState.suggestions[index];
    if (!suggestion) return;

    // Track in history
    addRecentSuggestion(suggestion);

    // Dispatch event for chat-input to handle
    window.dispatchEvent(new CustomEvent('suggestionSelected', {
        detail: { text: suggestion, mode },
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

    const setupToggle = document.getElementById('suggestions-view-toggle');
    if (setupToggle) {
        setupToggle.addEventListener('click', (e) => {
            e.stopPropagation();
            suggestionsState.setupCollapsed = !suggestionsState.setupCollapsed;
            updateDropdownUI();
        });
    }
}

// ── Category Buttons ─────────────────────────────────────────────────────────

function setupCategoryButtons() {
    document.querySelectorAll('.suggestion-category-btn').forEach(btn => {
        const previewCategory = () => {
            suggestionsState.previewCategory = btn.dataset.category;
            updateDropdownUI();
        };
        const resetPreviewCategory = () => {
            suggestionsState.previewCategory = suggestionsState.currentCategory;
            updateDropdownUI();
        };

        btn.addEventListener('mouseenter', previewCategory);
        btn.addEventListener('focus', previewCategory);
        btn.addEventListener('mouseleave', resetPreviewCategory);
        btn.addEventListener('blur', resetPreviewCategory);
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

    const manageBtn = document.getElementById('suggestions-manage-btn');
    if (manageBtn) {
        manageBtn.addEventListener('click', (e) => {
            e.stopPropagation();
            manageCategories();
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
        const searchInput = document.getElementById('suggestion-search-input');
        if (document.activeElement === searchInput && !['Escape'].includes(e.key)) return;

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
            useSuggestion(parseInt(index, 10), 'replace');
        } else if (e.key === 'Tab' && activeItem) {
            e.preventDefault();
            const index = activeItem.dataset.index || activeItem.dataset.recentIndex;
            useSuggestion(parseInt(index, 10), 'replace');
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
            suggestionsState.previewCategory = suggestionsState.currentCategory;
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
    window.addEventListener('activeTabChanged', updateDropdownUI);
    window.addEventListener('explicitModeChanged', updateDropdownUI);
    updateDropdownUI();
    renderRecentSuggestions();
}
