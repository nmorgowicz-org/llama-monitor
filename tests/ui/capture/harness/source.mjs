// Runtime-source selection for capture scenarios.
//
// Phase 1 deliberately implements only the existing remote attach strategy.
// Local llama.cpp and Rapid-MLX strategies will be added behind this same
// contract in later phases without requiring scenario rewrites.
import { attachToServer } from './attach.mjs';

export const CAPTURE_SOURCES = Object.freeze([
    'remote',
    'local-llamacpp',
    'local-mlx',
    'auto',
]);

const IMPLEMENTED_SOURCES = new Set(['remote']);

function validateSource(source, origin) {
    if (!CAPTURE_SOURCES.includes(source)) {
        throw new Error(
            `[CAPTURE] Unknown source "${source}" from ${origin}. ` +
            `Choose one of: ${CAPTURE_SOURCES.join(', ')}.`,
        );
    }
    return source;
}

/**
 * Resolve the source using the documented precedence order:
 * force → CLI option → CAPTURE_SOURCE → scenario default → remote.
 */
export function resolveCaptureSource({
    force,
    source,
    envSource = process.env.CAPTURE_SOURCE,
    scenarioSource = 'remote',
} = {}) {
    if (force) return validateSource(force, 'scenario force');
    if (source) return validateSource(source, '--source');
    if (envSource) return validateSource(envSource, 'CAPTURE_SOURCE');
    if (scenarioSource) return validateSource(scenarioSource, 'scenario default');
    return 'remote';
}

/**
 * Connect a capture page to the selected runtime source.
 *
 * The returned handle is intentionally uniform so future local strategies can
 * own their spawned process/preset cleanup without changing scenarios.
 */
export async function connectSource(page, opts = {}) {
    const source = resolveCaptureSource(opts);
    if (!IMPLEMENTED_SOURCES.has(source)) {
        throw new Error(
            `[CAPTURE] Source "${source}" is reserved for a later local-runtime ` +
            'phase; Phase 1 implements only "remote". Use --source remote or ' +
            'unset CAPTURE_SOURCE.',
        );
    }

    await (opts.attach || attachToServer)(page, opts.remoteServer);
    return {
        kind: source,
        async teardown() {
            // Remote attach owns no process or temporary preset.
        },
    };
}
