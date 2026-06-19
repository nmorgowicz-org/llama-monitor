/// Shared mactop cache — allows both GPU and system metric paths to read
/// cluster frequencies, power, and load without running mactop twice.
///
/// The GPU poller populates this every ~500ms; the system poller reads it
/// every ~5s, so the data is always fresh when needed.
use std::sync::OnceLock;

static MACTOP_CACHE: OnceLock<std::sync::RwLock<Option<MactopCacheEntry>>> = OnceLock::new();

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct MactopCacheEntry {
    pub power_total_w: f32,
    pub power_cpu_w: f32,
    pub power_gpu_w: f32,
    pub p_cluster_freq_mhz: u32,
    pub s_cluster_freq_mhz: u32,
    pub e_cluster_freq_mhz: u32,
    pub p_cluster_active: f32,
    pub s_cluster_active: f32,
    pub e_cluster_active: f32,
}

pub fn set_cache(entry: MactopCacheEntry) {
    let rw = MACTOP_CACHE.get_or_init(|| std::sync::RwLock::new(None));
    if let Ok(mut guard) = rw.try_write() {
        *guard = Some(entry);
    }
}

pub fn get_cache() -> Option<MactopCacheEntry> {
    let rw = MACTOP_CACHE.get_or_init(|| std::sync::RwLock::new(None));
    match rw.try_read() {
        Ok(guard) => guard.clone(),
        Err(_) => None,
    }
}
