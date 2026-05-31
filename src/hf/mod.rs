//! HuggingFace Hub integration.
//!
//! Provides:
//! - Model search (keyword + author filter + sort)
//! - Author/org model browsing
//! - GGUF file listing with real sizes (HF tree API)
//! - Quant-type classification (standard / imatrix / unsloth-dynamic)
//! - Quant provider detection (bartowski / mradermacher / unsloth / etc.)
//! - Streaming download with resume support
//! - HF token management

#![allow(dead_code)]

use anyhow::{Context, Result};
use hf_hub::api::sync::ApiBuilder;
use hf_hub::{Repo, RepoType};
use std::path::Path;
use tokio::fs::File;
use tokio::io::AsyncWriteExt;

// ── Search sort options ───────────────────────────────────────────────────────

/// Sort order for HF model search / author browse.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum HfSort {
    #[default]
    Downloads, // most downloaded (best signal for quality community quants)
    Likes,     // most liked
    CreatedAt, // newest first
    Trending,  // HF trending score
}

impl HfSort {
    fn as_api_str(self) -> &'static str {
        match self {
            HfSort::Downloads => "downloads",
            HfSort::Likes => "likes",
            HfSort::CreatedAt => "createdAt",
            HfSort::Trending => "trendingScore",
        }
    }
}

// ── Quant-type and provider classification ────────────────────────────────────

/// How the GGUF file was quantized / what calibration was used.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum QuantFileType {
    #[default]
    Standard, // standard llama.cpp quants (Q4_K_M, Q5_K_M, Q8_0, …)
    Imatrix,        // importance-matrix calibrated (typically mradermacher's i1-* naming)
    UnslothDynamic, // Unsloth UD-* dynamic quants (mixed bpw per layer)
    BnB,            // bitsandbytes (rare in GGUF land but worth flagging)
    Unknown,
}

impl QuantFileType {
    pub fn label(self) -> &'static str {
        match self {
            QuantFileType::Standard => "Standard",
            QuantFileType::Imatrix => "imatrix",
            QuantFileType::UnslothDynamic => "UD (Unsloth)",
            QuantFileType::BnB => "BnB",
            QuantFileType::Unknown => "Unknown",
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            QuantFileType::Standard => "Standard llama.cpp quantization",
            QuantFileType::Imatrix => {
                "Importance-matrix calibrated — generally better quality at same size"
            }
            QuantFileType::UnslothDynamic => {
                "Unsloth dynamic quant — per-layer mixed bpw, excellent quality/size tradeoff"
            }
            QuantFileType::BnB => "bitsandbytes quantization",
            QuantFileType::Unknown => "Unknown quantization type",
        }
    }
}

/// Who made the quantization.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum QuantProvider {
    Bartowski,       // bartowski — standard Q4_K_M/Q5_K_M/Q8_0, extremely reliable
    Mradermacher,    // mradermacher — imatrix specialist, i1-* naming
    Unsloth,         // Unsloth — UD dynamic quants, best quality/size
    Lmstudio,        // LM Studio community quants
    TheBlokeRetired, // TheBloke (retired, legacy quants still widely used)
    #[default]
    Community, // other community quantizer
    Official,        // first-party / model author's own quants
}

impl QuantProvider {
    pub fn label(&self) -> &'static str {
        match self {
            QuantProvider::Bartowski => "bartowski",
            QuantProvider::Mradermacher => "mradermacher",
            QuantProvider::Unsloth => "unsloth",
            QuantProvider::Lmstudio => "LM Studio",
            QuantProvider::TheBlokeRetired => "TheBloke",
            QuantProvider::Community => "community",
            QuantProvider::Official => "official",
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            QuantProvider::Bartowski => {
                "Standard quants — Q4_K_M through Q8_0. Fast, reliable, no imatrix."
            }
            QuantProvider::Mradermacher => {
                "imatrix expert. i1-* files use importance calibration for better quality at same bpw."
            }
            QuantProvider::Unsloth => {
                "UD dynamic quants — mixed bpw per layer. Excellent quality/size. Also fine-tunes."
            }
            QuantProvider::Lmstudio => "LM Studio community quants.",
            QuantProvider::TheBlokeRetired => {
                "TheBloke (retired). Legacy GGUF quants; well-tested but not the latest models."
            }
            QuantProvider::Community => "Community quantizer.",
            QuantProvider::Official => "Model author's own quantization.",
        }
    }

    /// Infer provider from HF repo owner username.
    pub fn from_username(username: &str) -> Self {
        match username.to_ascii_lowercase().as_str() {
            "bartowski" => QuantProvider::Bartowski,
            "mradermacher" => QuantProvider::Mradermacher,
            "unsloth" => QuantProvider::Unsloth,
            "lmstudio-community" | "lmstudio" => QuantProvider::Lmstudio,
            "thebloke" => QuantProvider::TheBlokeRetired,
            "davidau" | "davidau-hf" => QuantProvider::Community,
            "mudler" => QuantProvider::Community,
            "jackrong" => QuantProvider::Community,
            _ => QuantProvider::Community,
        }
    }
}

/// Detect quant file type from GGUF filename patterns.
pub fn detect_quant_type(filename: &str) -> QuantFileType {
    let lower = filename.to_ascii_lowercase();
    // Unsloth UD: "UD-" anywhere or "-UD-" — e.g. "Qwen3.6-27B-UD-Q4_K_S.gguf"
    if lower.contains("-ud-") || lower.contains("/ud-") || lower.starts_with("ud-") {
        return QuantFileType::UnslothDynamic;
    }
    // Unsloth compact/apex variants
    if lower.contains("-unsloth-") || lower.contains("_unsloth_") {
        return QuantFileType::UnslothDynamic;
    }
    // Mradermacher imatrix: i1- prefix or .i1- pattern
    // e.g. "model.i1-Q4_K_M.gguf" or "model-i1-IQ3_S.gguf"
    if lower.contains(".i1-") || lower.contains("-i1-") {
        return QuantFileType::Imatrix;
    }
    // Generic imatrix hint
    if lower.contains("imatrix") || lower.contains(".imat") {
        return QuantFileType::Imatrix;
    }
    QuantFileType::Standard
}

// ── Model info structs ────────────────────────────────────────────────────────

/// Model result from search or author browse.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct SimpleModelInfo {
    pub id: String,
    pub gated: bool,
    pub tags: Vec<String>,
    pub downloads: u64,
    pub likes: u64,
    /// ISO 8601 creation timestamp from HF.
    pub created_at: String,
    /// Author/org username (the part before "/" in the repo id).
    pub author: String,
    /// Inferred from author username.
    pub quant_provider: QuantProvider,
    /// True if this is an imatrix or UD quant repo (detected from tags or name).
    pub has_imatrix: bool,
    /// Approximate parameter count inferred from model name (0 if unknown).
    pub param_b: f64,
    /// Base model the quantization derives from (from model card metadata).
    pub base_model: String,
}

/// A GGUF file in an HF repo, with real file size and quant classification.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct HfGgufFile {
    /// Path within the repo (e.g. "model-Q4_K_M.gguf").
    pub path: String,
    /// File size in bytes (0 if unknown). Fetched from HF tree API.
    pub size: u64,
    /// Short quant label (e.g. "Q4_K_M").
    pub label: String,
    /// Quant type classification.
    pub quant_type: QuantFileType,
    /// True if this is an importance-matrix calibrated file.
    pub is_imatrix: bool,
    /// mmproj companion hint: true if this looks like a vision projector.
    pub is_mmproj: bool,
}

/// High-level model info from HF (for the model info endpoint).
#[derive(Debug, Clone, serde::Serialize)]
pub struct HfModelInfo {
    pub repo_id: String,
    pub gated: bool,
    pub private: bool,
    pub tags: Vec<String>,
}

/// A single file in an HF repo.
#[derive(Debug, Clone, serde::Serialize)]
pub struct HfFileInfo {
    pub r#type: String,
    pub path: String,
    pub size: Option<u64>,
}

// ── Known GGUF quantizer list ─────────────────────────────────────────────────

/// Curated list of well-known GGUF quantizers shown as quick-picks in the wizard.
#[derive(Debug, Clone, serde::Serialize)]
pub struct KnownQuantizer {
    pub username: String,
    pub display_name: String,
    pub description: String,
    pub quant_style: &'static str, // "standard" | "imatrix" | "ud"
    pub note: Option<String>,
}

/// User-editable version of KnownQuantizer (all owned strings, round-trips through JSON).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct UserQuantizer {
    pub username: String,
    pub display_name: String,
    pub description: String,
    pub quant_style: String, // "standard" | "imatrix" | "ud"
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

impl From<&KnownQuantizer> for UserQuantizer {
    fn from(q: &KnownQuantizer) -> Self {
        UserQuantizer {
            username: q.username.clone(),
            display_name: q.display_name.clone(),
            description: q.description.clone(),
            quant_style: q.quant_style.to_string(),
            note: q.note.clone(),
        }
    }
}

/// Load user-customized quantizers from `config_dir/hf-quantizers.json`.
/// Returns None if the file does not exist (caller should fall back to defaults).
pub fn load_user_quantizers(config_dir: &std::path::Path) -> Option<Vec<UserQuantizer>> {
    let path = config_dir.join("hf-quantizers.json");
    let text = std::fs::read_to_string(&path).ok()?;
    serde_json::from_str(&text).ok()
}

/// Persist user-customized quantizers to `config_dir/hf-quantizers.json`.
pub fn save_user_quantizers(
    config_dir: &std::path::Path,
    quantizers: &[UserQuantizer],
) -> Result<()> {
    let path = config_dir.join("hf-quantizers.json");
    let json =
        serde_json::to_string_pretty(quantizers).context("Failed to serialize quantizers")?;
    std::fs::write(&path, json).context("Failed to write hf-quantizers.json")?;
    Ok(())
}

pub fn known_gguf_quantizers() -> Vec<KnownQuantizer> {
    vec![
        KnownQuantizer {
            username: "bartowski".into(),
            display_name: "bartowski".into(),
            description: "Standard GGUF quants — Q4_K_M through Q8_0. Most popular, extremely reliable.".into(),
            quant_style: "standard",
            note: None,
        },
        KnownQuantizer {
            username: "mradermacher".into(),
            display_name: "mradermacher".into(),
            description: "imatrix specialist. i1-* files use importance calibration for better quality at same bpw. Validates quantizations.".into(),
            quant_style: "imatrix",
            note: Some("i1-* files are imatrix quants; others are standard".into()),
        },
        KnownQuantizer {
            username: "unsloth".into(),
            display_name: "Unsloth".into(),
            description: "UD (Unsloth Dynamic) quants — mixed bpw per layer. Excellent quality/size. Also does fine-tuning and finetune-GGUF releases.".into(),
            quant_style: "ud",
            note: Some("UD-* files are dynamic quants; look for BF16 mmproj companions".into()),
        },
        KnownQuantizer {
            username: "lmstudio-community".into(),
            display_name: "LM Studio".into(),
            description: "LM Studio community quants.".into(),
            quant_style: "standard",
            note: None,
        },
        KnownQuantizer {
            username: "TheBloke".into(),
            display_name: "TheBloke (legacy)".into(),
            description: "Retired but prolific. Legacy GGUF quants for older models — extremely well-tested.".into(),
            quant_style: "standard",
            note: Some("Retired; no new quants. Legacy models still widely used.".into()),
        },
        // Community finetune quantizers of interest
        KnownQuantizer {
            username: "davidau".into(),
            display_name: "davidau".into(),
            description: "Fine-tune and merge specialist, often heretic/abliterated and uncensored variants.".into(),
            quant_style: "standard",
            note: None,
        },
        KnownQuantizer {
            username: "mudler".into(),
            display_name: "mudler".into(),
            description: "LocalAI author. Curated model selections and gguf releases.".into(),
            quant_style: "standard",
            note: None,
        },
        KnownQuantizer {
            username: "jackrong".into(),
            display_name: "jackrong".into(),
            description: "GGUF releases, often larger models.".into(),
            quant_style: "standard",
            note: None,
        },
    ]
}

// ── Core API functions ────────────────────────────────────────────────────────

/// Get basic model info for a repo (async, uses configured HF token).
pub async fn hf_get_model_info(repo_id: &str) -> Result<HfModelInfo> {
    let token = hf_load_token();
    let client = reqwest::Client::new();
    let url = format!("https://huggingface.co/api/models/{repo_id}");
    let mut req = client.get(&url);
    if let Some(ref tok) = token {
        req = req.bearer_auth(tok);
    }

    let resp = req.send().await.context("Failed to call HF models API")?;
    if !resp.status().is_success() {
        return Ok(HfModelInfo {
            repo_id: repo_id.to_string(),
            gated: false,
            private: false,
            tags: Vec::new(),
        });
    }

    let body: serde_json::Value = resp
        .json()
        .await
        .context("Failed to parse HF models API response")?;
    Ok(HfModelInfo {
        repo_id: repo_id.to_string(),
        gated: body.get("gated").and_then(|v| v.as_bool()).unwrap_or(false),
        private: body
            .get("private")
            .and_then(|v| v.as_bool())
            .unwrap_or(false),
        tags: body
            .get("tags")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|t| t.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default(),
    })
}

/// List repo files; filters for GGUF if gguf_only=true.
pub fn hf_list_repo_files(repo_id: &str, gguf_only: bool) -> Result<Vec<HfFileInfo>> {
    let api = ApiBuilder::new()
        .with_token(hf_load_token())
        .build()
        .context("Failed to build HF API client")?;
    let info = api
        .repo(Repo::new(repo_id.to_string(), RepoType::Model))
        .info()
        .context("Failed to list repo files")?;

    Ok(info
        .siblings
        .iter()
        .filter(|s| !gguf_only || s.rfilename.to_ascii_lowercase().ends_with(".gguf"))
        .map(|s| HfFileInfo {
            r#type: "file".into(),
            path: s.rfilename.clone(),
            size: None,
        })
        .collect())
}

/// Get info for a single file in a repo.
pub fn hf_get_file_info(repo_id: &str, path: &str) -> Result<HfFileInfo> {
    let api = ApiBuilder::new()
        .with_token(hf_load_token())
        .build()
        .context("Failed to build HF API client")?;
    let info = api
        .repo(Repo::new(repo_id.to_string(), RepoType::Model))
        .info()
        .context("Failed to list repo files")?;

    info.siblings
        .iter()
        .find(|s| s.rfilename == path)
        .map(|s| HfFileInfo {
            r#type: "file".into(),
            path: s.rfilename.clone(),
            size: None,
        })
        .ok_or_else(|| anyhow::anyhow!("File not found: {path}"))
}

/// Stream-download a file from HF with optional resume.
/// Returns total bytes written.
pub async fn hf_download_file_stream(
    repo_id: &str,
    path: &str,
    token: Option<&str>,
    local_path: &Path,
    resume_from: u64,
) -> Result<u64> {
    let api = ApiBuilder::new()
        .with_token(token.map(String::from))
        .build()
        .context("Failed to build HF API client")?;
    let url = api
        .repo(Repo::new(repo_id.to_string(), RepoType::Model))
        .url(path);
    if url.is_empty() {
        anyhow::bail!("Failed to resolve HF URL for {path}");
    }

    let client = reqwest::Client::new();
    let mut req = client.get(&url);
    if let Some(t) = token {
        req = req.bearer_auth(t);
    }
    if resume_from > 0 {
        req = req.header("Range", format!("bytes={}-", resume_from));
    }

    let resp = req.send().await.context("Failed to start HF download")?;
    if !resp.status().is_success() && resp.status() != reqwest::StatusCode::PARTIAL_CONTENT {
        anyhow::bail!(
            "HF download failed: HTTP {} for {}/{}",
            resp.status(),
            repo_id,
            path
        );
    }

    if let Some(parent) = local_path.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .context("Failed to create download dir")?;
    }

    // No content-length: read all at once
    if resp.content_length().is_none() {
        let bytes = resp.bytes().await.context("Failed to read response body")?;
        File::create(local_path)
            .await
            .context("Failed to create file")?
            .write_all(&bytes)
            .await?;
        return Ok(bytes.len() as u64);
    }

    // Streaming with proper open mode (truncate on fresh, append on resume)
    let mut file = if resume_from > 0 {
        File::options()
            .create(true)
            .write(true)
            .append(true)
            .open(local_path)
            .await
            .context("Failed to open file for resume")?
    } else {
        File::create(local_path)
            .await
            .context("Failed to create file")?
    };

    use futures_util::StreamExt;
    let mut stream = resp.bytes_stream();
    let mut written = resume_from;

    while let Some(chunk) = stream.next().await {
        let chunk = chunk.context("Stream error from HF")?;
        file.write_all(&chunk).await?;
        written += chunk.len() as u64;
    }
    Ok(written)
}

// ── Search and browse ─────────────────────────────────────────────────────────

/// Full search request parameters.
#[derive(Debug, Clone, Default)]
pub struct HfSearchParams {
    /// Keyword query. Can be empty when author is set.
    pub query: String,
    /// Filter to a specific author/org (e.g. "bartowski", "unsloth").
    pub author: Option<String>,
    pub sort: HfSort,
    pub limit: usize,
}

/// Search HuggingFace for GGUF models with keyword + optional author filter.
///
/// - Empty query + author = browse all of that author's GGUF models
/// - Non-empty query + no author = keyword search across all GGUF models
/// - Both = keyword search scoped to that author (uses both API params)
pub async fn hf_search_models(params: &HfSearchParams) -> Result<Vec<SimpleModelInfo>, String> {
    let limit = params.limit.clamp(1, 100);
    let token = hf_load_token();
    let client = reqwest::Client::new();

    let mut url = reqwest::Url::parse("https://huggingface.co/api/models")
        .map_err(|e| format!("Invalid HF API URL: {e}"))?;

    {
        let mut p = url.query_pairs_mut();
        if !params.query.is_empty() {
            p.append_pair("search", &params.query);
        }
        if let Some(ref author) = params.author {
            p.append_pair("author", author);
        }
        p.append_pair("limit", &limit.to_string());
        p.append_pair("sort", params.sort.as_api_str());
        p.append_pair("direction", "-1");
        // Always filter for GGUF
        p.append_pair("filter", "gguf");
    }

    let mut req = client.get(url);
    if let Some(ref tok) = token {
        req = req.bearer_auth(tok);
    }

    let resp = req
        .send()
        .await
        .map_err(|e| format!("HF search request failed: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("HF search failed: HTTP {}", resp.status()));
    }

    let items: Vec<serde_json::Value> = resp
        .json()
        .await
        .map_err(|e| format!("Failed to parse HF response: {e}"))?;

    Ok(items.into_iter().filter_map(parse_model_item).collect())
}

/// Browse all GGUF models from a specific HF author/org (convenience wrapper).
pub async fn hf_browse_author(
    author: &str,
    sort: HfSort,
    limit: usize,
) -> Result<Vec<SimpleModelInfo>, String> {
    hf_search_models(&HfSearchParams {
        query: String::new(),
        author: Some(author.to_string()),
        sort,
        limit,
    })
    .await
}

/// Parse a single model JSON object from the HF API into SimpleModelInfo.
fn parse_model_item(item: serde_json::Value) -> Option<SimpleModelInfo> {
    let id = item
        .get("id")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())?
        .to_string();

    let author = id.split('/').next().unwrap_or("").to_string();
    let quant_provider = QuantProvider::from_username(&author);

    let tags: Vec<String> = item
        .get("tags")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|t| t.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();

    let has_imatrix = tags.iter().any(|t| t == "imatrix")
        || id.to_ascii_lowercase().contains("imatrix")
        || id.to_ascii_lowercase().contains("-i1-")
        || matches!(quant_provider, QuantProvider::Mradermacher);

    let base_model = item
        .get("cardData")
        .and_then(|cd| cd.get("base_model"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    // Infer parameter count from repo name
    let param_b = infer_param_b_from_name(&id);

    Some(SimpleModelInfo {
        id,
        gated: item.get("gated").and_then(|v| v.as_bool()).unwrap_or(false),
        tags,
        downloads: item.get("downloads").and_then(|v| v.as_u64()).unwrap_or(0),
        likes: item.get("likes").and_then(|v| v.as_u64()).unwrap_or(0),
        created_at: item
            .get("createdAt")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        author,
        quant_provider,
        has_imatrix,
        param_b,
        base_model,
    })
}

/// Infer parameter count in billions from a model name/repo id.
fn infer_param_b_from_name(name: &str) -> f64 {
    // Match all NB patterns, take the largest (total params, not active)
    let matches: Vec<f64> = name
        .split(|c: char| !c.is_alphanumeric() && c != '.')
        .filter_map(|token| {
            let lower = token.to_ascii_lowercase();
            if lower.ends_with('b') {
                lower[..lower.len() - 1].parse::<f64>().ok()
            } else {
                None
            }
        })
        .filter(|&v| (0.5..=2000.0).contains(&v))
        .collect();
    matches.into_iter().fold(0.0_f64, f64::max)
}

// ── GGUF file listing with real sizes ────────────────────────────────────────

/// List GGUF files for a repo, fetching real file sizes from the HF tree API.
///
/// Falls back to 0-size entries if the tree API is unavailable (e.g. private repos
/// without a token, or rate limiting).
pub async fn hf_list_gguf_files(repo_id: &str) -> Result<Vec<HfGgufFile>, String> {
    let token = hf_load_token();

    // First try the HF tree API for real sizes
    let sizes = fetch_file_sizes(repo_id, token.as_deref())
        .await
        .unwrap_or_default();

    // Then get the file list from hf-hub
    let api = ApiBuilder::new()
        .with_token(token)
        .build()
        .map_err(|e| format!("Failed to build HF API client: {e}"))?;
    let info = api
        .repo(Repo::new(repo_id.to_string(), RepoType::Model))
        .info()
        .map_err(|e| format!("Failed to list repo files: {e}"))?;

    // Infer provider from repo owner
    let repo_owner = repo_id.split('/').next().unwrap_or("");
    let _provider = QuantProvider::from_username(repo_owner);

    let mut result: Vec<HfGgufFile> = info
        .siblings
        .iter()
        .map(|s| s.rfilename.as_str())
        .filter(|name| name.to_ascii_lowercase().ends_with(".gguf"))
        .map(|name| {
            let quant_type = detect_quant_type(name);
            let is_imatrix = matches!(quant_type, QuantFileType::Imatrix);
            let is_mmproj = name.to_ascii_lowercase().contains("mmproj")
                || name.to_ascii_lowercase().contains("projector");
            HfGgufFile {
                path: name.to_string(),
                size: sizes.get(name).copied().unwrap_or(0),
                label: infer_quant_label(name),
                quant_type,
                is_imatrix,
                is_mmproj,
            }
        })
        .collect();

    // Sort: mmproj last, then by quant quality (higher quality first), then by size desc
    result.sort_by(|a, b| {
        // mmproj files go at the end
        a.is_mmproj
            .cmp(&b.is_mmproj)
            .then_with(|| sort_rank_quant_label(&a.label).cmp(&sort_rank_quant_label(&b.label)))
            .then_with(|| b.size.cmp(&a.size))
    });

    Ok(result)
}

/// Fetch file sizes from the HF tree API.
/// Returns a map of filename → size in bytes.
async fn fetch_file_sizes(
    repo_id: &str,
    token: Option<&str>,
) -> Result<std::collections::HashMap<String, u64>> {
    let client = reqwest::Client::new();
    let url = format!("https://huggingface.co/api/models/{repo_id}/tree/main");
    let mut req = client.get(&url);
    if let Some(t) = token {
        req = req.bearer_auth(t);
    }

    let resp = req.send().await.context("Failed to fetch HF tree")?;
    if !resp.status().is_success() {
        anyhow::bail!("HF tree API returned {}", resp.status());
    }

    let items: Vec<serde_json::Value> = resp.json().await.context("Failed to parse HF tree")?;
    let mut map = std::collections::HashMap::new();

    for item in items {
        let path = item
            .get("path")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        if !path.to_ascii_lowercase().ends_with(".gguf") {
            continue;
        }

        // LFS files: size is in "lfs.size"; non-LFS: in "size"
        let size = item
            .get("lfs")
            .and_then(|lfs| lfs.get("size"))
            .and_then(|v| v.as_u64())
            .or_else(|| item.get("size").and_then(|v| v.as_u64()))
            .unwrap_or(0);

        if size > 0 {
            map.insert(path, size);
        }
    }

    Ok(map)
}

/// Sort rank for quant labels (lower = higher quality / bigger file = shown first).
fn sort_rank_quant_label(label: &str) -> u8 {
    let lower = label.to_ascii_lowercase();
    if lower.contains("f32") {
        return 0;
    }
    if lower.contains("f16") || lower.contains("bf16") {
        return 1;
    }
    if lower.contains("q8") {
        return 2;
    }
    if lower.contains("q6") {
        return 3;
    }
    if lower.contains("q5_k_m") || lower.contains("q5km") {
        return 4;
    }
    if lower.contains("q5") {
        return 5;
    }
    if lower.contains("q4_k_m") || lower.contains("q4km") {
        return 6;
    }
    if lower.contains("iq4") {
        return 7;
    }
    if lower.contains("q4") {
        return 8;
    }
    if lower.contains("q3") {
        return 10;
    }
    if lower.contains("iq3") {
        return 11;
    }
    if lower.contains("q2") {
        return 14;
    }
    if lower.contains("iq2") {
        return 15;
    }
    if lower.contains("iq1") || lower.contains("q1") {
        return 18;
    }
    19
}

/// Infer a human-readable quant label from a GGUF filename.
pub fn infer_quant_label(filename: &str) -> String {
    let lower = filename.to_ascii_lowercase();

    // IQ variants first (before plain Q matches)
    if lower.contains("iq4_xs") || lower.contains("iq4xs") {
        return "IQ4_XS".into();
    }
    if lower.contains("iq4_nl") || lower.contains("iq4nl") {
        return "IQ4_NL".into();
    }
    if lower.contains("iq3_xxs") {
        return "IQ3_XXS".into();
    }
    if lower.contains("iq3_xs") {
        return "IQ3_XS".into();
    }
    if lower.contains("iq3_s") {
        return "IQ3_S".into();
    }
    if lower.contains("iq3_m") {
        return "IQ3_M".into();
    }
    if lower.contains("iq2_xxs") {
        return "IQ2_XXS".into();
    }
    if lower.contains("iq2_xs") {
        return "IQ2_XS".into();
    }
    if lower.contains("iq2_s") {
        return "IQ2_S".into();
    }
    if lower.contains("iq2_m") {
        return "IQ2_M".into();
    }
    if lower.contains("iq1_m") {
        return "IQ1_M".into();
    }
    if lower.contains("iq1_s") {
        return "IQ1_S".into();
    }

    // Standard quants
    if lower.contains("q8_0") {
        return "Q8_0".into();
    }
    if lower.contains("q6_k") {
        return "Q6_K".into();
    }
    if lower.contains("q5_k_xl") {
        return "Q5_K_XL".into();
    } // Unsloth variant
    if lower.contains("q5_k_m") {
        return "Q5_K_M".into();
    }
    if lower.contains("q5_k_s") {
        return "Q5_K_S".into();
    }
    if lower.contains("q5_0") {
        return "Q5_0".into();
    }
    if lower.contains("q4_k_xl") {
        return "Q4_K_XL".into();
    } // Unsloth variant
    if lower.contains("q4_k_m") {
        return "Q4_K_M".into();
    }
    if lower.contains("q4_k_s") {
        return "Q4_K_S".into();
    }
    if lower.contains("q4_0") {
        return "Q4_0".into();
    }
    if lower.contains("q3_k_l") {
        return "Q3_K_L".into();
    }
    if lower.contains("q3_k_m") {
        return "Q3_K_M".into();
    }
    if lower.contains("q3_k_s") {
        return "Q3_K_S".into();
    }
    if lower.contains("q2_k") {
        return "Q2_K".into();
    }
    if lower.contains("bf16") {
        return "BF16".into();
    }
    if lower.contains("f16") {
        return "F16".into();
    }
    if lower.contains("f32") {
        return "F32".into();
    }

    // Compact/APEX/etc. Unsloth special naming
    if lower.contains("compact") || lower.contains("apex") {
        return "UD (custom)".into();
    }

    "Unknown".into()
}

// ── Token management ──────────────────────────────────────────────────────────

/// Load HF token: 1) HUGGING_FACE_HUB_TOKEN env var  2) ~/.config/llama-monitor/hf-token.
pub fn hf_load_token() -> Option<String> {
    if let Ok(v) = std::env::var("HUGGING_FACE_HUB_TOKEN")
        && !v.trim().is_empty()
    {
        return Some(v.trim().to_string());
    }
    dirs::home_dir().and_then(|home| {
        let path = home.join(".config").join("llama-monitor").join("hf-token");
        std::fs::read_to_string(&path)
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
    })
}

/// Save HF token to ~/.config/llama-monitor/hf-token.
pub fn hf_save_token(token: &str) -> Result<()> {
    let home = dirs::home_dir().ok_or_else(|| anyhow::anyhow!("Home directory not found"))?;
    let dir = home.join(".config").join("llama-monitor");
    std::fs::create_dir_all(&dir).context("Failed to create config dir")?;
    std::fs::write(dir.join("hf-token"), token.trim()).context("Failed to write HF token")?;
    Ok(())
}

/// Mask a token for safe logging: first4****last4.
pub fn mask_token(token: &str) -> String {
    let t = token.trim();
    if t.len() <= 8 {
        return "****".to_string();
    }
    format!("{}****{}", &t[..4], &t[t.len() - 4..])
}

// ── Start a managed download ──────────────────────────────────────────────────

/// Start a download via the model_download manager.
pub fn hf_start_download(
    repo_id: &str,
    file_path: &str,
    target_path: &Path,
    _resume: bool,
) -> Result<String, String> {
    crate::model_download::start_download(repo_id, file_path, target_path, hf_load_token())
        .map_err(|e| format!("Failed to start download: {e}"))
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_hf_get_model_info_smoke() {
        let _ = hf_get_model_info("gpt2").await;
    }

    #[test]
    fn test_infer_quant_label() {
        assert_eq!(infer_quant_label("model-Q4_K_M.gguf"), "Q4_K_M");
        assert_eq!(infer_quant_label("model-Q8_0.gguf"), "Q8_0");
        assert_eq!(infer_quant_label("model-bf16.gguf"), "BF16");
        assert_eq!(infer_quant_label("model-random.gguf"), "Unknown");
        assert_eq!(infer_quant_label("model.i1-IQ3_S.gguf"), "IQ3_S");
        assert_eq!(infer_quant_label("Qwen3.6-27B-UD-Q4_K_M.gguf"), "Q4_K_M");
        assert_eq!(infer_quant_label("model-Q5_K_XL.gguf"), "Q5_K_XL");
        assert_eq!(infer_quant_label("model-IQ2_XXS.gguf"), "IQ2_XXS");
    }

    #[test]
    fn test_detect_quant_type() {
        // Mradermacher imatrix
        assert_eq!(
            detect_quant_type("model.i1-Q4_K_M.gguf"),
            QuantFileType::Imatrix
        );
        assert_eq!(
            detect_quant_type("model-i1-IQ3_S.gguf"),
            QuantFileType::Imatrix
        );
        // Unsloth UD
        assert_eq!(
            detect_quant_type("Qwen3.6-27B-UD-Q4_K_S.gguf"),
            QuantFileType::UnslothDynamic
        );
        assert_eq!(
            detect_quant_type("model-UD-mmproj-BF16.gguf"),
            QuantFileType::UnslothDynamic
        );
        // Standard
        assert_eq!(
            detect_quant_type("model-Q4_K_M.gguf"),
            QuantFileType::Standard
        );
        assert_eq!(
            detect_quant_type("bartowski-model-Q5_K_M.gguf"),
            QuantFileType::Standard
        );
    }

    #[test]
    fn test_quant_provider_from_username() {
        assert!(matches!(
            QuantProvider::from_username("bartowski"),
            QuantProvider::Bartowski
        ));
        assert!(matches!(
            QuantProvider::from_username("mradermacher"),
            QuantProvider::Mradermacher
        ));
        assert!(matches!(
            QuantProvider::from_username("unsloth"),
            QuantProvider::Unsloth
        ));
        assert!(matches!(
            QuantProvider::from_username("davidau"),
            QuantProvider::Community
        ));
        assert!(matches!(
            QuantProvider::from_username("TheBloke"),
            QuantProvider::TheBlokeRetired
        ));
    }

    #[test]
    fn test_mask_token() {
        assert_eq!(mask_token("1234567890"), "1234****7890");
        assert_eq!(mask_token("1234"), "****");
    }

    #[test]
    fn test_infer_param_b_from_name() {
        assert_eq!(infer_param_b_from_name("bartowski/Qwen3.6-27B-GGUF"), 27.0);
        assert_eq!(
            infer_param_b_from_name("mradermacher/Qwen3.6-35B-A3B-Instruct-i1-GGUF"),
            35.0
        );
        assert_eq!(
            infer_param_b_from_name("unsloth/Llama-3.3-70B-Instruct-GGUF"),
            70.0
        );
        // Should be > 0
        assert!(infer_param_b_from_name("some-27b-model") > 0.0);
    }

    #[test]
    fn test_has_imatrix_detection() {
        let item = serde_json::json!({
            "id": "mradermacher/Qwen3.6-27B-i1-GGUF",
            "gated": false,
            "tags": ["gguf", "imatrix"],
            "downloads": 1000,
            "likes": 50,
        });
        let info = parse_model_item(item).unwrap();
        assert!(
            info.has_imatrix,
            "mradermacher repo should be flagged as imatrix"
        );
    }

    #[test]
    fn test_known_quantizers_has_expected_entries() {
        let quantizers = known_gguf_quantizers();
        let usernames: Vec<&str> = quantizers.iter().map(|q| q.username.as_str()).collect();
        assert!(usernames.contains(&"bartowski"));
        assert!(usernames.contains(&"mradermacher"));
        assert!(usernames.contains(&"unsloth"));
        assert!(usernames.contains(&"davidau"));
        assert!(usernames.contains(&"mudler"));
        assert!(usernames.contains(&"jackrong"));
    }

    #[test]
    fn test_sort_rank_quality_order() {
        // Higher quality should sort before lower quality
        assert!(sort_rank_quant_label("Q8_0") < sort_rank_quant_label("Q4_K_M"));
        assert!(sort_rank_quant_label("Q4_K_M") < sort_rank_quant_label("Q3_K_M"));
        assert!(sort_rank_quant_label("Q3_K_M") < sort_rank_quant_label("IQ2_XXS"));
    }

    #[test]
    fn test_simple_model_info_serde_default() {
        let json = r#"{"id":"test/model"}"#;
        let info: SimpleModelInfo = serde_json::from_str(json).expect("should deserialize");
        assert_eq!(info.id, "test/model");
        assert!(!info.gated);
        assert!(info.tags.is_empty());
        assert_eq!(info.downloads, 0);
    }

    #[test]
    fn test_hf_gguf_file_serde_default() {
        let json = r#"{"path":"file.gguf"}"#;
        let f: HfGgufFile = serde_json::from_str(json).expect("should deserialize");
        assert_eq!(f.path, "file.gguf");
        assert_eq!(f.size, 0);
    }
}
