#!/usr/bin/env node
// One-command requalification lane for Rapid-MLX speculative decoding.
//
// Rapid-MLX 0.11.1's MTP scheduler was directly observed to be greedy-only and
// to bail whenever a logits processor is installed. Under those two conditions
// no shipping agentic workload can ever reach it, so speculative decoding is
// held at Unavailable in the capability snapshot regardless of --speculative
// being present. See docs/reference/rapid-mlx-mtp-evidence.md.
//
// This script answers exactly one question about a new build: did upstream fix
// that yet? It runs three gates and prints one verdict per gate.
//
//   sampled      speculation is active at temperature > 0
//   constrained  speculation is active with a tool grammar installed
//   parity       greedy output is token-identical with speculation on and off
//
// The gate names and their order are mirrored in SPEC_DECODE_GATES in
// src/inference/rapid_mlx/capabilities.rs; the probe message the user sees
// after a managed upgrade points here. Keep the two in step.
//
// Usage:
//   node scripts/rapid-mlx-requalify-spec-decode.mjs \
//     --model /path/to/trunk \
//     --speculative-control-model mlx-community/Qwen3.6-27B-MTP-4bit \
//     --out tmp/requalify-0.12.0
//
// Exit codes:
//   0   all gates pass; speculative decoding may be promoted for this build
//   20  gates ran cleanly but the scheduler still does not engage (still blocked)
//   1   a gate is uninterpretable: control failed, or a run errored

import { spawn, spawnSync } from 'node:child_process';
import { mkdir, readFile, writeFile } from 'node:fs/promises';
import { join, resolve } from 'node:path';

const SUITE = resolve('scripts/rapid-mlx-benchmark-suite.mjs');

function die(message) {
  process.stderr.write(`${message}\n`);
  process.exit(1);
}

function parseArgs(argv) {
  const options = { port: 8110 };
  const rest = argv.slice(2);
  for (let index = 0; index < rest.length; index += 1) {
    const key = rest[index];
    if (!key.startsWith('--')) die(`Invalid argument: ${key}`);
    const value = rest[index + 1];
    if (value === undefined) die(`Missing value for ${key}`);
    index += 1;
    if (key === '--model') options.model = value;
    else if (key === '--speculative-control-model') options.controlModel = value;
    else if (key === '--speculative-model') options.subjectModel = value;
    else if (key === '--out') options.out = value;
    else if (key === '--rapid-mlx-bin') options.rapidMlxBin = value;
    else if (key === '--port') options.port = Number(value);
    else if (key === '--tool-call-parser') options.toolCallParser = value;
    else if (key === '--reasoning-parser') options.reasoningParser = value;
    else if (key === '--profile-alias') options.profileAlias = value;
    else if (key === '--gate') (options.gates ??= []).push(value);
    else die(`Unknown option: ${key}`);
  }
  if (!options.model) die('--model is required (the trunk to run).');
  if (!options.controlModel) {
    die('--speculative-control-model is required. Requalification without a '
      + 'known-good positive control cannot distinguish "upstream is still '
      + 'blocked" from "this sidecar is broken".');
  }
  if (!options.out) die('--out RECEIPT_DIRECTORY is required.');
  const unknown = (options.gates ?? []).filter((name) => !GATES.some((gate) => gate.name === name));
  if (unknown.length) die(`Unknown gate(s): ${unknown.join(', ')}`);
  return options;
}

// Each gate is one suite invocation plus one predicate over its receipts.
// Deliberately the smallest cell selection that can answer the question: this
// lane is meant to be cheap enough to run on every upstream bump.
const GATES = [
  {
    name: 'sampled',
    question: 'Is speculation active at temperature > 0?',
    suite: 'spec-decode',
    cells: ['spec-off-8000-int8', 'spec-control-mtp-8000-int8'],
    // Explicit, not 'recommended', on purpose. The recommended lane resolves
    // this family's vendor profile to temperature 1.0 with reasoning ON, and
    // these cells budget 512 tokens: the model spends the whole budget inside
    // <think>, emits empty content, and the inner benchmark fails its
    // completion floor. That failure says nothing about speculation. This gate
    // owns one axis — temperature > 0 — so it pins temperature and leaves
    // reasoning off, which is also the axis the greedy-only scheduler bails on.
    // Coding temperature is the one that matters in practice.
    sampling: 'explicit',
    temperature: 0.6,
    evaluate: (facts) => {
      if (!facts.temperature) {
        return {
          verdict: 'fail',
          reason: 'The recommended sampling lane resolved to temperature 0, so '
            + 'this run measured the greedy path again and says nothing about '
            + 'sampled decode. Supply the model\'s recommended sampling or run '
            + 'the suite with an explicit temperature.',
        };
      }
      if (facts.attempts <= 0) {
        return {
          verdict: 'blocked',
          reason: `No speculative attempts recorded at temperature ${facts.temperature}: `
            + 'the scheduler still falls through for sampled requests.',
        };
      }
      return {
        verdict: 'pass',
        reason: `${facts.accepts}/${facts.attempts} draft tokens accepted at `
          + `temperature ${facts.temperature}.`,
      };
    },
  },
  {
    name: 'constrained',
    question: 'Is speculation active with a tool grammar installed?',
    // The warm suite is the one that installs tools with tool_choice=required,
    // which is what puts a logits processor in the decode path.
    suite: 'spec-decode-warm',
    cells: ['spec-warm-off-8000-int8', 'spec-warm-control-mtp-8000-int8'],
    sampling: 'greedy',
    evaluate: (facts) => {
      if (facts.attempts <= 0) {
        return {
          verdict: 'blocked',
          reason: 'No speculative attempts recorded with a tool grammar installed: '
            + 'the scheduler still bails when a logits processor is present.',
        };
      }
      return {
        verdict: 'pass',
        reason: `${facts.accepts}/${facts.attempts} draft tokens accepted with tools on.`,
      };
    },
  },
  {
    name: 'parity',
    question: 'Is greedy output token-identical with speculation on and off?',
    suite: 'spec-decode',
    cells: ['spec-off-8000-int8', 'spec-control-mtp-8000-int8'],
    sampling: 'greedy',
    // Two trials so the baseline's own reproducibility is measured; without it
    // a mismatch cannot be attributed to speculation.
    trials: 2,
    evaluate: (facts) => {
      const parity = facts.suiteIndex?.greedy_parity;
      if (!parity) {
        return { verdict: 'fail', reason: 'Run produced no greedy_parity block.' };
      }
      if (parity.baseline_deterministic === false) {
        return {
          verdict: 'fail',
          reason: 'The off-lane baseline is not reproducible across trials, so an '
            + 'off-vs-MTP difference proves nothing about losslessness.',
        };
      }
      if (facts.attempts <= 0) {
        return {
          verdict: 'blocked',
          reason: 'No speculative attempts recorded, so there is no speculative '
            + 'output to compare against the baseline.',
        };
      }
      if (parity.mismatches > 0) {
        return {
          verdict: 'fail',
          reason: `${parity.mismatches}/${parity.comparisons_total} comparisons diverged; `
            + 'greedy speculation is not lossless on this build.',
        };
      }
      return {
        verdict: 'pass',
        reason: `${parity.comparisons_total}/${parity.comparisons_total} comparisons `
          + 'token-identical.',
      };
    },
  },
];

// `rapid-mlx info` resolves tool/reasoning parsers only for HF repo aliases. A
// plain local --model directory reports the literal "(none)" for both, so a
// local trunk silently runs with no parser installed. That is fatal to the
// `constrained` gate specifically: with no tool-call parser there is no tool
// grammar in the decode path, so the gate would report "speculation works with
// tools" from a run that never installed tools. Resolve, then refuse.
const INFO_ABSENT_VALUES = new Set(['(none)', 'none', 'n/a', '-', 'unknown', 'unset', '']);

function infoParsers(model, bin) {
  const result = spawnSync(bin, ['info', model], { encoding: 'utf8', timeout: 30000 });
  const parsers = {};
  if (result.status !== 0 || !result.stdout) return parsers;
  for (const line of result.stdout.split('\n')) {
    const clean = line.replace(/^\s*│/, '').replace(/│\s*$/, '').trim();
    const colon = clean.indexOf(':');
    if (colon === -1) continue;
    const key = clean.slice(0, colon).trim().toLowerCase().replace(/[ _-]/g, '');
    const value = clean.slice(colon + 1).trim();
    if (INFO_ABSENT_VALUES.has(value.toLowerCase())) continue;
    if (key === 'toolformat' || key === 'tool') parsers.toolCallParser = value;
    else if (key === 'reasoningparser' || key === 'reasoning') parsers.reasoningParser = value;
  }
  return parsers;
}

// Ordered sources, most explicit first, with the source recorded in the report
// so a verdict can never be read without knowing where its parsers came from.
function resolveParsers(options, bin) {
  if (options.toolCallParser && options.reasoningParser) {
    return { ...options, parserSource: 'explicit flags' };
  }
  const fromModel = infoParsers(options.model, bin);
  const fromAlias = options.profileAlias ? infoParsers(options.profileAlias, bin) : {};
  const toolCallParser = options.toolCallParser ?? fromModel.toolCallParser ?? fromAlias.toolCallParser;
  const reasoningParser = options.reasoningParser ?? fromModel.reasoningParser ?? fromAlias.reasoningParser;
  const parserSource = [
    options.toolCallParser || options.reasoningParser ? 'explicit flags' : null,
    fromModel.toolCallParser || fromModel.reasoningParser ? `rapid-mlx info ${options.model}` : null,
    fromAlias.toolCallParser || fromAlias.reasoningParser ? `rapid-mlx info ${options.profileAlias}` : null,
  ].filter(Boolean).join(' + ') || 'unresolved';
  return { ...options, toolCallParser, reasoningParser, parserSource };
}

function installedVersion(bin) {
  const result = spawnSync(bin, ['--version'], { encoding: 'utf8', timeout: 30000 });
  const version = `${result.stdout ?? ''}\n${result.stderr ?? ''}`.trim();
  if (result.status !== 0 || !version) {
    die('Could not read `rapid-mlx --version`. A requalification verdict that is '
      + 'not pinned to a build is worthless.');
  }
  return version;
}

function runSuite(args) {
  return new Promise((resolveRun) => {
    const child = spawn(process.execPath, [SUITE, 'run', ...args], {
      stdio: ['ignore', 'inherit', 'inherit'],
    });
    child.on('error', (error) => resolveRun({ status: null, error: error.message }));
    child.on('close', (status) => resolveRun({ status, error: null }));
  });
}

// Read back what the suite recorded rather than what we asked for: a lane that
// silently fell back to greedy, or a control cell that never installed MTP,
// must not be reported as the thing we requested.
async function gateFacts(outputDir) {
  const suiteIndex = JSON.parse(await readFile(join(outputDir, 'suite-index.json'), 'utf8'));
  let accepts = 0;
  let attempts = 0;
  for (const entry of suiteIndex.receipts ?? []) {
    let receipt;
    try {
      receipt = JSON.parse(await readFile(join(outputDir, entry.receipt), 'utf8'));
    } catch {
      continue;
    }
    for (const cell of receipt.cells ?? []) {
      if (cell.configuration?.speculative_role !== 'control') continue;
      for (const attempt of cell.attempts ?? []) {
        accepts += sumByPrefix(attempt.metrics_delta, 'rapid_mlx_spec_decode_accepts_total');
        attempts += sumByPrefix(attempt.metrics_delta, 'rapid_mlx_spec_decode_attempts_total');
      }
    }
  }
  return {
    suiteIndex,
    accepts,
    attempts,
    temperature: suiteIndex.sampling_lane?.temperature ?? 0,
  };
}

function sumByPrefix(metrics, prefix) {
  if (!metrics) return 0;
  let total = 0;
  for (const [key, value] of Object.entries(metrics)) {
    if (key.startsWith(prefix) && Number.isFinite(value)) total += value;
  }
  return total;
}

const parsedOptions = parseArgs(process.argv);
const rapidMlxBin = parsedOptions.rapidMlxBin ?? 'rapid-mlx';
const options = resolveParsers(parsedOptions, rapidMlxBin);
const version = installedVersion(rapidMlxBin);
const outRoot = resolve(options.out);
await mkdir(outRoot, { recursive: true });

const selected = options.gates?.length
  ? GATES.filter((gate) => options.gates.includes(gate.name))
  : GATES;

if (selected.some((gate) => gate.name === 'constrained') && !options.toolCallParser) {
  die('The `constrained` gate needs a tool-call parser, and none could be resolved '
    + `for ${options.model}. \`rapid-mlx info\` reports "(none)" for local model `
    + 'directories, so a run without one installs no tool grammar and the gate would '
    + 'pass on a request that never constrained anything. Pass --tool-call-parser '
    + 'explicitly, or --profile-alias with the HF repo alias for this family '
    + '(e.g. --profile-alias unsloth/Qwen3.6-27B-MLX-8bit), or skip it with '
    + '--gate sampled --gate parity.');
}

process.stderr.write(`Requalifying speculative decoding on ${version}\n`);
process.stderr.write(`Parsers: tool=${options.toolCallParser ?? 'none'} `
  + `reasoning=${options.reasoningParser ?? 'none'} (source: ${options.parserSource})\n`);

const results = [];
for (const gate of selected) {
  const gateDir = join(outRoot, gate.name);
  process.stderr.write(`\n=== gate ${gate.name}: ${gate.question}\n`);
  const args = [
    '--model', options.model,
    '--suite', gate.suite,
    '--out', gateDir,
    '--port', String(options.port),
    '--rapid-mlx-bin', rapidMlxBin,
    // Never 'forced': a forced run overrides Rapid's own eligibility verdict,
    // so it can only ever be research evidence and can never requalify.
    '--spec-decode-lane', 'natural',
    // Every gate below evaluates attempts<=0 itself and calls it 'blocked'. The
    // suite's default is to fail the run on zero speculative activity, which
    // never lets those branches run: a still-blocked build would exit 1
    // (uninterpretable / harness broken) instead of 20 (upstream still limited).
    // Those two must not be confusable -- that distinction is this lane's point.
    '--spec-zero-activity', 'observed',
    '--speculative-control-model', options.controlModel,
    '--sampling', gate.sampling,
    ...(gate.temperature !== undefined
      ? ['--temperature', String(options.sampledTemperature ?? gate.temperature)]
      : []),
    ...(options.subjectModel ? ['--speculative-model', options.subjectModel] : []),
    ...(gate.trials ? ['--trials', String(gate.trials)] : []),
    ...(options.toolCallParser ? ['--tool-call-parser', options.toolCallParser] : []),
    ...(options.reasoningParser ? ['--reasoning-parser', options.reasoningParser] : []),
    ...gate.cells.flatMap((cell) => ['--cell', cell]),
  ];

  const run = await runSuite(args);
  let result;
  if (run.error) {
    result = { verdict: 'error', reason: `Suite failed to start: ${run.error}` };
  } else {
    let facts = null;
    try {
      facts = await gateFacts(gateDir);
    } catch (error) {
      result = { verdict: 'error', reason: `Could not read run output: ${error.message}` };
    }
    if (facts) {
      // The positive control is the precondition for every other reading. A
      // failing control means the harness or the sidecar is broken, which is a
      // different finding from upstream still being blocked.
      const control = facts.suiteIndex.positive_control;
      if (control && control.ok === false) {
        result = { verdict: 'fail', reason: `Positive control failed: ${control.reason}` };
      } else if (facts.suiteIndex.complete === false) {
        result = {
          verdict: 'error',
          reason: `Run did not complete: ${facts.suiteIndex.failure ?? 'unknown failure'}`,
        };
      } else {
        result = gate.evaluate(facts);
      }
      // A 'pass' means "speculation engaged and produced accepted tokens", which
      // is exactly what a control that recorded no activity cannot support. If a
      // gate ever returns pass on such a run, the gate's predicate and the
      // control disagree; refuse to promote capability off a contradiction.
      if (result.verdict === 'pass' && control?.code === 'no-activity') {
        result = {
          verdict: 'error',
          reason: `Gate passed but the positive control recorded zero speculative activity: ${control.reason} `
            + 'The gate predicate and the control disagree, so neither reading can be trusted.',
        };
      }
      result.control_code = control?.code ?? 'absent';
      result.accepts = facts.accepts;
      result.attempts = facts.attempts;
      result.temperature = facts.temperature;
    }
  }
  result.gate = gate.name;
  result.question = gate.question;
  result.receipts = gateDir;
  results.push(result);
  process.stderr.write(`--- ${gate.name}: ${result.verdict.toUpperCase()} — ${result.reason}\n`);
}

const verdicts = new Set(results.map((result) => result.verdict));
const overall = verdicts.has('error') || verdicts.has('fail')
  ? 'uninterpretable'
  : verdicts.has('blocked')
    ? 'still-blocked'
    : 'qualified';

const report = {
  rapid_mlx_version: version,
  model: options.model,
  speculative_control_model: options.controlModel,
  speculative_model: options.subjectModel ?? null,
  // A gate verdict is only readable alongside the parsers that were installed:
  // "tools did not slow speculation down" means nothing if no tool parser ran.
  tool_call_parser: options.toolCallParser ?? null,
  reasoning_parser: options.reasoningParser ?? null,
  parser_source: options.parserSource,
  // Present so a reader knows a partial run is partial.
  gates_run: selected.map((gate) => gate.name),
  gates_defined: GATES.map((gate) => gate.name),
  overall,
  // Only a full sweep with every gate passing can promote the capability;
  // a partial run is evidence, not qualification.
  promotes_capability: overall === 'qualified' && selected.length === GATES.length,
  results,
  generated_at: new Date().toISOString(),
};
const reportPath = join(outRoot, 'requalification.json');
await writeFile(reportPath, `${JSON.stringify(report, null, 2)}\n`);

process.stderr.write(`\nOverall: ${overall}\n`);
if (report.promotes_capability) {
  process.stderr.write(
    'All gates pass. Remove this version from SPEC_DECODE_GREEDY_ONLY_VERSIONS in '
    + 'src/inference/rapid_mlx/capabilities.rs and record the evidence in '
    + 'docs/reference/rapid-mlx-mtp-evidence.md before promoting spec_decode.\n',
  );
} else if (overall === 'still-blocked') {
  process.stderr.write(
    'Upstream has not fixed the greedy-only / logits-processor limitation. Keep '
    + 'spec_decode Unavailable; nothing in the app needs to change.\n',
  );
}
process.stderr.write(`Report: ${reportPath}\n`);

process.exit(overall === 'qualified' ? 0 : overall === 'still-blocked' ? 20 : 1);
