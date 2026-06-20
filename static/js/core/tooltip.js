// Global lightweight tooltip — intercepts [title] and [data-tooltip] across the whole app.
//
// Strategy: on mouseover, steal the `title` attribute (to suppress the browser default),
// store it in `data-original-title`, and show a styled floating div. Restore on mouseout.
// This means zero HTML changes are needed in other files — existing title attrs just work.

const SHOW_DELAY = 300;
const MAX_WIDTH = 300;
const GAP = 10;

// Exclude form controls — browsers own those and you can't meaningfully prevent their tooltips.
const EXCLUDE_TAGS = new Set(['INPUT', 'TEXTAREA', 'SELECT', 'OPTION', 'OPTGROUP']);
const SELECTOR = '[title]:not([title=""]), [data-tooltip], [data-original-title]';

let _tip = null;
let _timer = null;
let _current = null;

function _getEl() {
    if (!_tip) {
        _tip = document.createElement('div');
        _tip.id = 'global-tooltip';
        _tip.className = 'global-tooltip';
        _tip.style.display = 'none';
        document.body.appendChild(_tip);
    }
    return _tip;
}

function _getText(el) {
    return el.dataset.tooltip || el.dataset.originalTitle || '';
}

function _position(srcEl, tipEl) {
    const r = srcEl.getBoundingClientRect();
    const vw = window.innerWidth;
    const vh = window.innerHeight;

    // Measure after making visible (opacity 0)
    const tw = Math.min(tipEl.scrollWidth, MAX_WIDTH);
    const th = tipEl.offsetHeight;

    let top = r.top - th - GAP;
    let left = r.left + r.width / 2 - tw / 2;

    if (top < GAP) top = r.bottom + GAP;     // flip below
    if (left < GAP) left = GAP;
    if (left + tw > vw - GAP) left = vw - tw - GAP;
    if (top + th > vh - GAP) top = r.top - th - GAP; // last resort: above

    tipEl.style.top = top + 'px';
    tipEl.style.left = left + 'px';
}

function _show(el) {
    const text = _getText(el);
    if (!text) return;
    const tipEl = _getEl();
    tipEl.textContent = text;
    tipEl.style.cssText = 'display:block; opacity:0; transform:translateY(4px); max-width:' + MAX_WIDTH + 'px;';
    void tipEl.offsetHeight; // force layout for measurement
    _position(el, tipEl);
    tipEl.style.opacity = '1';
    tipEl.style.transform = 'translateY(0)';
}

function _hide() {
    clearTimeout(_timer);
    if (!_tip) return;
    _tip.style.opacity = '0';
    _tip.style.transform = 'translateY(4px)';
    setTimeout(() => {
        if (_tip && _tip.style.opacity === '0') _tip.style.display = 'none';
    }, 180);
}

function _intercept(target) {
    if (target.title && !target.dataset.originalTitle) {
        target.dataset.originalTitle = target.title;
        target.removeAttribute('title');
    }
}

function _restore(target) {
    if (target.dataset.originalTitle !== undefined) {
        target.setAttribute('title', target.dataset.originalTitle);
        delete target.dataset.originalTitle;
    }
}

export function initGlobalTooltip() {
    document.addEventListener('mouseover', e => {
        const raw = e.target;
        if (!raw || typeof raw.closest !== 'function') return;
        if (EXCLUDE_TAGS.has(raw.tagName)) return;

        const target = raw.closest(SELECTOR);
        if (target === _current) return;

        // Leaving the previous source
        if (_current) { _restore(_current); _current = null; }
        clearTimeout(_timer);

        if (!target || EXCLUDE_TAGS.has(target.tagName)) {
            _hide();
            return;
        }

        _intercept(target);
        _current = target;
        _timer = setTimeout(() => _show(target), SHOW_DELAY);
    }, false);

    document.addEventListener('mouseout', e => {
        const raw = e.target;
        if (!raw || typeof raw.closest !== 'function') return;
        const src = raw.closest('[data-original-title], [data-tooltip]');
        if (!src || src !== _current) return;

        // Still inside the same tooltip source (moving to child)?
        const relEl = e.relatedTarget;
        if (relEl && src.contains(relEl)) return;

        _restore(src);
        _current = null;
        clearTimeout(_timer);
        _hide();
    }, false);

    // Hide on any navigation action
    document.addEventListener('mousedown', () => {
        if (_current) { _restore(_current); _current = null; }
        _hide();
    }, false);
    document.addEventListener('scroll', _hide, true);
    window.addEventListener('resize', _hide, false);
}
