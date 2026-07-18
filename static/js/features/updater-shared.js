// ── Updater modal shared helpers ─────────────────────────────────────────────
// Focus-trap + Escape-key handling, release-list fetching, and release-row
// badge rendering shared between the llama.cpp updater (llama-updater.js)
// and the Rapid-MLX runtime updater (rapid-mlx-updater.js). Only the pieces
// that are byte-for-byte identical in behavior between the two callers live
// here — anything with even a small semantic difference (row layout, dataset
// keys, age formatting, install/upgrade flows, confirm-token handling) stays
// in its own file. See the Item 6 remediation note in
// docs/plans/20260718-rapid_mlx_phase8_remediation.md for the exact diff.

function focusableIn(modal) {
  return [...modal.querySelectorAll(
    'button, [href], input, select, textarea, [tabindex]:not([tabindex="-1"])'
  )].filter(el => {
    const rect = el.getBoundingClientRect();
    const style = getComputedStyle(el);
    return rect.width > 0 || rect.height > 0 || style.display !== 'none';
  });
}

function trapTabKey(e, modal) {
  const focusable = focusableIn(modal);
  if (!focusable.length) return;
  const first = focusable[0];
  const last = focusable[focusable.length - 1];
  const active = document.activeElement;

  if (e.shiftKey && active === first) {
    e.preventDefault();
    last.focus();
  } else if (!e.shiftKey && active === last) {
    e.preventDefault();
    first.focus();
  }
}

/**
 * Attach Escape-to-close and Tab-cycle focus trapping to an open modal.
 * `onClose` is the caller's own close function so its modal-specific
 * cleanup (resetting panes, clearing state, etc.) still runs.
 */
export function attachModalFocusTrap(modal, onClose) {
  if (!modal) return;
  modal._focusTrapPreviousFocus = document.activeElement;
  const handler = (e) => {
    if (!modal.classList.contains('open')) return;
    if (e.key === 'Escape') {
      e.preventDefault();
      e.stopPropagation();
      onClose();
      return;
    }
    if (e.key === 'Tab') trapTabKey(e, modal);
  };
  document.addEventListener('keydown', handler, true);
  modal._focusTrapHandler = handler;
}

/** Detach the focus trap and restore focus to the element active before open. */
export function detachModalFocusTrap(modal) {
  if (!modal) return;
  if (modal._focusTrapHandler) {
    document.removeEventListener('keydown', modal._focusTrapHandler, true);
    modal._focusTrapHandler = null;
  }
  const previous = modal._focusTrapPreviousFocus;
  if (previous && typeof previous.focus === 'function') previous.focus();
  modal._focusTrapPreviousFocus = null;
}

// ── Release-list fetching ────────────────────────────────────────────────────

/**
 * Fetch a release-list endpoint (llama.cpp `/api/llama-binary/releases` or
 * Rapid-MLX `/api/rapid-mlx/runtime/releases`) and return the parsed JSON
 * body. Throws `Error('HTTP <status>')` on a non-OK response so callers can
 * keep their existing try/catch semantics unchanged (llama-updater surfaces
 * the error to the release list UI; rapid-mlx-updater swallows it silently
 * as a background refresh) — this helper only dedups the fetch/headers/parse
 * plumbing, not the error-handling policy, which differs between callers.
 */
export async function fetchReleaseList(url) {
  const headers = window.authHeaders ? window.authHeaders() : {};
  const resp = await fetch(url, { headers });
  if (!resp.ok) throw new Error(`HTTP ${resp.status}`);
  return resp.json();
}

// ── Release-row badge rendering ──────────────────────────────────────────────

/**
 * Build the "latest" / "installed" badge `<span>` pair used inside both
 * updaters' release rows. The two callers use different CSS class names
 * (`llama-version-*` vs `rapid-mlx-*`), so the class strings are passed in
 * rather than hardcoded — the DOM structure, textContent, and append order
 * this produces are otherwise identical to what each file built inline.
 */
export function buildReleaseBadges({ wrapperClass, badgeClass, isLatest, isCurrent }) {
  const badges = document.createElement('span');
  badges.className = wrapperClass;

  if (isLatest) {
    const b = document.createElement('span');
    b.className = `${badgeClass} ${badgeClass}--latest`;
    b.textContent = 'latest';
    badges.appendChild(b);
  }
  if (isCurrent) {
    const b = document.createElement('span');
    b.className = `${badgeClass} ${badgeClass}--installed`;
    b.textContent = 'installed';
    badges.appendChild(b);
  }

  return badges;
}
