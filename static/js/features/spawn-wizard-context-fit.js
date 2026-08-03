// Context-size quick-picks, native-context warnings, and use-case fit warnings
// for the spawn wizard hardware step.
import { dom, wizardState } from './spawn-wizard.js';
import { updateVramDisplay } from './spawn-wizard-vram-display.js';

// ── Context quick-picks ───────────────────────────────────────────────────────

export function bindCtxQuickPicks() {
  document.querySelectorAll('.ctx-pick').forEach(btn => {
    btn.addEventListener('click', () => {
      const ctx = parseInt(btn.dataset.ctx, 10);
      if (!ctx) return;
      const nativeCap = wizardState.model.nCtxTrain || 0;
      if (nativeCap > 0 && ctx > nativeCap) {
        updateCtxTrainWarning();
        return;
      }
      if (dom.contextSizeInput) dom.contextSizeInput.value = ctx;
      wizardState.hardware.contextSize = ctx;
      updateCtxQuickPickActive();
      showCtxFitWarning(ctx, wizardState.useCase, true); // manual — no fit warning
      updateCtxTrainWarning();
      updateVramDisplay();
    });
  });

  // Manual input: update state, chip highlight, and both warning types
  dom.contextSizeInput?.addEventListener('change', () => {
    const ctx = parseInt(dom.contextSizeInput.value, 10);
    if (!ctx) return;
    wizardState.hardware.contextSize = ctx;
    updateCtxQuickPickActive();
    showCtxFitWarning(ctx, wizardState.useCase, true);
    updateCtxTrainWarning();
    updateVramDisplay();
  });
}

export function updateCtxQuickPickActive() {
  const current = parseInt(dom.contextSizeInput?.value || '0', 10);
  const nativeCap = wizardState.model.nCtxTrain || 0;
  document.querySelectorAll('.ctx-pick').forEach(btn => {
    if (!btn.dataset.nativeTitle) btn.dataset.nativeTitle = btn.title;
    const value = parseInt(btn.dataset.ctx, 10);
    const advancedOnly = nativeCap > 0 && value > nativeCap;
    btn.classList.toggle('active', value === current);
    btn.classList.toggle('ctx-pick-advanced', advancedOnly);
    btn.setAttribute('aria-disabled', String(advancedOnly));
    btn.title = advancedOnly
      ? `${Math.round(value / 1024)}k exceeds this model's native ${Math.round(nativeCap / 1024)}k context. Advanced context extension is not configured.`
      : btn.dataset.nativeTitle;
  });
}

// Show the model's training context ceiling near the context size input so users
// always know the limit before accidentally exceeding it.
export function updateCtxModelMaxHint() {
  const hint = document.getElementById('ctx-model-max-hint');
  const btn  = document.getElementById('ctx-pick-model-max');
  const nCtxTrain = wizardState.model.nCtxTrain || 0;
  if (!nCtxTrain) {
    if (hint) hint.style.display = 'none';
    if (btn)  btn.style.display  = 'none';
    return;
  }
  const fmtK = n => n >= 1000 ? `${Math.round(n / 1024)}k` : `${n}`;
  const label = fmtK(nCtxTrain);
  if (hint) {
    hint.textContent = `model max: ${label}`;
    hint.style.display = '';
  }
  if (btn) {
    btn.dataset.ctx = nCtxTrain;
    btn.childNodes[0].textContent = label + ' ';
    btn.style.display = '';
    // Remove duplicate if there's already a static pick with the same value
    document.querySelectorAll('.ctx-pick:not(#ctx-pick-model-max)').forEach(el => {
      if (Number(el.dataset.ctx) === nCtxTrain) btn.style.display = 'none';
    });
  }
}

// Native context is a hard supported ceiling in the standard flow. Extension
// parameters are intentionally out of scope until their per-model support and
// memory math are qualified.
export function updateCtxTrainWarning() {
  const el = document.getElementById('ctx-train-warning');
  if (!el) return;
  const nCtxTrain = wizardState.model.nCtxTrain;
  const selected  = wizardState.hardware.contextSize;
  if (!nCtxTrain || !selected || selected <= nCtxTrain) {
    el.style.display = 'none';
    return;
  }
  const fmtK = n => n >= 1024 ? `${Math.round(n / 1024)}k` : `${n}`;
  el.textContent = '';
  const strong = document.createElement('strong');
  strong.textContent = `Context (${fmtK(selected)}) exceeds this model's native window (${fmtK(nCtxTrain)})`;
  el.appendChild(strong);
  el.appendChild(document.createTextNode(
    '. This is outside the model’s supported and benchmarked context. Context extension (RoPE/YaRN and its memory validation) requires Advanced Context controls, which are not configured in this release.'
  ));
  el.className = 'ctx-fit-warning';
  el.style.display = '';
  updateCtxModelMaxHint();
}

// Minimum-context guidance: warn when auto-size lands below the target for the use case.
// Never warn when the user has manually typed or picked a high value — that's intentional.
export const CTX_TARGETS = { agentic: 131072, general: 32768, roleplay: 65536 };

export function showCtxFitWarning(ctx, useCase, manualSet = false) {
  const el = document.getElementById('ctx-fit-warning');
  if (!el) return;

  // If user explicitly chose this value (chip click or manual type), no warning
  if (manualSet) { el.style.display = 'none'; return; }

  const target = CTX_TARGETS[useCase] ?? 0;
  if (!target || ctx >= target) { el.style.display = 'none'; return; }

  const fmtCtx = c => c >= 1024 ? `${Math.round(c / 1024)}k` : `${c}`;
  const got = fmtCtx(ctx), need = fmtCtx(target);

  el.textContent = '';
  const strong = document.createElement('strong');
  if (useCase === 'agentic') {
    strong.textContent = `Can't reach ${need} for agentic work`;
    el.appendChild(strong);
    el.appendChild(document.createTextNode(
      ` — auto-sized to ${got} with the high-precision cache mode. Try a smaller model quant like Q4_K_M or IQ3_XXS to shrink weights, or override by typing a custom value.`
    ));
  } else if (useCase === 'roleplay') {
    strong.textContent = `Below ${need} RP target`;
    el.appendChild(strong);
    el.appendChild(document.createTextNode(
      ` — auto-sized to ${got}. Try the More context mode or use a smaller model quant if long transcripts matter more than cache precision.`
    ));
  } else {
    el.appendChild(document.createTextNode(
      `Auto-size returned ${got} (target ${need}). Consider a smaller quantization.`
    ));
  }
  el.className = ctx < target * 0.5 ? 'ctx-fit-warning ctx-fit-error' : 'ctx-fit-warning';
  el.style.display = '';
}
