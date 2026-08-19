#!/usr/bin/env node
/**
 * One-command wrapper around llama-cpp-benchmark-suite.mjs and
 * rapid-mlx-benchmark-suite.mjs. Runs whichever backend(s) it's given
 * (single-backend calls are normal, not a degraded mode), writes both
 * backends' receipts under one unified directory, and generates the combined
 * comparison report via `model-runtime-benchmark.mjs compare`.
 *
 * Usage: node scripts/backend-ab-suite.mjs run --label NAME
 *          [--llama-cpp-model PATH] [--llama-cpp-server PATH]
 *          [--rapid-mlx-model REPO] [--rapid-mlx-suite SUITE]
 *          [--out-root DIR] [--resume]
 *
 * At least one of --llama-cpp-model / --rapid-mlx-model is required.
 * See docs/reference/model-runtime-benchmarking.md for a full walkthrough.
 */
import { mkdir } from 'node:fs/promises';
import { spawn } from 'node:child_process';
import { join, resolve } from 'node:path';

const DEFAULT_OUT_ROOT = 'tests/fixtures/calibration/ab-runs';
const DEFAULT_RAPID_MLX_SUITE = 'ubatch';

function die(message) { throw new Error(message); }

function parseArgs(argv) {
  const [command, ...rest] = argv.slice(2);
  const options = { rapidMlxSuite: DEFAULT_RAPID_MLX_SUITE, outRoot: DEFAULT_OUT_ROOT };
  for (let index = 0; index < rest.length; index += 1) {
    const key = rest[index];
    if (!key.startsWith('--')) die(`Invalid argument: ${key}`);
    if (key === '--resume') { options.resume = true; continue; }
    const value = rest[index + 1];
    if (value === undefined) die(`Missing value for ${key}`);
    index += 1;
    if (key === '--label') options.label = value;
    else if (key === '--llama-cpp-model') options.llamaCppModel = value;
    else if (key === '--llama-cpp-server') options.llamaCppServer = value;
    else if (key === '--rapid-mlx-model') options.rapidMlxModel = value;
    else if (key === '--rapid-mlx-suite') options.rapidMlxSuite = value;
    else if (key === '--out-root') options.outRoot = value;
    else die(`Unknown option: ${key}`);
  }
  if (command !== 'run') die('Use run.');
  if (!options.label) die('--label is required.');
  if (!options.llamaCppModel && !options.rapidMlxModel) {
    die('Provide at least one of --llama-cpp-model or --rapid-mlx-model.');
  }
  return options;
}

function runProcess(command, args) {
  return new Promise((resolvePromise, reject) => {
    const child = spawn(command, args, { stdio: 'inherit' });
    child.once('error', reject);
    child.once('exit', (code, signal) => (
      code === 0 ? resolvePromise() : reject(new Error(`${command} ${args.join(' ')} exited ${code ?? signal}`))
    ));
  });
}

const options = parseArgs(process.argv);
const runRoot = resolve(options.outRoot, options.label);
await mkdir(runRoot, { recursive: true });
const compareDirs = [];

if (options.llamaCppModel) {
  const outDir = join(runRoot, 'llama-cpp');
  const args = ['scripts/llama-cpp-benchmark-suite.mjs', 'run', '--model', options.llamaCppModel, '--out', outDir];
  if (options.llamaCppServer) args.push('--server', options.llamaCppServer);
  if (options.resume) args.push('--resume');
  console.log(`\n=== llama.cpp: ${options.llamaCppModel} -> ${outDir} ===`);
  await runProcess(process.execPath, args);
  compareDirs.push(outDir);
}

if (options.rapidMlxModel) {
  const outDir = join(runRoot, 'rapid-mlx');
  const args = ['scripts/rapid-mlx-benchmark-suite.mjs', 'run', '--model', options.rapidMlxModel, '--suite', options.rapidMlxSuite, '--out', outDir];
  if (options.resume) args.push('--resume');
  console.log(`\n=== rapid-mlx: ${options.rapidMlxModel} (suite ${options.rapidMlxSuite}) -> ${outDir} ===`);
  await runProcess(process.execPath, args);
  compareDirs.push(outDir);
}

const reportPath = join(runRoot, 'report.md');
await runProcess(process.execPath, [
  'scripts/model-runtime-benchmark.mjs', 'compare',
  ...compareDirs.flatMap((dir) => ['--dir', dir]),
  '--out', reportPath,
]);
console.log(`\nWrote ${reportPath}`);
