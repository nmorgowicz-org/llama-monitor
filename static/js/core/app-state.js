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
    /** Currently active session ID */
    activeSessionId: 'default',
    /** Currently active session port */
    activeSessionPort: 8080,
    /** Whether the local server is running */
    serverRunning: false,
    /** Previous log length for incremental rendering */
    prevLogLen: 0,
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
export function setLastSystemMetrics(v) { lastSystemMetrics = v; }
export function setLastGpuMetrics(v) { lastGpuMetrics = v; }
export function setLastCapabilities(v) { lastCapabilities = v; }
export function setLastGpuData(v) { lastGpuData = v; }

// ── Settings ──────────────────────────────────────────────────────────────────

/** Settings state container — mutable properties, imported as a live reference */
export const settingsState = {
    /** Whether settings modal has unsaved changes */
    isDirty: false,
    /** Timer ID for debounced settings save */
    saveTimer: null,
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
