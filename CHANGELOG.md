# Llama Monitor Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.7.3](https://github.com/nmorgowicz-org/llama-monitor/compare/v0.7.2...v0.7.3) (2026-04-25)


### Bug Fixes

* **remote-agent:** stop Windows agent before repair install ([#70](https://github.com/nmorgowicz-org/llama-monitor/issues/70)) ([ee7fb1a](https://github.com/nmorgowicz-org/llama-monitor/commit/ee7fb1aeb4671dcf5e0756075ad56be75f71de59))

## [0.7.2](https://github.com/nmorgowicz-org/llama-monitor/compare/v0.7.1...v0.7.2) (2026-04-25)


### Bug Fixes

* **remote-agent:** repair Windows remote install archive handling ([#68](https://github.com/nmorgowicz-org/llama-monitor/issues/68)) ([17d404c](https://github.com/nmorgowicz-org/llama-monitor/commit/17d404c5c31800ff7215ff7b7058b508630446e4))

## [0.7.1](https://github.com/nmorgowicz-org/llama-monitor/compare/v0.7.0...v0.7.1) (2026-04-25)


### Bug Fixes

* **ci:** remove setup-dotnet action and add PR labeler ([#60](https://github.com/nmorgowicz-org/llama-monitor/issues/60)) ([c7ab22f](https://github.com/nmorgowicz-org/llama-monitor/commit/c7ab22f85f33a99c07ce01d639037b1be11e5f2f))

## [0.7.0](https://github.com/nmorgowicz-org/llama-monitor/compare/v0.6.3...v0.7.0) (2026-04-24)


### Features

* add tool-call blocked state detection with throughput card integration ([#56](https://github.com/nmorgowicz-org/llama-monitor/issues/56)) ([36880b4](https://github.com/nmorgowicz-org/llama-monitor/commit/36880b4b93844ce0d746b6e6f20f14bacf97b11c))
* **agent:** resolve Windows %APPDATA% path for remote agent scheduler ([035b659](https://github.com/nmorgowicz-org/llama-monitor/commit/035b659579c896833f01ef8419eed116b99f1a54))


### Bug Fixes

* **agent:** resolve Windows %APPDATA% path for remote agent scheduler ([44ad9a9](https://github.com/nmorgowicz-org/llama-monitor/commit/44ad9a96ee51a47fde37834cdaa676c0ebee42da))
* **agent:** wire resolve_windows_appdata into all schtasks command paths ([0d96a13](https://github.com/nmorgowicz-org/llama-monitor/commit/0d96a13f9e45f160a1ca9f3c8c53aaee209eca03))
* **ui:** refine hardware metrics visuals and windows helper packaging ([#57](https://github.com/nmorgowicz-org/llama-monitor/issues/57)) ([44200fb](https://github.com/nmorgowicz-org/llama-monitor/commit/44200fbed630fb93991890c7fa3b19809b88f482))

## [0.6.3](https://github.com/nmorgowicz-org/llama-monitor/compare/v0.6.2...v0.6.3) (2026-04-23)


### Bug Fixes

* **ci:** add explicit permissions to CI workflow ([791b826](https://github.com/nmorgowicz-org/llama-monitor/commit/791b82612747e27c307edfa53da54817d44a695c))
* **js:** properly escape backslashes in file browser paths ([8b89703](https://github.com/nmorgowicz-org/llama-monitor/commit/8b897038c8dcc93a4835a2f9a162e8c6597f7753))

## [0.6.2](https://github.com/nmorgowicz-org/llama-monitor/compare/v0.6.1...v0.6.2) (2026-04-22)


### Bug Fixes

* **ci:** use shared cache key to eliminate cache duplication ([0bc4c96](https://github.com/nmorgowicz-org/llama-monitor/commit/0bc4c96a4cf37626041bdf2fb00231da77fb084c))
* **ci:** use shared cache key to eliminate cache duplication ([4ff264d](https://github.com/nmorgowicz-org/llama-monitor/commit/4ff264d9f0adf0d4bfb2301f8da2265ea33d16eb))

## [0.6.1](https://github.com/nmorgowicz-org/llama-monitor/compare/v0.6.0...v0.6.1) (2026-04-22)


### Bug Fixes

* **release:** add AR and RANLIB env vars for macOS cross-compilation ([#41](https://github.com/nmorgowicz-org/llama-monitor/issues/41)) ([0fb73bf](https://github.com/nmorgowicz-org/llama-monitor/commit/0fb73bfa342a2700f5a20cfb656e850cb9e44298))

## [0.6.0](https://github.com/nmorgowicz-org/llama-monitor/compare/v0.5.1...v0.6.0) (2026-04-22)


### Features

* **ui:** comprehensive UI/UX modernization with remote agent, inference dashboard, and capability-aware rendering ([#38](https://github.com/nmorgowicz-org/llama-monitor/issues/38)) ([dfc0ed8](https://github.com/nmorgowicz-org/llama-monitor/commit/dfc0ed8870c55255a4205e7208ff2f5bc9833f13))
* **ui:** Phase 5 modern UI - dashboard grid, toast notifications, keyboard shortcuts ([dfc0ed8](https://github.com/nmorgowicz-org/llama-monitor/commit/dfc0ed8870c55255a4205e7208ff2f5bc9833f13))

## [Unreleased]

### Added

- Remote agent functionality
- `--agent` flag to run as lightweight remote metrics agent
- `--agent-host` and `--agent-port` CLI flags for agent configuration
- `--agent-token` for bearer token authentication
- `--remote-agent-url` and `--remote-agent-token` for dashboard polling configuration
- `--remote-agent-ssh-autostart` and SSH-related flags for remote agent autostart
- `/api/remote-agent/releases/latest` endpoint for release checking
- `/api/remote-agent/detect` endpoint for remote host detection
- `agent.rs` module with `run_agent_server`, `latest_release_info`, and `detect_remote_agent` functions
- Remote agent URL inference from attached llama-server endpoint
- SSH autostart support for unreachable remote agents
- Remote agent connection tracking in state
- Capability-aware UI with "Inference only" warning when host metrics unavailable
- Compact view support for remote agent scenarios
- GPU/System section visibility toggling based on capabilities
- Remote agent status display in web UI
- Session endpoint information in WebSocket updates
- `host_metrics_available()` and `remote_agent_connected()` state methods

### Changed

- Refactored `AppState` to track `remote_agent_connected` and `remote_agent_url`
- Added `current_session_kind()` and `current_endpoint_kind()` helper methods
- Updated tray to use `host_metrics_available()` instead of `active_session_uses_local_metrics()`
- Updated WebSocket updates to include remote agent connection state
- Updated metrics polling to skip when no local metrics needed
- GPU metrics types now implement `Deserialize` for remote agent compatibility
- System metrics types now implement `Deserialize` for remote agent compatibility
- Capability calculation now includes remote agent connected scenario
- UI settings now store remote agent configuration

### Fixed

- GPU detection logic now handles Windows `where` command
- System/GPU section visibility in compact view

## [0.2.0] - 2026-04-20

### Added

- Initial release with core functionality
- Session management (spawn/attach modes)
- GPU metrics monitoring (NVIDIA, AMD, Apple Silicon)
- Llama server integration
- System metrics collection
- Tray icon with capability-aware display
- Remote agent UI affordance
- Integration tests for capabilities
