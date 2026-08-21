// Scenario: models-v2
// Extracted from tests/ui/capture.mjs (Phase A3).
import { attachToServer } from '../../harness/attach.mjs';
import { gotoApp } from '../../harness/browser.mjs';
import { DEFAULT_VIEWPORT, sleep } from '../../harness/paths.mjs';
import { captureShot } from '../../harness/shot.mjs';

// Installs the deterministic fetch mocks used by this scenario. `available`
// controls the Rapid-MLX/Apple-Silicon platform simulation. Used both for the
// initial page load and (via evaluateOnNewDocument + reload) to flip platform
// availability, since getPlatformInfo() caches its response in a
// module-scoped promise that a same-page flag flip cannot invalidate.
function installFetchMocks(available) {
    window.__captureRapidMlxAvailable = available;
    const typedInventory = [
        { model_name: 'Qwen 3.6 35B', filename: 'qwen-35b-Q6_K.gguf', path: '/models/gguf/qwen-35b-Q6_K.gguf', format: 'gguf', source: 'local', lifecycle: 'ready', compatibility: 'verified', supported_backends: ['llama_cpp'], size_display: '25.4 GiB', quant_type: 'Q6_K' },
        { model_name: 'Qwen 3 MLX 4-bit', filename: 'mlx-community/Qwen3-8B-4bit', path: '/models/cache/huggingface/qwen3-mlx', format: 'mlx', source: 'hugging_face', lifecycle: 'ready', compatibility: 'provisional', supported_backends: ['rapid_mlx'], size_display: '4.8 GiB' },
        { model_name: 'Gemma conversion', filename: 'gemma-official-conversion', path: '/models/mlx/converted/gemma', format: 'mlx', source: 'official_conversion', lifecycle: 'converting', compatibility: 'verified', supported_backends: ['rapid_mlx'], size_display: '12.1 GiB' },
        { model_name: 'Recovered FP16 (Experimental)', filename: 'fp16', path: '/models/rapid-mlx/imports/recovered/fp16', format: 'mlx', source: 'recovered_gguf', lifecycle: 'ready', compatibility: 'experimental', supported_backends: [], size_display: '272.4 MiB', quant_type: 'F16 recovered' },
        { model_name: 'Re-quantized MLX (Experimental)', filename: 'model', path: '/models/rapid-mlx/requantized/quant/model', format: 'mlx', source: 'requantized_mlx', lifecycle: 'ready', compatibility: 'experimental', supported_backends: [], size_display: '146.5 MiB', quant_type: 'affine_8bit_g64' },
        { model_name: 'Staged download', filename: 'model.safetensors.part', path: '/models/.staging/downloads/model.safetensors.part', format: 'unknown', source: 'legacy', lifecycle: 'incomplete', compatibility: 'unknown', supported_backends: [] },
        { model_name: 'Invalid tokenizer bundle', filename: 'broken-transformers-model', path: '/models/transformers/broken', format: 'transformers', source: 'local', lifecycle: 'invalid', compatibility: 'unsupported', supported_backends: [] },
        { model_name: 'Gemma vision projector', filename: 'gemma-mmproj.gguf', path: '/models/gguf/gemma-mmproj.gguf', format: 'gguf', source: 'local', lifecycle: 'ready', compatibility: 'verified', supported_backends: ['llama_cpp'], companion_kind: 'mmproj' },
        { model_name: 'Qwen draft companion', filename: 'qwen-draft.gguf', path: '/models/gguf/qwen-draft.gguf', format: 'gguf', source: 'local', lifecycle: 'ready', compatibility: 'verified', supported_backends: ['llama_cpp'], companion_kind: 'draft', is_draft_assistant: true },
    ];
    const originalFetch = window.fetch.bind(window);
    window.fetch = (input, init) => {
        const url = new URL(typeof input === 'string' ? input : input.url, location.href);
        if (url.pathname === '/api/models') {
            return Promise.resolve(new Response(JSON.stringify(typedInventory), {
                status: 200,
                headers: { 'Content-Type': 'application/json' },
            }));
        }
        if (url.pathname === '/api/hf/download-dir') {
            return Promise.resolve(new Response(JSON.stringify({ dir: '/models', configured: true }), {
                status: 200,
                headers: { 'Content-Type': 'application/json' },
            }));
        }
        if (url.pathname === '/api/llama-binary/platform-info') {
            return Promise.resolve(new Response(JSON.stringify({
                os: window.__captureRapidMlxAvailable ? 'macos' : 'linux',
                arch: window.__captureRapidMlxAvailable ? 'aarch64' : 'x86_64',
                rapid_mlx_local_available: window.__captureRapidMlxAvailable,
                rapid_mlx_local_requirement: 'Rapid-MLX local execution requires macOS on Apple Silicon',
            }), {
                status: 200,
                headers: { 'Content-Type': 'application/json' },
            }));
        }
        if (url.pathname === '/api/models/import-lab/availability') {
            return Promise.resolve(new Response(JSON.stringify({
                local_execution_available: window.__captureRapidMlxAvailable,
                platform_requirement: 'Apple Silicon macOS',
                supported_profile: 'smollm2-135m-instruct-llama-v1',
                compatibility: 'experimental',
                launchable: false,
                fallback_engine: 'llama.cpp',
            }), { status: 200, headers: { 'Content-Type': 'application/json' } }));
        }
        if (url.pathname === '/api/models/gguf/import/compatibility/preview') {
            return Promise.resolve(new Response(JSON.stringify({
                architecture: 'llama', tensor_count: 272, compatibility: 'experimental',
                missing_profile_fields: [], missing_assets: [],
                warnings: ['Experimental profile; llama.cpp remains available'],
                unsupported_reasons: [],
            }), { status: 200, headers: { 'Content-Type': 'application/json' } }));
        }
        if (url.pathname === '/api/models/import-lab/resource-estimate') {
            return Promise.resolve(new Response(JSON.stringify({
                source_bytes: 146456727, estimated_fp16_bytes: 272437300,
                required_disk_bytes: 1081741896, available_disk_bytes: 53687091200,
                available_ram_bytes: 34359738368, disk_sufficient: true,
                ram_guidance: 'comfortable',
            }), { status: 200, headers: { 'Content-Type': 'application/json' } }));
        }
        if (url.pathname === '/api/models/import-lab/jobs') {
            return Promise.resolve(new Response(JSON.stringify([{
                id: 'capture-job', state: 'recovering', phase: 'recovering_fp16',
                progress_percent: 63,
                message: 'Recovering tensors into an isolated non-launchable staging cache',
                can_cancel: true,
                diagnostics: ['Original GGUF will not be modified', 'Validated exact Q8_0 recovery profile'],
            }]), { status: 200, headers: { 'Content-Type': 'application/json' } }));
        }
        return originalFetch(input, init);
    };
}

export default async function(ctx, options) {
    const { page, baseUrl } = ctx;
    await gotoApp(page, baseUrl);
    await page.evaluate(() => {
        localStorage.setItem('llama-monitor-models-prefs', JSON.stringify({
            viewMode: 'cards',
            showMmproj: true,
            showMain: true,
            showSplit: true,
            showDraftModels: true,
        }));
    });
    await page.evaluateOnNewDocument(installFetchMocks, true);
    await page.reload({ waitUntil: 'networkidle0' });
    await page.waitForSelector('html.modules-ready', { timeout: 15000 });
    await page.evaluate(installFetchMocks, true);
    if (!options.noAttach) {
        try { await attachToServer(page); } catch {}
    }

    await page.evaluate(() => window.openModelsModal?.());
    await page.waitForSelector('#models-modal.open', { timeout: 8000 });
    await sleep(1500);
    await captureShot(page, 'models-discovery-overview.png', { fullPage: true });
    await captureShot(page, 'models-inventory-dark.png', { fullPage: true });
    await page.evaluate(() => { document.documentElement.dataset.theme = 'light'; });
    await sleep(250);
    await captureShot(page, 'models-inventory-light.png', { fullPage: true });
    await page.evaluate(() => { document.documentElement.dataset.theme = 'dark'; });
    await page.setViewport({ width: 430, height: 900, deviceScaleFactor: 1 });
    await sleep(250);
    await captureShot(page, 'models-inventory-narrow.png', { fullPage: true });
    await page.setViewport(DEFAULT_VIEWPORT);
    await sleep(250);

    await page.evaluate(() => {
        const card = [...document.querySelectorAll('.mm-model-card')]
            .find(candidate => candidate.textContent.includes('Qwen 3 MLX 4-bit'));
        const button = [...(card?.querySelectorAll('button') || [])]
            .find(candidate => candidate.textContent.includes('Repair MTP sidecar'));
        button?.click();
    });
    await page.waitForSelector('#preset-modal.open', { timeout: 8000 });
    await page.waitForSelector('#modal-rapid-speculative-repair-form');
    await captureShot(page, 'models-mtp-repair-seeded.png', { fullPage: true });
    await page.click('#preset-modal-close');
    await page.evaluate(() => window.openModelsModal?.());
    await page.waitForSelector('#models-modal.open', { timeout: 8000 });
    await sleep(500);

    await page.evaluate(() => {
        document.documentElement.dataset.theme = 'dark';
        document.querySelector('.mm-tab[data-tab="import-lab"]')?.click();
    });
    await page.waitForSelector('#mm-import-platform');
    await page.type('#mm-import-source', 'gguf/SmolLM2-135M-Instruct-Q8_0.gguf');
    await page.click('#mm-import-analyze');
    await page.waitForSelector('.mm-import-verdict-badge.experimental');
    await sleep(350);
    await captureShot(page, 'models-import-lab-dark.png', { fullPage: true });
    await page.evaluate(() => { document.documentElement.dataset.theme = 'light'; });
    await sleep(250);
    await captureShot(page, 'models-import-lab-light.png', { fullPage: true });
    await page.emulateMediaFeatures([{ name: 'prefers-reduced-motion', value: 'reduce' }]);
    await page.setViewport({ width: 430, height: 900, deviceScaleFactor: 1 });
    await sleep(250);
    await captureShot(page, 'models-import-lab-reduced-narrow.png', { fullPage: true });
    await page.setViewport(DEFAULT_VIEWPORT);
    await page.emulateMediaFeatures([{ name: 'prefers-reduced-motion', value: 'no-preference' }]);
    await page.evaluate(() => { document.documentElement.dataset.theme = 'dark'; });

    await page.evaluate(() => {
        const downloadTab = document.querySelector('.mm-tab[data-tab="download"]');
        if (downloadTab) downloadTab.click();
    });
    await sleep(800);

    await page.evaluate(() => {
        const panel = document.getElementById('mm-hf-download-panel');
        const idle = document.getElementById('mm-hf-dlp-idle');
        const progress = document.getElementById('mm-hf-dlp-progress');
        const fileName = document.getElementById('mm-hf-dlp-file-name');
        const destPath = document.getElementById('mm-hf-dlp-dest-path');
        const progressFile = document.getElementById('mm-hf-dlp-progress-file');
        const progressBar = document.getElementById('mm-hf-dlp-bar');
        const progressPct = document.getElementById('mm-hf-dlp-progress-pct');
        const stats = document.getElementById('mm-hf-dlp-stats');

        if (panel) panel.style.display = 'block';
        if (panel) panel.style.maxHeight = 'none';
        if (idle) idle.style.display = 'none';
        if (progress) progress.style.display = 'block';
        if (fileName) fileName.textContent = 'llama-3.1-8b-instruct-Q4_K_M.gguf';
        if (destPath) destPath.textContent = '~/.config/llama-monitor/models/';
        if (progressFile) progressFile.textContent = 'llama-3.1-8b-instruct-Q4_K_M.gguf';
        if (progressBar) progressBar.style.width = '65%';
        if (progressPct) progressPct.textContent = '65%';
        if (stats) stats.textContent = '24.7 MB / 37.9 MB · 2.4 MB/s';
    });
    await sleep(800);
    await captureShot(page, 'models-hf-download-panel.png', { fullPage: true });

    // Non-Apple-silicon inventory capture must come last: it needs a fresh
    // page load with the platform mock pre-installed (evaluateOnNewDocument)
    // because getPlatformInfo() caches its response and never refetches, so
    // flipping the availability flag on an already-loaded page has no effect.
    await page.evaluateOnNewDocument(installFetchMocks, false);
    await page.reload({ waitUntil: 'networkidle0' });
    await page.waitForSelector('html.modules-ready', { timeout: 15000 });
    await page.evaluate(() => window.openModelsModal?.());
    await page.waitForSelector('#models-modal.open', { timeout: 8000 });
    await sleep(1000);
    await captureShot(page, 'models-inventory-non-apple.png', { fullPage: true });
}
