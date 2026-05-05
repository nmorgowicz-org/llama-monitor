// ── Config ────────────────────────────────────────────────────────────────────
// Config modal, GPU environment, and config save.

import { showToast } from './toast.js';
import { openDeferredFileBrowser } from './file-browser-launcher.js';
import { collectSettings, closeSettingsModal } from './settings.js';
import { settingsState } from '../core/app-state.js';

// ── Config Modal ──────────────────────────────────────────────────────────────

export function openConfigModal() {
    closeSettingsModal();
    document.getElementById('config-modal').classList.add('open');
}

export function closeConfigModal() {
    document.getElementById('config-modal').classList.remove('open');
}

// ── GPU Environment ───────────────────────────────────────────────────────────

async function loadGpuEnv() {
    try {
        const resp = await fetch('/api/gpu-env');
        const data = await resp.json();
        const env = data.env;
        const archs = data.architectures;
        const detected = data.detected;

        const sel = document.getElementById('gpu-env-arch');
        sel.innerHTML = '';
        archs.forEach(a => {
            const opt = document.createElement('option');
            opt.value = a.id;
            let label = a.name;
            if (detected && detected.arch === a.id) label += ' (detected)';
            opt.textContent = label;
            sel.appendChild(opt);
        });
        sel.value = env.arch;

        document.getElementById('gpu-env-devices').value = env.devices;
        document.getElementById('gpu-env-rocm-path').value = env.rocm_path || '/opt/rocm';

        const infoEl = document.getElementById('gpu-detected-info');
        const summaryInfo = document.getElementById('gpu-env-info');
        if (detected) {
            const source = detected.arch === 'apple' ? 'local macOS system profile' : detected.arch === 'nvidia' ? 'local nvidia-smi' : 'local rocminfo';
            infoEl.textContent = 'Local detection: ' + detected.count + 'x ' + detected.arch + ' (' + detected.names.join(', ') + ') via ' + source;
            summaryInfo.textContent = '\u2014 ' + detected.count + 'x ' + detected.arch;
        } else {
            infoEl.textContent = 'No local GPU detected via Apple Silicon, rocminfo, or nvidia-smi. Remote hosts need a remote agent.';
            summaryInfo.textContent = '';
        }
    } catch (err) {
        console.error('Failed to load GPU env:', err);
    }
}

// ── Save Config ───────────────────────────────────────────────────────────────

function saveConfig() {
    clearTimeout(settingsState.saveTimer);
    fetch('/api/settings', {
        method: 'PUT',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(collectSettings()),
    }).catch(() => {});

    const env = {
        arch: document.getElementById('gpu-env-arch').value,
        devices: document.getElementById('gpu-env-devices').value.trim(),
        rocm_path: document.getElementById('gpu-env-rocm-path').value.trim() || '/opt/rocm',
        extra_env: [],
    };
    fetch('/api/gpu-env', {
        method: 'PUT',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(env),
    }).catch(() => {});

    closeConfigModal();
    showToast('Configuration saved', 'success');
}

function usePathServerBinary() {
    const input = document.getElementById('set-server-path');
    if (input) input.value = '';
    showToast('llama-server will be resolved from PATH', 'info');
}

// ── Public API ────────────────────────────────────────────────────────────────

export function initConfig() {
    const configModal = document.getElementById('config-modal');
    if (configModal) {
        configModal.addEventListener('click', e => {
            if (e.target === e.currentTarget) closeConfigModal();
        });
    }

    // Bind config modal buttons
    const configClose = document.getElementById('config-modal-close');
    if (configClose) configClose.addEventListener('click', closeConfigModal);

    const configCancel = document.getElementById('config-modal-cancel');
    if (configCancel) configCancel.addEventListener('click', closeConfigModal);

    const configSave = document.getElementById('config-modal-save');
    if (configSave) configSave.addEventListener('click', saveConfig);

    // Bind Browse buttons in config modal
    const browseServerPath = document.getElementById('config-browse-server-path');
    if (browseServerPath) browseServerPath.addEventListener('click', () => openDeferredFileBrowser('set-server-path', 'executable'));

    const usePathBtn = document.getElementById('config-use-path-btn');
    if (usePathBtn) usePathBtn.addEventListener('click', usePathServerBinary);

    const browseCwd = document.getElementById('config-browse-cwd');
    if (browseCwd) browseCwd.addEventListener('click', () => openDeferredFileBrowser('set-server-cwd', 'dir'));

    // Bind "Open Runtime Configuration" in settings modal
    const openConfigBtn = document.getElementById('settings-open-config-btn');
    if (openConfigBtn) openConfigBtn.addEventListener('click', openConfigModal);

}
