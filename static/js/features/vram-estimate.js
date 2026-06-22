// Centralized VRAM estimation using the Rust backend as the single source of truth.
// No local VRAM formulas — all math comes from /api/vram-estimate.

import { showToast } from './toast.js';

let debounce = null;
let currentRequestId = 0;

// Build the request body for /api/vram-estimate from the given wizardState.
function buildEstimateBody(state) {
  const hw = state.hardware;
  const m = state.model;

  return {
    model_path: m.path || m.localPath || '',
    n_ctx: hw.contextSize || 4096,
    parallel_slots: hw.parallelSlots || 1,
    ubatch_size: hw.ubatchSize || 2048,
    ctk: hw.cacheTypeK || 'q8_0',
    ctv: hw.cacheTypeV || 'q8_0',
    n_cpu_moe: hw.nCpuMoe || 0,
    available_vram_bytes: state.vram?.available || 0,
    is_unified_memory: state.vram?.isUnifiedMemory || false,
    mmproj_path: m.mmprojPath || '',
    mmproj_bytes: m.mmprojBytes || 0,
  };
}

// Fetch VRAM estimate from backend.
async function fetchEstimate(state) {
  const body = buildEstimateBody(state);
  if (!body.model_path) return null;

  try {
    const headers = (window.authHeaders
      ? window.authHeaders()
      : {}) ;
    const res = await fetch('/api/vram-estimate', {
      method: 'POST',
      headers: { ...headers, 'Content-Type': 'application/json' },
      body: JSON.stringify(body),
    });
    if (!res.ok) return null;
    const data = await res.json();
    if (!data.ok || !data.total_bytes) return null;
    return data;
  } catch {
    return null;
  }
}

// Public: schedule an estimate (debounced). Callback receives estimate or null.
export function scheduleEstimate(
  wizardState,
  onEstimate,
  { debounceMs = 120, force = false } = {},
) {
  if (debounce && !force) {
    clearTimeout(debounce);
  }

  const reqId = ++currentRequestId;

  const doWork = async () => {
    const est = await fetchEstimate(wizardState);
    if (reqId === currentRequestId) {
      onEstimate(est);
    }
  };

  if (force) {
    doWork();
    debounce = null;
  } else {
    debounce = setTimeout(doWork, debounceMs);
  }
}

// Public: cancel pending debounce (used during cleanup).
export function cancelEstimate() {
  if (debounce) {
    clearTimeout(debounce);
    debounce = null;
  }
}
