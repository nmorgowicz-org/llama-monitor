// ── User Menu ─────────────────────────────────────────────────────────────────
// User menu, profile, preferences modal, theme toggle.

// ── User Menu ─────────────────────────────────────────────────────────────────

function toggleUserMenu(event) {
    event.preventDefault();
    event.stopPropagation();
    document.querySelector('.nav-user-menu')?.classList.toggle('open');
}

function closeUserMenu() {
    document.querySelector('.nav-user-menu')?.classList.remove('open');
}

document.addEventListener('click', event => {
    if (!event.target.closest('.nav-user')) {
        closeUserMenu();
    }
});

// ── User Profile ──────────────────────────────────────────────────────────────

function openUserProfile(event) {
    event?.preventDefault();
    closeUserMenu();
    openUserPreferencesModal();
    window.showToast('Profile is local-only for now. Preferences are available here.', 'info');
}

// ── User Preferences Modal ────────────────────────────────────────────────────

function openUserPreferencesModal(event) {
    event?.preventDefault();
    closeUserMenu();
    const enterCheckbox = document.getElementById('pref-enter-to-send');
    if (enterCheckbox) enterCheckbox.checked = window.enterToSend;
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

    window.applyChatStyle(chatStyle);
    localStorage.setItem('llama-monitor-chat-style', chatStyle);

    window.enterToSend = enterToSendChecked;
    localStorage.setItem('llama-monitor-enter-to-send', window.enterToSend ? 'true' : 'false');

    localStorage.setItem('llama-monitor-preferences', JSON.stringify({
        theme,
        fontScale,
        spacingScale,
    }));

    closeUserPreferencesModal();
    window.showToast('Preferences saved', 'success');
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
    window.showToast('Theme set to ' + next, 'success');
}

// ── User Help ─────────────────────────────────────────────────────────────────

function openUserHelp(event) {
    event?.preventDefault();
    closeUserMenu();
    window.openKeyboardShortcutsModal();
}

// ── Logout ────────────────────────────────────────────────────────────────────

function logoutUser(event) {
    event?.preventDefault();
    closeUserMenu();
    window.showToast('No signed-in account is configured for this local app.', 'info');
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
        }
    } catch (_) {}
}

// ── Public API ────────────────────────────────────────────────────────────────

export function initUserMenu() {
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
