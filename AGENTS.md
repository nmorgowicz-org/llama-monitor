# Llama Monitor Project Rules

## Conventional Commits

All commits MUST follow the [Conventional Commits](https://www.conventionalcommits.org/) format:

```
<type>(<scope>): <description>
```

### Commit Types

| Type | Purpose | Version Bump |
|------|---------|--------------|
| `feat` | New feature | MINOR |
| `fix` | Bug fix | PATCH |
| `perf` | Performance improvement | PATCH |
| `refactor` | Code refactoring (no behavior change) | No bump |
| `test` | Adding tests | No bump |
| `chore` | Maintenance tasks | No bump |
| `docs` | Documentation changes | No bump |
| `ci` | CI/CD changes | No bump |
| `revert` | Revert previous commit | Depends on reverted commit |

### Format Examples

✅ **Valid:**
- `feat: add new API endpoint`
- `feat(api): add user authentication`
- `fix: resolve memory leak in session manager`
- `fix(gpu): correct temperature reading for AMD GPUs`
- `perf: optimize GPU metrics polling`
- `refactor: simplify session state management`
- `test: add unit tests for GPU monitoring`
- `chore: update dependencies`
- `docs: add API reference documentation`
- `ci: add conventional commit validation`
- `revert: revert breaking change from v0.2.0`

❌ **Invalid:**
- `Update README` (missing type)
- `feat add new feature` (missing colon)
- `fix: bug fix` (no description after colon)
- `Fix bug in GPU monitoring` (wrong format)

### Branch Naming

When creating branches for releases, use the format:

```
release/v<version>
```

Examples:
- `release/v0.2.0`
- `release/v1.0.0`

### AI Agent Guidelines

1. **Always use conventional commit format** - Never commit without a type prefix
2. **Use lowercase** - `ci:` not `CI:`, `feat:` not `Feat:`
3. **Use parentheses for scope** - `feat(api):`, not `feat api:`
4. **Describe what changed** - Be specific about the change
5. **Chores don't bump version** - `chore:`, `refactor:`, `ci:`, `docs:` don't trigger version bumps
6. **feat and fix bump versions** - Only these types trigger semantic version increments

## Development Workflow

1. Create feature branch: `git checkout -b feature/my-feature`
2. Make changes and commit with conventional format
3. Push branch and create PR
4. Merge to `main` → release-please creates release PR
5. Merge release PR → tag created → release artifacts built

## Project Structure

- `src/` - Rust backend code
- `static/` - Web frontend (HTML/CSS/JS, embedded at compile time)
- `docs/` - Documentation files

## Build & Test Commands

```bash
# Build
cargo build --release

# Run tests
cargo test

# Lint
cargo clippy -- -D warnings

# Format
cargo fmt
```

## CI/CD Workflow

### Pull Requests

1. **Title format**: Use conventional commit format (`feat:`, `fix:`, etc.)
2. **Auto-labeling**: GitHub labels PRs based on `src/` directory paths:
   - `gpu/` → `gpu`, `nvidia/` or `rocm/` or `apple.rs` → related vendor labels
   - `llama/` → `llama`, `web/` → `web`, `state.rs` → `core`
3. **CI checks**: All PRs must pass:
   - `cargo fmt -- --check`
   - `cargo clippy -- -D warnings`
   - `cargo test`
   - `cargo build --release`

### Releases

- **Automated**: release-please creates release PRs when `feat:` or `fix:` commits merge to `main`
- **Release type**: Rust (semantic versioning based on commit types)
- **Version bump**: `feat:` → MINOR, `fix:` → PATCH, others → no bump

## File Persistence

All user data persists to `~/.config/llama-monitor/`:

| File | Purpose |
|------|---------|
| `sessions.json` | Session definitions (spawn/attach mode, ports, server configs) |
| `presets.json` | Model presets with all llama.cpp parameters |
| `ui-settings.json` | Web UI preferences (paths, ports, presets) |
| `gpu-env.json` | GPU environment config (architecture, device indices) |

Data is persisted:
- Sessions: every 30 seconds + on explicit save
- Presets/Settings: on explicit save via API

## Git Branch Strategy

- **`main`**: Stable, release-ready code
- **`feature/*`**: Feature development branches
- **`release/*`**: Release preparation (created by release-please)
- **`hotfix/*`**: Critical bug fixes for production

All branches should be deleted after merge.
