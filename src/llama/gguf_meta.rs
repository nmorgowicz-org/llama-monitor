//! Minimal GGUF metadata reader.
//!
//! Reads only the KV metadata header of a GGUF file — the small section that
//! precedes tensor data. Works on any GGUF version (1, 2, 3) with no external
//! binary and no new dependencies. Calling this on a 70B model is effectively
//! instant because tensor weights are never touched.
//!
//! GGUF format reference: <https://github.com/ggml-org/ggml/blob/master/docs/gguf.md>

use std::collections::HashMap;
use std::fs::File;
use std::io::{BufReader, Read, Seek, SeekFrom};
use std::path::Path;

// ── Format constants ──────────────────────────────────────────────────────────

const GGUF_MAGIC: &[u8; 4] = b"GGUF";

const TYPE_UINT8: u32 = 0;
const TYPE_INT8: u32 = 1;
const TYPE_UINT16: u32 = 2;
const TYPE_INT16: u32 = 3;
const TYPE_UINT32: u32 = 4;
const TYPE_INT32: u32 = 5;
const TYPE_FLOAT32: u32 = 6;
const TYPE_BOOL: u32 = 7;
const TYPE_STRING: u32 = 8;
const TYPE_ARRAY: u32 = 9;
const TYPE_UINT64: u32 = 10;
const TYPE_INT64: u32 = 11;
const TYPE_FLOAT64: u32 = 12;

// ── Public output type ────────────────────────────────────────────────────────

/// Architecture metadata extracted from a GGUF file's KV header.
///
/// All fields are optional; absent fields are not present in the file.
/// Callers should fall back to name-based heuristics for missing values.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct GgufMetadata {
    /// `general.architecture` — e.g. `"llama"`, `"qwen3_6"`, `"gemma4"`.
    /// This is the canonical key used by llama.cpp to select its model loader.
    /// Present in every well-formed GGUF regardless of filename.
    pub architecture: Option<String>,

    /// `general.parameter_count` — total parameters (not active for MoE).
    pub param_count: Option<u64>,

    /// `{arch}.block_count` — total transformer layers.
    pub block_count: Option<u32>,

    /// `{arch}.attention.head_count` — query head count.
    pub head_count: Option<u32>,

    /// `{arch}.attention.head_count_kv` — KV head count (GQA/MQA compressed).
    pub head_count_kv: Option<u32>,

    /// `{arch}.attention.key_length` — per-head K/V dimension.
    pub key_length: Option<u32>,

    /// `{arch}.context_length` — training context window size.
    pub context_length: Option<u32>,

    /// `{arch}.embedding_length` — hidden (embedding) dimension.
    pub embedding_length: Option<u32>,

    /// `{arch}.feed_forward_length` — FFN intermediate dimension.
    pub feed_forward_length: Option<u32>,

    /// `{arch}.expert_count` — total MoE experts per layer.
    pub expert_count: Option<u32>,

    /// `{arch}.expert_used_count` — active MoE experts per token.
    pub expert_used_count: Option<u32>,

    /// MTP prediction depth (`{arch}.next_n_token_count` or similar).
    pub mtp_depth: Option<u32>,
}

impl GgufMetadata {
    /// Approximate parameter count in billions, derived from `param_count`.
    pub fn param_b(&self) -> Option<f64> {
        self.param_count.map(|p| p as f64 / 1e9)
    }

    /// Convert to the `ModelMetadata` type used by the spawn wizard / VRAM estimator.
    ///
    /// Sets `gguf_arch` so that renamed finetunes (e.g. "Pantheon-27B" from a
    /// Qwen3.6 base) get the correct hybrid-DeltaNet heuristic regardless of filename.
    pub fn to_model_metadata(&self) -> crate::llama::spawn_wizard::ModelMetadata {
        crate::llama::spawn_wizard::ModelMetadata {
            n_layers: self.block_count,
            n_ctx_train: self.context_length,
            n_embd: self.embedding_length,
            n_ff: self.feed_forward_length,
            n_head: self.head_count,
            n_kv_heads: self.head_count_kv,
            head_dim: self.key_length,
            gguf_arch: self.architecture.clone(),
            n_experts: self.expert_count,
            n_experts_used: self.expert_used_count,
            mtp_depth: self.mtp_depth,
            mmproj_required: false,
            cached: false,
        }
    }
}

// ── Entry point ───────────────────────────────────────────────────────────────

/// Read GGUF metadata from a file without touching tensor data.
///
/// # Errors
/// Returns a human-readable error string if the file cannot be opened,
/// is not a valid GGUF file, or uses an unsupported version.
pub fn read_gguf_metadata(path: &Path) -> Result<GgufMetadata, String> {
    let file = File::open(path).map_err(|e| format!("Cannot open '{}': {e}", path.display()))?;
    let mut r = BufReader::with_capacity(64 * 1024, file);

    // Magic
    let mut magic = [0u8; 4];
    r.read_exact(&mut magic)
        .map_err(|e| format!("Cannot read GGUF magic: {e}"))?;
    if &magic != GGUF_MAGIC {
        return Err(format!(
            "'{}' is not a GGUF file (magic: {magic:02x?})",
            path.display()
        ));
    }

    // Version
    let version = read_u32(&mut r)?;
    if version == 0 || version > 3 {
        return Err(format!("Unsupported GGUF version {version}"));
    }

    // tensor_count and kv_count (u32 in v1, u64 in v2+)
    let (_tensor_count, kv_count) = if version == 1 {
        (read_u32(&mut r)? as u64, read_u32(&mut r)? as u64)
    } else {
        (read_u64(&mut r)?, read_u64(&mut r)?)
    };

    // Guard against pathological files
    if kv_count > 100_000 {
        return Err(format!("Implausible kv_count {kv_count}"));
    }

    // Read all KV pairs into a flat map.
    // We store scalar values; arrays are consumed/skipped.
    let mut kv: HashMap<String, KvValue> = HashMap::with_capacity(128);

    for _ in 0..kv_count {
        let key = read_str(&mut r, version)?;
        let vtype = read_u32(&mut r)?;
        let value = read_value(&mut r, vtype, version)?;
        kv.insert(key, value);
    }

    // ── Extract fields ────────────────────────────────────────────────────────
    let arch: Option<String> = kv
        .get("general.architecture")
        .and_then(KvValue::as_str)
        .map(|s| s.to_ascii_lowercase());

    let mut meta = GgufMetadata::default();
    meta.architecture = arch.clone();
    meta.param_count = kv.get("general.parameter_count").and_then(KvValue::as_u64);

    if let Some(a) = arch.as_deref() {
        macro_rules! get_u32 {
            ($($key:expr),+) => {
                None $(.or_else(|| kv.get(&format!("{a}.{}", $key)).and_then(KvValue::as_u32)))+
            };
        }
        macro_rules! get_u32_bare {
            ($($key:expr),+) => {
                None $(.or_else(|| kv.get($key).and_then(KvValue::as_u32)))+
            };
        }

        meta.block_count = get_u32!("block_count");
        meta.head_count = get_u32!("attention.head_count");
        meta.head_count_kv = get_u32!("attention.head_count_kv");
        meta.key_length = get_u32!("attention.key_length");
        meta.context_length = get_u32!("context_length");
        meta.embedding_length = get_u32!("embedding_length");
        meta.feed_forward_length = get_u32!("feed_forward_length");
        meta.expert_count = get_u32!("expert_count");
        meta.expert_used_count = get_u32!("expert_used_count");

        // MTP depth — key name varies across llama.cpp versions
        meta.mtp_depth = get_u32!(
            "next_n_token_count",
            "num_nextn_predict_layers",
            "multi_token_prediction_depth"
        )
        .or_else(|| get_u32_bare!("general.next_n_token_count"));
    }

    Ok(meta)
}

// ── Internal value type ───────────────────────────────────────────────────────

#[derive(Debug)]
enum KvValue {
    U32(u32),
    U64(u64),
    I32(i32),
    I64(i64),
    Str(String),
    Other, // bool, floats, arrays — skipped/irrelevant for architecture metadata
}

impl KvValue {
    fn as_u32(&self) -> Option<u32> {
        match self {
            KvValue::U32(v) => Some(*v),
            KvValue::U64(v) => u32::try_from(*v).ok(),
            KvValue::I32(v) => u32::try_from(*v).ok(),
            KvValue::I64(v) => u32::try_from(*v).ok(),
            _ => None,
        }
    }

    fn as_u64(&self) -> Option<u64> {
        match self {
            KvValue::U32(v) => Some(*v as u64),
            KvValue::U64(v) => Some(*v),
            KvValue::I32(v) => u64::try_from(*v).ok(),
            KvValue::I64(v) => u64::try_from(*v).ok(),
            _ => None,
        }
    }

    fn as_str(&self) -> Option<&str> {
        if let KvValue::Str(s) = self {
            Some(s)
        } else {
            None
        }
    }
}

// ── Binary readers ────────────────────────────────────────────────────────────

fn read_u8(r: &mut impl Read) -> Result<u8, String> {
    let mut b = [0u8; 1];
    r.read_exact(&mut b).map_err(|e| format!("read u8: {e}"))?;
    Ok(b[0])
}

fn read_u16(r: &mut impl Read) -> Result<u16, String> {
    let mut b = [0u8; 2];
    r.read_exact(&mut b).map_err(|e| format!("read u16: {e}"))?;
    Ok(u16::from_le_bytes(b))
}

fn read_u32(r: &mut impl Read) -> Result<u32, String> {
    let mut b = [0u8; 4];
    r.read_exact(&mut b).map_err(|e| format!("read u32: {e}"))?;
    Ok(u32::from_le_bytes(b))
}

fn read_u64(r: &mut impl Read) -> Result<u64, String> {
    let mut b = [0u8; 8];
    r.read_exact(&mut b).map_err(|e| format!("read u64: {e}"))?;
    Ok(u64::from_le_bytes(b))
}

/// Read a GGUF string. v1 uses a u32 length prefix; v2+ use u64.
fn read_str<R: Read>(r: &mut R, version: u32) -> Result<String, String> {
    let len = if version == 1 {
        read_u32(r)? as u64
    } else {
        read_u64(r)?
    };
    if len > 4_000_000 {
        return Err(format!("String too long ({len} bytes) — likely corrupt"));
    }
    let mut buf = vec![0u8; len as usize];
    r.read_exact(&mut buf)
        .map_err(|e| format!("read str body: {e}"))?;
    String::from_utf8(buf).map_err(|e| format!("string not UTF-8: {e}"))
}

/// Read a single GGUF value of the given type, returning a `KvValue`.
/// Fixed-size non-architecture types are consumed and discarded (returned as `Other`).
fn read_value<R: Read + Seek>(r: &mut R, vtype: u32, version: u32) -> Result<KvValue, String> {
    match vtype {
        TYPE_UINT8 => Ok(KvValue::U32(read_u8(r)? as u32)),
        TYPE_INT8 => Ok(KvValue::I32(read_u8(r)? as i8 as i32)),
        TYPE_UINT16 => Ok(KvValue::U32(read_u16(r)? as u32)),
        TYPE_INT16 => Ok(KvValue::I32(read_u16(r)? as i16 as i32)),
        TYPE_UINT32 => Ok(KvValue::U32(read_u32(r)?)),
        TYPE_INT32 => Ok(KvValue::I32(read_u32(r)? as i32)),
        TYPE_FLOAT32 => {
            r.seek(SeekFrom::Current(4))
                .map_err(|e| format!("seek f32: {e}"))?;
            Ok(KvValue::Other)
        }
        TYPE_BOOL => {
            let _ = read_u8(r)?;
            Ok(KvValue::Other)
        }
        TYPE_STRING => Ok(KvValue::Str(read_str(r, version)?)),
        TYPE_UINT64 => Ok(KvValue::U64(read_u64(r)?)),
        TYPE_INT64 => Ok(KvValue::I64(read_u64(r)? as i64)),
        TYPE_FLOAT64 => {
            r.seek(SeekFrom::Current(8))
                .map_err(|e| format!("seek f64: {e}"))?;
            Ok(KvValue::Other)
        }
        TYPE_ARRAY => {
            skip_array(r, version)?;
            Ok(KvValue::Other)
        }
        other => Err(format!("Unknown GGUF value type {other}")),
    }
}

/// Skip an array value: read the element type and count, then seek/read past all elements.
fn skip_array<R: Read + Seek>(r: &mut R, version: u32) -> Result<(), String> {
    let elem_type = read_u32(r)?;
    let n = if version == 1 {
        read_u32(r)? as u64
    } else {
        read_u64(r)?
    };

    // For fixed-size element types, a single seek is much faster than iterating.
    let fixed_size: Option<u64> = match elem_type {
        TYPE_UINT8 | TYPE_INT8 | TYPE_BOOL => Some(1),
        TYPE_UINT16 | TYPE_INT16 => Some(2),
        TYPE_UINT32 | TYPE_INT32 | TYPE_FLOAT32 => Some(4),
        TYPE_UINT64 | TYPE_INT64 | TYPE_FLOAT64 => Some(8),
        _ => None,
    };

    if let Some(stride) = fixed_size {
        let total = n.saturating_mul(stride);
        r.seek(SeekFrom::Current(total as i64))
            .map_err(|e| format!("seek array: {e}"))?;
    } else {
        // STRING or nested ARRAY — must iterate (variable-length elements)
        for _ in 0..n {
            skip_value_type(r, elem_type, version)?;
        }
    }
    Ok(())
}

/// Skip a single value of `vtype` without returning it.
fn skip_value_type<R: Read + Seek>(r: &mut R, vtype: u32, version: u32) -> Result<(), String> {
    match vtype {
        TYPE_UINT8 | TYPE_INT8 | TYPE_BOOL => {
            let _ = read_u8(r)?;
        }
        TYPE_UINT16 | TYPE_INT16 => {
            let _ = read_u16(r)?;
        }
        TYPE_UINT32 | TYPE_INT32 | TYPE_FLOAT32 => {
            r.seek(SeekFrom::Current(4))
                .map_err(|e| format!("skip: {e}"))?;
        }
        TYPE_UINT64 | TYPE_INT64 | TYPE_FLOAT64 => {
            r.seek(SeekFrom::Current(8))
                .map_err(|e| format!("skip: {e}"))?;
        }
        TYPE_STRING => {
            let _ = read_str(r, version)?;
        }
        TYPE_ARRAY => skip_array(r, version)?,
        other => return Err(format!("Unknown type to skip: {other}")),
    }
    Ok(())
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    /// Build a minimal GGUF v3 byte stream in memory for testing.
    fn make_gguf(kv: &[(&str, KvEntry)]) -> Vec<u8> {
        let mut out = Vec::new();
        // Magic + version
        out.extend_from_slice(b"GGUF");
        out.extend_from_slice(&3u32.to_le_bytes()); // version 3
        out.extend_from_slice(&0u64.to_le_bytes()); // tensor_count = 0
        out.extend_from_slice(&(kv.len() as u64).to_le_bytes());

        for (key, entry) in kv {
            // key string (u64 len + bytes)
            out.extend_from_slice(&(key.len() as u64).to_le_bytes());
            out.extend_from_slice(key.as_bytes());
            // value type + value
            match entry {
                KvEntry::U32(v) => {
                    out.extend_from_slice(&TYPE_UINT32.to_le_bytes());
                    out.extend_from_slice(&v.to_le_bytes());
                }
                KvEntry::U64(v) => {
                    out.extend_from_slice(&TYPE_UINT64.to_le_bytes());
                    out.extend_from_slice(&v.to_le_bytes());
                }
                KvEntry::Str(s) => {
                    out.extend_from_slice(&TYPE_STRING.to_le_bytes());
                    out.extend_from_slice(&(s.len() as u64).to_le_bytes());
                    out.extend_from_slice(s.as_bytes());
                }
            }
        }
        out
    }

    enum KvEntry {
        U32(u32),
        U64(u64),
        Str(String),
    }

    fn read_from_bytes(bytes: &[u8]) -> Result<GgufMetadata, String> {
        use std::io::Cursor;
        struct RwCursor(Cursor<Vec<u8>>);
        impl Read for RwCursor {
            fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
                self.0.read(buf)
            }
        }
        impl Seek for RwCursor {
            fn seek(&mut self, pos: SeekFrom) -> std::io::Result<u64> {
                self.0.seek(pos)
            }
        }

        let mut r = RwCursor(Cursor::new(bytes.to_vec()));
        // Re-implement entry point over a Cursor for testing
        let mut magic = [0u8; 4];
        r.read_exact(&mut magic).unwrap();
        assert_eq!(&magic, b"GGUF");
        let version = read_u32(&mut r)?;
        let (_tc, kv_count) = (read_u64(&mut r)?, read_u64(&mut r)?);

        let mut kv: HashMap<String, KvValue> = HashMap::new();
        for _ in 0..kv_count {
            let key = read_str(&mut r, version)?;
            let vtype = read_u32(&mut r)?;
            let value = read_value(&mut r, vtype, version)?;
            kv.insert(key, value);
        }

        let arch: Option<String> = kv
            .get("general.architecture")
            .and_then(KvValue::as_str)
            .map(|s| s.to_ascii_lowercase());
        let mut meta = GgufMetadata::default();
        meta.architecture = arch.clone();
        meta.param_count = kv.get("general.parameter_count").and_then(KvValue::as_u64);
        if let Some(a) = arch.as_deref() {
            meta.block_count = kv
                .get(&format!("{a}.block_count"))
                .and_then(KvValue::as_u32);
            meta.head_count = kv
                .get(&format!("{a}.attention.head_count"))
                .and_then(KvValue::as_u32);
            meta.head_count_kv = kv
                .get(&format!("{a}.attention.head_count_kv"))
                .and_then(KvValue::as_u32);
            meta.key_length = kv
                .get(&format!("{a}.attention.key_length"))
                .and_then(KvValue::as_u32);
            meta.context_length = kv
                .get(&format!("{a}.context_length"))
                .and_then(KvValue::as_u32);
            meta.embedding_length = kv
                .get(&format!("{a}.embedding_length"))
                .and_then(KvValue::as_u32);
            meta.expert_count = kv
                .get(&format!("{a}.expert_count"))
                .and_then(KvValue::as_u32);
            meta.expert_used_count = kv
                .get(&format!("{a}.expert_used_count"))
                .and_then(KvValue::as_u32);
        }
        Ok(meta)
    }

    #[test]
    fn parses_qwen36_metadata() {
        let bytes = make_gguf(&[
            ("general.architecture", KvEntry::Str("qwen3_6".into())),
            ("general.parameter_count", KvEntry::U64(27_000_000_000)),
            ("qwen3_6.block_count", KvEntry::U32(64)),
            ("qwen3_6.attention.head_count", KvEntry::U32(24)),
            ("qwen3_6.attention.head_count_kv", KvEntry::U32(4)),
            ("qwen3_6.attention.key_length", KvEntry::U32(256)),
            ("qwen3_6.context_length", KvEntry::U32(262144)),
            ("qwen3_6.embedding_length", KvEntry::U32(5120)),
        ]);
        let meta = read_from_bytes(&bytes).unwrap();
        assert_eq!(meta.architecture.as_deref(), Some("qwen3_6"));
        assert_eq!(meta.block_count, Some(64));
        assert_eq!(meta.head_count_kv, Some(4));
        assert_eq!(meta.key_length, Some(256));
        assert_eq!(meta.context_length, Some(262144));
        assert!((meta.param_b().unwrap() - 27.0).abs() < 0.1);
    }

    #[test]
    fn parses_moe_expert_fields() {
        let bytes = make_gguf(&[
            ("general.architecture", KvEntry::Str("qwen3_6".into())),
            ("qwen3_6.block_count", KvEntry::U32(40)),
            ("qwen3_6.expert_count", KvEntry::U32(256)),
            ("qwen3_6.expert_used_count", KvEntry::U32(9)),
            ("qwen3_6.attention.head_count_kv", KvEntry::U32(2)),
        ]);
        let meta = read_from_bytes(&bytes).unwrap();
        assert_eq!(meta.expert_count, Some(256));
        assert_eq!(meta.expert_used_count, Some(9));
        assert_eq!(meta.head_count_kv, Some(2));
    }

    #[test]
    fn to_model_metadata_sets_gguf_arch() {
        let bytes = make_gguf(&[
            ("general.architecture", KvEntry::Str("qwen3_6".into())),
            ("qwen3_6.block_count", KvEntry::U32(64)),
            ("qwen3_6.attention.head_count_kv", KvEntry::U32(4)),
        ]);
        let gguf = read_from_bytes(&bytes).unwrap();
        let mm = gguf.to_model_metadata();
        assert_eq!(mm.gguf_arch.as_deref(), Some("qwen3_6"));
        assert_eq!(mm.n_layers, Some(64));
        assert_eq!(mm.n_kv_heads, Some(4));
    }

    #[test]
    fn rejects_non_gguf_file() {
        let mut bytes = make_gguf(&[]);
        bytes[0] = b'X'; // corrupt magic
        // Can't use read_from_bytes since magic check is in the outer fn; test via tempfile
        let tmp = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(tmp.path(), &bytes).unwrap();
        let result = read_gguf_metadata(tmp.path());
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("not a GGUF"));
    }

    #[test]
    fn gguf_arch_drives_hybrid_heuristic_for_renamed_model() {
        // Simulate "Pantheon-Reasoning-27B" whose GGUF says qwen3_6.
        // to_arch() must select hybrid-DeltaNet heuristic, not standard_heuristic.
        let bytes = make_gguf(&[
            ("general.architecture", KvEntry::Str("qwen3_6".into())),
            ("qwen3_6.block_count", KvEntry::U32(64)),
            ("qwen3_6.attention.head_count_kv", KvEntry::U32(4)),
            ("qwen3_6.attention.key_length", KvEntry::U32(256)),
        ]);
        let gguf = read_from_bytes(&bytes).unwrap();
        let mm = gguf.to_model_metadata();
        let arch = mm.to_arch("Pantheon-Reasoning-27B-Q4_K_M.gguf", 27.0);
        assert!(
            arch.is_hybrid_attn(),
            "gguf_arch=qwen3_6 must yield hybrid-DeltaNet arch"
        );
        assert_eq!(
            arch.n_attn_layers, 16,
            "only 16 of 64 layers should have KV cache"
        );
    }
}
