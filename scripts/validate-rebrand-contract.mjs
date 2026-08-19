#!/usr/bin/env node

/** Fail-closed checks for the source-side 2.0 identity contract. */
import fs from 'node:fs';
import path from 'node:path';

const repo = process.cwd();
const args = new Set(process.argv.slice(2));
const expectedIndex = process.argv.indexOf('--expected-version');
const expectedVersion = expectedIndex >= 0 ? process.argv[expectedIndex + 1] : null;

function read(relative) {
    return fs.readFileSync(path.join(repo, relative), 'utf8');
}

function fail(message) {
    console.error(`FAIL: ${message}`);
    process.exitCode = 1;
}

const errors = [];
const identity = JSON.parse(read('docs/reference/identity-contract.json'));
const cargo = read('Cargo.toml');
const release = read('.github/workflows/release.yml');
const ci = read('.github/workflows/ci.yml');

if (identity.product?.name !== 'Local LLM Foundry') errors.push('identity contract product name is not canonical');
if (identity.product?.slug !== 'local-llm-foundry') errors.push('identity contract slug is not canonical');
if (identity.public_identity?.cargo_package !== 'local-llm-foundry') errors.push('identity contract package is not canonical');
if (!/^name\s*=\s*"local-llm-foundry"/m.test(cargo)) errors.push('Cargo package name is not local-llm-foundry');
if (!/^name\s*=\s*"local-llm-foundry"/m.test(cargo)) errors.push('canonical binary is missing');
if (!/^name\s*=\s*"llama-monitor"/m.test(cargo)) errors.push('legacy binary bridge is missing');
if (!read('static/icon.svg').includes('Token Ingot')) errors.push('static/icon.svg is not the approved Token Ingot mark');
for (const asset of [
    'local-llm-foundry-linux-x86_64',
    'local-llm-foundry-linux-aarch64',
    'local-llm-foundry-windows-x86_64.zip',
    'local-llm-foundry-macos-aarch64.tar.gz',
]) {
    if (!release.includes(asset)) errors.push(`release workflow does not publish ${asset}`);
}
if (!release.includes('llama-monitor-linux-x86_64') || !release.includes('llama-monitor-windows-x86_64.zip')) {
    errors.push('release workflow does not publish the complete legacy bridge');
}
if (!ci.includes(".github/workflows/release.yml")) errors.push('CI release path filter omits release workflow changes');
if (!ci.includes('target/release/local-llm-foundry')) errors.push('CI UI smoke path is not canonical');

const versionMatch = cargo.match(/^version\s*=\s*"([^"]+)"/m);
const version = versionMatch?.[1];
if (!version) errors.push('Cargo version is missing');
if (version === '1.9.0') errors.push('rebrand cannot ship as 1.9.0; the bridge release is 2.0.0');
if (expectedVersion && version !== expectedVersion) errors.push(`expected Cargo version ${expectedVersion}, found ${version}`);
if (args.has('--require-2-0') && version !== '2.0.0') errors.push(`2.0 gate requires Cargo version 2.0.0, found ${version}`);

if (errors.length) {
    for (const error of errors) fail(error);
    process.exit(1);
}
console.log(`PASS: source identity contract (${version})`);
