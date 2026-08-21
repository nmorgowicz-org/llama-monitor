const USABLE_SIDECAR_STATUSES = new Set([
    'candidate',
    'qualified',
    'built_unvalidated_online',
]);

export function normalizeRapidMlxPath(value) {
    return String(value || '').replace(/[\\/]+$/, '');
}

export function rapidMlxSidecarProvenance(sidecar) {
    return sidecar?.provenance || sidecar || {};
}

export function isUsableRapidMlxSidecar(sidecar) {
    const provenance = rapidMlxSidecarProvenance(sidecar);
    return sidecar?.hasWeights !== false
        && sidecar?.hasProvenance !== false
        && provenance.normCheckPassed !== false
        && USABLE_SIDECAR_STATUSES.has(provenance.status);
}

export function findRapidMlxSidecarForTrunk(sidecars, trunk) {
    const selectedTrunk = String(trunk || '').trim();
    if (!selectedTrunk.startsWith('/')) return null;
    const normalizedTrunk = normalizeRapidMlxPath(selectedTrunk);
    return (Array.isArray(sidecars) ? sidecars : []).find(sidecar => {
        if (!isUsableRapidMlxSidecar(sidecar)) return false;
        const provenance = rapidMlxSidecarProvenance(sidecar);
        return normalizeRapidMlxPath(provenance.trunk) === normalizedTrunk;
    }) || null;
}
