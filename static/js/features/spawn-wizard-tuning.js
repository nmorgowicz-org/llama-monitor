// Spawn wizard performance-advisor apply/auto-tune/sweep actions.
import { showToast } from './toast.js';
import { suggestionPatch, requestNcpuMoeTune, requestDepthSweep, renderDepthSweep, requestBatchSweep, renderBatchSweep } from './tuning-cards.js';
import { dom, wizardState, getSizingArch, getModelBytes, effectiveAvailBytes, isUnifiedMemory, updateAdvisor } from './spawn-wizard.js';

// Map an advisor suggestion's patch onto the wizard controls. We drive the real
// DOM inputs and dispatch their events so existing handlers keep wizardState in
// sync, then refresh the advice.
export function applyWizardSuggestion(suggestion) {
  const patch = suggestionPatch(suggestion);
  const map = {
    ctk: { id: 'spawn-cache-type-k', evt: 'change' },
    ctv: { id: 'spawn-cache-type-v', evt: 'change' },
    context_size: { id: 'spawn-context-size', evt: 'input' },
    spec_draft_n_max: { id: 'hw-mtp-depth', evt: 'input' },
  };
  Object.entries(patch).forEach(([k, v]) => {
    if (k === 'spec_type') {
      const useMtp = String(v).includes('draft-mtp');
      const cb = document.getElementById('hw-use-mtp');
      if (cb && cb.checked !== useMtp) {
        cb.checked = useMtp;
        cb.dispatchEvent(new Event('change', { bubbles: true }));
      }
      return;
    }
    const m = map[k];
    const el = m && document.getElementById(m.id);
    if (!el) return;
    el.value = String(v);
    el.dispatchEvent(new Event(m.evt, { bubbles: true }));
  });
  updateAdvisor();
  showToast('Applied', 'success', suggestion.label);
}

// Auto-tune n_cpu_moe: instant estimate, or empirical llama-bench sweep (verify).
export async function autoTuneWizard(verify) {
  const statusEl = document.getElementById('spawn-moe-autotune-status');
  const arch = getSizingArch();
  const hw = wizardState.hardware;
  const m = wizardState.model;
  const body = {
    name: (m.path || m.hfRepo || '').split('/').pop() || '',
    param_b: arch.paramB || m.paramB || 0,
    model_size_bytes: getModelBytes(),
    available_vram_bytes: effectiveAvailBytes(),
    ubatch_size: hw.ubatchSize || 2048,
    is_unified_memory: isUnifiedMemory(),
    verify: !!verify,
  };
  if (verify) {
    if (!m.path) { showToast('Verify needs a local model file', 'warn'); return; }
    body.model_path = m.path;
    body.ngl = -1; // -1 → -ngl all
    body.ctk = hw.cacheTypeK;
    body.ctv = hw.cacheTypeV;
    body.flash_attn = hw.flashAttn === 'on';
  }
  if (statusEl) {
    statusEl.textContent = verify
      ? 'Running sweep… this can take a few minutes'
      : 'Estimating…';
  }
  try {
    const data = await requestNcpuMoeTune(body);
    if (data.error) { if (statusEl) statusEl.textContent = data.error; return; }
    const rec = data.recommended_n_cpu_moe;
    const input = document.getElementById('spawn-n-cpu-moe');
    if (input) {
      input.value = String(rec);
      input.dispatchEvent(new Event('input', { bubbles: true }));
    }
    if (statusEl) {
      statusEl.textContent = data.verified ? `Verified best: ${rec} (measured)` : `Estimated: ${rec}`;
    }
  } catch {
    if (statusEl) statusEl.textContent = 'Auto-tune failed';
  }
}

// Batch/ubatch sweep: find optimal PP throughput across common batch/ubatch combos.
export async function runBatchSweep() {
  const statusEl = document.getElementById('wizard-batch-sweep-status');
  const resultsEl = document.getElementById('wizard-batch-sweep-results');
  const hw = wizardState.hardware;
  const m = wizardState.model;
  if (!m.path || !m.path.toLowerCase().endsWith('.gguf')) {
    showToast('Batch sweep needs a local .gguf file', 'warn');
    return;
  }
  const body = {
    model_path: m.path,
    ngl: 99,
    ctk: hw.cacheTypeK,
    ctv: hw.cacheTypeV,
    flash_attn: hw.flashAttn === 'on',
    n_cpu_moe: hw.nCpuMoe || null,
    prompt_tokens: 2048,
  };
  if (statusEl) statusEl.textContent = 'Running… testing 10 batch/ubatch combinations, ~5–10 min total.';
  if (resultsEl) resultsEl.replaceChildren();
  try {
    const data = await requestBatchSweep(body);
    if (data.error) { if (statusEl) statusEl.textContent = data.error; return; }
    renderBatchSweep(resultsEl, data.probes || [], data.recommended_batch_size, data.recommended_ubatch_size);
    if (data.recommended_batch_size != null) {
      if (statusEl) statusEl.textContent =
        `Best: batch=${data.recommended_batch_size}, ubatch=${data.recommended_ubatch_size}. Apply to use these values.`;
      // Auto-apply recommended values to wizard state
      wizardState.hardware.batchSize = data.recommended_batch_size;
      wizardState.hardware.ubatchSize = data.recommended_ubatch_size;
      if (dom.batchSizeInput) dom.batchSizeInput.value = data.recommended_batch_size;
      if (dom.ubatchSizeInput) dom.ubatchSizeInput.value = data.recommended_ubatch_size;
    } else {
      if (statusEl) statusEl.textContent = 'Sweep complete — no successful probes.';
    }
  } catch {
    if (statusEl) statusEl.textContent = 'Batch sweep failed';
  }
}

// Depth sweep: measure decode/prefill at several context depths via llama-bench.
export async function runDepthSweep() {
  const statusEl = document.getElementById('wizard-depth-sweep-status');
  const resultsEl = document.getElementById('wizard-depth-sweep-results');
  const hw = wizardState.hardware;
  const m = wizardState.model;
  if (!m.path || !m.path.toLowerCase().endsWith('.gguf')) {
    showToast('Depth sweep needs a local .gguf file', 'warn');
    return;
  }
  const ctx = hw.contextSize || 32768;
  const depths = [0, 16384, 32768, 65536, 131072].filter((d) => d === 0 || d < ctx);
  const body = {
    model_path: m.path,
    ngl: 99,
    ctk: hw.cacheTypeK,
    ctv: hw.cacheTypeV,
    flash_attn: hw.flashAttn === 'on',
    batch_size: hw.batchSize || 2048,
    ubatch_size: hw.ubatchSize || 512,
    n_cpu_moe: hw.nCpuMoe || null,
    depths,
  };
  if (statusEl) statusEl.textContent = 'Running… llama-bench reloads per depth, so this can take several minutes.';
  if (resultsEl) resultsEl.replaceChildren();
  try {
    const data = await requestDepthSweep(body);
    if (data.error) { if (statusEl) statusEl.textContent = data.error; return; }
    if (statusEl) statusEl.textContent = '';
    renderDepthSweep(resultsEl, data.points || []);
  } catch {
    if (statusEl) statusEl.textContent = 'Depth sweep failed';
  }
}
