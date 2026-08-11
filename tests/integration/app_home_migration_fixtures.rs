//! Phase 3 pure application-home migration fixture matrix.
//!
//! These fixtures deliberately exercise planning/inventory only unless a test
//! name explicitly says execution. They are safe to run on a developer machine:
//! every root lives below a temporary directory and no real application root is
//! consulted.

use std::fs;
use std::path::{Path, PathBuf};

use llama_monitor::app_migration::{
    EntryKind, ResourceClass, RootState, inspect_roots, plan_application_home,
};

struct Fixture {
    root: tempfile::TempDir,
    source: PathBuf,
    destination: PathBuf,
}

impl Fixture {
    fn new(name: &str) -> Self {
        let root = tempfile::Builder::new()
            .prefix(&format!("foundry-phase3-{name}-"))
            .tempdir()
            .expect("fixture tempdir");
        let source = root.path().join("legacy");
        let destination = root.path().join("canonical");
        fs::create_dir_all(&source).expect("source root");
        Self {
            root,
            source,
            destination,
        }
    }

    fn write(&self, relative: &str, bytes: &[u8]) {
        let path = self.source.join(relative);
        fs::create_dir_all(path.parent().expect("fixture parent")).expect("fixture directory");
        fs::write(path, bytes).expect("fixture file");
    }

    fn plan(&self) -> llama_monitor::app_migration::AppHomeMigrationPlan {
        plan_application_home(&self.source, &self.destination).expect("pure plan")
    }
}

#[test]
fn matrix_fresh_empty_old_only_and_custom_root_states_are_explicit() {
    let fixture = Fixture::new("states");
    let empty_canonical = fixture.root.path().join("empty-canonical");
    fs::create_dir_all(&empty_canonical).unwrap();
    assert_eq!(
        inspect_roots(&empty_canonical, &fixture.source, false)
            .unwrap()
            .state,
        RootState::Fresh
    );

    fixture.write("presets.json", b"{}");
    assert_eq!(
        inspect_roots(&empty_canonical, &fixture.source, false)
            .unwrap()
            .state,
        RootState::LegacyActive
    );
    assert_eq!(
        inspect_roots(&empty_canonical, &fixture.source, true)
            .unwrap()
            .state,
        RootState::CustomConfig
    );
}

#[test]
fn matrix_both_roots_collision_is_fail_closed() {
    let fixture = Fixture::new("collision");
    fixture.write("presets.json", b"legacy");
    fs::create_dir_all(&fixture.destination).unwrap();
    fs::write(fixture.destination.join("presets.json"), b"new").unwrap();
    assert_eq!(
        inspect_roots(&fixture.destination, &fixture.source, false)
            .unwrap()
            .state,
        RootState::Conflict
    );
    assert!(plan_application_home(&fixture.source, &fixture.destination).is_err());
}

#[test]
fn matrix_partial_downloads_hf_cache_models_and_runtime_are_retained() {
    let fixture = Fixture::new("external");
    fixture.write("models/.staging/model.part", b"partial");
    fixture.write("models/cache/huggingface/blobs/blob", b"blob");
    fixture.write(
        "models/rapid-mlx/mtp-sidecars/sidecar/mtp.safetensors",
        b"weights",
    );
    fixture.write("runtimes/rapid-mlx/bin/rapid-mlx", b"runtime");
    let plan = fixture.plan();
    for retained in ["models", "runtimes"] {
        assert!(
            plan.retained_entries
                .iter()
                .any(|path| path == Path::new(retained))
        );
    }
    assert!(plan.entries.iter().any(|entry| {
        entry.relative_path == Path::new("models")
            && entry.class == ResourceClass::ModelRetained
            && entry.kind == EntryKind::Directory
    }));
}

#[test]
fn matrix_critical_tls_certs_tokens_chat_db_and_wal_are_copy_candidates() {
    let fixture = Fixture::new("critical");
    for file in [
        "encryption-key",
        "api-token",
        "db-admin-token",
        "certs/ca.pem",
        "tls-config.json",
        "chat.db",
        "chat.db-wal",
    ] {
        fixture.write(file, b"fixture");
    }
    let plan = fixture.plan();
    assert!(plan.entries.iter().any(|entry| {
        entry.relative_path == Path::new("chat.db") && entry.class == ResourceClass::Critical
    }));
    assert!(plan.required_copy_bytes > 0);
    assert!(
        plan.entries
            .iter()
            .any(|entry| entry.relative_path == Path::new("chat.db-wal"))
    );
}

#[cfg(unix)]
#[test]
fn matrix_symlink_and_special_entry_are_rejected_before_mutation() {
    let fixture = Fixture::new("unsafe");
    std::os::unix::fs::symlink("/tmp", fixture.source.join("escape")).unwrap();
    assert!(plan_application_home(&fixture.source, &fixture.destination).is_err());

    let fixture = Fixture::new("special");
    let fifo = fixture.source.join("socket");
    let _listener = std::os::unix::net::UnixListener::bind(&fifo).unwrap();
    assert!(plan_application_home(&fixture.source, &fixture.destination).is_err());
}

#[test]
fn matrix_cross_volume_simulation_has_stable_plan_id_and_no_preview_writes() {
    let fixture = Fixture::new("stable");
    fixture.write("presets.json", b"stable");
    let first = fixture.plan();
    let second = fixture.plan();
    assert_eq!(first.plan_id, second.plan_id);
    assert!(!fixture.destination.exists());
}
