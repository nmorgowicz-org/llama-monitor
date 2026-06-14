// ── Setup / Monitor View ──────────────────────────────────────────────────────
// View transitions, animations, quick stats, and view state initialization.

import { setupViewState, chat, sessionState } from '../core/app-state.js';
import { doAttachFromSetup } from './attach-detach.js';
import { escapeHtml } from '../core/format.js';
import { quickStartSession } from './sessions.js';
import { showToast } from './toast.js';
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
        let metalGpuLimitMb = 0;
        let isUnified = false;
        let vramUsedBytes = 0;

        if (sysResp.ok) {
            const d = await sysResp.json();
            totalBytes = (d.ram_total_gb || 0) * 1024 ** 3;
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

        if (isUnified) {
            const cap = _metalCap(totalBytes, metalGpuLimitMb);
            availBytes = Math.max(0, Math.min(cap, totalBytes) - _MEM_OS_RESERVE);
            fillPct = Math.round((availBytes / totalBytes) * 100);
            availLabel = _fmtGb(availBytes) + ' GB available for inference';
            totalLabel = _fmtGb(totalBytes) + ' GB unified';
        } else {
            availBytes = Math.max(0, totalBytes - vramUsedBytes);
            fillPct = Math.round((availBytes / totalBytes) * 100);
            availLabel = _fmtGb(availBytes) + ' GB VRAM free';
            totalLabel = _fmtGb(totalBytes) + ' GB total';
        }

        const fill = document.getElementById('setup-mem-bar-fill');
        if (fill) fill.style.width = fillPct + '%';
        const availEl = document.getElementById('setup-mem-bar-avail');
        if (availEl) availEl.textContent = availLabel;
        const totalEl = document.getElementById('setup-mem-bar-total');
        if (totalEl) totalEl.textContent = totalLabel;
        bar.style.display = '';

        _memState = { availBytes, isUnified };
        _fetchCardVramEstimates(availBytes, isUnified);
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
    const presets = _visiblePresetsLocal(allPresets);
    const activePresetId = document.getElementById('preset-select')?.value || '';

    const showNewConfigCard = presets.length <= 2;

    if (!hasUserPresets) {
        // Onboarding hint (first-time / no presets)
        const hint = document.createElement('div');
        hint.className = 'launch-grid-hint';
        hint.style.gridColumn = '1 / -1';
        hint.textContent = 'New here? Open the setup wizard to pick a model and start your first local AI in a few steps.';
        grid.appendChild(hint);

        // No user presets: New Config goes first so it's the obvious CTA
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
        // User has presets: show them first (leftmost), New Config goes last only when presets are few
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
        // eslint-disable-next-line no-unsanitized/property -- content sanitized via escapeHtml
        card.innerHTML = `
            <div class="launch-card-top">
                <div class="launch-card-name" title="${escapeHtml(preset.name)}">${escapeHtml(preset.name)}</div>
                ${isRunning ? '<span class="launch-card-running-badge">● Running</span>' : ''}
            </div>
            <div class="launch-card-model ${hasModel ? '' : 'launch-card-model--empty'}" title="${escapeHtml(modelFile || '')}">${escapeHtml(modelFile || 'No model configured')}</div>
            <div class="launch-card-chips">
                <span class="launch-chip">${ctxDisplay}</span>
                <span class="launch-chip">${ctkDisplay}</span>
                ${preset.ngram_spec ? '<span class="launch-chip launch-chip--accent">n-gram</span>' : ''}
            </div>
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
            const mainSel = document.getElementById('preset-select');
            if (mainSel) mainSel.value = preset.id;
            import('./presets.js').then(({ openPresetModal }) => openPresetModal('edit'));
        });

        // Quick-edit: click context or KV chip to open preset modal focused on context section
        card.querySelectorAll('.launch-chip').forEach(chip => {
            chip.style.cursor = 'pointer';
            chip.title = 'Click to quickly edit context or KV cache settings';
            chip.addEventListener('click', () => {
                const mainSel = document.getElementById('preset-select');
                if (mainSel) mainSel.value = preset.id;
                import('./presets.js').then(({ openPresetModal }) => {
                    openPresetModal('edit', 'context');
                });
            });
        });

        card.querySelector('.launch-card-btn-trash').addEventListener('click', async (e) => {
            e.stopPropagation();
            if (!confirm(`Delete preset "${escapeHtml(preset.name)}"? This cannot be undone.`)) return;
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
            const setupSel = document.getElementById('setup-preset-select');
            if (setupSel) setupSel.value = preset.id;
            const mainSel = document.getElementById('preset-select');
            if (mainSel) mainSel.value = preset.id;
            import('./attach-detach.js').then(({ doStartFromSetup }) => doStartFromSetup());
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
    const activePresetId = document.getElementById('preset-select')?.value || '';
    document.querySelectorAll('.launch-card[data-preset-id]').forEach(card => {
        const isRunning = sessionState.serverRunning && card.dataset.presetId === activePresetId && activePresetId;
        card.classList.toggle('launch-card--running', !!isRunning);
        const badge = card.querySelector('.launch-card-running-badge');
        if (badge) badge.style.display = isRunning ? '' : 'none';
    });
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
