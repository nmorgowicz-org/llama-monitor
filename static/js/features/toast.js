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
