// ── Setup / Monitor View ──────────────────────────────────────────────────────
// View transitions, animations, quick stats, and view state initialization.

import { setupViewState, chat, sessionState } from '../core/app-state.js';
import { doAttachFromSetup } from './attach-detach.js';
import { escapeHtml } from '../core/format.js';
import { showToast, showConfirmDialog } from './toast.js';

// ── Model / preset classification (from GGUF-derived metadata) ────────────────
// No name-based guessing: labels come from preset.family and preset.size_class
// which are derived from GGUF metadata (architecture, parameter_count).
// For presets missing metadata, we intentionally show no family/size badge
// instead of guessing.

const FAMILY_LABEL_MAP = {
    qwen36: 'Qwen3.6',
    qwen35: 'Qwen3.5',
    qwen3: 'Qwen3',
    llama3: 'Llama 3.x',
    gemma4: 'Gemma4',
    gemma: 'Gemma',
    mistral: 'Mistral',
    exaone: 'EXAONE',
    heretic: 'Heretic',
    deepseek: 'DeepSeek',
    phi: 'Phi',
};

function classifyPreset(preset) {
    const family = preset.family || null;
    const sizeClass = preset.size_class || 'unknown';
    return { family, sizeClass };
}

// ── Quantization tag extraction ───────────────────────────────────────────────

function extractQuantFromFilename(filename) {
    if (!filename) return null;
    if (/mmproj/i.test(filename)) return null;
    const base = filename.replace(/\.gguf$/i, '');
    // APEX-MTP draft models: extract quality label after -APEX-MTP-
    const mtp = base.match(/-APEX-(?:MTP-)?(.+)$/i);
    if (mtp) return mtp[1];
    // Standard GGUF quant suffix after - or . separator: Q4_K_M, IQ2_XXS, BF16, F16, F32 …
    const m = base.match(/[-.]((IQ\d+|Q\d+)(?:_[A-Z0-9]+)*|BF16|F16|F32)$/i);
    return m ? m[1].toUpperCase() : null;
}

// ── Launch filters state (for filter bar + grouping) ──────────────────────────

const launchFilters = {
    family: null,
    size: null,
    tags: [],
    collection: null,
    groupByFamily: false
};
// ── Memory bar ────────────────────────────────────────────────────────────────
// Same Metal cap logic as effectiveAvailBytes() in spawn-wizard.js.

const _MEM_OS_RESERVE = 512 * 1024 * 1024; // 512 MB

// Cached memory state shared between the memory bar and card VRAM estimates.
let _memState = { availBytes: 0, isUnified: false };

function _metalCap(totalBytes, metalGpuLimitMb) {
    if (metalGpuLimitMb > 0) return metalGpuLimitMb * 1024 * 1024;
    const fraction = totalBytes <= 36 * 1024 ** 3 ? 2 / 3 : 3 / 4;
    return Math.floor(totalBytes * fraction);
}

function _fmtGb(bytes) {
    const gb = bytes / 1024 ** 3;
    return gb >= 10 ? gb.toFixed(0) : gb.toFixed(1);
}

export async function fetchAndRenderMemoryBar() {
    const bar = document.getElementById('setup-mem-bar');
    if (!bar) return;
    try {
        const headers = window.authHeaders ? window.authHeaders() : {};
        const [sysResp, gpuResp, platResp, limResp] = await Promise.all([
            fetch('/metrics/system', { headers }),
            fetch('/metrics/gpu', { headers }),
            fetch('/api/llama-binary/platform-info', { headers }),
            fetch('/api/system/metal-gpu-limit', { headers }),
        ]);

        let totalBytes = 0;
        let ramUsedBytes = 0;
        let metalGpuLimitMb = 0;
        let isUnified = false;
        let vramUsedBytes = 0;

        if (sysResp.ok) {
            const d = await sysResp.json();
            totalBytes = (d.ram_total_gb || 0) * 1024 ** 3;
            ramUsedBytes = (d.ram_used_gb || 0) * 1024 ** 3;
        }

        if (gpuResp.ok) {
            const data = await gpuResp.json();
            const gpus = Array.isArray(data) ? data : (data.gpus ? data.gpus : Object.values(data));
            for (const g of gpus) {
                const tMb = g.vram_total_mb || g.total_mb || g.total_memory_mb || g.vram_total || 0;
                const uMb = g.vram_used_mb || g.used_mb || g.vram_used || 0;
                if (g.metal_gpu_limit_mb !== undefined) {
                    isUnified = true;
                    metalGpuLimitMb = g.metal_gpu_limit_mb || 0;
                    if (!totalBytes) totalBytes = tMb * 1024 * 1024;
                } else {
                    totalBytes = totalBytes || tMb * 1024 * 1024;
                    vramUsedBytes += uMb * 1024 * 1024;
                }
            }
        }

        // Fallback: if GPU metrics haven't populated yet (mactop race on startup),
        // detect unified memory from platform-info (independent of mactop).
        if (!isUnified && platResp.ok) {
            const plat = await platResp.json();
            if (plat.auto_backend === 'metal') {
                isUnified = true;
                if (!metalGpuLimitMb && limResp.ok) {
                    const lim = await limResp.json();
                    metalGpuLimitMb = lim.limit_mb || 0;
                }
            }
        }

        if (!totalBytes) return;

        let availBytes, fillPct, availLabel, totalLabel;

        let vramEstimateBytes;
        if (isUnified) {
            const cap = _metalCap(totalBytes, metalGpuLimitMb);
            availBytes = Math.max(0, Math.min(cap, totalBytes) - _MEM_OS_RESERVE);
            fillPct = Math.round((availBytes / totalBytes) * 100);
            availLabel = _fmtGb(availBytes) + ' GB available for inference';
            totalLabel = _fmtGb(totalBytes) + ' GB unified';
            // Card estimates use current free RAM so high system load shows as risk
            vramEstimateBytes = ramUsedBytes > 0
                ? Math.max(0, Math.min(cap, totalBytes - ramUsedBytes) - _MEM_OS_RESERVE)
                : availBytes;
        } else {
            availBytes = Math.max(0, totalBytes - vramUsedBytes);
            fillPct = Math.round((availBytes / totalBytes) * 100);
            availLabel = _fmtGb(availBytes) + ' GB VRAM free';
            totalLabel = _fmtGb(totalBytes) + ' GB total';
            vramEstimateBytes = availBytes;
        }

        const fill = document.getElementById('setup-mem-bar-fill');
        if (fill) fill.style.width = fillPct + '%';
        const availEl = document.getElementById('setup-mem-bar-avail');
        if (availEl) availEl.textContent = availLabel;
        const totalEl = document.getElementById('setup-mem-bar-total');
        if (totalEl) totalEl.textContent = totalLabel;
        bar.style.display = '';

        _memState = { availBytes, isUnified };
        _fetchCardVramEstimates(vramEstimateBytes, isUnified);
    } catch {
        // leave bar hidden if metrics unavailable
    }
}

// ── Drop zone ─────────────────────────────────────────────────────────────────

export function initSetupDropZone() {
    const zone = document.getElementById('setup-drop-zone');
    if (!zone) return;

    zone.addEventListener('dragover', e => {
        const items = Array.from(e.dataTransfer?.items || []);
        const hasGguf = items.some(i => i.kind === 'file');
        if (!hasGguf) return;
        e.preventDefault();
        e.dataTransfer.dropEffect = 'copy';
        zone.classList.add('is-over');
    });

    zone.addEventListener('dragleave', e => {
        if (!zone.contains(e.relatedTarget)) zone.classList.remove('is-over');
    });

    zone.addEventListener('drop', async e => {
        e.preventDefault();
        zone.classList.remove('is-over');
        const file = e.dataTransfer?.files?.[0];
        if (!file || !file.name.toLowerCase().endsWith('.gguf')) return;

        // Try to resolve the full filesystem path from the known models library
        try {
            const headers = window.authHeaders ? window.authHeaders() : {};
            const resp = await fetch('/api/models', { headers });
            if (resp.ok) {
                const models = await resp.json();
                const match = models.find(m =>
                    (m.path || '').split(/[/\\]/).pop() === file.name
                );
                if (match) {
                    import('./spawn-wizard.js').then(({ openSpawnWizard }) =>
                        openSpawnWizard({ localPath: match.path, localModel: match })
                    );
                    return;
                }
            }
        } catch {}

        // File not in library — open wizard on the model step so user can browse
        import('./spawn-wizard.js').then(({ openSpawnWizard }) => openSpawnWizard({}));
    });

    // Also open wizard on click (as an explicit affordance)
    zone.addEventListener('click', () =>
        import('./spawn-wizard.js').then(({ openSpawnWizard }) => openSpawnWizard({}))
    );
}

function setAttachButtonLabel(button, label) {
    if (!button) return;
    const icon = document.createElement('span');
    icon.className = 'btn-icon';
    icon.textContent = '⚡';
    button.replaceChildren(icon, document.createTextNode(` ${label}`));
}

// ── View Switching ────────────────────────────────────────────────────────────

export function switchView(targetView) {
    if (setupViewState.view === 'transitioning') return;

    const previousView = setupViewState.view;
    setupViewState.view = 'transitioning';

    if (targetView === 'setup' && previousView === 'monitor') {
        savePreviousPosition();
    }

    const currentViewEl = document.getElementById('view-' + previousView);
    const targetViewEl = document.getElementById('view-' + targetView);
    const setupStrip = document.getElementById('endpoint-strip-setup');
    const monitorStrip = document.getElementById('endpoint-strip-monitor');

    if (!currentViewEl || !targetViewEl) {
        setupViewState.view = targetView;
        return;
    }

    if (targetView === 'monitor') {
        currentViewEl.classList.add('exiting');
        setTimeout(() => {
            currentViewEl.style.display = 'none';
            currentViewEl.classList.remove('exiting');
            targetViewEl.style.display = '';
            targetViewEl.classList.add('entering');
            showFlashOverlay();
            animateCardsEnter();
            if (setupStrip) setupStrip.style.display = 'none';
            if (monitorStrip) monitorStrip.style.display = '';
            document.body.classList.remove('setup-active');
            setTimeout(() => {
                targetViewEl.classList.remove('entering');
                setupViewState.view = 'monitor';
            }, 500);
        }, 400);
    } else {
        animateCardsExit();
        if (setupStrip) setupStrip.style.display = '';
        if (monitorStrip) monitorStrip.style.display = 'none';
        document.body.classList.add('setup-active');
        setTimeout(() => {
            currentViewEl.style.display = 'none';
            currentViewEl.classList.remove('exiting');
            targetViewEl.style.display = '';
            targetViewEl.classList.add('entering');
            animateSetupCardsEnter();
            setTimeout(() => {
                targetViewEl.classList.remove('entering');
                setupViewState.view = 'setup';
            }, 400);
        }, 600);
    }
}

// ── Connecting State ──────────────────────────────────────────────────────────

export function showConnectingState() {
    const connectingDots = document.getElementById('connecting-dots');
    if (connectingDots) connectingDots.style.display = '';
}

export function hideConnectingState() {
    const connectingDots = document.getElementById('connecting-dots');
    if (connectingDots) connectingDots.style.display = 'none';
}

// ── Animations ────────────────────────────────────────────────────────────────

function showFlashOverlay() {
    const existing = document.querySelector('.view-flash');
    if (existing) existing.remove();
    const flash = document.createElement('div');
    flash.className = 'view-flash';
    document.body.appendChild(flash);
    setTimeout(() => flash.remove(), 800);
}

function animateCardsEnter() {
    const cards = document.querySelectorAll('.view-monitor .widget-card');
    cards.forEach((card, i) => {
        card.classList.add('entrance');
        setTimeout(() => card.classList.add('active'), 120 * i);
    });
}

function animateCardsExit() {
    const cards = [...document.querySelectorAll('.view-monitor .widget-card')].reverse();
    cards.forEach((card, i) => {
        card.style.transition = `opacity 0.3s ease ${60 * i}ms, transform 0.3s ease ${60 * i}ms`;
        card.style.opacity = '0';
        card.style.transform = 'translateY(16px)';
    });
}

function animateSetupCardsEnter() {
    // no-op: launch cards use CSS animation-delay via inline style
}

// Map of preset_id → last_connected_at (ms), populated by renderRecentEndpoints
const _spawnLastLaunched = new Map();

function _applyLastLaunchedToCards() {
    document.querySelectorAll('.launch-card[data-preset-id]').forEach(card => {
        const presetId = card.dataset.presetId;
        const ts = _spawnLastLaunched.get(presetId);
        let el = card.querySelector('.launch-card-last-launched');
        if (!ts) { if (el) el.remove(); return; }
        if (!el) {
            el = document.createElement('div');
            el.className = 'launch-card-last-launched';
            const modelEl = card.querySelector('.launch-card-model');
            if (modelEl) modelEl.after(el);
            else card.querySelector('.launch-card-chips')?.before(el);
        }
        el.textContent = 'Last launched · ' + formatRelativeTime(ts);
    });
}



// ── Launch Grid — Preset Cards ────────────────────────────────────────────────

function _visiblePresetsLocal(presets) {
    const user = presets.filter(p => !p.id.startsWith('default-'));
    return user.length > 0 ? user : presets;
}

export function renderLaunchGrid() {
    const grid = document.getElementById('setup-launch-grid');
    if (!grid) return;
    grid.innerHTML = '';

    const allPresets = sessionState.presets || [];
    const userPresets = allPresets.filter(p => !p.id.startsWith('default-'));
    const hasUserPresets = userPresets.length > 0;
    let presets = _visiblePresetsLocal(allPresets);
    const activePresetId = sessionState.activeSessionPresetId || '';
    const showNewConfigCard = presets.length <= 2;

    // Apply filters
    presets = _filterPresets(presets);

    // Optional grouped view
    if (launchFilters.groupByFamily && presets.length > 4) {
        _renderGroupedLaunchGrid(grid, presets, activePresetId, hasUserPresets, showNewConfigCard);
    } else {
        _renderFlatLaunchGrid(grid, presets, activePresetId, hasUserPresets, showNewConfigCard);
    }

    // Stamp last-launched timestamps if session data is already available
    _applyLastLaunchedToCards();
    // Memory bar and drop zone: init once (idempotent after first call)
    if (!document.getElementById('setup-drop-zone')?._dzInited) {
        initSetupDropZone();
        const dz = document.getElementById('setup-drop-zone');
        if (dz) dz._dzInited = true;
    }
    fetchAndRenderMemoryBar();
}

function _filterPresets(presets) {
    const f = launchFilters;
    if (!f.family && !f.size && !f.collection && (f.tags || []).length === 0) return presets;

    return presets.filter(p => {
        const c = classifyPreset(p);
        if (f.family && c.family !== f.family) return false;
        if (f.size && c.sizeClass !== f.size) return false;

        if (f.collection) {
            const col = (sessionState.collections || []).find(co => co.id === f.collection);
            if (!col || !col.preset_ids.includes(p.id)) return false;
        }

        if ((f.tags || []).length > 0) {
            const presetTags = (p.tags || []);
            if (!f.tags.some(t => presetTags.includes(t))) return false;
        }

        return true;
    });
}

function _renderFlatLaunchGrid(grid, presets, activePresetId, hasUserPresets, showNewConfigCard) {
    if (!hasUserPresets) {
        const hint = document.createElement('div');
        hint.className = 'launch-grid-hint';
        hint.style.gridColumn = '1 / -1';
        hint.textContent = 'New here? Open the setup wizard to pick a model and start your first local AI in a few steps.';
        grid.appendChild(hint);

        if (showNewConfigCard) {
            const newCard = _buildNewConfigCard(true);
            newCard.style.animationDelay = '80ms';
            grid.appendChild(newCard);
        }
        presets.forEach((preset, i) => {
            const card = _buildLaunchCard(preset, activePresetId);
            card.style.animationDelay = `${(i + (showNewConfigCard ? 2 : 1)) * 55}ms`;
            grid.appendChild(card);
        });
    } else {
        presets.forEach((preset, i) => {
            const card = _buildLaunchCard(preset, activePresetId);
            card.style.animationDelay = `${i * 55}ms`;
            grid.appendChild(card);
        });
        if (showNewConfigCard) {
            const newCard = _buildNewConfigCard(false);
            newCard.style.animationDelay = `${presets.length * 55}ms`;
            grid.appendChild(newCard);
        }
    }
}

function _renderGroupedLaunchGrid(grid, presets, activePresetId, hasUserPresets, showNewConfigCard) {
    const byFamily = {};
    presets.forEach(p => {
        const key = p.family || 'other';
        if (!byFamily[key]) byFamily[key] = [];
        byFamily[key].push(p);
    });

    let i = 0;
    const sections = Object.keys(byFamily);
    for (const fam of sections) {
        const header = document.createElement('div');
        header.className = 'launch-grid-group';
        header.style.gridColumn = '1 / -1';
        const label = fam === 'other' ? 'Other' : (FAMILY_LABEL_MAP[fam] || fam);
        const list = byFamily[fam];
        header.textContent = `${label} (${list.length})`;
        grid.appendChild(header);

        list.forEach(p => {
            const card = _buildLaunchCard(p, activePresetId);
            card.style.animationDelay = `${i * 55}ms`;
            grid.appendChild(card);
            i++;
        });
    }

    if (showNewConfigCard) {
        const newCard = _buildNewConfigCard(false);
        newCard.style.animationDelay = `${i * 55}ms`;
        grid.appendChild(newCard);
    }
}

function _buildLaunchCard(preset, activePresetId) {
    const isExample = preset.id.startsWith('default-');
    const card = document.createElement('div');
    card.className = 'launch-card';
    card.dataset.presetId = preset.id;
    if (isExample) card.classList.add('launch-card--example');

    // Only show running if the server is actually live and this preset is the active one
    const isRunning = !isExample && sessionState.serverRunning && preset.id === activePresetId && activePresetId;
    if (isRunning) card.classList.add('launch-card--running');

    const modelFile = (preset.model_path || '').split(/[/\\]/).pop() ||
                      (preset.hf_repo ? preset.hf_repo.split('/').slice(-1)[0] : '');
    const hasModel = !!modelFile;

    const ctxK = preset.context_size ? Math.round(preset.context_size / 1024) : 128;
    const ctxDisplay = ctxK >= 1000 ? `${(ctxK / 1024).toFixed(1)}M context` : `${ctxK}k context`;
    const ctkDisplay = (preset.ctk || 'q8_0') + '/' + (preset.ctv || 'f16');
    const quantTag = extractQuantFromFilename(modelFile);

    if (isExample) {
        // Example card: dimmed, no edit button, use-wizard CTA only
        // eslint-disable-next-line no-unsanitized/property -- content sanitized via escapeHtml
        card.innerHTML = `
            <div class="launch-card-top">
                <div class="launch-card-name" title="${escapeHtml(preset.name)}">${escapeHtml(preset.name)}</div>
                <span class="launch-card-example-badge">Example</span>
            </div>
            <div class="launch-card-model launch-card-model--empty">Add a model to use these settings</div>
            <div class="launch-card-chips">
                <span class="launch-chip">${ctxDisplay}</span>
                <span class="launch-chip">${ctkDisplay}</span>
            </div>
            <div class="launch-card-actions">
                <button class="launch-card-btn-start launch-card-btn-start--configure" type="button"
                    title="Open the setup wizard with this preset's settings pre-loaded as a starting point">
                    Use as template →
                </button>
            </div>
        `;
        card.querySelector('.launch-card-btn-start').addEventListener('click', () => {
            import('./spawn-wizard.js').then(({ openSpawnWizard }) => openSpawnWizard({ templatePreset: preset }));
        });
    } else {
        const c = classifyPreset(preset);
        const familyLabel = c.family ? FAMILY_LABEL_MAP[c.family] || c.family.toUpperCase() : '';
        const sizeLabel =
            c.sizeClass && c.sizeClass !== 'unknown' && c.sizeClass !== 'huge'
                ? c.sizeClass.charAt(0).toUpperCase() + c.sizeClass.slice(1)
                : '';
        const presetTags = (preset.tags || []);
        const tagPills = presetTags.length > 0
            ? '<div class="launch-card-tags">' +
              presetTags.slice(0, 3).map(t => `<span class="launch-tag">${escapeHtml(t)}</span>`).join('') +
              (presetTags.length > 3 ? `<span class="launch-tag launch-tag--more">+${presetTags.length - 3}</span>` : '') +
              '</div>' : '';

        // eslint-disable-next-line no-unsanitized/property -- content sanitized via escapeHtml
        card.innerHTML = `
            <div class="launch-card-top">
                <div class="launch-card-name" title="${escapeHtml(preset.name)}">${escapeHtml(preset.name)}</div>
                ${isRunning ? '<span class="launch-card-running-badge">● Running</span>' : ''}
            </div>
            <div class="launch-card-meta">
                ${familyLabel ? `<span class="launch-meta-badge launch-meta-badge--family" title="Model family">${escapeHtml(familyLabel)}</span>` : ''}
                ${sizeLabel ? `<span class="launch-meta-badge launch-meta-badge--size" title="Size class">${escapeHtml(sizeLabel)}</span>` : ''}
            </div>
            <div class="launch-card-model ${hasModel ? '' : 'launch-card-model--empty'}" title="${escapeHtml(modelFile || '')}">${escapeHtml(modelFile || 'No model configured')}</div>
            <div class="launch-card-chips">
                <span class="launch-chip">${ctxDisplay}</span>
                <span class="launch-chip">${ctkDisplay}</span>
                ${quantTag ? `<span class="launch-chip launch-chip--quant" title="File quantization: ${escapeHtml(quantTag)}">${escapeHtml(quantTag)}</span>` : ''}
                ${preset.ngram_spec ? '<span class="launch-chip launch-chip--accent">n-gram</span>' : ''}
            </div>
            ${tagPills}
            ${hasModel ? '<div class="launch-card-vram launch-card-vram--loading"><span class="launch-card-vram-total">…</span></div>' : ''}
            <div class="launch-card-actions">
                <button class="launch-card-btn-edit" type="button">Edit</button>
                <button class="launch-card-btn-start ${hasModel ? '' : 'launch-card-btn-start--configure'}" type="button"
                    title="${hasModel ? 'Start the llama-server with this preset' : 'Open the setup wizard to set up a model for this preset'}">
                    ${hasModel ? '▶ Start' : 'Set up model →'}
                </button>
                <button class="launch-card-btn-trash" type="button" title="Delete preset">
                    <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor"
                         stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                        <path d="M3 6h18"/>
                        <path d="M8 6V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2"/>
                        <path d="M19 6l-1 14a2 2 0 0 1-2 2H8a2 2 0 0 1-2-2L5 6"/>
                        <line x1="10" y1="11" x2="10" y2="17"/>
                        <line x1="14" y1="11" x2="14" y2="17"/>
                    </svg>
                </button>
            </div>
        `;

        card.querySelector('.launch-card-btn-edit').addEventListener('click', () => {
            import('./presets.js').then(({ openPresetModal, syncSelectedPresetSelection }) => {
                syncSelectedPresetSelection(preset.id, { userIntent: true, persist: true });
                openPresetModal('edit');
            });
        });

        // Quick-edit: click context or KV chip to open preset modal focused on context section
        card.querySelectorAll('.launch-chip:not(.launch-chip--quant)').forEach(chip => {
            chip.style.cursor = 'pointer';
            chip.title = 'Click to quickly edit context or KV cache settings';
            chip.addEventListener('click', () => {
                import('./presets.js').then(({ openPresetModal, syncSelectedPresetSelection }) => {
                    syncSelectedPresetSelection(preset.id, { userIntent: true, persist: true });
                    openPresetModal('edit', 'context');
                });
            });
        });

        card.querySelector('.launch-card-btn-trash').addEventListener('click', async (e) => {
            e.stopPropagation();
            const ok = await showConfirmDialog(
                'Delete preset',
                `Delete preset "${escapeHtml(preset.name)}"? This cannot be undone.`,
                'Delete'
            );
            if (!ok) return;
            try {
                const headers = window.authHeaders ? window.authHeaders() : {};
                const resp = await fetch(`/api/presets/${preset.id}`, { method: 'DELETE', headers });
                if (resp.ok) {
                    await import('./presets.js').then(({ loadPresets }) => loadPresets());
                    renderLaunchGrid();
                }
            } catch (err) {
                console.error('Delete preset failed:', err);
                showToast('Failed to delete preset', 'error', err.message || String(err));
            }
        });

        card.querySelector('.launch-card-btn-start').addEventListener('click', () => {
            if (!hasModel) {
                import('./spawn-wizard.js').then(({ openSpawnWizard }) => openSpawnWizard());
                return;
            }
            import('./presets.js').then(({ syncSelectedPresetSelection }) => {
                syncSelectedPresetSelection(preset.id, { userIntent: true, persist: true });
                import('./attach-detach.js').then(({ doStartFromSetup }) => doStartFromSetup());
            });
        });
    }

    return card;
}

// ── Card VRAM estimates ───────────────────────────────────────────────────────

async function _fetchCardVramEstimates(availBytes, isUnified) {
    const cards = document.querySelectorAll('.launch-card[data-preset-id]');
    const presets = sessionState.presets || [];

    await Promise.all([...cards].map(async (card) => {
        const preset = presets.find(p => p.id === card.dataset.presetId);
        if (!preset?.model_path) return;
        const vramEl = card.querySelector('.launch-card-vram');
        if (!vramEl) return;

        try {
            const headers = window.authHeaders ? window.authHeaders() : {};
            const resp = await fetch('/api/vram-estimate', {
                method: 'POST',
                headers: { ...headers, 'Content-Type': 'application/json' },
                body: JSON.stringify({
                    model_path: preset.model_path,
                    n_ctx: preset.context_size || 131072,
                    ctk: preset.ctk || 'q8_0',
                    ctv: preset.ctv || 'q8_0',
                    parallel_slots: preset.parallel_slots || 1,
                    ubatch_size: preset.ubatch_size || 1024,
                    n_cpu_moe: preset.n_cpu_moe || 0,
                    available_vram_bytes: availBytes,
                    is_unified_memory: isUnified,
                }),
            });
            if (!resp.ok) return;
            const data = await resp.json();
            if (!data.ok) return;
            // Guard: card may have been re-rendered while awaiting
            if (!document.contains(vramEl)) return;
            _renderCardVram(vramEl, data, availBytes);
        } catch {
            // silently skip — VRAM row stays in loading state
        }
    }));
}

function _renderCardVram(el, data, availBytes) {
    const budget = availBytes > 0 ? availBytes : data.total_bytes * 1.25;
    const totalGb = data.total_bytes / 1e9;
    const weightsGb = data.weights_bytes / 1e9;
    const kvGb = data.kv_cache_bytes / 1e9;
    const extrasBytes = (data.mmproj_bytes || 0) + (data.mtp_bytes || 0) +
                        (data.linear_attn_state_bytes || 0) + (data.overhead_bytes || 0);

    // Under budget: segments + empty track shows headroom.
    // Over budget: bar is full, segments proportional to total (not budget).
    const denominator = data.total_bytes > budget ? data.total_bytes : budget;
    const toWidth = (b) => Math.min(100, (b / denominator) * 100).toFixed(1) + '%';

    const rec = data.recommendation || 'risk';
    const dotClass = rec === 'fit' ? 'fit' : rec === 'tight' ? 'tight' : 'risk';

    const parts = [
        `Weights ${weightsGb.toFixed(1)} GB`,
        `KV ${kvGb.toFixed(1)} GB`,
    ];
    if (data.mmproj_bytes > 0) parts.push(`mmproj ${(data.mmproj_bytes / 1e9).toFixed(1)} GB`);
    if (data.mtp_bytes > 0)    parts.push(`MTP ${(data.mtp_bytes / 1e9).toFixed(1)} GB`);
    parts.push(`overhead ${(data.overhead_bytes / 1e9).toFixed(2)} GB`);

    el.classList.remove('launch-card-vram--loading');
    el.title = parts.join(' · ');
    // eslint-disable-next-line no-unsanitized/property -- all values are numeric; no user strings
    el.innerHTML = `
        <div class="launch-card-vram-bar">
            <div class="launch-card-vram-seg launch-card-vram-seg--weights" style="width:${toWidth(data.weights_bytes)}"></div>
            <div class="launch-card-vram-seg launch-card-vram-seg--kv" style="width:${toWidth(data.kv_cache_bytes)}"></div>
            <div class="launch-card-vram-seg launch-card-vram-seg--extras" style="width:${toWidth(extrasBytes)}"></div>
        </div>
        <span class="launch-card-vram-total" title="Approximate total VRAM: ${parts.join(' · ')}">approx. ${totalGb.toFixed(1)} GB</span>
        <span class="launch-card-vram-dot launch-card-vram-dot--${dotClass}" title="${dotClass === 'fit' ? 'Fits comfortably in VRAM' : dotClass === 'tight' ? 'Tight fit — may work but leaves little headroom' : 'May exceed available VRAM — consider reducing context or KV quant'}"></span>
    `;
}

function _buildNewConfigCard(isPrimary = false) {
    const card = document.createElement('div');
    card.className = 'launch-card launch-card--new' + (isPrimary ? ' launch-card--new-primary' : '');
    // eslint-disable-next-line no-unsanitized/property -- static HTML, no user data
        card.innerHTML = `
        <div class="launch-card-new-icon">
            <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round"><line x1="12" y1="5" x2="12" y2="19"/><line x1="5" y1="12" x2="19" y2="12"/></svg>
        </div>
        <div class="launch-card-new-label">New model</div>
        ${isPrimary ? '<div class="launch-card-new-hint">Set up your first local model</div>' : ''}
    `;
    card.addEventListener('click', () => {
        import('./spawn-wizard.js').then(({ openSpawnWizard }) => openSpawnWizard());
    });
    return card;
}

export function updateRunningCardHighlight() {
    const activePresetId = sessionState.activeSessionPresetId || '';
    document.querySelectorAll('.launch-card[data-preset-id]').forEach(card => {
        const isRunning = sessionState.serverRunning && card.dataset.presetId === activePresetId && activePresetId;
        card.classList.toggle('launch-card--running', !!isRunning);
        const badge = card.querySelector('.launch-card-running-badge');
        if (badge) badge.style.display = isRunning ? '' : 'none';
    });

    // Show the "Dashboard" toolbar button whenever a server is live or a remote endpoint is attached
    const dashBtn = document.getElementById('setup-dashboard-btn');
    if (dashBtn) {
        // Only show when server is confirmed running — activeSessionId defaults to 'default'
        // so it can't be used as a live-session signal.
        dashBtn.style.display = sessionState.serverRunning ? '' : 'none';
    }
}

// ── Recent Sessions ───────────────────────────────────────────────────────────

export async function loadRecentSessions() {
    try {
        const headers = (typeof window.authHeaders === 'function') ? window.authHeaders() : {};
        const resp = await fetch('/api/sessions/recent', { headers });
        if (!resp.ok) {
            // Recent sessions are optional; silently ignore 4xx/5xx to avoid noisy errors.
            return;
        }
        const data = await resp.json();
        renderRecentEndpoints(data.sessions, data.active_session_id);
    } catch {
        // Silent fail — first-run users or unreachable backend won't have this endpoint.
    }
}

export function renderRecentEndpoints(sessions, activeId) {
    const list = document.getElementById('setup-endpoint-list');
    const container = document.getElementById('setup-recent-endpoints');
    const attachBtn = document.getElementById('setup-attach-btn');
    const lastSession = setupViewState.lastSessionData || loadLastSessionData();
    if (!list || !container) return;

    const allSessions = Array.isArray(sessions) ? sessions : [];
    // Only attach sessions appear in the recent list — spawn sessions can't be
    // "resumed" (you'd need to re-spawn), so they are only used to stamp last-launch
    // timestamps on preset cards.
    const attachSessions = allSessions.filter(session => !!session.mode?.Attach);

    if (!allSessions.length) {
        container.style.display = 'none';
        setAttachButtonLabel(attachBtn, 'Attach');
        return;
    }

    container.style.display = attachSessions.length ? '' : 'none';
    list.innerHTML = '';

    // Stamp preset cards with the last time they were spawned (for "last launched" display).
    _spawnLastLaunched.clear();
    for (const session of allSessions.filter(s => !!s.mode?.Spawn)) {
        if (!session.preset_id || !session.last_connected_at) continue;
        const ts = session.last_connected_at * 1000;
        if (ts > (_spawnLastLaunched.get(session.preset_id) || 0)) {
            _spawnLastLaunched.set(session.preset_id, ts);
        }
    }
    _applyLastLaunchedToCards();

    // buildCard is only called for Attach sessions — Spawn sessions cannot be
    // "resumed" from here (you'd need to re-spawn via a preset card).
    const buildCard = (session) => {
        const card = document.createElement('div');
        card.className = 'setup-endpoint-card';
        if (activeId && session.id === activeId) {
            card.classList.add('is-active-session');
        }

        const endpoint = session.mode?.Attach?.endpoint || '';
        const apiKey = session.mode?.Attach?.api_key || '';

        const statusClass = session.status === 'Running' ? 'status-running' :
                            session.status === 'Error' ? 'status-error' : 'status-disconnected';

        const lastConnected = session.last_connected_at ? formatRelativeTime(session.last_connected_at * 1000) : 'Never';
        const connectCount = session.connect_count || 0;

        const statusDot = document.createElement('div');
        statusDot.className = 'setup-endpoint-status ' + statusClass;

        const infoWrap = document.createElement('div');
        infoWrap.className = 'setup-endpoint-info';

        const nameEl = document.createElement('div');
        nameEl.className = 'setup-endpoint-name';
        nameEl.textContent = session.name || endpoint || 'Unnamed';

        const endpointEl = document.createElement('div');
        endpointEl.className = 'setup-endpoint-url';
        endpointEl.textContent = endpoint;
        endpointEl.title = endpoint;

        const metaEl = document.createElement('div');
        metaEl.className = 'setup-endpoint-meta';
        const metaParts = [];
        if (activeId && session.id === activeId) metaParts.push('Active workspace');
        else if (session.status === 'Running') metaParts.push('Last seen running');
        else if (session.status === 'Disconnected') metaParts.push('Ready to reconnect');
        else if (session.status === 'Error') metaParts.push(session.last_error || 'Needs attention');
        if (lastSession?.endpoint && endpoint && lastSession.endpoint === endpoint && lastSession.telemetryLabel) {
            metaParts.push(lastSession.telemetryLabel);
        }
        if (lastConnected !== 'Never') metaParts.push(lastConnected);
        if (connectCount > 0) metaParts.push(connectCount + 'x');
        let meta = metaParts.join(' · ');
        if (!meta) meta = 'Saved endpoint';
        metaEl.textContent = meta;

        infoWrap.appendChild(nameEl);
        infoWrap.appendChild(endpointEl);
        infoWrap.appendChild(metaEl);

        const connectBtn = document.createElement('button');
        connectBtn.className = 'setup-endpoint-connect';

        const doConnect = () => {
            const urlInput = document.getElementById('setup-endpoint-url');
            if (urlInput) urlInput.value = endpoint;
            const apiKeyInput = document.getElementById('setup-endpoint-api-key');
            if (apiKeyInput) apiKeyInput.value = apiKey;
            doAttachFromSetup();
        };

        connectBtn.textContent = activeId && session.id === activeId
            ? 'Resume'
            : (session.last_connected_at ? 'Reconnect' : 'Connect');

        card.appendChild(statusDot);
        card.appendChild(infoWrap);
        card.appendChild(connectBtn);

        connectBtn.addEventListener('click', (e) => { e.stopPropagation(); doConnect(); });
        card.addEventListener('click', doConnect);

        return card;
    };

    attachSessions.forEach(session => list.appendChild(buildCard(session)));

    // Live health-check attach sessions that aren't already confirmed Running
    attachSessions.forEach((session, i) => {
        if (session.status === 'Running') return;
        const endpoint = session.mode?.Attach?.endpoint;
        if (!endpoint) return;
        const card = list.children[i];
        if (!card) return;
        const dot = card.querySelector('.setup-endpoint-status');
        if (!dot) return;
        const authHdrs = window.authHeaders ? window.authHeaders() : {};
        const checkUrl = '/api/sessions/check-endpoint?url=' + encodeURIComponent(endpoint);
        fetch(checkUrl, { headers: authHdrs, signal: AbortSignal.timeout(6000) })
            .then(r => r.ok ? r.json() : null)
            .then(data => { if (data?.reachable) dot.className = 'setup-endpoint-status status-running'; })
            .catch(() => {});
    });
}

function formatRelativeTime(ts) {
    const diff = Date.now() - ts;
    const seconds = Math.floor(diff / 1000);
    if (seconds < 60) return 'Just now';
    const minutes = Math.floor(seconds / 60);
    if (minutes < 60) return minutes + 'm ago';
    const hours = Math.floor(minutes / 60);
    if (hours < 24) return hours + 'h ago';
    const days = Math.floor(hours / 24);
    return days + 'd ago';
}

// ── Session Data ──────────────────────────────────────────────────────────────

export function saveLastSessionData(data) {
    const payload = { ...data, timestamp: Date.now() };
    localStorage.setItem('llama-monitor-last-session', JSON.stringify(payload));
    setupViewState.lastSessionData = payload;
}

export function loadLastSessionData() {
    try {
        const raw = localStorage.getItem('llama-monitor-last-session');
        if (!raw) return null;
        const data = JSON.parse(raw);
        if (Date.now() - data.timestamp > 24 * 60 * 60 * 1000) {
            localStorage.removeItem('llama-monitor-last-session');
            return null;
        }
        return data;
    } catch {
        return null;
    }
}

// ── Previous Position ─────────────────────────────────────────────────────────

export function savePreviousPosition() {
    const activePage = document.querySelector('.page.active');
    const navTab = activePage?.id?.replace('page-', '') || 'server';
    const chatTabId = navTab === 'chat' ? chat.activeTabId : null;
    const scrollPosition = activePage?.scrollTop || 0;

    const position = {
        view: setupViewState.view,
        navTab,
        chatTabId,
        scrollPosition,
        timestamp: Date.now(),
    };

    localStorage.setItem('llama-monitor-previous-position', JSON.stringify(position));
    setupViewState.previousPosition = position;
}

export function loadPreviousPosition() {
    try {
        const raw = localStorage.getItem('llama-monitor-previous-position');
        if (!raw) return null;
        const data = JSON.parse(raw);
        if (Date.now() - data.timestamp > 24 * 60 * 60 * 1000) {
            localStorage.removeItem('llama-monitor-previous-position');
            return null;
        }
        return data;
    } catch {
        return null;
    }
}

export function clearPreviousPosition() {
    localStorage.removeItem('llama-monitor-previous-position');
    setupViewState.previousPosition = null;
}

export async function restorePreviousPosition() {
    const position = loadPreviousPosition();
    if (!position) return;

    // Switch to saved nav tab
    if (position.navTab && position.navTab !== 'server') {
        const { switchTab } = await import('./nav.js');
        switchTab(position.navTab);
    }

    // Switch to saved chat tab
    if (position.chatTabId && position.navTab === 'chat') {
        const { switchChatTab } = await import('./chat-state.js');
        switchChatTab(position.chatTabId);
    }

    // Restore scroll position
    const activePage = document.querySelector('.page.active');
    if (activePage && position.scrollPosition > 0) {
        activePage.scrollTop = position.scrollPosition;
    }

    clearPreviousPosition();
}

// ── Quick Stats ───────────────────────────────────────────────────────────────

export function renderQuickStats() {
    const data = loadLastSessionData();
    const statsEl = document.getElementById('setup-stats');
    if (!statsEl) return;

    if (data) {
        const promptRate = document.getElementById('setup-last-prompt-rate');
        const genRate = document.getElementById('setup-last-gen-rate');
        const session = document.getElementById('setup-last-session');
        if (promptRate) promptRate.textContent = data.promptRate || '—';
        if (genRate) genRate.textContent = data.genRate || '—';
        if (session) session.textContent = data.sessionName || '—';
        statsEl.style.display = 'flex';
    } else {
        statsEl.style.display = 'none';
    }
}

export function syncSetupPresetSelect() {
    const setupSelect = document.getElementById('setup-preset-select');
    const mainSelect = document.getElementById('preset-select');
    if (!setupSelect || !mainSelect) return;

    // Mirror the main select (already filtered to visible presets by presets.js)
    setupSelect.innerHTML = '';
    const options = mainSelect.querySelectorAll('option');
    options.forEach(opt => {
        const clone = document.createElement('option');
        clone.value = opt.value;
        clone.textContent = opt.textContent;
        setupSelect.appendChild(clone);
    });
    setupSelect.value = mainSelect.value;

    // Also re-render the launch grid when presets change
    renderLaunchGrid();
}

// ── Launch Filters ────────────────────────────────────────────────────────────

export async function initLaunchFilters() {
    const bar = document.getElementById('setup-filter-bar');
    if (!bar) return;

    // Only populate if we have multiple presets (avoid clutter with 0-2)
    const presets = sessionState.presets || [];
    const userPresets = presets.filter(p => !p.id.startsWith('default-'));
    if (userPresets.length < 3) {
        bar.style.display = 'none';
        return;
    }
    bar.style.display = '';

    // Populate family pills based on available families (from GGUF metadata)
    const families = new Set();
    userPresets.forEach(p => {
        const f = p.family;
        if (f) families.add(f);
    });

    const familyContainer = document.getElementById('setup-filter-family-pills');
    if (familyContainer) {
        // Keep "All" button
        const allBtn = familyContainer.querySelector('[data-filter="family-all"]');
        if (allBtn) allBtn.addEventListener('click', () => {
            launchFilters.family = null;
            updateFilterPillActive(familyContainer, 'family-all');
            renderLaunchGrid();
        });
        for (const fam of families) {
            const btn = document.createElement('button');
            btn.className = 'launch-filter-pill';
            btn.dataset.filter = fam;
            btn.type = 'button';
            btn.textContent = FAMILY_LABEL_MAP[fam] || fam;
            btn.addEventListener('click', () => {
                launchFilters.family = fam;
                updateFilterPillActive(familyContainer, fam);
                renderLaunchGrid();
            });
            familyContainer.appendChild(btn);
        }
    }

    // Size pills
    const sizeContainer = document.getElementById('setup-filter-size-pills');
    if (sizeContainer) {
        const sizes = ['tiny', 'small', 'medium', 'large'];
        const allBtn = sizeContainer.querySelector('[data-filter="size-all"]');
        if (allBtn) allBtn.addEventListener('click', () => {
            launchFilters.size = null;
            updateFilterPillActive(sizeContainer, 'size-all');
            renderLaunchGrid();
        });
        for (const s of sizes) {
            const btn = document.createElement('button');
            btn.className = 'launch-filter-pill';
            btn.dataset.filter = s;
            btn.type = 'button';
            btn.textContent = s.charAt(0).toUpperCase() + s.slice(1);
            btn.addEventListener('click', () => {
                launchFilters.size = s;
                updateFilterPillActive(sizeContainer, s);
                renderLaunchGrid();
            });
            sizeContainer.appendChild(btn);
        }
    }

    // Tags button: simple pill picker popup
    const tagsBtn = document.getElementById('setup-filter-tags-btn');
    if (tagsBtn) {
        const knownTags = ['coding', 'agentic', 'roleplay', 'creative', 'vision', 'fast'];
        tagsBtn.addEventListener('click', () => {
            if (document.querySelector('.launch-tags-popup')) return;
            const popup = document.createElement('div');
            popup.className = 'launch-tags-popup';
            // Build via DOM to satisfy no-unsanitized rule
            knownTags.forEach(t => {
                const label = document.createElement('label');
                label.className = 'launch-tag-option';
                const input = document.createElement('input');
                input.type = 'checkbox';
                input.value = t;
                label.appendChild(input);
                label.appendChild(document.createTextNode(t));
                popup.appendChild(label);
            });
            const apply = () => {
                launchFilters.tags = [...popup.querySelectorAll('input:checked')].map(i => i.value);
                if (launchFilters.tags.length > 0) {
                    tagsBtn.classList.add('launch-filter-pill--active');
                    tagsBtn.textContent = `+ ${launchFilters.tags.length}`;
                } else {
                    tagsBtn.classList.remove('launch-filter-pill--active');
                    tagsBtn.textContent = '+ Tags';
                }
                popup.remove();
                renderLaunchGrid();
            };
            popup.addEventListener('click', e => {
                if (e.target.tagName === 'INPUT') {
                    setTimeout(apply, 50);
                }
            });
            tagsBtn.parentElement.appendChild(popup);
        });
    }

    // Collections dropdown
    const collectionsGroup = document.getElementById('setup-filter-collections-group');
    const collectionsSelect = document.getElementById('setup-filter-collections');
    if (collectionsSelect && collectionsGroup) {
        const collections = sessionState.collections || [];
        if (collections.length > 0) {
            collectionsGroup.style.display = '';
            collectionsSelect.innerHTML = '<option value="all">All</option>';
            collections.forEach(c => {
                const opt = document.createElement('option');
                opt.value = c.id;
                opt.textContent = c.name;
                collectionsSelect.appendChild(opt);
            });
            collectionsSelect.addEventListener('change', () => {
                launchFilters.collection = collectionsSelect.value === 'all' ? null : collectionsSelect.value;
                renderLaunchGrid();
            });
        }
    }

    // Group by family toggle
    const groupToggle = document.getElementById('setup-filter-group-by-family');
    if (groupToggle) {
        const saved = localStorage.getItem('llama-monitor-group-by-family');
        if (saved === '1') {
            launchFilters.groupByFamily = true;
            groupToggle.checked = true;
        }
        groupToggle.addEventListener('change', () => {
            launchFilters.groupByFamily = !!groupToggle.checked;
            localStorage.setItem('llama-monitor-group-by-family', launchFilters.groupByFamily ? '1' : '0');
            renderLaunchGrid();
        });
    }
}

function updateFilterPillActive(container, activeId) {
    if (!container) return;
    container.querySelectorAll('.launch-filter-pill').forEach(btn => {
        btn.classList.toggle('launch-filter-pill--active', btn.dataset.filter === activeId);
    });
}

// ── Initialization ────────────────────────────────────────────────────────────

export function initViewState() {
    if (document.body.classList.contains('setup-active')) return; // already initialized
    renderQuickStats();
    syncSetupPresetSelect(); // also calls renderLaunchGrid
    const lastEndpoint = localStorage.getItem('llama-monitor-last-endpoint');
    if (lastEndpoint) {
        const input = document.getElementById('setup-endpoint-url');
        if (input) input.value = lastEndpoint;
    }

    // Bind models button
    document.getElementById('setup-models-btn')?.addEventListener('click', () => {
        import('./models.js').then(({ openModelsModal }) => openModelsModal());
    });

    // Dashboard button: visible when a server is running or remote endpoint attached
    const dashBtn = document.getElementById('setup-dashboard-btn');
    if (dashBtn) {
        dashBtn.addEventListener('click', () => switchView('monitor'));
    }

    // Init filter bar after presets are loaded
    initLaunchFilters();

    document.body.classList.add('setup-active');
    const setupView = document.getElementById('view-setup');
    const monitorView = document.getElementById('view-monitor');
    if (setupView) {
        setupView.style.display = '';
        setupView.classList.add('entering');
        setTimeout(() => setupView.classList.remove('entering'), 600);
    }
    if (monitorView) monitorView.style.display = 'none';
    loadRecentSessions();

// ── New Configuration button (header) ──────────────────────────────────

    const btn = document.getElementById('setup-new-config-btn');
    if (btn) {
        btn.addEventListener('click', async () => {
            try {
                const { openSpawnWizard } = await import('./spawn-wizard.js');
                openSpawnWizard();
            } catch (e) {
                console.error('Failed to open spawn wizard from new-config button:', e);
            }
        });
    }
}

// ── Public API ────────────────────────────────────────────────────────────────

export function initSetupView() {
    // Initialize view state immediately — defensive functions return early if DOM not ready
    initViewState();
}
