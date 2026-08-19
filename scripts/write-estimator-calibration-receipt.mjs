#!/usr/bin/env node
/**
 * Pair one exact /api/vram-estimate response with one fresh-server benchmark
 * cell. This intentionally does not run either system: the operator supplies
 * the already-captured JSON bodies, keeping the API token out of artifacts.
 *
 * node scripts/write-estimator-calibration-receipt.mjs \
 *   --runtime-receipt RECEIPT --cell CELL --attempt cold \
 *   --estimator-request REQUEST.json --estimator-response RESPONSE.json \
 *   --dataset-role tuning|holdout --out CALIBRATION.json
 */
import { readFile, writeFile } from 'node:fs/promises';
import { createHash } from 'node:crypto';
import { relative, resolve } from 'node:path';

function die(message) { throw new Error(message); }

function parseArgs(argv) {
  const options = {};
  for (let index = 2; index < argv.length; index += 2) {
    const key = argv[index];
    const value = argv[index + 1];
    if (!key?.startsWith('--') || value === undefined) die(`Invalid argument: ${key ?? ''}`);
    const name = key.slice(2).replace(/-([a-z])/g, (_, letter) => letter.toUpperCase());
    options[name] = value;
  }
  for (const key of ['runtimeReceipt', 'cell', 'attempt', 'estimatorRequest', 'estimatorResponse', 'datasetRole', 'out']) {
    if (!options[key]) die(`--${key.replace(/[A-Z]/g, (letter) => `-${letter.toLowerCase()}`)} is required.`);
  }
  if (!['cold', 'repeat', 'extension'].includes(options.attempt)) die('--attempt must be cold, repeat, or extension.');
  if (!['tuning', 'holdout'].includes(options.datasetRole)) die('--dataset-role must be tuning or holdout.');
  return options;
}

async function json(path) {
  try {
    return JSON.parse(await readFile(path, 'utf8'));
  } catch (error) {
    die(`Could not read JSON ${path}: ${error.message}`);
  }
}

function assertTokenFree(value, path = '$') {
  if (Array.isArray(value)) value.forEach((item, index) => assertTokenFree(item, `${path}[${index}]`));
  else if (value && typeof value === 'object') {
    for (const [key, item] of Object.entries(value)) {
      if (/(authorization|api[_-]?token)/i.test(key)) die(`Refusing to persist secret-like field ${path}.${key}.`);
      assertTokenFree(item, `${path}.${key}`);
    }
  }
}

const options = parseArgs(process.argv);
const receiptPath = resolve(options.runtimeReceipt);
const [receiptBytes, runtimeReceipt, request, response] = await Promise.all([
  readFile(receiptPath), json(receiptPath), json(options.estimatorRequest), json(options.estimatorResponse),
]);
assertTokenFree(request);
assertTokenFree(response);
if (runtimeReceipt.kind !== 'model_runtime_benchmark_receipt') die('runtime receipt kind must be model_runtime_benchmark_receipt.');
if (runtimeReceipt.runtime?.fresh_server_per_cell !== true) die('runtime receipt must have fresh_server_per_cell=true.');
if (!runtimeReceipt.runtime?.version || /record from/i.test(runtimeReceipt.runtime.version)) die('runtime receipt must record the exact Rapid-MLX version or commit.');
const cell = runtimeReceipt.cells?.find((item) => item.id === options.cell);
if (!cell) die(`No cell named ${options.cell}.`);
const attempt = cell.attempts?.find((item) => item.phase === options.attempt);
if (!attempt) die(`Cell ${options.cell} has no ${options.attempt} attempt.`);
const actualPeak = attempt.metrics_after?.rapid_mlx_metal_peak_memory_bytes;
if (!Number.isFinite(actualPeak) || actualPeak <= 0) die('The selected attempt has no positive rapid_mlx_metal_peak_memory_bytes measurement.');
const predicted = response.total_bytes;
if (!Number.isFinite(predicted) || predicted <= 0) die('Estimator response must contain a positive total_bytes.');
const residual = actualPeak - predicted;
const output = {
  schema_version: 1,
  kind: 'estimator_calibration_receipt',
  captured_at: new Date().toISOString(),
  estimator: { endpoint: '/api/vram-estimate', request, response },
  runtime_receipt: {
    path: relative(process.cwd(), receiptPath),
    cell_id: cell.id,
    attempt_phase: attempt.phase,
    fresh_server_per_cell: true,
    sha256: createHash('sha256').update(receiptBytes).digest('hex'),
    runtime_version: runtimeReceipt.runtime.version,
  },
  model: {
    hf_repo_id: runtimeReceipt.model?.hf_repo_id,
    revision: runtimeReceipt.model?.revision,
    config_sha256: runtimeReceipt.model?.config_sha256,
  },
  requested_effective_kv: {
    requested: request.kv_cache_dtype ?? null,
    effective_active: response.effective_kv_dtype ?? null,
  },
  measurement: { metric_name: 'rapid_mlx_metal_peak_memory_bytes', actual_peak_bytes: actualPeak },
  residual: {
    predicted_total_bytes: predicted,
    actual_peak_bytes: actualPeak,
    residual_bytes: residual,
    residual_pct: residual / predicted * 100,
    predicted_gib: predicted / 1024 ** 3,
    actual_gib: actualPeak / 1024 ** 3,
  },
  dataset_role: options.datasetRole,
};
await writeFile(resolve(options.out), `${JSON.stringify(output, null, 2)}\n`);
