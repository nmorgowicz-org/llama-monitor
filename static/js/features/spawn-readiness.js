// ── Spawn readiness ────────────────────────────────────────────────────────────
// Shared readiness check used by doStart() and the spawn wizard.

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

    throw new Error(`llama-server started but did not become reachable on port ${port} in time.`);
}
