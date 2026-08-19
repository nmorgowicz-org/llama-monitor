#!/usr/bin/env node
import { createHash } from 'node:crypto';
import { mkdir, readFile, writeFile } from 'node:fs/promises';
import { dirname, resolve } from 'node:path';

// Deliberately explicit: no config, secrets, generated assets, receipts, or
// arbitrary working-tree discovery may enter a reproducible cache fixture.
const FILES = [
  'src/agent.rs',
  'src/chat_storage.rs',
  'src/main.rs',
  'docs/plans/20260718-final_rapidmlx_followups_execution.md',
  'docs/plans/20260726-phase6_rapidmlx_cache_benchmarking.md',
];
const out = resolve(process.argv[2] ?? 'tests/fixtures/calibration/workspace-cache/project-pack.json');
const sha256 = (value) => createHash('sha256').update(value).digest('hex');
const entries = [];
for (const path of FILES) {
  const normalized = (await readFile(resolve(path), 'utf8')).replace(/\r\n/g, '\n');
  entries.push({ path, bytes: Buffer.byteLength(normalized), sha256: sha256(normalized), text: normalized });
}
const corpus = entries.map(({ path, text }) => `===== ${path} =====\n${text}`).join('\n\n');
const manifest = {
  schema_version: 1,
  purpose: 'Frozen allowlisted coding-workspace corpus for cache benchmark scenarios.',
  files: entries.map(({ path, bytes, sha256: hash }) => ({ path, bytes, sha256: hash })),
  corpus_sha256: sha256(corpus),
  corpus_bytes: Buffer.byteLength(corpus),
  corpus,
};
await mkdir(dirname(out), { recursive: true });
await writeFile(out, `${JSON.stringify(manifest, null, 2)}\n`);
process.stdout.write(`${out}\n${manifest.corpus_bytes} bytes\n${manifest.corpus_sha256}\n`);
