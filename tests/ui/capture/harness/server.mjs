// Local server lifecycle: seed config, spawn/wait/cleanup llama-monitor.
// Extracted from tests/ui/capture.mjs (Phase A1).
import fs from 'fs';
import { join } from 'path';
import { spawn } from 'child_process';
import net from 'net';
import { BINARY_PATH, DEFAULT_PORT, REAL_APP_CONFIG_DIR, ROOT_DIR, TEMP_APP_CONFIG_DIR, TEMP_CONFIG_HOME, TEMP_HOME, TEMP_WINDOWS_APPDATA, TEMP_WINDOWS_LOCALAPPDATA, sleep } from './paths.mjs';

export function seedConfig() {
    // Copy encryption-key first so encrypted values in ui-settings.json (e.g. remote_agent_token)
    // can be decrypted — without it the ephemeral instance generates a new key and auth fails.
    // Copy ssh-known-hosts.json so the agent SSH host key check passes (prevents enrollment block).
    // Copy hf-token so HF API searches use auth (higher rate limits, better trending results).
    const filesToCopy = ['encryption-key', 'ssh-known-hosts.json', 'hf-token', 'ui-settings.json', 'presets.json', 'gpu-env.json', 'community-picks.json'];
    for (const filename of filesToCopy) {
        const source = join(REAL_APP_CONFIG_DIR, filename);
        const destination = join(TEMP_APP_CONFIG_DIR, filename);
        if (fs.existsSync(source)) {
            fs.copyFileSync(source, destination);
        }
    }

    // Copy the certs/ directory (including remote-cas/) so the remote agent https_client starts
    // with the correct trust anchors. Without this it's built once at startup without the CA cert
    // and the mTLS agent connection always fails in that session.
    const sourceCerts = join(REAL_APP_CONFIG_DIR, 'certs');
    const destCerts = join(TEMP_APP_CONFIG_DIR, 'certs');
    if (fs.existsSync(sourceCerts)) {
        fs.cpSync(sourceCerts, destCerts, { recursive: true });
    }

    // If no community-picks.json exists in real config, use the bundled example fixture.
    const cpDest = join(TEMP_APP_CONFIG_DIR, 'community-picks.json');
    if (!fs.existsSync(cpDest)) {
        const cpExample = join(ROOT_DIR, 'docs/reference/community-picks-example.json');
        if (fs.existsSync(cpExample)) {
            fs.copyFileSync(cpExample, cpDest);
        }
    }
}

export function cleanupTempHome() {
    fs.rmSync(TEMP_HOME, { recursive: true, force: true });
}

export async function findAvailablePort(startPort = DEFAULT_PORT) {
    for (let port = startPort; port < startPort + 200; port += 1) {
        const available = await new Promise(resolve => {
            const server = net.createServer();
            server.unref();
            server.on('error', () => resolve(false));
            server.listen(port, '127.0.0.1', () => {
                server.close(() => resolve(true));
            });
        });
        if (available) return port;
    }
    throw new Error(`No available port found starting at ${startPort}`);
}

export async function waitForHttp(url, timeout = 30000) {
    const start = Date.now();
    while (Date.now() - start < timeout) {
        try {
            const response = await fetch(url, { method: 'GET' });
            if (response.ok) return;
        } catch {
            // Keep polling.
        }
        await sleep(250);
    }
    throw new Error(`Server did not become ready at ${url} within ${timeout}ms`);
}

export async function spawnLlamaMonitor(port, extraArgs = []) {
    const proc = spawn(BINARY_PATH, ['--config-dir', TEMP_APP_CONFIG_DIR, '--port', String(port), '--headless', ...extraArgs], {
        stdio: ['ignore', 'pipe', 'pipe'],
        env: {
            ...process.env,
            HOME: TEMP_HOME,
            XDG_CONFIG_HOME: TEMP_CONFIG_HOME,
            APPDATA: TEMP_WINDOWS_APPDATA,
            LOCALAPPDATA: TEMP_WINDOWS_LOCALAPPDATA,
            USERPROFILE: TEMP_HOME,
            // Suppress llama-monitor self-update checks during capture so the
            // GitHub rate limit is not consumed before the llama-updater scenario
            // needs it to fetch real llama.cpp release notes.
            LLAMA_SKIP_RELEASE_CHECK: '1',
        },
    });

    proc.stdout.on('data', data => {
        const output = data.toString().trim();
        if (output) console.log(`[llama-monitor] ${output}`);
    });

    proc.stderr.on('data', data => {
        const output = data.toString().trim();
        if (output) console.log(`[llama-monitor] ${output}`);
    });

    proc.on('error', err => {
        console.error(`Failed to spawn llama-monitor: ${err.message}`);
    });

    const url = `http://127.0.0.1:${port}`;
    await waitForHttp(url);
    return { proc, url };
}

export async function cleanupServer(server) {
    if (!server?.proc) return;
    server.proc.kill('SIGTERM');
    await sleep(750);
    if (server.proc.exitCode === null) {
        server.proc.kill('SIGKILL');
    }
}
