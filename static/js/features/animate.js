// ── Animate ───────────────────────────────────────────────────────────────────
// Number counting animation for smooth value transitions.

// Cancel any in-progress rAF loop before starting a new one so two calls
// 500ms apart (one tick) don't pile up overlapping writes to the same element.
const activeAnimations = new WeakMap();

export function animateNumber(element, from, to, duration = 300, decimals = 1, suffix = '') {
    if (!element) return;

    const prev = activeAnimations.get(element);
    if (prev) cancelAnimationFrame(prev);

    const startTime = performance.now();
    const diff = to - from;

    function update(currentTime) {
        const elapsed = currentTime - startTime;
        const progress = Math.min(elapsed / duration, 1);

        // Ease-out cubic
        const ease = 1 - Math.pow(1 - progress, 3);
        const current = from + (diff * ease);

        element.textContent = current.toFixed(decimals) + suffix;

        if (progress < 1) {
            const id = requestAnimationFrame(update);
            activeAnimations.set(element, id);
        } else {
            activeAnimations.delete(element);
        }
    }

    const id = requestAnimationFrame(update);
    activeAnimations.set(element, id);
}

// ── Public API ────────────────────────────────────────────────────────────────

export function initAnimate() {
}
