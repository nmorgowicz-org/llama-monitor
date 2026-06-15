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

document.addEventListener('click', event => {
    if (!event.target.closest('.nav-user') && !event.target.closest('#nav-user-menu-items')) {
        closeUserMenu();
    }
});

document.addEventListener('keydown', event => {
    if (event.key === 'Escape') {
        closeUserMenu();
    }
});

// ── User Profile ──────────────────────────────────────────────────────────────

function openUserProfile(event) {
    event?.preventDefault();
    closeUserMenu();
    openUserPreferencesModal();
    showToast('Profile is local-only for now. Preferences are available here.', 'info');
}

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

    localStorage.setItem('llama-monitor-preferences', JSON.stringify({
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
    const profileBtn = document.getElementById('user-menu-profile');
    if (profileBtn) profileBtn.addEventListener('click', (e) => openUserProfile(e));

    const prefsBtn = document.getElementById('user-menu-preferences');
    if (prefsBtn) prefsBtn.addEventListener('click', (e) => openUserPreferencesModal(e));

    const themeBtn = document.getElementById('user-menu-theme');
    if (themeBtn) themeBtn.addEventListener('click', (e) => toggleTheme(e));

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

    _loadSavedPreferences();
}
