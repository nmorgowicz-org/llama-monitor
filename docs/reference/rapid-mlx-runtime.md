# Rapid-MLX Runtime Management

Llama Monitor treats Rapid-MLX as a first-class inference engine while keeping its
managed runtime isolated from the user's Python tooling. Local Rapid-MLX launch and
managed runtime changes are supported only on Apple Silicon macOS. Remote attachment
remains backend-aware on other platforms.

## Runtime ownership

The application may discover managed, custom, Homebrew, Pip, or Pipx installations, but
it mutates only environments under:

```text
<config-dir>/runtimes/rapid-mlx/
├── current.json
├── environments/
├── uv-cache/
└── uv-python/
```

Each managed environment is immutable after completion. Its manifest records the exact
published version, release channel, runtime source, compatibility result, executable
location relative to the environment, and executable SHA-256. Inventory and rollback
revalidate the manifest, completion marker, path containment, and hash before trusting
an environment.

The public API does not expose executable paths. User-owned runtimes are never repaired,
upgraded, removed, or converted into managed runtimes.

## Install and upgrade transaction

An install or upgrade performs one bounded transaction:

1. Confirm the exact version and channel against non-draft published release metadata.
2. Create a new app-owned staging environment.
3. Run `uv tool install rapid-mlx==<version>` with inherited environment variables
   cleared, configuration disabled, copy link mode, bounded output, and app-owned tool,
   cache, and Python directories.
4. Resolve the executable inside the uv tool environment, hash it, and probe its exact
   version and required serve capabilities.
5. Write the typed completion manifest and marker.
6. Atomically replace `current.json`, retaining the former active environment as the
   previous known-good rollback candidate.
7. Remove complete environments that are neither active nor previous.

Failure, timeout, excess output, capability mismatch, or version mismatch terminates and
reaps the installer process tree, removes the failed staging environment, and leaves the
active pointer unchanged.

## Channels and daily updates

Release support is capability-driven rather than allowlist-driven. Any exact stable
release at or above the compatibility floor can activate after all staged checks pass.
Stable is the default channel. A non-draft published prerelease may be selected
explicitly and receives the same checks. Drafts, mutable branches, arbitrary package
names, and unversioned local builds are not managed releases.

This permits frequent upstream updates without coupling Llama Monitor releases to every
Rapid-MLX tag, while still failing safely when a new release changes a required contract.

## Repair and rollback

Repair reads the active manifest, re-verifies its exact version and channel against
published release metadata, and installs that release into a fresh immutable environment.
It does not change the current environment in place.

Rollback re-hashes and compatibility-probes the previous known-good environment before
atomically swapping active and previous. A missing or tampered rollback candidate is
rejected without changing the pointer.

Only one mutation may be queued or running. Runtime mutation endpoints require the
`db-admin-token` and an exact app-native confirmation string. Status, release, and job
reads require the `api-token`. See [api.md](api.md#rapid-mlx-runtime-management) for the
HTTP contracts.
