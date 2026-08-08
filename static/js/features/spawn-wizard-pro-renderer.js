// Pro renderer: dense multi-column layout that replaces the card+drawer in Guided mode.
// Each supersection becomes a section header; each group becomes a subsection with
// controls in a compact grid. All controls visible — nothing collapsed.

import {
  renderHardwareModelHeader,
} from './spawn-wizard-hardware-model.js';

/** Render the Pro layout — all controls visible in dense grid, nothing collapsed. */
export function renderProLayout() {
  const main = document.querySelector('#wizard-step-1 .wizard-main');
  const drawer = document.getElementById('all-settings-drawer');
  if (!main) return;

  // Hide the "All settings" drawer (everything is visible in Pro)
  if (drawer) drawer.style.display = 'none';

  // Hide card-based controls (context size card, KV precision card, etc.)
  const cards = main.querySelectorAll(
    '#spawn-context-size, #spawn-cache-type-k-wrap, #spawn-cache-type-v-wrap, .hw-mmproj-row, .hw-mtp-row, #spawn-rapid-reasoning-mode-wrap'
  );
  cards.forEach(c => {
    c.style.display = 'none';
    c.dataset.proCardHidden = '1';
  });

  // Remove any existing Pro layout
  const existing = document.getElementById('pro-layout');
  if (existing) existing.remove();

  const proLayout = document.createElement('div');
  proLayout.id = 'pro-layout';
  proLayout.className = 'pro-layout';

  // Add Pro layout after the model header, before hardware controls
  const modelHeader = document.getElementById('hw-model-header');
  if (modelHeader) {
    modelHeader.after(proLayout);
  } else {
    main.insertBefore(proLayout, main.firstChild);
  }

  // Pro header bar with quick filter, modified-only, reset
  const headerBar = document.createElement('div');
  headerBar.id = 'pro-header-bar';
  headerBar.className = 'pro-header-bar';
  headerBar.innerHTML = `
    <input type="text" class="pro-filter-input" id="pro-filter-input" placeholder="⌘K Find a setting…" style="display:none;" />
    <button type="button" class="pro-filter-btn" id="pro-filter-btn" title="Press ⌘K / Ctrl+K to find a setting">⌘K</button>
    <label class="pro-modified-only" id="pro-modified-only">
      <input type="checkbox" id="pro-modified-only-check" />
      <span class="pro-modified-only-label">☐ Modified only (<span id="pro-modified-count">0</span>)</span>
    </label>
    <button type="button" class="pro-reset-all" id="pro-reset-all" title="Reset all changed settings to defaults">↺ Reset all</button>
  `;
  proLayout.appendChild(headerBar);

  // ── Quick filter (⌘K) ──────────────────────────────────────────────────────
  const filterInput = document.getElementById('pro-filter-input');
  const filterBtn = document.getElementById('pro-filter-btn');

  // Global keyboard shortcut
  document.addEventListener('keydown', (e) => {
    if ((e.metaKey || e.ctrlKey) && e.key === 'k') {
      e.preventDefault();
      filterInput.style.display = 'block';
      filterInput.focus();
    }
    if (e.key === 'Escape' && filterInput.style.display === 'block') {
      filterInput.style.display = 'none';
      filterInput.value = '';
      filterInput.dispatchEvent(new Event('input'));
    }
  });

  // Search input: filter controls by label/key match
  filterInput?.addEventListener('input', () => {
    const q = filterInput.value.toLowerCase().trim();
    const allGroups = proLayout.querySelectorAll('.pro-group, .pro-section');
    allGroups.forEach(g => {
      const text = g.textContent.toLowerCase();
      g.style.display = (!q || text.includes(q)) ? '' : 'none';
    });
  });

  filterBtn?.addEventListener('click', () => {
    filterInput.style.display = 'block';
    filterInput.focus();
  });

  // ── Modified only ───────────────────────────────────────────────────────────
  const modifiedOnlyCheck = document.getElementById('pro-modified-only-check');
  const modifiedCountEl = document.getElementById('pro-modified-count');
  const allControls = new Map(); // controlId → { defaultVal, currentValue }

  // Scan all form controls and record defaults vs current values
  const scanAllControls = () => {
    const inputs = main.querySelectorAll('input, select, textarea');
    for (const input of inputs) {
      const id = input.id;
      if (!id) continue;
      const field = input.closest('.hardware-field, .kv-inline-row');
      if (!field && !input.closest('.hardware-grid, #pro-layout')) continue;

      const defaultVal = getDefaultForControl(id);
      const currentVal = getControlValue(input);
      allControls.set(id, { defaultVal, currentValue: currentVal, element: input });
    }
    updateModifiedCount();
  };

  const getDefaultForControl = (id) => {
    // Map control IDs to their shipped defaults
    const defaults = {
      'spawn-batch-size': '2048',
      'spawn-ubatch-size': '2048',
      'spawn-parallel-slots': '1',
      'spawn-flash-attn': 'on',
      'spawn-threads': '',
      'spawn-threads-batch': '',
      'spawn-prio': 'default',
      'spawn-n-cpu-moe': 'auto',
      'spawn-tensor-split': '',
      'spawn-cache-mode': 'custom',
      'spawn-cache-ram': '',
      'spawn-fit-enable': '',
      'spawn-fit-target': '',
      'spawn-mlock': 'off',
      'spawn-rapid-max-concurrent-requests': '',
      'spawn-rapid-max-num-seqs': '',
      'spawn-rapid-gpu-memory-utilization': '',
      'spawn-rapid-pflash-policy': 'off',
      'spawn-rapid-auto-tool-choice': 'off',
      'spawn-rapid-speculative-enabled': 'off',
      'spawn-rapid-prefill-step-size': '512',
      'spawn-rapid-reasoning-mode': 'on',
      'spawn-rapid-completion-batch-size': '',
      'spawn-rapid-prefill-batch-size': '',
      'spawn-sampling-mode': 'auto',
      'spawn-retained-cache-mib': '8192',
      'spawn-rapid-hybrid-cache-entries': '16',
      'spawn-gpu-layers': 'auto',
      'spawn-temperature': '',
      'spawn-top-p': '',
      'spawn-top-k': '',
      'spawn-min-p': '',
      'spawn-repeat-penalty': '',
      'spawn-presence-penalty': '',
      'spawn-max-tokens': '',
      'spawn-seed': '',
      'spawn-rapid-reasoning-parser': '',
      'spawn-rapid-tool-call-parser': '',
      'spawn-rapid-hybrid-mode': 'auto',
    };
    return defaults[id] ?? '';
  };

  const getControlValue = (el) => {
    if (el.type === 'checkbox' || el.type === 'radio') return el.checked ? 'on' : 'off';
    return el.value || '';
  };

  const updateModifiedCount = () => {
    let count = 0;
    for (const [id, data] of allControls) {
      if (data.currentValue !== data.defaultVal) count++;
    }
    if (modifiedCountEl) modifiedCountEl.textContent = count;

    // Apply modified-only filter
    const onlyModified = modifiedOnlyCheck?.checked;
    allControls.forEach((data, id) => {
      const el = data.element;
      if (!el) return;
      const isModified = data.currentValue !== data.defaultVal;
      const parent = el.closest('.pro-control, .hardware-field, .kv-inline-row');
      if (!parent) return;
      parent.classList.toggle('pro-modified', isModified);
      parent.classList.toggle('pro-at-default', !isModified);
      if (onlyModified) {
        parent.style.display = isModified ? '' : 'none';
      }
    });
  };

  // Re-scan on input/change events
  main.addEventListener('input', scanAllControls);
  main.addEventListener('change', scanAllControls);

  // Scan after a short delay to let initial values settle
  setTimeout(scanAllControls, 100);

  // ── Reset all ───────────────────────────────────────────────────────────────
  document.getElementById('pro-reset-all')?.addEventListener('click', () => {
    for (const [id, data] of allControls) {
      if (data.currentValue !== data.defaultVal) {
        const el = data.element;
        if (el.type === 'checkbox' || el.type === 'radio') {
          el.checked = data.defaultVal === 'on';
        } else if (el.tagName === 'SELECT') {
          const selVal = el.options[0]?.value ?? '';
          el.value = data.defaultVal || selVal;
        } else {
          el.value = data.defaultVal;
        }
      }
    }
    // Trigger input event to update display
    main.dispatchEvent(new Event('input'));
  });

  // Use the active engine's IA file (not both)
  const rapid = (window.wizardState?.engine?.selected || '') === 'rapid_mlx';
  const llamaIA = window.spawnWizardLlamaIA;
  const mlxIA = window.spawnWizardMlxIA;
  const iaFile = rapid ? mlxIA : llamaIA;
  if (!iaFile) return;

  const groups = iaFile.GROUPS || [];
  const supersections = iaFile.SUPERSECTIONS || [];

  // Group by supersection
  const supersectionMap = new Map();
  for (const g of groups) {
    if (!supersectionMap.has(g.supersection)) {
      supersectionMap.set(g.supersection, []);
    }
    supersectionMap.get(g.supersection).push(g);
  }

  for (const [ssId, ssGroups] of supersectionMap) {
    const ssInfo = supersections.find(s => s.id === ssId);
    if (!ssInfo) continue;

    const section = document.createElement('section');
    section.className = 'pro-section';
    section.dataset.proSection = ssId;

    const header = document.createElement('div');
    header.className = 'pro-section-header';
    /* eslint-disable-next-line no-unsanitized/property */
    header.innerHTML = `<h3 class="pro-section-title">${ssInfo.title}</h3><p class="pro-section-desc">${ssInfo.description || ''}</p>`;
    section.appendChild(header);

    for (const group of ssGroups) {
      const groupEl = document.createElement('div');
      groupEl.className = 'pro-group';
      groupEl.dataset.proGroup = group.id;

      const groupHeader = document.createElement('div');
      groupHeader.className = 'pro-group-header';
      /* eslint-disable-next-line no-unsanitized/property */
      groupHeader.innerHTML = `<h4 class="pro-group-title">${group.title}</h4><p class="pro-group-desc">${group.description || ''}</p>`;
      groupEl.appendChild(groupHeader);

      const grid = document.createElement('div');
      grid.className = 'pro-controls-grid';

      for (const controlId of group.controls) {
        const controlEl = document.getElementById(controlId);
        if (controlEl) {
          const field = controlEl.closest('.hardware-field, .kv-inline-row') || controlEl;
          const clone = field.cloneNode(true);
          clone.classList.add('pro-control');
          grid.appendChild(clone);
        }
      }

      groupEl.appendChild(grid);
      section.appendChild(groupEl);
    }

    proLayout.appendChild(section);
  }
}

/** Restore the Guided layout — show card controls, show drawer. */
export function restoreGuidedLayout() {
  const main = document.querySelector('#wizard-step-1 .wizard-main');
  const drawer = document.getElementById('all-settings-drawer');

  // Show the "All settings" drawer
  if (drawer) {
    drawer.style.display = '';
    // Reset to hidden body, shown button
    const body = document.getElementById('all-settings-body');
    if (body) body.style.display = 'none';
  }

  // Show card controls
  const main2 = document.querySelector('#wizard-step-1 .wizard-main');
  if (main2) {
    main2.querySelectorAll('[data-pro-card-hidden]')
      .forEach(c => {
        c.style.display = '';
        c.removeAttribute('data-pro-card-hidden');
      });
  }

  // Remove Pro layout
  const proLayout = document.getElementById('pro-layout');
  if (proLayout) proLayout.remove();
}
