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
        // Default to unix-style; caller can override via OS hint if needed.
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

    let preset = build_preset_from_args(&binary_path, &args);
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

    (binary_path, args, warnings)
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

fn build_preset_from_args(_binary_path: &str, args: &[String]) -> ModelPreset {
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
                    ngram_spec = args[i + 1].contains("ngram");
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
                // Unknown or unrecognized flags go into extra_args.
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
        n_cpu_moe,
        gpu_layers,
        mlock: false,
        flash_attn: String::new(),
        split_mode: String::new(),
        main_gpu: None,
        threads: None,
        threads_batch: None,
        rope_scaling: String::new(),
        rope_freq_base: None,
        rope_freq_scale: None,
        draft_model: String::new(),
        draft_min: None,
        draft_max: None,
        spec_ngram_size: None,
        seed: None,
        system_prompt_file: String::new(),
        extra_args: extra_args.trim().to_string(),
        hf_repo: None,
        chat_template_file: None,
        mmproj: None,
        grammar: None,
        json_schema: None,
        cache_type_k: None,
        cache_type_v: None,
        max_tokens: None,
        api_key: None,
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
