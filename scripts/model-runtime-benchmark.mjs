#!/usr/bin/env node
/**
 * Reproducible, runtime-neutral benchmark runner for local OpenAI-compatible
 * model servers. The server is deliberately started outside this script: a
 * benchmark cell must record the exact argv/effective settings supplied by
 * the operator, and alternative runtimes need not share a launcher.
 *
 * Run:     node scripts/model-runtime-benchmark.mjs run --manifest FILE --out RECEIPT
 * Report:  node scripts/model-runtime-benchmark.mjs report --input RECEIPT [--input RECEIPT ...] [--out REPORT.md]
 * Compare: node scripts/model-runtime-benchmark.mjs compare --dir RECEIPT_DIR [--dir RECEIPT_DIR ...] --out REPORT.md
 */
import { readFile, readdir, writeFile } from 'node:fs/promises';
import { spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { performance } from 'node:perf_hooks';
import { join } from 'node:path';

const WORDS = [
  'architecture', 'boundary', 'cache', 'context', 'deterministic', 'evidence', 'function',
  'generation', 'implementation', 'latency', 'memory', 'model', 'observation', 'performance',
  'quality', 'request', 'response', 'runtime', 'sequence', 'system', 'token', 'validation',
  'workflow', 'reliable', 'structured', 'measure', 'repeatable', 'independent', 'analysis',
];

function die(message) {
  throw new Error(message);
}

function parseArgs(argv) {
  const [command, ...rest] = argv.slice(2);
  const options = { inputs: [] };
  for (let index = 0; index < rest.length; index += 1) {
    const key = rest[index];
    const value = rest[index + 1];
    if (!key.startsWith('--') || value === undefined) die(`Invalid argument: ${key}`);
    index += 1;
    if (key === '--manifest') options.manifest = value;
    else if (key === '--base-url') options.baseUrl = value.replace(/\/$/, '');
    else if (key === '--out') options.out = value;
    else if (key === '--input') options.inputs.push(value);
    else if (key === '--dir') (options.dirs ??= []).push(value);
    else if (key === '--cell') options.cells ??= [], options.cells.push(value);
    else if (key === '--server-pid') options.serverPid = Number(value);
    else die(`Unknown option: ${key}`);
  }
  if (!['run', 'report', 'compare', 'self-test'].includes(command)) die('Use run, report, compare, or self-test.');
  if (command === 'run' && (!options.manifest || !options.out)) die('run requires --manifest and --out.');
  if (command === 'report' && (!options.inputs.length || !options.out)) die('report requires --input and --out.');
  if (command === 'compare' && (!options.dirs?.length || !options.out)) die('compare requires --dir (repeatable) and --out.');
  return { command, options };
}

// Markers are scattered numeric constants (not one verbatim phrase) so
// fidelity is scored as graduated recall across context positions rather
// than a single pass/fail string match, and so a model cannot satisfy the
// check by pattern-completing a repeated word from the surrounding filler.
function sortedMarkerPositions(targetWords, markers) {
  return [...(markers ?? [])]
    .map((marker) => ({ ...marker, wordIndex: Math.floor(targetWords * marker.position) }))
    .sort((a, b) => a.wordIndex - b.wordIndex);
}

function makeReference(targetWords, markers) {
  const words = [];
  const positions = new Map(sortedMarkerPositions(targetWords, markers).map((marker) => [marker.wordIndex, marker]));
  for (let index = 0; index < targetWords; index += 1) {
    const marker = positions.get(index);
    if (marker) words.push(`${marker.name}=${marker.value}`);
    words.push(WORDS[index % WORDS.length]);
  }
  return words.join(' ');
}

function makeCodeReference(targetWords, markers) {
  const blocks = [];
  let words = 0;
  const positions = sortedMarkerPositions(targetWords, markers);
  let nextMarker = 0;
  for (let index = 0; words < targetWords; index += 1) {
    while (nextMarker < positions.length && words >= positions[nextMarker].wordIndex) {
      const marker = positions[nextMarker];
      blocks.push(`export const ${marker.name} = ${marker.value};`);
      words += 6;
      nextMarker += 1;
    }
    const suffix = String(index).padStart(5, '0');
    // Per-block values are distinct (not a uniform boilerplate literal), so a
    // model cannot recover a marker by pattern-completing a repeated answer.
    const revision = (index % 7) + 1;
    const enabled = index % 3 !== 0;
    blocks.push([
      `export type Document${suffix} = { id: string; revision: number; enabled: boolean };`,
      `export function validateDocument${suffix}(document: Document${suffix}): boolean {`,
      `  if (!document.id || document.revision < ${revision}) return false;`,
      `  return document.enabled === ${enabled};`,
      '}',
      '',
    ].join('\n'));
    words += 28;
  }
  return blocks.join('\n');
}

function makeCorpus(workload, targetWords, includeMarkers = true) {
  const markers = includeMarkers ? workload.markers : null;
  return workload.corpus === 'code'
    ? makeCodeReference(targetWords, markers)
    : makeReference(targetWords, markers);
}

function escapeRegex(value) {
  return value.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
}

// A missing or duplicated marker makes recall uninterpretable. Validate the
// generated corpus before it ever reaches a model so an output discrepancy is
// attributable to the request/runtime rather than fixture construction.
function assertGeneratedMarkers(reference, markers) {
  for (const marker of markers ?? []) {
    const separator = reference.includes(`export const ${marker.name}`) ? '\\s*=\\s*' : '\\s*[:=]\\s*';
    const pattern = new RegExp(`${escapeRegex(marker.name)}${separator}${marker.value}(?!\\d)`, 'g');
    const occurrences = [...reference.matchAll(pattern)].length;
    if (occurrences !== 1) {
      die(`Generated corpus must contain ${marker.name}=${marker.value} exactly once; found ${occurrences}.`);
    }
  }
}

function workloadText(workload, extension = false) {
  const reference = extension && workload.exact_extension_text
    ? `${makeCorpus(workload, workload.target_words)}\n\n${workload.exact_extension_text}`
    : workload.reference_text ?? (extension
    ? `${makeCorpus(workload, workload.target_words)}\n\n${makeCorpus(workload, workload.extension_words, false)}`
    : makeCorpus(workload, workload.target_words));
  assertGeneratedMarkers(reference, workload.markers);
  const instruction = workload.instruction ?? (workload.markers?.length
    ? 'List every CHECK_* constant name and its numeric value found in the reference below, one per line as NAME=VALUE. Output nothing else.'
    : `Read the reference, then output exactly ${workload.output_words ?? 96} ordinary English words separated by spaces, with no explanation or punctuation.`);
  return `${instruction}\n\n${reference}`;
}

function makeBranchAudit(targetWords, branchNumber) {
  const lines = [];
  let words = 0;
  for (let index = 0; words < targetWords; index += 1) {
    const file = String((branchNumber * 97 + index) % 1000).padStart(5, '0');
    const line = `src/documents/document${file}.ts audit status unchanged contract stable types pass lint pass unit-tests pass branch ${branchNumber}`;
    lines.push(line);
    words += line.split(/\s+/).length;
  }
  return lines.join('\n');
}

function branchReviewFixture(phase, targetWords = 0) {
  const branchNumber = Number(phase.replace('fork-wide-', ''));
  if (!Number.isInteger(branchNumber) || branchNumber < 1) die(`Invalid branch-review phase: ${phase}`);
  const targetIndex = 100 + branchNumber;
  const suffix = String(targetIndex).padStart(5, '0');
  const revision = (targetIndex % 7) + 1;
  const enabled = targetIndex % 3 !== 0;
  const safe = branchNumber % 2 === 1;
  const beforeGuard = `  if (!document.id || document.revision < ${revision}) return false;`;
  const afterGuard = safe
    ? beforeGuard
    : `  if (document.revision < ${revision}) return false;`;
  const changedLine = safe
    ? `+ // Branch ${branchNumber}: keep the validation contract explicit for maintainers.`
    : `+ // Branch ${branchNumber}: permit documents without an id.`;
  const auditWords = Math.max(0, targetWords - 120);
  const audit = makeBranchAudit(auditWords, branchNumber);
  const expected = `BRANCH_${branchNumber}=${safe ? 'SAFE' : 'UNSAFE'}`;
  return {
    expected,
    text: [
      `Branch ${branchNumber} patch review`,
      '',
      'Review policy:',
      '- SAFE: both the missing-id guard and revision floor remain enforced, and the enabled-state comparison is unchanged.',
      '- UNSAFE: either validation guard is removed or weakened, or the enabled-state comparison changes.',
      '',
      `Patch for src/documents/document${suffix}.ts:`,
      '```diff',
      ` export function validateDocument${suffix}(document: Document${suffix}): boolean {`,
      `-${beforeGuard}`,
      `+${afterGuard}`,
      changedLine,
      `   return document.enabled === ${enabled};`,
      ' }',
      '```',
      '',
      'Return exactly one of these two lines and nothing else:',
      `BRANCH_${branchNumber}=SAFE`,
      `BRANCH_${branchNumber}=UNSAFE`,
      '',
      'Frozen branch audit context (informational; the diff and policy above are authoritative):',
      audit,
    ].join('\n'),
  };
}

function branchReviewFixturePhase(phase) {
  if (phase.startsWith('fork-wide-')) return phase;
  const lineage = phase.match(/^root-(cold|prime|probe)-(\d+)$/);
  if (!lineage) return null;
  const branchNumber = Number(lineage[2]);
  // Use disjoint fixture IDs so root creation, prime, and probe requests cannot
  // accidentally satisfy fidelity by repeating an earlier verdict.
  const offset = { cold: 300, prime: 400, probe: 500 }[lineage[1]];
  return `fork-wide-${offset + branchNumber}`;
}

function runSelfTest() {
  const markers = [
    { position: 0.1, name: 'CHECK_ALPHA', value: 8291 },
    { position: 0.3, name: 'CHECK_BRAVO', value: 4417 },
    { position: 0.5, name: 'CHECK_CHARLIE', value: 6053 },
    { position: 0.7, name: 'CHECK_DELTA', value: 1928 },
    { position: 0.9, name: 'CHECK_ECHO', value: 7360 },
  ];
  const reference = makeCorpus({ corpus: 'code', markers }, 12500);
  assertGeneratedMarkers(reference, markers);
  const recall = scoreMarkerRecall(markers, markers.map((marker) => `${marker.name}=${marker.value}`).join('\n'));
  if (recall?.markers_correct !== markers.length) die('Self-test failed: marker scorer did not recover every generated marker.');

  const safeBranch = branchReviewFixture('fork-wide-1', 500);
  const unsafeBranch = branchReviewFixture('fork-wide-2', 500);
  if (safeBranch.expected !== 'BRANCH_1=SAFE' || !safeBranch.text.includes('-  if (!document.id')) {
    die('Self-test failed: safe patch-review fixture is not contract-preserving.');
  }
  if (unsafeBranch.expected !== 'BRANCH_2=UNSAFE' || !unsafeBranch.text.includes('+  if (document.revision')) {
    die('Self-test failed: unsafe patch-review fixture does not weaken the id guard.');
  }
  const response = {
    completion_text: safeBranch.expected,
    server_errors: [],
    usage: { prompt_tokens: 1, completion_tokens: 1 },
    tool_calls: [],
  };
  if (scoreFidelity({ branch_workload: 'patch-review' }, response, 'fork-wide-1').final_answer_passed !== true) {
    die('Self-test failed: branch final-answer gate rejected the expected verdict.');
  }
  const lineageFixture = branchReviewFixture(branchReviewFixturePhase('root-probe-1'));
  const lineageResponse = { ...response, completion_text: lineageFixture.expected };
  if (lineageFixture.expected !== 'BRANCH_501=SAFE'
      || scoreFidelity({ branch_workload: 'patch-review' }, lineageResponse, 'root-probe-1').final_answer_passed !== true
      || phaseIncludesMarkers('root-prime-1')) {
    die('Self-test failed: lineage prime/probe phases are not isolated and fidelity-gated.');
  }
  const reasoningOnly = { ...response, completion_text: '', reasoning_preview: safeBranch.expected };
  if (scoreFidelity({ branch_workload: 'patch-review' }, reasoningOnly, 'fork-wide-1').request_succeeded !== false) {
    die('Self-test failed: reasoning-only output was accepted as a final answer.');
  }
  const cacheAttempt = {
    phase: 'repeat',
    fidelity: {
      request_succeeded: true,
      completion_length_passed: null,
      marker_recall: null,
      final_answer_passed: null,
      tool_call_observed: null,
      tool_arguments_passed: null,
    },
    tool_trace: [],
    metrics_delta: {
      rapid_mlx_prefix_cache_hits_total: 1,
      rapid_mlx_prefix_cache_misses_total: 0,
      rapid_mlx_prefix_cache_tokens_saved_total: 32000,
      rapid_mlx_prefix_cache_evictions_total: 0,
    },
  };
  assertAttemptQualified({
    id: 'self-test-cache',
    configuration: {},
    workload: { prefix_cache_expectations: { repeat: { min_hits: 1, max_misses: 0, min_tokens_saved: 28000 } } },
  }, cacheAttempt);
  if (cacheAttempt.fidelity.prefix_cache?.passed !== true) {
    die('Self-test failed: valid prefix-cache metrics did not clear their gate.');
  }
  process.stdout.write('model-runtime-benchmark self-test passed.\n');
}

function prepareExactExtension(workload, runtime, model) {
  if (!workload.exact_extension_tokens || workload.exact_extension_text) return;
  if (!runtime.tokenizer_python || !model.tokenizer_snapshot_path) {
    die('exact_extension_tokens requires --tokenizer-python and a pinned tokenizer snapshot.');
  }
  const baseText = workloadText(workload, false);
  const script = String.raw`import json, sys
from transformers import AutoTokenizer
payload = json.load(sys.stdin)
tok = AutoTokenizer.from_pretrained(payload['snapshot'], local_files_only=True)
def count(text):
    # Some community Qwen tokenizer snapshots expose a chat-template entry
    # which renders an empty user turn through Transformers, while Rapid uses
    # its own compatible formatter. Count the exact user-content suffix with
    # the model tokenizer instead; the receipt separately records the server's
    # actual rendered prompt-token delta.
    return len(tok.encode(text, add_special_tokens=False))
base = count(payload['base'])
target = payload['target']
def candidate(n): return ' cache' * n
hi = max(target * 2, 64)
while count(payload['base'] + '\n\n' + candidate(hi)) - base < target:
    hi *= 2
    if hi > target * 32 + 4096: raise RuntimeError('could not bracket exact suffix')
lo = 0
while lo < hi:
    mid = (lo + hi) // 2
    if count(payload['base'] + '\n\n' + candidate(mid)) - base < target: lo = mid + 1
    else: hi = mid
suffix = candidate(lo)
delta = count(payload['base'] + '\n\n' + suffix) - base
if delta != target: raise RuntimeError(f'exact suffix unavailable: requested {target}, got {delta}')
print(json.dumps({'text': suffix, 'base_tokens': base, 'extension_tokens': delta, 'tokenization_scope': 'user_content'}))`;
  const result = spawnSync(runtime.tokenizer_python, ['-c', script], {
    input: JSON.stringify({ snapshot: model.tokenizer_snapshot_path, base: baseText, target: workload.exact_extension_tokens }),
    encoding: 'utf8',
    maxBuffer: 1024 * 1024,
  });
  if (result.status !== 0) die(`Exact-token suffix construction failed: ${result.stderr || result.stdout}`);
  const prepared = JSON.parse(result.stdout);
  workload.exact_extension_text = prepared.text;
  workload.exact_extension_tokenizer = { requested_tokens: workload.exact_extension_tokens, ...prepared };
}

function parsePrometheus(text) {
  const metrics = {};
  for (const line of text.split('\n')) {
    if (!line || line.startsWith('#')) continue;
    const match = line.match(/^([A-Za-z_:][A-Za-z0-9_:]*)(\{[^}]*\})?\s+([-+0-9.eE]+)$/);
    if (!match) continue;
    const [, name, labels = '', rawValue] = match;
    metrics[`${name}${labels}`] = Number(rawValue);
  }
  return metrics;
}

function selectedMetrics(metrics) {
  return Object.fromEntries(
    Object.entries(metrics).filter(([name]) => /^(rapid_mlx|metal|process|llama|llamacpp|ggml|backend)_/.test(name)),
  );
}

function metricDelta(before, after) {
  const keys = new Set([...Object.keys(before), ...Object.keys(after)]);
  return Object.fromEntries([...keys].map((key) => [key, (after[key] ?? 0) - (before[key] ?? 0)]));
}

function metricSumByPrefix(metrics, prefix) {
  return Object.entries(metrics)
    .filter(([name]) => name.startsWith(prefix))
    .reduce((sum, [, value]) => sum + value, 0);
}

function metricSumByPrefixOrNull(metrics, prefix) {
  const matches = Object.entries(metrics).filter(([name]) => name.startsWith(prefix));
  return matches.length ? matches.reduce((sum, [, value]) => sum + value, 0) : null;
}

async function getMetrics(baseUrl, path) {
  const response = await fetch(`${baseUrl}${path}`);
  if (!response.ok) die(`GET ${path} failed: ${response.status}`);
  return selectedMetrics(parsePrometheus(await response.text()));
}

function verifyExpectedMetrics(cell, metrics) {
  const expected = cell.configuration?.expected_metrics;
  if (!expected) return;
  const mismatches = Object.entries(expected)
    .filter(([name, value]) => metrics[name] !== value)
    .map(([name, value]) => `${name}: expected ${value}, got ${metrics[name] ?? 'absent'}`);
  if (mismatches.length) die(`${cell.id} runtime setting mismatch: ${mismatches.join('; ')}`);
}

async function streamRequest(baseUrl, request) {
  const startedAt = performance.now();
  const response = await fetch(`${baseUrl}/v1/chat/completions`, {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify({ ...request, stream: true, stream_options: { include_usage: true } }),
  });
  if (!response.ok || !response.body) die(`Chat request failed: ${response.status} ${await response.text()}`);
  const reader = response.body.getReader();
  const decoder = new TextDecoder();
  let buffer = '';
  let firstTokenAt = null;
  let usage = null;
  let completion = '';
  let reasoning = '';
  let generated = '';
  const errors = [];
  const toolCalls = [];
  while (true) {
    const { done, value } = await reader.read();
    if (done) break;
    buffer += decoder.decode(value, { stream: true });
    const lines = buffer.split('\n');
    buffer = lines.pop() ?? '';
    for (const line of lines) {
      if (!line.startsWith('data: ')) continue;
      const payload = line.slice(6).trim();
      if (!payload || payload === '[DONE]') continue;
      const event = JSON.parse(payload);
      if (event.error) errors.push(event.error);
      if (event.usage) usage = event.usage;
      const delta = event.choices?.[0]?.delta;
      const contentToken = delta?.content;
      const reasoningToken = delta?.reasoning_content;
      if (contentToken || reasoningToken) {
        if (firstTokenAt === null) firstTokenAt = performance.now();
        if (reasoningToken) {
          reasoning += reasoningToken;
          generated += reasoningToken;
        }
        if (contentToken) {
          completion += contentToken;
          generated += contentToken;
        }
        // Opt-in diagnostic only: lets an operator inspect the full thinking
        // trace live; the receipt retains only its bounded preview and digest.
        if (process.env.BENCHMARK_DEBUG_STREAM === '1') {
          if (reasoningToken) process.stderr.write(reasoningToken);
          if (contentToken) process.stderr.write(contentToken);
        }
      }
      if (Array.isArray(delta?.tool_calls)) toolCalls.push(...delta.tool_calls);
    }
  }
  const endedAt = performance.now();
  const mergedToolCalls = Object.values(toolCalls.reduce((accumulator, call) => {
    const index = call.index ?? 0;
    const merged = accumulator[index] ?? { index, function: { name: '', arguments: '' } };
    if (call.id) merged.id = call.id;
    if (call.function?.name) merged.function.name += call.function.name;
    if (call.function?.arguments) merged.function.arguments += call.function.arguments;
    accumulator[index] = merged;
    return accumulator;
  }, {}));
  const generationMs = firstTokenAt === null ? null : endedAt - firstTokenAt;
  return {
    usage,
    first_token_ms: firstTokenAt === null ? null : Math.round((firstTokenAt - startedAt) * 100) / 100,
    total_ms: Math.round((endedAt - startedAt) * 100) / 100,
    generation_ms: generationMs === null ? null : Math.round(generationMs * 100) / 100,
    generation_tokens_per_second:
      usage?.completion_tokens && generationMs && generationMs > 0
        ? Math.round((usage.completion_tokens * 100000) / generationMs) / 100
        : null,
    // Keep the complete response only until fidelity/tool-trace scoring has
    // run. Long reasoning models can put the decisive marker after the
    // receipt preview cutoff; serializing the full text would make fixtures
    // unnecessarily large and may retain user-like prompt material.
    completion_text: completion,
    completion_preview: completion.slice(0, 2000),
    reasoning_preview: reasoning.slice(0, 2000),
    // Keep final-content and complete generated-stream digests distinct.
    // Speculative lossless parity consumes generated_sha256 because reasoning
    // tokens are part of the accepted stream even though only final content is
    // valid for fidelity scoring and conversation replay.
    completion_sha256: createHash('sha256').update(completion).digest('hex'),
    completion_characters: completion.length,
    reasoning_sha256: reasoning ? createHash('sha256').update(reasoning).digest('hex') : null,
    reasoning_characters: reasoning.length,
    generated_sha256: createHash('sha256').update(generated).digest('hex'),
    generated_characters: generated.length,
    server_errors: errors,
    tool_calls: mergedToolCalls,
  };
}

function isSubset(expected, actual) {
  if (expected === actual) return true;
  if (!expected || !actual || typeof expected !== 'object' || typeof actual !== 'object') return false;
  return Object.entries(expected).every(([key, value]) => isSubset(value, actual[key]));
}

// `followup` and `fork` rebuild their prompt with includeMarkers=false, so the CHECK_*
// constants are absent from the corpus and the instruction asks for a concise answer instead
// of NAME=VALUE lines. Recall is therefore unmeasurable in those phases, and scoring it
// anyway records 0/5 on a third of every cache receipt's attempts — which reads as the cache
// corrupting long context. Any phase that did not carry the markers scores null instead.
function phaseIncludesMarkers(phase) {
  return phase !== 'followup'
    && phase !== 'fork'
    && !phase.startsWith('fork-wide-')
    && !phase.startsWith('root-cold-')
    && !phase.startsWith('root-prime-')
    && !phase.startsWith('root-probe-');
}

// Markers are scattered numeric constants scored as graduated recall rather than
// a single verbatim string match, so fidelity survives sampling/temperature
// variance and cannot be satisfied by pattern-completing repeated filler text.
function scoreMarkerRecall(markers, completionText) {
  if (!markers?.length) return null;
  const found = new Map();
  const pattern = /([A-Z0-9_]+)\s*[:=]\s*(-?\d+)/g;
  let match;
  while ((match = pattern.exec(completionText))) {
    found.set(match[1], Number(match[2]));
  }
  let correct = 0;
  const perMarker = markers.map((marker) => {
    const recalledValue = found.has(marker.name) ? found.get(marker.name) : null;
    const valueCorrect = recalledValue === marker.value;
    if (valueCorrect) correct += 1;
    return {
      name: marker.name,
      expected_value: marker.value,
      recalled_value: recalledValue,
      correct: valueCorrect,
    };
  });
  return {
    markers_expected: markers.length,
    markers_recalled: perMarker.filter((entry) => entry.recalled_value !== null).length,
    markers_correct: correct,
    recall_rate: Math.round((correct / markers.length) * 10000) / 10000,
    per_marker: perMarker,
  };
}

function scoreFidelity(workload, response, phase) {
  const matchingTool = workload.expected_tool_name
    ? response.tool_calls.find((call) => call.function?.name === workload.expected_tool_name)
    : null;
  let argumentsPassed = null;
  if (workload.expected_tool_arguments) {
    try {
      argumentsPassed = Boolean(matchingTool) && isSubset(
        workload.expected_tool_arguments,
        JSON.parse(matchingTool.function.arguments),
      );
    } catch {
      argumentsPassed = false;
    }
  }
  // A server can terminate an SSE stream with a syntactically valid but empty
  // usage payload. Treat that as a failed workload, not an executed row. A
  // tool-only completion is valid even when it contains no text token.
  const hasFinalAnswer = (response.completion_text ?? '').trim().length > 0;
  const requestSucceeded = response.server_errors.length === 0
    && (response.usage?.prompt_tokens ?? 0) > 0
    && (hasFinalAnswer || response.tool_calls.length > 0);
  const failureReason = requestSucceeded
    ? null
    : response.server_errors.length > 0
      ? 'server_error_event'
      : (response.usage?.prompt_tokens ?? 0) === 0
        ? 'empty_or_missing_prompt_usage'
        : 'missing_final_answer_without_tool_call';
  const fixturePhase = branchReviewFixturePhase(phase);
  const branchExpectation = fixturePhase && workload.branch_workload === 'patch-review'
    ? branchReviewFixture(fixturePhase).expected
    : null;
  const normalizedFinalAnswer = (response.completion_text ?? '').trim();
  return {
    request_succeeded: requestSucceeded,
    failure_reason: failureReason,
    marker_recall: phaseIncludesMarkers(phase)
      ? scoreMarkerRecall(
        workload.markers,
        response.completion_text ?? response.completion_preview,
      )
      : null,
    expected_final_answer: branchExpectation,
    final_answer_passed: branchExpectation === null
      ? null
      : normalizedFinalAnswer === branchExpectation,
    expected_tool_name: workload.expected_tool_name ?? null,
    tool_call_observed: workload.expected_tool_name
      ? Boolean(matchingTool)
      : null,
    expected_tool_arguments: workload.expected_tool_arguments ?? null,
    tool_arguments_passed: argumentsPassed,
    minimum_completion_tokens: workload.minimum_completion_tokens ?? null,
    completion_length_passed: workload.minimum_completion_tokens
      ? (response.usage?.completion_tokens ?? 0) >= workload.minimum_completion_tokens
      : null,
    expected_visual_terms: workload.expected_visual_terms ?? null,
    visual_terms_passed: workload.expected_visual_terms
      ? workload.expected_visual_terms.every((term) => (response.completion_text ?? '').toLowerCase().includes(term.toLowerCase()))
      : null,
    expected_content_terms: workload.expected_content_terms ?? null,
    content_terms_passed: workload.expected_content_terms
      ? workload.expected_content_terms.every((term) => (response.completion_text ?? '').toLowerCase().includes(term.toLowerCase()))
      : null,
  };
}

function scoreToolExpectation(response, expectation = {}) {
  const matchingTool = expectation.expected_tool_name
    ? response.tool_calls.find((call) => call.function?.name === expectation.expected_tool_name)
    : response.tool_calls[0] ?? null;
  let argumentsPassed = null;
  if (expectation.expected_tool_arguments) {
    try {
      argumentsPassed = Boolean(matchingTool) && isSubset(
        expectation.expected_tool_arguments,
        JSON.parse(matchingTool.function.arguments),
      );
    } catch {
      argumentsPassed = false;
    }
  }
  return {
    expected_tool_name: expectation.expected_tool_name ?? null,
    tool_call_observed: Boolean(matchingTool),
    expected_tool_arguments: expectation.expected_tool_arguments ?? null,
    tool_arguments_passed: argumentsPassed,
  };
}

function assistantToolMessage(response) {
  return {
    role: 'assistant',
    content: response.completion_text ?? response.completion_preview ?? null,
    tool_calls: response.tool_calls.map((call, index) => ({
      id: call.id ?? `benchmark-tool-${index}`,
      type: 'function',
      function: {
        name: call.function?.name ?? '',
        arguments: call.function?.arguments ?? '{}',
      },
    })),
  };
}

async function requestFor(cell, extension = false) {
  const workload = cell.workload;
  const text = workloadText(workload, extension);
  let content = text;
  if (workload.image_path) {
    const bytes = await readFile(workload.image_path);
    const mime = workload.image_path.endsWith('.jpg') || workload.image_path.endsWith('.jpeg')
      ? 'image/jpeg'
      : 'image/png';
    content = [
      { type: 'text', text },
      { type: 'image_url', image_url: { url: `data:${mime};base64,${bytes.toString('base64')}` } },
    ];
  }
  const request = {
    model: cell.model,
    max_tokens: workload.max_tokens ?? 128,
    messages: [{ role: 'user', content }],
  };
  // Native OpenCode Build does not impose a sampling preset.  When its model
  // and agent configuration also leave sampling unset (the current Gemma
  // path), it omits these properties and lets the provider/runtime apply the
  // model defaults.  Keep that wire behavior distinct from an explicit
  // temperature: 0 control: `undefined` is not equivalent to greedy.
  if (!workload.omit_sampling_parameters) request.temperature = workload.temperature ?? 0;
  // Only sent when the workload asks for them, so the greedy default stays a
  // clean temperature-0 request rather than one carrying inert nucleus
  // parameters. A model's recommended sampling settings are top_p/top_k as
  // much as temperature, and a run that can only vary temperature cannot
  // reproduce the shipped configuration.
  for (const key of ['top_p', 'top_k', 'min_p', 'presence_penalty', 'repetition_penalty', 'seed']) {
    if (workload[key] !== undefined) request[key] = workload[key];
  }
  if (workload.tools) request.tools = workload.tools;
  if (workload.extra_body) Object.assign(request, workload.extra_body);
  return request;
}

function receiptRequest(request) {
  const contents = JSON.stringify(request.messages);
  return {
    model: request.model,
    temperature: request.temperature ?? null,
    // A receipt that records only temperature cannot be read back as "these
    // are the model's recommended settings" or "these are not".
    top_p: request.top_p ?? null,
    top_k: request.top_k ?? null,
    min_p: request.min_p ?? null,
    presence_penalty: request.presence_penalty ?? null,
    repetition_penalty: request.repetition_penalty ?? null,
    seed: request.seed ?? null,
    sampling_mode: request.temperature === undefined
      ? 'provider-default'
      : (request.temperature === 0 ? 'greedy' : 'sampled'),
    sampling_parameter_source: request.temperature === undefined
      ? 'omitted (OpenCode Build-compatible provider default)'
      : 'explicit request field',
    max_tokens: request.max_tokens,
    reasoning_max_tokens: request.reasoning_max_tokens ?? null,
    reasoning_effort: request.reasoning_effort ?? null,
    enable_thinking: request.enable_thinking ?? request.chat_template_kwargs?.enable_thinking ?? null,
    message_characters: contents.length,
    message_sha256: createHash('sha256').update(contents).digest('hex'),
    tool_names: request.tools?.map((tool) => tool.function?.name).filter(Boolean) ?? [],
  };
}

function parseVmStatText(text) {
  let pageSize = 16384;
  const counts = { free: 0, wired: 0, compressor: 0, compressed: 0, purgeable: 0, inactive: 0, swapins: 0, swapouts: 0 };
  for (const line of text.split('\n')) {
    const sizeMatch = line.match(/^Mach Virtual Memory Statistics: \(page size of (\d+)/);
    if (sizeMatch) {
      pageSize = Number(sizeMatch[1]);
      continue;
    }
    const colonIndex = line.indexOf(':');
    if (colonIndex === -1) continue;
    const key = line.slice(0, colonIndex).trim();
    const value = Number(line.slice(colonIndex + 1).trim().replace(/\.$/, '').replace(/,/g, '')) || 0;
    switch (key) {
      case 'Pages free': counts.free = value; break;
      case 'Pages wired down': counts.wired = value; break;
      case 'Pages occupied by compressor': counts.compressor = value; break;
      case 'Pages stored in compressor': counts.compressed = value; break;
      case 'Pages purgeable': counts.purgeable = value; break;
      case 'Pages inactive': counts.inactive = value; break;
      case 'Swapins': counts.swapins = value; break;
      case 'Swapouts': counts.swapouts = value; break;
      default: break;
    }
  }
  return { pageSize, ...counts };
}

// Shells out directly to macOS primitives rather than calling llama-monitor's
// own HTTP API, since a benchmark cell may run rapid-mlx natively without the
// app/server present.
function sampleOsMemory(serverPid) {
  if (process.platform !== 'darwin') return null;
  const vmStat = spawnSync('vm_stat', [], { encoding: 'utf8', timeout: 5000 });
  const counts = vmStat.status === 0 && vmStat.stdout ? parseVmStatText(vmStat.stdout) : null;
  const wiredLimit = spawnSync('/usr/sbin/sysctl', ['-n', 'iogpu.wired_limit_mb'], { encoding: 'utf8', timeout: 5000 });
  const iogpuWiredLimitMb = wiredLimit.status === 0 ? Number(wiredLimit.stdout.trim()) || 0 : 0;
  let serverRssBytes = null;
  if (serverPid) {
    const ps = spawnSync('ps', ['-o', 'rss=', '-p', String(serverPid)], { encoding: 'utf8', timeout: 5000 });
    if (ps.status === 0) {
      const rssKb = Number(ps.stdout.trim());
      serverRssBytes = Number.isFinite(rssKb) ? rssKb * 1024 : null;
    }
  }
  return {
    sampled_at: new Date().toISOString(),
    free_bytes: counts ? counts.free * counts.pageSize : null,
    wired_bytes: counts ? counts.wired * counts.pageSize : null,
    compressor_bytes: counts ? counts.compressor * counts.pageSize : null,
    compressed_bytes: counts ? counts.compressed * counts.pageSize : null,
    purgeable_bytes: counts ? counts.purgeable * counts.pageSize : null,
    inactive_bytes: counts ? counts.inactive * counts.pageSize : null,
    iogpu_wired_limit_mb: iogpuWiredLimitMb,
    server_rss_bytes: serverRssBytes,
  };
}

async function runAttempt(baseUrl, metricsPath, cell, phase, extension = false, serverPid = null, requestOverride = null) {
  const before = await getMetrics(baseUrl, metricsPath);
  const osMemoryBefore = sampleOsMemory(serverPid);
  verifyExpectedMetrics(cell, before);
  const request = requestOverride ?? await requestFor(cell, extension);
  const response = await streamRequest(baseUrl, request);
  const trace = [];
  let traceMessages = request.messages;
  let traceResponse = response;
  for (const step of cell.workload.tool_trace?.steps ?? []) {
    const tool = step.tool_name
      ? traceResponse.tool_calls.find((call) => call.function?.name === step.tool_name)
      : traceResponse.tool_calls[0];
    if (!tool) {
      trace.push({
        injected_tool_name: step.tool_name ?? null,
        request: null,
        response: null,
        fidelity: { tool_call_observed: false, failure_reason: 'required_prior_tool_call_missing' },
      });
      break;
    }
    const toolCallId = tool.id ?? `benchmark-tool-${trace.length}`;
    traceMessages = [
      ...traceMessages,
      assistantToolMessage(traceResponse),
      { role: 'tool', tool_call_id: toolCallId, content: step.tool_result ?? '' },
    ];
    const followupRequest = { ...request, messages: traceMessages };
    const followupResponse = await streamRequest(baseUrl, followupRequest);
    trace.push({
      injected_tool_name: tool.function?.name ?? null,
      request: receiptRequest(followupRequest),
      response: followupResponse,
      fidelity: scoreToolExpectation(followupResponse, step),
    });
    traceResponse = followupResponse;
  }
  const after = await getMetrics(baseUrl, metricsPath);
  const osMemoryAfter = sampleOsMemory(serverPid);
  const delta = metricDelta(before, after);
  const compressed = metricSumByPrefix(delta, 'rapid_mlx_pflash_compressed_tokens_total');
  response.pflash_compressed_tokens = compressed;
  response.pflash_retained_tokens_estimate = response.usage?.prompt_tokens
    ? Math.max(0, response.usage.prompt_tokens - compressed)
    : null;
  response.raw_prompt_tokens_per_second = response.usage?.prompt_tokens && response.first_token_ms
    ? Math.round((response.usage.prompt_tokens * 100000) / response.first_token_ms) / 100
    : null;
  response.retained_prompt_tokens_per_second = response.pflash_retained_tokens_estimate && response.first_token_ms
    ? Math.round((response.pflash_retained_tokens_estimate * 100000) / response.first_token_ms) / 100
    : null;
  const allResponses = [response, ...trace.map((item) => item.response).filter(Boolean)];
  const roundTripCompletionTokens = allResponses.reduce((sum, item) => sum + (item.usage?.completion_tokens ?? 0), 0);
  const roundTripGenerationMs = allResponses.reduce((sum, item) => sum + (item.generation_ms ?? 0), 0);
  response.round_trip_request_count = allResponses.length;
  response.round_trip_total_ms = allResponses.reduce((sum, item) => sum + (item.total_ms ?? 0), 0);
  response.round_trip_first_token_ms = allResponses.reduce((sum, item) => sum + (item.first_token_ms ?? 0), 0);
  response.round_trip_completion_tokens = roundTripCompletionTokens;
  response.round_trip_generation_tokens_per_second = roundTripGenerationMs > 0
    ? Math.round((roundTripCompletionTokens * 100000) / roundTripGenerationMs) / 100
    : null;
  const fidelity = scoreFidelity(cell.workload, response, phase);
  const conversationAssistantContent = response.completion_text;
  // Full completions are needed for scoring but are deliberately kept out of
  // durable receipts; the bounded preview remains the diagnostic artifact.
  delete response.completion_text;
  for (const item of trace) {
    if (item.response) delete item.response.completion_text;
  }
  return {
    phase,
    conversation_assistant_content: conversationAssistantContent,
    conversation_request_messages: request.messages,
    request: { ...receiptRequest(request), image_fixture: cell.workload.image_fixture ?? null, workspace_fixture: cell.workload.workspace_fixture ?? null },
    response,
    fidelity,
    tool_trace: trace,
    metrics_before: before,
    metrics_after: after,
    metrics_delta: delta,
    os_memory_before: osMemoryBefore,
    os_memory_after: osMemoryAfter,
  };
}

function assertAttemptQualified(cell, attempt) {
  if (!attempt.fidelity.request_succeeded) {
    die(`${cell.id}/${attempt.phase} request failed qualification: ${attempt.fidelity.failure_reason ?? 'unknown'}`);
  }
  if (attempt.fidelity.completion_length_passed === false) {
    die(`${cell.id}/${attempt.phase} completed fewer than the required ${cell.workload.minimum_completion_tokens} tokens.`);
  }
  const markerRecall = attempt.fidelity.marker_recall;
  if (markerRecall && cell.workload.minimum_marker_recall_rate !== undefined
      && markerRecall.recall_rate < cell.workload.minimum_marker_recall_rate) {
    die(`${cell.id}/${attempt.phase} recalled ${markerRecall.markers_correct}/${markerRecall.markers_expected} markers; required at least ${(cell.workload.minimum_marker_recall_rate * 100).toFixed(0)}%.`);
  }
  if (attempt.fidelity.final_answer_passed === false) {
    die(`${cell.id}/${attempt.phase} final answer did not equal ${attempt.fidelity.expected_final_answer}.`);
  }
  const prefixExpectation = cell.workload.prefix_cache_expectations?.[attempt.phase];
  if (prefixExpectation) {
    const observed = {
      hits: metricSumByPrefixOrNull(attempt.metrics_delta, 'rapid_mlx_prefix_cache_hits_total'),
      misses: metricSumByPrefixOrNull(attempt.metrics_delta, 'rapid_mlx_prefix_cache_misses_total'),
      tokens_saved: metricSumByPrefixOrNull(attempt.metrics_delta, 'rapid_mlx_prefix_cache_tokens_saved_total'),
      evictions: metricSumByPrefixOrNull(attempt.metrics_delta, 'rapid_mlx_prefix_cache_evictions_total'),
    };
    const failures = [];
    for (const metric of ['hits', 'misses', 'tokens_saved', 'evictions']) {
      const value = observed[metric];
      const minimum = prefixExpectation[`min_${metric}`];
      const maximum = prefixExpectation[`max_${metric}`];
      if (value === null) failures.push(`${metric}=absent`);
      else {
        if (minimum !== undefined && value < minimum) failures.push(`${metric}=${value} < ${minimum}`);
        if (maximum !== undefined && value > maximum) failures.push(`${metric}=${value} > ${maximum}`);
      }
    }
    attempt.fidelity.prefix_cache = {
      expected: prefixExpectation,
      observed,
      passed: failures.length === 0,
      failures,
    };
    if (failures.length) {
      die(`${cell.id}/${attempt.phase} failed prefix-cache metric gate: ${failures.join('; ')}.`);
    }
  }
  if (attempt.fidelity.tool_call_observed === false || attempt.fidelity.tool_arguments_passed === false) {
    die(`${cell.id}/${attempt.phase} failed the initial tool-call fidelity gate.`);
  }
  for (const [index, item] of attempt.tool_trace.entries()) {
    if (item.fidelity.tool_call_observed === false || item.fidelity.tool_arguments_passed === false) {
      die(`${cell.id}/${attempt.phase} failed tool-trace step ${index + 1} fidelity.`);
    }
  }
  const method = cell.configuration?.speculative_method;
  if (method && method !== 'off') {
    const attempts = metricSumByPrefix(attempt.metrics_delta, 'rapid_mlx_spec_decode_attempts_total');
    const parks = metricSumByPrefix(attempt.metrics_delta, 'rapid_mlx_spec_decode_park_total');
    const rounds = metricSumByPrefix(attempt.metrics_delta, 'rapid_mlx_spec_decode_k_chosen_rounds_total');
    const engaged = attempts > 0 || parks > 0 || rounds > 0;
    // Recorded on both outcomes: an absent field would read as "not checked".
    attempt.fidelity.speculative_activity_observed = engaged;
    // A requalification cell asks whether the scheduler engages at all, so zero
    // activity is its answer, not a broken run. Record the finding and let the
    // caller's predicate decide; dying here would make a known-blocked build
    // indistinguishable from a broken harness.
    if (!engaged && cell.configuration?.speculative_zero_activity !== 'observed') {
      die(`${cell.id}/${attempt.phase} requested ${method} but observed zero speculative activity.`);
    }
  }
}

async function runManifest(manifest, selectedCells, serverPid = null) {
  if (manifest.schema_version !== 1) die('Manifest schema_version must be 1.');
  const runtime = manifest.runtime;
  if (!runtime?.base_url || !runtime?.metrics_path) die('runtime.base_url and runtime.metrics_path are required.');
  const health = await fetch(`${runtime.base_url}${runtime.health_path ?? '/health'}`);
  if (!health.ok) die(`Server health check failed: ${health.status}`);
  const cells = manifest.cells.filter((cell) => !selectedCells || selectedCells.includes(cell.id));
  if (!cells.length) die('No benchmark cells selected.');
  const results = [];
  for (const cell of cells) {
    if (!cell.id || !cell.model || !cell.workload?.target_words) die(`Malformed cell: ${cell.id ?? 'unknown'}`);
    prepareExactExtension(cell.workload, runtime, manifest.model);
    const sequence = cell.sequence ?? ['cold'];
    const attempts = [];
    let coldAttempt = null;
    for (const phase of sequence) {
      const isForkPhase = phase === 'fork' || phase.startsWith('fork-wide-');
      const lineageMatch = phase.match(/^root-(cold|prime|probe)-(\d+)$/);
      if (!['cold', 'repeat', 'extension', 'followup', 'fork'].includes(phase) && !isForkPhase && !lineageMatch) die(`Unsupported phase ${phase} in ${cell.id}`);
      if (phase === 'extension' && !cell.workload.extension_words && !cell.workload.exact_extension_text) die(`${cell.id} needs extension_words or exact_extension_tokens for extension.`);
      let requestOverride = null;
      if (phase === 'followup' || isForkPhase) {
        if (!coldAttempt?.conversation_assistant_content) die(`${cell.id} ${phase} requires a completed cold assistant turn.`);
        // Markerless by construction — the branch that makes `phaseIncludesMarkers(phase)`
        // false. Keep the two in step: a phase added here must be added there too, or its
        // receipts will record a recall score for markers that were never in the prompt.
        const baseRequest = await requestFor(cell, false);
        const priorMessages = [...baseRequest.messages, { role: 'assistant', content: coldAttempt.conversation_assistant_content }];
        const branchId = phase.replace('fork-wide-', '') || '1';
        const branchFixture = phase.startsWith('fork-wide-') && cell.workload.branch_workload === 'patch-review'
          ? branchReviewFixture(phase, cell.workload.extension_words ?? 0)
          : null;
        const suffix = branchFixture
          ? branchFixture.text
          : isForkPhase
            ? makeCorpus(cell.workload, cell.workload.extension_words ?? 0, false)
          : (cell.workload.exact_extension_text ?? makeCorpus(cell.workload, cell.workload.extension_words ?? 0, false));
        const instruction = phase === 'followup'
          ? 'Continue the coding task using the same project context. Inspect this follow-up note and answer concisely:'
          : branchFixture
            ? `Review alternative branch ${branchId} using the self-contained policy and patch below.`
            : 'Review an alternative follow-up for the same completed coding task. Answer concisely:';
        requestOverride = { ...baseRequest, messages: [...priorMessages, { role: 'user', content: `${instruction}\n\n${suffix}` }] };
      } else if (lineageMatch?.[1] === 'cold') {
        const fixturePhase = branchReviewFixturePhase(phase);
        const branchFixture = branchReviewFixture(fixturePhase, cell.workload.target_words ?? 0);
        const rootRequest = await requestFor(cell, false);
        requestOverride = {
          ...rootRequest,
          messages: [{ role: 'user', content: `Create independent conversation root ${lineageMatch[2]} by reviewing the policy and patch below.\n\n${branchFixture.text}` }],
        };
      } else if (lineageMatch) {
        const lineageId = lineageMatch[2];
        const parent = attempts.find((candidate) => candidate.phase === `root-cold-${lineageId}`);
        if (!parent?.conversation_assistant_content || !parent.conversation_request_messages) {
          die(`${cell.id} ${phase} requires completed root-cold-${lineageId}.`);
        }
        const fixturePhase = branchReviewFixturePhase(phase);
        const branchFixture = branchReviewFixture(fixturePhase, cell.workload.extension_words ?? 0);
        const lineageRequest = await requestFor(cell, false);
        const priorMessages = [
          ...parent.conversation_request_messages,
          { role: 'assistant', content: parent.conversation_assistant_content },
        ];
        const action = lineageMatch[1] === 'prime' ? 'Prime' : 'Revisit';
        requestOverride = {
          ...lineageRequest,
          messages: [
            ...priorMessages,
            { role: 'user', content: `${action} independent root ${lineageId} with the review below.\n\n${branchFixture.text}` },
          ],
        };
      }
      const attempt = await runAttempt(runtime.base_url, runtime.metrics_path, cell, phase, phase === 'extension', serverPid, requestOverride);
      assertAttemptQualified(cell, attempt);
      attempts.push(attempt);
      if (phase === 'cold') coldAttempt = attempt;
    }
    for (const attempt of attempts) {
      delete attempt.conversation_assistant_content;
      delete attempt.conversation_request_messages;
    }
    // trial/base_cell_id are backend-neutral repeat-measurement identity, not
    // Rapid-specific: without them a receipt from a counterbalanced run cannot
    // be grouped back to the cell it repeats.
    results.push({ id: cell.id, model: cell.model, target_tokens: cell.target_tokens, trial: cell.trial ?? null, base_cell_id: cell.base_cell_id ?? cell.id, configuration: cell.configuration, workload: cell.workload, attempts });
  }
  return {
    schema_version: 1,
    kind: 'model_runtime_benchmark_receipt',
    captured_at: new Date().toISOString(),
    benchmark: manifest.benchmark,
    runtime,
    hardware: manifest.hardware,
    model: manifest.model,
    cells: results,
    limitations: [
      'Each receipt proves only its pinned runtime/model/hardware/configuration envelope.',
      'Extrapolations in reports are mathematical estimates, never measured fit or fidelity claims.',
      'TurboQuant comparisons require cache-enabled repeat/extension cells; cold rows alone are dispatch controls.',
    ],
  };
}

function rowsFromReceipt(receipt) {
  return receipt.cells.flatMap((cell) => cell.attempts.map((attempt) => {
    const specAttempts = metricSumByPrefixOrNull(attempt.metrics_delta, 'rapid_mlx_spec_decode_attempts_total');
    const specAccepts = metricSumByPrefixOrNull(attempt.metrics_delta, 'rapid_mlx_spec_decode_accepts_total');
    const specParked = metricSumByPrefixOrNull(attempt.metrics_delta, 'rapid_mlx_spec_decode_park_total');
    const specRounds = metricSumByPrefixOrNull(attempt.metrics_delta, 'rapid_mlx_spec_decode_k_chosen_rounds_total');
    const specKHistogram = Object.fromEntries(
      Object.entries(attempt.metrics_delta)
        .filter(([name, value]) => name.startsWith('rapid_mlx_spec_decode_k_chosen_total') && value > 0)
        .map(([name, value]) => [name.match(/k="(\d+)"/)?.[1] ?? 'unknown', value]),
    );
    return {
      receipt,
      cell,
      attempt,
      targetTokens: (cell.target_tokens ?? Number(cell.id.match(/(?:^|-)(\d{4,6})(?:-|$)/)?.[1])) || null,
      promptTokens: attempt.response.usage?.prompt_tokens ?? null,
      ttft: attempt.response.round_trip_first_token_ms ?? attempt.response.first_token_ms,
      totalMs: attempt.response.round_trip_total_ms ?? attempt.response.total_ms,
      tg: attempt.response.round_trip_generation_tokens_per_second ?? attempt.response.generation_tokens_per_second,
      serverPeak: attempt.metrics_after.rapid_mlx_metal_peak_memory_bytes ?? null,
      newServerPeak: Math.max(0,
        (attempt.metrics_after.rapid_mlx_metal_peak_memory_bytes ?? 0)
        - (attempt.metrics_before.rapid_mlx_metal_peak_memory_bytes ?? 0)),
      peak: receipt.runtime?.fresh_server_per_cell
        ? attempt.metrics_after.rapid_mlx_metal_peak_memory_bytes ?? null
        : null,
      compressed: metricSumByPrefixOrNull(attempt.metrics_delta, 'rapid_mlx_pflash_compressed_tokens_total'),
      prefixHits: metricSumByPrefixOrNull(attempt.metrics_delta, 'rapid_mlx_prefix_cache_hits_total'),
      prefixTokensSaved: metricSumByPrefixOrNull(attempt.metrics_delta, 'rapid_mlx_prefix_cache_tokens_saved_total'),
      serverRss: attempt.os_memory_after?.server_rss_bytes ?? null,
      specAttempts,
      specAccepts,
      specTokensSaved: metricSumByPrefixOrNull(attempt.metrics_delta, 'rapid_mlx_spec_decode_tokens_saved_total'),
      specParked,
      specRounds,
      specKHistogram,
      // The gauge is cumulative for the server lifetime. Prefer labeled
      // counter deltas so repeat/extension phases report only their window.
      specAcceptRatio: specAttempts > 0
        ? (specAccepts ?? 0) / specAttempts
        : metricSumByPrefixOrNull(attempt.metrics_after, 'rapid_mlx_spec_decode_accept_ratio'),
      specParkRate: specRounds > 0
        ? (specParked ?? 0) / specRounds
        : (specAttempts > 0 && specParked === 0 ? 0 : null),
    };
  }));
}

function rowQualificationFailure(row) {
  const fidelity = row.attempt.fidelity;
  if (!fidelity.request_succeeded) return fidelity.failure_reason ?? 'request_failed';
  if (fidelity.completion_length_passed === false) return 'minimum_completion_tokens_not_met';
  if (fidelity.final_answer_passed === false) return 'final_answer_fidelity_failed';
  if (fidelity.prefix_cache?.passed === false) return 'prefix_cache_metric_gate_failed';
  if (fidelity.marker_recall && row.cell.workload.minimum_marker_recall_rate !== undefined
      && fidelity.marker_recall.recall_rate < row.cell.workload.minimum_marker_recall_rate) {
    return 'marker_recall_floor_not_met';
  }
  if (fidelity.tool_call_observed === false || fidelity.tool_arguments_passed === false) return 'initial_tool_fidelity_failed';
  if (row.attempt.tool_trace?.some((item) => item.fidelity.tool_call_observed === false || item.fidelity.tool_arguments_passed === false)) {
    return 'tool_trace_fidelity_failed';
  }
  const method = row.cell.configuration?.speculative_method;
  if (method && method !== 'off' && !(row.specAttempts > 0 || row.specRounds > 0 || row.specParked > 0)) {
    return 'zero_speculative_activity';
  }
  return null;
}

function fidelityLabel(attempt) {
  if (attempt.fidelity.expected_final_answer !== null && attempt.fidelity.expected_final_answer !== undefined) {
    return attempt.fidelity.final_answer_passed
      ? `exact: ${attempt.fidelity.expected_final_answer}`
      : `failed exact: ${attempt.fidelity.expected_final_answer}`;
  }
  const recall = attempt.fidelity.marker_recall;
  return !recall
    ? 'not scored'
    : `${recall.markers_correct}/${recall.markers_expected} (${(recall.recall_rate * 100).toFixed(0)}%)`;
}

function linearPredict(rows, target, field) {
  const usable = rows.filter((row) => rowQualificationFailure(row) === null && Number.isFinite(row.promptTokens) && Number.isFinite(row[field]));
  if (usable.length < 2) return null;
  const meanX = usable.reduce((sum, row) => sum + row.promptTokens, 0) / usable.length;
  const meanY = usable.reduce((sum, row) => sum + row[field], 0) / usable.length;
  const denominator = usable.reduce((sum, row) => sum + (row.promptTokens - meanX) ** 2, 0);
  if (!denominator) return null;
  const slope = usable.reduce((sum, row) => sum + (row.promptTokens - meanX) * (row[field] - meanY), 0) / denominator;
  return Math.max(0, meanY + slope * (target - meanX));
}

function markdownReport(receipts) {
  const rows = receipts.flatMap(rowsFromReceipt);
  const output = ['# Model Runtime Benchmark Report', '', 'Measured rows only; estimates are explicitly labelled below.', ''];
  output.push('| Model | Cell | Phase | Raw prompt tokens | Raw PP tok/s | TTFT | TG tok/s | Server RSS after | Native peak after | Prefix hits | Prefix tokens saved | PFlash compressed | Spec attempts | Spec accepts | Spec accept ratio | Spec tokens saved | Spec park rate | Request | Fidelity |');
  output.push('|---|---|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---|---|');
  for (const row of rows) {
    const fidelity = fidelityLabel(row.attempt);
    const qualificationFailure = rowQualificationFailure(row);
    const request = qualificationFailure === null ? 'success' : `failed: ${qualificationFailure}`;
    output.push(`| ${row.receipt.model?.hf_repo_id ?? row.receipt.model?.filename ?? row.cell.model} | ${row.cell.id} | ${row.attempt.phase} | ${row.promptTokens ?? 'n/a'} | ${row.attempt.response.raw_prompt_tokens_per_second ?? 'n/a'} | ${row.ttft ?? 'n/a'} ms | ${row.tg ?? 'n/a'} | ${row.serverRss ? `${(row.serverRss / 1e9).toFixed(2)} GB` : 'n/a'} | ${row.serverPeak ? `${(row.serverPeak / 1e9).toFixed(2)} GB` : 'n/a'} | ${row.prefixHits ?? 'n/a'} | ${row.prefixTokensSaved ?? 'n/a'} | ${row.compressed ?? 'n/a'} | ${row.specAttempts ?? 'n/a'} | ${row.specAccepts ?? 'n/a'} | ${row.specAcceptRatio === null ? 'n/a' : `${(row.specAcceptRatio * 100).toFixed(1)}%`} | ${row.specTokensSaved ?? 'n/a'} | ${row.specParkRate === null ? 'n/a' : `${(row.specParkRate * 100).toFixed(1)}%`} | ${request} | ${fidelity} |`);
  }
  const speculativeRows = rows.filter((row) => {
    const method = row.cell.configuration.speculative_method;
    return Boolean(method) && method !== 'off' && rowQualificationFailure(row) === null;
  });
  const pairedSpeculativeRows = speculativeRows.flatMap((row) => {
    const config = row.cell.configuration;
    const baselines = rows.filter((candidate) => (
      rowQualificationFailure(candidate) === null
      && candidate.receipt.model?.hf_repo_id === row.receipt.model?.hf_repo_id
      && candidate.receipt.model?.revision === row.receipt.model?.revision
      && candidate.receipt.model?.config_sha256 === row.receipt.model?.config_sha256
      && candidate.receipt.runtime?.backend === row.receipt.runtime?.backend
      && candidate.receipt.runtime?.version === row.receipt.runtime?.version
      && JSON.stringify(candidate.receipt.hardware ?? null) === JSON.stringify(row.receipt.hardware ?? null)
      && candidate.targetTokens === row.targetTokens
      && candidate.attempt.phase === row.attempt.phase
      && candidate.attempt.request?.message_sha256 === row.attempt.request?.message_sha256
      && candidate.attempt.request?.max_tokens === row.attempt.request?.max_tokens
      && candidate.cell.configuration.speculative_method === 'off'
      && candidate.cell.configuration.kv_cache_dtype_requested === config.kv_cache_dtype_requested
      && candidate.cell.configuration.prefill_step_size === config.prefill_step_size
      && candidate.cell.configuration.prefix_cache === config.prefix_cache
      && candidate.cell.configuration.speculative_workload === config.speculative_workload
      && (candidate.cell.configuration.trial_id ?? null) === (config.trial_id ?? null)
    ));
    return baselines.length === 1 ? [{ row, baseline: baselines[0] }] : [];
  });
  if (pairedSpeculativeRows.length) {
    output.push('', '## Paired speculative throughput', '');
    output.push('Each row compares a speculative cell with the same model, context target, workload, phase, KV dtype, prefill step, and cache setting against its non-speculative control.', '');
    output.push('| Model | Context target | Workload | Phase | Sidecar role | Configured max K | Controller mode | Observed K histogram | Off TG tok/s | Spec TG tok/s | TG change | TTFT change | End-to-end change | Accept ratio | Park rate | Tokens saved |');
    output.push('|---|---:|---|---|---|---:|---|---|---:|---:|---:|---:|---:|---:|---:|---:|');
    for (const { row, baseline } of pairedSpeculativeRows) {
      const config = row.cell.configuration;
      const offTg = baseline.tg;
      const change = Number.isFinite(offTg) && Number.isFinite(row.tg) && offTg > 0
        ? ((row.tg / offTg) - 1) * 100
        : null;
      const ttftChange = Number.isFinite(baseline.ttft) && Number.isFinite(row.ttft) && baseline.ttft > 0
        ? ((row.ttft / baseline.ttft) - 1) * 100
        : null;
      const endToEndChange = Number.isFinite(baseline.totalMs) && Number.isFinite(row.totalMs) && row.totalMs > 0
        ? ((baseline.totalMs / row.totalMs) - 1) * 100
        : null;
      const kHistogram = Object.entries(row.specKHistogram)
        .map(([k, rounds]) => `K${k}:${rounds}`)
        .join(', ') || (config.speculative_disable_auto_k ? 'fixed K1 (controller bypassed)' : 'n/a');
      output.push(`| ${row.receipt.model?.hf_repo_id ?? row.cell.model} | ${row.targetTokens ?? 'n/a'} | ${config.speculative_workload ?? 'unspecified'} | ${row.attempt.phase} | ${config.speculative_role ?? 'subject'} | ${config.num_speculative_tokens ?? 'n/a'} | ${config.speculative_disable_auto_k ? 'fixed K1' : 'auto'} | ${kHistogram} | ${offTg ?? 'n/a'} | ${row.tg ?? 'n/a'} | ${change === null ? 'n/a' : `${change >= 0 ? '+' : ''}${change.toFixed(1)}%`} | ${ttftChange === null ? 'n/a' : `${ttftChange >= 0 ? '+' : ''}${ttftChange.toFixed(1)}%`} | ${endToEndChange === null ? 'n/a' : `${endToEndChange >= 0 ? '+' : ''}${endToEndChange.toFixed(1)}%`} | ${row.specAcceptRatio === null ? 'n/a' : `${(row.specAcceptRatio * 100).toFixed(1)}%`} | ${row.specParkRate === null ? 'n/a' : `${(row.specParkRate * 100).toFixed(1)}%`} | ${row.specTokensSaved ?? 'n/a'} |`);
    }
  }
  const targets = [160000, 200000];
  const groups = new Map();
  for (const row of rows) {
    const key = JSON.stringify({ model: row.receipt.model?.hf_repo_id ?? row.cell.model, configuration: row.cell.configuration, phase: row.attempt.phase });
    groups.set(key, [...(groups.get(key) ?? []), row]);
  }
  output.push('', '## Linear extrapolations (not measurements)', '', 'These are same-configuration straight-line estimates from at least two measured rows. They do not establish fit, correctness, or recommendation safety.', '');
  output.push('| Model/configuration | Target context | Estimated TTFT | Estimated TG tok/s | Estimated Metal peak |');
  output.push('|---|---:|---:|---:|---:|');
  for (const [key, group] of groups) {
    const metadata = JSON.parse(key);
    // Suppress extrapolation once recall drops below majority-correct rather than
    // requiring a perfect score, since graduated recall is expected to vary with
    // sampling/temperature even on a healthy configuration.
    const failedFidelity = group.find((row) => (row.attempt.fidelity.marker_recall?.recall_rate ?? 1) < 0.5);
    for (const target of targets) {
      if (failedFidelity) {
        output.push(`| ${metadata.model} / ${JSON.stringify(metadata.configuration)} | ${target.toLocaleString()} | suppressed | suppressed | suppressed: fidelity failed at ${failedFidelity.promptTokens ?? 'unknown'} tokens |`);
        continue;
      }
      const ttft = linearPredict(group, target, 'ttft');
      const tg = linearPredict(group, target, 'tg');
      const peak = linearPredict(group, target, 'peak');
      if (ttft === null && tg === null && peak === null) continue;
      output.push(`| ${metadata.model} / ${JSON.stringify(metadata.configuration)} | ${target.toLocaleString()} | ${ttft === null ? 'insufficient data' : `${ttft.toFixed(0)} ms`} | ${tg === null ? 'insufficient data' : tg.toFixed(2)} | ${peak === null ? 'insufficient data' : `${(peak / 1e9).toFixed(2)} GB`} |`);
    }
  }
  return `${output.join('\n')}\n`;
}

// Rapid-MLX has no batch-size flag; --prefill-step-size is its per-chunk
// prompt-processing width and stands in for llama.cpp's -ub/--ubatch-size so
// the two backends' rows can be sorted and read side by side.
function batchStepLabel(configuration) {
  if (configuration?.ubatch_size !== undefined) return `ubatch ${configuration.ubatch_size}`;
  if (configuration?.prefill_step_size !== undefined) return `prefill-step ${configuration.prefill_step_size}`;
  return 'n/a';
}

async function loadReceiptsFromDir(dir) {
  const entries = await readdir(dir);
  const files = entries.filter((name) => name.endsWith('.json') && name !== 'suite-index.json').sort();
  return Promise.all(files.map(async (name) => JSON.parse(await readFile(join(dir, name), 'utf8'))));
}

function compareMarkdownReport(receipts) {
  const rows = receipts.flatMap(rowsFromReceipt);
  rows.sort((a, b) => {
    const backendA = a.receipt.runtime?.backend ?? '';
    const backendB = b.receipt.runtime?.backend ?? '';
    if (backendA !== backendB) return backendA.localeCompare(backendB);
    const stepA = batchStepLabel(a.cell.configuration);
    const stepB = batchStepLabel(b.cell.configuration);
    if (stepA !== stepB) return stepA.localeCompare(stepB);
    return (a.promptTokens ?? 0) - (b.promptTokens ?? 0);
  });
  const output = [
    '# Cross-Backend Benchmark Comparison', '',
    'Measured rows only, sorted by backend, then batch/step, then prompt size.', '',
    '| Backend | Batch/Step | Model | Prompt tokens | Raw PP tok/s | TTFT | TG tok/s | Completion tokens | Fidelity |',
    '|---|---|---|---:|---:|---:|---:|---:|---|',
  ];
  for (const row of rows) {
    const fidelity = row.attempt.fidelity.request_succeeded
      ? fidelityLabel(row.attempt)
      : `failed: ${row.attempt.fidelity.failure_reason ?? 'unknown'}`;
    const backend = row.receipt.runtime?.backend ?? 'unknown';
    const model = row.receipt.model?.hf_repo_id ?? row.receipt.model?.filename ?? row.cell.model;
    const completionTokens = row.attempt.response.usage?.completion_tokens ?? 'n/a';
    output.push(`| ${backend} | ${batchStepLabel(row.cell.configuration)} | ${model} | ${row.promptTokens ?? 'n/a'} | ${row.attempt.response.raw_prompt_tokens_per_second ?? 'n/a'} | ${row.ttft ?? 'n/a'} ms | ${row.tg ?? 'n/a'} | ${completionTokens} | ${fidelity} |`);
  }
  return `${output.join('\n')}\n`;
}

const { command, options } = parseArgs(process.argv);
if (command === 'self-test') {
  runSelfTest();
} else if (command === 'run') {
  const manifest = JSON.parse(await readFile(options.manifest, 'utf8'));
  if (options.baseUrl) manifest.runtime.base_url = options.baseUrl;
  const receipt = await runManifest(manifest, options.cells, options.serverPid ?? null);
  await writeFile(options.out, `${JSON.stringify(receipt, null, 2)}\n`);
  console.log(`Wrote ${options.out} (${receipt.cells.length} cells).`);
} else if (command === 'compare') {
  const receiptSets = await Promise.all(options.dirs.map(loadReceiptsFromDir));
  const receipts = receiptSets.flat();
  await writeFile(options.out, compareMarkdownReport(receipts));
  console.log(`Wrote ${options.out} (${receipts.length} receipts across ${options.dirs.length} directories).`);
} else {
  const receipts = await Promise.all(options.inputs.map(async (path) => JSON.parse(await readFile(path, 'utf8'))));
  await writeFile(options.out, markdownReport(receipts));
  console.log(`Wrote ${options.out} (${receipts.length} receipts).`);
}
