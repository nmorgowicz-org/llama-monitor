// Capture CLI entry point: arg parsing, scenario registry, run orchestration.
// Rebuilt from tests/ui/capture.mjs (Phase A1-A4 split); see docs/plans/
// 20260804-branch_audit_capture_split_chat_template_ux.md Phase A.
import {
    DEFAULT_VIEWPORT, RUNNING_PORT,
} from './harness/paths.mjs';
import { seedConfig, findAvailablePort, spawnLlamaMonitor, cleanupServer, cleanupTempHome } from './harness/server.mjs';
import { seedRapidMlxCapturePreset, seedNestedMlxFixture, seedModelsDirFixture } from './harness/fixtures.mjs';
import { launchBrowser } from './harness/browser.mjs';
import { cleanupFrames } from './harness/shot.mjs';

import scenarioWelcome from './scenarios/core/welcome.mjs';
import scenarioFreeCache from './scenarios/core/free-cache.mjs';
import scenarioChat from './scenarios/core/chat.mjs';
import scenarioSidebar from './scenarios/core/sidebar.mjs';
import scenarioGuidedGen from './scenarios/core/guided-gen.mjs';
import scenarioNavbar from './scenarios/core/navbar.mjs';
import scenarioSmoke from './scenarios/core/smoke.mjs';
import scenarioPanels from './scenarios/core/panels.mjs';
import scenarioDashboard from './scenarios/core/dashboard.mjs';
import scenarioModels from './scenarios/models/models.mjs';
import scenarioModelsV2 from './scenarios/models/models-v2.mjs';
import scenarioModelDiscovery from './scenarios/models/model-discovery.mjs';
import scenarioFilebrowser from './scenarios/models/filebrowser.mjs';
import scenarioPresetEditor from './scenarios/presets/preset-editor.mjs';
import scenarioRapidPreset from './scenarios/presets/rapid-preset.mjs';
import scenarioEvidenceDrawer from './scenarios/presets/evidence-drawer.mjs';
import scenarioCommunitySources from './scenarios/presets/community-sources.mjs';
import scenarioDiscussions from './scenarios/presets/discussions.mjs';
import scenarioSpawnWizard from './scenarios/wizard-llamacpp/spawn-wizard.mjs';
import scenarioSpawnWizardEngines from './scenarios/wizard-llamacpp/spawn-wizard-engines.mjs';
import scenarioSpawnWizardGif from './scenarios/wizard-llamacpp/spawn-wizard-gif.mjs';
import scenarioSpawnWizardHfDownload from './scenarios/wizard-llamacpp/spawn-wizard-hf-download.mjs';
import scenarioSpawnWizardRapidMlxGif from './scenarios/wizard-rapidmlx/spawn-wizard-rapid-mlx-gif.mjs';
import scenarioRapidMlxRuntime from './scenarios/wizard-rapidmlx/rapid-mlx-runtime.mjs';
import scenarioRapidMlxLive from './scenarios/wizard-rapidmlx/rapid-mlx-live.mjs';
import scenarioDashboardRapidMlx from './scenarios/wizard-rapidmlx/dashboard-rapid-mlx.mjs';
import scenarioSettings from './scenarios/config/settings.mjs';
import scenarioAppearancePalette from './scenarios/config/appearance-palette.mjs';
import scenarioTls from './scenarios/config/tls.mjs';
import scenarioTunePanel from './scenarios/features/tune-panel.mjs';
import scenarioBenchmarkResults from './scenarios/features/benchmark-results.mjs';
import scenarioLlamaUpdater from './scenarios/features/llama-updater.mjs';
import scenarioChatHistoryQA from './scenarios/features/chat-history-qa.mjs';
import scenarioSparkline from './scenarios/validation/sparkline.mjs';
import scenarioGifs from './scenarios/validation/gifs.mjs';

function parseArgs(argv) {
    const options = {
        scenario: null,
        gpuOnly: false,
        inferenceOnly: false,
        listScenarios: false,
        noAttach: false,
        closeUp: false,
        viewport: { ...DEFAULT_VIEWPORT },
    };

    for (let i = 0; i < argv.length; i += 1) {
        const arg = argv[i];
        if (arg === '--scenario' && argv[i + 1]) {
            options.scenario = argv[i + 1];
            i += 1;
        } else if (arg === '--chat-only') {
            options.chatOnly = true;
        } else if (arg === '--gpu-only') {
            options.gpuOnly = true;
        } else if (arg === '--inference-only') {
            options.inferenceOnly = true;
        } else if (arg === '--list-scenarios') {
            options.listScenarios = true;
        } else if (arg === '--no-attach') {
            options.noAttach = true;
        } else if (arg === '--close-up') {
            options.closeUp = true;
        } else if (arg === '--help' || arg === '-h') {
            options.help = true;
        }
    }

    if (options.gpuOnly && options.inferenceOnly) {
        throw new Error('Use only one of --gpu-only or --inference-only');
    }

    return options;
}

function printUsage() {
    console.log(`Usage:
  node tests/ui/capture.mjs --scenario <name> [options]

Scenarios:
  Core
    welcome          Welcome screen, auth shell, and setup wizard button (no attach required)
    free-cache       Native Free Cache confirmation on the welcome screen
    chat             Chat, telemetry, logs

  Chat Features
    guided-gen       Suggestions, quick guide, director, surprise, explicit mode
    sidebar          Sidebar, FTS search flyout, context menu, title filter
    chat-history-qa  History Q&A panel (ask questions about conversation)

  Models and Presets
    models-v2        Models modal: typed inventory, Import Lab, and HF download panel
    model-discovery  HF Download tab: scope selector, sort, category/qualification badges (real HF data)
    preset-editor    Preset editor: model/context, GPU, and advanced tabs
    rapid-preset     Rapid-MLX welcome cards and preset editor (legacy and typed sources)
    evidence-drawer  Shared decision evidence drawer: dark, expanded, light, and narrow reduced-motion
    community-sources Model Manager source catalog: list and editor in dark/light/narrow states

  Configuration
    settings         Settings modal, preferences, persona, models, shortcuts
    tls              TLS modes and ACME (Certificates tab, each TLS mode, custom certs, ACME config)
    filebrowser      File browser modal (Browse buttons in Config modal, modal open)
    panels           Chat config panels (behavior, model, style, debug)
    dashboard        Server tab, GPU section
    dashboard-rapid-mlx  Deterministic Rapid-MLX telemetry cards (dark and light)

   Setup wizard
     spawn-wizard           Steps 1–6: profiles, model, VRAM, parameters, summary, start/close
     spawn-wizard-engines      llama.cpp/Rapid-MLX engine cards and Rapid-MLX hardware handoff
     spawn-wizard-gif          Animated GIF walking through all setup wizard steps (llama.cpp path)
     spawn-wizard-rapid-mlx-gif  Animated GIF: Rapid-MLX engine, hardware controls, profile hints, spawn
     spawn-wizard-hf-download  HF download panel: idle options and simulated progress
     discussions             Chat template Discussions: Qwen (froggeric) and Gemma4 (google) feeds,
                            Create fix modal, and Lifecycle modal Discussions section

   Performance & Updates
    tune-panel          Performance benchmark dropdown (pill + panel) on server tab
    benchmark-results   Runs a real benchmark and captures the results view
    llama-updater       Llama-server binary update pill and version modal with release notes

  Validation
    sparkline        Sparkline validation screenshots
    gifs             Inference/GPU animated GIF capture
    smoke            Startup smoke test

  Rapid-MLX Runtime (developer only, NOT CI)
    rapid-mlx-live   Full runtime flow: spawn Qwen3-0.6B-4bit → telemetry → chat → stop

  Appearance
    appearance-palette Settings Appearance palette stills plus light-mode dashboard
    navbar           Nav bar close-ups: idle-dark, low-power active, idle-light (requires --close-up)

Options:
  --gpu-only         For gifs scenario, capture only GPU/system animation
  --inference-only   For gifs scenario, capture only inference animation
  --no-attach        Skip remote attach for scenarios that do not require it
  --close-up         Also capture element-level close-ups (debugging only)
  --list-scenarios   Print available scenarios

Examples:
  node tests/ui/capture.mjs --scenario welcome
  SCREENSHOT_PORT=8892 node tests/ui/capture.mjs --scenario chat
  SCREENSHOT_PORT=9001 node tests/ui/capture.mjs --scenario guided-gen
  SCREENSHOT_PORT=8893 node tests/ui/capture.mjs --scenario sidebar
  SCREENSHOT_PORT=8895 node tests/ui/capture.mjs --scenario gifs --gpu-only
  SCREENSHOT_PORT=8894 node tests/ui/capture.mjs --scenario settings --close-up
  SCREENSHOT_PORT=8896 node tests/ui/capture.mjs --scenario spawn-wizard --no-attach
  SCREENSHOT_PORT=8898 node tests/ui/capture.mjs --scenario spawn-wizard-engines --no-attach
  SCREENSHOT_PORT=8897 node tests/ui/capture.mjs --scenario spawn-wizard-gif --no-attach
   SCREENSHOT_PORT=8997 node tests/ui/capture.mjs --scenario spawn-wizard-rapid-mlx-gif --no-attach
   SCREENSHOT_PORT=8910 node tests/ui/capture.mjs --scenario discussions --no-attach
   SCREENSHOT_PORT=8900 node tests/ui/capture.mjs --scenario tune-panel
   SCREENSHOT_PORT=8901 node tests/ui/capture.mjs --scenario llama-updater
  SCREENSHOT_PORT=8902 node tests/ui/capture.mjs --scenario chat-history-qa
   RUNNING_PORT=8080 node tests/ui/capture.mjs --scenario dashboard
   RUNNING_PORT=8080 node tests/ui/capture.mjs --scenario gifs --gpu-only
   SCREENSHOT_PORT=8910 node tests/ui/capture.mjs --scenario rapid-mlx-live

Note: RUNNING_PORT connects to an already-running llama-monitor (e.g. your production instance
with a remote agent reporting GPU data). No binary is spawned; no temp config is seeded.
`);
}

const SCENARIOS = {
    'welcome': { run: scenarioWelcome },
    'free-cache': { run: scenarioFreeCache },
    'rapid-preset': { run: scenarioRapidPreset, setup: () => { seedRapidMlxCapturePreset(); } },
    'evidence-drawer': { run: scenarioEvidenceDrawer },
    'community-sources': { run: scenarioCommunitySources },
    'chat': { run: scenarioChat },
    'guided-gen': { run: scenarioGuidedGen },
    'sidebar': { run: scenarioSidebar },
    'models-v2': { run: scenarioModelsV2, setup: () => ({ extraArgs: seedModelsDirFixture() }) },
    'model-discovery': { run: scenarioModelDiscovery },
    'preset-editor': { run: scenarioPresetEditor },
    'settings': { run: scenarioSettings },
    'appearance-palette': { run: scenarioAppearancePalette },
    'tls': { run: scenarioTls },
    'filebrowser': { run: scenarioFilebrowser },
    'panels': { run: scenarioPanels, setup: () => ({ extraArgs: seedModelsDirFixture() }) },
    'models': { run: scenarioModels, setup: () => ({ extraArgs: seedModelsDirFixture() }) },
    'dashboard': { run: scenarioDashboard },
    'dashboard-rapid-mlx': { run: scenarioDashboardRapidMlx },
    'spawn-wizard': { run: scenarioSpawnWizard },
    'spawn-wizard-engines': { run: scenarioSpawnWizardEngines, setup: () => { seedNestedMlxFixture(); } },
    'spawn-wizard-gif': { run: scenarioSpawnWizardGif },
    'spawn-wizard-rapid-mlx-gif': { run: scenarioSpawnWizardRapidMlxGif },
    'spawn-wizard-hf-download': { run: scenarioSpawnWizardHfDownload },
    'discussions': { run: scenarioDiscussions, setup: () => { seedRapidMlxCapturePreset(); } },
    'tune-panel': { run: scenarioTunePanel },
    'benchmark-results': { run: scenarioBenchmarkResults },
    'llama-updater': { run: scenarioLlamaUpdater },
    'chat-history-qa': { run: scenarioChatHistoryQA },
    'rapid-mlx-runtime': { run: scenarioRapidMlxRuntime },
    'rapid-mlx-live': { run: scenarioRapidMlxLive },
    'sparkline': { run: scenarioSparkline },
    'gifs': { run: scenarioGifs },
    'smoke': { run: scenarioSmoke },
    'navbar': { run: scenarioNavbar },
};

export async function runCli({ scenario: forcedScenario = null, argv = process.argv.slice(2) } = {}) {
    const options = parseArgs(argv);
    if (options.help) {
        printUsage();
        return;
    }
    if (options.listScenarios) {
        Object.keys(SCENARIOS).forEach(name => console.log(name));
        return;
    }

    // Default to the welcome flow so an unqualified invocation still succeeds
    // without requiring a remote attach.
    const scenarioName = forcedScenario || options.scenario || 'welcome';
    const scenario = SCENARIOS[scenarioName];
    if (!scenario) {
        throw new Error(`Unknown scenario "${scenarioName}". Use --list-scenarios.`);
    }

    let server = null;
    let browser = null;
    let baseUrl;

    if (RUNNING_PORT) {
        baseUrl = `http://127.0.0.1:${RUNNING_PORT}`;
        console.log(`[CAPTURE] Using running llama-monitor at ${baseUrl} for scenario "${scenarioName}"...`);
    } else {
        seedConfig();
        const setupResult = scenario.setup ? (await scenario.setup()) || {} : {};
        const extraArgs = setupResult.extraArgs || [];

        const port = await findAvailablePort();
        console.log(`[CAPTURE] Spawning llama-monitor on port ${port} for scenario "${scenarioName}"...`);
        server = await spawnLlamaMonitor(port, extraArgs);
        baseUrl = server.url;
    }

    try {
        const launched = await launchBrowser(options.viewport);
        browser = launched.browser;
        const page = launched.page;
        await scenario.run({ page, baseUrl, browser }, options);
        console.log(`[CAPTURE] Scenario "${scenarioName}" complete.`);
    } catch (err) {
        console.error(err.stack || err.message);
        process.exitCode = 1;
    } finally {
        cleanupFrames();
        if (browser) await browser.close();
        if (server) await cleanupServer(server);
        if (!RUNNING_PORT) cleanupTempHome();
    }
}

if (import.meta.url === `file://${process.argv[1]}`) {
    await runCli();
}
