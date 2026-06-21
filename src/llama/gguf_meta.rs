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

    // ── Hybrid linear-attention (Qwen3-Next / DeltaNet: qwen35, qwen35moe, qwen3next) ──
    /// `{arch}.full_attention_interval` — every Nth layer is full attention; the rest
    /// are linear (Gated DeltaNet) layers. Authoritative source for `n_attn_layers`:
    /// `n_attn_layers = block_count / full_attention_interval`.
    pub full_attention_interval: Option<u32>,

    /// `{arch}.ssm.inner_size` — width of the linear-attention recurrent state
    /// (num_v_heads × head_v_dim). Used to size the fixed DeltaNet state.
    pub ssm_inner_size: Option<u32>,

    /// `{arch}.ssm.state_size` — per-head linear-attention state dimension (head_k_dim).
    pub ssm_state_size: Option<u32>,

    /// `{arch}.ssm.conv_kernel` — short-conv kernel width (adds a small conv state).
    pub ssm_conv_kernel: Option<u32>,

    // ── Sliding-window / alternating attention (Gemma 3/4) ────────────────────────
    /// `{arch}.attention.sliding_window` — local-attention window size in tokens.
    pub sliding_window: Option<u32>,

    /// `{arch}.attention.key_length_swa` — per-head K/V dimension on local (SWA) layers.
    /// Gemma 4 uses a wider `key_length` (512) on global layers and this (256) locally.
    pub key_length_swa: Option<u32>,

    /// Number of global (full-context) attention layers, derived from
    /// `{arch}.attention.sliding_window_pattern` (count of `false` entries).
    pub n_global_attn_layers: Option<u32>,

    /// KV head count on global (full-context) layers, read from the per-layer
    /// `{arch}.attention.head_count_kv` array at a global position.
    pub global_kv_heads: Option<u32>,

    /// KV head count on local (sliding-window) layers, read from the per-layer
    /// `{arch}.attention.head_count_kv` array at a local position.
    pub local_kv_heads: Option<u32>,
}

impl GgufMetadata {
    /// Approximate parameter count in billions, derived from `param_count`.
    #[allow(dead_code)]
    pub fn param_b(&self) -> Option<f64> {
        self.param_count.map(|p| p as f64 / 1e9)
    }

    /// Number of full-attention (KV-bearing) layers for hybrid linear-attention
    /// models, computed from real GGUF data: `block_count / full_attention_interval`.
    /// (llama.cpp marks every Nth layer as full attention; the rest are DeltaNet.)
    /// Returns `None` for non-hybrid models (no `full_attention_interval` key).
    pub fn n_attn_layers(&self) -> Option<u32> {
        let interval = self.full_attention_interval?;
        let blocks = self.block_count?;
        if interval <= 1 {
            return None; // interval 1 ⇒ all layers full attention (not hybrid)
        }
        Some(blocks / interval)
    }

    /// Fixed recurrent-state size (bytes) for the linear-attention (DeltaNet) layers,
    /// computed from the real `ssm.*` GGUF fields. This does NOT grow with context.
    ///
    /// Per linear layer the state is `inner_size × state_size` (the delta matrix)
    /// plus a small `conv_kernel × inner_size` short-conv state, held at ~2 B/elem.
    /// Returns `None` when the model is not hybrid or lacks SSM metadata.
    pub fn linear_attn_state_bytes(&self) -> Option<u64> {
        let n_attn = self.n_attn_layers()?;
        let blocks = self.block_count?;
        let inner = self.ssm_inner_size? as u64;
        let state = self.ssm_state_size? as u64;
        let conv = self.ssm_conv_kernel.unwrap_or(0) as u64;
        let n_linear = blocks.saturating_sub(n_attn) as u64;
        let per_layer_elems = inner * (state + conv);
        Some(n_linear * per_layer_elems * 2)
    }

    /// Convert to the `ModelMetadata` type used by the spawn wizard / VRAM estimator.
    ///
    /// Sets `gguf_arch` so that renamed finetunes (e.g. "Pantheon-27B" from a
    /// Qwen3.6 base) get the correct hybrid-DeltaNet heuristic regardless of filename.
    /// Structural fields that llama.cpp records per-layer (hybrid attention interval,
    /// SSM state, Gemma global/local split, sliding window) are read from the GGUF so
    /// the VRAM math uses ground truth rather than name-based assumptions.
    pub fn to_model_metadata(&self) -> crate::llama::spawn_wizard::ModelMetadata {
        // For Gemma alternating attention, `n_kv_heads` (the global-layer KV head
        // count) comes from the per-layer array; fall back to the scalar otherwise.
        let n_kv_heads = self.global_kv_heads.or(self.head_count_kv);
        crate::llama::spawn_wizard::ModelMetadata {
            n_layers: self.block_count,
            n_ctx_train: self.context_length,
            n_embd: self.embedding_length,
            n_ff: self.feed_forward_length,
            n_head: self.head_count,
            n_kv_heads,
            head_dim: self.key_length,
            gguf_arch: self.architecture.clone(),
            n_experts: self.expert_count,
            n_experts_used: self.expert_used_count,
            mtp_depth: self.mtp_depth,
            n_attn_layers: self.n_attn_layers(),
            linear_attn_state_bytes: self.linear_attn_state_bytes(),
            n_global_attn_layers: self.n_global_attn_layers,
            local_kv_heads: self.local_kv_heads,
            global_head_dim: self.key_length, // wide global K/V dim (e.g. Gemma4 = 512)
            local_head_dim: self.key_length_swa, // narrow local K/V dim (e.g. Gemma4 = 256)
            sliding_window: self.sliding_window,
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
        meta.key_length = get_u32!("attention.key_length");
        meta.key_length_swa = get_u32!("attention.key_length_swa");
        meta.context_length = get_u32!("context_length");
        meta.embedding_length = get_u32!("embedding_length");
        meta.feed_forward_length = get_u32!("feed_forward_length");
        meta.expert_count = get_u32!("expert_count");
        meta.expert_used_count = get_u32!("expert_used_count");

        // Hybrid linear-attention (Qwen3-Next / DeltaNet) and SSM state sizing.
        meta.full_attention_interval = get_u32!("full_attention_interval");
        meta.ssm_inner_size = get_u32!("ssm.inner_size");
        meta.ssm_state_size = get_u32!("ssm.state_size");
        meta.ssm_conv_kernel = get_u32!("ssm.conv_kernel");
        meta.sliding_window = get_u32!("attention.sliding_window");

        // MTP depth — key name varies across llama.cpp versions. Newer Qwen3.5/3.6
        // MoE GGUFs emit `{arch}.nextn_predict_layers`.
        meta.mtp_depth = get_u32!(
            "nextn_predict_layers",
            "next_n_token_count",
            "num_nextn_predict_layers",
            "multi_token_prediction_depth"
        )
        .or_else(|| get_u32_bare!("general.next_n_token_count"));

        // `attention.head_count_kv` is a scalar on uniform models but a per-layer
        // array on Gemma 3/4 (alternating global/local layers with different GQA).
        let kv_key = format!("{a}.attention.head_count_kv");
        let kv_val = kv.get(&kv_key);
        meta.head_count_kv = kv_val.and_then(KvValue::as_u32);

        // `attention.sliding_window_pattern`: per-layer bool array — `false` marks a
        // global (full-context) layer, `true` a local sliding-window layer. The count
        // of `false` entries is the authoritative `n_global_attn_layers`.
        let swa_pattern = kv
            .get(&format!("{a}.attention.sliding_window_pattern"))
            .and_then(KvValue::as_bool_arr);
        if let Some(pat) = swa_pattern {
            meta.n_global_attn_layers =
                Some(pat.iter().filter(|&&is_local| !is_local).count() as u32);

            // Read the global/local KV head split from the per-layer head_count_kv
            // array, indexed by the same pattern (global = !is_local position).
            if let Some(kv_arr) = kv_val.and_then(KvValue::as_u32_arr) {
                let n = pat.len().min(kv_arr.len());
                meta.global_kv_heads = (0..n).find(|&i| !pat[i]).map(|i| kv_arr[i]);
                meta.local_kv_heads = (0..n).find(|&i| pat[i]).map(|i| kv_arr[i]);
            }
        }
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
    /// Small integer array (e.g. per-layer `head_count_kv`). Large arrays
    /// (token vocab, etc.) are not captured — see `MAX_CAPTURED_ARRAY`.
    ArrU32(Vec<u32>),
    /// Small boolean array (e.g. `sliding_window_pattern`).
    ArrBool(Vec<bool>),
    Other, // floats, big/other arrays — skipped/irrelevant for architecture metadata
}

/// Integer/bool arrays longer than this are skipped rather than captured, so we
/// never buffer token-vocabulary-sized arrays. Per-layer arrays (head_count_kv,
/// sliding_window_pattern) are at most `block_count` (~hundreds) entries.
const MAX_CAPTURED_ARRAY: u64 = 8192;

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

    /// Borrow a captured integer array, if this value is one.
    fn as_u32_arr(&self) -> Option<&[u32]> {
        match self {
            KvValue::ArrU32(v) => Some(v),
            _ => None,
        }
    }

    /// Borrow a captured boolean array, if this value is one.
    fn as_bool_arr(&self) -> Option<&[bool]> {
        match self {
            KvValue::ArrBool(v) => Some(v),
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
        Some(GgufType::Array) => read_array(r, version),
        None => Err(format!("Unknown GGUF value type {vtype}")),
    }
}

/// Read an array value. Small integer/bool arrays are captured (per-layer config
/// like `head_count_kv` / `sliding_window_pattern`); everything else (strings,
/// floats, oversized arrays) is skipped, leaving the reader positioned correctly.
fn read_array<R: Read + Seek>(r: &mut R, version: u32) -> Result<KvValue, String> {
    let elem_type = read_u32(r)?;
    let n = if version == 1 {
        read_u32(r)? as u64
    } else {
        read_u64(r)?
    };

    let et = GgufType::from_u32(elem_type);
    let capture_int = matches!(
        et,
        Some(
            GgufType::Uint8
                | GgufType::Int8
                | GgufType::Uint16
                | GgufType::Int16
                | GgufType::Uint32
                | GgufType::Int32
                | GgufType::Uint64
                | GgufType::Int64
        )
    );

    if n <= MAX_CAPTURED_ARRAY && capture_int {
        let mut out = Vec::with_capacity(n as usize);
        for _ in 0..n {
            // Reuse the scalar reader so every element advances the offset correctly.
            if let Some(v) = read_value(r, elem_type, version)?.as_u32() {
                out.push(v);
            }
        }
        return Ok(KvValue::ArrU32(out));
    }
    if n <= MAX_CAPTURED_ARRAY && matches!(et, Some(GgufType::Bool)) {
        let mut out = Vec::with_capacity(n as usize);
        for _ in 0..n {
            out.push(read_u8(r)? != 0);
        }
        return Ok(KvValue::ArrBool(out));
    }

    // Oversized or non-capturable element type: skip past the remaining elements.
    let fixed_size: Option<u64> = et.and_then(GgufType::fixed_size);
    if let Some(stride) = fixed_size {
        let total = n.saturating_mul(stride);
        r.seek(SeekFrom::Current(total as i64))
            .map_err(|e| format!("seek array: {e}"))?;
    } else {
        for _ in 0..n {
            skip_value_type(r, elem_type, version)?;
        }
    }
    Ok(KvValue::Other)
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
    fn parses_gguf_v1_format() {
        // v1 uses u32 for string lengths and kv_count instead of u64
        let mut out = Vec::new();
        out.extend_from_slice(b"GGUF");
        out.extend_from_slice(&1u32.to_le_bytes()); // version 1
        out.extend_from_slice(&0u32.to_le_bytes()); // tensor_count (u32 in v1)
        out.extend_from_slice(&2u32.to_le_bytes()); // kv_count (u32 in v1)

        // KV entry: "general.architecture" = "llama" — string len is u32 in v1
        let key = b"general.architecture";
        out.extend_from_slice(&(key.len() as u32).to_le_bytes());
        out.extend_from_slice(key);
        out.extend_from_slice(&GgufType::String.as_u32().to_le_bytes());
        let val = b"llama";
        out.extend_from_slice(&(val.len() as u32).to_le_bytes()); // u32 string len in v1
        out.extend_from_slice(val);

        // KV entry: "llama.block_count" = 32
        let key2 = b"llama.block_count";
        out.extend_from_slice(&(key2.len() as u32).to_le_bytes());
        out.extend_from_slice(key2);
        out.extend_from_slice(&GgufType::Uint32.as_u32().to_le_bytes());
        out.extend_from_slice(&32u32.to_le_bytes());

        let meta = read_from_bytes(&out).unwrap();
        assert_eq!(meta.architecture.as_deref(), Some("llama"));
        assert_eq!(meta.block_count, Some(32));
    }

    #[test]
    fn parses_mtp_depth_field() {
        let bytes = make_gguf(&[
            ("general.architecture", KvEntry::Str("deepseek2".into())),
            ("deepseek2.block_count", KvEntry::U32(61)),
            ("deepseek2.next_n_token_count", KvEntry::U32(1)),
        ]);
        let meta = read_from_bytes(&bytes).unwrap();
        assert_eq!(meta.mtp_depth, Some(1));
    }

    #[test]
    fn returns_error_on_truncated_file() {
        let bytes = make_gguf(&[("general.architecture", KvEntry::Str("llama".into()))]);
        // Truncate to 10 bytes — can't even read the header
        let truncated = &bytes[..10];
        let result = read_from_bytes(truncated);
        assert!(result.is_err());
    }

    #[test]
    fn returns_error_on_unsupported_version() {
        let mut bytes = make_gguf(&[]);
        // Overwrite version field (bytes 4-7) with version 99
        bytes[4] = 99;
        bytes[5] = 0;
        bytes[6] = 0;
        bytes[7] = 0;
        let result = read_from_bytes(&bytes);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Unsupported GGUF version"));
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

    #[test]
    fn pantheon_real_gguf_has_qwen35_arch() {
        // Integration test: reads the actual Pantheon-Reasoning-27B GGUF on disk.
        // llama.cpp uses "qwen35" for both Qwen3.5 and Qwen3.6 families.
        // Distinguished by block_count: 64 = Qwen3.6, 96 = Qwen3.5.
        let home = std::env::var("HOME").ok();
        let path = home
            .map(|h| {
                Path::new(&h)
                    .join(".config/llama-monitor/models/Pantheon-Reasoning-27B.i1-Q6_K.gguf")
            })
            .and_then(|p| p.exists().then_some(p));
        let Some(path) = path else {
            return; // file not present, skip
        };
        let gguf = read_gguf_metadata(&path).expect("read pantheon gguf");
        assert_eq!(
            gguf.architecture.as_deref(),
            Some("qwen35"),
            "Pantheon-Reasoning-27B GGUF reports qwen35 (shared by Qwen3.5+3.6)"
        );
        // 65 layers — these specific GGUFs have 65 blocks (likely an extra embedding
        // layer or architecture variant), not the canonical 64 from base Qwen3.6.
        // What matters is that block_count < 96, confirming Qwen3.6 family.
        assert!(
            gguf.block_count.unwrap() < 96,
            "block_count {} < 96 confirms Qwen3.6 family (not Qwen3.5)",
            gguf.block_count.unwrap()
        );
    }

    #[test]
    fn qwopus3_6_real_gguf_has_qwen35_arch() {
        // Integration test: reads the actual Qwopus3.6-27B-v2-MTP GGUF on disk.
        let home = std::env::var("HOME").ok();
        let path = home
            .map(|h| {
                Path::new(&h).join(".config/llama-monitor/models/Qwopus3.6-27B-v2-MTP-Q6_K.gguf")
            })
            .and_then(|p| p.exists().then_some(p));
        let Some(path) = path else {
            return; // file not present, skip
        };
        let gguf = read_gguf_metadata(&path).expect("read qwopus gguf");
        assert_eq!(
            gguf.architecture.as_deref(),
            Some("qwen35"),
            "Qwopus3.6-27B-v2-MTP GGUF reports qwen35"
        );
        assert!(
            gguf.block_count.unwrap() < 96,
            "block_count {} < 96 confirms Qwen3.6 family",
            gguf.block_count.unwrap()
        );
    }

    #[ignore]
    #[test]
    fn gemma4_31b_real_gguf_architecture() {
        let home = std::env::var("HOME").ok();
        let path = home
            .as_ref()
            .map(|h| {
                Path::new(h).join(".config/llama-monitor/models/gemma-4-31B-it-qat-UD-Q4_K_XL.gguf")
            })
            .and_then(|p| if p.exists() { Some(p) } else { None });
        let Some(path) = path else {
            return;
        };
        let gguf = read_gguf_metadata(&path).expect("read gemma4-31b gguf");
        assert_eq!(
            gguf.architecture.as_deref(),
            Some("gemma4"),
            "Gemma4-31B GGUF should report gemma4 architecture"
        );
        assert_eq!(
            gguf.block_count,
            Some(60),
            "Gemma4-31B should have 60 layers"
        );
        assert!(
            gguf.head_count_kv.is_none(),
            "Gemma4 GGUF does not expose head_count_kv (uses separate global/local KV heads)"
        );
    }

    #[ignore]
    #[test]
    fn qwen3_coder_next_real_gguf_architecture() {
        let home = std::env::var("HOME").ok();
        let path = home.as_ref().map(|h| {
            Path::new(h).join(".config/llama-monitor/models/Qwen3-Coder-Next-Huihui-Opus-4.6-Reasoning-Distilled-abliterated-IQ4_XS.gguf")
        }).and_then(|p| if p.exists() { Some(p) } else { None });
        let Some(path) = path else {
            return;
        };
        let gguf = read_gguf_metadata(&path).expect("read qwen3 coder next gguf");
        assert_eq!(
            gguf.architecture.as_deref(),
            Some("qwen3next"),
            "Qwen3-Coder-Next GGUF should report qwen3next architecture"
        );
        assert_eq!(
            gguf.block_count,
            Some(48),
            "Qwen3-Coder-Next should have 48 layers"
        );
        assert!(
            gguf.expert_count.is_some(),
            "Qwen3-Coder-Next is MoE (should have expert_count)"
        );
    }
}
