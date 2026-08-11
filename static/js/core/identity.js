// Public product identity and frozen browser-compatibility identifiers.
//
// The legacy storage keys are intentionally never renamed during the 2.x
// line. They are part of the user's durable browser state, not product copy.
export const PRODUCT_NAME = 'Local LLM Foundry';
export const PRODUCT_SHORT_NAME = 'Foundry';
export const CLI_NAME = 'local-llm-foundry';
export const LEGACY_CLI_NAME = 'llama-monitor';
export const REPOSITORY_URL = 'https://github.com/nmorgowicz-org/local-llm-foundry';

export const LEGACY_STORAGE_KEYS = Object.freeze({
    preferences: 'llama-monitor-preferences',
    lastEndpoint: 'llama-monitor-last-endpoint',
    lastSession: 'llama-monitor-last-session',
    previousPosition: 'llama-monitor-previous-position',
    chatStyle: 'llama-monitor-chat-style',
    chatFont: 'llama-monitor-chat-font',
    enterToSend: 'llama-monitor-enter-to-send',
    chatTelemetryPinned: 'llama-monitor-chat-telemetry-pinned',
    dateFormat: 'llama-monitor-date-format',
    chatFocusMode: 'llama-monitor-chat-focus-mode',
    gpuViz: 'llama-monitor-gpu-viz',
    systemViz: 'llama-monitor-system-viz',
    logFontSize: 'llama-monitor-log-font-size',
    logTailEnabled: 'llama-monitor-log-tail-enabled',
    logTailLines: 'llama-monitor-log-tail-lines',
    modelsPreferences: 'llama-monitor-models-prefs',
    groupByFamily: 'llama-monitor-group-by-family',
    presetSort: 'llama-monitor-preset-sort',
    notifications: 'llama-monitor-notifications',
    sidebarWidth: 'llama_monitor_sidebar_width',
    sidebarExpanded: 'llama_monitor_sidebar_expanded',
    contextNotesIntroHidden: 'llama_monitor_context_notes_intro_hidden',
});

export function applyProductIdentity(root = typeof document !== 'undefined' ? document : null) {
    if (!root) return;
    if (root.title !== PRODUCT_NAME) root.title = PRODUCT_NAME;
    root.querySelectorAll('[data-product-name]').forEach((element) => {
        element.textContent = PRODUCT_NAME;
    });
    root.querySelectorAll('[data-product-short-name]').forEach((element) => {
        element.textContent = PRODUCT_SHORT_NAME;
    });
}

if (typeof window !== 'undefined') {
    window.__LOCAL_LLM_FOUNDRY_IDENTITY = Object.freeze({
        PRODUCT_NAME,
        PRODUCT_SHORT_NAME,
        CLI_NAME,
        LEGACY_CLI_NAME,
        REPOSITORY_URL,
        LEGACY_STORAGE_KEYS,
    });
}
