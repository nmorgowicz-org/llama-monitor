import { sessionState } from '../core/app-state.js';
import { saveSettings } from './settings.js';

export async function updateActiveSessionInfo() {
    try {
        const headers = window.authHeaders ? window.authHeaders() : {};
        const resp = await fetch('/api/sessions/active', { headers });
        if (resp.status === 401) {
            console.error('Failed to update active session info: authentication required');
            return;
        }

        const data = await resp.json();
        if (!data?.mode) return;

        const modeParts = data.mode.split(':');
        if (modeParts[0] === 'Spawn') {
            sessionState.activeSessionPort = parseInt(modeParts[1], 10) || 8080;
            return;
        }

        if (modeParts[0] !== 'Attach') return;

        const endpoint = modeParts.slice(1).join(':');
        try {
            const url = new URL(endpoint);
            sessionState.activeSessionPort = parseInt(url.port, 10) || 8080;
        } catch {
            sessionState.activeSessionPort = 8080;
        }

        const endpointInput = document.getElementById('server-endpoint');
        if (endpointInput && endpointInput.value !== endpoint) {
            endpointInput.value = endpoint;
            saveSettings();
        }
    } catch (err) {
        console.error('Failed to update active session info:', err);
    }
}
