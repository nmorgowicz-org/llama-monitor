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

function die(message) { throw new Error(message); }

function parseArgs(argv) {
  const [command, ...rest] = argv.slice(2);
  const options = { suite: 'smoke', port: DEFAULT_PORT, utilization: DEFAULT_UTILIZATION };
  for (let index = 0; index < rest.length; index += 1) {
    const key = rest[index];
    if (!key.startsWith('--')) die(`Invalid argument: ${key}`);
    if (key === '--keep-manifests') { options.keepManifests = true; continue; }
    if (key === '--resume') { options.resume = true; continue; }
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

function configuration({ dtype = 'int8', pflash = 'off', turboquant = 'none', cache = false, maxTokens = MAX_TOKENS, mllm = false, prefillStepSize = DEFAULT_PREFILL_STEP_SIZE }) {
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
    prefix_cache: cache ? 'enabled; cache-memory-mb=4096; hybrid-cache-entries=4' : 'disabled',
    pflash,
    server_max_tokens: maxTokens,
    concurrency: 1,
    mllm,
    prefill_step_size: prefillStepSize,
    expected_metrics: expectedMetrics,
  };
}

function cell(model, id, tokens, config, overrides = {}) {
  return { id, model, configuration: config, sequence: ['cold'], workload: workload(tokens, overrides) };
}

function suiteCells(model, suite, imagePath) {
  const include = (name) => suite === 'all' || suite === name;
  const cells = [];
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
    // 160k/200k are int8-only, not swept across both dtypes: KV-cache RAM at
    // a given dtype is a direct linear multiply of context length, so it
    // extrapolates cleanly from the 32k/65k/131k int4 points without needing
    // a second full run up there. PP/prefill throughput and crash-safety do
    // NOT extrapolate the same way (prefill cost grows close to quadratically
    // with context via the chunked-attention buffer, which is the exact
    // mechanism behind the original 131k crash), so those top two tiers are
    // still measured directly, just under one dtype (int8) as the spot-check
    // rather than the full matrix.
    for (const dtype of ['bf16', 'int8', 'int4']) {
      for (const tokens of [32000, 65536, 131072]) cells.push(cell(model, `context-${tokens}-${dtype}`, tokens, configuration({ dtype, prefillStepSize: '512' })));
    }
    for (const tokens of [160000, 200000]) cells.push(cell(model, `context-${tokens}-int8`, tokens, configuration({ dtype: 'int8', prefillStepSize: '512' })));
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
  // fixed at int8 rather than swept: rapid-mlx's --kv-cache-dtype is
  // currently a batch-path no-op (live KV cache stays bf16 regardless of the
  // requested dtype), so int4 would not exercise a different code path here.
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
    for (const turboquant of ['none', 'k8v4']) {
      const cached = cell(model, `cache-8k-${turboquant}`, 8000, configuration({ dtype: 'int8', turboquant, cache: true }), { extension_words: 500 });
      cached.sequence = ['cold', 'repeat', 'extension'];
      cells.push(cached);
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
    cells.push(cell(model, 'image-smoke-8k', 8000, configuration({ dtype: 'int8', mllm: true }), { image_path: imagePath }));
  }
  if (suite === 'image' && !imagePath) die('The image suite requires --image PATH.');
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
  return ['--no-telemetry', 'serve', model, ...(servedModelName ? ['--served-model-name', servedModelName] : []), '--port', String(port), '--host', '127.0.0.1', '--max-num-seqs', '1', '--max-concurrent-requests', '1', cache ? '--enable-prefix-cache' : '--disable-prefix-cache', ...(cache ? ['--cache-memory-mb', '4096', '--hybrid-cache-entries', '4'] : []), '--pflash', config.pflash, ...(config.pflash === 'auto' ? ['--pflash-threshold', '32768'] : []), '--prefill-step-size', config.prefill_step_size ?? DEFAULT_PREFILL_STEP_SIZE, '--max-tokens', String(config.server_max_tokens ?? 128), '--kv-cache-dtype', config.kv_cache_dtype_requested, '--kv-cache-turboquant', config.turboquant_requested, config.mllm ? '--mllm' : '--no-mllm', '--gpu-memory-utilization', String(utilization), ...(profile.tool_call_parser ? ['--tool-call-parser', profile.tool_call_parser, '--enable-auto-tool-choice'] : []), ...(profile.reasoning_parser ? ['--reasoning-parser', profile.reasoning_parser] : []), '--log-level', 'INFO'];
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

function manifestFor(model, identity, runtimeVersion, rapidMlxBin, config, cells, port, utilization, serverModelPath, chatTemplate, profile = {}) {
  const argv = [rapidMlxBin, ...argvFor(serverModelPath ?? model, config, port, utilization, model, profile)];
  return {
    schema_version: 1,
    benchmark: { name: `Rapid-MLX generated ${cells[0].id}`, purpose: 'Generated by rapid-mlx-benchmark-suite.mjs; do not hand-edit.' },
    runtime: { backend: 'rapid-mlx', version: runtimeVersion, base_url: `http://127.0.0.1:${port}`, health_path: '/health', metrics_path: '/metrics', fresh_server_per_cell: true, exact_argv: argv },
    hardware: { cpu: 'recorded by operator', unified_memory_bytes: null },
    model: { hf_repo_id: model, revision: identity.revision, config_sha256: identity.config_sha256, profile_overrides: profile, ...(chatTemplate ? { chat_template_override: chatTemplate } : {}) },
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
const allCells = suiteCells(options.model, options.suite, options.image);
const cells = options.cells
  ? allCells.filter((cell) => options.cells.includes(cell.id))
  : allCells;
if (!cells.length) die(`No cells selected for ${options.suite}; check --cell values.`);
if (options.cells) {
  const known = new Set(allCells.map((cell) => cell.id));
  const unknown = options.cells.filter((cell) => !known.has(cell));
  if (unknown.length) die(`Unknown cells for ${options.suite}: ${unknown.join(', ')}`);
}
const profile = await profileOverridesFor(options.model, rapidMlxBin);
const runtimeVersion = installedRapidMlxVersion(rapidMlxBin);

if (command === 'plan') {
  const manifests = cells.map((item, index) => ({
    label: `${String(index).padStart(2, '0')}-${item.id}`,
    manifest: manifestFor(options.model, identity, runtimeVersion, rapidMlxBin, item.configuration, [item], options.port, options.utilization, null, options.chatTemplate ? { source_path: resolve(options.chatTemplate), note: 'materialized as an isolated overlay at run time' } : null, profile),
  }));
  process.stdout.write(`${JSON.stringify({ model: options.model, identity, suite: options.suite, manifests: manifests.map(({ label, manifest }) => ({ label, cells: manifest.cells.map((item) => item.id), argv: manifest.runtime.exact_argv })) }, null, 2)}\n`);
} else {
  const tempDir = await mkdtemp(join(tmpdir(), 'rapid-mlx-benchmark-suite-'));
  try {
    const template = await patchChatTemplateInSnapshot(identity, options.chatTemplate);
    const manifests = cells.map((item, index) => ({
      label: `${String(index).padStart(2, '0')}-${item.id}`,
      manifest: manifestFor(options.model, identity, runtimeVersion, rapidMlxBin, item.configuration, [item], options.port, options.utilization, template.modelPath, template.metadata, profile),
    }));
    try { await runSuite(options, manifests, tempDir); } finally { await template.restore(); }
  } finally { if (!options.keepManifests) await rm(tempDir, { recursive: true, force: true }); }
}
