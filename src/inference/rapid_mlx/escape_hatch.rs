#[derive(Debug, Clone, serde::Serialize)]
pub struct EscapeFlagDescriptor {
    pub flag: &'static str,
    pub value_type: &'static str,
    pub description: &'static str,
    pub tooltip: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enum_options: Option<&'static [&'static str]>,
}

pub const ALLOWED_ESCAPE_FLAGS: &[EscapeFlagDescriptor] = &[
    // ── PFlash family ──────────────────────────────────────────────────────────
    EscapeFlagDescriptor {
        flag: "pflash",
        value_type: "enum",
        description: "Prefilter cache mode",
        tooltip: "Controls how Rapid-MLX reuses KV cache across requests. 'auto' lets the runtime decide; 'always' maximizes reuse but uses more memory; 'off' disables it.",
        enum_options: Some(&["off", "auto", "always"]),
    },
    EscapeFlagDescriptor {
        flag: "pflash-threshold",
        value_type: "int",
        description: "PFlash activation threshold",
        tooltip: "Minimum prompt length (tokens) at which PFlash reuse becomes active. Below this, a direct forward pass is used.",
        enum_options: None,
    },
    EscapeFlagDescriptor {
        flag: "pflash-keep-ratio",
        value_type: "float",
        description: "PFlash keep ratio",
        tooltip: "Fraction of the prefix cache to preserve between requests. 0.0–1.0; higher keeps more history but consumes more memory.",
        enum_options: None,
    },
    EscapeFlagDescriptor {
        flag: "pflash-min-keep-tokens",
        value_type: "int",
        description: "PFlash minimum keep tokens",
        tooltip: "Absolute minimum number of tokens to retain in the prefix cache, regardless of keep-ratio.",
        enum_options: None,
    },
    EscapeFlagDescriptor {
        flag: "pflash-sink-tokens",
        value_type: "int",
        description: "PFlash sink tokens",
        tooltip: "Number of tokens at the end of a prompt to always treat as non-reusable (sink). Higher values reduce false positives in cache matches.",
        enum_options: None,
    },
    EscapeFlagDescriptor {
        flag: "pflash-tail-tokens",
        value_type: "int",
        description: "PFlash tail tokens",
        tooltip: "Tokens reserved at the tail of the cache for new content growth. Adjusts how aggressively the cache can extend.",
        enum_options: None,
    },
    EscapeFlagDescriptor {
        flag: "pflash-block-size",
        value_type: "int",
        description: "PFlash block size",
        tooltip: "Size of individual PFlash cache blocks in tokens. Must be a power of two (e.g. 64, 128, 256).",
        enum_options: None,
    },
    EscapeFlagDescriptor {
        flag: "pflash-query-window",
        value_type: "int",
        description: "PFlash query window",
        tooltip: "Number of leading tokens used as the query key for cache lookups. Larger windows give more precise matches but slower lookups.",
        enum_options: None,
    },
    EscapeFlagDescriptor {
        flag: "pflash-stride-blocks",
        value_type: "int",
        description: "PFlash stride blocks",
        tooltip: "Stride between PFlash block comparisons during cache scanning. Higher values skip blocks for speed but may miss matches.",
        enum_options: None,
    },
    EscapeFlagDescriptor {
        flag: "pflash-include-tools",
        value_type: "bool",
        description: "PFlash include tools",
        tooltip: "When true, tool definitions and tool-call history are included in PFlash cache keys. Disabling can improve reuse in agentic workloads with rotating tool sets.",
        enum_options: None,
    },
    // ── Spec-decode force toggles ───────────────────────────────────────────────
    EscapeFlagDescriptor {
        flag: "force-spec-decode",
        value_type: "bool",
        description: "Force speculative decoding",
        tooltip: "Override runtime heuristics and always enable speculative decoding, even if the model or hardware would normally disable it.",
        enum_options: None,
    },
    EscapeFlagDescriptor {
        flag: "no-spec-decode",
        value_type: "bool",
        description: "Disable speculative decoding",
        tooltip: "Force speculative decoding off regardless of model capabilities or runtime auto-detection.",
        enum_options: None,
    },
    // ── Hybrid force toggles ────────────────────────────────────────────────────
    EscapeFlagDescriptor {
        flag: "force-hybrid",
        value_type: "bool",
        description: "Force hybrid attention",
        tooltip: "Override the runtime's attention mode and force hybrid attention path. Useful for models where auto-detection is uncertain.",
        enum_options: None,
    },
    EscapeFlagDescriptor {
        flag: "no-hybrid",
        value_type: "bool",
        description: "Disable hybrid attention",
        tooltip: "Force pure attention path; disable hybrid attention mode even if the model supports it.",
        enum_options: None,
    },
];

/// Validate that all flags in a preset are from the allowlist. Returns the
/// subset of flags that failed validation, or empty if all are valid.
pub fn validate_escape_flags(flags: &[(String, serde_json::Value)]) -> Result<(), Vec<String>> {
    let allowlist: std::collections::HashSet<&str> =
        ALLOWED_ESCAPE_FLAGS.iter().map(|d| d.flag).collect();
    let mut invalid = Vec::new();
    for (name, _value) in flags {
        if !allowlist.contains(name.as_str()) {
            invalid.push(name.clone());
        }
    }
    if invalid.is_empty() {
        Ok(())
    } else {
        Err(invalid)
    }
}

/// Ensure the two process-lifecycle-internal flags are never exposed in the
/// allowlist. This is a compile-time sanity check.
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn watchdog_ppid_not_in_allowlist() {
        let names: Vec<&str> = ALLOWED_ESCAPE_FLAGS.iter().map(|d| d.flag).collect();
        assert!(
            !names.contains(&"watchdog-ppid"),
            "--watchdog-ppid must never be in the escape-hatch allowlist"
        );
    }

    #[test]
    fn listen_fd_not_in_allowlist() {
        let names: Vec<&str> = ALLOWED_ESCAPE_FLAGS.iter().map(|d| d.flag).collect();
        assert!(
            !names.contains(&"listen-fd"),
            "--listen-fd must never be in the escape-hatch allowlist"
        );
    }

    #[test]
    fn validate_rejects_non_allowlisted_flag() {
        let flags = vec![
            (
                "pflash".to_string(),
                serde_json::Value::String("auto".into()),
            ),
            ("unknown-flag".to_string(), serde_json::Value::Bool(true)),
        ];
        let err = validate_escape_flags(&flags).unwrap_err();
        assert_eq!(err, vec!["unknown-flag"]);
    }

    #[test]
    fn validate_accepts_all_allowlisted_flags() {
        let flags: Vec<_> = ALLOWED_ESCAPE_FLAGS
            .iter()
            .map(|d| {
                let value = match d.value_type {
                    "bool" => serde_json::Value::Bool(true),
                    "int" => serde_json::Value::Number(serde_json::Number::from(42)),
                    "float" => {
                        serde_json::Value::Number(serde_json::Number::from_f64(0.75).unwrap())
                    }
                    "enum" => serde_json::Value::String(
                        d.enum_options
                            .and_then(|opts| opts.first().copied())
                            .unwrap_or("auto")
                            .to_string(),
                    ),
                    _ => serde_json::Value::Null,
                };
                (d.flag.to_string(), value)
            })
            .collect();
        assert!(validate_escape_flags(&flags).is_ok());
    }
}
