#!/usr/bin/env node
/**
 * Human-friendly Rapid-MLX matrix driver.
 *
 * It materializes evidence-pinned manifests internally, starts one fresh
 * server per configuration, delegates individual requests to the generic
 * OpenAI-compatible runner, and writes an index of the resulting receipts.
 * This deliberately keeps the generic runner reusable for llama.cpp/OMLX.
 */
import { mkdir, mkdtemp, readFile, readdir, readlink, rm, symlink, unlink, writeFile } from 'node:fs/promises';
import { spawn, spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { tmpdir } from 'node:os';
import { basename, join, resolve } from 'node:path';

const DEFAULT_PORT = 18087;
// Matches rapid-mlx serve --help's own documented default (0.0-1.0, default: 0.90).
// Do not raise to 0.95 here: upstream reserves that for 200GB+ models, not a system-size default.
const DEFAULT_UTILIZATION = '0.90';
// 200000 is the highest tier exercised anywhere in this suite: the user's
// stated pie-in-the-sky ceiling is 200k regardless of a model's larger
// (256k/262k) advertised native context, so no cell in this file should
// target more.
const CONTEXT_WORDS = { 8000: 3000, 16000: 6000, 32000: 12500, 65536: 25000, 131072: 52000, 160000: 63500, 200000: 79500 };
const MODEL_PREFIX = 'models--';
// Qwen3.5/3.6 think by default. Without a bounded reasoning_max_tokens, a
// 128-token probe cap is spent entirely on <think> preamble and the model
// never reaches the CHECK_* answer, corrupting the fidelity check (not the
// PP/TG throughput numbers themselves, which are content-agnostic token
// counts). 16000 mirrors the operator's own everyday reasoning-budget
// setting and the tools suite's existing reasoning_max_tokens; 512 is
// headroom for the short marker-list answer once the budget closes thinking.
const REASONING_BUDGET = 16000;
const MAX_TOKENS = REASONING_BUDGET + 512;
const SAFE_MLLM_PREFILL_STEPS = new Set([512, 1024, 1536, 2048]);

function die(message) { throw new Error(message); }

function parseArgs(argv) {
  const [command, ...rest] = argv.slice(2);
  const options = { suite: 'smoke', port: DEFAULT_PORT, utilization: DEFAULT_UTILIZATION };
  for (let index = 0; index < rest.length; index += 1) {
    const key = rest[index];
    if (!key.startsWith('--')) die(`Invalid argument: ${key}`);
    if (key === '--keep-manifests') { options.keepManifests = true; continue; }
    if (key === '--resume') { options.resume = true; continue; }
    if (key === '--force-hybrid') { options.forceHybrid = true; continue; }
    const value = rest[index + 1];
    if (value === undefined) die(`Missing value for ${key}`);
    index += 1;
    if (key === '--model') options.model = value;
    else if (key === '--suite') options.suite = value;
    else if (key === '--cell') (options.cells ??= []).push(value);
    else if (key === '--rapid-mlx-bin') options.rapidMlxBin = value;
    else if (key === '--out') options.out = value;
    else if (key === '--port') options.port = Number(value);
    else if (key === '--gpu-memory-utilization') options.utilization = value;
    else if (key === '--image') options.image = value;
    else if (key === '--expected-visual-term') (options.expectedVisualTerms ??= []).push(value);
    else if (key === '--mllm-prefill-step-size') options.mllmPrefillStepSize = Number(value);
    else if (key === '--tokenizer-python') options.tokenizerPython = value;
    else if (key === '--workspace-pack') options.workspacePack = value;
    else if (key === '--cache-contexts') options.cacheContexts = value.split(',').map((item) => Number(item));
    else if (key === '--cache-memory-mb') options.cacheMemoryMb = value.split(',').map((item) => Number(item));
    else if (key === '--cache-dtypes') options.cacheDtypes = value.split(',');
    else if (key === '--cache-disk-checkpoint-intervals') options.cacheDiskCheckpointIntervals = value.split(',').map((item) => Number(item));
    else if (key === '--revision') options.revision = value;
    else if (key === '--chat-template') options.chatTemplate = value;
    else die(`Unknown option: ${key}`);
  }
  if (!['plan', 'run'].includes(command)) die('Use plan or run.');
  if (!options.model) die('--model is required.');
  if (command === 'run' && !options.out) die('run requires --out RECEIPT_DIRECTORY.');
  if (!['smoke', 'context', 'pflash', 'cache', 'tools', 'image', 'prefill', 'quant-baseline', 'turboquant-scale', 'ubatch', 'all'].includes(options.suite)) {
    die(`Unknown suite: ${options.suite}`);
  }
  return { command, options };
}

function toolDefinitions() {
  return [
    { type: 'function', function: { name: 'read_file', description: 'Read a source file.', parameters: { type: 'object', properties: { path: { type: 'string' } }, required: ['path'], additionalProperties: false } } },
    { type: 'function', function: { name: 'apply_patch', description: 'Apply a small source edit.', parameters: { type: 'object', properties: { path: { type: 'string' }, replacement: { type: 'string' } }, required: ['path', 'replacement'], additionalProperties: false } } },
    { type: 'function', function: { name: 'list_files', description: 'List files in a directory.', parameters: { type: 'object', properties: { path: { type: 'string' } }, required: ['path'], additionalProperties: false } } },
    { type: 'function', function: { name: 'search_code', description: 'Search source files for a pattern.', parameters: { type: 'object', properties: { pattern: { type: 'string' }, path: { type: 'string' } }, required: ['pattern'], additionalProperties: false } } },
    { type: 'function', function: { name: 'run_command', description: 'Execute a shell command and return its output.', parameters: { type: 'object', properties: { command: { type: 'string' } }, required: ['command'], additionalProperties: false } } },
  ];
}

// Five scattered numeric markers across the context window score fidelity as
// graduated recall rather than a single verbatim string match, so results
// stay meaningful across model/quant/KV-quant/sampling variation instead of
// flipping pass/fail on sampling noise.
const CONTEXT_MARKERS = [
  { position: 0.1, name: 'CHECK_ALPHA', value: 8291 },
  { position: 0.3, name: 'CHECK_BRAVO', value: 4417 },
  { position: 0.5, name: 'CHECK_CHARLIE', value: 6053 },
  { position: 0.7, name: 'CHECK_DELTA', value: 1928 },
  { position: 0.9, name: 'CHECK_ECHO', value: 7360 },
];

function workload(tokens, overrides = {}) {
  return {
    corpus: 'code',
    target_words: CONTEXT_WORDS[tokens],
    max_tokens: MAX_TOKENS,
    markers: CONTEXT_MARKERS,
    extra_body: { reasoning_max_tokens: REASONING_BUDGET },
    ...overrides,
  };
}

function configuration({
  dtype = 'int8',
  pflash = 'off',
  turboquant = 'none',
  cache = false,
  cacheMemoryMb = 8192,
  hybridCacheEntries = 16,
  diskCheckpointInterval = 0,
  maxTokens = MAX_TOKENS,
  mllm = false,
  prefillStepSize = DEFAULT_PREFILL_STEP_SIZE,
  mllmPrefillStepSize = null,
}) {
  if (mllm && pflash !== 'off') die('MLLM benchmark cells must disable PFlash.');
  if (mllm && !SAFE_MLLM_PREFILL_STEPS.has(mllmPrefillStepSize)) {
    die('MLLM benchmark cells allow only --prefill-step-size 512, 1024, 1536, or 2048.');
  }
  const expectedMetrics = {
    [`rapid_mlx_turboquant_mode{mode="${turboquant === 'none' ? 'disabled' : turboquant}"}`]: 1,
  };
  // TurboQuant replaces the normal dtype implementation; its explicit mode
  // label is the authoritative assertion for that A/B, not the pre-transform
  // `--kv-cache-dtype` request.
  if (turboquant === 'none') expectedMetrics[`rapid_mlx_kv_cache_dtype{dtype="${dtype}"}`] = 1;
  return {
    kv_cache_dtype_requested: dtype,
    turboquant_requested: turboquant,
    // Long-context cache controls start with the agreed 8 GiB floor. Later
    // rows may raise this only from observed occupancy/eviction evidence.
    prefix_cache: cache ? `enabled; cache-memory-mb=${cacheMemoryMb}; hybrid-cache-entries=${hybridCacheEntries}` : 'disabled',
    cache_memory_mb: cache ? cacheMemoryMb : null,
    hybrid_cache_entries: cache ? hybridCacheEntries : null,
    disk_checkpoint_interval: diskCheckpointInterval,
    pflash,
    server_max_tokens: maxTokens,
    concurrency: 1,
    mllm,
    prefill_step_size: prefillStepSize,
    // This is a separately recorded MLLM setting. Never relax the ceiling
    // above 2048: larger values caused unsafe unified-memory spikes on M5
    // Max before request processing began.
    mllm_prefill_step_size: mllmPrefillStepSize,
    expected_metrics: expectedMetrics,
  };
}

function cell(model, id, tokens, config, overrides = {}) {
  return { id, model, target_tokens: tokens, configuration: config, sequence: ['cold'], workload: workload(tokens, overrides) };
}

async function imageFixtureMetadata(imagePath) {
  const sourcePath = resolve(imagePath);
  const bytes = await readFile(sourcePath);
  const isPng = bytes.subarray(0, 8).equals(Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]));
  if (!isPng || bytes.length < 24) die(`Image fixture must be a readable PNG: ${sourcePath}`);
  return {
    source_path: sourcePath,
    sha256: createHash('sha256').update(bytes).digest('hex'),
    bytes: bytes.length,
    mime_type: 'image/png',
    width: bytes.readUInt32BE(16),
    height: bytes.readUInt32BE(20),
  };
}

async function suiteCells(model, suite, imagePath, expectedVisualTerms = [], mllmPrefillStepSize = 1024, workspacePackPath = 'tests/fixtures/calibration/workspace-cache/project-pack.json') {
  const include = (name) => suite === 'all' || suite === name;
  const cells = [];
  let workspacePack = null;
  if (include('cache')) {
    const parsed = JSON.parse(await readFile(resolve(workspacePackPath), 'utf8'));
    const words = parsed.corpus.trim().split(/\s+/).length;
    workspacePack = {
      sha256: parsed.corpus_sha256,
      files: parsed.files,
      corpus: parsed.corpus,
      words,
    };
  }
  if (include('smoke')) cells.push(cell(model, 'smoke-8k-int8', 8000, configuration({ dtype: 'int8' })));
  if (include('context')) {
    // 131072/160000/200000 here use pflash 'off' (the configuration()
    // default): this is the native, no-compression "if memory allows"
    // measurement, distinct from the pflash suite's lossy-compressed cells
    // at the same token counts. Ladder starts at 32k (8k/16k smoke-level
    // sanity is covered by the 'smoke' suite) and runs to the user's
    // pie-in-the-sky 200k coding-context ceiling so long-context recall
    // fidelity is proven at the sizes that actually matter for coding
    // workloads, not just comfortable mid-range ones.
    //
    // 160k/200k retain both int8 and int4. Source #1197 qualification made
    // live quantization effective, so these rows validate the high-context
    // int4 slope instead of assuming it extrapolates from 32k/65k/131k.
    for (const dtype of ['bf16', 'int8', 'int4']) {
      for (const tokens of [32000, 65536, 131072]) cells.push(cell(model, `context-${tokens}-${dtype}`, tokens, configuration({ dtype, prefillStepSize: '512' })));
    }
    for (const dtype of ['int8', 'int4']) {
      for (const tokens of [160000, 200000]) cells.push(cell(model, `context-${tokens}-${dtype}`, tokens, configuration({ dtype, prefillStepSize: '512' })));
    }
  }
  // Explicit throughput/peak comparison above the harness's conservative
  // 512-token default. This suite is not a persistent-KV calibration lane.
  // Deliberately NOT folded into 'all': it's a one-time knob-tuning pass, not
  // part of the recurring model-comparison matrix. 131072 is included because
  // that's the exact token count the crash was observed at, so both values
  // must be proven safe there, not just faster/slower at a comfortable size.
  // Ladder extends to the user's pie-in-the-sky 200k coding-context ceiling
  // since that's the size that most needs to be proven crash-safe, not just
  // the historical crash point.
  if (suite === 'prefill') {
    for (const prefillStepSize of ['2048', '4096']) {
      for (const tokens of [32000, 65536, 131072, 160000, 200000]) {
        cells.push(cell(model, `prefill-${prefillStepSize}-${tokens}-int8`, tokens, configuration({ dtype: 'int8', prefillStepSize })));
      }
    }
  }
  // Direct counterpart to llama-cpp-benchmark-suite.mjs's UBATCH_SIZES x
  // CONTEXT_SIZES matrix (same 512/2048 values, same five context tiers, same
  // `ubatch-${step}-context-${tokens}` cell-id shape) so the two backends'
  // PP/TG rows land side by side in one report. Rapid-MLX has no batch-size
  // flag of its own; --prefill-step-size is the analogous per-chunk prompt-
  // processing width (see DEFAULT_PREFILL_STEP_SIZE above), so it stands in
  // for llama.cpp's -ub here. Deliberately NOT folded into 'all': this is a
  // one-off cross-backend comparison run, not part of the recurring matrix.
  if (suite === 'ubatch') {
    for (const prefillStepSize of ['512', '2048']) {
      for (const tokens of [32000, 65536, 131072, 160000, 200000]) {
        cells.push(cell(model, `ubatch-${prefillStepSize}-context-${tokens}`, tokens, configuration({ dtype: 'int8', prefillStepSize })));
      }
    }
  }
  // Baseline quant-effect pass at the prefill-step-size winner from the
  // calibration above (512 — lowest peak memory at every tier, competitive-
  // to-better PP throughput; see prefill-step-size comparison). dtype is
  // fixed at int8 rather than swept. The preceding 0.10.17 release had a
  // batch-path no-op (live KV remained bf16), but the 0.11.0 source build
  // under qualification includes the upstream live-KV fix. Keep the legacy
  // calibration interpretation version-scoped; cache cells intentionally
  // cover both int8 and int4 on the fixed source build.
  // TurboQuant and pflash are separate, real mechanisms (retained prefix-
  // cache compression and long-context KV compression respectively) and are
  // the two axes actually worth baselining until upstream fixes KV
  // quantization. Deliberately NOT folded into 'all' for the same reason as
  // 'prefill': a one-time calibration pass, not the recurring model matrix.
  if (suite === 'quant-baseline') {
    for (const turboquant of ['none', 'k8v4']) {
      for (const pflash of ['off', 'auto']) {
        for (const tokens of [32000, 65536, 131072, 160000, 200000]) {
          cells.push(cell(model, `quant-512-${turboquant}-${pflash}-${tokens}-int8`, tokens, configuration({ dtype: 'int8', prefillStepSize: '512', turboquant, pflash })));
        }
      }
    }
  }
  if (include('pflash')) {
    for (const tokens of [65536, 131072, 160000, 200000]) cells.push(cell(model, `pflash-${tokens}-int4`, tokens, configuration({ dtype: 'int4', pflash: 'auto' })));
  }
  if (include('cache')) {
    const capacitiesFor = (tokens) => tokens === 131072 ? [8192, 12288, 16384] : (tokens >= 160000 ? [8192, 16384] : [8192]);
    for (const tokens of [8000, 32000, 65536, 131072, 160000, 200000]) {
      for (const cacheMemoryMb of capacitiesFor(tokens)) {
        // Qwen's tokenizer measured this frozen pack at ~101,070 tokens.
        // Preserve source formatting while taking a proportional final slice;
        // the server receipt remains the authoritative rendered-token count.
        const targetTokens = tokens === 131072 ? 131000 : tokens;
        const estimatedPackTokens = 101070;
        const fullCopies = Math.floor(targetTokens / estimatedPackTokens);
        const remainder = targetTokens % estimatedPackTokens;
        const tailChars = Math.floor(workspacePack.corpus.length * (remainder / estimatedPackTokens));
        const referenceText = [
          ...Array.from({ length: fullCopies }, () => workspacePack.corpus),
          ...(tailChars > 0 ? [workspacePack.corpus.slice(0, tailChars)] : []),
        ].join('\n\n===== repeated workspace context =====\n\n');
        const repeats = fullCopies + (tailChars > 0 ? 1 : 0);
        for (const dtype of ['int8', 'int4']) {
          const cached = cell(model, `cache-${tokens}-ram-${cacheMemoryMb}-${dtype}`, tokens, configuration({ dtype, cache: true, cacheMemoryMb, diskCheckpointInterval: 0 }), {
            exact_extension_tokens: 512,
            reference_text: referenceText,
            workspace_fixture: { corpus_sha256: workspacePack.sha256, files: workspacePack.files, repetitions: repeats },
            instruction: 'You are working in this coding project. Review the supplied source and plans, identify the requested implementation boundary, and give a concise implementation plan with concrete files to change.',
          });
          cached.sequence = ['cold', 'repeat', 'followup', 'fork'];
          cells.push(cached);
        }
      }
    }
  }
  // TurboQuant only compresses the retained/reused prefix-cache snapshot, so
  // its effect can only be measured through the cold->repeat->extension
  // reuse sequence, not a single cold request (that's what the quant-baseline
  // suite's cold-only pass above could not show). This suite exercises that
  // sequence at step=512 (the prefill-step-size winner) across the same
  // context ladder used elsewhere, so results are directly comparable across
  // architectures on a per-model calibration point, per the breadth-over-
  // depth framework. Deliberately NOT folded into 'all': a one-time
  // calibration pass across architectures, not the recurring model matrix.
  if (suite === 'turboquant-scale') {
    for (const turboquant of ['none', 'k8v4']) {
      for (const tokens of [8000, 32000, 65536, 131072]) {
        const cached = cell(model, `turboquant-512-${turboquant}-${tokens}`, tokens, configuration({ dtype: 'int8', turboquant, cache: true, prefillStepSize: '512' }), { extension_words: 500 });
        cached.sequence = ['cold', 'repeat', 'extension'];
        cells.push(cached);
      }
    }
  }
  if (include('tools')) {
    for (const dtype of ['int8', 'int4']) {
      for (const toolChoice of ['required', 'auto']) {
        // TurboQuant only affects the retained/reused prefix-cache snapshot, so
        // its effect on tool-call fidelity (argument correctness, trace order)
        // can only surface in the repeat/extension phases below, not cold. This
        // mirrors the cache suite's none/k8v4 split rather than assuming
        // TurboQuant is safe for tool content the way dtype already was tested.
        for (const turboquant of ['none', 'k8v4']) {
          const toolsCell = cell(model, `tools-sequential-8k-${dtype}-${toolChoice}-${turboquant}`, 8000, configuration({ dtype, turboquant, cache: true, maxTokens: 32768 }), {
            // Primary coding-agent ceiling requested by the user: 32k output and
            // 16k reasoning (wider than the other suites' shared MAX_TOKENS/
            // REASONING_BUDGET default, since tool-call turns need more output
            // room than a short marker-list answer).
            max_tokens: 32768,
            instruction: 'Use read_file on src/example.ts. After seeing the file result, use apply_patch to replace the marked value.',
            markers: null,
            extension_words: 500,
            tools: toolDefinitions(),
            extra_body: { tool_choice: toolChoice, reasoning_max_tokens: 16000 },
            expected_tool_name: 'read_file',
            expected_tool_arguments: { path: 'src/example.ts' },
            tool_trace: { steps: [{
              tool_name: 'read_file',
              tool_result: 'export const MARKED_VALUE = "before";\n',
              expected_tool_name: 'apply_patch',
              expected_tool_arguments: { path: 'src/example.ts' },
            }] },
          });
          // The tools cell enables the prefix cache but previously only ever
          // ran a single cold phase, so the cache it configured was never
          // actually exercised. Mirror the cache suite's repeat/extension rows.
          toolsCell.sequence = ['cold', 'repeat', 'extension'];
          cells.push(toolsCell);
        }
      }
    }
  }
  if (include('image') && imagePath) {
    const imageFixture = await imageFixtureMetadata(imagePath);
    const imageWorkload = (overrides = {}) => ({
      image_path: imageFixture.source_path,
      image_fixture: imageFixture,
      // The visual question makes this a vision-fidelity probe, rather than a
      // text-retrieval request that happens to carry an ignored image.
      instruction: 'Identify the exact large product title in the top-left logo area of the screenshot, then list every CHECK_* constant name and numeric value in the reference below. Include the product title and output one CHECK_* NAME=VALUE per line.',
      expected_visual_terms: expectedVisualTerms,
      ...overrides,
    });
    const imageAgentReference = [
      'File: src/components/header.tsx',
      'export const brandTitle = "Lama Monitor";',
      'export function Header() {',
      '  return <h1>{brandTitle}</h1>;',
      '}',
    ].join('\n');
    const imageConfig = (dtype, maxTokens) => configuration({
      dtype,
      mllm: true,
      maxTokens,
      prefillStepSize: DEFAULT_PREFILL_STEP_SIZE,
      mllmPrefillStepSize,
    });
    // Rapid's current MLLM admission path rejects any rendered prompt wider
    // than this safe prefill step. Real image-plus-instruction work begins at
    // 1024: the selected screenshot alone consumed roughly 486 rendered
    // tokens, leaving no useful instruction headroom at 512. Therefore this
    // is a real vision-capability
    // smoke, not a false long-context image matrix. Long image context stays
    // refused until Rapid exposes a safe chunked-admission mechanism.
    cells.push({
      id: 'image-visual-smoke-int8',
      model,
      configuration: imageConfig('int8', 1024),
      sequence: ['cold'],
      workload: workload(8000, imageWorkload({
        target_words: 32,
        markers: [],
        max_tokens: 1024,
        extra_body: { reasoning_max_tokens: 512 },
      })),
    });
    // This is the representative image-use cell: ordinary agent-scale
    // output/reasoning, an actual screenshot, and a 1024-token MLLM admission
    // width. It is intentionally short only because current Rapid MLLM still
    // rejects rendered prompts wider than that width rather than chunking them.
    cells.push({
      id: 'image-agent-smoke-int8',
      model,
      configuration: imageConfig('int8', MAX_TOKENS),
      sequence: ['cold'],
      workload: workload(8000, imageWorkload({
        target_words: 32,
        markers: [],
        reference_text: imageAgentReference,
        instruction: 'A user pasted this UI screenshot and asks you to fix the typo in the code below so the visible product name and header match. Identify the large top-left product title in the screenshot, then return a concise unified diff that fixes the code. Do not speculate or repeatedly re-read the prompt.',
        max_tokens: MAX_TOKENS,
        expected_content_terms: ['brandTitle', 'Llama Monitor'],
        extra_body: { reasoning_max_tokens: REASONING_BUDGET },
      })),
    });
  }
  if (suite === 'image' && !imagePath) die('The image suite requires --image PATH.');
  if (suite === 'image' && !expectedVisualTerms.length) {
    die('The image suite requires one or more --expected-visual-term values for the tracked fixture.');
  }
  return cells;
}

function extractInfoPair(line) {
  const clean = line.replace(/^│/, '').replace(/│$/, '').trim();
  const colonIndex = clean.indexOf(':');
  if (colonIndex === -1) return null;
  const key = clean.slice(0, colonIndex).trim();
  const value = clean.slice(colonIndex + 1).trim();
  if (!key || !value) return null;
  return [key, value];
}

// Mirrors src/inference/rapid_mlx/info_query.rs::parse_model_profile so the
// suite driver's argv derivation stays in lockstep with the Rust runtime's
// own `rapid-mlx info` field parsing instead of a hand-pinned model list.
async function profileOverridesFor(model, rapidMlxBin) {
  const result = spawnSync(rapidMlxBin, ['info', model], { encoding: 'utf8', timeout: 30000, maxBuffer: 1024 * 1024 });
  if (result.status !== 0 || !result.stdout || result.stdout.includes('Error:') || !result.stdout.trim()) return {};
  const profile = {};
  for (const rawLine of result.stdout.split('\n')) {
    const trimmed = rawLine.trim();
    if (!trimmed || !trimmed.includes(':')) continue;
    if (!(trimmed.startsWith('##') || (trimmed.startsWith('│') && trimmed.includes('│')))) continue;
    const pair = extractInfoPair(trimmed);
    if (!pair) continue;
    const [key, value] = pair;
    const keyNormalized = key.toLowerCase().replace(/[ _-]/g, '');
    if (keyNormalized === 'toolformat' || keyNormalized === 'tool') profile.tool_call_parser = value;
    else if (keyNormalized === 'reasoningparser' || keyNormalized === 'reasoning') profile.reasoning_parser = value;
    else if (keyNormalized === 'architecture' || keyNormalized === 'arch') profile.architecture = value;
  }
  return profile;
}

function installedRapidMlxVersion(rapidMlxBin) {
  const result = spawnSync(rapidMlxBin, ['--version'], { encoding: 'utf8', timeout: 30000 });
  const version = `${result.stdout ?? ''}\n${result.stderr ?? ''}`.trim();
  if (result.status !== 0 || !version) die('Could not record `rapid-mlx --version`; a versioned receipt is required.');
  return version;
}

// prefill-step-size is the per-chunk prompt-processing width. It is NOT a
// throughput-only knob: full-attention layers materialize a [heads, q_len,
// k_len] score buffer per prefill chunk, and q_len == prefill-step-size. On
// Apple Silicon that buffer must fit under Metal's single-buffer ceiling
// (max_buffer_length, ~41.7 GB on M5 Max). Empirically the crash condition is
//   prefill_step * context_tokens * num_heads * dtype_bytes > max_buffer_length
// so the previous values (32768, and 150000 for pflash=auto) silently crashed
// any context above ~40k with a swallowed Metal OOM (clean 200, zero tokens).
// 512 is the measurement and launch default: it keeps transient prefill
// allocation from contaminating persistent-KV accounting. Higher values are
// exercised only by the explicit prefill/ubatch throughput suites.
const DEFAULT_PREFILL_STEP_SIZE = '512';

function argvFor(model, config, port, utilization, servedModelName = null, profile = {}) {
  const cache = config.prefix_cache.startsWith('enabled');
  const prefillStepSize = config.mllm
    ? config.mllm_prefill_step_size
    : (config.prefill_step_size ?? DEFAULT_PREFILL_STEP_SIZE);
  return ['--no-telemetry', 'serve', model, ...(servedModelName ? ['--served-model-name', servedModelName] : []), '--port', String(port), '--host', '127.0.0.1', '--max-num-seqs', '1', '--max-concurrent-requests', '1', cache ? '--enable-prefix-cache' : '--disable-prefix-cache', ...(cache ? ['--cache-memory-mb', String(config.cache_memory_mb ?? 8192), '--hybrid-cache-entries', String(config.hybrid_cache_entries ?? 16)] : []), '--kv-disk-checkpoint-interval', String(config.disk_checkpoint_interval ?? 0), '--pflash', config.pflash, ...(config.pflash === 'auto' ? ['--pflash-threshold', '32768'] : []), '--prefill-step-size', String(prefillStepSize), '--max-tokens', String(config.server_max_tokens ?? 128), '--kv-cache-dtype', config.kv_cache_dtype_requested, '--kv-cache-turboquant', config.turboquant_requested, config.mllm ? '--mllm' : '--no-mllm', '--gpu-memory-utilization', String(utilization), ...(profile.tool_call_parser ? ['--tool-call-parser', profile.tool_call_parser, '--enable-auto-tool-choice'] : []), ...(profile.reasoning_parser ? ['--reasoning-parser', profile.reasoning_parser] : []), ...(profile.force_hybrid ? ['--force-hybrid'] : []), '--log-level', 'INFO'];
}

async function localModelIdentity(model, requestedRevision) {
  const modelPath = join(process.env.HF_HOME ?? join(process.env.HOME ?? '', '.cache/huggingface'), 'hub', `${MODEL_PREFIX}${model.replace('/', '--')}`, 'snapshots');
  const snapshots = await readdir(modelPath);
  const revision = requestedRevision ?? snapshots.sort().at(-1);
  if (!revision) die(`No local snapshot found for ${model}. Run rapid-mlx pull first.`);
  const config = await readFile(join(modelPath, revision, 'config.json'));
  return { revision, snapshot_path: join(modelPath, revision), config_sha256: createHash('sha256').update(config).digest('hex') };
}

async function patchChatTemplateInSnapshot(identity, templatePath) {
  if (!templatePath) return { metadata: null, restore: async () => {} };
  const template = await readFile(resolve(templatePath));
  const snapshotConfigPath = join(identity.snapshot_path, 'tokenizer_config.json');
  const originalConfig = await readFile(snapshotConfigPath);
  const tokenizerConfig = JSON.parse(originalConfig.toString('utf8'));
  tokenizerConfig.chat_template = template.toString('utf8');
  const replacements = [
    { path: snapshotConfigPath, contents: Buffer.from(`${JSON.stringify(tokenizerConfig, null, 2)}\n`) },
    // Standalone files take precedence over tokenizer_config.json in this
    // Rapid-MLX/Transformers stack, so patch the matching default as well.
    { path: join(identity.snapshot_path, 'chat_template.jinja'), contents: template },
  ];
  const originals = await Promise.all(replacements.map(async (replacement) => ({
    ...replacement,
    original: await readFile(replacement.path),
    originalLink: await readlink(replacement.path).catch((error) => {
      if (error.code === 'EINVAL') return null;
      throw error;
    }),
    backupPath: `${replacement.path}.rapid-mlx-benchmark-backup`,
  })));

  // Rapid-MLX has no --chat-template flag, and an overlay path disables its
  // repo-alias tool-parser detection. Temporarily replace the snapshot entry
  // itself. The original bytes are kept beside it for crash recovery and the
  // exact symlinks are restored in the caller's finally block.
  for (const item of originals) await writeFile(item.backupPath, item.original, { flag: 'wx' });
  for (const item of originals) {
    if (item.originalLink !== null) await unlink(item.path);
    await writeFile(item.path, item.contents);
  }
  return {
    metadata: {
      mode: 'temporary snapshot tokenizer_config.json replacement; exact original restored after run',
      source_path: resolve(templatePath),
      sha256: createHash('sha256').update(template).digest('hex'),
      targets: ['tokenizer_config.json:chat_template', 'chat_template.jinja'],
    },
    restore: async () => {
      for (const item of originals) {
        await unlink(item.path);
        if (item.originalLink !== null) await symlink(item.originalLink, item.path);
        else await writeFile(item.path, item.original);
        await rm(item.backupPath, { force: true });
      }
    },
  };
}

function manifestFor(model, identity, runtimeVersion, rapidMlxBin, config, cells, port, utilization, serverModelPath, chatTemplate, profile = {}, tokenizerPython = null) {
  const argv = [rapidMlxBin, ...argvFor(serverModelPath ?? model, config, port, utilization, model, profile)];
  return {
    schema_version: 1,
    benchmark: { name: `Rapid-MLX generated ${cells[0].id}`, purpose: 'Generated by rapid-mlx-benchmark-suite.mjs; do not hand-edit.' },
    runtime: { backend: 'rapid-mlx', version: runtimeVersion, base_url: `http://127.0.0.1:${port}`, health_path: '/health', metrics_path: '/metrics', fresh_server_per_cell: true, exact_argv: argv, ...(tokenizerPython ? { tokenizer_python: tokenizerPython } : {}) },
    hardware: { cpu: 'recorded by operator', unified_memory_bytes: null },
    model: { hf_repo_id: model, revision: identity.revision, config_sha256: identity.config_sha256, tokenizer_snapshot_path: identity.snapshot_path, profile_overrides: profile, ...(chatTemplate ? { chat_template_override: chatTemplate } : {}) },
    cells,
  };
}

function runProcess(command, args, options = {}) {
  return new Promise((resolvePromise, reject) => {
    const child = spawn(command, args, { stdio: options.stdio ?? 'inherit', cwd: options.cwd });
    child.once('error', reject);
    child.once('exit', (code, signal) => code === 0 ? resolvePromise() : reject(new Error(`${command} exited ${code ?? signal}`)));
  });
}

function launchServer(command, args, env) {
  const output = [];
  const child = spawn(command, args, { stdio: ['ignore', 'pipe', 'pipe'], env });
  const append = (chunk) => {
    output.push(chunk.toString());
    while (output.join('').length > 12000) output.shift();
  };
  child.stdout.on('data', append);
  child.stderr.on('data', append);
  return { child, logs: () => output.join('') };
}

async function waitForHealth(baseUrl, processHandle) {
  const deadline = Date.now() + 180000;
  while (Date.now() < deadline) {
    if (processHandle.exitCode !== null) die('Rapid-MLX exited before becoming healthy.');
    try {
      const response = await fetch(`${baseUrl}/health`);
      if (response.ok && (await response.json()).ready) return;
    } catch { /* model is still loading */ }
    await new Promise((resolvePromise) => setTimeout(resolvePromise, 500));
  }
  die('Timed out waiting for Rapid-MLX health endpoint.');
}

async function stopServer(server) {
  if (server.exitCode !== null) return;
  const exited = new Promise((resolvePromise) => server.once('exit', resolvePromise));
  server.kill('SIGTERM');
  const graceful = await Promise.race([
    exited.then(() => true),
    new Promise((resolvePromise) => setTimeout(() => resolvePromise(false), 20000)),
  ]);
  if (!graceful && server.exitCode === null) {
    server.kill('SIGKILL');
    await exited;
  }
  // MLX/Metal allocations and macOS file-backed model pages can lag process
  // exit. Starting the next fresh cell immediately makes Rapid's own pressure
  // guard mistake teardown residue for a second live model.
  await new Promise((resolvePromise) => setTimeout(resolvePromise, 5000));
}

async function runSuite(options, manifests, tempDir) {
  const outputDir = resolve(options.out);
  await mkdir(outputDir, { recursive: true });
  const receipts = [];
  let failure = null;
  for (const [index, { manifest, label }] of manifests.entries()) {
    const manifestPath = join(tempDir, `${String(index).padStart(2, '0')}-${label}.json`);
    const receiptPath = join(outputDir, `${String(index).padStart(2, '0')}-${label}.json`);
    if (options.resume) {
      try {
        await readFile(receiptPath);
        receipts.push({ label, receipt: basename(receiptPath), manifest: null, resumed: true });
        continue;
      } catch { /* no durable receipt for this cell */ }
    }
    await writeFile(manifestPath, `${JSON.stringify(manifest, null, 2)}\n`);
    const [command, ...args] = manifest.runtime.exact_argv;
    const cacheHome = join(tempDir, 'cache-homes', label);
    await mkdir(cacheHome, { recursive: true });
    const hfHome = process.env.HF_HOME ?? join(process.env.HOME ?? '', '.cache', 'huggingface');
    const started = launchServer(command, args, { ...process.env, HOME: cacheHome, HF_HOME: hfHome });
    const server = started.child;
    try {
      await waitForHealth(manifest.runtime.base_url, server);
      await runProcess(process.execPath, [resolve('scripts/model-runtime-benchmark.mjs'), 'run', '--manifest', manifestPath, '--out', receiptPath, '--server-pid', String(server.pid)], { cwd: process.cwd() });
      receipts.push({ label, receipt: basename(receiptPath), manifest: options.keepManifests ? manifestPath : null });
    } catch (error) {
      failure = new Error(`${error.message}\nRapid-MLX log tail for ${label}:\n${started.logs()}`);
    } finally {
      await stopServer(server);
    }
    if (failure) break;
  }
  await writeFile(join(outputDir, 'suite-index.json'), `${JSON.stringify({
    model: options.model,
    suite: options.suite,
    receipts,
    complete: failure === null,
    failure: failure?.message ?? null,
  }, null, 2)}\n`);
  if (failure) throw failure;
}

const { command, options } = parseArgs(process.argv);
const rapidMlxBin = options.rapidMlxBin ?? 'rapid-mlx';
const identity = await localModelIdentity(options.model, options.revision);
const allCells = await suiteCells(options.model, options.suite, options.image, options.expectedVisualTerms, options.mllmPrefillStepSize ?? 1024, options.workspacePack);
let cells = options.cells
  ? allCells.filter((cell) => options.cells.includes(cell.id))
  : allCells;
if (options.cacheContexts || options.cacheMemoryMb || options.cacheDtypes || options.cacheDiskCheckpointIntervals) {
  if (options.suite !== 'cache') die('cache selectors require --suite cache.');
  cells = cells.filter((cell) => (
    (!options.cacheContexts || options.cacheContexts.includes(cell.target_tokens))
    && (!options.cacheMemoryMb || options.cacheMemoryMb.includes(cell.configuration.cache_memory_mb))
    && (!options.cacheDtypes || options.cacheDtypes.includes(cell.configuration.kv_cache_dtype_requested))
  ));
}
if (options.cacheDiskCheckpointIntervals) {
  cells = cells.flatMap((cell) => options.cacheDiskCheckpointIntervals.map((interval) => ({
    ...cell,
    id: `${cell.id}-checkpoint-${interval}`,
    configuration: { ...cell.configuration, disk_checkpoint_interval: interval },
  })));
}
if (!cells.length) die(`No cells selected for ${options.suite}; check --cell values.`);
if (options.cells) {
  const known = new Set(allCells.map((cell) => cell.id));
  const unknown = options.cells.filter((cell) => !known.has(cell));
  if (unknown.length) die(`Unknown cells for ${options.suite}: ${unknown.join(', ')}`);
}
const profile = await profileOverridesFor(options.model, rapidMlxBin);
if (options.forceHybrid) profile.force_hybrid = true;
const runtimeVersion = installedRapidMlxVersion(rapidMlxBin);

if (command === 'plan') {
  const manifests = cells.map((item, index) => ({
    label: `${String(index).padStart(2, '0')}-${item.id}`,
    manifest: manifestFor(options.model, identity, runtimeVersion, rapidMlxBin, item.configuration, [item], options.port, options.utilization, null, options.chatTemplate ? { source_path: resolve(options.chatTemplate), note: 'materialized as an isolated overlay at run time' } : null, profile, options.tokenizerPython ?? null),
  }));
  process.stdout.write(`${JSON.stringify({ model: options.model, identity, suite: options.suite, manifests: manifests.map(({ label, manifest }) => ({ label, cells: manifest.cells.map((item) => item.id), argv: manifest.runtime.exact_argv })) }, null, 2)}\n`);
} else {
  const tempDir = await mkdtemp(join(tmpdir(), 'rapid-mlx-benchmark-suite-'));
  try {
    const template = await patchChatTemplateInSnapshot(identity, options.chatTemplate);
    const manifests = cells.map((item, index) => ({
      label: `${String(index).padStart(2, '0')}-${item.id}`,
      manifest: manifestFor(options.model, identity, runtimeVersion, rapidMlxBin, item.configuration, [item], options.port, options.utilization, template.modelPath, template.metadata, profile, options.tokenizerPython ?? null),
    }));
    try { await runSuite(options, manifests, tempDir); } finally { await template.restore(); }
  } finally { if (!options.keepManifests) await rm(tempDir, { recursive: true, force: true }); }
}
