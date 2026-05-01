// ── Toast ─────────────────────────────────────────────────────────────────────
// Toast notifications, progress toasts, and action toasts.

import { escapeHtml } from '../core/format.js';

const TOAST_AUTO_DISMISS = 3500;

function getToastIcon(type) {
    const icons = {
        success: '✓',
        error: '✗',
        warning: '⚠',
        info: 'ℹ'
    };
    return icons[type] || 'ℹ';
}

function showToast(title, type = 'error', message = '') {
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
            info: 'info'
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

    toast.innerHTML = content;
    container.appendChild(toast);
    requestAnimationFrame(() => { toast.classList.add('show'); });

    if (type === 'progress') {
        return toast;
    } else {
        setTimeout(() => {
            toast.classList.remove('show');
            setTimeout(() => toast.remove(), 300);
        }, TOAST_AUTO_DISMISS);
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

function showToastWithActions(title, type, message, actions = []) {
    const container = document.getElementById('toast-container');
    if (!container) return;

    const toast = document.createElement('div');
    toast.className = 'toast toast-' + type + ' toast-with-actions';

    const iconMap = {
        success: 'success',
        error: 'error',
        warning: 'warning',
        info: 'info'
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

    toast.innerHTML = `
        <div class="toast-icon ${type}">${getToastIcon(iconType)}</div>
        <div class="toast-content">
            ${title ? '<div class="toast-title">' + escapeHtml(title) + '</div>' : ''}
            ${message ? '<div class="toast-message">' + escapeHtml(message) + '</div>' : ''}
        </div>
        ${actionsHtml}
        <button class="toast-close" data-toast-close="">&times;</button>
    `;

    if (actions.length > 0) {
        toast.querySelectorAll('[data-action]').forEach(btn => {
            btn.addEventListener('click', () => {
                const action = actions.find(a => a.id === btn.dataset.action);
                if (action && action.handler) action.handler();
                toast.classList.remove('show');
                setTimeout(() => toast.remove(), 300);
            });
        });
    }

    container.appendChild(toast);
    requestAnimationFrame(() => { toast.classList.add('show'); });

    setTimeout(() => {
        toast.classList.remove('show');
        setTimeout(() => toast.remove(), 300);
    }, Math.max(TOAST_AUTO_DISMISS, 5000));
}

function showToastProgress(title, type = 'info') {
    const container = document.getElementById('toast-container');
    if (!container) return null;

    const toast = document.createElement('div');
    toast.className = 'toast toast-' + type;
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

    window.showToast = showToast;
    window.showToastWithActions = showToastWithActions;
}
