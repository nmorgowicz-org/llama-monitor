// ── Centralized application state ──────────────────────────────────────────────
// Single source of truth for shared mutable state. Replaces ad hoc window.* and
// top-level let declarations scattered across app.js.

// ── Dashboard / Monitor ───────────────────────────────────────────────────────

/** Previous metric values for animation */
export const prevValues = {
    prompt: 0,
    generation: 0,
    contextPct: 0,
};

/** Time-series data for sparklines */
export const metricSeries = {
    prompt: [],
    generation: [],
    liveOutput: [],
};

/** Slot snapshots for GPU visualization */
export const slotSnapshots = new Map();

/** Request activity segments */
export const requestActivity = [];

/** Most recent completed tasks */
export const recentTasks = [];

/** Metric capabilities from backend */
export const metricCapabilities = {};

/** Live output rate tracker */
export const liveOutputTracker = {
    taskId: null,
    previousDecoded: null,
    previousMs: null,
    latestRate: 0,
    rates: [],
};

/** Last known server state from WebSocket */
export let lastServerState = null;

/** Last known llama metrics from WebSocket */
export let lastLlamaMetrics = null;
export let lastRapidMlxMetrics = null;
/** Normalized context capacity: actual loaded limit, not stale KV-only reports */
export let contextCapacityTokens = null;

/** Last known system metrics from WebSocket */
export let lastSystemMetrics = null;

/** Last known GPU metrics from WebSocket */
export let lastGpuMetrics = null;

/** Last known capabilities from backend */
export let lastCapabilities = null;

/** Current polling interval in ms */
export let currentPollInterval = 5000;

/** Snapshot of last GPU data for hardware viz rerender */
export let lastGpuData = null;

// ── Presets / Sessions ────────────────────────────────────────────────────────
// Container object so that reassignments (presets = [...]) are visible through
// ES module imports (live binding to the object, not the variable).

/** Presets/sessions state container — mutable properties, imported as a live reference */
export const sessionState = {
    /** Loaded presets from backend */
    presets: [],
    /** Loaded sessions from backend */
    sessions: [],
    /** Loaded preset collections from backend */
    collections: [],
    /** Currently active session ID */
    activeSessionId: 'default',
    /** Preset currently staged in the UI for edit/start actions */
    selectedPresetId: '',
    /** Preset attached to the active running spawn session, if any */
    activeSessionPresetId: '',
    /** Currently active session port */
    activeSessionPort: 8080,
    /** Whether the local server is running */
    serverRunning: false,
    /** Previous log length for incremental rendering */
    prevLogLen: 0,
    /** Previous backend log snapshot, used to detect fixed-size ring rotation */
    prevLogs: [],
};

// ── Remote Agent ──────────────────────────────────────────────────────────────
// Container object so that reassignments are visible through ES module imports.

/** Remote agent state container — mutable properties, imported as a live reference */
export const remoteAgent = {
    /** Whether a remote-agent operation is in progress */
    inProgress: false,
    /** SSH connection info for remote agent */
    sshConnection: null,
    /** Latest SSH host key from scan */
    latestHostKey: null,
};

/** Latest dashboard websocket snapshot */
export let wsData = null;

export function setWsData(data) { wsData = data; }
export function setLastServerState(v) { lastServerState = v; }
export function setLastLlamaMetrics(v) { lastLlamaMetrics = v; }
export function setLastRapidMlxMetrics(v) { lastRapidMlxMetrics = v; }
export function getLastRapidMlxMetrics() { return lastRapidMlxMetrics; }
export function setContextCapacityTokens(v) { contextCapacityTokens = v; }
export function setLastSystemMetrics(v) { lastSystemMetrics = v; }
export function setLastGpuMetrics(v) { lastGpuMetrics = v; }
export function setLastCapabilities(v) { lastCapabilities = v; }
export function setLastGpuData(v) { lastGpuData = v; }

// ── Settings ──────────────────────────────────────────────────────────────────

/**
 * Settings state container — mutable properties, imported as a live reference.
 * Shared workflow preferences live here because they round-trip through
 * `/api/settings`; device-local presentation choices stay in User Preferences.
 */
export const settingsState = {
    /** Whether settings modal has unsaved changes */
    isDirty: false,
    /** Timer ID for debounced settings save */
    saveTimer: null,
    /** Guided-generation: context notes enabled */
    enabled_context_notes: true,
    /** Guided-generation: suggestions enabled */
    enabled_suggestions: true,
    /** Guided-generation: quick guide enabled */
    enabled_quick_guide: true,
    /** Custom suggestion prompts per category */
    suggestion_prompts: {},
    /** Context depth for suggestion generation */
    context_depth: 10,
    /** Number of suggestions to generate */
    suggestion_count: 5,
    /** Shared chat date format */
    chat_date_format: 'MM/DD/YY',
    /** Shared enter-to-send preference */
    enter_to_send: true,
    /** Shared context notes sidebar expanded state */
    context_notes_sidebar_expanded: false,
    /** Shared context notes intro visibility */
    context_notes_intro_hidden: false,
    /** Whether assistant thinking blocks should be restored from saved chat history */
    persist_thinking_content: false,
    /** Shared custom suggestion categories */
    custom_suggestion_categories: {},
};

// ── Chat ──────────────────────────────────────────────────────────────────────
// Container object so that reassignments (chatTabs = [...]) are visible through
// ES module imports (live binding to the object, not the variable).

/** Chat state container — mutable properties, imported as a live reference */
export const chat = {
    /** Whether a chat request is in progress */
    busy: false,
    /** Whether compaction is in progress */
    compactionInProgress: false,
    /** Unread chat count */
    unreadChatCount: 0,
    /** Abort controller for the current chat request */
    abortController: null,
    /** Chat tab collection */
    tabs: [],
    /** ID of the active chat tab */
    activeTabId: null,
    /** Index of the active chat tab */
    activeTabIdx: 0,
    /** Whether the chat tabs have unsaved changes */
    tabsDirty: false,
    /** Timer ID for debounced chat tab persistence */
    persistTimer: null,
    /** Whether the chat view has been initialized */
    initialized: false,
    /** Trash bin for recently deleted tabs: array of { tab, trashedAt } */
    tabTrash: [],
    /** Interval ID for periodic trash purge (24h) */
    trashPurgeTimer: null,
    /** Whether auto-scroll is disabled (user scrolled up during generation) */
    disableAutoScroll: false,
    /** Visibility UI state */
    visibilityUi: {
        archiveOpen: false,
        hiddenOpen: false,
        hiddenRevealed: false,
        activeSearchVisibility: ['active'],
        selectedIds: new Set(),
    },
};

// ── LHM (Windows Hardware Monitor) ───────────────────────────────────────────

/** LHM state container — mutable properties, imported as a live reference */
export const lhm = {
    /** Temporary bridge for LHM overlay flow */
    resolve: null,
};

/** Setup/monitor view state */
export const setupViewState = {
    view: 'setup',
    sessionActive: false,
    lastSessionData: null,
    previousPosition: null,
};

/** Monitor metrics UI state */
export const monitorState = {
    speedMax: {
        prompt: 0,
        generation: 0,
    },
};

// ── Updates ───────────────────────────────────────────────────────────────────

/** Whether the update notification has been dismissed */
export let updateDismissed = false;

/** Current app version */
export let appVersion = '';

/** Dismissed update version (to avoid re-showing) */
export let dismissedUpdateVersion = '';

// ── UI State ──────────────────────────────────────────────────────────────────

/** Whether the sidebar is collapsed */
export let sidebarCollapsed = false;

/** Current visualization style preference */
export let vizStyle = null;

/** Chat style preference */
export let chatStyle = null;

/** Whether Enter sends messages (vs Ctrl+Enter) */
export let enterToSend = true;

/** Chat font preference */
export let chatFont = null;
