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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
enum GgufType {
    Uint8 = 0,
    Int8 = 1,
    Uint16 = 2,
    Int16 = 3,
    Uint32 = 4,
    Int32 = 5,
    Float32 = 6,
    Bool = 7,
    String = 8,
    Array = 9,
    Uint64 = 10,
    Int64 = 11,
    Float64 = 12,
}

impl GgufType {
    fn from_u32(v: u32) -> Option<Self> {
        match v {
            0 => Some(Self::Uint8),
            1 => Some(Self::Int8),
            2 => Some(Self::Uint16),
            3 => Some(Self::Int16),
            4 => Some(Self::Uint32),
            5 => Some(Self::Int32),
            6 => Some(Self::Float32),
            7 => Some(Self::Bool),
            8 => Some(Self::String),
            9 => Some(Self::Array),
            10 => Some(Self::Uint64),
            11 => Some(Self::Int64),
            12 => Some(Self::Float64),
            _ => None,
        }
    }

    #[allow(dead_code)]
    fn as_u32(self) -> u32 {
        self as u32
    }

    fn fixed_size(self) -> Option<u64> {
        match self {
            Self::Uint8 | Self::Int8 | Self::Bool => Some(1),
            Self::Uint16 | Self::Int16 => Some(2),
            Self::Uint32 | Self::Int32 | Self::Float32 => Some(4),
            Self::Uint64 | Self::Int64 | Self::Float64 => Some(8),
            Self::String | Self::Array => None,
        }
    }
}

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
    #[allow(dead_code)]
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

    let mut meta = GgufMetadata {
        architecture: arch.clone(),
        param_count: kv.get("general.parameter_count").and_then(KvValue::as_u64),
        ..Default::default()
    };

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
    match GgufType::from_u32(vtype) {
        Some(GgufType::Uint8) => Ok(KvValue::U32(read_u8(r)? as u32)),
        Some(GgufType::Int8) => Ok(KvValue::I32(read_u8(r)? as i8 as i32)),
        Some(GgufType::Uint16) => Ok(KvValue::U32(read_u16(r)? as u32)),
        Some(GgufType::Int16) => Ok(KvValue::I32(read_u16(r)? as i16 as i32)),
        Some(GgufType::Uint32) => Ok(KvValue::U32(read_u32(r)?)),
        Some(GgufType::Int32) => Ok(KvValue::I32(read_u32(r)? as i32)),
        Some(GgufType::Float32) => {
            r.seek(SeekFrom::Current(4))
                .map_err(|e| format!("seek f32: {e}"))?;
            Ok(KvValue::Other)
        }
        Some(GgufType::Bool) => {
            let _ = read_u8(r)?;
            Ok(KvValue::Other)
        }
        Some(GgufType::String) => Ok(KvValue::Str(read_str(r, version)?)),
        Some(GgufType::Uint64) => Ok(KvValue::U64(read_u64(r)?)),
        Some(GgufType::Int64) => Ok(KvValue::I64(read_u64(r)? as i64)),
        Some(GgufType::Float64) => {
            r.seek(SeekFrom::Current(8))
                .map_err(|e| format!("seek f64: {e}"))?;
            Ok(KvValue::Other)
        }
        Some(GgufType::Array) => {
            skip_array(r, version)?;
            Ok(KvValue::Other)
        }
        None => Err(format!("Unknown GGUF value type {vtype}")),
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
    let fixed_size: Option<u64> = GgufType::from_u32(elem_type).and_then(GgufType::fixed_size);

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
    match GgufType::from_u32(vtype) {
        Some(GgufType::Uint8 | GgufType::Int8 | GgufType::Bool) => {
            let _ = read_u8(r)?;
        }
        Some(GgufType::Uint16 | GgufType::Int16) => {
            let _ = read_u16(r)?;
        }
        Some(GgufType::Uint32 | GgufType::Int32 | GgufType::Float32) => {
            r.seek(SeekFrom::Current(4))
                .map_err(|e| format!("skip: {e}"))?;
        }
        Some(GgufType::Uint64 | GgufType::Int64 | GgufType::Float64) => {
            r.seek(SeekFrom::Current(8))
                .map_err(|e| format!("skip: {e}"))?;
        }
        Some(GgufType::String) => {
            let _ = read_str(r, version)?;
        }
        Some(GgufType::Array) => skip_array(r, version)?,
        None => return Err(format!("Unknown type to skip: {vtype}")),
    }
    Ok(())
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a minimal GGUF v3 byte stream in memory for testing.
    fn make_gguf(kv: &[(&str, KvEntry)]) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(b"GGUF");
        out.extend_from_slice(&3u32.to_le_bytes());
        out.extend_from_slice(&0u64.to_le_bytes());
        out.extend_from_slice(&(kv.len() as u64).to_le_bytes());

        for (key, entry) in kv {
            out.extend_from_slice(&(key.len() as u64).to_le_bytes());
            out.extend_from_slice(key.as_bytes());
            match entry {
                KvEntry::U32(v) => {
                    out.extend_from_slice(&GgufType::Uint32.as_u32().to_le_bytes());
                    out.extend_from_slice(&v.to_le_bytes());
                }
                KvEntry::U64(v) => {
                    out.extend_from_slice(&GgufType::Uint64.as_u32().to_le_bytes());
                    out.extend_from_slice(&v.to_le_bytes());
                }
                KvEntry::Str(s) => {
                    out.extend_from_slice(&GgufType::String.as_u32().to_le_bytes());
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
        let tmp = tempfile::NamedTempFile::new().map_err(|e| format!("tempfile: {e}"))?;
        std::fs::write(tmp.path(), bytes).map_err(|e| format!("write: {e}"))?;
        read_gguf_metadata(tmp.path())
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
        bytes[0] = b'X';
        let result = read_from_bytes(&bytes);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("not a GGUF"));
    }

    #[test]
    fn gguf_arch_drives_hybrid_heuristic_for_renamed_model() {
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
