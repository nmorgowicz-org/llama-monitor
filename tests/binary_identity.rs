use std::process::Command;

fn run(binary: &str, args: &[&str]) -> std::process::Output {
    Command::new(binary)
        .args(args)
        .output()
        .unwrap_or_else(|error| panic!("failed to run {binary}: {error}"))
}

#[test]
fn canonical_and_legacy_versions_are_identical() {
    let canonical = run(env!("CARGO_BIN_EXE_local-llm-foundry"), &["--version"]);
    let legacy = run(env!("CARGO_BIN_EXE_llama-monitor"), &["--version"]);
    assert!(canonical.status.success());
    assert!(legacy.status.success());
    assert_eq!(canonical.stdout, legacy.stdout);
    assert!(String::from_utf8_lossy(&legacy.stderr).contains("use local-llm-foundry"));
}

#[test]
fn canonical_and_legacy_help_are_identical() {
    let canonical = run(env!("CARGO_BIN_EXE_local-llm-foundry"), &["--help"]);
    let legacy = run(env!("CARGO_BIN_EXE_llama-monitor"), &["--help"]);
    assert!(canonical.status.success());
    assert!(legacy.status.success());
    assert_eq!(canonical.stdout, legacy.stdout);
    assert!(String::from_utf8_lossy(&canonical.stdout).contains("Usage: local-llm-foundry"));
}

#[test]
fn canonical_and_legacy_invalid_arguments_share_exit_code() {
    let canonical = run(
        env!("CARGO_BIN_EXE_local-llm-foundry"),
        &["--not-a-real-flag"],
    );
    let legacy = run(env!("CARGO_BIN_EXE_llama-monitor"), &["--not-a-real-flag"]);
    assert!(!canonical.status.success());
    assert_eq!(canonical.status.code(), legacy.status.code());
    assert!(String::from_utf8_lossy(&legacy.stderr).contains("use local-llm-foundry"));
}
