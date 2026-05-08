// ── Suggestions (Dropdown) ──────────────────────────────────────────────────
// Dropdown menu with AI-generated suggestions (General, Plot Twist, New Character).

import { activeChatTab } from './chat-state.js';
import { escapeHtml } from '../core/sanitization.js';
import { showToast } from './toast.js';

let suggestionsState = {
    expanded: false,
    currentCategory: 'general',
    isLoading: false,
    suggestions: [],
};

// ── Dropdown Toggle ──────────────────────────────────────────────────────────

export function toggleSuggestionsDropdown() {
    suggestionsState.expanded = !suggestionsState.expanded;
    updateDropdownUI();
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

    // Update category buttons
    categoryBtns.forEach(btn => {
        const category = btn.dataset.category;
        btn.classList.toggle('active', category === suggestionsState.currentCategory);
    });

    if (listContainer) {
        renderSuggestionsList();
    }
}

// ── Category Switching ───────────────────────────────────────────────────────

export function setSuggestionCategory(category) {
    suggestionsState.currentCategory = category;
    suggestionsState.suggestions = [];
    updateDropdownUI();
    fetchSuggestions();
}

// ── Suggestions List Rendering ───────────────────────────────────────────────

function renderSuggestionsList() {
    const container = document.getElementById('suggestions-list');
    if (!container) return;

    if (suggestionsState.isLoading) {
        container.innerHTML = `
            <div class="suggestions-loading">
                <div class="spinner"></div>
                <p>Generating suggestions...</p>
            </div>
        `;
        return;
    }

    const suggestions = suggestionsState.suggestions;

    if (suggestions.length === 0) {
        container.innerHTML = `
            <div class="suggestions-empty-state">
                <p>No suggestions yet</p>
                <p class="text-sm">Click "Generate" to get AI-powered suggestions based on your conversation.</p>
            </div>
        `;
        return;
    }

    // eslint-disable-next-line no-unsanitized/property -- User content escaped via escapeHtml()
    container.innerHTML = suggestions.map((suggestion, index) => `
        <div class="suggestion-item" data-index="${index}">
            <div class="suggestion-content">${escapeHtml(suggestion)}</div>
            <button class="suggestion-btn suggestion-btn-use" title="Use this suggestion">Use</button>
        </div>
    `).join('');

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

// ── Fetch Suggestions from API ───────────────────────────────────────────────

async function fetchSuggestions() {
    const tab = activeChatTab();
    if (!tab) return;

    suggestionsState.isLoading = true;
    updateDropdownUI();

    try {
        const response = await fetch('/api/chat/suggestions', {
            method: 'POST',
            headers: {
                'Content-Type': 'application/json',
            },
            body: JSON.stringify({
                tab_id: tab.id,
                category: suggestionsState.currentCategory,
                count: 5,
                context_depth: 10,
            }),
        });

        if (!response.ok) {
            throw new Error(`HTTP ${response.status}: ${response.statusText}`);
        }

        const data = await response.json();
        suggestionsState.suggestions = data.suggestions || [];
        suggestionsState.isLoading = false;
        updateDropdownUI();
    } catch (error) {
        suggestionsState.isLoading = false;
        suggestionsState.suggestions = [];
        updateDropdownUI();
        showToast(`Failed to fetch suggestions: ${error.message}`, 'error');
    }
}

// ── Use Suggestion ───────────────────────────────────────────────────────────

function useSuggestion(index) {
    const suggestion = suggestionsState.suggestions[index];
    if (!suggestion) return;

    // Dispatch event for chat-input to handle
    window.dispatchEvent(new CustomEvent('suggestionSelected', {
        detail: { text: suggestion },
    }));

    // Close dropdown
    suggestionsState.expanded = false;
    updateDropdownUI();
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
        btn.addEventListener('click', () => {
            const category = btn.dataset.category;
            setSuggestionCategory(category);
        });
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
            items.forEach(item => item.classList.remove('active'));
            const nextIndex = Math.min(currentIndex + 1, items.length - 1);
            if (nextIndex >= 0) {
                items[nextIndex].classList.add('active');
                items[nextIndex].scrollIntoView({ block: 'nearest' });
            }
        } else if (e.key === 'ArrowUp') {
            e.preventDefault();
            items.forEach(item => item.classList.remove('active'));
            const prevIndex = Math.max(currentIndex - 1, 0);
            if (prevIndex >= 0) {
                items[prevIndex].classList.add('active');
                items[prevIndex].scrollIntoView({ block: 'nearest' });
            }
        } else if (e.key === 'Enter' && activeItem) {
            e.preventDefault();
            useSuggestion(currentIndex);
        } else if (e.key === 'Escape') {
            suggestionsState.expanded = false;
            updateDropdownUI();
        }
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

export function initSuggestionsDropdown() {
    setupGenerateButton();
    setupCategoryButtons();
    setupKeyboardNav();
    setupClickOutside();
    updateDropdownUI();
}
