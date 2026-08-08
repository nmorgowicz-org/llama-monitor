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
