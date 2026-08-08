// spawn-wizard-guided.js — Option A (Guided) decision cards wiring
import { wizardState } from './spawn-wizard.js';

// Wire up decision cards on step entry
export function initGuidedCards() {
  wireContextTiles();
  wireKvTiles();
  wireVisionCard();
  wireSpeedBoost();
  wireAllSettingsDrawer();
  wireStickyBar();
}

// Card 1: Context size tiles sync with #spawn-context-size
function wireContextTiles() {
  const tiles = document.querySelectorAll('#hw-ctx-tiles .hw-decision-tile');
  const customInput = document.getElementById('hw-ctx-custom');
  const origInput = document.getElementById('spawn-context-size');

  function setTileActive(ctx) {
    tiles.forEach(t => {
      t.classList.toggle('hw-decision-tile-active', t.dataset.ctx === ctx);
    });
  }

  tiles.forEach(tile => {
    tile.addEventListener('click', () => {
      const ctx = tile.dataset.ctx;
      if (customInput) customInput.value = ctx;
      if (origInput) origInput.value = ctx;
      setTileActive(ctx);
      // Trigger input event for existing handlers
      customInput?.dispatchEvent(new Event('input', { bubbles: true }));
      origInput?.dispatchEvent(new Event('input', { bubbles: true }));
    });
  });

  // Custom input updates tiles
  if (customInput) {
    customInput.addEventListener('change', () => {
      const ctx = customInput.value;
      setTileActive(ctx);
    });
  }

  // Sync from original input (when set by other code)
  if (origInput) {
    origInput.addEventListener('input', () => {
      const ctx = origInput.value;
      if (customInput) customInput.value = ctx;
      setTileActive(ctx);
    });
  }
}

// Card 2: KV precision tiles sync with #spawn-cache-type-k/v
function wireKvTiles() {
  const tiles = document.querySelectorAll('#hw-kv-tiles .hw-decision-tile');
  const kInput = document.getElementById('spawn-cache-type-k');
  const vInput = document.getElementById('spawn-cache-type-v');

  function setTileActive(kv) {
    tiles.forEach(t => {
      t.classList.toggle('hw-decision-tile-active', t.dataset.kv === kv);
    });
  }

  tiles.forEach(tile => {
    tile.addEventListener('click', () => {
      const kv = tile.dataset.kv;
      if (kInput) kInput.value = kv;
      if (vInput) vInput.value = kv;
      setTileActive(kv);
      kInput?.dispatchEvent(new Event('change', { bubbles: true }));
      vInput?.dispatchEvent(new Event('change', { bubbles: true }));
    });
  });

  // Sync from original inputs
  function syncFromOrig() {
    const kv = kInput?.value || vInput?.value;
    if (kv) setTileActive(kv);
  }
  kInput?.addEventListener('change', syncFromOrig);
  vInput?.addEventListener('change', syncFromOrig);
}

// Card 3: Vision select sync with existing mmproj controls
function wireVisionCard() {
  const visionSelect = document.getElementById('hw-vision-select');
  const origSelect = document.getElementById('hw-mmproj-select');

  if (visionSelect && origSelect) {
    // Populate vision select from original select options
    function syncOptions() {
      const options = Array.from(origSelect.options, option => option.cloneNode(true));
      if (!options.length) {
        const fallback = document.createElement('option');
        fallback.value = '';
        fallback.textContent = '( none — text only )';
        options.push(fallback);
      }
      visionSelect.replaceChildren(...options);
    }
    syncOptions();

    // Keep in sync on change
    visionSelect.addEventListener('change', () => {
      origSelect.value = visionSelect.value;
      origSelect.dispatchEvent(new Event('change', { bubbles: true }));
    });
    origSelect.addEventListener('change', () => {
      visionSelect.value = origSelect.value;
    });
  }
}

// Card 4: Speed boost radios sync with MTP controls
function wireSpeedBoost() {
  const radios = document.querySelectorAll('input[name="hw-speed"]');
  const mtpCheckbox = document.getElementById('hw-use-mtp');

  radios.forEach(radio => {
    radio.addEventListener('change', () => {
      const value = radio.value;
      if (mtpCheckbox) {
        mtpCheckbox.checked = value === 'on';
        mtpCheckbox.dispatchEvent(new Event('change', { bubbles: true }));
      }
    });
  });

  // Sync from MTP checkbox
  if (mtpCheckbox) {
    mtpCheckbox.addEventListener('change', () => {
      const checked = mtpCheckbox.checked;
      const radio = document.querySelector(`input[name="hw-speed"][value="${checked ? 'on' : 'off'}"]`);
      if (radio) radio.checked = true;
    });
  }
}

// All settings drawer toggle
function wireAllSettingsDrawer() {
  const toggle = document.getElementById('hw-all-settings-toggle');
  const panel = document.getElementById('hw-all-settings-panel');

  if (toggle && panel) {
    toggle.addEventListener('click', () => {
      panel.classList.toggle('open');
      const isOpen = panel.classList.contains('open');
      toggle.textContent = isOpen ? '⚙ All settings (23) ▲' : '⚙ All settings (23)';
    });
  }
}

// Sticky context bar: populate from wizardState
function wireStickyBar() {
  function updateStickyBar() {
    const modelEl = document.getElementById('hw-sticky-model-name');
    const quantEl = document.getElementById('hw-sticky-quant');
    const loaderEl = document.getElementById('hw-sticky-loader');
    const usecaseEl = document.getElementById('hw-sticky-usecase');
    const ctxEl = document.getElementById('hw-sticky-ctx');
    const kvEl = document.getElementById('hw-sticky-kv');

    if (modelEl) {
      const modelName = wizardState.model.name || wizardState.model.path || '—';
      modelEl.textContent = modelName.split('/').pop() || modelName;
    }
    if (quantEl) {
      quantEl.textContent = wizardState.model.quant || '—';
    }
    if (loaderEl) {
      loaderEl.textContent = wizardState.engine?.selected === 'rapid_mlx' ? 'Rapid-MLX' : 'llama.cpp';
    }
    if (usecaseEl) {
      const useCaseLabel = wizardState.useCase || 'General';
      usecaseEl.textContent = useCaseLabel;
    }
    if (ctxEl) {
      const ctx = wizardState.hardware?.contextSize || 0;
      if (ctx > 0) {
        const display = ctx >= 1000 ? `${(ctx / 1000).toFixed(0)}k` : ctx;
        ctxEl.textContent = `ctx ${display}`;
      }
    }
    if (kvEl) {
      const kCache = document.getElementById('spawn-cache-type-k');
      if (kCache && kCache.value) {
        kvEl.textContent = `KV ${kCache.value}`;
      }
    }
  }

  // Initial update and on state changes
  updateStickyBar();
  new MutationObserver(updateStickyBar).observe(document.body, {
    subtree: true,
    attributes: true,
    attributeFilter: ['value']
  });
}
