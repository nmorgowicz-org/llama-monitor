use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use warp::Filter;
use warp::http::StatusCode;

use crate::config::AppConfig;
use crate::inference::InferenceBackend;
use crate::inference::rapid_mlx::discovery::Discovery;
use crate::state::AppState;

use super::common::{ApiCtx, ApiRoute, check_api_token, unauthorized_api_token};

// ── Phase 2: POST /api/benchmark (with 15-second cooldown) ────────────────────

static BENCHMARK_LAST_TS: std::sync::LazyLock<std::sync::Mutex<u64>> =
    std::sync::LazyLock::new(|| std::sync::Mutex::new(0u64));

/// Build the argv (excluding the binary itself) for a `rapid-mlx bench` invocation.
/// `rapid-mlx bench` requires a positional `model` argument immediately after the
/// `bench` subcommand — without it the CLI exits with code 2 ("the following
/// arguments are required: model"). Kept as a pure function so the exact argument
/// order/shape can be unit-tested without spawning a process.
fn build_rapid_mlx_bench_args(model: &str, base_url: &str, tier: &str) -> Vec<String> {
    vec![
        "bench".to_string(),
        model.to_string(),
        "--base-url".to_string(),
        base_url.to_string(),
        "--tier".to_string(),
        tier.to_string(),
    ]
}

/// Run `rapid-mlx bench <model> --base-url ... --tier speed` against a running
/// server and parse its text output into normalized benchmark metrics.
async fn run_rapid_mlx_bench(
    base_url: &str,
    model: &str,
    binary_path: Option<PathBuf>,
    managed_path: Option<PathBuf>,
) -> Result<Option<(f64, f64, f64)>, String> {
    if model.is_empty() {
        return Err(
            "No model identity is known for the active Rapid-MLX session; rapid-mlx bench requires a model argument.".to_string(),
        );
    }

    let (binary, _) = Discovery::resolve_binary(binary_path.as_deref(), managed_path.as_deref())
        .await
        .map_err(|e| format!("rapid-mlx binary not found: {e}"))?;

    // Version-guard the text scrape: below MIN_TRUSTED_MINOR the bench output
    // layout is not verified, so degrade to a clear no-result instead of risking
    // a garbage parse. Mirrors the same guard used for `rapid-mlx info` in
    // info_query.rs.
    let version_trusted = matches!(
        crate::inference::rapid_mlx::info_query::cached_version(&binary).await,
        Ok(Some((_, minor))) if minor >= crate::inference::rapid_mlx::info_query::MIN_TRUSTED_MINOR
    );
    if !version_trusted {
        return Err(
            "Unsupported rapid-mlx version: bench output parsing is only verified from 0.10.x onward. Update rapid-mlx or use the llama.cpp backend.".to_string(),
        );
    }

    let args = build_rapid_mlx_bench_args(model, base_url, "speed");

    let output = tokio::time::timeout(
        Duration::from_secs(90),
        tokio::process::Command::new(&binary).args(&args).output(),
    )
    .await
    .map_err(|_| "rapid-mlx bench timed out after 90 s".to_string())?
    .map_err(|e| format!("failed to run rapid-mlx bench: {e}"))?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let text = format!("{stdout}\n{stderr}");

    let prompt_tps = parse_rapid_mlx_throughput(&text, "prompt")
        .or_else(|| parse_rapid_mlx_throughput(&text, "prefill"));
    let gen_tps = parse_rapid_mlx_throughput(&text, "generation")
        .or_else(|| parse_rapid_mlx_throughput(&text, "gen"));
    let ttft_ms = parse_rapid_mlx_ttft(&text);

    match (prompt_tps, gen_tps, ttft_ms) {
        (Some(pps), Some(gps), Some(ttft)) => Ok(Some((pps, gps, ttft))),
        _ => Ok(None),
    }
}

/// Extract a throughput number (tokens/s) from rapid-mlx bench text output.
/// Matches legacy label-based patterns like "prompt: 42.3 t/s" or "Prefill throughput: 85 tokens/s",
/// plus the real CLI's key=value format: "tps=198.3" on the PASS line (generation throughput).
fn parse_rapid_mlx_throughput(text: &str, label: &str) -> Option<f64> {
    let label_lower = label.to_lowercase();
    for line in text.lines() {
        let lower = line.to_lowercase();
        if lower.contains(&label_lower)
            && (lower.contains("t/s") || lower.contains("tokens/s") || lower.contains("tok/s"))
            && let Some(val) = extract_first_float(line)
        {
            return Some(val);
        }
    }
    // Real rapid-mlx 0.10.x CLI emits a single `tps=` value on the PASS line
    // (generation throughput). Match it for labels "generation"/"gen"/"throughput".
    if label_lower == "generation" || label_lower == "gen" || label_lower == "throughput" {
        for line in text.lines() {
            if let Some(idx) = line.find("tps=")
                && let Some(val) = extract_first_float(&line[idx + 4..])
            {
                return Some(val);
            }
        }
    }
    None
}

/// Extract TTFT in milliseconds from rapid-mlx bench text output.
/// Matches legacy label-based patterns like "TTFT: 123 ms" or "time to first token: 0.42 s",
/// plus the real CLI's key=value format: "ttft=140ms" on the PASS line (smoke tier).
fn parse_rapid_mlx_ttft(text: &str) -> Option<f64> {
    // Real rapid-mlx 0.10.x CLI emits `ttft=140ms` on the PASS line (smoke tier).
    for line in text.lines() {
        if let Some(idx) = line.find("ttft=") {
            let rest = &line[idx + 5..];
            if let Some(val) = extract_first_float(rest) {
                if rest.contains("ms") {
                    return Some(val);
                } else if rest.contains("s") && !rest.contains("ms") {
                    return Some(val * 1000.0);
                }
                return Some(val);
            }
        }
    }
    // Legacy label-based patterns
    for line in text.lines() {
        let lower = line.to_lowercase();
        if (lower.contains("ttft")
            || lower.contains("time to first token")
            || lower.contains("first token"))
            && let Some(val) = extract_first_float(line)
        {
            if lower.contains("ms") {
                return Some(val);
            } else if lower.contains("s") {
                return Some(val * 1000.0);
            }
            return Some(val);
        }
    }
    None
}

/// Extract the first floating-point number from a line.
fn extract_first_float(line: &str) -> Option<f64> {
    let trimmed = line.trim();
    let bytes = trimmed.as_bytes();
    let mut i = 0;
    while i < bytes.len() && !bytes[i].is_ascii_digit() {
        i += 1;
    }
    if i >= bytes.len() {
        return None;
    }
    let mut j = i;
    let mut has_dot = false;
    while j < bytes.len() && (bytes[j].is_ascii_digit() || (!has_dot && bytes[j] == b'.')) {
        if bytes[j] == b'.' {
            has_dot = true;
        }
        j += 1;
    }
    if j <= i {
        return None;
    }
    let slice = &trimmed[i..j];
    slice.parse().ok()
}

fn api_benchmark(
    state: AppState,
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (Box<dyn warp::reply::Reply>,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "benchmark")
        .and(warp::post())
        .and(warp::header::optional::<String>("authorization"))
        .and(super::super::hf_json_body::<serde_json::Value>())
        .and_then(
            move |auth: Option<String>,
                  body: serde_json::Value|
                  {
                let state = state.clone();
                let cfg = app_config.clone();
                async move {
                    // Auth
                    if !check_api_token(&auth, &cfg) {
                        return Ok(unauthorized_api_token());
                    }

                    // When tuning is active, skip cooldown so user gets live feedback.
                    let tuning = body.get("tuning")
                        .and_then(serde_json::Value::as_bool)
                        .unwrap_or(false);

                    if !tuning {
                        // Cooldown to prevent repeated heavy loads on the running llama-server.
                        let now = std::time::SystemTime::UNIX_EPOCH
                            .elapsed()
                            .unwrap_or_default()
                            .as_secs();
                        let mut last = BENCHMARK_LAST_TS.lock().unwrap();
                        if now.saturating_sub(*last) < 15 {
                            let remaining = 15 - (now.saturating_sub(*last));
                            return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(
                                Box::new(warp::reply::with_status(
                                    warp::reply::json(&serde_json::json!({
                                        "ok": false,
                                        "error": "Benchmark rate limited. Try again in 15 seconds.",
                                        "seconds_remaining": remaining
                                    })),
                                    StatusCode::TOO_MANY_REQUESTS,
                                )),
                            );
                        }
                        *last = now;
                    }

                    // Ensure a server is running
                    let running = match state.server_running.lock() {
                        Ok(g) => *g,
                        Err(_) => {
                            return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(
                                Box::new(warp::reply::json(&serde_json::json!({
                                    "error": "No inference runtime is currently running."
                                }))),
                            );
                        }
                    };
                    if !running {
                        return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(
                            Box::new(warp::reply::json(&serde_json::json!({
                                "error": "No inference runtime is currently running."
                            }))),
                        );
                    }

                    let prepared = match super::upstream::prepare_inference_request(&state).await {
                        Ok(prepared) => prepared,
                        Err(error) => return Err(error),
                    };

                    // Backend-specific benchmark path
                    if prepared.backend == InferenceBackend::RapidMlx {
                        let base_url = prepared.url.clone();
                        let binary_path = state
                            .local_launch_request
                            .lock()
                            .unwrap()
                            .as_ref()
                            .and_then(|req| match req {
                                crate::inference::launch::LocalLaunchRequest::RapidMlx(cfg) => {
                                    cfg.executable_path.clone()
                                }
                                _ => None,
                            });
                        let managed_path = state
                            .local_launch_request
                            .lock()
                            .unwrap()
                            .as_ref()
                            .and_then(|req| match req {
                                crate::inference::launch::LocalLaunchRequest::RapidMlx(cfg) => {
                                    cfg.managed_runtime_path.clone()
                                }
                                _ => None,
                            });

                        let model = prepared.model_identity().to_string();
                        let bench_result =
                            run_rapid_mlx_bench(&base_url, &model, binary_path, managed_path)
                                .await;
                        match bench_result {
                            Ok(Some((prompt_tps, gen_tps, ttft_ms))) => {
                                let benchmark =
                                    crate::llama::spawn_wizard::classify_benchmark_result(
                                        prompt_tps,
                                        gen_tps,
                                        ttft_ms,
                                        None,
                                        None,
                                        0,
                                    );
                                return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(
                                    Box::new(warp::reply::json(&serde_json::json!({
                                        "prompt_tokens_per_second": (benchmark.prompt_tokens_per_second * 100.0).round() / 100.0,
                                        "gen_tokens_per_second": (benchmark.gen_tokens_per_second * 100.0).round() / 100.0,
                                        "time_to_first_token_ms": (benchmark.time_to_first_token_ms * 100.0).round() / 100.0,
                                        "verdict": benchmark.verdict,
                                        "hints": benchmark.hints,
                                        "suggestions": benchmark.suggestions,
                                    }))),
                                );
                            }
                            Ok(None) => {
                                return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(
                                    Box::new(warp::reply::json(&serde_json::json!({
                                        "error": "rapid-mlx bench completed but output could not be parsed. The tool's output format may have changed."
                                    }))),
                                );
                            }
                            Err(e) => {
                                return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(
                                    Box::new(warp::reply::json(&serde_json::json!({
                                        "error": format!("rapid-mlx bench failed: {e}")
                                    }))),
                                );
                            }
                        }
                    }

                    // ── llama.cpp path (HTTP-driven against running server) ──────

                    let url = prepared.url.clone();

                    let prompt =
                        "Explain in one sentence what llama.cpp is used for.";
                    let max_tokens: u64 = 512;

                    let payload = serde_json::json!({
                        "messages": [{"role": "user", "content": prompt}],
                        "max_tokens": max_tokens,
                        "temperature": 0.5,
                        "stream": true,
                        // Disable thinking mode so Qwen3 reasoning tokens don't inflate TTFT
                        "chat_template_kwargs": {"enable_thinking": false},
                    });
                    let payload = prepared.map_chat_body(
                        &serde_json::to_vec(&payload).map_err(|error| {
                            warp::reject::custom(super::ApiError::internal(error.to_string()))
                        })?,
                    )?;

                    let client = match reqwest::Client::builder()
                        .timeout(Duration::from_secs(55))
                        .build()
                    {
                        Ok(c) => c,
                        Err(_) => {
                            return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(
                                Box::new(warp::reply::json(&serde_json::json!({
                                    "error": "Failed to create HTTP client."
                                }))),
                            );
                        }
                    };

                    let result = tokio::time::timeout(
                        Duration::from_secs(60),
                        async {
                            let start = std::time::Instant::now();
                            let mut first_token_time = None;
                            let mut buf = String::new();
                            let mut generated_tokens = 0u64;
                            let mut prompt_tokens_reported = 0u64;

                            let resp = match prepared
                                .authenticate(
                                    client
                                        .post(&url)
                                        .header("Content-Type", "application/json")
                                        .body(payload.clone()),
                                )
                                .send()
                                .await
                            {
                                Ok(r) => r,
                                Err(_) => return None,
                            };

                            if !resp.status().is_success() {
                                return None;
                            }

                            let mut stream = resp.bytes_stream();
                            use futures_util::StreamExt;

                            while let Some(Ok(chunk)) = stream.next().await {
                                let s = match std::str::from_utf8(&chunk) {
                                    Ok(s) => s.to_string(),
                                    Err(_) => continue,
                                };

                                // Try to parse streaming tokens
                                for line in s.lines() {
                                    let trimmed = line.trim();
                                    if let Some(data) = trimmed.strip_prefix("data: ") {
                                        if data == "[DONE]" {
                                            break;
                                        }
                                        if let Ok(v) =
                                            serde_json::from_str::<serde_json::Value>(data)
                                        {
                                            // Attempt to read token count
                                            if let Some(c) = v["usage"]["completion_tokens"]
                                                .as_u64()
                                            {
                                                generated_tokens = c;
                                            }
                                            // Count tokens from content
                                            if let Some(content) =
                                                v["choices"][0]["delta"]["content"]
                                                    .as_str()
                                            {
                                                if first_token_time.is_none() && !content.is_empty() {
                                                    first_token_time =
                                                        Some(start.elapsed().as_millis() as f64);
                                                }
                                                // Track prompt token count if server reports it
                                                if let Some(p) = v["usage"]["prompt_tokens"].as_u64() {
                                                    prompt_tokens_reported = p;
                                                }
                                                // Each content-bearing chunk ≈ 1 token
                                                if v["usage"]["completion_tokens"].is_null() {
                                                    generated_tokens =
                                                        generated_tokens.saturating_add(1);
                                                }
                                            }
                                        }
                                    }
                                }
                                buf.push_str(&s);
                            }

                            let end = start.elapsed();
                            let ttft_ms =
                                first_token_time.unwrap_or(end.as_millis() as f64);
                            let gen_dur_ms =
                                end.as_millis() as f64 - ttft_ms;
                            let gen_dur_s = gen_dur_ms.max(1.0) / 1000.0;

                            // Fallback: estimate from raw buffer if server didn't report counts
                            if generated_tokens == 0 {
                                generated_tokens = (buf.len() as u64 / 4).max(1);
                            }

                            let ttft_s = ttft_ms / 1000.0;
                            // Use server-reported prompt tokens; fall back to ~¼ char estimate
                            let effective_prompt_tokens = if prompt_tokens_reported > 0 {
                                prompt_tokens_reported as f64
                            } else {
                                (prompt.len() as f64 / 4.0).max(1.0)
                            };
                            let prompt_tps = if ttft_s > 0.0 {
                                effective_prompt_tokens / ttft_s
                            } else {
                                0.0
                            };
                            let gen_tps = if generated_tokens > 0 {
                                (generated_tokens as f64) / gen_dur_s
                            } else {
                                0.0
                            };

                            Some((prompt_tps, gen_tps, ttft_ms))
                        },
                    )
                    .await;

                    match result {
                        Ok(Some((prompt_tps, gen_tps, ttft_ms))) => {
                            let benchmark =
                                crate::llama::spawn_wizard::classify_benchmark_result(
                                    prompt_tps,
                                    gen_tps,
                                    ttft_ms,
                                    None,
                                    None,
                                    0,
                                );
                            Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(
                                Box::new(warp::reply::json(&serde_json::json!({
                                    "prompt_tokens_per_second": (benchmark.prompt_tokens_per_second * 100.0).round() / 100.0,
                                    "gen_tokens_per_second": (benchmark.gen_tokens_per_second * 100.0).round() / 100.0,
                                    "time_to_first_token_ms": (benchmark.time_to_first_token_ms * 100.0).round() / 100.0,
                                    "verdict": benchmark.verdict,
                                    "hints": benchmark.hints,
                                    "suggestions": benchmark.suggestions,
                                }))),
                            )
                        }
                        _ => {
                            Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(
                                Box::new(warp::reply::json(&serde_json::json!({
                                    "error": "Benchmark timed out or failed."
                                }))),
                            )
                        }
                    }
                }
            },
        )
}

// ── Phase 2: POST /api/model-defaults ────────────────────────────────────────

fn api_model_defaults(
    _state: AppState,
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (Box<dyn warp::reply::Reply>,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "model-defaults")
        .and(warp::post())
        .and(warp::header::optional::<String>("authorization"))
        .and(super::super::hf_json_body::<serde_json::Value>())
        .and_then(move |auth: Option<String>, body: serde_json::Value| {
            let cfg = app_config.clone();
            async move {
                if !check_api_token(&auth, &cfg) {
                    return Ok(unauthorized_api_token());
                }

                let name_or_repo = body["model_name_or_repo"]
                    .as_str()
                    .unwrap_or("")
                    .to_string();
                let size_bytes = body["size_bytes"].as_u64().unwrap_or(0);
                let tags: Vec<String> = body["tags"]
                    .as_array()
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|v| v.as_str().map(String::from))
                            .collect()
                    })
                    .unwrap_or_default();
                let gguf_arch = body["gguf_arch"].as_str().unwrap_or("").to_string();
                let arch_family = body["arch_family"].as_str().unwrap_or("").to_string();

                if name_or_repo.is_empty() {
                    return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                        warp::reply::json(&serde_json::json!({
                            "error": "Missing 'model_name_or_repo'."
                        })),
                    ));
                }

                let presets = crate::llama::sampling_catalog::SamplingCatalog::modes_as_presets(
                    &name_or_repo,
                    size_bytes,
                    &tags,
                    &gguf_arch,
                    &arch_family,
                );
                let defaults = presets
                    .first()
                    .map(|p| p.defaults.clone())
                    .unwrap_or_default();

                Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(warp::reply::json(
                    &serde_json::json!({
                        "defaults": &defaults,
                        "temperature": defaults.temperature,
                        "top_p": defaults.top_p,
                        "top_k": defaults.top_k,
                        "min_p": defaults.min_p,
                        "repeat_penalty": defaults.repeat_penalty,
                        "presence_penalty": defaults.presence_penalty,
                        "max_tokens": defaults.max_tokens,
                        "enable_thinking": defaults.enable_thinking,
                        "preserve_thinking": defaults.preserve_thinking,
                        "tool_call_format": defaults.tool_call_format,
                        "reasoning": defaults.reasoning,
                        "reasoning_budget": defaults.reasoning_budget,
                        "reasoning_budget_message": defaults.reasoning_budget_message,
                        "presets": presets,
                    }),
                )))
            }
        })
}

// ── Phase 2: POST /api/moe-tune ──────────────────────────────────────────────

fn api_moe_tune(
    _state: AppState,
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (Box<dyn warp::reply::Reply>,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "moe-tune")
        .and(warp::post())
        .and(warp::header::optional::<String>("authorization"))
        .and(super::super::hf_json_body::<serde_json::Value>())
        .and_then(move |auth: Option<String>, body: serde_json::Value| {
            let cfg = app_config.clone();
            async move {
                if !check_api_token(&auth, &cfg) {
                    return Ok(unauthorized_api_token());
                }

                let model_size_bytes = body["model_size_bytes"].as_u64().unwrap_or(0);
                let available_vram_bytes = body["available_vram_bytes"].as_u64().unwrap_or(0);
                // `--n-cpu-moe` is layer-based; prefer n_moe_layers/n_layers, fall back
                // to the legacy total_experts key for older callers.
                let n_moe_layers: u64 = body["n_moe_layers"]
                    .as_u64()
                    .or_else(|| body["n_layers"].as_u64())
                    .or_else(|| body["total_experts"].as_u64())
                    .unwrap_or(0);

                let suggestion = crate::llama::spawn_wizard::suggest_moe_tuning(
                    model_size_bytes,
                    available_vram_bytes,
                    n_moe_layers,
                );

                Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(warp::reply::json(
                    &serde_json::json!({
                        "recommended_n_cpu_moe": suggestion.recommended_n_cpu_moe,
                        "note": suggestion.note,
                    }),
                )))
            }
        })
}

// ── Config-time performance advisor ───────────────────────────────────────────
// Predictive hints (dense-vs-MoE, KV type, MTP) for the Spawn Wizard / Preset
// Editor, computed from the model architecture before any benchmark is run.
fn api_advise(
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (Box<dyn warp::reply::Reply>,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "advise")
        .and(warp::post())
        .and(warp::header::optional::<String>("authorization"))
        .and(super::super::hf_json_body::<serde_json::Value>())
        .and_then(move |auth: Option<String>, body: serde_json::Value| {
            let cfg = app_config.clone();
            async move {
                if !check_api_token(&auth, &cfg) {
                    return Ok(unauthorized_api_token());
                }

                let name = body["name"].as_str().unwrap_or("");
                let param_b = body["param_b"].as_f64().unwrap_or(0.0);
                let context_size = body["context_size"].as_u64().unwrap_or(8192);
                let ctk = body["ctk"].as_str().unwrap_or("q8_0");
                let ctv = body["ctv"].as_str().unwrap_or("q8_0");
                let is_unified = body["is_unified"].as_bool().unwrap_or(false);
                let spec_type = body["spec_type"].as_str();
                let has_mtp = body["has_mtp"].as_bool().unwrap_or(false);

                let arch =
                    crate::llama::vram_estimator::ModelArch::from_name_and_params(name, param_b);
                let suggestions = crate::llama::spawn_wizard::predict_perf_hints(
                    &arch,
                    context_size,
                    ctk,
                    ctv,
                    is_unified,
                    spec_type,
                    has_mtp,
                );

                Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(warp::reply::json(
                    &serde_json::json!({ "suggestions": suggestions }),
                )))
            }
        })
}

// ── n_cpu_moe auto-tuner (estimate + optional empirical verify) ────────────────
fn api_tune_ncpumoe(
    state: AppState,
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (Box<dyn warp::reply::Reply>,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "tune" / "ncpumoe")
        .and(warp::post())
        .and(warp::header::optional::<String>("authorization"))
        .and(super::super::hf_json_body::<serde_json::Value>())
        .and_then(move |auth: Option<String>, body: serde_json::Value| {
            let state = state.clone();
            let cfg = app_config.clone();
            async move {
                if !check_api_token(&auth, &cfg) {
                    return Ok(unauthorized_api_token());
                }

                let name = body["name"].as_str().unwrap_or("");
                let param_b = body["param_b"].as_f64().unwrap_or(0.0);
                let model_size_bytes = body["model_size_bytes"].as_u64().unwrap_or(0);
                let available_vram_bytes = body["available_vram_bytes"].as_u64().unwrap_or(0);
                let batch_size = body["batch_size"].as_u64().unwrap_or(2048) as u32;
                let ubatch_size = body["ubatch_size"].as_u64().unwrap_or(512) as u32;
                let verify = body["verify"].as_bool().unwrap_or(false);
                let is_unified_memory = body["is_unified_memory"].as_bool().unwrap_or(true);

                let arch =
                    crate::llama::vram_estimator::ModelArch::from_name_and_params(name, param_b);

                // Instant estimate — same fit-search the VRAM bar uses, so they agree.
                let recommended =
                    crate::llama::vram_estimator::find_min_cpu_moe_to_fit_weights(
                        model_size_bytes,
                        &arch,
                        available_vram_bytes,
                        ubatch_size,
                        is_unified_memory,
                        crate::llama::vram_estimator::Backend::LlamaCpp,
                    );

                if !verify {
                    return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                        warp::reply::json(&serde_json::json!({
                            "recommended_n_cpu_moe": recommended,
                            "verified": false,
                        })),
                    ));
                }

                // Empirical verify needs the GPU free — refuse while a server runs.
                let running = state.server_running.lock().map(|g| *g).unwrap_or(false);
                if running {
                    return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                        warp::reply::json(&serde_json::json!({
                            "recommended_n_cpu_moe": recommended,
                            "verified": false,
                            "error": "Stop the running server before verifying — llama-bench needs the GPU.",
                        })),
                    ));
                }

                let model_path = body["model_path"].as_str().unwrap_or("").to_string();
                let ngl = body["ngl"].as_i64().unwrap_or(99) as i32;
                let ctk = body["ctk"].as_str().unwrap_or("q8_0").to_string();
                let ctv = body["ctv"].as_str().unwrap_or("q8_0").to_string();
                let flash_attn = body["flash_attn"].as_bool().unwrap_or(true);

                // Probe a layer-space ladder (what llama.cpp actually consumes) plus
                // the instant estimate; measure real decode and pick the fastest.
                let nl = arch.n_layers.max(1) as i32;
                let mut candidates: Vec<i32> =
                    vec![0, nl / 4, nl / 2, (nl * 3) / 4, nl, recommended];
                candidates.retain(|&c| (0..=nl).contains(&c));
                candidates.sort_unstable();
                candidates.dedup();

                let bench_bin = crate::llama::bench_runner::llama_bench_path(
                    &cfg.llama_server_path,
                );
                let probes = crate::llama::bench_runner::probe_ncpumoe(
                    &bench_bin,
                    &cfg.llama_server_cwd,
                    &model_path,
                    ngl,
                    flash_attn,
                    &ctk,
                    &ctv,
                    batch_size,
                    ubatch_size,
                    &candidates,
                )
                .await;

                let best = probes
                    .iter()
                    .filter(|p| p.tg_tps > 0.0)
                    .max_by(|a, b| a.tg_tps.partial_cmp(&b.tg_tps).unwrap());

                Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(warp::reply::json(
                    &serde_json::json!({
                        "recommended_n_cpu_moe": best.map(|b| b.n_cpu_moe).unwrap_or(recommended),
                        "verified": true,
                        "estimate": recommended,
                        "probes": probes,
                    }),
                )))
            }
        })
}

// ── Offline depth sweep via llama-bench ───────────────────────────────────────
fn api_bench_sweep(
    state: AppState,
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (Box<dyn warp::reply::Reply>,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "bench" / "sweep")
        .and(warp::post())
        .and(warp::header::optional::<String>("authorization"))
        .and(super::super::hf_json_body::<serde_json::Value>())
        .and_then(move |auth: Option<String>, body: serde_json::Value| {
            let state = state.clone();
            let cfg = app_config.clone();
            async move {
                if !check_api_token(&auth, &cfg) {
                    return Ok(unauthorized_api_token());
                }

                // Depth sweep requires llama-bench; not available for Rapid-MLX.
                let is_rapid_mlx = matches!(
                    state.local_launch_request.lock().unwrap().as_ref(),
                    Some(crate::inference::launch::LocalLaunchRequest::RapidMlx(_))
                );
                if is_rapid_mlx {
                    return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                        warp::reply::with_status(
                            warp::reply::json(&serde_json::json!({
                                "ok": false,
                                "error": "Depth sweep is not supported for Rapid-MLX — llama-bench (required for depth sweep) is a llama.cpp-only tool. Rapid-MLX does not expose KV-cache depth as a tunable parameter.",
                            })),
                            StatusCode::BAD_REQUEST,
                        ),
                    ));
                }

                // llama-bench needs the GPU to itself.
                let running = state.server_running.lock().map(|g| *g).unwrap_or(false);
                if running {
                    return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                        warp::reply::json(&serde_json::json!({
                            "error": "Stop the running server before a depth sweep — llama-bench needs the GPU.",
                        })),
                    ));
                }

                let model_path = body["model_path"].as_str().unwrap_or("").to_string();
                if model_path.is_empty() {
                    return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                        warp::reply::json(&serde_json::json!({ "error": "model_path required" })),
                    ));
                }
                let ngl = body["ngl"].as_i64().unwrap_or(99) as i32;
                let ctk = body["ctk"].as_str().unwrap_or("q8_0").to_string();
                let ctv = body["ctv"].as_str().unwrap_or("q8_0").to_string();
                let flash_attn = body["flash_attn"].as_bool().unwrap_or(true);
                let batch_size = body["batch_size"].as_u64().unwrap_or(2048) as u32;
                let ubatch_size = body["ubatch_size"].as_u64().unwrap_or(512) as u32;
                let n_cpu_moe = body["n_cpu_moe"].as_i64().map(|n| n as i32);
                let depths: Vec<u64> = body["depths"]
                    .as_array()
                    .map(|a| a.iter().filter_map(|v| v.as_u64()).collect())
                    .filter(|v: &Vec<u64>| !v.is_empty())
                    .unwrap_or_else(|| vec![0, 16384, 32768]);

                let bench_bin =
                    crate::llama::bench_runner::llama_bench_path(&cfg.llama_server_path);
                match crate::llama::bench_runner::run_sweep(
                    &bench_bin,
                    &cfg.llama_server_cwd,
                    &model_path,
                    ngl,
                    flash_attn,
                    &ctk,
                    &ctv,
                    batch_size,
                    ubatch_size,
                    &depths,
                    n_cpu_moe,
                )
                .await
                {
                    Ok(points) => Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                        warp::reply::json(&serde_json::json!({ "points": points })),
                    )),
                    Err(e) => Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                        warp::reply::json(&serde_json::json!({ "error": e })),
                    )),
                }
            }
        })
}

// ── Offline batch/ubatch sweep ───────────────────────────────────────────────
fn api_bench_batch_sweep(
    state: AppState,
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (Box<dyn warp::reply::Reply>,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "bench" / "batch-sweep")
        .and(warp::post())
        .and(warp::header::optional::<String>("authorization"))
        .and(super::super::hf_json_body::<serde_json::Value>())
        .and_then(move |auth: Option<String>, body: serde_json::Value| {
            let state = state.clone();
            let cfg = app_config.clone();
            async move {
                if !check_api_token(&auth, &cfg) {
                    return Ok(unauthorized_api_token());
                }

                // Batch sweep is llama.cpp-only — Rapid-MLX does not expose batch/ubatch
                // as standalone CLI knobs like llama-bench; they are baked into the
                // server's internal scheduler and not safely tunable via rapid-mlx bench.
                let is_rapid_mlx = matches!(
                    state.local_launch_request.lock().unwrap().as_ref(),
                    Some(crate::inference::launch::LocalLaunchRequest::RapidMlx(_))
                );
                if is_rapid_mlx {
                    return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                        warp::reply::with_status(
                            warp::reply::json(&serde_json::json!({
                                "ok": false,
                                "error": "Batch sweep is not supported for Rapid-MLX — it is a llama.cpp-only feature using llama-bench. Rapid-MLX does not expose batch/ubatch as standalone tunable knobs.",
                            })),
                            StatusCode::BAD_REQUEST,
                        ),
                    ));
                }

                let running = state.server_running.lock().map(|g| *g).unwrap_or(false);
                if running {
                    return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                        warp::reply::json(&serde_json::json!({
                            "error": "Stop the running server before a batch sweep — llama-bench needs the GPU.",
                        })),
                    ));
                }

                let model_path = body["model_path"].as_str().unwrap_or("").to_string();
                if model_path.is_empty() {
                    return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                        warp::reply::json(&serde_json::json!({ "error": "model_path required" })),
                    ));
                }

                let ngl = body["ngl"].as_i64().unwrap_or(99) as i32;
                let ctk = body["ctk"].as_str().unwrap_or("q8_0").to_string();
                let ctv = body["ctv"].as_str().unwrap_or("q8_0").to_string();
                let flash_attn = body["flash_attn"].as_bool().unwrap_or(true);
                let n_cpu_moe = body["n_cpu_moe"].as_i64().map(|n| n as i32);
                let prompt_tokens = body["prompt_tokens"].as_u64().unwrap_or(2048) as u32;

                // Standard ladder of (batch, ubatch) pairs to probe.
                // ubatch must be ≤ batch, so we test ubatch = batch (max parallelism)
                // and ubatch = 512 (conservative baseline) at each batch size.
                let candidates: Vec<(u32, u32)> = vec![
                    (512, 512),
                    (1024, 512), (1024, 1024),
                    (2048, 512), (2048, 1024), (2048, 2048),
                    (4096, 512), (4096, 1024), (4096, 2048), (4096, 4096),
                ];

                let bench_bin =
                    crate::llama::bench_runner::llama_bench_path(&cfg.llama_server_path);
                let probes = crate::llama::bench_runner::probe_batch(
                    &bench_bin,
                    &cfg.llama_server_cwd,
                    &model_path,
                    ngl,
                    flash_attn,
                    &ctk,
                    &ctv,
                    &candidates,
                    prompt_tokens,
                    n_cpu_moe,
                )
                .await;

                let best = probes
                    .iter()
                    .filter(|p| p.pp_tps > 0.0)
                    .max_by(|a, b| a.pp_tps.partial_cmp(&b.pp_tps).unwrap());

                Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(warp::reply::json(
                    &serde_json::json!({
                        "probes": probes,
                        "recommended_batch_size": best.map(|b| b.batch_size),
                        "recommended_ubatch_size": best.map(|b| b.ubatch_size),
                    }),
                )))
            }
        })
}

// ── Online MTP n-max sweep ────────────────────────────────────────────────────

/// Send one streaming chat completion to `url` using `prompt_type` and return
/// `(gen_tps, ttft_ms)`. Returns `None` if the server is unreachable or fails.
async fn online_probe_gen_tps(url: &str, prompt_type: &str) -> Option<(f64, f64)> {
    let prompt = match prompt_type {
        "code" => concat!(
            "Write a Rust function that reads a JSON file from disk, parses it into a ",
            "serde_json::Value, iterates over every key in the top-level object, and ",
            "prints each key-value pair. Include error handling with anyhow."
        ),
        _ => "Explain in one sentence what llama.cpp is used for.",
    };

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(90))
        .build()
        .ok()?;

    let payload = serde_json::json!({
        "messages": [{"role": "user", "content": prompt}],
        "max_tokens": 512,
        "temperature": 0.2,
        "stream": true,
        "chat_template_kwargs": {"enable_thinking": false},
    });

    let start = std::time::Instant::now();
    let resp = client
        .post(url)
        .header("Content-Type", "application/json")
        .json(&payload)
        .send()
        .await
        .ok()?;

    if !resp.status().is_success() {
        return None;
    }

    let mut stream = resp.bytes_stream();
    use futures_util::StreamExt;

    let mut first_token_time: Option<f64> = None;
    let mut generated_tokens = 0u64;

    while let Some(Ok(chunk)) = stream.next().await {
        let s = std::str::from_utf8(&chunk).unwrap_or("").to_string();
        for line in s.lines() {
            if let Some(data) = line.trim().strip_prefix("data: ") {
                if data == "[DONE]" {
                    break;
                }
                if let Ok(v) = serde_json::from_str::<serde_json::Value>(data) {
                    if let Some(c) = v["usage"]["completion_tokens"].as_u64() {
                        generated_tokens = c;
                    }
                    if let Some(content) = v["choices"][0]["delta"]["content"].as_str() {
                        if first_token_time.is_none() && !content.is_empty() {
                            first_token_time = Some(start.elapsed().as_millis() as f64);
                        }
                        if v["usage"]["completion_tokens"].is_null() {
                            generated_tokens = generated_tokens.saturating_add(1);
                        }
                    }
                }
            }
        }
    }

    let end_ms = start.elapsed().as_millis() as f64;
    let ttft_ms = first_token_time.unwrap_or(end_ms);
    let gen_dur_s = (end_ms - ttft_ms).max(1.0) / 1000.0;
    if generated_tokens == 0 {
        generated_tokens = 1;
    }
    let gen_tps = generated_tokens as f64 / gen_dur_s;

    Some((gen_tps, ttft_ms))
}

/// Poll `http://127.0.0.1:{port}/health` until it returns 200 or timeout.
async fn wait_for_server_health(port: u16, timeout_secs: u64) -> bool {
    let url = format!("http://127.0.0.1:{port}/health");
    let client = match reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
    {
        Ok(c) => c,
        Err(_) => return false,
    };
    let deadline = std::time::Instant::now() + Duration::from_secs(timeout_secs);
    loop {
        if std::time::Instant::now() >= deadline {
            return false;
        }
        if let Ok(resp) = client.get(&url).send().await
            && resp.status().is_success()
        {
            return true;
        }
        tokio::time::sleep(Duration::from_secs(2)).await;
    }
}

fn api_bench_mtp_sweep(
    state: AppState,
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (Box<dyn warp::reply::Reply>,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "bench" / "mtp-sweep")
        .and(warp::post())
        .and(warp::header::optional::<String>("authorization"))
        .and(super::super::hf_json_body::<serde_json::Value>())
        .and_then(move |auth: Option<String>, body: serde_json::Value| {
            let state = state.clone();
            let cfg = app_config.clone();
            async move {
                if !check_api_token(&auth, &cfg) {
                    return Ok(unauthorized_api_token());
                }

                // Only works with a locally-spawned server
                let local_running = state
                    .local_server_running
                    .lock()
                    .map(|g| *g)
                    .unwrap_or(false);
                if !local_running {
                    return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                        warp::reply::json(&serde_json::json!({
                            "ok": false,
                            "error": "MTP sweep requires a locally-spawned server."
                        })),
                    ));
                }

                let is_llama_cpp = matches!(
                    state.local_launch_request.lock().unwrap().as_ref(),
                    Some(crate::inference::launch::LocalLaunchRequest::LlamaCpp(_))
                );
                if !is_llama_cpp {
                    return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                        warp::reply::with_status(
                            warp::reply::json(&serde_json::json!({
                                "ok": false,
                                "error": "MTP sweep is only supported by the llama.cpp backend."
                            })),
                            warp::http::StatusCode::BAD_REQUEST,
                        ),
                    ));
                }

                // Read the current server config before we touch anything
                let base_config = {
                    let guard = state.server_config.lock().unwrap();
                    guard.clone()
                };
                let Some(base_config) = base_config else {
                    return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                        warp::reply::json(&serde_json::json!({
                            "ok": false,
                            "error": "No saved server configuration found."
                        })),
                    ));
                };

                // Validate that spec decoding is actually configured
                let has_spec = base_config.spec.spec_type.is_some()
                    || base_config.spec.spec_default
                    || !base_config.spec.draft_model.is_empty();
                if !has_spec {
                    return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                        warp::reply::json(&serde_json::json!({
                            "ok": false,
                            "error": "MTP sweep only applies to servers with speculative decoding enabled."
                        })),
                    ));
                }

                // Determine port from current session
                let port = match state.get_active_session() {
                    Some(s) => match &s.mode {
                        crate::state::SessionMode::Spawn { port, .. } => *port,
                        _ => {
                            return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                                warp::reply::json(&serde_json::json!({
                                    "ok": false,
                                    "error": "MTP sweep only works with locally-spawned servers."
                                })),
                            ));
                        }
                    },
                    None => {
                        return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                            warp::reply::json(&serde_json::json!({
                                "ok": false,
                                "error": "No active session."
                            })),
                        ));
                    }
                };

                // Parse request
                let n_max_values: Vec<u32> = body["n_max_values"]
                    .as_array()
                    .map(|a| {
                        a.iter()
                            .filter_map(|v| v.as_u64().map(|n| n as u32))
                            .filter(|&n| (1..=16).contains(&n))
                            .collect()
                    })
                    .unwrap_or_else(|| vec![1, 2, 3, 4]);

                if n_max_values.is_empty() || n_max_values.len() > 8 {
                    return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                        warp::reply::json(&serde_json::json!({
                            "ok": false,
                            "error": "n_max_values must have 1–8 entries in range 1–16."
                        })),
                    ));
                }

                let prompt_type = body["prompt_type"]
                    .as_str()
                    .unwrap_or("code")
                    .to_string();

                let chat_url = format!("http://127.0.0.1:{port}/v1/chat/completions");

                // Run probes
                let mut probes: Vec<serde_json::Value> = Vec::new();

                for &n_max in &n_max_values {
                    state.push_log(format!(
                        "[mtp-sweep] probing spec-draft-n-max={n_max}"
                    ));

                    // Clone config and set n_max for this probe
                    let mut probe_config = base_config.clone();
                    probe_config.spec.spec_draft_n_max = Some(n_max);

                    // Stop the current server
                    if let Err(e) =
                        crate::llama::server::stop_server(&state).await
                    {
                        return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                            warp::reply::json(&serde_json::json!({
                                "ok": false,
                                "error": format!("Failed to stop server for n_max={n_max}: {e}")
                            })),
                        ));
                    }

                    // Wait for the port to actually become free before the next spawn.
                    // 2 s was too short after killing a large model on macOS; poll instead.
                    let port_deadline = std::time::Instant::now() + Duration::from_secs(15);
                    loop {
                        let is_free = tokio::net::TcpListener::bind(
                            std::net::SocketAddr::from(([127, 0, 0, 1], port)),
                        )
                        .await
                        .is_ok();
                        if is_free || std::time::Instant::now() >= port_deadline {
                            break;
                        }
                        tokio::time::sleep(Duration::from_millis(500)).await;
                    }
                    // Extra settle time for OS memory reclaim after a large model unload.
                    tokio::time::sleep(Duration::from_secs(2)).await;

                    // Start with modified config
                    if let Err(e) =
                        crate::llama::server::start_server(
                            Arc::new(state.clone()),
                            probe_config,
                            &cfg,
                        )
                        .await
                    {
                        state.push_log(format!(
                            "[mtp-sweep] start_server failed for n_max={n_max}: {e}"
                        ));
                        probes.push(serde_json::json!({
                            "n_max": n_max,
                            "gen_tps": 0.0,
                            "ttft_ms": 0.0,
                            "error": format!("Server failed to start: {e}")
                        }));
                        continue;
                    }

                    // Wait up to 120s for the model to load
                    if !wait_for_server_health(port, 120).await {
                        state.push_log(format!(
                            "[mtp-sweep] server did not become healthy for n_max={n_max}"
                        ));
                        probes.push(serde_json::json!({
                            "n_max": n_max,
                            "gen_tps": 0.0,
                            "ttft_ms": 0.0,
                            "error": "Server did not become healthy within 120 s"
                        }));
                        continue;
                    }

                    // Additional warmup: let KV cache and GPU buffers settle
                    tokio::time::sleep(Duration::from_secs(3)).await;

                    // Benchmark
                    match online_probe_gen_tps(&chat_url, &prompt_type).await {
                        Some((gen_tps, ttft_ms)) => {
                            state.push_log(format!(
                                "[mtp-sweep] n_max={n_max} → {gen_tps:.1} t/s"
                            ));
                            probes.push(serde_json::json!({
                                "n_max": n_max,
                                "gen_tps": (gen_tps * 10.0).round() / 10.0,
                                "ttft_ms": (ttft_ms * 10.0).round() / 10.0,
                            }));
                        }
                        None => {
                            state.push_log(format!(
                                "[mtp-sweep] n_max={n_max} → probe failed"
                            ));
                            probes.push(serde_json::json!({
                                "n_max": n_max,
                                "gen_tps": 0.0,
                                "ttft_ms": 0.0,
                                "error": "Benchmark probe timed out"
                            }));
                        }
                    }
                }

                // Pick the n_max with the highest gen_tps (ignore failed probes)
                let recommended_n_max = probes
                    .iter()
                    .filter(|p| p["error"].is_null())
                    .max_by(|a, b| {
                        a["gen_tps"]
                            .as_f64()
                            .unwrap_or(0.0)
                            .partial_cmp(&b["gen_tps"].as_f64().unwrap_or(0.0))
                            .unwrap_or(std::cmp::Ordering::Equal)
                    })
                    .and_then(|p| p["n_max"].as_u64())
                    .unwrap_or(2) as u32;

                let last_probed_n_max = n_max_values.last().copied().unwrap_or(0);

                // If the recommended n_max differs from the last probed value,
                // restart the server with the recommended config so the user
                // is left with the optimal setting.
                if recommended_n_max != last_probed_n_max {
                    state.push_log(format!(
                        "[mtp-sweep] restarting with recommended n_max={recommended_n_max}"
                    ));
                    let mut final_config = base_config.clone();
                    final_config.spec.spec_draft_n_max = Some(recommended_n_max);

                    let _ = crate::llama::server::stop_server(&state).await;
                    tokio::time::sleep(Duration::from_secs(2)).await;
                    let _ = crate::llama::server::start_server(
                        Arc::new(state.clone()),
                        final_config,
                        &cfg,
                    )
                    .await;
                }

                Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                    warp::reply::json(&serde_json::json!({
                        "ok": true,
                        "probes": probes,
                        "recommended_n_max": recommended_n_max,
                        "applied_n_max": recommended_n_max,
                    })),
                ))
            }
        })
}

pub(crate) fn routes(ctx: ApiCtx) -> ApiRoute {
    let state = ctx.state.clone();
    let config = ctx.config.clone();

    let mut r = api_benchmark(state.clone(), config.clone())
        .or(api_model_defaults(state.clone(), config.clone()))
        .unify()
        .boxed();
    r = r
        .or(api_moe_tune(state.clone(), config.clone()))
        .unify()
        .boxed();
    r = r.or(api_advise(config.clone())).unify().boxed();
    r = r
        .or(api_tune_ncpumoe(state.clone(), config.clone()))
        .unify()
        .boxed();
    r = r
        .or(api_bench_sweep(state.clone(), config.clone()))
        .unify()
        .boxed();
    r = r
        .or(api_bench_batch_sweep(state.clone(), config.clone()))
        .unify()
        .boxed();
    r = r
        .or(api_bench_mtp_sweep(state.clone(), config.clone()))
        .unify()
        .boxed();
    r
}

#[cfg(test)]
mod rapid_mlx_bench_tests {
    use super::*;

    /// Real `rapid-mlx bench qwen3-0.6b-4bit --tier speed` output
    /// (rapid-mlx 0.10.12, captured 2026-07-18). Note: the speed tier reports
    /// a single `tps=` metric (generation throughput) — no separate prompt/prefill
    /// throughput or TTFT line is emitted for this tier.
    const WELL_FORMED_BENCH_OUTPUT: &str = "\
Alias: qwen3-0.6b-4bit → mlx-community/Qwen3-0.6B-4bit
Rapid-MLX bench — tier=speed model=mlx-community/Qwen3-0.6B-4bit
============================================================
  [server] booted mlx-community/Qwen3-0.6B-4bit on port 8500

  [PASS] tier=speed duration=0.6s
        PASS model=mlx-community/Qwen3-0.6B-4bit sampling=greedy tokens=117 chars=580 tps=198.3

============================================================
  OK: 1/1 tiers passed (speed=pass)
  total: 2.6s
";

    #[test]
    fn build_args_places_model_positional_immediately_after_bench() {
        let args = build_rapid_mlx_bench_args("qwen3-0.6b-4bit", "http://127.0.0.1:8000", "speed");
        assert_eq!(
            args,
            vec![
                "bench",
                "qwen3-0.6b-4bit",
                "--base-url",
                "http://127.0.0.1:8000",
                "--tier",
                "speed",
            ]
        );
        assert_eq!(args[0], "bench");
        assert_eq!(args[1], "qwen3-0.6b-4bit");
    }

    #[test]
    fn build_args_handles_empty_model_without_panicking() {
        let args = build_rapid_mlx_bench_args("", "http://127.0.0.1:8000", "speed");
        assert_eq!(args[1], "");
    }

    #[test]
    fn parses_generation_throughput_via_tps_from_real_speed_tier_output() {
        // Real rapid-mlx speed tier emits a single `tps=` value (generation throughput)
        // on the PASS line; no separate prompt/prefill throughput or TTFT.
        let prompt_tps = parse_rapid_mlx_throughput(WELL_FORMED_BENCH_OUTPUT, "prompt")
            .or_else(|| parse_rapid_mlx_throughput(WELL_FORMED_BENCH_OUTPUT, "prefill"));
        let gen_tps = parse_rapid_mlx_throughput(WELL_FORMED_BENCH_OUTPUT, "generation")
            .or_else(|| parse_rapid_mlx_throughput(WELL_FORMED_BENCH_OUTPUT, "gen"));

        assert_eq!(prompt_tps, None);
        assert_eq!(gen_tps, Some(198.3));
    }

    #[test]
    fn speed_tier_output_has_no_ttft() {
        // Real rapid-mlx speed tier does not emit a TTFT line.
        let ttft = parse_rapid_mlx_ttft(WELL_FORMED_BENCH_OUTPUT);
        assert_eq!(ttft, None);
    }

    #[test]
    fn parses_ttft_and_converts_seconds_to_milliseconds() {
        let text = "time to first token: 0.42 s\n";
        assert_eq!(parse_rapid_mlx_ttft(text), Some(420.0));
    }

    #[test]
    fn throughput_parser_returns_none_on_malformed_output_without_panicking() {
        let malformed = "unexpected format\nno numbers here at all\n";
        assert_eq!(parse_rapid_mlx_throughput(malformed, "prompt"), None);
        assert_eq!(parse_rapid_mlx_throughput(malformed, "generation"), None);
    }

    #[test]
    fn throughput_parser_returns_none_on_empty_output_without_panicking() {
        assert_eq!(parse_rapid_mlx_throughput("", "prompt"), None);
        assert_eq!(parse_rapid_mlx_ttft(""), None);
        assert_eq!(extract_first_float(""), None);
    }

    #[test]
    fn ttft_parser_returns_none_when_no_ttft_line_present() {
        let text = "Prefill throughput: 100.0 tokens/s\nGeneration throughput: 50.0 t/s\n";
        assert_eq!(parse_rapid_mlx_ttft(text), None);
    }

    #[test]
    fn extract_first_float_handles_leading_labels_and_units() {
        assert_eq!(extract_first_float("TTFT: 123 ms"), Some(123.0));
        assert_eq!(extract_first_float("prefill: 42.3 t/s"), Some(42.3));
        assert_eq!(extract_first_float("no digits here"), None);
        assert_eq!(extract_first_float("trailing dot 5."), Some(5.0));
    }

    #[test]
    fn parses_ttft_from_real_smoke_tier_ttft_key() {
        // Real rapid-mlx smoke tier: `ttft=140ms` on the PASS line.
        let text = "\
Alias: qwen3-0.6b-4bit → mlx-community/Qwen3-0.6B-4bit
Rapid-MLX bench — tier=smoke model=mlx-community/Qwen3-0.6B-4bit
============================================================
  [server] booted mlx-community/Qwen3-0.6B-4bit on port 8500

  [PASS] tier=smoke duration=0.2s
        PASS model=mlx-community/Qwen3-0.6B-4bit ttft=140ms response='Hello! 2 + 2 equals 4.'

============================================================
  OK: 1/1 tiers passed (smoke=pass)
  total: 3.2s
";
        assert_eq!(parse_rapid_mlx_ttft(text), Some(140.0));
    }

    #[test]
    fn well_formed_fixture_has_no_llama_bench_labels() {
        // Verify the rapid-mlx fixture contains no llama-bench labels.
        assert!(
            !WELL_FORMED_BENCH_OUTPUT.contains("llama-bench"),
            "rapid-mlx fixture must not contain 'llama-bench'"
        );
        assert!(
            !WELL_FORMED_BENCH_OUTPUT.contains("llama_bench"),
            "rapid-mlx fixture must not contain 'llama_bench'"
        );
        assert!(
            !WELL_FORMED_BENCH_OUTPUT.contains("llama.cpp"),
            "rapid-mlx fixture must not contain 'llama.cpp'"
        );
    }

    #[tokio::test]
    async fn run_rapid_mlx_bench_rejects_empty_model_before_spawning() {
        let result = run_rapid_mlx_bench(
            "http://127.0.0.1:8000",
            "",
            Some(PathBuf::from("/nonexistent/rapid-mlx")),
            None,
        )
        .await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("model"));
    }

    /// Contract test against the real CLI, mirroring
    /// `info_query::parses_real_rapid_mlx_info_output_contract`. Not run in CI —
    /// requires a real rapid-mlx install and (for a non-empty parse) a live
    /// server at --base-url. A follow-up live pass exercises the full,
    /// server-backed contract; this stub only pins the argv shape against the
    /// installed CLI's own `--help`.
    #[tokio::test]
    #[ignore = "requires rapid-mlx CLI installed; run manually to verify against real output"]
    async fn rapid_mlx_bench_help_still_documents_a_positional_model_arg() {
        let output = std::process::Command::new("rapid-mlx")
            .args(["bench", "--help"])
            .output()
            .expect("rapid-mlx CLI must be installed for this contract test");
        let text = String::from_utf8_lossy(&output.stdout);
        assert!(
            text.contains("model"),
            "rapid-mlx bench --help no longer documents a `model` argument; \
             update build_rapid_mlx_bench_args if the CLI contract changed"
        );
    }
}
