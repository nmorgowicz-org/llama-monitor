# Llama Monitor Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.12.0](https://github.com/nmorgowicz-org/llama-monitor/compare/v0.11.0...v0.12.0) (2026-04-30)


### Features

* **chat:** context compaction with summarization, polish, and smart trigger ([#125](https://github.com/nmorgowicz-org/llama-monitor/issues/125)) ([ba018a7](https://github.com/nmorgowicz-org/llama-monitor/commit/ba018a7b4e27740417120109f0ff1b8675a53bdb))

## [0.11.0](https://github.com/nmorgowicz-org/llama-monitor/compare/v0.10.2...v0.11.0) (2026-04-29)


### Features

* **chat:** UX overhaul — pagination, version display, and 18 enhancements ([#123](https://github.com/nmorgowicz-org/llama-monitor/issues/123)) ([a84bec0](https://github.com/nmorgowicz-org/llama-monitor/commit/a84bec047a082d11a302610fd7ea0c35fb12f490))

## [0.10.2](https://github.com/nmorgowicz-org/llama-monitor/compare/v0.10.1...v0.10.2) (2026-04-29)


### Bug Fixes

* **agent:** wake poller immediately on token change, quiet expected 401s ([#121](https://github.com/nmorgowicz-org/llama-monitor/issues/121)) ([f571ae7](https://github.com/nmorgowicz-org/llama-monitor/commit/f571ae7a49e8e2df4372cc7dd68a0edffbb2a865))

## [0.10.1](https://github.com/nmorgowicz-org/llama-monitor/compare/v0.10.0...v0.10.1) (2026-04-29)


### Bug Fixes

* **agent:** fix remote agent 401 loop and eliminate redundant SSH operations in update flow ([#119](https://github.com/nmorgowicz-org/llama-monitor/issues/119)) ([1c0af52](https://github.com/nmorgowicz-org/llama-monitor/commit/1c0af520b793b1eac4748fd4ff82afe6c676e524))

## [0.10.0](https://github.com/nmorgowicz-org/llama-monitor/compare/v0.9.4...v0.10.0) (2026-04-29)


### Features

* **chat:** overhaul UX with labels, suggested prompts, safe defaults, and advanced toggle ([#117](https://github.com/nmorgowicz-org/llama-monitor/issues/117)) ([07a4d63](https://github.com/nmorgowicz-org/llama-monitor/commit/07a4d6340010b6fb69dfa905d283fb54844b05f1))

## [0.9.4](https://github.com/nmorgowicz-org/llama-monitor/compare/v0.9.3...v0.9.4) (2026-04-28)


### Bug Fixes

* **docs:** enforce PR title convention for release-please compatibility ([#115](https://github.com/nmorgowicz-org/llama-monitor/issues/115)) ([d9e0e9e](https://github.com/nmorgowicz-org/llama-monitor/commit/d9e0e9eae1836bd58936327d90be2bb5cfdd9d87))

## [0.9.3](https://github.com/nmorgowicz-org/llama-monitor/compare/v0.9.2...v0.9.3) (2026-04-28)


### Bug Fixes

* **security:** eliminate TOCTOU race via inline script execution ([#112](https://github.com/nmorgowicz-org/llama-monitor/issues/112)) ([f2113ec](https://github.com/nmorgowicz-org/llama-monitor/commit/f2113ecf4af38d9f124e9b6cce8b4d2d24de747a))

## [0.9.2](https://github.com/nmorgowicz-org/llama-monitor/compare/v0.9.1...v0.9.2) (2026-04-28)


### Bug Fixes

* **security:** add HTTP security headers via warp-helmet ([#109](https://github.com/nmorgowicz-org/llama-monitor/issues/109)) ([b527c94](https://github.com/nmorgowicz-org/llama-monitor/commit/b527c943b1bc5681551491045b29e7e4dd12b126))

## [0.9.1](https://github.com/nmorgowicz-org/llama-monitor/compare/v0.9.0...v0.9.1) (2026-04-27)


### Bug Fixes

* **security:** migrate extract_archive to tempfile crate ([#15](https://github.com/nmorgowicz-org/llama-monitor/issues/15)) ([#107](https://github.com/nmorgowicz-org/llama-monitor/issues/107)) ([20faa38](https://github.com/nmorgowicz-org/llama-monitor/commit/20faa382ec102018164cb5c4a5b9a9d91bf0c916))

## [0.9.0](https://github.com/nmorgowicz-org/llama-monitor/compare/v0.8.5...v0.9.0) (2026-04-27)


### Features

* **security:** add mTLS infrastructure (cert generation, CA distribution) ([#104](https://github.com/nmorgowicz-org/llama-monitor/issues/104)) ([ffefb62](https://github.com/nmorgowicz-org/llama-monitor/commit/ffefb62ae4a282255a2e918f11c7c09cc63c9bac))

## [0.8.5](https://github.com/nmorgowicz-org/llama-monitor/compare/v0.8.4...v0.8.5) (2026-04-27)


### Bug Fixes

* secure temp files and GPU UI improvements ([#102](https://github.com/nmorgowicz-org/llama-monitor/issues/102)) ([52a3a04](https://github.com/nmorgowicz-org/llama-monitor/commit/52a3a04c685ff4244d78f1a081998079c66c8dc9))

## [0.8.4](https://github.com/nmorgowicz-org/llama-monitor/compare/v0.8.3...v0.8.4) (2026-04-27)


### Bug Fixes

* **ui:** hide Fix button by default and shrink GPU metrics ([#100](https://github.com/nmorgowicz-org/llama-monitor/issues/100)) ([7bc399f](https://github.com/nmorgowicz-org/llama-monitor/commit/7bc399fbf790d4ef65d7cfb893850b7fa61212c5))

## [0.8.3](https://github.com/nmorgowicz-org/llama-monitor/compare/v0.8.2...v0.8.3) (2026-04-27)


### Bug Fixes

* **ssrf:** remove user-controlled port from chat endpoint ([#96](https://github.com/nmorgowicz-org/llama-monitor/issues/96)) ([ee5905c](https://github.com/nmorgowicz-org/llama-monitor/commit/ee5905c34a0b1f230bd34011b0a505d58b3ced82))

## [0.8.2](https://github.com/nmorgowicz-org/llama-monitor/compare/v0.8.1...v0.8.2) (2026-04-27)


### Bug Fixes

* remote agent sensor_bridge install and UI improvements ([#94](https://github.com/nmorgowicz-org/llama-monitor/issues/94)) ([e6d531f](https://github.com/nmorgowicz-org/llama-monitor/commit/e6d531f4328cc24a56775fdfd44784df6662f102))

## [0.8.1](https://github.com/nmorgowicz-org/llama-monitor/compare/v0.8.0...v0.8.1) (2026-04-26)


### Bug Fixes

* **agent:** use cmd.exe-compatible quoting for schtasks ([#92](https://github.com/nmorgowicz-org/llama-monitor/issues/92)) ([9c04aac](https://github.com/nmorgowicz-org/llama-monitor/commit/9c04aacda13b8fe93b6788c25b477b03b9305660))

## [0.8.0](https://github.com/nmorgowicz-org/llama-monitor/compare/v0.7.10...v0.8.0) (2026-04-26)


### Features

* add --host flag and optional Basic Auth ([#90](https://github.com/nmorgowicz-org/llama-monitor/issues/90)) ([f2bb6f6](https://github.com/nmorgowicz-org/llama-monitor/commit/f2bb6f6a3cc440648a9f5d2aeaf93b9997d211b8))

## [0.7.10](https://github.com/nmorgowicz-org/llama-monitor/compare/v0.7.9...v0.7.10) (2026-04-26)


### Bug Fixes

* **agent:** prevent command injection via install paths ([#88](https://github.com/nmorgowicz-org/llama-monitor/issues/88)) ([8eed884](https://github.com/nmorgowicz-org/llama-monitor/commit/8eed884f4aa758be9c322b2dc5a42abd9495b065))

## [0.7.9](https://github.com/nmorgowicz-org/llama-monitor/compare/v0.7.8...v0.7.9) (2026-04-26)


### Bug Fixes

* **agent:** run Windows scheduled task as SYSTEM and cache system metrics ([#86](https://github.com/nmorgowicz-org/llama-monitor/issues/86)) ([dbfe6dd](https://github.com/nmorgowicz-org/llama-monitor/commit/dbfe6dd563458677314efd570e2d755c5090d1b3))

## [0.7.8](https://github.com/nmorgowicz-org/llama-monitor/compare/v0.7.7...v0.7.8) (2026-04-26)


### Bug Fixes

* **agent:** attempt SSH autostart once per disconnect instead of retrying ([#84](https://github.com/nmorgowicz-org/llama-monitor/issues/84)) ([57b2d31](https://github.com/nmorgowicz-org/llama-monitor/commit/57b2d31365e0584b461dae376eaaddd1e60766db))

## [0.7.7](https://github.com/nmorgowicz-org/llama-monitor/compare/v0.7.6...v0.7.7) (2026-04-26)


### Bug Fixes

* **agent:** suppress autostart during install and fix Windows file lock race ([#81](https://github.com/nmorgowicz-org/llama-monitor/issues/81)) ([f331e25](https://github.com/nmorgowicz-org/llama-monitor/commit/f331e255837da7cae642bed79a4ccb199006ab53))

## [0.7.6](https://github.com/nmorgowicz-org/llama-monitor/compare/v0.7.5...v0.7.6) (2026-04-26)


### Bug Fixes

* **ui:** restore scrolling in settings modal ([#78](https://github.com/nmorgowicz-org/llama-monitor/issues/78)) ([6908b08](https://github.com/nmorgowicz-org/llama-monitor/commit/6908b08346b738d0988df4a1d30a94b19c5e7477))

## [0.7.5](https://github.com/nmorgowicz-org/llama-monitor/compare/v0.7.4...v0.7.5) (2026-04-25)


### Bug Fixes

* **remote-agent:** repair Windows install follow-ups ([#75](https://github.com/nmorgowicz-org/llama-monitor/issues/75)) ([395e959](https://github.com/nmorgowicz-org/llama-monitor/commit/395e95987b0cb909a293933dc81665c5b9e21942))

## [0.7.4](https://github.com/nmorgowicz-org/llama-monitor/compare/v0.7.3...v0.7.4) (2026-04-25)


### Bug Fixes

* **remote-agent:** repair autostart and clarify setup errors ([#72](https://github.com/nmorgowicz-org/llama-monitor/issues/72)) ([c5686e5](https://github.com/nmorgowicz-org/llama-monitor/commit/c5686e55365116fa902f2ffe61dd27110d955870))

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
