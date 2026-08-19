#!/usr/bin/env node
/**
 * Run one deterministic streaming context probe against a running Rapid-MLX
 * OpenAI-compatible server. It intentionally does not start the server: the
 * caller owns the exact loader, cache, KV, TurboQuant, and PFlash arguments.
 *
 * Example:
 *   node scripts/rapid_mlx_context_probe.mjs \
 *     --base-url http://127.0.0.1:18081 --model org/model \
 *     --target-words 7000 --max-tokens 128 --label cold-8k
 */
import { readFile } from 'node:fs/promises';
import { performance } from 'node:perf_hooks';

function usage() {
  console.error(
    'Usage: rapid_mlx_context_probe.mjs --base-url URL --model ID --target-words N ' +
      '[--max-tokens N] [--image PATH] [--label TEXT] [--temperature N]',
  );
  process.exit(2);
}

function parseArgs(argv) {
  const options = { maxTokens: 128, temperature: 0, label: null, image: null };
  for (let index = 2; index < argv.length; index += 1) {
    const arg = argv[index];
    const value = argv[index + 1];
    if (!arg.startsWith('--') || value === undefined) usage();
    index += 1;
    if (arg === '--base-url') options.baseUrl = value.replace(/\/$/, '');
    else if (arg === '--model') options.model = value;
    else if (arg === '--target-words') options.targetWords = Number.parseInt(value, 10);
    else if (arg === '--max-tokens') options.maxTokens = Number.parseInt(value, 10);
    else if (arg === '--temperature') options.temperature = Number.parseFloat(value);
    else if (arg === '--image') options.image = value;
    else if (arg === '--label') options.label = value;
    else usage();
  }
  if (!options.baseUrl || !options.model || !Number.isInteger(options.targetWords) || options.targetWords < 1) {
    usage();
  }
  return options;
}

const WORDS = [
  'architecture', 'boundary', 'cache', 'context', 'deterministic', 'evidence', 'function',
  'generation', 'implementation', 'latency', 'memory', 'model', 'observation', 'performance',
  'quality', 'request', 'response', 'runtime', 'sequence', 'system', 'token', 'validation',
  'workflow', 'reliable', 'structured', 'measure', 'repeatable', 'independent', 'analysis',
];

function makePrompt(targetWords) {
  const words = [];
  for (let index = 0; index < targetWords; index += 1) words.push(WORDS[index % WORDS.length]);
  return [
    'Read the following deterministic reference material. Then output exactly 96 ordinary English words, separated by spaces, with no explanation or punctuation.',
    words.join(' '),
  ].join('\n\n');
}

function metricSnapshot(text) {
  const selected = {};
  for (const line of text.split('\n')) {
    if (!line || line.startsWith('#')) continue;
    const match = line.match(/^([A-Za-z_:][A-Za-z0-9_:]*)(?:\{[^}]*\})?\s+([-+0-9.eE]+)$/);
    if (!match) continue;
    const [, name, value] = match;
    if (/^(rapid_mlx|metal|process)_/.test(name)) selected[name] = Number(value);
  }
  return selected;
}

async function metrics(baseUrl) {
  const response = await fetch(`${baseUrl}/metrics`);
  if (!response.ok) throw new Error(`GET /metrics failed: ${response.status}`);
  return metricSnapshot(await response.text());
}

function contentFor(prompt, imageDataUri) {
  if (!imageDataUri) return prompt;
  return [
    { type: 'text', text: prompt },
    { type: 'image_url', image_url: { url: imageDataUri } },
  ];
}

async function imageDataUri(path) {
  if (!path) return null;
  const bytes = await readFile(path);
  return `data:image/png;base64,${bytes.toString('base64')}`;
}

async function run(options) {
  const prompt = makePrompt(options.targetWords);
  const image = await imageDataUri(options.image);
  const before = await metrics(options.baseUrl);
  const request = {
    model: options.model,
    temperature: options.temperature,
    max_tokens: options.maxTokens,
    stream: true,
    stream_options: { include_usage: true },
    messages: [{ role: 'user', content: contentFor(prompt, image) }],
  };
  const startedAt = performance.now();
  const response = await fetch(`${options.baseUrl}/v1/chat/completions`, {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify(request),
  });
  if (!response.ok || !response.body) throw new Error(`chat completion failed: ${response.status} ${await response.text()}`);

  const reader = response.body.getReader();
  const decoder = new TextDecoder();
  let buffer = '';
  let firstTokenAt = null;
  let usage = null;
  let completion = '';
  const serverErrors = [];
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
      if (event.error) serverErrors.push(event.error);
      if (event.usage) usage = event.usage;
      const delta = event.choices?.[0]?.delta;
      const token = delta?.content ?? delta?.reasoning_content;
      if (token) {
        if (firstTokenAt === null) firstTokenAt = performance.now();
        completion += token;
      }
    }
  }
  const endedAt = performance.now();
  const after = await metrics(options.baseUrl);
  const elapsedMs = endedAt - startedAt;
  const firstTokenMs = firstTokenAt === null ? null : firstTokenAt - startedAt;
  const completionMs = firstTokenAt === null ? null : endedAt - firstTokenAt;
  const completionTokens = usage?.completion_tokens ?? null;
  const result = {
    schema_version: 1,
    kind: 'rapid_mlx_context_probe',
    label: options.label,
    request: {
      model: options.model,
      target_words: options.targetWords,
      prompt_characters: prompt.length,
      max_tokens: options.maxTokens,
      temperature: options.temperature,
      has_image: Boolean(image),
    },
    response: {
      usage,
      first_token_ms: firstTokenMs === null ? null : Math.round(firstTokenMs * 100) / 100,
      total_ms: Math.round(elapsedMs * 100) / 100,
      generation_ms: completionMs === null ? null : Math.round(completionMs * 100) / 100,
      generation_tokens_per_second:
        completionTokens && completionMs && completionMs > 0
          ? Math.round((completionTokens * 1000 * 100) / completionMs) / 100
          : null,
      completion_preview: completion.slice(0, 200),
      server_errors: serverErrors,
    },
    metrics_before: before,
    metrics_after: after,
  };
  console.log(JSON.stringify(result));
}

run(parseArgs(process.argv)).catch((error) => {
  console.error(error.stack || String(error));
  process.exit(1);
});
