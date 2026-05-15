// Database Administration Panel
// Premium modal with tabs for maintenance, backups, indexes, repair, and query

const dbAdminLog = [];
let dbAdminOverlay = null;

export function initDbAdmin() {
    dbAdminOverlay = document.getElementById('db-admin-modal');
    if (!dbAdminOverlay) return;

    // Load initial stats
    loadDbStats();

    // Close handlers
    document.getElementById('db-admin-modal-close')?.addEventListener('click', closeDbAdminModal);
    dbAdminOverlay?.addEventListener('click', (e) => {
        if (e.target === dbAdminOverlay) closeDbAdminModal();
    });
    document.addEventListener('keydown', (e) => {
        if (e.key === 'Escape' && dbAdminOverlay?.classList.contains('active')) {
            closeDbAdminModal();
        }
    });

    // Tab switching
    document.querySelectorAll('.db-tab').forEach(tab => {
        tab.addEventListener('click', () => switchTab(tab.dataset.tab));
    });

    // Bind button handlers
    document.getElementById('db-btn-refresh')?.addEventListener('click', () => {
        loadDbStats();
        loadBackups();
        loadIndexes();
    });
    document.getElementById('db-btn-backup')?.addEventListener('click', handleBackup);
    document.getElementById('db-btn-check')?.addEventListener('click', handleIntegrityCheck);
    document.getElementById('db-btn-checkpoint')?.addEventListener('click', () => handleMaintenance('checkpoint'));
    document.getElementById('db-btn-vacuum')?.addEventListener('click', () => handleMaintenance('vacuum'));
    document.getElementById('db-btn-rebuild-fts')?.addEventListener('click', () => handleMaintenance('rebuild_fts'));
    document.getElementById('db-btn-analyze')?.addEventListener('click', () => handleMaintenance('analyze'));

    // Backups tab
    document.getElementById('db-btn-clear-log')?.addEventListener('click', () => {
        dbAdminLog.length = 0;
        renderLog();
    });

    // Indexes tab
    document.getElementById('db-btn-rebuild-all-indexes')?.addEventListener('click', handleRebuildAllIndexes);

    // Repair tab
    document.getElementById('db-btn-repair-indexes')?.addEventListener('click', handleRepairIndexes);
    document.getElementById('db-btn-repair-compact')?.addEventListener('click', handleRepairCompact);
    document.getElementById('db-btn-repair-emergency')?.addEventListener('click', handleRepairEmergency);

    // Query tab
    document.getElementById('db-btn-run-query')?.addEventListener('click', handleQuery);
    document.getElementById('db-btn-clear-query')?.addEventListener('click', () => {
        const input = document.getElementById('db-query-input');
        if (input) input.value = '';
        const result = document.getElementById('db-query-result');
        if (result) result.hidden = true;
    });
    document.getElementById('db-btn-copy-result')?.addEventListener('click', () => {
        const output = document.getElementById('db-query-output');
        if (output) {
            navigator.clipboard.writeText(output.textContent);
            addLogEntry('success', 'Result copied to clipboard');
        }
    });

    // Enter key for query
    document.getElementById('db-query-input')?.addEventListener('keydown', (e) => {
        if (e.key === 'Enter' && (e.ctrlKey || e.metaKey)) {
            e.preventDefault();
            handleQuery();
        }
    });

    // Auto-refresh stats every 30 seconds
    setInterval(loadDbStats, 30000);
}

function closeDbAdminModal() {
    if (!dbAdminOverlay) return;
    const modal = dbAdminOverlay.querySelector('.db-admin-modal');
    if (modal) {
        modal.classList.add('closing');
        setTimeout(() => {
            dbAdminOverlay.classList.remove('active');
            modal.classList.remove('closing');
        }, 250);
    } else {
        dbAdminOverlay.classList.remove('active');
    }
}

function switchTab(tabName) {
    document.querySelectorAll('.db-tab').forEach(t => t.classList.remove('active'));
    document.querySelectorAll('.db-tab-panel').forEach(p => p.classList.remove('active'));

    const tab = document.querySelector(`.db-tab[data-tab="${tabName}"]`);
    const panel = document.querySelector(`.db-tab-panel[data-panel="${tabName}"]`);

    if (tab) tab.classList.add('active');
    if (panel) panel.classList.add('active');

    // Load data for specific tabs
    if (tabName === 'backups') loadBackups();
    if (tabName === 'indexes') loadIndexes();
}

async function loadDbStats() {
    try {
        const [statsRes, integrityRes] = await Promise.all([
            fetch('/api/db/stats'),
            fetch('/api/db/integrity'),
        ]);

        const stats = await statsRes.json();
        const integrity = await integrityRes.json();

        // Update stats display
        document.getElementById('db-stat-tabs').textContent = stats.tab_count || '-';
        document.getElementById('db-stat-messages').textContent = stats.message_count || '-';
        document.getElementById('db-stat-size').textContent = formatBytes(stats.file_size_bytes || 0);
        document.getElementById('db-stat-fts').textContent = stats.fts_index_count || '-';

        // Update status indicator
        const dot = document.getElementById('db-status-dot');
        const text = document.getElementById('db-status-text');

        if (dot && text) {
            if (integrity.status === 'healthy') {
                dot.className = 'db-status-dot healthy';
                text.textContent = 'Healthy';
            } else if (integrity.status === 'warning') {
                dot.className = 'db-status-dot warning';
                text.textContent = 'Issues Detected';
            } else {
                dot.className = 'db-status-dot error';
                text.textContent = 'Errors Found';
            }
        }
    } catch (error) {
        console.error('[db-admin] Failed to load stats:', error);
        addLogEntry('error', 'Failed to load database statistics');
    }
}

async function handleBackup() {
    addLogEntry('info', 'Creating backup...');
    setButtonLoading('db-btn-backup', true);

    try {
        const res = await fetch('/api/db/backup', { method: 'POST' });
        const result = await res.json();

        if (result.status === 'backup_created') {
            addLogEntry('success', `Backup created: ${formatBytes(result.size_bytes)}`);
        } else {
            addLogEntry('error', result.error || 'Backup failed');
        }
    } catch (error) {
        addLogEntry('error', `Backup failed: ${error.message}`);
    } finally {
        setButtonLoading('db-btn-backup', false);
    }
}

async function handleIntegrityCheck() {
    addLogEntry('info', 'Running integrity check...');
    setButtonLoading('db-btn-check', true);

    try {
        const res = await fetch('/api/db/integrity');
        const result = await res.json();

        if (result.status === 'healthy') {
            addLogEntry('success', 'Database integrity: OK');
        } else {
            addLogEntry('warning', `Database integrity issue: ${result.detail}`);
        }
    } catch (error) {
        addLogEntry('error', `Integrity check failed: ${error.message}`);
    } finally {
        setButtonLoading('db-btn-check', false);
    }
}

async function handleMaintenance(operation) {
    const labels = {
        checkpoint: 'WAL Checkpoint',
        vacuum: 'Vacuum',
        rebuild_fts: 'Rebuild FTS Index',
        analyze: 'Analyze',
    };

    addLogEntry('info', `Running ${labels[operation] || operation}...`);

    try {
        const res = await fetch('/api/db/maintenance', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ operation }),
        });
        const result = await res.json();
        addLogEntry('success', `${labels[operation] || operation}: ${result.status || 'completed'}`);
    } catch (error) {
        addLogEntry('error', `${labels[operation] || operation} failed: ${error.message}`);
    }

    loadDbStats();
}

async function loadBackups() {
    const listEl = document.getElementById('db-backups-list');
    const emptyEl = document.getElementById('db-backups-empty');
    const countEl = document.getElementById('db-backup-count');
    const sizeEl = document.getElementById('db-backup-total-size');

    if (!listEl) return;

    try {
        const res = await fetch('/api/db/backups');
        const data = await res.json();

        if (!data.backups || data.backups.length === 0) {
            emptyEl.innerHTML = '<p>No backups found. Create one with "Backup Now".</p>';
            countEl.textContent = '0';
            sizeEl.textContent = '0 B';
            return;
        }

        countEl.textContent = data.backups.length;
        sizeEl.textContent = formatBytes(data.total_size || 0);

        // eslint-disable-next-line no-unsanitized/property -- all content escaped via escapeHtml()
        listEl.innerHTML = data.backups.map(b => `
            <div class="db-backup-item">
                <div class="db-backup-info">
                    <span class="db-backup-name">${escapeHtml(b.name)}</span>
                    <span class="db-backup-meta">${formatBytes(b.size)} &middot; ${formatDate(b.modified)}</span>
                </div>
                <div class="db-backup-actions">
                    <button type="button" class="db-action-btn small" onclick="restoreBackup('${escapeHtml(b.name)}')" title="Restore">
                        <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M3 12a9 9 0 019-9 9.75 9.75 0 016.74 2.74L21 8"/><path d="M21 3v5h-5"/><path d="M21 12a9 9 0 01-9 9 9.75 9.75 0 01-6.74-2.74L3 16"/><path d="M3 21v-5h5"/></svg>
                    </button>
                    <button type="button" class="db-action-btn small" onclick="deleteBackup('${escapeHtml(b.name)}')" title="Delete">
                        <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><polyline points="3 6 5 6 21 6"/><path d="M19 6v14a2 2 0 01-2 2H7a2 2 0 01-2-2V6m3 0V4a2 2 0 012-2h4a2 2 0 012 2v2"/></svg>
                    </button>
                </div>
            </div>
        `).join('');
    } catch (error) {
        console.error('[db-admin] Failed to load backups:', error);
    }
}

async function loadIndexes() {
    const listEl = document.getElementById('db-indexes-list');
    if (!listEl) return;

    try {
        const res = await fetch('/api/db/indexes');
        const data = await res.json();

        if (!data.indexes || data.indexes.length === 0) {
            listEl.innerHTML = '<div class="db-empty-state"><p>No indexes found.</p></div>';
            return;
        }

        // eslint-disable-next-line no-unsanitized/property -- all content escaped via escapeHtml()
        listEl.innerHTML = data.indexes.map(idx => `
            <div class="db-index-item">
                <div>
                    <div class="db-index-name">${escapeHtml(idx.name)}</div>
                    <div class="db-index-table">${escapeHtml(idx.table || '')}</div>
                </div>
                <div class="db-index-actions">
                    ${idx.rebuildable ? `<button type="button" class="db-action-btn small" onclick="rebuildIndex('${escapeHtml(idx.name)}')">Rebuild</button>` : ''}
                </div>
            </div>
        `).join('');
    } catch (error) {
        console.error('[db-admin] Failed to load indexes:', error);
    }
}

async function handleRebuildAllIndexes() {
    addLogEntry('info', 'Rebuilding all indexes...');
    setButtonLoading('db-btn-rebuild-all-indexes', true);

    try {
        const res = await fetch('/api/db/maintenance', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ operation: 'rebuild_fts' }),
        });
        const result = await res.json();
        addLogEntry('success', `Index rebuild: ${result.status || 'completed'}`);
    } catch (error) {
        addLogEntry('error', `Index rebuild failed: ${error.message}`);
    } finally {
        setButtonLoading('db-btn-rebuild-all-indexes', false);
    }

    loadIndexes();
}

// Repair operations
async function handleRepairIndexes() {
    if (!confirm('This will drop and recreate all indexes. Continue?')) return;

    addLogEntry('info', 'Repairing indexes...');
    setButtonLoading('db-btn-repair-indexes', true);

    try {
        // First create a backup
        await fetch('/api/db/backup', { method: 'POST' });
        addLogEntry('info', 'Backup created before repair');

        const res = await fetch('/api/db/repair', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ operation: 'repair_indexes' }),
        });
        const result = await res.json();
        addLogEntry('success', `Index repair: ${result.status || 'completed'}`);
    } catch (error) {
        addLogEntry('error', `Index repair failed: ${error.message}`);
    } finally {
        setButtonLoading('db-btn-repair-indexes', false);
    }

    loadIndexes();
}

async function handleRepairCompact() {
    if (!confirm('This will compact the database. The app may be unresponsive during this operation. Continue?')) return;

    addLogEntry('info', 'Compacting database...');
    setButtonLoading('db-btn-repair-compact', true);

    try {
        await fetch('/api/db/backup', { method: 'POST' });
        addLogEntry('info', 'Backup created before compact');

        const res = await fetch('/api/db/maintenance', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ operation: 'vacuum' }),
        });
        const result = await res.json();
        addLogEntry('success', `Compact: ${result.status || 'completed'}`);
    } catch (error) {
        addLogEntry('error', `Compact failed: ${error.message}`);
    } finally {
        setButtonLoading('db-btn-repair-compact', false);
    }

    loadDbStats();
}

async function handleRepairEmergency() {
    if (!confirm('EMERGENCY RECOVERY: This attempts to recover data from a corrupted database. A backup will be created first. Continue?')) return;

    addLogEntry('warning', 'Starting emergency recovery...');
    setButtonLoading('db-btn-repair-emergency', true);

    try {
        await fetch('/api/db/backup', { method: 'POST' });
        addLogEntry('info', 'Backup created before recovery');

        const res = await fetch('/api/db/repair', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ operation: 'emergency_recovery' }),
        });
        const result = await res.json();
        addLogEntry('success', `Emergency recovery: ${result.status || 'completed'}`);
    } catch (error) {
        addLogEntry('error', `Emergency recovery failed: ${error.message}`);
    } finally {
        setButtonLoading('db-btn-repair-emergency', false);
    }

    loadDbStats();
}

async function handleQuery() {
    const input = document.getElementById('db-query-input');
    const resultEl = document.getElementById('db-query-result');
    const outputEl = document.getElementById('db-query-output');

    if (!input || !resultEl || !outputEl) return;

    const sql = input.value.trim();
    if (!sql) return;

    // Safety check - only allow SELECT and PRAGMA
    const upperSql = sql.toUpperCase().trim();
    if (!upperSql.startsWith('SELECT') && !upperSql.startsWith('PRAGMA')) {
        addLogEntry('error', 'Only SELECT and PRAGMA queries are allowed');
        return;
    }

    addLogEntry('info', `Executing: ${sql.substring(0, 50)}...`);

    try {
        const res = await fetch('/api/db/query', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ sql }),
        });
        const result = await res.json();

        if (result.error) {
            addLogEntry('error', result.error);
            outputEl.textContent = result.error;
        } else {
            addLogEntry('success', `Query returned ${result.rows?.length || 0} rows`);
            outputEl.textContent = JSON.stringify(result, null, 2);
        }
        resultEl.hidden = false;
    } catch (error) {
        addLogEntry('error', `Query failed: ${error.message}`);
        outputEl.textContent = error.message;
        resultEl.hidden = false;
    }
}

// Global functions for inline handlers
window.restoreBackup = async function(name) {
    if (!confirm(`Restore from backup "${name}"? Current data will be replaced.`)) return;

    addLogEntry('warning', `Restoring from ${name}...`);
    try {
        const res = await fetch('/api/db/restore', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ backup_name: name }),
        });
        const result = await res.json();
        if (result.error) {
            addLogEntry('error', result.error);
        } else {
            addLogEntry('success', `Restored from ${name}`);
            loadDbStats();
        }
    } catch (error) {
        addLogEntry('error', `Restore failed: ${error.message}`);
    }
};

window.deleteBackup = async function(name) {
    if (!confirm(`Delete backup "${name}"?`)) return;

    try {
        const res = await fetch('/api/db/backup', {
            method: 'DELETE',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ backup_name: name }),
        });
        const result = await res.json();
        if (result.error) {
            addLogEntry('error', result.error);
        } else {
            addLogEntry('success', `Deleted ${name}`);
            loadBackups();
        }
    } catch (error) {
        addLogEntry('error', `Delete failed: ${error.message}`);
    }
};

window.rebuildIndex = async function(name) {
    addLogEntry('info', `Rebuilding index ${name}...`);
    try {
        const res = await fetch('/api/db/maintenance', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ operation: 'rebuild_fts' }),
        });
        const result = await res.json();
        addLogEntry('success', `Index rebuild: ${result.status || 'completed'}`);
        loadIndexes();
    } catch (error) {
        addLogEntry('error', `Index rebuild failed: ${error.message}`);
    }
};

function addLogEntry(type, message) {
    const timestamp = new Date().toLocaleTimeString();
    dbAdminLog.push({ type, message, timestamp });

    // Keep only last 100 entries
    while (dbAdminLog.length > 100) {
        dbAdminLog.shift();
    }

    renderLog();
}

function renderLog() {
    const logContent = document.getElementById('db-log-content');
    if (!logContent) return;

    // eslint-disable-next-line no-unsanitized/property -- all content escaped via escapeHtml()
    logContent.innerHTML = dbAdminLog
        .map(
            (entry) =>
                `<div class="db-log-entry ${entry.type}">
                    <span class="db-log-time">${entry.timestamp}</span>
                    <span class="db-log-message">${escapeHtml(entry.message)}</span>
                </div>`
        )
        .join('');

    // Scroll to bottom
    logContent.scrollTop = logContent.scrollHeight;
}

function setButtonLoading(btnId, loading) {
    const btn = document.getElementById(btnId);
    if (!btn) return;

    if (loading) {
        btn.disabled = true;
        btn.dataset.originalText = btn.textContent;
        btn.innerHTML = '<svg class="db-spinner" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M21 12a9 9 0 11-6.219-8.56"/></svg> Working...';
    } else {
        btn.disabled = false;
        btn.textContent = btn.dataset.originalText || btn.textContent;
    }
}

function formatBytes(bytes) {
    if (bytes === 0) return '0 B';
    const k = 1024;
    const sizes = ['B', 'KB', 'MB', 'GB'];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return parseFloat((bytes / Math.pow(k, i)).toFixed(1)) + ' ' + sizes[i];
}

function formatDate(timestamp) {
    const date = new Date(timestamp);
    const now = new Date();
    const diff = now - date;

    if (diff < 60000) return 'just now';
    if (diff < 3600000) return `${Math.floor(diff / 60000)}m ago`;
    if (diff < 86400000) return `${Math.floor(diff / 3600000)}h ago`;
    return date.toLocaleDateString();
}

function escapeHtml(s) {
    const div = document.createElement('div');
    div.textContent = s;
    return div.innerHTML;
}
