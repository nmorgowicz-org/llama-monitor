// ── User Menu ─────────────────────────────────────────────────────────────────
// User menu, profile, preferences modal, theme toggle.

import { showToast } from './toast.js';
import { applyChatStyle, getEnterToSend, setEnterToSend } from './chat-params.js';
import { openKeyboardShortcutsModal } from './shortcuts.js';
import { saveSettings } from './settings.js';

// ── User Menu ─────────────────────────────────────────────────────────────────

function _positionUserMenu() {
    const menu = document.getElementById('nav-user-menu-items');
    const btn = document.getElementById('nav-user-btn');
    if (!menu || !btn) return;
    const rect = btn.getBoundingClientRect();
    menu.style.top = (rect.bottom + 8) + 'px';
    menu.style.right = (window.innerWidth - rect.right) + 'px';
}

function toggleUserMenu(event) {
    event.preventDefault();
    event.stopPropagation();
    const menu = document.getElementById('nav-user-menu-items');
    const btn = document.getElementById('nav-user-btn');
    if (!menu || !btn) return;
    const nextOpen = !menu.classList.contains('open');
    if (nextOpen) _positionUserMenu();
    menu.classList.toggle('open', nextOpen);
    btn.setAttribute('aria-expanded', nextOpen ? 'true' : 'false');
}

function closeUserMenu() {
    document.getElementById('nav-user-menu-items')?.classList.remove('open');
    document.getElementById('nav-user-btn')?.setAttribute('aria-expanded', 'false');
}

function closePaletteDropdown() {
    const dropdown = document.getElementById('nav-palette-dropdown');
    const btn = document.getElementById('nav-palette-btn');
    if (dropdown) {
        dropdown.classList.remove('open');
        dropdown.style.display = 'none';
        // Move back under its original parent so page load layout is stable
        const parent = document.getElementById('nav-palette-selector');
        if (parent && dropdown.parentNode !== parent) {
            parent.appendChild(dropdown);
            // Reset to CSS default positioning
            dropdown.style.position = '';
            dropdown.style.top = '';
            dropdown.style.bottom = '';
            dropdown.style.left = '';
            dropdown.style.right = '';
        }
    }
    if (btn) btn.setAttribute('aria-expanded', 'false');
}

function togglePaletteDropdown(event) {
    event?.preventDefault();
    event?.stopPropagation();
    const dropdown = document.getElementById('nav-palette-dropdown');
    const btn = document.getElementById('nav-palette-btn');
    if (!dropdown || !btn) return;
    const isOpen = dropdown.classList.contains('open') || dropdown.style.display === 'block';
    const nextOpen = !isOpen;

    if (nextOpen) {
        // Show briefly to get correct size before measuring
        dropdown.style.display = 'block';
        dropdown.classList.add('open');
        btn.setAttribute('aria-expanded', 'true');

        // Move to body so it escapes any stacking/clip context
        const btnRect = btn.getBoundingClientRect();
        const winW = window.innerWidth || document.documentElement.clientWidth;
        const winH = window.innerHeight || document.documentElement.clientHeight;

        dropdown.style.position = 'fixed';
        dropdown.style.left = '';
        dropdown.style.top = '';
        dropdown.style.bottom = 'auto';
        dropdown.style.right = 'auto';

        // Align its right edge with the button's right edge
        dropdown.style.right = (winW - btnRect.right) + 'px';

        requestAnimationFrame(() => {
            const dRect = dropdown.getBoundingClientRect();
            const spaceBelow = winH - btnRect.bottom;
            if (dRect.height > spaceBelow - 8) {
                // Open upward when there isn't enough room below
                dropdown.style.top = (btnRect.top - dRect.height - 8) + 'px';
            } else {
                // Open downward
                dropdown.style.top = (btnRect.bottom + 8) + 'px';
            }
        });

        // Ensure it's in body after measurements
        if (dropdown.parentNode !== document.body) {
            document.body.appendChild(dropdown);
        }
    } else {
        closePaletteDropdown();
    }
}

function setPalette(palette) {
    const html = document.documentElement;
    html.classList.add('palette-changing');
    setTimeout(() => html.classList.remove('palette-changing'), 350);
    if (palette && palette !== 'carbon-mint') {
        html.dataset.palette = palette;
    } else {
        delete html.dataset.palette;
    }
    // Update active swatch
    document.querySelectorAll('#nav-palette-dropdown .palette-swatch').forEach(btn => {
        const matches = (btn.dataset.palette || '') === palette;
        btn.classList.toggle('active', matches);
        btn.setAttribute('aria-pressed', String(matches));
    });
    // Persist palette to localStorage
    try {
        const existing = JSON.parse(localStorage.getItem('llama-monitor-preferences') || '{}');
        localStorage.setItem('llama-monitor-preferences', { ...existing, palette });
    } catch (_) {}
    // Close dropdown after selection
    closePaletteDropdown();
    showToast('Palette set to ' + (palette || 'Carbon Mint'), 'success');
}

document.addEventListener('click', event => {
    if (!event.target.closest('.nav-user') && !event.target.closest('#nav-user-menu-items')) {
        closeUserMenu();
    }
    if (!event.target.closest('.nav-palette-selector') && !event.target.closest('#nav-palette-dropdown')) {
        closePaletteDropdown();
    }
});

document.addEventListener('keydown', event => {
    if (event.key === 'Escape') {
        closeUserMenu();
        closePaletteDropdown();
    }
});

// ── User Preferences Modal ────────────────────────────────────────────────────

function openUserPreferencesModal(event) {
    event?.preventDefault();
    closeUserMenu();
    const enterCheckbox = document.getElementById('pref-enter-to-send');
    if (enterCheckbox) enterCheckbox.checked = getEnterToSend();
    document.getElementById('user-preferences-modal')?.classList.add('open');
}

function closeUserPreferencesModal() {
    document.getElementById('user-preferences-modal')?.classList.remove('open');
}

function saveUserPreferences() {
    const theme = document.getElementById('pref-theme-mode')?.value || 'dark';
    const fontScale = document.getElementById('pref-font-scale')?.value || '1';
    const spacingScale = document.getElementById('pref-spacing-scale')?.value || '1';
    const chatStyle = document.getElementById('pref-chat-style')?.value || 'rounded';
    const enterToSendChecked = document.getElementById('pref-enter-to-send')?.checked !== false;

    applyThemePreference(theme);
    document.documentElement.style.fontSize = (Number(fontScale) * 16) + 'px';
    document.documentElement.style.setProperty('--gap-md', (Number(spacingScale) * 16) + 'px');

    // Chat style/font/spacing/theme remain device-local on purpose; only
    // continuity-sensitive workflow prefs get promoted into shared settings.
    applyChatStyle(chatStyle);
    localStorage.setItem('llama-monitor-chat-style', chatStyle);

    setEnterToSend(enterToSendChecked);
    saveSettings();

    // Preserve other prefs (e.g. palette) that are managed elsewhere
    const existing = JSON.parse(localStorage.getItem('llama-monitor-preferences') || '{}');
    localStorage.setItem('llama-monitor-preferences', JSON.stringify({
        ...existing,
        theme,
        fontScale,
        spacingScale,
    }));

    closeUserPreferencesModal();
    showToast('Preferences saved', 'success');
}

// ── Theme ─────────────────────────────────────────────────────────────────────

function applyThemePreference(theme) {
    const effectiveTheme = theme === 'auto'
        ? (window.matchMedia('(prefers-color-scheme: light)').matches ? 'light' : 'dark')
        : theme;
    document.documentElement.dataset.theme = effectiveTheme;
}

function toggleTheme(event) {
    event?.preventDefault();
    closeUserMenu();
    const current = document.documentElement.dataset.theme === 'light' ? 'light' : 'dark';
    const next = current === 'light' ? 'dark' : 'light';
    document.documentElement.dataset.theme = next;
    const pref = document.getElementById('pref-theme-mode');
    if (pref) pref.value = next;
    // Persist so the next page load keeps the choice
    try {
        const existing = JSON.parse(localStorage.getItem('llama-monitor-preferences') || '{}');
        localStorage.setItem('llama-monitor-preferences', JSON.stringify({ ...existing, theme: next }));
    } catch (_) {}
    showToast('Theme set to ' + next, 'success');
}

// ── User Help ─────────────────────────────────────────────────────────────────

function openUserHelp(event) {
    event?.preventDefault();
    closeUserMenu();
    openKeyboardShortcutsModal();
}

// ── Logout ────────────────────────────────────────────────────────────────────

async function logoutUser(event) {
    event?.preventDefault();
    closeUserMenu();
    if (typeof window.logoutCurrentUser === 'function') {
        await window.logoutCurrentUser();
        return;
    }
    showToast('No signed-in account is configured for this local app.', 'info');
}

// ── Load saved preferences on import ──────────────────────────────────────────

function _loadSavedPreferences() {
    try {
        const savedPreferences = JSON.parse(localStorage.getItem('llama-monitor-preferences') || 'null');
        if (savedPreferences) {
            applyThemePreference(savedPreferences.theme || 'dark');
            // Apply color palette before first paint to avoid flash
            const palette = savedPreferences.palette || '';
            if (palette && palette !== 'carbon-mint') {
                document.documentElement.dataset.palette = palette;
            }
            // Sync palette swatch UI
            const savedPalette = savedPreferences.palette || '';
            document.querySelectorAll('#nav-palette-dropdown .palette-swatch').forEach(btn => {
                const matches = (btn.dataset.palette || '') === savedPalette;
                btn.classList.toggle('active', matches);
                btn.setAttribute('aria-pressed', String(matches));
            });
            if (savedPreferences.fontScale) {
                document.documentElement.style.fontSize = (Number(savedPreferences.fontScale) * 16) + 'px';
            }
            if (savedPreferences.spacingScale) {
                document.documentElement.style.setProperty('--gap-md', (Number(savedPreferences.spacingScale) * 16) + 'px');
            }
            // Apply chat-specific prefs once DOM is ready
            if (savedPreferences.timestamps || savedPreferences.msgWidth) {
                document.addEventListener('DOMContentLoaded', () => {
                    const chatPage = document.querySelector('.chat-page');
                    if (chatPage && savedPreferences.timestamps && savedPreferences.timestamps !== 'hover') {
                        chatPage.dataset.timestamps = savedPreferences.timestamps;
                    }
                    const chatMsgs = document.getElementById('chat-messages');
                    const widthMap = { narrow: '65%', normal: '82%', wide: '100%' };
                    if (chatMsgs && savedPreferences.msgWidth && savedPreferences.msgWidth !== 'normal') {
                        chatMsgs.style.setProperty('--chat-message-max-width', widthMap[savedPreferences.msgWidth] || '82%');
                    }
                }, { once: true });
            }
        }
    } catch (_) {}
}

// ── Public API ────────────────────────────────────────────────────────────────

export function initUserMenu() {
    // Portal the menu to <body> so it escapes the nav bar's backdrop-filter stacking context
    const menu = document.getElementById('nav-user-menu-items');
    if (menu) document.body.appendChild(menu);

    // Bind user menu toggle
    const userBtn = document.getElementById('nav-user-btn');
    if (userBtn) {
        userBtn.addEventListener('click', (e) => toggleUserMenu(e));
    }

    // Bind user menu items
    const prefsBtn = document.getElementById('user-menu-preferences');
    if (prefsBtn) prefsBtn.addEventListener('click', (e) => openUserPreferencesModal(e));

    const themeBtn = document.getElementById('user-menu-theme');
    if (themeBtn) themeBtn.addEventListener('click', (e) => toggleTheme(e));

    const navThemeToggle = document.getElementById('nav-theme-toggle');
    if (navThemeToggle) navThemeToggle.addEventListener('click', (e) => toggleTheme(e));

    const helpBtn = document.getElementById('user-menu-help');
    if (helpBtn) helpBtn.addEventListener('click', (e) => openUserHelp(e));

    const logoutBtn = document.getElementById('user-menu-logout');
    if (logoutBtn) logoutBtn.addEventListener('click', (e) => logoutUser(e));

    // Bind user preferences modal buttons
    const prefsClose = document.getElementById('user-prefs-close');
    if (prefsClose) prefsClose.addEventListener('click', closeUserPreferencesModal);

    const prefsCancel = document.getElementById('user-prefs-cancel');
    if (prefsCancel) prefsCancel.addEventListener('click', closeUserPreferencesModal);

    const prefsSave = document.getElementById('user-prefs-save');
    if (prefsSave) prefsSave.addEventListener('click', saveUserPreferences);

    // Bind palette quick selector
    const paletteBtn = document.getElementById('nav-palette-btn');
    if (paletteBtn) {
        paletteBtn.addEventListener('click', (e) => togglePaletteDropdown(e));
    }

    const paletteDropdown = document.getElementById('nav-palette-dropdown');
    if (paletteDropdown) {
        paletteDropdown.addEventListener('click', (e) => {
            const btn = e.target.closest('.palette-swatch');
            if (!btn) return;
            const palette = btn.dataset.palette || '';
            setPalette(palette);
        });
    }

    _loadSavedPreferences();
}
