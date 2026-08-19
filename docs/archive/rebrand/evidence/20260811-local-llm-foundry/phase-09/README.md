# Phase 9 Runtime, agent, updater, tray, and platform identity

## Implemented

- Canonical and legacy release repositories, user-agent, Windows task names,
  sensor task names, agent log path, and update staging prefix are centralized
  in `src/identity.rs`.
- Release asset selection is deterministic: canonical `local-llm-foundry-*`
  assets win over legacy `llama-monitor-*` assets regardless of GitHub response
  ordering.
- `checksums.json` is retained as a first-class release field instead of being
  discarded by the binary-asset filter; updater verification now fetches the
  retained URL and fails closed when it is absent or malformed.
- Unix stop/remove operations target exact process names for both families;
  broad `pkill -f llama-monitor` matching is removed from those operations.
- Remote token reads prefer the canonical application root and retain a legacy
  fallback. Windows scheduled-agent startup passes an explicit resolved
  `--config-dir`, and canonical task registration retires the legacy agent task.
- Remote status/update/remove preserve a discovered legacy install path until
  the user explicitly migrates it; managed-task status recognizes both task
  families and reports the exact command/path match.
- Agent credentials now use `rand::rngs::SysRng` exclusively; timestamp/PID
  fallback entropy has been removed.
- Temporary bootstrap token files use system entropy, exclusive creation, mode
  `0600` on Unix, canonical filenames, and a legacy read fallback.
- Existing valid CA/key pairs are retained regardless of sentinel presence;
  the stable legacy CA subject (`llama-monitor CA`) remains unchanged.
- The tray now decodes the approved Token Ingot asset, uses the canonical
  tooltip/menu identity, and retains platform-specific template handling.
- Self-update staging/restart paths use `current_exe()` and canonical restart
  logging; Windows extraction accepts both canonical and legacy executable
  names.
- Repository-context detection accepts both the renamed package and legacy
  package manifests during the 2.x transition.

## Validation receipt

| Check | Result |
|---|---|
| `cargo fmt -- --check` | passed |
| `cargo clippy -- -D warnings` | passed |
| `cargo test` | 1,238 passed, 13 ignored |
| `npm run validate-js` | passed |
| `npm run lint` | passed |
| `git diff --check` | passed |
| `cargo build --release` | passed |
| `cargo check --target x86_64-pc-windows-gnu` | passed |
| Canonical-first asset selection test | passed |
| Checksum URL retention regression test | passed |

## Explicit return markers

- [ ] Native Windows scheduled-task migration and duplicate-task proof.
- [ ] Native Windows tray icon/popover proof.
- [ ] Mixed old-controller/new-agent and new-controller/old-agent matrix on
  Windows.
- [ ] Final Windows-machine verification remains required; GNU cross-check is
  not a substitute for native execution.
