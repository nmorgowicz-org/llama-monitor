// Client-side formatting helpers shared across the spawn wizard modules.

export const KV_BPE = {
  f32:2.0*2, f16:2.0, bf16:2.0,
  q8_0:1.0, q5_0:0.625, q5_1:0.625,
  q4_k_m:0.5, q4_k_s:0.5, q4_0:0.5, q4_1:0.5, iq4_xs:0.5, iq4_nl:0.5,
  q3_k_m:0.375, q3_k_s:0.375, q3_k_l:0.375, iq3_m:0.375, iq3_s:0.375,
  q3_k:0.375, iq3_xs:0.375, iq3_xxs:0.375,
  q2_k:0.25, iq2_m:0.25, iq2_s:0.25, iq2_xs:0.25, iq2_xxs:0.25,
  iq1_m:0.125, iq1_s:0.125,
};
export function kvBpe(quant) { return KV_BPE[quant] ?? 1.0; }

export function formatCtx(n) {
  if (!n) return '—';
  if (n >= 1_000_000) return (n / 1e6).toFixed(1) + 'M';
  if (n >= 1000) return (n / 1000).toFixed(0) + 'K';
  return String(n);
}
export function formatParams(paramB) {
  if (paramB >= 1000) return `${(paramB / 1000).toFixed(1)}T`;
  if (paramB % 1 === 0) return `${paramB}B`;
  return `${Number(paramB).toFixed(1)}B`;
}
export function formatGB(bytes) {
  if (!bytes) return '0 GB';
  return (bytes / 1e9).toFixed(1) + ' GB';
}
// Use binary GiB (1024³) for system VRAM totals so "64 GiB" shows as "64 GB",
// matching Apple's marketing convention (GiB labeled as GB).
export function formatVramTotal(bytes) {
  if (!bytes) return '0 GB';
  return (bytes / (1024 ** 3)).toFixed(1) + ' GB';
}
export function formatBytes(bytes) {
  if (!bytes) return '';
  const b = Number(bytes);
  if (!isFinite(b)) return '';
  if (b >= 1e9) return (b / 1e9).toFixed(1) + ' GB';
  if (b >= 1e6) return (b / 1e6).toFixed(1) + ' MB';
  if (b >= 1e3) return (b / 1e3).toFixed(0) + ' KB';
  return b + ' B';
}
export function formatSpeed(bps) {
  if (bps >= 1e9) return (bps / 1e9).toFixed(1) + ' GB/s';
  if (bps >= 1e6) return (bps / 1e6).toFixed(1) + ' MB/s';
  return (bps / 1e3).toFixed(0) + ' KB/s';
}
