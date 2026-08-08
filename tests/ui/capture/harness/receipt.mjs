// Per-scenario capture contract and receipt support. A scenario with a contract
// must produce every declared final filename in its assigned artifact category.
import fs from 'fs';
import { join } from 'path';
import { currentArtifactsDir } from './paths.mjs';

let active = null;

export function beginCaptureReceipt({ scenario, category, runtime, intent, expectedOutputs }) {
    if (!intent || typeof intent !== 'string') throw new Error(`Capture scenario "${scenario}" is missing INTENT`);
    if (!Array.isArray(expectedOutputs) || expectedOutputs.length === 0) {
        throw new Error(`Capture scenario "${scenario}" has no expected outputs`);
    }
    if (new Set(expectedOutputs).size !== expectedOutputs.length) {
        throw new Error(`Capture scenario "${scenario}" declares duplicate expected outputs`);
    }
    const artifactDir = currentArtifactsDir();
    for (const filename of expectedOutputs) fs.rmSync(join(artifactDir, filename), { force: true });
    active = { scenario, category, runtime, intent, expectedOutputs: new Set(expectedOutputs), produced: [] };
}

export function recordCapture(filename, viewport) {
    if (!active) return;
    active.produced.push({ filename, viewport });
}

export function recordArtifact(filename, viewport = null) {
    recordCapture(filename, viewport);
}

export function finishCaptureReceipt() {
    if (!active) return null;
    const receipt = active;
    active = null;
    const produced = receipt.produced.map(({ filename }) => filename);
    const unexpected = produced.filter(filename => !receipt.expectedOutputs.has(filename));
    const missing = [...receipt.expectedOutputs].filter(filename => !produced.includes(filename));
    if (unexpected.length || missing.length) {
        throw new Error(`Capture contract failed for "${receipt.scenario}": missing=[${missing.join(', ')}] unexpected=[${unexpected.join(', ')}]`);
    }
    for (const { filename } of receipt.produced) {
        const path = join(currentArtifactsDir(), filename);
        if (!fs.existsSync(path)) throw new Error(`Capture contract failed for "${receipt.scenario}": ${filename} was not written`);
    }
    for (const { filename, viewport } of receipt.produced) {
        const width = viewport?.width ?? 0;
        const height = viewport?.height ?? 0;
        const allowed = (width === 1440 && height === 900)
            || (width === 1280 && (height === 900 || height === 1400))
            || (width === 430 && height === 900);
        if (!allowed) throw new Error(`Capture contract failed for "${receipt.scenario}": ${filename} uses unrealistic viewport ${width}x${height}`);
    }
    const output = {
        scenario: receipt.scenario,
        category: receipt.category,
        runtime: receipt.runtime,
        intent: receipt.intent,
        produced: receipt.produced,
    };
    fs.writeFileSync(join(currentArtifactsDir(), `${receipt.scenario}--receipt.json`), `${JSON.stringify(output, null, 2)}\n`);
    return output;
}
