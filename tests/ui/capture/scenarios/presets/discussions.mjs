// Scenario: discussions
// Extracted from tests/ui/capture.mjs (Phase A3).
import { gotoApp } from '../../harness/browser.mjs';
import { sleep } from '../../harness/paths.mjs';
import { captureShot } from '../../harness/shot.mjs';

export default async function(ctx, options) {
    const { page, baseUrl } = ctx;
    await gotoApp(page, baseUrl);

    // Open spawn wizard.
    await page.evaluate(async () => {
        const { openSpawnWizard } = await import('/js/features/spawn-wizard.js');
        openSpawnWizard();
    });
    await page.waitForSelector('#spawn-wizard-overlay.open', { timeout: 10000 });
    await sleep(600);

    // Hide binary prereq banner.
    await page.evaluate(() => {
        const banner = document.getElementById('wizard-binary-prereq');
        if (banner) banner.style.display = 'none';
    });
    await sleep(200);

    // Step 0: select profile and usecase.
    await page.evaluate(() => {
        (document.querySelector('.profile-card[data-profile="power"]')
            || document.querySelector('.profile-card'))?.click();
    });
    await sleep(200);
    await page.evaluate(() => {
        (document.querySelector('.usecase-card[data-usecase="general"]')
            || document.querySelector('.usecase-card'))?.click();
    });
    await sleep(300);

    // Advance to Step 1: Model (where chat-template-section lives).
    await page.evaluate(() => document.getElementById('wizard-next-btn')?.click());
    await page.waitForFunction(
        () => document.getElementById('wizard-step-1')?.classList.contains('active'),
        { timeout: 5000 }
    ).catch(() => {});
    await sleep(600);

    // ── Qwen workflow: force family → froggeric/Qwen-Fixed-Chat-Templates ─────

    // Directly set wizardState.model.family to 'qwen' and trigger template install.
    // This is more reliable than interacting with the force-family dropdown which
    // is only rendered after an initial model selection.
    await page.evaluate(async () => {
        const { wizardState } = await import('/js/features/spawn-wizard.js');
        const { autoInstallChatTemplate } = await import('/js/features/spawn-wizard-chat-template.js');
        wizardState.model.family = 'qwen';
        wizardState.model.chatTemplateMode = 'auto';
        wizardState.model.chatTemplateCandidate = null;
        await autoInstallChatTemplate();
    });
    await sleep(1500);

    // Wait for installation to complete (status becomes "✓ Installed").
    await page.waitForFunction(() => {
        const status = document.getElementById('ct-status');
        return status && status.textContent.includes('Installed');
    }, { timeout: 30000 }).catch(() => {
        console.log('[CAPTURE] Qwen template install timed out; continuing...');
    });
    await sleep(1000);

    // Scroll to chat-template-section for visibility.
    await page.evaluate(() => {
        const section = document.getElementById('chat-template-section');
        if (section) section.scrollIntoView({ behavior: 'instant', block: 'center' });
    });
    await sleep(300);

    // Click "Manage template…" to open the shared lifecycle modal, which now
    // hosts the Community fixes section (was a standalone "Discussions" toggle).
    await page.evaluate(() => {
        const btn = Array.from(document.querySelectorAll('#ct-body button'))
            .find(b => b.textContent?.trim() === 'Manage template…');
        if (btn) btn.click();
    });
    await sleep(1500);

    // Wait for Community fixes content to load (>20 chars indicates real data).
    await page.waitForFunction(() => {
        const el = document.getElementById('chat-template-lifecycle-discussions');
        return el && el.textContent.length > 20 && !el.textContent.includes('Loading…');
    }, { timeout: 20000 }).catch(() => {
        console.log('[CAPTURE] Qwen discussions feed timed out; continuing...');
    });
    await sleep(500);

    // Capture 1: Community fixes section for Qwen (froggeric/Qwen-Fixed-Chat-Templates).
    await captureShot(page, 'discussions-feed-qwen-froggeric.png', { fullPage: true });

    // Click "Edit and install this fix" inside Community fixes to open the editor.
    const createFixOpened = await page.evaluate(() => {
        const btn = Array.from(
            document.querySelectorAll('#chat-template-lifecycle-discussions button')
        ).find(b => b.textContent?.trim() === 'Edit and install this fix');
        if (btn) btn.click();
        return !!btn;
    });
    await sleep(1200);

    if (createFixOpened) {
        // Wait for the create-fix editor overlay to appear (title "Edit and install this fix").
        const createFixState = await page.evaluate(() => {
            const modal = document.querySelector('.chat-template-create-fix-overlay');
            if (!modal) return { found: false };
            const repoInput = modal.querySelector('input[placeholder*="HF repo"]');
            return {
                found: true,
                repoValue: repoInput?.value || '',
                repoPlaceholder: repoInput?.placeholder || '',
            };
        });
        console.log('[CAPTURE] Create fix modal state:', JSON.stringify(createFixState));
        await captureShot(page, 'create-fix-auto-inferred-repo.png', { fullPage: true });

        // Close the create-fix editor via its Cancel button.
        await page.evaluate(() => {
            const modal = document.querySelector('.chat-template-create-fix-overlay');
            const cancelBtn = Array.from(modal?.querySelectorAll('button') || [])
                .find(b => b.textContent?.trim() === 'Cancel');
            if (cancelBtn) cancelBtn.click();
        });
        await sleep(400);
    } else {
        console.log('[CAPTURE] Create fix button not found; skipping create-fix capture');
    }

    // Close the lifecycle modal.
    await page.evaluate(() => {
        document.getElementById('chat-template-lifecycle-close')?.click();
    });
    await sleep(300);

    // ── Gemma4 workflow: force family → google/gemma-4-31B-it ──────────────────

    // Switch family to "gemma4" via wizardState and trigger install.
    await page.evaluate(async () => {
        const { wizardState } = await import('/js/features/spawn-wizard.js');
        const { autoInstallChatTemplate } = await import('/js/features/spawn-wizard-chat-template.js');
        wizardState.model.family = 'gemma4';
        wizardState.model.chatTemplateMode = 'auto';
        wizardState.model.chatTemplateCandidate = null;
        await autoInstallChatTemplate();
    });
    await sleep(1500);

    // Wait for Gemma4 template installation.
    await page.waitForFunction(() => {
        const status = document.getElementById('ct-status');
        return status && status.textContent.includes('Installed');
    }, { timeout: 30000 }).catch(() => {
        console.log('[CAPTURE] Gemma4 template install timed out; continuing...');
    });
    await sleep(1000);

    // Scroll to chat-template-section.
    await page.evaluate(() => {
        const section = document.getElementById('chat-template-section');
        if (section) section.scrollIntoView({ behavior: 'instant', block: 'center' });
    });
    await sleep(300);

    // Click "Manage template…" for Gemma4 to reveal the Community fixes section.
    await page.evaluate(() => {
        const btn = Array.from(document.querySelectorAll('#ct-body button'))
            .find(b => b.textContent?.trim() === 'Manage template…');
        if (btn) btn.click();
    });
    await sleep(1500);

    // Wait for Gemma4 Community fixes content.
    await page.waitForFunction(() => {
        const el = document.getElementById('chat-template-lifecycle-discussions');
        return el && el.textContent.length > 20 && !el.textContent.includes('Loading…');
    }, { timeout: 20000 }).catch(() => {
        console.log('[CAPTURE] Gemma4 discussions feed timed out; continuing...');
    });
    await sleep(500);

    // Capture 2: Community fixes section for Gemma4 (google/gemma-4-31B-it).
    await captureShot(page, 'discussions-feed-gemma4-google.png', { fullPage: true });

    // Close the lifecycle modal before continuing.
    await page.evaluate(() => {
        document.getElementById('chat-template-lifecycle-close')?.click();
    });
    await sleep(300);

    // Save the current gemma4 template path for the lifecycle modal capture below.
    const gemma4TemplatePath = await page.evaluate(async () => {
        const { wizardState } = await import('/js/features/spawn-wizard.js');
        return wizardState.model.chatTemplatePath || '';
    });
    // Close wizard.
    await page.keyboard.press('Escape');
    await sleep(800);

    // ── Preset editor: Lifecycle modal Discussions section ─────────────────────

    // Wait for the monitor/welcome view to settle after closing wizard.
    await page.waitForFunction(() => {
        const setup = document.getElementById('view-setup');
        const monitor = document.getElementById('view-monitor');
        return (setup && getComputedStyle(setup).display !== 'none')
            || (monitor && getComputedStyle(monitor).display !== 'none');
    }, { timeout: 5000 }).catch(() => {});
    await sleep(500);

    // Get the gemma4 template path for the lifecycle modal capture.
    // (gemma4TemplatePath was saved before closing the wizard).
    console.log('[CAPTURE] Using gemma4 template path:', gemma4TemplatePath);
    const installedTemplatePath = gemma4TemplatePath;

    // Open the preset editor via the seeded Rapid-MLX Qwen preset.
    const hasRapidPreset = await page.evaluate(() => {
        return !!document.querySelector('.launch-card[data-preset-id="capture-rapid-mlx"]');
    });

    if (hasRapidPreset) {
        await page.click('.launch-card[data-preset-id="capture-rapid-mlx"] .launch-card-btn-edit');
        await page.waitForSelector('#preset-modal.open.preset-editor--rapid-mlx', { visible: true });
        await sleep(600);

        // Ensure a chat template path is set (from the Qwen workflow above).
        let templatePath = await page.evaluate(() => {
            return (document.getElementById('modal-chat-template-file')?.value || '').trim();
        });

        // If no template is set, use the one from the wizard install.
        if (!templatePath && installedTemplatePath) {
            await page.evaluate((path) => {
                document.getElementById('modal-chat-template-file').value = path;
            }, installedTemplatePath);
            await sleep(300);
            templatePath = installedTemplatePath;
        }

        if (templatePath) {
            // Click "Manage" to open the Lifecycle modal.
            await page.evaluate(() => {
                document.getElementById('preset-chat-template-manage-btn')?.click();
            });
            await sleep(1500);

            // Wait for the lifecycle modal to open and Discussions section to populate.
            await page.waitForFunction(() => {
                const modal = document.getElementById('chat-template-lifecycle-modal');
                if (!modal || !modal.classList.contains('open')) return false;
                const discussionsEl = document.getElementById('chat-template-lifecycle-discussions');
                if (!discussionsEl) return false;
                const text = discussionsEl.textContent || '';
                return text.length > 20 && !text.includes('Loading…');
            }, { timeout: 20000 }).catch(() => {
                console.log('[CAPTURE] Lifecycle modal Discussions section timed out; continuing...');
            });

            // Capture 4: Lifecycle modal with Discussions section populated (gemma4/google).
            await captureShot(page, 'discussions-lifecycle-gemma4-google.png', { fullPage: true });

            // Close lifecycle modal.
            await page.evaluate(() => {
                document.getElementById('chat-template-lifecycle-close')?.click();
            });
            await sleep(300);
        } else {
            console.log('[CAPTURE] No template path available; skipping lifecycle modal capture');
        }

        // Close preset modal.
        await page.evaluate(() => document.getElementById('preset-modal-close')?.click());
        await sleep(300);
    } else {
        console.log('[CAPTURE] Rapid-MLX preset card not found; skipping lifecycle modal capture');
    }
}

// Animated GIF walking through the setup wizard: welcome → profile → model → VRAM → summary.
//
// Design: fully sequential — each wizard state is fully reached before frames are captured.
// This guarantees every frame reflects real UI state, with no race between capture and interaction.
//
// VRAM panel uses injected state so the bar renders reliably without a GPU or HF API.
