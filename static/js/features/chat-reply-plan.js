// ── Reply Plan Summary ────────────────────────────────────────────────────────
// Shows active steering inputs as chips above the chat composer.

import { activeChatTab } from './chat-state.js';

export function updateReplyPlanSummary() {
    const container = document.getElementById('reply-plan-summary');
    if (!container) return;

    const tab = activeChatTab();
    if (!tab) {
        container.innerHTML = '';
        container.classList.remove('visible');
        return;
    }

    const chips = [];

    // Persona
    if (tab.active_template_id) {
        const personaName = resolvePersonaName(tab.active_template_id);
        chips.push({ class: 'chip-persona', text: personaName || 'Persona' });
    }

    // Explicit mode
    if (tab.explicit_level && tab.explicit_level > 0) {
        chips.push({ class: 'chip-explicit', text: 'Explicit: ' + tab.explicit_level });
    }

    // Context notes
    const notes = tab.context_notes || [];
    if (notes.length > 0) {
        chips.push({ class: 'chip-notes', text: notes.length + ' note' + (notes.length !== 1 ? 's' : '') });
    }

    // Quick guide
    if (tab.quick_guide_active || tab.quick_guide_draft) {
        chips.push({ class: 'chip-guide', text: tab.quick_guide_active ? 'Guide active' : 'Guide draft' });
    }

    const composerInput = document.getElementById('chat-input');
    if (composerInput?.dataset.suggestionDraft === 'true') {
        chips.push({ class: 'chip-guide', text: 'Draft override armed' });
    }

    // Armed story beats / surprise
    const beats = tab.armed_story_beats || [];
    if (beats.length > 0) {
        chips.push({ class: 'chip-surprise', text: beats.length + ' beat' + (beats.length !== 1 ? 's' : '') + ' armed' });
    }

    // Compaction
    if (tab.auto_compact) {
        chips.push({ class: 'chip-compact', text: 'Auto-compact ' + (tab.compact_threshold || 75) + '%' });
    }

    const compactedMemory = (tab.messages || []).filter(msg => msg.compaction_marker && msg.content?.trim());
    if (compactedMemory.length > 0) {
        chips.push({ class: 'chip-compact', text: compactedMemory.length + ' memory' + (compactedMemory.length !== 1 ? ' blocks' : ' block') });
    }

    if (chips.length === 0) {
        container.innerHTML = '';
        container.classList.remove('visible');
        return;
    }

    container.classList.add('visible');
    container.innerHTML = '';

    chips.forEach(chip => {
        const el = document.createElement('span');
        el.className = 'reply-plan-chip ' + chip.class;
        el.textContent = chip.text;
        container.appendChild(el);
    });
}

function resolvePersonaName(templateId) {
    try {
        const templates = window.chatTemplates || [];
        const found = templates.find(t => t.id === templateId);
        return found ? found.name : null;
    } catch {
        return null;
    }
}

// Auto-update on tab switch and relevant events
export function initReplyPlanUpdates() {
    updateReplyPlanSummary();

    const observer = new MutationObserver(() => {
        updateReplyPlanSummary();
    });

    const chatArea = document.getElementById('chat-messages');
    if (chatArea) {
        observer.observe(chatArea, { childList: true, subtree: false });
    }

    // Also update on visibility change (in case user switches tabs while minimized)
    document.addEventListener('visibilitychange', () => {
        if (!document.hidden) {
            updateReplyPlanSummary();
        }
    });

    window.addEventListener('activeTabChanged', updateReplyPlanSummary);
    window.addEventListener('explicitModeChanged', updateReplyPlanSummary);
    window.addEventListener('chatReplyComplete', updateReplyPlanSummary);
    window.addEventListener('replyPlanChanged', updateReplyPlanSummary);
    document.addEventListener('input', (event) => {
        const target = event.target;
        if (!(target instanceof HTMLElement)) return;
        if ([
            'chat-input',
            'quick-guide-input',
            'quick-guide-director-input',
            'quick-guide-surprise-input',
        ].includes(target.id)) {
            updateReplyPlanSummary();
        }
    });
    document.addEventListener('change', (event) => {
        const target = event.target;
        if (!(target instanceof HTMLElement)) return;
        if ([
            'chat-auto-compact',
            'chat-auto-compact-summarize',
            'chat-compact-threshold',
        ].includes(target.id)) {
            updateReplyPlanSummary();
        }
    });
}
