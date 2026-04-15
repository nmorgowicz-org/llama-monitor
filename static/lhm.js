// LHM (LibreHardwareMonitor) integration - full implementation
// This is what was in commit 205b92d

async function checkLHMAndPrompt() {
    // Only run LHM checks on Windows
    const lhmBtn = document.getElementById('btn-lhm');
    if (!lhmBtn) return;

    try {
        // Check if LHM is available
        const statusResp = await fetch('/api/lhm/status');
        const statusData = await statusResp.json();
        
        if (statusData.disabled) {
            // User disabled LHM - hide button
            lhmBtn.style.display = 'none';
            return;
        }
        
        const checkResp = await fetch('/api/lhm/check');
        const checkData = await checkResp.json();
        const lhmAvailable = checkData.available || false;

        // If LHM is available, hide button entirely
        if (lhmAvailable) {
            lhmBtn.style.display = 'none';
            return;
        }

        // LHM not available - show button
        lhmBtn.style.display = 'flex';
        
        // If user disabled LHM, add pulse animation to notify them
        if (statusData.disabled) {
            lhmBtn.classList.add('need-attention');
        }
        
        // Add click handler to show notification
        lhmBtn.onclick = async () => {
            lhmBtn.onclick = null; // Remove handler
            const action = await showLHMNotification();
            
            if (action === 'install') {
                const installResp = await fetch('/api/lhm/install', { method: 'POST' });
                const installData = await installResp.json();
                if (installData.success) {
                    showToast('LibreHardwareMonitor installed successfully! Reloading...', 'success');
                    setTimeout(() => location.reload(), 2000);
                } else {
                    showToast('Failed to install LibreHardwareMonitor: ' + (installData.error || 'Unknown error'), 'error');
                }
            } else if (action === 'disable') {
                await fetch('/api/lhm/disable', { method: 'POST' });
                console.error('Failed to save LHM disable preference');
                showToast('LHM monitoring disabled', 'info');
                lhmBtn.style.display = 'none';
            }
        };
        
        // Pulse animation to draw attention
        lhmBtn.classList.add('pulse');
    } catch (e) {
        console.error('LHM check failed:', e);
    }
}

// Show the LHM notification (triggered by user clicking the LHM button or first load)
async function showLHMNotification() {
    return new Promise((resolve) => {
        const overlay = document.createElement('div');
        overlay.style.cssText = 'position:fixed;top:0;left:0;width:100%;height:100%;background:rgba(0,0,0,0.5);display:flex;align-items:center;justify-content:center;z-index:1000;';
        
        const modal = document.createElement('div');
        modal.style.cssText = 'background:#2e3440;border-radius:8px;padding:24px;max-width:400px;box-shadow:0 4px 20px rgba(0,0,0,0.5);';
        
        modal.innerHTML = `
            <h3 style="margin:0 0 16px 0;color:#d8dee9;">LibreHardwareMonitor Not Found</h3>
            <p style="color:#eceff4;margin-bottom:24px;font-size:0.9rem;">
                LHM is required to monitor GPU metrics. Would you like to install it automatically?
            </p>
            <div style="display:flex;gap:12px;">
                <button id="btn-lhm-install" 
                    style="flex:1;padding:10px;background:#a3be8c;border:none;border-radius:4px;cursor:pointer;font-weight:bold;">
                    Install Automatically
                </button>
                <button id="btn-lhm-cancel" 
                    style="flex:1;padding:10px;background:#bf616a;border:none;border-radius:4px;cursor:pointer;">
                    Disable
                </button>
            </div>
        `;
        
        overlay.appendChild(modal);
        document.body.appendChild(overlay);
        
        overlay.querySelector('#btn-lhm-install').onclick = () => {
            resolve('install');
        };
        
        overlay.querySelector('#btn-lhm-cancel').onclick = () => {
            resolve('disable');
        };
    });
}
