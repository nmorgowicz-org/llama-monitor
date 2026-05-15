import { test, expect } from '@playwright/test';

const sshTarget = process.env.LLAMA_MONITOR_SSH_TARGET || '';
const agentUrl = process.env.LLAMA_MONITOR_AGENT_URL || null;
const sshPassword = process.env.LLAMA_MONITOR_SSH_PASSWORD || '';
const sshPrivateKeyPath = process.env.LLAMA_MONITOR_SSH_KEY_PATH || '';
const sshPrivateKeyPassphrase = process.env.LLAMA_MONITOR_SSH_KEY_PASSPHRASE || '';

function connectionFromTarget(target) {
  if (!sshPassword && !sshPrivateKeyPath) return undefined;

  const normalized = target.startsWith('ssh://') ? target : 'ssh://' + target;
  const url = new URL(normalized);
  const connection = {
    host: url.hostname,
    username: decodeURIComponent(url.username || ''),
    port: Number(url.port || 22),
  };

  if (sshPassword) connection.password = sshPassword;
  if (sshPrivateKeyPath) {
    connection.private_key_path = sshPrivateKeyPath;
    if (sshPrivateKeyPassphrase) connection.private_key_passphrase = sshPrivateKeyPassphrase;
  }

  return connection;
}

test.describe('remote agent SSH integration', () => {
  test('detects a real SSH target when explicitly enabled', async ({ request }) => {
    test.skip(!sshTarget, 'Set LLAMA_MONITOR_SSH_TARGET=user@host to run the SSH integration test.');

    const payload = {
      ssh_target: sshTarget,
      agent_url: agentUrl,
    };
    const sshConnection = connectionFromTarget(sshTarget);
    if (sshConnection) payload.ssh_connection = sshConnection;

    const hostKeyResponse = await request.post('/api/remote-agent/ssh/host-key', { data: payload });
    expect(hostKeyResponse.ok()).toBe(true);

    const hostKeyData = await hostKeyResponse.json();
    expect(hostKeyData.ok, hostKeyData.error || 'host key should be scannable').toBe(true);
    expect(hostKeyData.host_key.key_hex).toBeTruthy();

    if (!hostKeyData.host_key.trusted) {
      const trustResponse = await request.post('/api/remote-agent/ssh/trust', {
        data: {
          ...payload,
          key_hex: hostKeyData.host_key.key_hex,
        },
      });
      expect(trustResponse.ok()).toBe(true);
      const trustData = await trustResponse.json();
      expect(trustData.ok, trustData.error || 'host key should be trusted').toBe(true);
    }

    const response = await request.post('/api/remote-agent/detect', { data: payload });

    expect(response.ok()).toBe(true);

    const data = await response.json();
    expect(data.ssh_target).toBe(sshTarget);
    expect(data.os, data.error || 'remote OS should be detected over SSH').not.toBe('unknown');
    expect(data.arch, data.error || 'remote architecture should be detected over SSH').not.toBe('unknown');
    expect(data.install_path, data.error || 'remote install path should be derived').toBeTruthy();
    expect(typeof data.installed).toBe('boolean');
    expect(typeof data.reachable).toBe('boolean');
  });
});
