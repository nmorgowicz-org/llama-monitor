// ── Animate ───────────────────────────────────────────────────────────────────
// Number counting animation for smooth value transitions.

export function animateNumber(element, from, to, duration = 300, decimals = 1, suffix = '') {
    if (!element) return;

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
            requestAnimationFrame(update);
        }
    }

    requestAnimationFrame(update);
}

// ── Public API ────────────────────────────────────────────────────────────────

export function initAnimate() {
}
