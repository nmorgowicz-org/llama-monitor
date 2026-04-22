# Llama Monitor Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

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
