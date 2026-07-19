# Platform Expectations

## macOS

- Status: Functional support REQUIRED.
- Rapid-MLX is a first-class backend on Apple Silicon.
- All Phase outputs (estimates, launch, cache, diagnostics) must work correctly.
- Testing/validation uses real hardware (M5 Max).

## Linux

- Status: Graceful unavailability REQUIRED.
- Rapid-MLX depends on MLX (Apple Metal) and cannot run on Linux.
- Expectations:
  - Wizard/preset editor: viewable and editable; Rapid backend shows clear "Unavailable on this platform" state.
  - No crashes or misleading capability claims.
  - Llama.cpp backend remains fully functional.
  - No hidden Rapid-specific launch attempts.

## Windows

- Status: Graceful unavailability REQUIRED.
- Same as Linux: MLX unavailable; Rapid backend shows clear unavailable state.
- Wizard/preset editor viewable and editable.
- Llama.cpp backend fully functional.

## Cross-Platform Rule

- No platform-specific code without equivalent or explicit `#[cfg]` stub.
- After changes to `src/tray.rs`, `Cargo.toml`, or files with `#[cfg]`, run:
  `rustup target add x86_64-pc-windows-gnu && cargo check --target x86_64-pc-windows-gnu`
- Rapid-specific capabilities must degrade safely: hidden read-only or explicit unavailable message; never silently accepted and failed at launch.
