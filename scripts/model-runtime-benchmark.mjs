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
  if (!['run', 'report', 'compare'].includes(command)) die('Use run, report, or compare.');
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

function workloadText(workload, extension = false) {
  const reference = extension && workload.exact_extension_text
    ? `${makeCorpus(workload, workload.target_words)}\n\n${workload.exact_extension_text}`
    : workload.reference_text ?? (extension
    ? `${makeCorpus(workload, workload.target_words)}\n\n${makeCorpus(workload, workload.extension_words, false)}`
    : makeCorpus(workload, workload.target_words));
  const instruction = workload.instruction ?? (workload.markers?.length
    ? 'List every CHECK_* constant name and its numeric value found in the reference below, one per line as NAME=VALUE. Output nothing else.'
    : `Read the reference, then output exactly ${workload.output_words ?? 96} ordinary English words separated by spaces, with no explanation or punctuation.`);
  return `${instruction}\n\n${reference}`;
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
      const token = delta?.content ?? delta?.reasoning_content;
      if (token) {
        if (firstTokenAt === null) firstTokenAt = performance.now();
        completion += token;
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
    // Speculative decoding claims to be lossless at temperature 0: the
    // accepted token stream must be identical to what the target model would
    // have produced alone. Checking that needs the whole completion, but the
    // full text is dropped after scoring (see above), so hash it here. The
    // digest survives into the receipt at fixed size and retains no prompt-like
    // material, and an off-vs-MTP comparison is then an equality test.
    completion_sha256: createHash('sha256').update(completion).digest('hex'),
    completion_characters: completion.length,
    server_errors: errors,
    tool_calls: mergedToolCalls,
  };
}

function isSubset(expected, actual) {
  if (expected === actual) return true;
  if (!expected || !actual || typeof expected !== 'object' || typeof actual !== 'object') return false;
  return Object.entries(expected).every(([key, value]) => isSubset(value, actual[key]));
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

function scoreFidelity(workload, response) {
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
  const requestSucceeded = response.server_errors.length === 0
    && (response.usage?.prompt_tokens ?? 0) > 0
    && ((response.usage?.completion_tokens ?? 0) > 0 || response.tool_calls.length > 0);
  const failureReason = requestSucceeded
    ? null
    : response.server_errors.length > 0
      ? 'server_error_event'
      : (response.usage?.prompt_tokens ?? 0) === 0
        ? 'empty_or_missing_prompt_usage'
        : 'empty_completion_without_tool_call';
  return {
    request_succeeded: requestSucceeded,
    failure_reason: failureReason,
    marker_recall: scoreMarkerRecall(
      workload.markers,
      response.completion_text ?? response.completion_preview,
    ),
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
    temperature: workload.temperature ?? 0,
    max_tokens: workload.max_tokens ?? 128,
    messages: [{ role: 'user', content }],
  };
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
    temperature: request.temperature,
    // A receipt that records only temperature cannot be read back as "these
    // are the model's recommended settings" or "these are not".
    top_p: request.top_p ?? null,
    top_k: request.top_k ?? null,
    min_p: request.min_p ?? null,
    presence_penalty: request.presence_penalty ?? null,
    repetition_penalty: request.repetition_penalty ?? null,
    seed: request.seed ?? null,
    sampling_mode: (request.temperature ?? 0) === 0 ? 'greedy' : 'sampled',
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
  const fidelity = scoreFidelity(cell.workload, response);
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
    if (attempts <= 0 && parks <= 0 && rounds <= 0) {
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
      if (!['cold', 'repeat', 'extension', 'followup', 'fork'].includes(phase)) die(`Unsupported phase ${phase} in ${cell.id}`);
      if (phase === 'extension' && !cell.workload.extension_words && !cell.workload.exact_extension_text) die(`${cell.id} needs extension_words or exact_extension_tokens for extension.`);
      let requestOverride = null;
      if (phase === 'followup' || phase === 'fork') {
        if (!coldAttempt?.conversation_assistant_content) die(`${cell.id} ${phase} requires a completed cold assistant turn.`);
        const baseRequest = await requestFor(cell, false);
        const priorMessages = [...baseRequest.messages, { role: 'assistant', content: coldAttempt.conversation_assistant_content }];
        const suffix = cell.workload.exact_extension_text ?? makeCorpus(cell.workload, cell.workload.extension_words ?? 0, false);
        const instruction = phase === 'followup'
          ? 'Continue the coding task using the same project context. Inspect this follow-up note and answer concisely:'
          : 'Review an alternative follow-up for the same completed coding task. Answer concisely:';
        requestOverride = { ...baseRequest, messages: [...priorMessages, { role: 'user', content: `${instruction}\n\n${suffix}` }] };
      }
      const attempt = await runAttempt(runtime.base_url, runtime.metrics_path, cell, phase, phase === 'extension', serverPid, requestOverride);
      assertAttemptQualified(cell, attempt);
      attempts.push(attempt);
      if (phase === 'cold') coldAttempt = attempt;
    }
    for (const attempt of attempts) delete attempt.conversation_assistant_content;
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
    const recall = row.attempt.fidelity.marker_recall;
    const fidelity = recall === null
      ? 'not scored'
      : `${recall.markers_correct}/${recall.markers_expected} (${(recall.recall_rate * 100).toFixed(0)}%)`;
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
    const recall = row.attempt.fidelity.marker_recall;
    const fidelity = recall === null
      ? (row.attempt.fidelity.request_succeeded ? 'success' : `failed: ${row.attempt.fidelity.failure_reason ?? 'unknown'}`)
      : `${recall.markers_correct}/${recall.markers_expected} (${(recall.recall_rate * 100).toFixed(0)}%)`;
    const backend = row.receipt.runtime?.backend ?? 'unknown';
    const model = row.receipt.model?.hf_repo_id ?? row.receipt.model?.filename ?? row.cell.model;
    const completionTokens = row.attempt.response.usage?.completion_tokens ?? 'n/a';
    output.push(`| ${backend} | ${batchStepLabel(row.cell.configuration)} | ${model} | ${row.promptTokens ?? 'n/a'} | ${row.attempt.response.raw_prompt_tokens_per_second ?? 'n/a'} | ${row.ttft ?? 'n/a'} ms | ${row.tg ?? 'n/a'} | ${completionTokens} | ${fidelity} |`);
  }
  return `${output.join('\n')}\n`;
}

const { command, options } = parseArgs(process.argv);
if (command === 'run') {
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
