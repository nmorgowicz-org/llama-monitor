#!/usr/bin/env node
/**
 * Human-friendly Rapid-MLX matrix driver.
 *
 * It materializes evidence-pinned manifests internally, starts one fresh
 * server per configuration, delegates individual requests to the generic
 * OpenAI-compatible runner, and writes an index of the resulting receipts.
 * This deliberately keeps the generic runner reusable for llama.cpp/OMLX.
 */
import { createReadStream, createWriteStream } from 'node:fs';
import { mkdir, mkdtemp, open, readFile, readdir, readlink, realpath, rm, stat, symlink, unlink, writeFile } from 'node:fs/promises';
import { spawn, spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { tmpdir } from 'node:os';
import { basename, isAbsolute, join, relative, resolve } from 'node:path';

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
// This is only the requested ceiling, not proof that K=2 executes. It is the
// smallest useful clamp probe above K=1 for Qwen3.5/3.6 and matches the common
// vLLM/llama.cpp recommendation operators are likely to compare against.
// Qualification still needs explicit K=1 and requested-K=2 cells; the observed
// K histogram, not this value, is authoritative.
const DEFAULT_SPECULATIVE_TOKENS = 2;
// Set to catch a dead draft head (the stale-extractor failure mode measured
// ~0%), not to assert a performance target. Observed healthy acceptance on
// this family is 59-97%, so the floor sits far below any real result.
const DEFAULT_CONTROL_ACCEPT_FLOOR = 0.3;
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
    if (key === '--disable-speculative-auto-k') { options.disableSpeculativeAutoK = true; continue; }
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
    else if (key === '--speculative-model') options.speculativeModel = value;
    else if (key === '--speculative-control-model') options.speculativeControlModel = value;
    else if (key === '--speculative-tokens') options.speculativeTokens = Number(value);
    else if (key === '--speculative-methods') options.speculativeMethods = value.split(',');
    else if (key === '--speculative-workloads') options.speculativeWorkloads = value.split(',');
    // `rapid-mlx info` only recognizes HF-repo-alias models; a plain local
    // --model directory (e.g. an MTP sidecar diagnostic path) comes back
    // "(none)"/"(none)" from profileOverridesFor, so these let the operator
    // supply the same values the model's repo alias would have produced.
    else if (key === '--tool-call-parser') options.toolCallParser = value;
    else if (key === '--reasoning-parser') options.reasoningParser = value;
    else if (key === '--spec-decode-lane') options.specDecodeLane = value;
    else if (key === '--trials') options.trials = Number(value);
    else if (key === '--settle-seconds') options.settleSeconds = Number(value);
    else if (key === '--control-accept-floor') options.controlAcceptFloor = Number(value);
    else die(`Unknown option: ${key}`);
  }
  if (!['plan', 'run'].includes(command)) die('Use plan or run.');
  if (!options.model) die('--model is required.');
  if (command === 'run' && !options.out) die('run requires --out RECEIPT_DIRECTORY.');
  if (options.trials !== undefined && (!Number.isInteger(options.trials) || options.trials < 1)) {
    die('--trials must be an integer >= 1.');
  }
  if (options.settleSeconds !== undefined && (!Number.isFinite(options.settleSeconds) || options.settleSeconds < 0)) {
    die('--settle-seconds must be a non-negative number.');
  }
  if (!['smoke', 'context', 'pflash', 'cache', 'tools', 'image', 'prefill', 'quant-baseline', 'turboquant-scale', 'ubatch', 'spec-decode', 'spec-decode-warm', 'all'].includes(options.suite)) {
    die(`Unknown suite: ${options.suite}`);
  }
  if (options.suite.startsWith('spec-decode')) {
    if (!options.speculativeModel && !options.speculativeControlModel) {
      die('Spec-decode suites require --speculative-model and/or --speculative-control-model; the trunk directory is never used as an implicit sidecar.');
    }
    const speculativeTokens = options.speculativeTokens ?? DEFAULT_SPECULATIVE_TOKENS;
    if (!Number.isInteger(speculativeTokens) || speculativeTokens < 1) {
      die('--speculative-tokens must be an integer >= 1. This is a configured ceiling; the observed K histogram is authoritative.');
    }
    const unknownWorkloads = (options.speculativeWorkloads ?? ['code']).filter((name) => !['code', 'prose'].includes(name));
    if (unknownWorkloads.length) die(`Unknown speculative workload(s): ${unknownWorkloads.join(', ')}; use code and/or prose.`);
    const unknownMethods = (options.speculativeMethods ?? ['mtp']).filter((name) => name !== 'mtp');
    if (unknownMethods.length) die(`This sidecar-qualified suite currently supports only method=mtp; got ${unknownMethods.join(', ')}.`);
    // Deliberately has no default. The driver used to pass --force-spec-decode
    // unconditionally, so every existing spec-decode number came from the
    // forced lane without ever saying so. Making the lane explicit is what
    // stops a research override from silently becoming a qualification result.
    if (!['forced', 'natural'].includes(options.specDecodeLane)) {
      die('Spec-decode suites require --spec-decode-lane forced|natural. "forced" passes --force-spec-decode, overriding the profile\'s supports_spec_decode verdict; results from it are research evidence and can never gate enablement. "natural" lets Rapid apply its own eligibility rules.');
    }
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
  speculative = null,
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
  const speculativeMethod = speculative?.method ?? 'off';
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
    // 'off' omits --speculative-config entirely (baseline control row).
    // Non-off methods require an explicit external sidecar file/directory/repo.
    // Never fall back to the trunk directory: an in-dir model-mtp.safetensors
    // can be globbed as a trunk shard by mlx-lm and corrupt model loading.
    speculative_method: speculativeMethod,
    speculative_model: speculativeMethod === 'off' ? null : (speculative?.model ?? null),
    speculative_role: speculativeMethod === 'off' ? 'baseline' : (speculative?.role ?? 'subject'),
    // Which eligibility lane produced this cell. 'forced' passes
    // --force-spec-decode, overriding the profile's own supports_spec_decode
    // verdict; that is a legitimate research probe and an illegitimate basis
    // for enabling anything. 'natural' lets Rapid decide. Stamped per run
    // after cell construction; see the --spec-decode-lane option.
    speculative_lane: null,
    speculative_workload: speculative?.workload ?? null,
    num_speculative_tokens: speculativeMethod === 'off' ? null : (speculative?.numSpeculativeTokens ?? DEFAULT_SPECULATIVE_TOKENS),
    speculative_disable_auto_k: speculativeMethod === 'off' ? null : Boolean(speculative?.disableAutoK),
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

async function suiteCells(model, suite, imagePath, expectedVisualTerms = [], mllmPrefillStepSize = 1024, workspacePackPath = 'tests/fixtures/calibration/workspace-cache/project-pack.json', speculativeModel = null, speculativeControlModel = null, speculativeTokens = DEFAULT_SPECULATIVE_TOKENS, speculativeMethods = ['mtp'], speculativeWorkloads = ['code'], disableSpeculativeAutoK = false) {
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
  // Diagnostic pass for spec-decode/MTP draft acceptance: an 'off' control
  // row plus one row per requested --speculative-methods entry, at each
  // context tier, so accept-ratio/tokens-saved/park-rate (captured
  // automatically via metrics_delta in model-runtime-benchmark.mjs) can be
  // compared against a same-config, non-speculative baseline instead of a
  // bare pass/fail read of a single run. Fixed at dtype int8 and the
  // prefill-step-size winner (512): this suite isolates the spec-decode
  // variable, not a KV-dtype or prefill sweep. Deliberately NOT folded into
  // 'all' for the same reason as 'prefill'/'quant-baseline': a one-time
  // diagnostic pass, not the recurring model matrix.
  if (suite === 'spec-decode') {
    const workloadProfiles = {
      code: {
        suffix: '',
        overrides: {
          corpus: 'code',
          markers: null,
          instruction: 'Continue the TypeScript reference with additional typed validation functions. Output TypeScript code only and continue until the token budget is exhausted.',
          max_tokens: 512,
          minimum_completion_tokens: 256,
          extra_body: { chat_template_kwargs: { enable_thinking: false } },
        },
      },
      prose: {
        suffix: '-prose',
        overrides: {
          corpus: 'prose',
          markers: null,
          instruction: 'Write a coherent technical essay about reliable local inference systems. Use the reference for themes, do not quote it, and continue until the token budget is exhausted.',
          max_tokens: 512,
          minimum_completion_tokens: 256,
          extra_body: { chat_template_kwargs: { enable_thinking: false } },
        },
      },
    };
    const sidecars = [
      ...(speculativeControlModel ? [{ role: 'control', model: speculativeControlModel }] : []),
      ...(speculativeModel && speculativeModel !== speculativeControlModel ? [{ role: 'subject', model: speculativeModel }] : []),
    ];
    for (const workloadName of speculativeWorkloads) {
      const profile = workloadProfiles[workloadName];
      for (const tokens of [8000, 32000, 65536, 131072]) {
        cells.push(cell(model, `spec-off${profile.suffix}-${tokens}-int8`, tokens, configuration({
          dtype: 'int8',
          prefillStepSize: '512',
          maxTokens: profile.overrides.max_tokens,
          speculative: { method: 'off', workload: workloadName },
        }), profile.overrides));
        for (const { role, model: sidecarModel } of sidecars) {
          for (const method of speculativeMethods) {
            cells.push(cell(model, `spec-${role}-${method}${profile.suffix}-${tokens}-int8`, tokens, configuration({
              dtype: 'int8',
              prefillStepSize: '512',
              maxTokens: profile.overrides.max_tokens,
              speculative: {
                method,
                model: sidecarModel,
                role,
                workload: workloadName,
                numSpeculativeTokens: speculativeTokens,
                disableAutoK: disableSpeculativeAutoK,
              },
            }), profile.overrides));
          }
        }
      }
    }
  }
  // Warm, code-pattern control for the cold spec-decode cells above: those
  // only ever probe a fresh session, but the acceptance rates the operator
  // actually observes come from warm, multi-turn coding sessions (persisted
  // server, repeated tool-call rounds), not an isolated single-shot request.
  // Reuses the 'tools' suite's cold->repeat->extension sequence and
  // read_file/apply_patch tool_trace so each phase exercises a genuine warm
  // prefix-cache + tool-call round-trip. One off/mtp pair at the tools
  // suite's own 8k tier is enough to discriminate benchmark-methodology from
  // genuine model/draft-quality causes; this is a diagnostic control, not a
  // sweep. Its own suite name so it can be re-run in isolation from the full
  // (slow, already-run) cold spec-decode sweep above.
  if (suite === 'spec-decode' || suite === 'spec-decode-warm') {
    const specToolWorkload = (overrides = {}) => ({
      max_tokens: 4096,
      instruction: 'Use read_file on src/example.ts. After seeing the file result, use apply_patch to replace the marked value.',
      markers: null,
      extension_words: 500,
      tools: toolDefinitions(),
      extra_body: { tool_choice: 'required', reasoning_max_tokens: 2048 },
      expected_tool_name: 'read_file',
      expected_tool_arguments: { path: 'src/example.ts' },
      tool_trace: { steps: [{
        tool_name: 'read_file',
        tool_result: 'export const MARKED_VALUE = "before";\n',
        expected_tool_name: 'apply_patch',
        expected_tool_arguments: { path: 'src/example.ts' },
      }] },
      ...overrides,
    });
    const specWarmOff = cell(model, 'spec-warm-off-8000-int8', 8000, configuration({ dtype: 'int8', prefillStepSize: '512', cache: true, maxTokens: 4096, speculative: { method: 'off', workload: 'tools' } }), specToolWorkload());
    specWarmOff.sequence = ['cold', 'repeat', 'extension'];
    cells.push(specWarmOff);
    const sidecars = [
      ...(speculativeControlModel ? [{ role: 'control', model: speculativeControlModel }] : []),
      ...(speculativeModel && speculativeModel !== speculativeControlModel ? [{ role: 'subject', model: speculativeModel }] : []),
    ];
    for (const { role, model: sidecarModel } of sidecars) {
      for (const method of speculativeMethods) {
        const specWarm = cell(model, `spec-warm-${role}-${method}-8000-int8`, 8000, configuration({
          dtype: 'int8',
          prefillStepSize: '512',
          cache: true,
          maxTokens: 4096,
          speculative: {
            method,
            model: sidecarModel,
            role,
            workload: 'tools',
            numSpeculativeTokens: speculativeTokens,
            disableAutoK: disableSpeculativeAutoK,
          },
        }), specToolWorkload());
        specWarm.sequence = ['cold', 'repeat', 'extension'];
        cells.push(specWarm);
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
  // The sidecar's "model" field must be an explicit path/repo id: the
  // injector refuses to guess a random-init head from method name alone
  // (see vllm_mlx/spec_decode/mtp/qwen3_5_inject.py's mtp_sidecar contract).
  const speculativeConfig = config.speculative_method && config.speculative_method !== 'off'
    ? JSON.stringify({
      method: config.speculative_method,
      model: config.speculative_model,
      num_speculative_tokens: config.num_speculative_tokens ?? DEFAULT_SPECULATIVE_TOKENS,
      disable_auto_k: Boolean(config.speculative_disable_auto_k),
    })
    : null;
  return ['--no-telemetry', 'serve', model, ...(servedModelName ? ['--served-model-name', servedModelName] : []), '--port', String(port), '--host', '127.0.0.1', '--max-num-seqs', '1', '--max-concurrent-requests', '1', cache ? '--enable-prefix-cache' : '--disable-prefix-cache', ...(cache ? ['--cache-memory-mb', String(config.cache_memory_mb ?? 8192), '--hybrid-cache-entries', String(config.hybrid_cache_entries ?? 16)] : []), '--kv-disk-checkpoint-interval', String(config.disk_checkpoint_interval ?? 0), '--pflash', config.pflash, ...(config.pflash === 'auto' ? ['--pflash-threshold', '32768'] : []), '--prefill-step-size', String(prefillStepSize), '--max-tokens', String(config.server_max_tokens ?? 128), '--kv-cache-dtype', config.kv_cache_dtype_requested, '--kv-cache-turboquant', config.turboquant_requested, config.mllm ? '--mllm' : '--no-mllm', '--gpu-memory-utilization', String(utilization), ...(profile.tool_call_parser ? ['--tool-call-parser', profile.tool_call_parser, '--enable-auto-tool-choice'] : []), ...(profile.reasoning_parser ? ['--reasoning-parser', profile.reasoning_parser] : []), ...(profile.force_hybrid ? ['--force-hybrid'] : []), ...(speculativeConfig ? [...(config.speculative_lane === 'forced' ? ['--force-spec-decode'] : []), '--speculative-config', speculativeConfig] : []), '--log-level', 'INFO'];
}

async function localModelIdentity(model, requestedRevision) {
  // Manually-maintained model directories (MTP sidecar extraction/patching
  // targets, e.g. nightmedia diagnostics) don't live in HF hub cache layout
  // at all, so an absolute --model path is read directly rather than
  // resolved through the models--org--repo/snapshots/<rev> convention.
  if (model.startsWith('/')) {
    const config = await readFile(join(model, 'config.json'));
    return { revision: requestedRevision ?? 'local', snapshot_path: model, config_sha256: createHash('sha256').update(config).digest('hex') };
  }
  const modelRoot = join(process.env.HF_HOME ?? join(process.env.HOME ?? '', '.cache/huggingface'), 'hub', `${MODEL_PREFIX}${model.replace('/', '--')}`);
  const modelPath = join(modelRoot, 'snapshots');
  const snapshots = await readdir(modelPath);
  const mainRef = await readFile(join(modelRoot, 'refs', 'main'), 'utf8').catch(() => null);
  const revision = requestedRevision ?? mainRef?.trim() ?? snapshots.sort().at(-1);
  if (!revision) die(`No local snapshot found for ${model}. Run rapid-mlx pull first.`);
  const config = await readFile(join(modelPath, revision, 'config.json'));
  return { revision, snapshot_path: join(modelPath, revision), config_sha256: createHash('sha256').update(config).digest('hex') };
}

async function sha256File(path) {
  const hash = createHash('sha256');
  for await (const chunk of createReadStream(path)) hash.update(chunk);
  return hash.digest('hex');
}

async function resolveSidecarReference(reference) {
  const localPath = resolve(reference);
  let localStats = null;
  try { localStats = await stat(localPath); } catch { /* treat as an HF repo id */ }

  let root;
  let revision;
  if (localStats?.isFile()) {
    return { file: localPath, root: resolve(localPath, '..'), revision: 'local-file' };
  }
  if (localStats?.isDirectory()) {
    root = localPath;
    revision = 'local-directory';
  } else {
    const repoRoot = join(
      process.env.HF_HOME ?? join(process.env.HOME ?? '', '.cache', 'huggingface'),
      'hub',
      `${MODEL_PREFIX}${reference.replace('/', '--')}`,
    );
    const repoPath = join(repoRoot, 'snapshots');
    const snapshots = await readdir(repoPath).catch(() => []);
    const mainRef = await readFile(join(repoRoot, 'refs', 'main'), 'utf8').catch(() => null);
    revision = mainRef?.trim() || snapshots.sort().at(-1);
    if (!revision) die(`No local sidecar snapshot found for ${reference}. Pull it before benchmarking.`);
    root = join(repoPath, revision);
  }

  for (const name of ['model-mtp.safetensors', 'model.safetensors']) {
    const candidate = join(root, name);
    try {
      if ((await stat(candidate)).isFile()) return { file: candidate, root, revision };
    } catch { /* try the next well-known filename */ }
  }
  die(`Sidecar ${reference} has neither model-mtp.safetensors nor model.safetensors.`);
}

function fp16ToNumber(bits) {
  const sign = (bits & 0x8000) ? -1 : 1;
  const exponent = (bits >> 10) & 0x1f;
  const fraction = bits & 0x03ff;
  if (exponent === 0) return sign * (fraction / 1024) * 2 ** -14;
  if (exponent === 0x1f) return fraction ? Number.NaN : sign * Number.POSITIVE_INFINITY;
  return sign * (1 + fraction / 1024) * 2 ** (exponent - 15);
}

async function readSafetensorsHeader(path) {
  const fileStats = await stat(path);
  if (!fileStats.isFile() || fileStats.size < 10) die(`Invalid or empty safetensors file: ${path}`);
  const handle = await open(path, 'r');
  try {
    const prefix = Buffer.alloc(8);
    const prefixRead = await handle.read(prefix, 0, prefix.length, 0);
    if (prefixRead.bytesRead !== prefix.length) die(`Truncated safetensors length prefix in ${path}.`);
    const headerLength = Number(prefix.readBigUInt64LE());
    if (!Number.isSafeInteger(headerLength) || headerLength <= 0 || headerLength > 64 * 1024 * 1024 || headerLength > fileStats.size - 8) {
      die(`Invalid safetensors header length in ${path}: ${headerLength}`);
    }
    const encoded = Buffer.alloc(headerLength);
    const headerRead = await handle.read(encoded, 0, encoded.length, 8);
    if (headerRead.bytesRead !== encoded.length) die(`Truncated safetensors header in ${path}.`);
    return { handle, header: JSON.parse(encoded.toString('utf8')), dataStart: 8 + headerLength, fileSize: fileStats.size };
  } catch (error) {
    await handle.close();
    throw error;
  }
}

async function tensorMean(handle, dataStart, fileSize, tensor, path, key) {
  const [start, end] = tensor.data_offsets ?? [];
  if (!Number.isSafeInteger(start) || !Number.isSafeInteger(end) || start < 0 || end <= start || end > fileSize - dataStart) {
    die(`Invalid data offsets for ${key} in ${path}.`);
  }
  const bytesPerValue = { BF16: 2, F16: 2, F32: 4 }[tensor.dtype];
  if (!bytesPerValue) die(`Unsupported ${key} dtype ${tensor.dtype}; expected BF16, F16, or F32.`);
  if (!Array.isArray(tensor.shape) || tensor.shape.length === 0 || tensor.shape.some((value) => !Number.isSafeInteger(value) || value <= 0)) {
    die(`Invalid shape for ${key} in ${path}.`);
  }
  const valueCount = tensor.shape.reduce((product, value) => product * value, 1);
  const byteLength = end - start;
  if (!Number.isSafeInteger(valueCount) || valueCount * bytesPerValue !== byteLength || byteLength > 16 * 1024 * 1024) {
    die(`Shape/byte-length mismatch or oversized norm tensor for ${key} in ${path}.`);
  }
  const bytes = Buffer.alloc(byteLength);
  const tensorRead = await handle.read(bytes, 0, bytes.length, dataStart + start);
  if (tensorRead.bytesRead !== bytes.length) die(`Truncated tensor data for ${key} in ${path}.`);
  let sum = 0;
  let count = 0;
  if (tensor.dtype === 'BF16') {
    for (let offset = 0; offset < bytes.length; offset += 2) {
      const word = bytes.readUInt16LE(offset);
      const fp32 = Buffer.allocUnsafe(4);
      fp32.writeUInt32LE(word * 65536);
      sum += fp32.readFloatLE();
      count += 1;
    }
  } else if (tensor.dtype === 'F16') {
    for (let offset = 0; offset < bytes.length; offset += 2) {
      sum += fp16ToNumber(bytes.readUInt16LE(offset));
      count += 1;
    }
  } else if (tensor.dtype === 'F32') {
    for (let offset = 0; offset < bytes.length; offset += 4) {
      sum += bytes.readFloatLE(offset);
      count += 1;
    }
  }
  const mean = sum / count;
  if (!Number.isFinite(mean)) die(`${key} in ${path} has a non-finite mean.`);
  return mean;
}

async function assertSpecDecodeTrunkSafe(baseIdentity) {
  const inTrunkSidecar = join(baseIdentity.snapshot_path, 'model-mtp.safetensors');
  try {
    if ((await stat(inTrunkSidecar)).isFile()) {
      die(`Refusing trunk containing model-mtp.safetensors: ${inTrunkSidecar}. mlx-lm may glob it as a trunk shard; move the sidecar to a separate managed location before benchmarking.`);
    }
  } catch (error) {
    if (error.code !== 'ENOENT') throw error;
  }
}

async function sidecarIdentity(reference, baseIdentity) {
  const resolvedSidecar = await resolveSidecarReference(reference);
  const requestedSidecarPath = resolve(resolvedSidecar.file);
  const requestedTrunkPath = resolve(baseIdentity.snapshot_path);
  const requestedRelativePath = relative(requestedTrunkPath, requestedSidecarPath);
  if (requestedRelativePath === '' || (!requestedRelativePath.startsWith('..') && !isAbsolute(requestedRelativePath))) {
    die(`Refusing sidecar inside trunk directory: ${requestedSidecarPath}. Keep MTP sidecars in a separate managed location.`);
  }
  const resolvedFilePath = await realpath(resolvedSidecar.file);
  const resolvedTrunkPath = await realpath(baseIdentity.snapshot_path);
  if (resolvedFilePath === resolvedTrunkPath || resolvedFilePath.startsWith(`${resolvedTrunkPath}/`)) {
    die(`Refusing sidecar inside trunk directory: ${resolvedFilePath}. Keep MTP sidecars in a separate managed location.`);
  }
  const { handle, header, dataStart, fileSize } = await readSafetensorsHeader(resolvedFilePath);
  try {
    const tensors = new Map(
      Object.entries(header)
        .filter(([key]) => key !== '__metadata__')
        .map(([key, value]) => [key.startsWith('mtp.') ? key.slice(4) : key, value]),
    );
    const required = [
      'fc.weight',
      'pre_fc_norm_embedding.weight',
      'pre_fc_norm_hidden.weight',
      'norm.weight',
    ];
    const missing = required.filter((key) => !tensors.has(key));
    if (missing.length) die(`Sidecar ${reference} is missing required MTP tensors: ${missing.join(', ')}`);

    const normMeans = {};
    const normRanges = {
      'pre_fc_norm_embedding.weight': [0.25, 0.9],
      'pre_fc_norm_hidden.weight': [0.4, 1.25],
    };
    for (const [key, [minimum, maximum]] of Object.entries(normRanges)) {
      const mean = await tensorMean(handle, dataStart, fileSize, tensors.get(key), resolvedFilePath, key);
      normMeans[key] = mean;
      if (mean <= minimum || mean >= maximum) {
        die(`Sidecar ${reference} failed the shifted-RMSNorm sanity gate: ${key} mean=${mean.toFixed(4)} (expected ${minimum} < mean < ${maximum}).`);
      }
    }

    const baseConfigBytes = await readFile(join(baseIdentity.snapshot_path, 'config.json'));
    const baseConfig = JSON.parse(baseConfigBytes);
    const textConfig = baseConfig.text_config ?? baseConfig;
    const modelType = baseConfig.model_type;
    if (!['qwen3_5', 'qwen3_5_moe'].includes(modelType)) {
      die(`Sidecar preflight currently supports qwen3_5/qwen3_5_moe trunks; got ${modelType ?? 'missing'}.`);
    }
    if (Number(textConfig.mtp_num_hidden_layers ?? baseConfig.mtp_num_hidden_layers ?? 0) < 1) {
      die(`Trunk config for ${baseIdentity.snapshot_path} does not declare mtp_num_hidden_layers >= 1.`);
    }
    const numExperts = Number(textConfig.num_experts ?? 0);
    if (numExperts > 0) {
      const hasCanonicalMoe = [...tensors.keys()].some((key) => key.includes('.mlp.switch_mlp.gate_proj.weight'));
      const hasUnconvertedExperts = [...tensors.keys()].some((key) => key.includes('.mlp.experts.'));
      if (!hasCanonicalMoe || hasUnconvertedExperts) {
        die(`MoE sidecar ${reference} is not in the canonical switch_mlp layout required by Rapid-MLX.`);
      }
    }
    let inferredAffineQuantization = null;
    const fcScales = tensors.get('fc.scales');
    if (fcScales) {
      if (!tensors.has('fc.biases')) die(`Quantized sidecar ${reference} has fc.scales but no fc.biases.`);
      const hiddenSize = Number(textConfig.hidden_size);
      const fcWeightShape = tensors.get('fc.weight').shape;
      const fcScalesShape = fcScales.shape;
      const packedColumns = Number(fcWeightShape?.[1]);
      const scaleColumns = Number(fcScalesShape?.[1]);
      const inputSize = hiddenSize * 2;
      const bits = packedColumns * 32 / inputSize;
      const groupSize = inputSize / scaleColumns;
      if (![2, 3, 4, 5, 6, 8].includes(bits) || ![32, 64, 128].includes(groupSize)) {
        die(`Sidecar ${reference} has unsupported affine fc packing: bits=${bits}, group_size=${groupSize}.`);
      }
      inferredAffineQuantization = { bits, group_size: groupSize, mode: 'affine' };
    }

    let sidecarConfigSha256 = null;
    let sidecarConfig = null;
    try {
      const configBytes = await readFile(join(resolvedSidecar.root, 'config.json'));
      sidecarConfigSha256 = createHash('sha256').update(configBytes).digest('hex');
      sidecarConfig = JSON.parse(configBytes);
      const sidecarText = sidecarConfig.text_config ?? sidecarConfig;
      if (sidecarText.hidden_size && textConfig.hidden_size && sidecarText.hidden_size !== textConfig.hidden_size) {
        die(`Sidecar hidden_size=${sidecarText.hidden_size} does not match trunk hidden_size=${textConfig.hidden_size}.`);
      }
    } catch (error) {
      if (error.code !== 'ENOENT') throw error;
    }

    // Guard the invariant directly rather than trusting the resolver: whatever
    // reaches --speculative-config must still look like a safetensors file to
    // mx.load. Preflight reads go through realpath and would not catch this.
    if (!requestedSidecarPath.endsWith('.safetensors')) {
      die(`Sidecar path handed to the server must end in .safetensors, got: ${requestedSidecarPath}`);
    }

    const fileStats = await stat(resolvedFilePath);
    return {
      reference,
      revision: resolvedSidecar.revision,
      // The *requested* path, not the realpath. Inside an HF cache the snapshot
      // entry is a symlink into blobs/<sha256>, a bare hash with no extension,
      // and mx.load dispatches on extension -- handing the server the realpath
      // fails the sidecar load with "Unknown file format". realpath stays in
      // use below for containment checks and byte reads, where it is correct.
      resolved_file: requestedSidecarPath,
      realpath: resolvedFilePath,
      sha256: await sha256File(resolvedFilePath),
      bytes: fileStats.size,
      config_sha256: sidecarConfigSha256,
      quantization: sidecarConfig?.quantization ?? null,
      preflight: {
        core_tensors_present: true,
        shifted_norm_means: normMeans,
        trunk_model_type: modelType,
        trunk_hidden_size: textConfig.hidden_size ?? null,
        trunk_num_experts: numExperts,
        inferred_sidecar_quantization: inferredAffineQuantization ?? { mode: 'full-precision' },
      },
    };
  } finally {
    await handle.close();
  }
}

async function patchChatTemplateInSnapshot(identity, templatePath) {
  if (!templatePath) return { modelPath: identity.snapshot_path, metadata: null, restore: async () => {} };
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
    modelPath: identity.snapshot_path,
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

function manifestFor(model, identity, runtimeVersion, rapidMlxBin, config, cells, port, utilization, serverModelPath, chatTemplate, profile = {}, tokenizerPython = null, speculativeSidecar = null) {
  const argv = [rapidMlxBin, ...argvFor(serverModelPath ?? model, config, port, utilization, model, profile)];
  return {
    schema_version: 1,
    benchmark: { name: `Rapid-MLX generated ${cells[0].id}`, purpose: 'Generated by rapid-mlx-benchmark-suite.mjs; do not hand-edit.' },
    runtime: { backend: 'rapid-mlx', version: runtimeVersion, base_url: `http://127.0.0.1:${port}`, health_path: '/health', metrics_path: '/metrics', fresh_server_per_cell: true, exact_argv: argv, ...(tokenizerPython ? { tokenizer_python: tokenizerPython } : {}) },
    hardware: { cpu: 'recorded by operator', unified_memory_bytes: null },
    model: { hf_repo_id: model, revision: identity.revision, config_sha256: identity.config_sha256, tokenizer_snapshot_path: identity.snapshot_path, profile_overrides: profile, ...(chatTemplate ? { chat_template_override: chatTemplate } : {}), ...(speculativeSidecar ? { speculative_sidecar: speculativeSidecar } : {}) },
    cells,
  };
}

function materializeSpeculativeCell(cell, speculativeSidecars) {
  const sidecar = speculativeSidecars.get(cell.configuration.speculative_model);
  if (!sidecar) return cell;
  return {
    ...cell,
    configuration: {
      ...cell.configuration,
      speculative_model_reference: cell.configuration.speculative_model,
      speculative_model: sidecar.resolved_file,
    },
  };
}

function runProcess(command, args, options = {}) {
  return new Promise((resolvePromise, reject) => {
    const child = spawn(command, args, { stdio: options.stdio ?? 'inherit', cwd: options.cwd });
    child.once('error', reject);
    child.once('exit', (code, signal) => code === 0 ? resolvePromise() : reject(new Error(`${command} exited ${code ?? signal}`)));
  });
}

// The in-memory tail exists only for failure messages. Everything is also
// streamed to disk verbatim: the MTP depth-clamp line is a per-request
// logger.info emitted from mtp_generate_step, so on a long cell the rolling
// tail evicts it long before the cell ends. Effective draft depth has to be
// read from captured backend output, never inferred from a K histogram.
function launchServer(command, args, env, logPath) {
  const output = [];
  const child = spawn(command, args, { stdio: ['ignore', 'pipe', 'pipe'], env });
  const sink = logPath ? createWriteStream(logPath) : null;
  const append = (chunk) => {
    if (sink) sink.write(chunk);
    output.push(chunk.toString());
    while (output.join('').length > 12000) output.shift();
  };
  child.stdout.on('data', append);
  child.stderr.on('data', append);
  const closed = sink
    ? new Promise((resolvePromise) => { child.once('close', () => sink.end(resolvePromise)); })
    : Promise.resolve();
  return { child, logs: () => output.join(''), flushed: () => closed };
}

// Rapid clamps MTP chain-of-K to 1 whenever the model cache contains an SSM
// slot, and only logs when the requested K exceeds 1. That asymmetry is the
// whole point: with K>1 requested, the presence of the line proves a clamp and
// its absence proves the clamp did not apply. With K<=1 requested the line can
// never fire, so the run yields no evidence about depth either way -- which is
// a distinct outcome from "no clamp", and is reported as such.
const CLAMP_PATTERN = /\[MTP-chain-of-K\][^\n]*clamping max_k from (\d+) to (\d+)/g;
// engine_core logs this when --force-spec-decode flips the profile's
// supports_spec_decode verdict to true. It makes the lane self-evidencing:
// the receipt no longer has to be believed about which lane it ran in.
const FORCE_OVERRIDE_PATTERN = /Routing override: supports_spec_decode forced True via --force-spec-decode/;

function analyzeBackendLog(text, requestedK, declaredLane = null, speculativeRequested = true) {
  const clamps = [...text.matchAll(CLAMP_PATTERN)].map((match) => ({
    line: match[0],
    from: Number(match[1]),
    to: Number(match[2]),
  }));
  const requested = Number.isFinite(requestedK) ? requestedK : null;
  let verdict;
  let effectiveMaxK = null;
  if (clamps.length) {
    // A clamp line is self-evidencing: it carries both the requested and the
    // effective depth, so it outranks whatever the manifest claimed.
    verdict = 'clamp_observed';
    effectiveMaxK = clamps[0].to;
  } else if (requested === null || requested <= 1) {
    verdict = 'not_probed';
  } else {
    verdict = 'clamp_absent';
    effectiveMaxK = requested;
  }
  const forcedObserved = FORCE_OVERRIDE_PATTERN.test(text);
  // An off/baseline cell carries no speculative config, so the driver never
  // passes --force-spec-decode and engine_core never logs the override. Its
  // absence there is correct behaviour, not a mislabelled lane -- comparing
  // anyway fails every baseline cell in a forced-lane run.
  const observedLane = speculativeRequested ? (forcedObserved ? 'forced' : 'natural') : 'not_applicable';
  return {
    captured: true,
    requested_max_k: requested,
    effective_max_k: effectiveMaxK,
    effective_max_k_source: verdict === 'not_probed' ? null : 'backend_log',
    clamp_verdict: verdict,
    clamp_events: clamps.length,
    clamp_lines: clamps.slice(0, 3).map((clamp) => clamp.line),
    declared_lane: declaredLane,
    observed_lane: observedLane,
    // A mismatch means the receipt's own lane label is wrong, so nothing in it
    // can be trusted to be on the side of the line it claims.
    lane_agrees: (declaredLane === null || !speculativeRequested) ? null : declaredLane === observedLane,
  };
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

async function stopServer(server, settleSeconds = 0) {
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
  // Separate concern from the teardown wait above: this one lets the die shed
  // heat so a late cell is not measured on a hotter machine than an early one.
  // Counterbalanced ordering bounds that drift; a settle window shrinks it.
  if (settleSeconds > 0) {
    await new Promise((resolvePromise) => setTimeout(resolvePromise, settleSeconds * 1000));
  }
}

// Metric names carry a label set (family, method, ...) that varies between
// cells -- a symlinked trunk dir yields family="unknown" where a sibling cell
// yields family="qwen3.6" -- so keys cannot be matched literally.
function sumMetricsByPrefix(bag, prefix) {
  let total = null;
  for (const [key, value] of Object.entries(bag ?? {})) {
    if (!key.startsWith(prefix)) continue;
    if (typeof value !== 'number') continue;
    total = (total ?? 0) + value;
  }
  return total;
}

// Acceptance is computed from the accepts/attempts counters rather than read
// off rapid_mlx_spec_decode_accept_ratio. That metric is a gauge: its "delta"
// across a phase equals its "after" value, so treating it as a per-phase rate
// silently reports the server's lifetime ratio instead of this cell's.
function cellAcceptRatio(cell) {
  let accepts = 0;
  let attempts = 0;
  for (const attempt of cell.attempts ?? []) {
    accepts += sumMetricsByPrefix(attempt.metrics_delta, 'rapid_mlx_spec_decode_accepts_total') ?? 0;
    attempts += sumMetricsByPrefix(attempt.metrics_delta, 'rapid_mlx_spec_decode_attempts_total') ?? 0;
  }
  if (attempts <= 0) return { ratio: null, accepts, attempts };
  return { ratio: accepts / attempts, accepts, attempts };
}

// A matrix without a passing positive control cannot tell "this pairing does
// not work" from "the harness does not work". That is exactly the confusion
// that produced the void receipts and the wrong "MTP is unviable for this
// family" verdict, so a failing control invalidates the whole run rather than
// being recorded as one bad cell among many.
function assertPositiveControl(receiptCells, floor) {
  const controls = receiptCells.filter((cell) => cell.configuration?.speculative_role === 'control');
  if (!controls.length) {
    return { ok: false, reason: 'No control-role cell ran. Pass --speculative-control-model so the matrix carries a known-good positive control.' };
  }
  const failures = [];
  for (const cell of controls) {
    const { ratio, accepts, attempts } = cellAcceptRatio(cell);
    if (ratio === null) {
      failures.push(`${cell.id}: no speculative attempts recorded (MTP never installed?)`);
    } else if (ratio < floor) {
      failures.push(`${cell.id}: acceptance ${(ratio * 100).toFixed(1)}% (${accepts}/${attempts}) below floor ${(floor * 100).toFixed(1)}%`);
    }
  }
  return failures.length
    ? { ok: false, reason: `Positive control failed:\n  ${failures.join('\n  ')}` }
    : { ok: true, controls: controls.length };
}

// A single pass over the matrix cannot separate a real effect from position in
// the run. Machine state drifts monotonically across a long suite -- die
// temperature climbs, the page cache fills, the HF cache warms -- so whichever
// arm runs last is systematically penalised. Repeating the matrix with the cell
// order reversed on alternate trials (ABBA) cancels that drift to first order:
// each cell occupies both an early and a late slot, and the paired difference
// keeps its meaning. Random shuffling would not; it re-rolls the confound
// rather than balancing it, and with n=2 or 3 it can easily land every trial of
// one arm late in the run.
//
// This is the mechanism the throughput inversion has to be tested against
// before any of it counts as qualification: the arm with the highest acceptance
// produced the smallest gain, which is what an uncontrolled ordering effect
// would look like.
function expandTrials(cells, trials) {
  if (trials <= 1) return cells.map((cell) => ({ ...cell, trial: 1, base_cell_id: cell.id }));
  if (trials % 2 === 1) {
    // With an odd trial count one direction runs once more than the other, so
    // mean run position still correlates with cell index and the drift is only
    // partly cancelled. Usable, but not the clean paired design.
    process.stderr.write(`Warning: --trials ${trials} is odd; counterbalancing is only exact for an even number of trials.\n`);
  }
  const expanded = [];
  for (let trial = 1; trial <= trials; trial += 1) {
    const ordered = trial % 2 === 1 ? cells : [...cells].reverse();
    for (const cell of ordered) {
      expanded.push({
        ...cell,
        // The label must stay unique per receipt file, and must remain
        // groupable back to the cell it repeats.
        id: `${cell.id}-t${trial}`,
        base_cell_id: cell.id,
        trial,
      });
    }
  }
  return expanded;
}

// The receipt is written by the generic runner in a separate process, which
// knows nothing about the server it talked to. Backend-log evidence is folded
// in here, after the server has exited, so the depth claim travels with the
// numbers it qualifies rather than living in a loose file beside them.
async function attachBackendLog(receiptPath, serverLogPath, manifest) {
  const requestedK = manifest.cells
    .map((cell) => cell.configuration?.num_speculative_tokens)
    .find((tokens) => tokens !== undefined) ?? null;
  const declaredLane = manifest.cells
    .map((cell) => cell.configuration?.speculative_lane)
    .find((lane) => lane) ?? null;
  const speculativeRequested = manifest.cells.some((cell) => {
    const method = cell.configuration?.speculative_method;
    return method && method !== 'off';
  });
  let analysis;
  try {
    analysis = analyzeBackendLog(await readFile(serverLogPath, 'utf8'), requestedK, declaredLane, speculativeRequested);
  } catch (error) {
    analysis = { captured: false, capture_error: error.message, clamp_verdict: 'uncaptured' };
  }
  analysis.path = basename(serverLogPath);
  try {
    const receipt = JSON.parse(await readFile(receiptPath, 'utf8'));
    receipt.runtime.backend_log = analysis;
    await writeFile(receiptPath, `${JSON.stringify(receipt, null, 2)}\n`);
  } catch (error) {
    // A receipt that cannot be annotated is still a valid receipt; the log
    // file itself remains on disk as the primary artifact.
    process.stderr.write(`Warning: could not attach backend log to ${basename(receiptPath)}: ${error.message}\n`);
  }
  return analysis;
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
        // A resumed cell never launches a server, so it contributes no backend
        // log. Carry forward whatever verdict the prior run recorded rather
        // than leaving the field absent, which reads as "not checked".
        const prior = JSON.parse(await readFile(receiptPath, 'utf8'));
        receipts.push({
          label,
          receipt: basename(receiptPath),
          manifest: null,
          resumed: true,
          clamp_verdict: prior.runtime?.backend_log?.clamp_verdict ?? 'uncaptured',
        });
        continue;
      } catch { /* no durable receipt for this cell */ }
    }
    await writeFile(manifestPath, `${JSON.stringify(manifest, null, 2)}\n`);
    const [command, ...args] = manifest.runtime.exact_argv;
    const cacheHome = join(tempDir, 'cache-homes', label);
    await mkdir(cacheHome, { recursive: true });
    const hfHome = process.env.HF_HOME ?? join(process.env.HOME ?? '', '.cache', 'huggingface');
    const serverLogPath = join(outputDir, `${String(index).padStart(2, '0')}-${label}.server.log`);
    const started = launchServer(command, args, { ...process.env, HOME: cacheHome, HF_HOME: hfHome }, serverLogPath);
    const server = started.child;
    let backendLog = null;
    try {
      await waitForHealth(manifest.runtime.base_url, server);
      await runProcess(process.execPath, [resolve('scripts/model-runtime-benchmark.mjs'), 'run', '--manifest', manifestPath, '--out', receiptPath, '--server-pid', String(server.pid)], { cwd: process.cwd() });
      receipts.push({ label, receipt: basename(receiptPath), server_log: basename(serverLogPath), manifest: options.keepManifests ? manifestPath : null });
    } catch (error) {
      failure = new Error(`${error.message}\nRapid-MLX log tail for ${label} (full log: ${serverLogPath}):\n${started.logs()}`);
    } finally {
      await stopServer(server, options.settleSeconds ?? 0);
      // Only safe to read the log after the process closed and the sink drained.
      await started.flushed();
      if (!failure) backendLog = await attachBackendLog(receiptPath, serverLogPath, manifest);
    }
    if (backendLog) {
      Object.assign(receipts[receipts.length - 1], {
        clamp_verdict: backendLog.clamp_verdict,
        observed_lane: backendLog.observed_lane,
      });
      // A receipt labelled with the wrong lane is worse than a missing one:
      // it invites a forced-lane number into an enablement decision. Stop.
      if (backendLog.lane_agrees === false) {
        failure = new Error(`Lane mismatch for ${label}: manifest declared "${backendLog.declared_lane}" but the backend log shows "${backendLog.observed_lane}". Full log: ${serverLogPath}`);
      }
    }
    if (failure) break;
  }
  // Only meaningful once the matrix ran to completion -- a run that aborted
  // early has no control cell yet, and reporting that as a control failure
  // would mask the real error.
  let controlGate = null;
  if (!failure && options.suite.startsWith('spec-decode')) {
    const receiptCells = [];
    for (const entry of receipts) {
      try {
        const receipt = JSON.parse(await readFile(join(outputDir, entry.receipt), 'utf8'));
        receiptCells.push(...(receipt.cells ?? []));
      } catch { /* a receipt that will not parse is caught by the loop above */ }
    }
    const floor = options.controlAcceptFloor ?? DEFAULT_CONTROL_ACCEPT_FLOOR;
    controlGate = { floor, ...assertPositiveControl(receiptCells, floor) };
    if (!controlGate.ok) {
      failure = new Error(`${controlGate.reason}\nThe positive control is what separates "this sidecar does not work" from "the harness does not work". Every subject number in this run is uninterpretable.`);
    }
  }
  await writeFile(join(outputDir, 'suite-index.json'), `${JSON.stringify({
    model: options.model,
    suite: options.suite,
    // A reader cannot otherwise tell a counterbalanced repeat from a single
    // pass, and a single pass is not a qualification result.
    trial_protocol: {
      trials: options.trials ?? 1,
      ordering: (options.trials ?? 1) > 1 ? 'counterbalanced-abba' : 'single-pass',
      settle_seconds: options.settleSeconds ?? 0,
    },
    speculative_lane: options.specDecodeLane ?? null,
    qualification_eligible: (options.specDecodeLane ?? null) !== 'forced',
    positive_control: controlGate,
    receipts,
    complete: failure === null,
    failure: failure?.message ?? null,
  }, null, 2)}\n`);
  if (failure) throw failure;
}

const { command, options } = parseArgs(process.argv);
const rapidMlxBin = options.rapidMlxBin ?? 'rapid-mlx';
const identity = await localModelIdentity(options.model, options.revision);
if (options.suite.startsWith('spec-decode')) await assertSpecDecodeTrunkSafe(identity);
const allCells = await suiteCells(options.model, options.suite, options.image, options.expectedVisualTerms, options.mllmPrefillStepSize ?? 1024, options.workspacePack, options.speculativeModel ?? null, options.speculativeControlModel ?? null, options.speculativeTokens ?? DEFAULT_SPECULATIVE_TOKENS, options.speculativeMethods ?? ['mtp'], options.speculativeWorkloads ?? ['code'], options.disableSpeculativeAutoK ?? false);
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
// After --cell validation, which is expressed against unsuffixed cell ids.
cells = expandTrials(cells, options.trials ?? 1);
if (options.specDecodeLane) {
  // Stamped on every cell, baselines included, so a receipt cannot be read
  // without knowing which eligibility lane produced the run it belongs to.
  cells = cells.map((cell) => ({
    ...cell,
    configuration: { ...cell.configuration, speculative_lane: options.specDecodeLane },
  }));
}
const speculativeSidecars = new Map();
for (const reference of new Set(cells.map((cell) => cell.configuration.speculative_model).filter(Boolean))) {
  speculativeSidecars.set(reference, await sidecarIdentity(reference, identity));
}
const profile = await profileOverridesFor(options.model, rapidMlxBin);
if (options.toolCallParser) profile.tool_call_parser = options.toolCallParser;
if (options.reasoningParser) profile.reasoning_parser = options.reasoningParser;
if (options.forceHybrid) profile.force_hybrid = true;
const runtimeVersion = installedRapidMlxVersion(rapidMlxBin);

if (command === 'plan') {
  const manifests = cells.map((item, index) => {
    const sidecar = speculativeSidecars.get(item.configuration.speculative_model) ?? null;
    const materialized = materializeSpeculativeCell(item, speculativeSidecars);
    return {
      label: `${String(index).padStart(2, '0')}-${item.id}`,
      manifest: manifestFor(options.model, identity, runtimeVersion, rapidMlxBin, materialized.configuration, [materialized], options.port, options.utilization, identity.snapshot_path, options.chatTemplate ? { source_path: resolve(options.chatTemplate), note: 'materialized as an isolated overlay at run time' } : null, profile, options.tokenizerPython ?? null, sidecar),
    };
  });
  process.stdout.write(`${JSON.stringify({ model: options.model, identity, suite: options.suite, manifests: manifests.map(({ label, manifest }) => ({ label, cells: manifest.cells.map((item) => item.id), speculative_sidecar: manifest.model.speculative_sidecar ?? null, argv: manifest.runtime.exact_argv })) }, null, 2)}\n`);
} else {
  const tempDir = await mkdtemp(join(tmpdir(), 'rapid-mlx-benchmark-suite-'));
  try {
    const template = await patchChatTemplateInSnapshot(identity, options.chatTemplate);
    const manifests = cells.map((item, index) => {
      const sidecar = speculativeSidecars.get(item.configuration.speculative_model) ?? null;
      const materialized = materializeSpeculativeCell(item, speculativeSidecars);
      return {
        label: `${String(index).padStart(2, '0')}-${item.id}`,
        manifest: manifestFor(options.model, identity, runtimeVersion, rapidMlxBin, materialized.configuration, [materialized], options.port, options.utilization, template.modelPath, template.metadata, profile, options.tokenizerPython ?? null, sidecar),
      };
    });
    try { await runSuite(options, manifests, tempDir); } finally { await template.restore(); }
  } finally { if (!options.keepManifests) await rm(tempDir, { recursive: true, force: true }); }
}
