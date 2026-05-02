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

/** Loaded presets from backend */
export let presets = [];

/** Loaded sessions from backend */
export let sessions = [];

/** Currently active session ID */
export let activeSessionId = 'default';

/** Currently active session port */
export let activeSessionPort = 8080;

/** Whether the local server is running */
export let serverRunning = false;

/** Previous log length for incremental rendering */
export let prevLogLen = 0;

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

// ── Settings ──────────────────────────────────────────────────────────────────

/** Whether settings modal has unsaved changes */
export let settingsIsDirty = false;

/** Timer ID for debounced settings save */
export let settingsSaveTimer = null;

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

/** Temporary bridge for LHM overlay flow */
export let lhmResolve = null;

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
