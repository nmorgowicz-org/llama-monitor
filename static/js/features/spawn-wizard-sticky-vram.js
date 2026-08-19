// ── Sticky header inline VRAM bar (M3-A) ────────────────────────────
// Reads the same VRAM state that drives the sidebar #vram-bar and
// renders a compact bar inside the sticky #hw-model-header so the
// user always sees memory budget while scrolling.

// Called from spawn-wizard-vram-display.js after each VRAM estimate update.
// Receives the same breakdown object used for the sidebar.
export function renderStickyVramBar(data) {
	const row = document.getElementById('hw-vram-bar-row');
	if (!row) return;

	if (!data || !data.vramTotal || data.vramTotal === 0) {
		row.style.display = 'none';
		return;
	}
	row.style.display = 'flex';

	const weights = document.getElementById('hw-vseg-weights');
	const kv = document.getElementById('hw-vseg-kv');
	const mmproj = document.getElementById('hw-vseg-mmproj');
	const mtp = document.getElementById('hw-vseg-mtp');
	const oh = document.getElementById('hw-vseg-overhead');
	const free = document.getElementById('hw-vseg-free');
	const total = document.getElementById('hw-vram-total');
	const status = document.getElementById('hw-vram-status');

	const used =
		(data.vramWeights || 0) +
		(data.vramKv || 0) +
		(data.vramMmproj || 0) +
		(data.vramMtp || 0) +
		(data.vramOverhead || 0);
	const avail = data.vramTotal - used;

	const pct = (v) => {
		const p = (v / data.vramTotal) * 100;
		return Math.max(0, Math.min(100, p));
	};

	if (weights) weights.style.width = pct(data.vramWeights || 0) + '%';
	if (kv) kv.style.width = pct(data.vramKv || 0) + '%';
	if (mmproj) {
		const has = (data.vramMmproj || 0) > 0;
		mmproj.style.display = has ? '' : 'none';
		mmproj.style.width = pct(data.vramMmproj || 0) + '%';
	}
	if (mtp) {
		const has = (data.vramMtp || 0) > 0;
		mtp.style.display = has ? '' : 'none';
		mtp.style.width = pct(data.vramMtp || 0) + '%';
	}
	if (oh) oh.style.width = pct(data.vramOverhead || 0) + '%';
	if (free) free.style.width = pct(avail) + '%';

	if (total) {
		total.textContent =
			`${(used / 1024 ** 3).toFixed(1)} / ${(data.vramTotal / 1024 ** 3).toFixed(1)} GB`;
	}

	if (status) {
		const freePct = avail / data.vramTotal;
		status.textContent = '';
		status.className = 'hw-vram-status';
		if (freePct < 0.05) {
			status.textContent = '⚠ Over';
			status.classList.add('over');
		} else if (freePct < 0.15) {
			status.textContent = '⚡ Tight';
			status.classList.add('tight');
		} else {
			status.textContent = '✓ Comfortable';
			status.classList.add('comfortable');
		}
	}
}

// Initialize visibility: hide row until first real update.
export function initStickyVramBar() {
	const row = document.getElementById('hw-vram-bar-row');
	if (row) row.style.display = 'none';
}
