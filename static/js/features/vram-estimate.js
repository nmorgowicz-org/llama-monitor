// Centralized VRAM estimation using the Rust backend as the single source of truth.
// No local VRAM formulas — all math comes from /api/vram-estimate.
//
// Phase 5a Part 5: Cross-surface equality — every UI surface must call this
// canonical builder with matching parameters so all surfaces display identical
// MemoryBreakdown values for the same model/context/backend.

import { showToast } from './toast.js';

let debounce = null;
let currentRequestId = 0;

// ── Canonical body builder ───────────────────────────────────────────────────
//
// Builder item 6: single source of truth for /api/vram-estimate parameters.
// Every JS surface (wizard, presets, welcome cards, Model Library, HF preview)
// must use this to construct its request body. Identical inputs → identical
// API result → identical displayed breakdown.
//
// @param {Object} params — canonical estimate parameters
// @param {string} params.backend — 'llama_cpp' or 'rapid_mlx'
// @param {string} params.model_path — local path or HF-repo-style alias (Rapid)
// @param {string|null} params.hf_repo_id — explicit HF repo (optional when model_path is alias)
// @param {string|null} params.hf_file_path — HF file path for GGUF introspection
// @param {number|null} params.model_size_bytes — explicit size override
// @param {number} params.n_ctx — context size (tokens)
// @param {number} params.parallel_slots — parallel generation slots
// @param {number} params.ubatch_size — ubatch size
// @param {string} params.ctk — KV key quant (llama.cpp only; e.g. 'q8_0')
// @param {string} params.ctv — KV value quant (llama.cpp only; e.g. 'q8_0')
// @param {string|null} params.kv_cache_dtype — Rapid KV dtype: 'bf16'|'int8'|'int4'
// @param {boolean} params.reasoning_mode — Rapid reasoning mode (pins KV to int8)
// @param {string|null} params.turboquant_mode — Rapid TurboQuant: 'v4'|'k8v4'|'none'
// @param {number} params.n_cpu_moe — MoE layers on CPU
// @param {number|null} params.gpu_layers — GPU layer count (-1 = auto/all)
// @param {number} params.available_vram_bytes — available VRAM/RAM budget
// @param {number} params.available_ram_bytes — available system RAM
// @param {boolean} params.is_unified_memory — unified memory (Apple Silicon)
// @param {string|null} params.mmproj_path — vision projector path
// @param {number} params.mmproj_bytes — vision projector size override
// @param {string|null} params.workload_scenario — scenario key (e.g. 'coding_agent')
// @param {number|null} params.rapid_planning_context_tokens — Rapid planning context tokens
// @param {number|null} params.rapid_retained_cache_tokens — Rapid retained cache tokens
// @param {string|null} params.client_type — 'app' or 'external_client'
// @param {string|null} params.concurrency_policy — 'single_active' or 'allow_overlap'
// @param {Object|null} params.mtp_config — MTP configuration object
// @returns {Object} request body ready for JSON.stringify
export function buildEstimateBody(params) {
  const body = {
    backend: params.backend || 'llama_cpp',
    model_path: params.model_path || '',
    n_ctx: params.n_ctx || 4096,
    parallel_slots: params.parallel_slots || 1,
    ubatch_size: params.ubatch_size || 2048,
    n_cpu_moe: params.n_cpu_moe || 0,
    gpu_layers: Number.isFinite(params.gpu_layers) ? params.gpu_layers : -1,
    available_vram_bytes: params.available_vram_bytes || 0,
    available_ram_bytes: params.available_ram_bytes || 0,
    is_unified_memory: !!params.is_unified_memory,
    mmproj_path: params.mmproj_path || '',
    mmproj_bytes: params.mmproj_bytes || 0,
  };

  // llama.cpp KV quant (ignored by Rapid path; kept for backward compatibility).
  if (params.backend !== 'rapid_mlx') {
    body.ctk = params.ctk || 'q8_0';
    body.ctv = params.ctv || 'q8_0';
  }

  // HF coordinates (pre-download introspection / Rapid alias resolution).
  if (!body.model_path && params.hf_repo_id && params.hf_file_path) {
    body.hf_repo_id = params.hf_repo_id;
    body.hf_file_path = params.hf_file_path;
    body.model_size_bytes = params.model_size_bytes || 0;
  } else if (params.hf_repo_id && params.hf_file_path) {
    body.hf_repo_id = params.hf_repo_id;
    body.hf_file_path = params.hf_file_path;
    body.model_size_bytes = params.model_size_bytes || 0;
  }

  // Rapid-MLX execution policy (Phase 5a Part 5: requested/effective distinction).
  if (params.backend === 'rapid_mlx') {
    if (params.kv_cache_dtype) body.kv_cache_dtype = params.kv_cache_dtype;
    if (params.reasoning_mode) body.reasoning_mode = params.reasoning_mode;
    if (params.turboquant_mode) body.turboquant_mode = params.turboquant_mode;
  }

  // Workload scenario (Phase 5a Part 4: scenario-aware estimates).
  if (params.workload_scenario) body.workload_scenario = params.workload_scenario;
  if (params.rapid_planning_context_tokens != null)
    body.rapid_planning_context_tokens = params.rapid_planning_context_tokens;
  if (params.rapid_retained_cache_tokens != null)
    body.rapid_retained_cache_tokens = params.rapid_retained_cache_tokens;
  if (params.client_type) body.client_type = params.client_type;
  if (params.concurrency_policy) body.concurrency_policy = params.concurrency_policy;
  if (params.mtp_config) body.mtp_config = params.mtp_config;

  return body;
}

// ── Fetch estimate (canonical API call) ──────────────────────────────────────
//
// All surfaces use this to ensure identical API consumption.
async function fetchEstimate(body) {
  if (!body.model_path && !(body.hf_repo_id && body.hf_file_path)) return null;
  try {
    const headers = (window.authHeaders ? window.authHeaders() : {}) ;
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

// ── Legacy wrapper for wizardState ────────────────────────────────────────────
//
// Builds the request body for /api/vram-estimate from the given wizardState.
// Provided for backward compatibility with existing wizard wiring.

function buildEstimateBodyFromWizardState(state) {
  const hw = state.hardware;
  const m = state.model;
  const gpuLayers = hw.gpuLayers === 'manual'
    ? (Number.isFinite(Number(hw.gpuLayersManual)) ? Number(hw.gpuLayersManual) : -1)
    : -1;

  const modelPath = m.path || m.localPath || '';
  const backend = state.engine?.selected === 'rapid_mlx' ? 'rapid_mlx' : 'llama_cpp';

  return buildEstimateBody({
    backend,
    model_path: modelPath,
    n_ctx: hw.contextSize || 4096,
    parallel_slots: hw.parallelSlots || 1,
    ubatch_size: hw.ubatchSize || 2048,
    ctk: hw.cacheTypeK || 'q8_0',
    ctv: hw.cacheTypeV || 'q8_0',
    n_cpu_moe: hw.nCpuMoe || 0,
    gpu_layers: gpuLayers,
    available_vram_bytes: state.vram?.available || 0,
    available_ram_bytes: state.vram?.availableRam || 0,
    is_unified_memory: state.vram?.isUnifiedMemory || false,
    mmproj_path: m.mmprojPath || '',
    mmproj_bytes: m.mmprojBytes || 0,
    hf_repo_id: m.originRepo || null,
    hf_file_path: m.hfFile || null,
    model_size_bytes: m.modelBytes || null,
    // Rapid execution policy fields (when wizard populates them).
    kv_cache_dtype: hw.kvCacheDtype || null,
    reasoning_mode: hw.reasoningMode || false,
    turboquant_mode: hw.turboquantMode || null,
    // Workload scenario (when wizard populates it).
    workload_scenario: state.workloadScenario || null,
    rapid_planning_context_tokens: hw.rapidPlanningContextTokens || null,
    rapid_retained_cache_tokens: hw.rapidRetainedCacheTokens || null,
    client_type: hw.clientType || null,
    concurrency_policy: hw.concurrencyPolicy || null,
    mtp_config: hw.mtpConfig || null,
  });
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
    const body = buildEstimateBodyFromWizardState(wizardState);
    const est = await fetchEstimate(body);
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
