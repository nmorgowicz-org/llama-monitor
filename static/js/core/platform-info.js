let platformInfoPromise = null;

export function getPlatformInfo({ refresh = false } = {}) {
    if (refresh || !platformInfoPromise) {
        const headers = window.authHeaders ? window.authHeaders() : {};
        platformInfoPromise = fetch('/api/llama-binary/platform-info', { headers })
            .then(response => {
                if (!response.ok) throw new Error(`Platform info failed (${response.status})`);
                return response.json();
            })
            .catch(error => {
                platformInfoPromise = null;
                throw error;
            });
    }
    return platformInfoPromise;
}
