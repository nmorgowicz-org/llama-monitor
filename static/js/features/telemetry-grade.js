/**
 * Derive a single telemetry grade from WebSocket snapshot data.
 * @param {object} d - WebSocket data snapshot
 * @returns {string} One of: 'local_full', 'remote_inference_only', 'remote_agent_connecting',
 *   'remote_agent_connected', 'remote_agent_degraded', 'remote_agent_firewall_blocked',
 *   'remote_agent_update_available', 'remote_partial_sensors', 'remote_error'
 */
export function deriveTelemetryGrade(d) {
    if (!d || !d.active_session_id) {
        return 'local_full';
    }

    const caps = d.capabilities || {};
    const isLocal = d.endpoint_kind === 'Local';
    const isRemote = d.endpoint_kind === 'Remote';

    // Local endpoint always gets full telemetry
    if (isLocal) {
        return 'local_full';
    }

    // Remote endpoint — derive grade from capabilities and agent state
    if (isRemote) {
        // Error: inference itself is broken
        if (!caps.inference) {
            return 'remote_error';
        }

        // No host metrics at all — inference only
        if (!caps.host_metrics) {
            return 'remote_inference_only';
        }

        // Agent connected states (priority order)
        if (d.remote_agent_protocol_too_old) {
            return 'remote_agent_degraded';
        }

        if (d.remote_agent_connected && !d.remote_agent_health_reachable) {
            return 'remote_agent_firewall_blocked';
        }

        if (d.remote_agent_connected && d.remote_agent_update_available) {
            return 'remote_agent_update_available';
        }

        // Partial sensor coverage — agent connected but not all sensors working
        if (d.remote_agent_connected && (!caps.system || !caps.gpu)) {
            return 'remote_partial_sensors';
        }

        if (d.remote_agent_connected) {
            return 'remote_agent_connected';
        }

        // Agent not connected but host_metrics somehow available (shouldn't happen, treat as inference-only)
        return 'remote_inference_only';
    }

    // Unknown endpoint kind — conservative default
    return 'remote_error';
}

/**
 * Human-readable label for a telemetry grade.
 */
export function gradeLabel(grade) {
    const labels = {
        local_full: 'Full telemetry',
        remote_inference_only: 'Inference only',
        remote_agent_connecting: 'Connecting...',
        remote_agent_connected: 'Full telemetry',
        remote_agent_degraded: 'Degraded',
        remote_agent_firewall_blocked: 'Firewall blocked',
        remote_agent_update_available: 'Update available',
        remote_partial_sensors: 'Partial sensors',
        remote_error: 'Error',
    };
    return labels[grade] || 'Unknown';
}

/**
 * CSS status class for a telemetry grade.
 */
export function gradeStatusClass(grade) {
    if (grade === 'local_full' || grade === 'remote_agent_connected') return 'ok';
    if (grade === 'remote_error') return 'error';
    return 'warning';
}

/**
 * Actionable next-step copy for limited telemetry grades.
 */
export function gradeActionCopy(grade) {
    const copies = {
        remote_inference_only: 'Install the remote agent on the host to enable system and GPU metrics.',
        remote_agent_firewall_blocked: 'Open port 7779 on the remote host or adjust firewall rules.',
        remote_agent_degraded: 'Upgrade the remote agent — running in degraded compatibility mode.',
        remote_partial_sensors: 'Some sensors unavailable. Check agent logs on the remote host.',
        remote_error: 'Agent connection failed. Verify the remote endpoint and agent status.',
    };
    return copies[grade] || '';
}
