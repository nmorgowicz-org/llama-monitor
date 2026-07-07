// ── Template Autoupdater ───────────────────────────────────────────────────────
// Quiet, 12-hour interval auto-checker for community chat templates.
// Uses /api/chat-template/active and /api/chat-template/check-update.
// Generic: covers Qwen, Gemma, and any future template with a meta.json.
// Modeled after llama-updater.js (visibility-aware, no spam).

const STORAGE_LAST_CHECK = 'template_autoupdater_lastCheck';
const STORAGE_BUSY = 'template_autoupdater_busy';
const STORAGE_LAST_STATUS = 'template_autoupdater_lastStatus';

const INTERVAL_MS = 12 * 60 * 60 * 1000; // 12 hours
const BUSY_TTL_MS = 60 * 1000; // busy guard: 1 minute

let _intervalId = null;

export function readLastStatus() {
  try {
    const v = localStorage.getItem(STORAGE_LAST_STATUS);
    if (!v) return { templates_with_updates: [] };
    const obj = JSON.parse(v);
    if (!obj || !Array.isArray(obj.templates_with_updates)) {
      return { templates_with_updates: [] };
    }
    return obj;
  } catch {
    return { templates_with_updates: [] };
  }
}

export function initTemplateAutoupdater() {
  // Start after a short delay; respect visibility.
  setTimeout(() => {
    if (!document.hidden) {
      scheduleNextCheck();
    }
  }, 3000);

  document.addEventListener('visibilitychange', () => {
    if (!document.hidden) {
      // When tab becomes visible, ensure we are up to date.
      scheduleNextCheck(true);
    } else {
      stopInterval();
    }
  });
}

function nowTs() {
  return Date.now();
}

function isBusy() {
  try {
    const ts = Number(localStorage.getItem(STORAGE_BUSY) || '0');
    if (!ts) return false;
    return (nowTs() - ts) < BUSY_TTL_MS;
  } catch {
    return false;
  }
}

function setBusy() {
  try {
    localStorage.setItem(STORAGE_BUSY, String(nowTs()));
  } catch {
    // ignore storage errors
  }
}

function clearBusy() {
  try {
    localStorage.removeItem(STORAGE_BUSY);
  } catch {
    // ignore
  }
}

function getLastCheck() {
  try {
    return Number(localStorage.getItem(STORAGE_LAST_CHECK) || '0');
  } catch {
    return 0;
  }
}

function setLastCheck(ts) {
  try {
    localStorage.setItem(STORAGE_LAST_CHECK, String(ts));
  } catch {
    // ignore
  }
}

function shouldRunCheck(immediate = false) {
  if (document.hidden) return false;
  if (isBusy()) return false;

  const last = getLastCheck();
  if (!immediate && last > 0) {
    const elapsed = nowTs() - last;
    if (elapsed < INTERVAL_MS) return false;
  }
  return true;
}

function scheduleNextCheck(immediate = false) {
  stopInterval();

  // If we should run now, do it, then schedule future interval.
  if (shouldRunCheck(immediate)) {
    // Small debounce so we don't fire immediately on every visibility change.
    setTimeout(() => {
      performCheck().finally(() => {
        startInterval();
      });
    }, 1500);
  } else {
    startInterval();
  }
}

function startInterval() {
  stopInterval();
  _intervalId = setInterval(() => {
    if (document.hidden) return;
    if (!shouldRunCheck()) return;
    performCheck();
  }, INTERVAL_MS);
}

function stopInterval() {
  if (_intervalId != null) {
    clearInterval(_intervalId);
    _intervalId = null;
  }
}

async function performCheck() {
  if (document.hidden) return;
  setBusy();
  setLastCheck(nowTs());

  try {
    const resp = await (await fetch('/api/chat-template/active', {
      headers: window.authHeaders ? window.authHeaders() : {},
    })).json();

    if (!resp.ok || !Array.isArray(resp.templates)) {
      clearBusy();
      return;
    }

    const changedTemplates = [];

    for (const tpl of resp.templates) {
      if (!tpl.path) continue;
      try {
        const checkResp = await fetch('/api/chat-template/check-update', {
          method: 'POST',
          headers: {
            ...(window.authHeaders ? window.authHeaders() : {}),
            'Content-Type': 'application/json',
          },
          body: JSON.stringify({ path: tpl.path }),
        });

        if (!checkResp.ok) continue;
        const checkData = await checkResp.json();

        if (checkData.ok === true && checkData.changed === true) {
          changedTemplates.push({
            name: tpl.name,
            path: tpl.path,
            source_url: tpl.source_url,
          });
        }
      } catch {
        // Silently ignore per-template errors
      }
    }

    // Store last status
    const status = {
      templates_with_updates: changedTemplates,
    };
    try {
      localStorage.setItem(STORAGE_LAST_STATUS, JSON.stringify(status));
    } catch {
      // ignore
    }

    // If any templates have upstream changes, show a soft toast.
    if (changedTemplates.length > 0) {
      const { showToast } = await import('./toast.js');
      if (typeof showToast === 'function') {
        const names = changedTemplates
          .map(t => t.name || 'unknown')
          .join(', ');
        showToast(
          'Chat template changes available',
          'warn',
          `Upstream changes detected for: ${names}. Use Recommended to refresh.`,
          5000
        );
      }
    }

    // Dispatch event for UI consumers (e.g., spawn wizard)
    window.dispatchEvent(
      new CustomEvent('templateAutoupdateResult', {
        detail: {
          changedTemplates,
          lastCheck: nowTs(),
        },
      })
    );
  } catch {
    // Network or parse errors: silently ignored; will retry at next interval.
  } finally {
    clearBusy();
  }
}
