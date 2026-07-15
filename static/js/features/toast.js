// ── Toast ─────────────────────────────────────────────────────────────────────
// Toast notifications, progress toasts, and action toasts.

import { escapeHtml } from '../core/format.js';

const TOAST_AUTO_DISMISS = 6000;

function getToastIcon(type) {
    const icons = {
        success: '✓',
        error: '✗',
        warning: '⚠',
        info: 'ℹ',
        explicit: '🔒'
    };
    return icons[type] || 'ℹ';
}

const EXPLICIT_LEVEL_ICONS = { 0: '🔒', 1: '🔓', 2: '🔥' };

export function showToast(title, type = 'error', message = '', options = {}) {
    const container = document.getElementById('toast-container');
    if (!container) return null;

    const toast = document.createElement('div');
    toast.className = 'toast toast-' + type;

    let content = '';

    if (type === 'progress') {
        content = '<div class="toast-content"><div class="toast-progress-bar"><div class="toast-progress-fill" style="width:0%"></div></div></div>';
    } else {
        const iconMap = {
            success: 'success',
            error: 'error',
            warning: 'warning',
            info: 'info',
            explicit: 'explicit'
        };
        const iconType = iconMap[type] || 'info';
        content = `
            <div class="toast-icon ${type}">${getToastIcon(iconType)}</div>
            <div class="toast-content">
                ${title ? '<div class="toast-title">' + escapeHtml(title) + '</div>' : ''}
                ${message ? '<div class="toast-message">' + escapeHtml(message) + '</div>' : ''}
            </div>
            <button class="toast-close" data-toast-close="">&times;</button>
        `;
    }

    // eslint-disable-next-line no-unsanitized/property -- content is built from hardcoded template; type is a caller-controlled enum used only in CSS class; title/message wrapped in escapeHtml()
    toast.innerHTML = content;

    // Set explicit-level icon and class
    if (type === 'explicit' && options.level !== undefined) {
        const iconEl = toast.querySelector('.toast-icon');
        if (iconEl) iconEl.textContent = EXPLICIT_LEVEL_ICONS[options.level] || 'G';
        toast.classList.add('explicit-level-' + options.level);
    }

    container.appendChild(toast);
    requestAnimationFrame(() => { toast.classList.add('show'); });

    if (type === 'progress') {
        return toast;
    } else {
        setTimeout(() => {
            toast.classList.remove('show');
            setTimeout(() => toast.remove(), 300);
        }, options.duration || TOAST_AUTO_DISMISS);
        return null;
    }
}

function updateToastProgress(toastElement, percent, message) {
    if (!toastElement) return;
    const fill = toastElement.querySelector('.toast-progress-fill');
    const content = toastElement.querySelector('.toast-content');
    if (fill) fill.style.width = percent + '%';
    if (content && message) {
        content.innerHTML = '<div class="toast-title">' + escapeHtml(message) + '</div>';
    }
}

export function showToastWithActions(title, type, message, actions = [], options = {}) {
    const container = document.getElementById('toast-container');
    if (!container) return;

    const { onDismiss = null, duration = Math.max(TOAST_AUTO_DISMISS, 5000) } = options;

    const toast = document.createElement('div');
    toast.className = 'toast toast-' + type + ' toast-with-actions';

    const iconMap = {
        success: 'success',
        error: 'error',
        warning: 'warning',
        info: 'info',
        explicit: 'explicit'
    };
    const iconType = iconMap[type] || 'info';

    let actionsHtml = '';
    if (actions.length > 0) {
        actionsHtml = '<div class="toast-actions">' +
            actions.map(action => {
                const cls = action.primary ? 'btn-sm btn-primary' : 'btn-sm btn-secondary';
                return '<button class="' + cls + '" data-action="' + action.id + '">' + escapeHtml(action.label) + '</button>';
            }).join('') + '</div>';
    }

    // eslint-disable-next-line no-unsanitized/property -- type is a hardcoded enum used only in CSS class; title/message wrapped in escapeHtml(); actionsHtml uses escapeHtml(); getToastIcon returns hardcoded strings
    toast.innerHTML = `
        <div class="toast-icon ${type}">${getToastIcon(iconType)}</div>
        <div class="toast-content">
            ${title ? '<div class="toast-title">' + escapeHtml(title) + '</div>' : ''}
            ${message ? '<div class="toast-message">' + escapeHtml(message) + '</div>' : ''}
        </div>
        ${actionsHtml}
        <button class="toast-close" data-toast-close="">&times;</button>
    `;

    let actionTaken = false;

    if (actions.length > 0) {
        toast.querySelectorAll('[data-action]').forEach(btn => {
            btn.addEventListener('click', () => {
                actionTaken = true;
                const action = actions.find(a => a.id === btn.dataset.action);
                if (action && action.handler) action.handler();
                toast.classList.remove('show');
                setTimeout(() => toast.remove(), 300);
            });
        });
    }

    // Close button also counts as dismissed without action
    toast.querySelector('[data-toast-close]')?.addEventListener('click', () => {
        if (!actionTaken && onDismiss) onDismiss();
    });

    container.appendChild(toast);
    requestAnimationFrame(() => { toast.classList.add('show'); });

    setTimeout(() => {
        if (!actionTaken && onDismiss) onDismiss();
        toast.classList.remove('show');
        setTimeout(() => toast.remove(), 300);
    }, duration);
}

function showToastProgress(title, type = 'info') {
    const container = document.getElementById('toast-container');
    if (!container) return null;

    const toast = document.createElement('div');
    toast.className = 'toast toast-' + type;
    // eslint-disable-next-line no-unsanitized/property -- type is a hardcoded enum used only in CSS class; title wrapped in escapeHtml(); getToastIcon returns hardcoded strings
    toast.innerHTML = `
        <div class="toast-icon ${type}">${getToastIcon(type)}</div>
        <div class="toast-content">
            ${title ? '<div class="toast-title">' + escapeHtml(title) + '</div>' : ''}
            <div class="toast-progress-bar"><div class="toast-progress-fill" style="width:0%"></div></div>
        </div>
    `;
    container.appendChild(toast);
    requestAnimationFrame(() => { toast.classList.add('show'); });
    return toast;
}

// ── Public API ────────────────────────────────────────────────────────────────

export function initToast() {
    // Event delegation for toast close buttons
    document.getElementById('toast-container')?.addEventListener('click', (e) => {
        const closeBtn = e.target.closest('[data-toast-close]');
        if (closeBtn) {
            closeBtn.closest('.toast')?.remove();
        }
    });
}

/**
 * Minimal confirmation dialog matching app style.
 * Returns true if user confirmed.
 */
export async function showConfirmDialog(title, message, confirmLabel = 'Confirm') {
    const overlay = document.createElement('div');
    overlay.className = 'modal-overlay app-confirm-overlay active';
    overlay.style.zIndex = '2000';

    const dialog = document.createElement('div');
    dialog.className = 'modal app-confirm-dialog';
    dialog.setAttribute('role', 'dialog');
    dialog.setAttribute('aria-modal', 'true');

    const dialogId = `app-confirm-${Date.now()}`;
    const titleId = `${dialogId}-title`;
    const messageId = `${dialogId}-message`;
    dialog.setAttribute('aria-labelledby', titleId);
    dialog.setAttribute('aria-describedby', messageId);

    const icon = document.createElement('div');
    icon.className = 'app-confirm-icon';
    icon.setAttribute('aria-hidden', 'true');
    icon.textContent = '✓';

    const copy = document.createElement('div');
    copy.className = 'app-confirm-copy';

    const titleEl = document.createElement('div');
    titleEl.className = 'app-confirm-title';
    titleEl.id = titleId;
    titleEl.textContent = title;

    const msgEl = document.createElement('div');
    msgEl.className = 'app-confirm-message';
    msgEl.id = messageId;
    msgEl.textContent = message;

    const actions = document.createElement('div');
    actions.className = 'app-confirm-actions';

    const cancelBtn = document.createElement('button');
    cancelBtn.type = 'button';
    cancelBtn.className = 'btn btn-modal-cancel';
    cancelBtn.textContent = 'Cancel';

    const confirmBtn = document.createElement('button');
    confirmBtn.type = 'button';
    confirmBtn.className = 'btn btn-modal-save';
    confirmBtn.textContent = confirmLabel;

    return new Promise(resolve => {
        let decided = false;

        function onKeydown(e) {
            if (e.key === 'Escape' && !decided) {
                decided = true;
                cleanup();
                resolve(false);
            }
        }

        function cleanup() {
            document.removeEventListener('keydown', onKeydown);
            if (overlay.parentElement) overlay.remove();
        }

        cancelBtn.addEventListener('click', () => {
            if (decided) return;
            decided = true;
            cleanup();
            resolve(false);
        });

        confirmBtn.addEventListener('click', () => {
            if (decided) return;
            decided = true;
            cleanup();
            resolve(true);
        });

        overlay.addEventListener('click', (e) => {
            if (decided) return;
            if (e.target === overlay) {
                decided = true;
                cleanup();
                resolve(false);
            }
        });

        document.addEventListener('keydown', onKeydown);

        actions.appendChild(cancelBtn);
        actions.appendChild(confirmBtn);
        copy.appendChild(titleEl);
        copy.appendChild(msgEl);
        dialog.appendChild(icon);
        dialog.appendChild(copy);
        dialog.appendChild(actions);
        overlay.appendChild(dialog);
        document.body.appendChild(overlay);
        cancelBtn.focus();
    });
}

/**
 * Minimal text prompt dialog matching app style.
 * Returns user text or null if cancelled.
 */
export async function showPromptDialog(title, message, defaultValue = '') {
    const overlay = document.createElement('div');
    overlay.className = 'modal-overlay';
    overlay.style.zIndex = '2000';
    overlay.style.display = 'grid';

    const dialog = document.createElement('div');
    dialog.className = 'modal';
    dialog.style.width = '420px';
    dialog.style.padding = '14px 16px';

    const titleEl = document.createElement('div');
    titleEl.style.fontSize = '15px';
    titleEl.style.fontWeight = '600';
    titleEl.style.marginBottom = '8px';
    titleEl.textContent = title;

    const msgEl = document.createElement('div');
    msgEl.style.fontSize = '13px';
    msgEl.style.color = 'var(--color-text-muted)';
    msgEl.style.marginBottom = '10px';
    msgEl.textContent = message;

    const input = document.createElement('input');
    input.type = 'text';
    input.value = defaultValue;
    input.style.width = '100%';
    input.style.boxSizing = 'border-box';
    input.style.padding = '8px 10px';
    input.style.marginBottom = '12px';
    input.style.borderRadius = '999px';
    input.style.border = '1px solid var(--border-subtle)';
    input.style.background = 'var(--color-bg-surface)';
    input.style.color = 'var(--color-text-primary)';
    input.style.fontSize = '14px';
    input.style.outline = 'none';

    const actions = document.createElement('div');
    actions.style.display = 'flex';
    actions.style.justifyContent = 'flex-end';
    actions.style.gap = '8px';

    const cancelBtn = document.createElement('button');
    cancelBtn.type = 'button';
    cancelBtn.className = 'btn btn-modal-cancel';
    cancelBtn.textContent = 'Cancel';

    const okBtn = document.createElement('button');
    okBtn.type = 'button';
    okBtn.className = 'btn btn-modal-save';
    okBtn.textContent = 'OK';

    return new Promise(resolve => {
        let decided = false;

        function cleanup() {
            if (overlay.parentElement) overlay.remove();
        }

        function handleCancel() {
            if (decided) return;
            decided = true;
            cleanup();
            resolve(null);
        }

        function handleOk() {
            if (decided) return;
            decided = true;
            cleanup();
            resolve(input.value === '' ? null : input.value);
        }

        cancelBtn.addEventListener('click', handleCancel);
        okBtn.addEventListener('click', handleOk);
        input.addEventListener('keydown', (e) => {
            if (e.key === 'Enter') handleOk();
        });

        overlay.addEventListener('click', (e) => {
            if (e.target === overlay) handleCancel();
        });

        document.addEventListener('keydown', (e) => {
            if (e.key === 'Escape') handleCancel();
        });

        actions.appendChild(cancelBtn);
        actions.appendChild(okBtn);
        dialog.appendChild(titleEl);
        dialog.appendChild(msgEl);
        dialog.appendChild(input);
        dialog.appendChild(actions);
        overlay.appendChild(dialog);
        document.body.appendChild(overlay);
        input.focus();
        input.select();
    });
}
