// ── Spawn readiness ────────────────────────────────────────────────────────────
// Shared readiness check used by doStart() and the spawn wizard.

async function fetchLastLogLines() {
    try {
        const headers = window.authHeaders ? window.authHeaders() : {};
        const resp = await fetch('/api/debug/logs', { headers, cache: 'no-store' });
        if (resp.ok) {
            const data = await resp.json().catch(() => null);
            if (data?.logs && Array.isArray(data.logs)) {
                // Take last 8 lines, filter noise
                const lines = data.logs
                    .slice(-16)
                    .filter(l => l && !l.includes('[poll]'))
                    .join('\n')
                    .trim();
                return lines || null;
            }
        }
    } catch {
        // Best-effort; ignore errors here.
    }
    return null;
}

function tryExtractHint(log) {
    if (!log) return null;
    const lower = log.toLowerCase();
    if (lower.includes('out of memory') || lower.includes('oom') || lower.includes('cannot allocate')) {
        return 'Likely out-of-memory. Try reducing context size or using a smaller model.';
    }
    if (lower.includes('cannot find') || lower.includes('file not found') || lower.includes('no such file')) {
        return 'A required file was not found (model, binary, or config). Check the model path.';
    }
    if (lower.includes('permission denied') || lower.includes('access denied') || lower.includes('13: permission')) {
        return 'Permission denied. Check file permissions and ownership.';
    }
    if (lower.includes('could not bind') || lower.includes('address already in use')) {
        return 'Port conflict. Another process is using this port.';
    }
    return null;
}

export async function waitForSpawnReadiness(port, timeoutMs = 30000) {
    const started = Date.now();
    const headers = window.authHeaders ? window.authHeaders() : {};

    while (Date.now() - started < timeoutMs) {
        try {
            const resp = await fetch('/api/sessions/active/readiness', {
                method: 'GET',
                headers,
                cache: 'no-store',
            });
            const data = await resp.json().catch(() => null);
            if (resp.ok && data?.ok && data.ready) return;
        } catch {
            // Keep polling until timeout; the backend route may lag while the process boots.
        }
        await new Promise(r => setTimeout(r, 800));
    }

    // Timeout: try to include helpful logs in the error
    const logs = await fetchLastLogLines();
    const hint = tryExtractHint(logs);
    const msgLines = [
        `llama-server started but did not become reachable on port ${port} in time.`,
    ];
    if (hint) {
        msgLines.push(hint);
    }
    if (logs) {
        // Include only last 2000 chars of logs to avoid huge toasts
        msgLines.push('Last log lines:', logs.slice(-2000));
    }
    throw new Error(msgLines.join('\n'));
}
