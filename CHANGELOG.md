# Changelog

## [0.5.0](https://github.com/nmorgowicz-org/llama-monitor/compare/v0.4.0...v0.5.0) (2026-04-18)


### Features

* **ci:** bump release-please-action to v4.4.1 ([#22](https://github.com/nmorgowicz-org/llama-monitor/issues/22)) ([30de47d](https://github.com/nmorgowicz-org/llama-monitor/commit/30de47dc136979c5504e864e071b00108b077ab0))

## [0.4.0](https://github.com/nickveldrin/llama-monitor/compare/v0.3.1...v0.4.0) (2026-04-15)


### Features

* add v0.4.0 features - motherboard detection, LHM integration, RAM fix, polling config ([#17](https://github.com/nickveldrin/llama-monitor/issues/17)) ([7b376c5](https://github.com/nickveldrin/llama-monitor/commit/7b376c5b17d69379e5d93ee61cb999623b916881))


### Bug Fixes

* LHM install button fix and Linux/MacOS system metrics ([#19](https://github.com/nickveldrin/llama-monitor/issues/19)) ([44358ff](https://github.com/nickveldrin/llama-monitor/commit/44358ffbb0535fe1347ab3af10a19743fd695d41))

## [0.4.0](https://github.com/nickveldrin/llama-monitor/compare/v0.3.1...v0.4.0) (2026-04-14)


### Features

* Add LHM (LibreHardwareMonitor) integration with polling improvements ([205b92d](https://github.com/nickveldrin/llama-monitor/commit/205b92d8e2f1c96f7c1a41104d0c8a1a8b5b8b8b))
* Add motherboard detection via WMI ([07b072f](https://github.com/nickveldrin/llama-monitor/commit/07b072f5a03b5a1e1b5b5b5b5b5b5b5b5b5b5b5b))
* Add polling interval configuration in UI ([205b92d](https://github.com/nickveldrin/llama-monitor/commit/205b92d8e2f1c96f7c1a41104d0c8a1a8b5b8b8b))


### Bug Fixes

* Fix RAM display calculation ([3c8cb6e](https://github.com/nickveldrin/llama-monitor/commit/3c8cb6ec732b18ea619a77789d15c52846bde437))


### Chore

* Add LHM service worker route and refactor JavaScript ([3c8cb6e](https://github.com/nickveldrin/llama-monitor/commit/3c8cb6ec732b18ea619a77789d15c52846bde437))


## [0.3.1](https://github.com/nickveldrin/llama-monitor/compare/v0.3.0...v0.3.1) (2026-04-13)


### Bug Fixes

* Use shell: bash for cross-platform packaging ([#14](https://github.com/nickveldrin/llama-monitor/issues/14)) ([0c350e0](https://github.com/nickveldrin/llama-monitor/commit/0c350e0bca83a3a32cf0130fcb9d0f92d35c7f30))


## [0.3.0](https://github.com/nickveldrin/llama-monitor/compare/v0.2.2...v0.3.0) (2026-04-13)


### Features

* Add Apple Silicon GPU monitoring support ([2ff92e1](https://github.com/nickveldrin/llama-monitor/commit/2ff92e11d256211420e967539c8254731828628e))
* Add Apple Silicon GPU monitoring support ([ad1320a](https://github.com/nickveldrin/llama-monitor/commit/ad1320a91158fdfb13864d2e3c556666ac1b95c3))
* Add missing api_attach endpoint, add recursion_limit, fix type casts and path traversal ([cbe18bf](https://github.com/nickveldrin/llama-monitor/commit/cbe18bfd6a4a87659cb0faecacd9ea33484b8033))
* Add multi-session support ([c13c5ac](https://github.com/nickveldrin/llama-monitor/commit/c13c5ac22fd10ee70549d09122bac32d9685c795))
* Add multi-session support ([867e71b](https://github.com/nickveldrin/llama-monitor/commit/867e71b3825f2f2def8ecc8eaf1c63469002f6ab))


### Bug Fixes

* Add warp features for server and websocket ([8ccf94f](https://github.com/nickveldrin/llama-monitor/commit/8ccf94f6f8e5a1c383fe136c74ac0fa0e36f4834))
* **api:** buffer response body for warp 0.4 compatibility ([8fcdca5](https://github.com/nickveldrin/llama-monitor/commit/8fcdca538f4d0c3a32ef72908fa2bdbc1f7b1c6c))
* **api:** buffer response body for warp 0.4 compatibility ([c1795da](https://github.com/nickveldrin/llama-monitor/commit/c1795da075bda24afe04cde81f119f9324c7b877))
* **apple:** cast f64 to f32 for temp and power_consumption ([e8c9f0c](https://github.com/nickveldrin/llama-monitor/commit/e8c9f0c2798dd2ca02ced33307ca6030f76a8679))
* **presets:** fix duplicate/default preset IDs ([ef62ed5](https://github.com/nickveldrin/llama-monitor/commit/ef62ed5863ad07e8710814beec4217bf20095ab6))
* **state:** resolve clippy warnings ([6f96b12](https://github.com/nickveldrin/llama-monitor/commit/6f96b12e56547a846e51c5bca6628e54c2addf9f))
* **system:** collapse collapsible-if statements ([7b49f9f](https://github.com/nickveldrin/llama-monitor/commit/7b49f9fb01b285ce7a785a1f2cf4c32455e4c3d2))
* Update Body import for warp 0.4 hyper v1 compatibility ([658458f](https://github.com/nickveldrin/llama-monitor/commit/658458fe729719e66d036b0b22800f6787325872))
* Update release workflow to use release.published event ([004da34](https://github.com/nickveldrin/llama-monitor/commit/004da348d039ca2d21b5184beb5324d1b84320b2))
* Update release workflow trigger to use release.published event ([32b933f](https://github.com/nickveldrin/llama-monitor/commit/32b933faf6db371e0b9a014e19f842e7c9110b34))


## [0.2.3](https://github.com/nickveldrin/llama-monitor/compare/v0.2.2...v0.2.3) (2026-04-13)


### Bug Fixes

* Fix release workflow by using RELEASE_PAT for automated artifact builds


## [0.2.2](https://github.com/nickveldrin/llama-monitor/compare/v0.2.1...v0.2.2) (2026-04-13)


### Bug Fixes

* Update release workflow to use release.published event ([004da34](https://github.com/nickveldrin/llama-monitor/commit/004da348d039ca2d21b5184beb5324d1b84320b2))
* Update release workflow trigger to use release.published event ([32b933f](https://github.com/nickveldrin/llama-monitor/commit/32b933faf6db371e0b9a014e19f842e7c9110b34))


## [0.2.1](https://github.com/nickveldrin/llama-monitor/compare/v0.2.0...v0.2.1) (2026-04-13)


### Bug Fixes

* Add warp features for server and websocket ([8ccf94f](https://github.com/nickveldrin/llama-monitor/commit/8ccf94f6f8e5a1c383fe136c74ac0fa0e36f4834))
* **api:** buffer response body for warp 0.4 compatibility ([8fcdca5](https://github.com/nickveldrin/llama-monitor/commit/8fcdca538f4d0c3a32ef72908fa2bdbc1f7b1c6c))
* **api:** buffer response body for warp 0.4 compatibility ([c1795da](https://github.com/nickveldrin/llama-monitor/commit/c1795da075bda24afe04cde81f119f9324c7b877))
* **presets:** fix duplicate/default preset IDs ([ef62ed5](https://github.com/nickveldrin/llama-monitor/commit/ef62ed5863ad07e8710814beec4217bf20095ab6))
* **state:** resolve clippy warnings ([6f96b12](https://github.com/nickveldrin/llama-monitor/commit/6f96b12e56547a846e51c5bca6628e54c2addf9f))
* **system:** collapse collapsible-if statements ([7b49f9f](https://github.com/nickveldrin/llama-monitor/commit/7b49f9fb01b285ce7a785a1f2cf4c32455e4c3d2))
* Update Body import for warp 0.4 hyper v1 compatibility ([658458f](https://github.com/nickveldrin/llama-monitor/commit/658458fe729719e66d036b0b22800f6787325872))


## [0.2.0](https://github.com/nickveldrin/llama-monitor/compare/v0.1.0...v0.2.0) (2026-04-12)


### Features

* Add missing api_attach endpoint, add recursion_limit, fix type casts and path traversal ([cbe18bf](https://github.com/nickveldrin/llama-monitor/commit/cbe18bfd6a4a87659cb0faecacd9ea33484b8033))
* Add Apple Silicon GPU monitoring support ([2ff92e1](https://github.com/nickveldrin/llama-monitor/commit/2ff92e11d256211420e967539c8254731828628e))
* Add Apple Silicon GPU monitoring support ([ad1320a](https://github.com/nickveldrin/llama-monitor/commit/ad1320a91158fdfb13864d2e3c556666ac1b95c3))
* Add multi-session support ([c13c5ac](https://github.com/nickveldrin/llama-monitor/commit/c13c5ac22fd10ee70549d09122bac32d9685c795))
* Add session management (v0.1.0-fork.1) ([98dba4d](https://github.com/nickveldrin/llama-monitor/commit/98dba4d4a4e6b1a4b1b1b1b1b1b1b1b1b1b1b1b1))


### Bug Fixes

* **apple:** cast f64 to f32 for temp and power_consumption ([e8c9f0c](https://github.com/nickveldrin/llama-monitor/commit/e8c9f0c2798dd2ca02ced33307ca6030f76a8679))
* **presets:** fix duplicate/default preset IDs ([ef62ed5](https://github.com/nickveldrin/llama-monitor/commit/ef62ed5863ad07e8710814beec4217bf20095ab6))
* **state:** resolve clippy warnings ([6f96b12](https://github.com/nickveldrin/llama-monitor/commit/6f96b12e56547a846e51c5bca6628e54c2addf9f))
* **system:** collapse collapsible-if statements ([7b49f9f](https://github.com/nickveldrin/llama-monitor/commit/7b49f9fb01b285ce7a785a1f2cf4c32455e4c3d2))
* Update CI/workflows and add AGENTS.md ([4858d7c](https://github.com/nickveldrin/llama-monitor/commit/4858d7c7e7e7e7e7e7e7e7e7e7e7e7e7e7e7e7e7))
* Add CI/release pipelines, bump tokio and reqwest, fix fmt/clippy ([b0434f7](https://github.com/nickveldrin/llama-monitor/commit/b0434f7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f7f))
* Remove models directory from config (replaced by file tree browser) ([6347a33](https://github.com/nickveldrin/llama-monitor/commit/6347a33a3a3a3a3a3a3a3a3a3a3a3a3a3a3a3a3a))
* Simplify control bar, merge Server+Monitor tabs, dedicate Logs tab ([ced2d8a](https://github.com/nickveldrin/llama-monitor/commit/ced2d8a8a8a8a8a8a8a8a8a8a8a8a8a8a8a8a8a8))
* Validate model path and server binary before starting ([739cc41](https://github.com/nickveldrin/llama-monitor/commit/739cc41414141414141414141414141414141414))
* Move server paths and GPU environment into config modal ([e4e31e4](https://github.com/nickveldrin/llama-monitor/commit/e4e31e4e4e4e4e4e4e4e4e4e4e4e4e4e4e4e4e4e))
* Replace model dropdown with file tree browser in preset modal ([0ae65b6](https://github.com/nickveldrin/llama-monitor/commit/0ae65b6b6b6b6b6b6b6b6b6b6b6b6b6b6b6b6b6b))
* Add copy preset button to duplicate selected preset ([a185fb4](https://github.com/nickveldrin/llama-monitor/commit/a185fb4b4b4b4b4b4b4b4b4b4b4b4b4b4b4b4b4b))
* Add file browser and UI-configurable server paths ([83acd62](https://github.com/nickveldrin/llama-monitor/commit/83acd62d2d2d2d2d2d2d2d2d2d2d2d2d2d2d2d2d))
* Remove hardcoded personal paths and machine-specific defaults ([0ead97c](https://github.com/nickveldrin/llama-monitor/commit/0ead97c7c7c7c7c7c7c7c7c7c7c7c7c7c7c7c7c7))
* Improve preset modal UX, persist settings, and proxy chat to UI port ([6c66e34](https://github.com/nickveldrin/llama-monitor/commit/6c66e34e4e4e4e4e4e4e4e4e4e4e4e4e4e4e4e4e))


## [0.1.0](https://github.com/nickveldrin/llama-monitor/compare/v0.1.0-fork.1...v0.1.0) (2026-04-10)


### Features

* Initial commit (clean) ([c5f5d0b](https://github.com/nickveldrin/llama-monitor/commit/c5f5d0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0))


## [0.1.0-fork.1](https://github.com/nickveldrin/llama-monitor/compare/v0.1.0...v0.1.0-fork.1) (2026-04-10)


### Features

* Add session management (v0.1.0-fork.1) ([98dba4d](https://github.com/nickveldrin/llama-monitor/commit/98dba4d4a4e6b1a4b1b1b1b1b1b1b1b1b1b1b1b1))
