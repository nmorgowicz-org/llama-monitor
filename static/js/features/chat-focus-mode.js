import { showToast } from './toast.js';

const STORAGE_KEY = 'llama-monitor-chat-focus-mode';
let focusModeActive = false;
let toastShown = false;

export function initChatFocusMode() {
    const beacon = document.getElementById('focus-mode-exit-beacon');
    const pill = document.getElementById('focus-mode-exit-pill');
    if (beacon) beacon.addEventListener('click', exitFocusMode);
    if (pill) pill.addEventListener('click', exitFocusMode);

    document.getElementById('chat-focus-mode-btn')
        ?.addEventListener('click', toggleFocusMode);

    // Restore persisted state silently (no toast on reload)
    if (localStorage.getItem(STORAGE_KEY) === '1') {
        enterFocusMode(true);
    }
}

export function enterFocusMode(silent = false) {
    focusModeActive = true;
    document.body.classList.add('chat-focus-mode');
    updateFocusModeButton();
    localStorage.setItem(STORAGE_KEY, '1');

    if (!silent && !toastShown) {
        toastShown = true;
        showToast('Focus Mode', 'info', 'Click again, hover top edge, or press ⌘⇧F to exit', { duration: 4000 });
    }
}

export function exitFocusMode() {
    focusModeActive = false;
    document.body.classList.remove('chat-focus-mode');
    updateFocusModeButton();
    localStorage.removeItem(STORAGE_KEY);
}

export function toggleFocusMode() {
    if (focusModeActive) {
        exitFocusMode();
    } else {
        enterFocusMode();
    }
}

export function isFocusModeActive() {
    return focusModeActive;
}

function updateFocusModeButton() {
    const btn = document.getElementById('chat-focus-mode-btn');
    if (!btn) return;
    btn.classList.toggle('active', focusModeActive);
    btn.setAttribute('aria-pressed', String(focusModeActive));
    btn.setAttribute('title', focusModeActive ? 'Exit Focus Mode (⌘⇧F)' : 'Focus Mode (⌘⇧F)');
}
