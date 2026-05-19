// Observes .chat-main-area width and applies density classes to <body>.
// Density classes: shell-width-tight, shell-width-very-tight (none = comfortable).
// Integration: call initChatWidthObserver() from bootstrap.js after DOM ready.
// Focus mode integration: call pinComfortableDensity() / unpinDensity() from chat-focus-mode.js.

const THRESHOLDS = {
    veryTight: 520,
    tight: 1000,
};

const DENSITY_CLASSES = ['shell-width-tight', 'shell-width-very-tight'];

let observer = null;
let pinned = false;

export function initChatWidthObserver() {
    const target = document.getElementById('chat-main-area');
    if (!target || typeof ResizeObserver === 'undefined') return;

    observer = new ResizeObserver(entries => {
        if (pinned) return;
        const width = entries[0].contentRect.width;
        applyDensityClass(classForWidth(width));
    });

    observer.observe(target);
}

export function applyDensityClass(cls) {
    document.body.classList.remove(...DENSITY_CLASSES);
    if (cls) document.body.classList.add(cls);
}

export function pinComfortableDensity() {
    pinned = true;
    applyDensityClass('');
}

export function unpinDensity() {
    pinned = false;
    const target = document.getElementById('chat-main-area');
    if (target) {
        applyDensityClass(classForWidth(target.getBoundingClientRect().width));
    }
}

function classForWidth(width) {
    if (width < THRESHOLDS.veryTight) return 'shell-width-very-tight';
    if (width < THRESHOLDS.tight) return 'shell-width-tight';
    return '';
}
