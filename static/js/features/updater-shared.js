// ── Updater modal accessibility helpers ─────────────────────────────────────
// Shared focus-trap + Escape-key handling for the llama.cpp and Rapid-MLX
// updater modals, so both stay keyboard-accessible without duplicating the
// tab-cycling logic.

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
