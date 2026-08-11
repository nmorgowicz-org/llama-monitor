import { showToast } from './toast.js';

const CHOICES = {
    keep_legacy: {
        label: 'KEEP_LEGACY_MODEL_ROOT',
        confirmation: 'KEEP_LEGACY_MODEL_ROOT',
    },
    move_into_foundry: {
        label: 'MOVE_MODELS_INTO_FOUNDRY',
        confirmation: 'MOVE_MODELS_INTO_FOUNDRY',
    },
};

function headers() {
    return window.authHeaders ? window.authHeaders({ 'Content-Type': 'application/json' }) : {
        'Content-Type': 'application/json',
    };
}

function setText(id, value) {
    const element = document.getElementById(id);
    if (element) element.textContent = value;
}

function selectedChoice() {
    return document.querySelector('input[name="model-root-choice"]:checked')?.value || 'keep_legacy';
}

export async function initModelRootMigration() {
    const card = document.getElementById('model-root-relocation-card');
    if (!card) return;
    const previewButton = document.getElementById('model-root-relocation-preview');
    const executeButton = document.getElementById('model-root-relocation-execute');
    const planElement = document.getElementById('model-root-relocation-plan');
    let plan = null;
    let status = null;

    try {
        const response = await window.authFetch('/api/models/root-relocation/status', {
            headers: window.authHeaders(),
        });
        if (!response.ok) return;
        status = await response.json();
        setText('model-root-relocation-state', status.relocation_required
            ? 'Legacy model resources are available for an explicit choice.'
            : (status.selection ? 'A model-root choice is already recorded.' : 'No legacy model-root relocation is required.'));
        setText('model-root-relocation-summary', status.source
            ? `Current source: ${status.source} · Foundry destination: ${status.destination}`
            : 'Model-root status is unavailable.');
        if (status.custom_root) {
            setText('model-root-relocation-state', 'A custom model root is active; it will not be moved implicitly.');
            previewButton.disabled = true;
        }
        if (!status.relocation_required) {
            previewButton.disabled = true;
        }
    } catch {
        setText('model-root-relocation-state', 'Could not read model-root status.');
        previewButton.disabled = true;
    }

    document.querySelectorAll('input[name="model-root-choice"]').forEach((input) => {
        input.addEventListener('change', () => {
            plan = null;
            executeButton.disabled = true;
            if (planElement) planElement.hidden = true;
        });
    });

    previewButton?.addEventListener('click', async () => {
        const choice = selectedChoice();
        previewButton.disabled = true;
        try {
            const response = await window.authFetch('/api/models/root-relocation/preview', {
                method: 'POST',
                headers: headers(),
                body: JSON.stringify({ choice }),
            });
            const payload = await response.json().catch(() => ({}));
            if (!response.ok || !payload.plan_id) throw new Error(payload.error || 'Preview unavailable');
            plan = payload;
            if (planElement) {
                planElement.hidden = false;
                planElement.textContent = JSON.stringify({
                    plan_id: payload.plan_id,
                    choice: payload.choice,
                    entries: payload.entries?.length || 0,
                    required_copy_bytes: payload.required_copy_bytes,
                    available_destination_bytes: payload.available_destination_bytes,
                    retained_external_roots: payload.retained_external_roots,
                }, null, 2);
            }
            executeButton.disabled = false;
        } catch (error) {
            showToast('Model-root preview failed', 'error', error.message || 'Try again later.');
        } finally {
            previewButton.disabled = false;
        }
    });

    executeButton?.addEventListener('click', async () => {
        if (!plan) return;
        const choice = selectedChoice();
        const choiceInfo = CHOICES[choice];
        if (!choiceInfo || !window.confirm(`Confirm ${choiceInfo.label}? The legacy source remains available for rollback.`)) return;
        executeButton.disabled = true;
        try {
            const tokenResponse = await window.authFetch('/api/db/admin-token', {
                headers: window.authHeaders(),
            });
            const tokenPayload = await tokenResponse.json().catch(() => ({}));
            if (!tokenResponse.ok || !tokenPayload.token) throw new Error('Administrator authorization is unavailable.');
            const response = await fetch('/api/models/root-relocation/execute', {
                method: 'POST',
                headers: { Authorization: `Bearer ${tokenPayload.token}`, 'Content-Type': 'application/json' },
                body: JSON.stringify({
                    plan_id: plan.plan_id,
                    choice,
                    confirmation: choiceInfo.confirmation,
                }),
            });
            const payload = await response.json().catch(() => ({}));
            if (!response.ok || !payload.ok) throw new Error(payload.error || 'Could not save model-root choice.');
            setText('model-root-relocation-state', 'Choice saved. Restart Foundry to activate the selected model root.');
            showToast('Model-root choice saved', 'success', 'The legacy source remains available until you explicitly clean it up.');
            executeButton.disabled = true;
            plan = null;
        } catch (error) {
            showToast('Model-root relocation failed', 'error', error.message || 'Try again later.');
            executeButton.disabled = false;
        }
    });
}
