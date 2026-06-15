use std::fs;
use std::path::Path;

use crate::presets::ModelPreset;

/// Parsed result from an imported launch script.
#[derive(Debug, Clone)]
pub struct ImportResult {
    pub preset: ModelPreset,
    pub warnings: Vec<String>,
}

/// Read a launch file from disk and parse it.
/// Detects OS from platform and file extension.
pub fn import_launch_file(file: &str) -> Result<ImportResult, String> {
    let content =
        fs::read_to_string(file).map_err(|e| format!("Failed to read file '{}': {}", file, e))?;

    let path = Path::new(file);
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");

    let os = if ext == "bat" || ext == "cmd" || cfg!(target_os = "windows") {
        "windows"
    } else {
        "linux"
    };

    parse_launch_script(&content, os)
}

/// Import a launch script (batch/sh) into a ModelPreset.
pub fn parse_launch_script(content: &str, os: &str) -> Result<ImportResult, String> {
    let (binary_path, args, warnings) = match os {
        "windows" => parse_windows_script(content),
        "macos" | "linux" => parse_unix_script(content),
        _ => return Err(format!("Unsupported OS: {os}")),
    };

    if binary_path.is_empty() {
        return Err("Could not detect llama-server binary path in script".into());
    }

    let preset = build_preset_from_args(&args);
    Ok(ImportResult { preset, warnings })
}

fn parse_windows_script(content: &str) -> (String, Vec<String>, Vec<String>) {
    let mut warnings = Vec::new();
    let normalized = content
        .replace("\r\n", "\n")
        .replace("\r", "\n")
        .replace(" ^\n", " ")
        .replace(" ^\r\n", " ")
        .replace(" ^\r", " ")
        .replace(" \\\n", " ")
        .replace(" \\\r\n", " ")
        .replace(" \\\r", " ");

    let lines: Vec<&str> = normalized.lines().collect();
    let command_line: &str = lines
        .iter()
        .find(|l| {
            let s = l.trim();
            !s.is_empty() && !s.starts_with("::") && !s.starts_with("@echo")
        })
        .copied()
        .unwrap_or("");

    let tokens = tokenize_win(command_line);
    let mut binary_path = String::new();
    let mut args = Vec::new();

    if let Some(first) = tokens.first() {
        binary_path = first.clone();
        for t in tokens.iter().skip(1) {
            args.push(t.clone());
        }
    }

    if binary_path.contains("llama-server") || binary_path.ends_with(".exe") {
        // OK.
    } else {
        warnings.push("Binary path may not be llama-server; verify manually.".to_string());
    }

    (binary_path, args, warnings)
}

fn tokenize_win(line: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut in_quote = false;

    for ch in line.chars() {
        match ch {
            '"' if !in_quote => in_quote = true,
            '"' if in_quote => {
                in_quote = false;
                current.push('"');
            }
            ' ' | '\t' if !in_quote => {
                if !current.is_empty() {
                    tokens.push(current.clone());
                    current.clear();
                }
            }
            _ => current.push(ch),
        }
    }

    if !current.is_empty() {
        tokens.push(current);
    }
    tokens
}

fn parse_unix_script(content: &str) -> (String, Vec<String>, Vec<String>) {
    let mut warnings = Vec::new();
    let normalized = content.replace("\\\n", " ").replace("\\\r\n", " ");

    let lines: Vec<&str> = normalized.lines().collect();

    let command_line: &str = lines
        .iter()
        .find(|l| {
            let s = l.trim();
            !s.is_empty() && !s.starts_with('#')
        })
        .copied()
        .unwrap_or("");

    let tokens: Vec<String> = shlex_like_split(command_line);
    let mut binary_path = String::new();
    let mut args = Vec::new();

    if let Some(first) = tokens.first() {
        binary_path = first.clone();
        for t in tokens.iter().skip(1) {
            args.push(t.clone());
        }
    }

    if binary_path.contains("llama-server") || binary_path.ends_with(".sh") {
        // OK.
    } else {
        warnings.push("Binary path may not be llama-server; verify manually.".to_string());
    }

    for line in &lines {
        let line = line.trim();
        if line.starts_with('#') || line.is_empty() {
            continue;
        }
        if let Some(env) = parse_gpu_env_hint(line) {
            warnings.push(env);
        }
    }

    (binary_path, args, warnings)
}

fn parse_gpu_env_hint(line: &str) -> Option<String> {
    let line = line.trim();
    if line.starts_with('#') {
        return None;
    }
    if let Some((var, val)) = extract_env_var(line) {
        match var {
            "CUDA_VISIBLE_DEVICES" => {
                return Some(format!(
                    "CUDA_VISIBLE_DEVICES={val} — only these GPU(s) will be used"
                ));
            }
            "HSA_OVERRIDE_GFX_VERSION" => {
                return Some(format!(
                    "HSA_OVERRIDE_GFX_VERSION={val} — ROCm GPU override in effect"
                ));
            }
            "ROCR_VISIBLE_DEVICES" => {
                return Some(format!(
                    "ROCR_VISIBLE_DEVICES={val} — ROCm device selection"
                ));
            }
            "GGML_CUDA_FORCE_MMQ"
            | "GGML_CUDA_FA_DISABLE"
            | "GGML_HIP_BLAS_HANDLE"
            | "ZES_ENABLE_SYSMAN"
            | "SYCL_DEVICE_FILTER" => {
                return Some(format!("GPU env: {var}={val}"));
            }
            _ => {}
        }
    }
    None
}

fn extract_env_var(line: &str) -> Option<(&str, &str)> {
    let line = line.trim();
    if line.starts_with('#') || line.starts_with("//") {
        return None;
    }
    let rest = if let Some(s) = line.strip_prefix("export ") {
        s
    } else if line.starts_with("set ") {
        return None;
    } else {
        line
    };
    let rest = rest.trim();
    let (var, val) = rest.split_once('=')?;
    let var = var.trim();
    let val = val.trim().trim_matches('"').trim_matches('\'');
    if var.is_empty() || val.is_empty() || !var.chars().all(|c| c.is_alphanumeric() || c == '_') {
        return None;
    }
    Some((var, val))
}

fn shlex_like_split(line: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut in_quote = None::<char>;

    for ch in line.chars() {
        match (ch, in_quote) {
            ('"' | '\'', Some(q)) if ch == q => {
                in_quote = None;
                current.push(ch);
            }
            ('"' | '\'', None) => {
                in_quote = Some(ch);
                current.push(ch);
            }
            (' ' | '\t', None) => {
                if !current.is_empty() {
                    tokens.push(current.clone());
                    current.clear();
                }
            }
            _ => current.push(ch),
        }
    }

    if !current.is_empty() {
        tokens.push(current);
    }
    tokens
}

fn build_preset_from_args(args: &[String]) -> ModelPreset {
    let mut model_path = String::new();
    let mut context_size: u64 = 4096;
    let mut gpu_layers: Option<i32> = None;
    let mut no_mmap = false;
    let mut ngram_spec = false;
    let mut temperature: Option<f64> = None;
    let mut top_p: Option<f64> = None;
    let mut top_k: Option<i32> = None;
    let mut min_p: Option<f64> = None;
    let mut repeat_penalty: Option<f64> = None;
    let mut n_cpu_moe: Option<i32> = None;
    let mut spec_type: Option<String> = None;
    let mut spec_default = false;
    let draft_model = String::new();
    let mut spec_draft_n_max: Option<u32> = None;
    let mut spec_draft_n_min: Option<u32> = None;
    let mut spec_draft_p_split: Option<f32> = None;
    let mut spec_draft_p_min: Option<f32> = None;
    let mut spec_draft_ngl: Option<i32> = None;
    let mut spec_draft_device: Option<String> = None;
    let mut spec_draft_cpu_moe = false;
    let mut spec_draft_n_cpu_moe: Option<i32> = None;
    let mut spec_draft_type_k: Option<String> = None;
    let mut spec_draft_type_v: Option<String> = None;
    let mut spec_ngram_mod_n_min: Option<u32> = None;
    let mut spec_ngram_mod_n_max: Option<u32> = None;
    let mut spec_ngram_mod_n_match: Option<u32> = None;
    let mut spec_ngram_simple_size_n: Option<u32> = None;
    let mut spec_ngram_simple_size_m: Option<u32> = None;
    let mut spec_ngram_simple_min_hits: Option<u32> = None;
    let mut spec_ngram_map_k_size_n: Option<u32> = None;
    let mut spec_ngram_map_k_size_m: Option<u32> = None;
    let mut spec_ngram_map_k_min_hits: Option<u32> = None;
    let mut spec_ngram_map_k4v_size_n: Option<u32> = None;
    let mut spec_ngram_map_k4v_size_m: Option<u32> = None;
    let mut spec_ngram_map_k4v_min_hits: Option<u32> = None;
    let mut kv_unified: Option<bool> = None;
    let mut cache_idle_slots: Option<bool> = None;
    let mut fit_enabled: Option<bool> = None;
    let mut fit_ctx: Option<u32> = None;
    let mut fit_target: Option<String> = None;
    let mut fit_print: Option<bool> = None;
    let mut prio: Option<i32> = None;
    let mut prio_batch: Option<i32> = None;
    let mut extra_args = String::new();

    let mut i = 0;
    while i < args.len() {
        let arg = &args[i];
        match arg.as_str() {
            "-m" | "--model" => {
                if i + 1 < args.len() {
                    model_path = args[i + 1].clone();
                    i += 2;
                    continue;
                }
            }
            "-c" | "--ctx-size" | "--context-size" => {
                if i + 1 < args.len() {
                    if let Ok(v) = args[i + 1].parse::<u64>() {
                        context_size = v;
                    }
                    i += 2;
                    continue;
                }
            }
            "-ngl" | "--gpu-layers" => {
                if i + 1 < args.len() {
                    if let Ok(v) = args[i + 1].parse::<i32>() {
                        gpu_layers = Some(v);
                    }
                    i += 2;
                    continue;
                }
            }
            "--no-mmap" => {
                no_mmap = true;
                i += 1;
                continue;
            }
            "--spec-type" => {
                if i + 1 < args.len() {
                    let v = args[i + 1].clone();
                    spec_type = Some(v.clone());
                    ngram_spec = v.contains("ngram");
                    i += 2;
                    continue;
                }
            }
            "--spec-default" => {
                spec_default = true;
                i += 1;
                continue;
            }
            "--spec-draft-n-max" => {
                if i + 1 < args.len() {
                    let _ = args[i + 1].parse::<u32>().map(|v| {
                        spec_draft_n_max = Some(v);
                    });
                    i += 2;
                    continue;
                }
            }
            "--spec-draft-n-min" => {
                if i + 1 < args.len() {
                    let _ = args[i + 1].parse::<u32>().map(|v| {
                        spec_draft_n_min = Some(v);
                    });
                    i += 2;
                    continue;
                }
            }
            "--spec-draft-p-split" => {
                if i + 1 < args.len() {
                    let _ = args[i + 1].parse::<f32>().map(|v| {
                        spec_draft_p_split = Some(v);
                    });
                    i += 2;
                    continue;
                }
            }
            "--spec-draft-p-min" => {
                if i + 1 < args.len() {
                    let _ = args[i + 1].parse::<f32>().map(|v| {
                        spec_draft_p_min = Some(v);
                    });
                    i += 2;
                    continue;
                }
            }
            "--spec-draft-ngl" => {
                if i + 1 < args.len() {
                    let _ = args[i + 1].parse::<i32>().map(|v| {
                        spec_draft_ngl = Some(v);
                    });
                    i += 2;
                    continue;
                }
            }
            "--spec-draft-device" => {
                if i + 1 < args.len() {
                    spec_draft_device = Some(args[i + 1].clone());
                    i += 2;
                    continue;
                }
            }
            "--spec-draft-cpu-moe" => {
                spec_draft_cpu_moe = true;
                i += 1;
                continue;
            }
            "--spec-draft-n-cpu-moe" => {
                if i + 1 < args.len() {
                    let _ = args[i + 1].parse::<i32>().map(|v| {
                        spec_draft_n_cpu_moe = Some(v);
                    });
                    i += 2;
                    continue;
                }
            }
            "--spec-draft-type-k" => {
                if i + 1 < args.len() {
                    spec_draft_type_k = Some(args[i + 1].clone());
                    i += 2;
                    continue;
                }
            }
            "--spec-draft-type-v" => {
                if i + 1 < args.len() {
                    spec_draft_type_v = Some(args[i + 1].clone());
                    i += 2;
                    continue;
                }
            }
            "--spec-ngram-mod-n-min" => {
                if i + 1 < args.len() {
                    let _ = args[i + 1].parse::<u32>().map(|v| {
                        spec_ngram_mod_n_min = Some(v);
                    });
                    i += 2;
                    continue;
                }
            }
            "--spec-ngram-mod-n-max" => {
                if i + 1 < args.len() {
                    let _ = args[i + 1].parse::<u32>().map(|v| {
                        spec_ngram_mod_n_max = Some(v);
                    });
                    i += 2;
                    continue;
                }
            }
            "--spec-ngram-mod-n-match" => {
                if i + 1 < args.len() {
                    let _ = args[i + 1].parse::<u32>().map(|v| {
                        spec_ngram_mod_n_match = Some(v);
                    });
                    i += 2;
                    continue;
                }
            }
            "--spec-ngram-simple-size-n" => {
                if i + 1 < args.len() {
                    let _ = args[i + 1].parse::<u32>().map(|v| {
                        spec_ngram_simple_size_n = Some(v);
                    });
                    i += 2;
                    continue;
                }
            }
            "--spec-ngram-simple-size-m" => {
                if i + 1 < args.len() {
                    let _ = args[i + 1].parse::<u32>().map(|v| {
                        spec_ngram_simple_size_m = Some(v);
                    });
                    i += 2;
                    continue;
                }
            }
            "--spec-ngram-simple-min-hits" => {
                if i + 1 < args.len() {
                    let _ = args[i + 1].parse::<u32>().map(|v| {
                        spec_ngram_simple_min_hits = Some(v);
                    });
                    i += 2;
                    continue;
                }
            }
            "--spec-ngram-map-k-size-n" => {
                if i + 1 < args.len() {
                    let _ = args[i + 1].parse::<u32>().map(|v| {
                        spec_ngram_map_k_size_n = Some(v);
                    });
                    i += 2;
                    continue;
                }
            }
            "--spec-ngram-map-k-size-m" => {
                if i + 1 < args.len() {
                    let _ = args[i + 1].parse::<u32>().map(|v| {
                        spec_ngram_map_k_size_m = Some(v);
                    });
                    i += 2;
                    continue;
                }
            }
            "--spec-ngram-map-k-min-hits" => {
                if i + 1 < args.len() {
                    let _ = args[i + 1].parse::<u32>().map(|v| {
                        spec_ngram_map_k_min_hits = Some(v);
                    });
                    i += 2;
                    continue;
                }
            }
            "--spec-ngram-map-k4v-size-n" => {
                if i + 1 < args.len() {
                    let _ = args[i + 1].parse::<u32>().map(|v| {
                        spec_ngram_map_k4v_size_n = Some(v);
                    });
                    i += 2;
                    continue;
                }
            }
            "--spec-ngram-map-k4v-size-m" => {
                if i + 1 < args.len() {
                    let _ = args[i + 1].parse::<u32>().map(|v| {
                        spec_ngram_map_k4v_size_m = Some(v);
                    });
                    i += 2;
                    continue;
                }
            }
            "--spec-ngram-map-k4v-min-hits" => {
                if i + 1 < args.len() {
                    let _ = args[i + 1].parse::<u32>().map(|v| {
                        spec_ngram_map_k4v_min_hits = Some(v);
                    });
                    i += 2;
                    continue;
                }
            }
            "--kv-unified" => {
                if i + 1 < args.len() {
                    let _ = args[i + 1].parse::<bool>().map(|v| {
                        kv_unified = Some(v);
                    });
                    i += 2;
                    continue;
                }
            }
            "--cache-idle-slots" => {
                if i + 1 < args.len() {
                    let _ = args[i + 1].parse::<bool>().map(|v| {
                        cache_idle_slots = Some(v);
                    });
                    i += 2;
                    continue;
                }
            }
            "--fit-enabled" => {
                if i + 1 < args.len() {
                    let _ = args[i + 1].parse::<bool>().map(|v| {
                        fit_enabled = Some(v);
                    });
                    i += 2;
                    continue;
                }
            }
            "--fit-ctx" => {
                if i + 1 < args.len() {
                    let _ = args[i + 1].parse::<u32>().map(|v| {
                        fit_ctx = Some(v);
                    });
                    i += 2;
                    continue;
                }
            }
            "--fit-target" => {
                if i + 1 < args.len() {
                    fit_target = Some(args[i + 1].clone());
                    i += 2;
                    continue;
                }
            }
            "--fit-print" => {
                if i + 1 < args.len() {
                    let _ = args[i + 1].parse::<bool>().map(|v| {
                        fit_print = Some(v);
                    });
                    i += 2;
                    continue;
                }
            }
            "--prio" => {
                if i + 1 < args.len() {
                    let _ = args[i + 1].parse::<i32>().map(|v| {
                        prio = Some(v);
                    });
                    i += 2;
                    continue;
                }
            }
            "--prio-batch" => {
                if i + 1 < args.len() {
                    let _ = args[i + 1].parse::<i32>().map(|v| {
                        prio_batch = Some(v);
                    });
                    i += 2;
                    continue;
                }
            }
            "--temp" => {
                if i + 1 < args.len() {
                    if let Ok(v) = args[i + 1].parse::<f64>() {
                        temperature = Some(v);
                    }
                    i += 2;
                    continue;
                }
            }
            "--top-p" => {
                if i + 1 < args.len() {
                    if let Ok(v) = args[i + 1].parse::<f64>() {
                        top_p = Some(v);
                    }
                    i += 2;
                    continue;
                }
            }
            "--top-k" => {
                if i + 1 < args.len() {
                    if let Ok(v) = args[i + 1].parse::<i32>() {
                        top_k = Some(v);
                    }
                    i += 2;
                    continue;
                }
            }
            "--min-p" => {
                if i + 1 < args.len() {
                    if let Ok(v) = args[i + 1].parse::<f64>() {
                        min_p = Some(v);
                    }
                    i += 2;
                    continue;
                }
            }
            "--repeat-penalty" => {
                if i + 1 < args.len() {
                    if let Ok(v) = args[i + 1].parse::<f64>() {
                        repeat_penalty = Some(v);
                    }
                    i += 2;
                    continue;
                }
            }
            "--n-cpu-moe" => {
                if i + 1 < args.len() {
                    if let Ok(v) = args[i + 1].parse::<i32>() {
                        n_cpu_moe = Some(v);
                    }
                    i += 2;
                    continue;
                }
            }
            _ => {
                if i + 1 < args.len() && args[i + 1].chars().next().is_some_and(|c| c != '-') {
                    extra_args.push_str(arg);
                    extra_args.push(' ');
                    extra_args.push_str(&args[i + 1]);
                    extra_args.push(' ');
                    i += 2;
                    continue;
                } else {
                    extra_args.push_str(arg);
                    extra_args.push(' ');
                    i += 1;
                    continue;
                }
            }
        }
        i += 1;
    }

    let name = if model_path.is_empty() {
        "Imported preset".to_string()
    } else {
        let file = std::path::Path::new(&model_path)
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "Imported preset".into());
        format!("Imported: {}", file)
    };

    ModelPreset {
        id: crate::presets::next_id(),
        name,
        model_path,
        context_size,
        ctk: "f16".into(),
        ctv: "f16".into(),
        tensor_split: String::new(),
        batch_size: 2048,
        ubatch_size: 2048,
        no_mmap,
        ngram_spec,
        parallel_slots: 1,
        temperature,
        top_p,
        top_k,
        min_p,
        repeat_penalty,
        presence_penalty: None,
        n_cpu_moe,
        gpu_layers,
        mlock: false,
        flash_attn: String::new(),
        split_mode: String::new(),
        main_gpu: None,
        threads: None,
        threads_batch: None,
        prio: None,
        prio_batch: None,
        rope_scaling: String::new(),
        rope_freq_base: None,
        rope_freq_scale: None,
        draft_model,
        draft_min: None,
        draft_max: None,
        spec_ngram_size: None,
        spec_type,
        spec_default,
        spec_draft_n_max,
        spec_draft_n_min,
        spec_draft_p_split,
        spec_draft_p_min,
        spec_draft_ngl,
        spec_draft_device,
        spec_draft_cpu_moe,
        spec_draft_n_cpu_moe,
        spec_draft_type_k,
        spec_draft_type_v,
        spec_ngram_mod_n_min,
        spec_ngram_mod_n_max,
        spec_ngram_mod_n_match,
        spec_ngram_simple_size_n,
        spec_ngram_simple_size_m,
        spec_ngram_simple_min_hits,
        spec_ngram_map_k_size_n,
        spec_ngram_map_k_size_m,
        spec_ngram_map_k_min_hits,
        spec_ngram_map_k4v_size_n,
        spec_ngram_map_k4v_size_m,
        spec_ngram_map_k4v_min_hits,
        kv_unified,
        cache_idle_slots,
        cache_ram_mib: None,
        fit_enabled,
        fit_ctx,
        fit_target,
        fit_print,
        seed: None,
        system_prompt_file: String::new(),
        extra_args: extra_args.trim().to_string(),
        bind_host: None,
        port: None,
        hf_repo: None,
        chat_template_file: None,
        mmproj: None,
        image_min_tokens: None,
        image_max_tokens: None,
        grammar: None,
        json_schema: None,
        cache_type_k: None,
        cache_type_v: None,
        max_tokens: None,
        enable_thinking: None,
        preserve_thinking: None,
        reasoning: None,
        reasoning_budget: None,
        reasoning_budget_message: None,
        api_key: None,
        alias: None,
        benchmark_mode: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_windows_script() {
        let script = r#"
@echo off
llama-server.exe -m models\my-model.gguf -c 4096 -ngl 99 --temp 0.8
"#;
        let result = parse_launch_script(script, "windows").unwrap();
        assert!(result.preset.model_path.contains("my-model.gguf"));
        assert_eq!(result.preset.context_size, 4096);
        assert_eq!(result.preset.gpu_layers, Some(99));
        assert_eq!(result.preset.temperature, Some(0.8));
    }

    #[test]
    fn test_parse_simple_unix_script() {
        let script = r#"
#!/bin/bash
./llama-server -m /models/my-model.gguf -c 8192 --top-p 0.95
"#;
        let result = parse_launch_script(script, "linux").unwrap();
        assert!(result.preset.model_path.contains("my-model.gguf"));
        assert_eq!(result.preset.context_size, 8192);
        assert_eq!(result.preset.top_p, Some(0.95));
    }

    #[test]
    fn test_parse_windows_line_continuation() {
        let script = r#"
llama-server.exe -m "models\my-model.gguf" -c 4096 ^
    -ngl 99 --temp 0.7
"#;
        let result = parse_launch_script(script, "windows").unwrap();
        assert!(result.preset.model_path.contains("my-model.gguf"));
        assert_eq!(result.preset.context_size, 4096);
        assert_eq!(result.preset.gpu_layers, Some(99));
        assert_eq!(result.preset.temperature, Some(0.7));
    }

    #[test]
    fn test_parse_unix_line_continuation() {
        let script = r#"
./llama-server -m /models/my-model.gguf -c 4096 \
    -ngl 99 --top-k 40
"#;
        let result = parse_launch_script(script, "linux").unwrap();
        assert!(result.preset.model_path.contains("my-model.gguf"));
        assert_eq!(result.preset.context_size, 4096);
        assert_eq!(result.preset.gpu_layers, Some(99));
        assert_eq!(result.preset.top_k, Some(40));
    }

    #[test]
    fn test_parse_unknown_flags_goes_to_extra_args() {
        let script = r#"
llama-server -m model.gguf --foo bar
"#;
        let result = parse_launch_script(script, "linux").unwrap();
        assert!(result.preset.extra_args.contains("--foo bar"));
    }
}
