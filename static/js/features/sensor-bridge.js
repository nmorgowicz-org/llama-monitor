// ── Sensor Bridge ─────────────────────────────────────────────────────────────
// Sensor bridge setup button handler.

import { escapeHtml } from '../core/format.js';

function _bindSensorBridgeSetup() {
    const btn = document.getElementById('btn-sensor-bridge-setup');
    if (!btn) return;
    btn.addEventListener('click', async () => {
        btn.disabled = true;
        btn.textContent = 'Installing...';
        const callout = document.getElementById('sensor-bridge-setup-callout');
        try {
            const res = await fetch('/api/sensor-bridge/install', { method: 'POST' });
            const data = await res.json();
            if (!data.started) {
                btn.textContent = 'Setup';
                btn.disabled = false;
                if (callout) {
                    callout.innerHTML = '<span style="color:#bf616a;">Install failed: ' + escapeHtml(data.error || 'Unknown error') + '</span>';
                }
                return;
            }
            if (callout) {
                callout.innerHTML = '<span style="color:#a3be8c;">A UAC prompt will appear on your desktop \u2014 approve it to install the sensor service. This takes a few seconds.</span>';
            }
            let elapsed = 0;
            const poll = setInterval(async () => {
                elapsed += 2000;
                try {
                    const s = await fetch('/api/sensor-bridge/status');
                    const sd = await s.json();
                    if (sd.running) {
                        clearInterval(poll);
                        if (callout) callout.style.display = 'none';
                    } else if (elapsed >= 30000) {
                        clearInterval(poll);
                        btn.textContent = 'Setup';
                        btn.disabled = false;
                        if (callout) {
                            callout.innerHTML = 'CPU temperature requires a one-time service install. <button id="btn-sensor-bridge-setup" style="margin-left:8px; padding:3px 10px; background:#5e81ac; border:none; border-radius:4px; color:#eceff4; cursor:pointer; font-size:12px;">Setup</button><span style="color:#ebcb8b; margin-left:8px;">Timed out \u2014 did you approve the UAC prompt?</span>';
                            const newBtn = document.getElementById('btn-sensor-bridge-setup');
                            if (newBtn) newBtn.addEventListener('click', () => btn.click());
                        }
                    }
                } catch (_) {}
            }, 2000);
        } catch (e) {
            btn.textContent = 'Setup';
            btn.disabled = false;
        }
    });
}

// ── Public API ────────────────────────────────────────────────────────────────

export function initSensorBridge() {
    _bindSensorBridgeSetup();
}
