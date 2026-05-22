# Llama Monitor Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.21.1](https://github.com/nmorgowicz-org/llama-monitor/compare/v0.21.0...v0.21.1) (2026-05-22)


### Bug Fixes

* **agent:** fix HTTPS client builder error preventing remote agent connections after reqwest 0.13 upgrade ([7203abf](https://github.com/nmorgowicz-org/llama-monitor/commit/7203abf6b0f8960e1729d99fa6dfb64a5d56d3cc))

## [0.21.0](https://github.com/nmorgowicz-org/llama-monitor/compare/v0.20.0...v0.21.0) (2026-05-22)


### Features

* **chat:** add next reply plan summary showing active steering inputs ([72950e3](https://github.com/nmorgowicz-org/llama-monitor/commit/72950e3b25b3023854753b51b10390a9e86b6a78))
* **chat:** add workspace command palette (Ctrl+K omnibox) with conversation search and quick actions ([72950e3](https://github.com/nmorgowicz-org/llama-monitor/commit/72950e3b25b3023854753b51b10390a9e86b6a78))
* **chat:** persist composer drafts per-tab with save/restore/clear lifecycle ([72950e3](https://github.com/nmorgowicz-org/llama-monitor/commit/72950e3b25b3023854753b51b10390a9e86b6a78))
* **chat:** track persona version per tab with template_version_or_hash for safe drift detection ([72950e3](https://github.com/nmorgowicz-org/llama-monitor/commit/72950e3b25b3023854753b51b10390a9e86b6a78))
* **chat:** use backend message_count for inactive tabs in sidebar and command palette ([72950e3](https://github.com/nmorgowicz-org/llama-monitor/commit/72950e3b25b3023854753b51b10390a9e86b6a78))
* **sessions:** add recent-endpoints dashboard to setup screen with one-click reconnect ([72950e3](https://github.com/nmorgowicz-org/llama-monitor/commit/72950e3b25b3023854753b51b10390a9e86b6a78))
* **sessions:** extend Session model with connect metadata and GET /api/sessions/recent endpoint ([72950e3](https://github.com/nmorgowicz-org/llama-monitor/commit/72950e3b25b3023854753b51b10390a9e86b6a78))
* **ui:** add actionable empty-state copy for GPU and system cards with connecting state ([72950e3](https://github.com/nmorgowicz-org/llama-monitor/commit/72950e3b25b3023854753b51b10390a9e86b6a78))
* **ui:** add unified telemetry grade with 9-state derivation and grade chip on agent badge ([72950e3](https://github.com/nmorgowicz-org/llama-monitor/commit/72950e3b25b3023854753b51b10390a9e86b6a78))
* **ui:** expose protocol_too_old and protocol_version on WebSocket payload ([72950e3](https://github.com/nmorgowicz-org/llama-monitor/commit/72950e3b25b3023854753b51b10390a9e86b6a78))


### Bug Fixes

* **chat:** context notes badge now shows correct per-tab count instead of leaking between tabs ([72950e3](https://github.com/nmorgowicz-org/llama-monitor/commit/72950e3b25b3023854753b51b10390a9e86b6a78))
* **chat:** persist ai_gender in DB so gender pill survives reload ([72950e3](https://github.com/nmorgowicz-org/llama-monitor/commit/72950e3b25b3023854753b51b10390a9e86b6a78))
* **remote-agent:** restore managed agent upgrade and start flows after HTTPS mTLS hardening ([72950e3](https://github.com/nmorgowicz-org/llama-monitor/commit/72950e3b25b3023854753b51b10390a9e86b6a78))
* **settings:** centralize guided-generation settings in backend-backed settingsState ([72950e3](https://github.com/nmorgowicz-org/llama-monitor/commit/72950e3b25b3023854753b51b10390a9e86b6a78))
* **settings:** promote shared workflow prefs (enter_to_send, date_format, continuity) to shared storage ([72950e3](https://github.com/nmorgowicz-org/llama-monitor/commit/72950e3b25b3023854753b51b10390a9e86b6a78))
* **settings:** remove dead runtime controls from persistent settings panes ([72950e3](https://github.com/nmorgowicz-org/llama-monitor/commit/72950e3b25b3023854753b51b10390a9e86b6a78))
* **ui:** correct remote_agent_health_reachable always-equal-to-connected bug ([72950e3](https://github.com/nmorgowicz-org/llama-monitor/commit/72950e3b25b3023854753b51b10390a9e86b6a78))
* **ui:** correct telemetry metric grid layout to avoid forced full-width span ([72950e3](https://github.com/nmorgowicz-org/llama-monitor/commit/72950e3b25b3023854753b51b10390a9e86b6a78))

## [0.20.0](https://github.com/nmorgowicz-org/llama-monitor/compare/v0.19.1...v0.20.0) (2026-05-20)


### Features

* **metrics:** add tokens_per_decode and per-request draft acceptance to dashboard and telemetry ([6303ea3](https://github.com/nmorgowicz-org/llama-monitor/commit/6303ea3a47ef4c1c1dc4c14044cdd2c0dccb2279))


### Bug Fixes

* **chat:** preserve scroll position when loading older messages ([6303ea3](https://github.com/nmorgowicz-org/llama-monitor/commit/6303ea3a47ef4c1c1dc4c14044cdd2c0dccb2279))
* **metrics:** fix speculative.types field name in metrics parsing ([6303ea3](https://github.com/nmorgowicz-org/llama-monitor/commit/6303ea3a47ef4c1c1dc4c14044cdd2c0dccb2279))
* **notes:** fix AI review panel scroll and spelling ([6303ea3](https://github.com/nmorgowicz-org/llama-monitor/commit/6303ea3a47ef4c1c1dc4c14044cdd2c0dccb2279))

## [0.19.1](https://github.com/nmorgowicz-org/llama-monitor/compare/v0.19.0...v0.19.1) (2026-05-20)


### Bug Fixes

* **chat:** extend busy-wait timeout to 5 minutes for long inference tasks ([bb11a2e](https://github.com/nmorgowicz-org/llama-monitor/commit/bb11a2e6c4c0778b4f6b2f43ab033b8733c678e4))
* **chat:** restore hidden chat access via sidebar pill click ([bb11a2e](https://github.com/nmorgowicz-org/llama-monitor/commit/bb11a2e6c4c0778b4f6b2f43ab033b8733c678e4))
* **ui:** add active state visual feedback to management pills ([bb11a2e](https://github.com/nmorgowicz-org/llama-monitor/commit/bb11a2e6c4c0778b4f6b2f43ab033b8733c678e4))
* **ui:** correct config bar button collapse thresholds and enhance snug tier ([bb11a2e](https://github.com/nmorgowicz-org/llama-monitor/commit/bb11a2e6c4c0778b4f6b2f43ab033b8733c678e4))

## [0.19.0](https://github.com/nmorgowicz-org/llama-monitor/compare/v0.18.0...v0.19.0) (2026-05-20)


### Features

* add graceful shutdown with WAL checkpoint and final session save ([db9000e](https://github.com/nmorgowicz-org/llama-monitor/commit/db9000eaa99673d009adb1a1e7d0cfb973007a08))
* add hourly WAL checkpoint and auto backups for database maintenance ([db9000e](https://github.com/nmorgowicz-org/llama-monitor/commit/db9000eaa99673d009adb1a1e7d0cfb973007a08))
* **api:** add DB admin token auth and restricted query allowlist for /api/db/query ([db9000e](https://github.com/nmorgowicz-org/llama-monitor/commit/db9000eaa99673d009adb1a1e7d0cfb973007a08))
* **api:** secure DB backup/restore endpoints with token auth and path validation ([db9000e](https://github.com/nmorgowicz-org/llama-monitor/commit/db9000eaa99673d009adb1a1e7d0cfb973007a08))
* **auth:** add --form-auth and --clear-auth-config CLI flags for dashboard auth ([db9000e](https://github.com/nmorgowicz-org/llama-monitor/commit/db9000eaa99673d009adb1a1e7d0cfb973007a08))
* **auth:** add auth-config.json persistence and migration from startup flags ([db9000e](https://github.com/nmorgowicz-org/llama-monitor/commit/db9000eaa99673d009adb1a1e7d0cfb973007a08))
* **auth:** add Basic Auth / form login / both modes and per-route auth guard ([db9000e](https://github.com/nmorgowicz-org/llama-monitor/commit/db9000eaa99673d009adb1a1e7d0cfb973007a08))
* **auth:** add dashboard form login with HttpOnly session cookie and auth shell ([db9000e](https://github.com/nmorgowicz-org/llama-monitor/commit/db9000eaa99673d009adb1a1e7d0cfb973007a08))
* **auth:** add Security tab controls to enable/disable dashboard auth and change password ([db9000e](https://github.com/nmorgowicz-org/llama-monitor/commit/db9000eaa99673d009adb1a1e7d0cfb973007a08))
* **chat:** add chat archive and hidden visibility states with sidebar management pills ([db9000e](https://github.com/nmorgowicz-org/llama-monitor/commit/db9000eaa99673d009adb1a1e7d0cfb973007a08))
* **chat:** add Chat Sessions sidebar with recency grouping, pinning, avatars, and context menus ([db9000e](https://github.com/nmorgowicz-org/llama-monitor/commit/db9000eaa99673d009adb1a1e7d0cfb973007a08))
* **chat:** add full-text search (FTS) across all chat messages with sidebar UI and live results ([db9000e](https://github.com/nmorgowicz-org/llama-monitor/commit/db9000eaa99673d009adb1a1e7d0cfb973007a08))
* **chat:** add hidden surface with deliberate reveal flow for privacy ([db9000e](https://github.com/nmorgowicz-org/llama-monitor/commit/db9000eaa99673d009adb1a1e7d0cfb973007a08))
* **chat:** add Hide Now button in chat header for quick conversation hiding ([db9000e](https://github.com/nmorgowicz-org/llama-monitor/commit/db9000eaa99673d009adb1a1e7d0cfb973007a08))
* **chat:** add inline archive surface with restore/hide/delete actions and undo toasts ([db9000e](https://github.com/nmorgowicz-org/llama-monitor/commit/db9000eaa99673d009adb1a1e7d0cfb973007a08))
* **chat:** add lazy loading of tab messages and per-tab incremental persistence ([db9000e](https://github.com/nmorgowicz-org/llama-monitor/commit/db9000eaa99673d009adb1a1e7d0cfb973007a08))
* **chat:** scope message search to active tabs by default with Active/Archived filter chips ([db9000e](https://github.com/nmorgowicz-org/llama-monitor/commit/db9000eaa99673d009adb1a1e7d0cfb973007a08))
* **cli:** add TLS-related CLI flags (--tls, --tls-cert, --tls-key, --tls-self-signed) ([db9000e](https://github.com/nmorgowicz-org/llama-monitor/commit/db9000eaa99673d009adb1a1e7d0cfb973007a08))
* **gpu:** WMI-based GPU discovery for Intel and unknown GPUs on Windows ([497db06](https://github.com/nmorgowicz-org/llama-monitor/commit/497db06f71216acc399fed96e1e715b9497889c8))
* **security:** add ACME DNS-01 integration with auto-renewal and Certificates management in Settings ([db9000e](https://github.com/nmorgowicz-org/llama-monitor/commit/db9000eaa99673d009adb1a1e7d0cfb973007a08))
* **security:** add API token auth for sensitive endpoints (DB backup/restore, query) ([db9000e](https://github.com/nmorgowicz-org/llama-monitor/commit/db9000eaa99673d009adb1a1e7d0cfb973007a08))
* **security:** add api-token auth to previously unprotected endpoints (settings, presets, templates, LHM, sensor-bridge) ([db9000e](https://github.com/nmorgowicz-org/llama-monitor/commit/db9000eaa99673d009adb1a1e7d0cfb973007a08))
* **security:** add CSP nonce for inline scripts and DOMPurify sanitization to prevent XSS in chat ([db9000e](https://github.com/nmorgowicz-org/llama-monitor/commit/db9000eaa99673d009adb1a1e7d0cfb973007a08))
* **security:** add global Origin validation to mitigate CSRF on cookie-authenticated endpoints ([db9000e](https://github.com/nmorgowicz-org/llama-monitor/commit/db9000eaa99673d009adb1a1e7d0cfb973007a08))
* **security:** add in-place token rotation for agent, API, and DB admin tokens ([db9000e](https://github.com/nmorgowicz-org/llama-monitor/commit/db9000eaa99673d009adb1a1e7d0cfb973007a08))
* **security:** add mTLS for remote-agent with role-based trust ([db9000e](https://github.com/nmorgowicz-org/llama-monitor/commit/db9000eaa99673d009adb1a1e7d0cfb973007a08))
* **security:** add per-endpoint cooldowns for expensive operations (remote-agent, sessions, file browser, chat-search, DB, ACME) ([db9000e](https://github.com/nmorgowicz-org/llama-monitor/commit/db9000eaa99673d009adb1a1e7d0cfb973007a08))
* **security:** add remote-agent command validation to block dangerous commands ([db9000e](https://github.com/nmorgowicz-org/llama-monitor/commit/db9000eaa99673d009adb1a1e7d0cfb973007a08))
* **security:** add TLS support with self-signed, custom cert, and ACME (Let's Encrypt) modes ([db9000e](https://github.com/nmorgowicz-org/llama-monitor/commit/db9000eaa99673d009adb1a1e7d0cfb973007a08))
* **security:** add visibility-aware list and search endpoints with query param filtering ([db9000e](https://github.com/nmorgowicz-org/llama-monitor/commit/db9000eaa99673d009adb1a1e7d0cfb973007a08))
* **security:** enforce api-token auth on all chat and chat-search endpoints ([db9000e](https://github.com/nmorgowicz-org/llama-monitor/commit/db9000eaa99673d009adb1a1e7d0cfb973007a08))
* **security:** enforce api-token auth on all remote-agent endpoints ([db9000e](https://github.com/nmorgowicz-org/llama-monitor/commit/db9000eaa99673d009adb1a1e7d0cfb973007a08))
* **security:** enforce api-token auth on archive, hide, and restore endpoints ([db9000e](https://github.com/nmorgowicz-org/llama-monitor/commit/db9000eaa99673d009adb1a1e7d0cfb973007a08))
* **security:** enforce api-token auth on session CRUD and attach/detach endpoints ([db9000e](https://github.com/nmorgowicz-org/llama-monitor/commit/db9000eaa99673d009adb1a1e7d0cfb973007a08))
* **security:** enforce db-admin-token auth on remote-agent install and remove ([db9000e](https://github.com/nmorgowicz-org/llama-monitor/commit/db9000eaa99673d009adb1a1e7d0cfb973007a08))
* **security:** enforce db-admin-token auth on session delete and spawn ([db9000e](https://github.com/nmorgowicz-org/llama-monitor/commit/db9000eaa99673d009adb1a1e7d0cfb973007a08))
* **security:** mask hidden chat names from collapsed sidebar label ([db9000e](https://github.com/nmorgowicz-org/llama-monitor/commit/db9000eaa99673d009adb1a1e7d0cfb973007a08))
* **security:** mask sensitive tokens and SSH credentials in settings with show/hide toggle ([db9000e](https://github.com/nmorgowicz-org/llama-monitor/commit/db9000eaa99673d009adb1a1e7d0cfb973007a08))
* **tray:** Windows WebView popover — replaces static context menu with live metrics ([497db06](https://github.com/nmorgowicz-org/llama-monitor/commit/497db06f71216acc399fed96e1e715b9497889c8))
* **ui:** add Database Administration modal with maintenance, backups, indexes, repair, and SQL query ([db9000e](https://github.com/nmorgowicz-org/llama-monitor/commit/db9000eaa99673d009adb1a1e7d0cfb973007a08))
* **ui:** add guided-generation prompt templates and category management in Settings ([db9000e](https://github.com/nmorgowicz-org/llama-monitor/commit/db9000eaa99673d009adb1a1e7d0cfb973007a08))
* **ui:** add keyboard shortcuts panel and improved chat input buttons ([db9000e](https://github.com/nmorgowicz-org/llama-monitor/commit/db9000eaa99673d009adb1a1e7d0cfb973007a08))
* **ui:** polish Settings modal layout, sections, and controls for chat, appearance, and guided generation ([db9000e](https://github.com/nmorgowicz-org/llama-monitor/commit/db9000eaa99673d009adb1a1e7d0cfb973007a08))
* **ui:** redesign file browser modal with premium UX, parent folder button, and config browse button ([db9000e](https://github.com/nmorgowicz-org/llama-monitor/commit/db9000eaa99673d009adb1a1e7d0cfb973007a08))


### Bug Fixes

* **chat:** serialize monitor inference requests and return explicit busy/offline errors when the active llama-server is occupied ([db9000e](https://github.com/nmorgowicz-org/llama-monitor/commit/db9000eaa99673d009adb1a1e7d0cfb973007a08))
* **chat:** stabilize AI response waiting to prevent "[stopped]" captures in guided-generation flows ([db9000e](https://github.com/nmorgowicz-org/llama-monitor/commit/db9000eaa99673d009adb1a1e7d0cfb973007a08))
* resolve generic-array version conflict after dependabot updates ([50a1147](https://github.com/nmorgowicz-org/llama-monitor/commit/50a114786c22f04f7afd27744673e8f4d4eff639))
* **security:** restrict /api/browse to allowed root paths to prevent path traversal ([db9000e](https://github.com/nmorgowicz-org/llama-monitor/commit/db9000eaa99673d009adb1a1e7d0cfb973007a08))
* **ui:** resolve file browser hint clipping and stabilize related e2e tests ([db9000e](https://github.com/nmorgowicz-org/llama-monitor/commit/db9000eaa99673d009adb1a1e7d0cfb973007a08))
* **windows:** Windows ACL hardening for secret files via icacls ([497db06](https://github.com/nmorgowicz-org/llama-monitor/commit/497db06f71216acc399fed96e1e715b9497889c8))

## [0.18.0](https://github.com/nmorgowicz-org/llama-monitor/compare/v0.17.1...v0.18.0) (2026-05-14)


### Features

* **chat:** add {{gender}} token support for dynamic system prompts ([3576cc3](https://github.com/nmorgowicz-org/llama-monitor/commit/3576cc35f60806e1d98d22cfd39b74cf700f691e))
* **chat:** add advanced features (suggestion history, fix last response) ([3a78049](https://github.com/nmorgowicz-org/llama-monitor/commit/3a780495acc9a69eaceaca0964a92a0203a9dac0))
* **chat:** add adversarial prompt engineering to Coder explicit L2 policy ([3a78049](https://github.com/nmorgowicz-org/llama-monitor/commit/3a780495acc9a69eaceaca0964a92a0203a9dac0))
* **chat:** add compact confirmation modal with stats and editable summary preview ([3576cc3](https://github.com/nmorgowicz-org/llama-monitor/commit/3576cc35f60806e1d98d22cfd39b74cf700f691e))
* **chat:** add context notes AI analysis endpoint and UI ([3576cc3](https://github.com/nmorgowicz-org/llama-monitor/commit/3576cc35f60806e1d98d22cfd39b74cf700f691e))
* **chat:** add custom categories in suggestions dropdown ([3576cc3](https://github.com/nmorgowicz-org/llama-monitor/commit/3576cc35f60806e1d98d22cfd39b74cf700f691e))
* **chat:** add custom role boundary override in behavior panel ([3576cc3](https://github.com/nmorgowicz-org/llama-monitor/commit/3576cc35f60806e1d98d22cfd39b74cf700f691e))
* **chat:** add debug prompt inspector with system prompt breakdown and token counts ([3576cc3](https://github.com/nmorgowicz-org/llama-monitor/commit/3576cc35f60806e1d98d22cfd39b74cf700f691e))
* **chat:** add Director Mode category and update build doc ([3a78049](https://github.com/nmorgowicz-org/llama-monitor/commit/3a780495acc9a69eaceaca0964a92a0203a9dac0))
* **chat:** add guided generation features (context notes, suggestions, quick guide) ([3a78049](https://github.com/nmorgowicz-org/llama-monitor/commit/3a780495acc9a69eaceaca0964a92a0203a9dac0))
* **chat:** add per-persona explicit policies with independent Level 1/Level 2 text ([3576cc3](https://github.com/nmorgowicz-org/llama-monitor/commit/3576cc35f60806e1d98d22cfd39b74cf700f691e))
* **chat:** add persistent disconnected banner on connection loss ([3576cc3](https://github.com/nmorgowicz-org/llama-monitor/commit/3576cc35f60806e1d98d22cfd39b74cf700f691e))
* **chat:** add pharmacology, harm reduction, and drug policy to explicit L2 ([3a78049](https://github.com/nmorgowicz-org/llama-monitor/commit/3a780495acc9a69eaceaca0964a92a0203a9dac0))
* **chat:** add reset to default button for built-in personas ([3576cc3](https://github.com/nmorgowicz-org/llama-monitor/commit/3576cc35f60806e1d98d22cfd39b74cf700f691e))
* **chat:** add template list sections (Active/Custom/Built-in) with active badge ([3576cc3](https://github.com/nmorgowicz-org/llama-monitor/commit/3576cc35f60806e1d98d22cfd39b74cf700f691e))
* **chat:** enhance explicit L2 policies with research findings ([3a78049](https://github.com/nmorgowicz-org/llama-monitor/commit/3a780495acc9a69eaceaca0964a92a0203a9dac0))
* **chat:** explicit mode v2 — persona-aware multi-level system ([3a78049](https://github.com/nmorgowicz-org/llama-monitor/commit/3a780495acc9a69eaceaca0964a92a0203a9dac0))
* **chat:** implement Pathweaver prompts and features ([3a78049](https://github.com/nmorgowicz-org/llama-monitor/commit/3a780495acc9a69eaceaca0964a92a0203a9dac0))
* **chat:** Phase 8 tag cloud, explicit v2 tests, and drug policy enhancements ([3a78049](https://github.com/nmorgowicz-org/llama-monitor/commit/3a780495acc9a69eaceaca0964a92a0203a9dac0))
* **chat:** rewrite send direction to inject suggestion directly as user message ([3576cc3](https://github.com/nmorgowicz-org/llama-monitor/commit/3576cc35f60806e1d98d22cfd39b74cf700f691e))
* **docs:** add docs/README.md index, update README.md, chat.md, api.md, dashboard.md ([3a78049](https://github.com/nmorgowicz-org/llama-monitor/commit/3a780495acc9a69eaceaca0964a92a0203a9dac0))
* **test:** add quick-guide-revise.spec.js for Revise Last button tests ([3a78049](https://github.com/nmorgowicz-org/llama-monitor/commit/3a780495acc9a69eaceaca0964a92a0203a9dac0))
* **ui:** add focus keywords input with auto-generate button ([3576cc3](https://github.com/nmorgowicz-org/llama-monitor/commit/3576cc35f60806e1d98d22cfd39b74cf700f691e))
* **ui:** add global Escape key handler for topmost modal ([3576cc3](https://github.com/nmorgowicz-org/llama-monitor/commit/3576cc35f60806e1d98d22cfd39b74cf700f691e))
* **ui:** improve chat UI/UX — toasts, suggestions, quick guide, context notes ([3a78049](https://github.com/nmorgowicz-org/llama-monitor/commit/3a780495acc9a69eaceaca0964a92a0203a9dac0))


### Bug Fixes

* **api:** standardize on snake_case for token fields to avoid duplicate field errors ([3576cc3](https://github.com/nmorgowicz-org/llama-monitor/commit/3576cc35f60806e1d98d22cfd39b74cf700f691e))
* **chat:** add alias for active_template_id to accept snake_case from frontend ([3576cc3](https://github.com/nmorgowicz-org/llama-monitor/commit/3576cc35f60806e1d98d22cfd39b74cf700f691e))
* **chat:** add auto_compact_summarize and compact_mode fields to ChatTab ([3576cc3](https://github.com/nmorgowicz-org/llama-monitor/commit/3576cc35f60806e1d98d22cfd39b74cf700f691e))
* **chat:** add periodic save every 30s to prevent data loss on force-kill ([3576cc3](https://github.com/nmorgowicz-org/llama-monitor/commit/3576cc35f60806e1d98d22cfd39b74cf700f691e))
* **chat:** add settings controls for guided generation features ([3a78049](https://github.com/nmorgowicz-org/llama-monitor/commit/3a780495acc9a69eaceaca0964a92a0203a9dac0))
* **chat:** correct import paths and function names in guided generation modules ([3a78049](https://github.com/nmorgowicz-org/llama-monitor/commit/3a780495acc9a69eaceaca0964a92a0203a9dac0))
* **chat:** delay modal close until view transition completes ([3a78049](https://github.com/nmorgowicz-org/llama-monitor/commit/3a780495acc9a69eaceaca0964a92a0203a9dac0))
* **chat:** fix 5 clippy errors (collapsible_str_replace, invalid_regex, redundant_closure) ([3a78049](https://github.com/nmorgowicz-org/llama-monitor/commit/3a780495acc9a69eaceaca0964a92a0203a9dac0))
* **chat:** fix e2e tests for suggestions send mode, settings success class, tag cloud UI refresh ([3a78049](https://github.com/nmorgowicz-org/llama-monitor/commit/3a780495acc9a69eaceaca0964a92a0203a9dac0))
* **chat:** fix reset button visibility and always show per-persona explicit policies ([3576cc3](https://github.com/nmorgowicz-org/llama-monitor/commit/3576cc35f60806e1d98d22cfd39b74cf700f691e))
* **chat:** handle streaming SSE response for auto-generate focus keywords ([3576cc3](https://github.com/nmorgowicz-org/llama-monitor/commit/3576cc35f60806e1d98d22cfd39b74cf700f691e))
* **chat:** harden explicit mode toggle against null/undefined values ([3a78049](https://github.com/nmorgowicz-org/llama-monitor/commit/3a780495acc9a69eaceaca0964a92a0203a9dac0))
* **chat:** improve chat message coloring and load older messages UX ([3576cc3](https://github.com/nmorgowicz-org/llama-monitor/commit/3576cc35f60806e1d98d22cfd39b74cf700f691e))
* **chat:** include explicit_policies in merged built-in templates ([3576cc3](https://github.com/nmorgowicz-org/llama-monitor/commit/3576cc35f60806e1d98d22cfd39b74cf700f691e))
* **chat:** move reset button to right panel next to edit and apply ([3576cc3](https://github.com/nmorgowicz-org/llama-monitor/commit/3576cc35f60806e1d98d22cfd39b74cf700f691e))
* **chat:** persist explicit mode level on tabs ([3576cc3](https://github.com/nmorgowicz-org/llama-monitor/commit/3576cc35f60806e1d98d22cfd39b74cf700f691e))
* **chat:** re-render tab bar after explicit mode toggle ([3a78049](https://github.com/nmorgowicz-org/llama-monitor/commit/3a780495acc9a69eaceaca0964a92a0203a9dac0))
* **chat:** remove 'Format each as:...' from custom category prompt ([3576cc3](https://github.com/nmorgowicz-org/llama-monitor/commit/3576cc35f60806e1d98d22cfd39b74cf700f691e))
* **chat:** remove recent suggestions section and fix send direction rewrite ([3576cc3](https://github.com/nmorgowicz-org/llama-monitor/commit/3576cc35f60806e1d98d22cfd39b74cf700f691e))
* **chat:** restore guided generation buttons and fix clipped dropdowns ([3a78049](https://github.com/nmorgowicz-org/llama-monitor/commit/3a780495acc9a69eaceaca0964a92a0203a9dac0))
* **chat:** restore guided generation flow and unify screenshot capture ([3a78049](https://github.com/nmorgowicz-org/llama-monitor/commit/3a780495acc9a69eaceaca0964a92a0203a9dac0))
* **chat:** restore manage categories popup layout (built-in left, custom right) ([3576cc3](https://github.com/nmorgowicz-org/llama-monitor/commit/3576cc35f60806e1d98d22cfd39b74cf700f691e))
* **chat:** set auto_compact_summarize to true by default for all chats ([3576cc3](https://github.com/nmorgowicz-org/llama-monitor/commit/3576cc35f60806e1d98d22cfd39b74cf700f691e))
* **chat:** stabilize guided reply and rolling memory flows ([3a78049](https://github.com/nmorgowicz-org/llama-monitor/commit/3a780495acc9a69eaceaca0964a92a0203a9dac0))
* **chat:** standardize on camelCase for explicitLevel and activeTemplateId ([3576cc3](https://github.com/nmorgowicz-org/llama-monitor/commit/3576cc35f60806e1d98d22cfd39b74cf700f691e))
* **chat:** update persona menu name on tab load and switch ([3576cc3](https://github.com/nmorgowicz-org/llama-monitor/commit/3576cc35f60806e1d98d22cfd39b74cf700f691e))
* **chat:** use /api/chat/suggestions endpoint for auto-generate focus keywords ([3576cc3](https://github.com/nmorgowicz-org/llama-monitor/commit/3576cc35f60806e1d98d22cfd39b74cf700f691e))
* **chat:** use correct thinking disable params for auto-generate focus call ([3576cc3](https://github.com/nmorgowicz-org/llama-monitor/commit/3576cc35f60806e1d98d22cfd39b74cf700f691e))
* **dashboard:** cap slot grid height to prevent card blowout at high parallelism ([3576cc3](https://github.com/nmorgowicz-org/llama-monitor/commit/3576cc35f60806e1d98d22cfd39b74cf700f691e))
* **ui:** add CSS variable aliases and remove duplicate selectors ([3a78049](https://github.com/nmorgowicz-org/llama-monitor/commit/3a780495acc9a69eaceaca0964a92a0203a9dac0))
* **ui:** fill card height and differentiate context window views ([3a78049](https://github.com/nmorgowicz-org/llama-monitor/commit/3a780495acc9a69eaceaca0964a92a0203a9dac0))
* **ui:** fix context notes sidebar, suggestions/quick guide buttons ([3a78049](https://github.com/nmorgowicz-org/llama-monitor/commit/3a780495acc9a69eaceaca0964a92a0203a9dac0))
* **ui:** fix context notes sidebar, tab badge, suggestions prompt ([3a78049](https://github.com/nmorgowicz-org/llama-monitor/commit/3a780495acc9a69eaceaca0964a92a0203a9dac0))
* **ui:** improve custom categories sizing and styling ([3576cc3](https://github.com/nmorgowicz-org/llama-monitor/commit/3576cc35f60806e1d98d22cfd39b74cf700f691e))
* **ui:** make custom and built-in lists share height equally (50/50 split) ([3576cc3](https://github.com/nmorgowicz-org/llama-monitor/commit/3576cc35f60806e1d98d22cfd39b74cf700f691e))
* **ui:** redesign context window card gauge and fleet views ([3a78049](https://github.com/nmorgowicz-org/llama-monitor/commit/3a780495acc9a69eaceaca0964a92a0203a9dac0))
* **ui:** reorganize manage categories modal with proper two-column layout ([3576cc3](https://github.com/nmorgowicz-org/llama-monitor/commit/3576cc35f60806e1d98d22cfd39b74cf700f691e))
* **ui:** wrap custom category buttons in chips container for proper layout ([3576cc3](https://github.com/nmorgowicz-org/llama-monitor/commit/3576cc35f60806e1d98d22cfd39b74cf700f691e))

## [0.17.1](https://github.com/nmorgowicz-org/llama-monitor/compare/v0.17.0...v0.17.1) (2026-05-08)


### Bug Fixes

* **agent:** preserve quick upgrade button in update indicator ([47768f1](https://github.com/nmorgowicz-org/llama-monitor/commit/47768f1375eb93eb49bcb1e2507d83fff348be48))
* **chat:** add real-time token counter to thinking block header ([49c9898](https://github.com/nmorgowicz-org/llama-monitor/commit/49c98981a2d47e9601bead370d79eee55bbcb7a5))
* **chat:** add resend button to user messages for quick retry ([49c9898](https://github.com/nmorgowicz-org/llama-monitor/commit/49c98981a2d47e9601bead370d79eee55bbcb7a5))
* **chat:** allow inline edit save during AI generation ([9ad98c2](https://github.com/nmorgowicz-org/llama-monitor/commit/9ad98c2454d1c33f577feff3ed7aeaa480682e83))
* **chat:** allow regeneration from last variant in navigation ([9ad98c2](https://github.com/nmorgowicz-org/llama-monitor/commit/9ad98c2454d1c33f577feff3ed7aeaa480682e83))
* **chat:** disable auto-scroll when user scrolls up during generation ([49c9898](https://github.com/nmorgowicz-org/llama-monitor/commit/49c98981a2d47e9601bead370d79eee55bbcb7a5))
* **chat:** ensure auto-scroll resumes after sending message ([9ad98c2](https://github.com/nmorgowicz-org/llama-monitor/commit/9ad98c2454d1c33f577feff3ed7aeaa480682e83))
* **chat:** move timeout setting to main panel, improve toast duration ([49c9898](https://github.com/nmorgowicz-org/llama-monitor/commit/49c98981a2d47e9601bead370d79eee55bbcb7a5))
* **chat:** persist input resize to settings instead of localStorage ([9ad98c2](https://github.com/nmorgowicz-org/llama-monitor/commit/9ad98c2454d1c33f577feff3ed7aeaa480682e83))
* **chat:** prevent scroll force and DOM wipe during streaming ([9ad98c2](https://github.com/nmorgowicz-org/llama-monitor/commit/9ad98c2454d1c33f577feff3ed7aeaa480682e83))
* **chat:** reorder persona menu with active at top, add edit buttons ([49c9898](https://github.com/nmorgowicz-org/llama-monitor/commit/49c98981a2d47e9601bead370d79eee55bbcb7a5))
* **chat:** restore user position after reconnecting to server ([49c9898](https://github.com/nmorgowicz-org/llama-monitor/commit/49c98981a2d47e9601bead370d79eee55bbcb7a5))
* **chat:** scroll to thinking block when it appears during generation ([49c9898](https://github.com/nmorgowicz-org/llama-monitor/commit/49c98981a2d47e9601bead370d79eee55bbcb7a5))
* **chat:** show connection lost modal on all errors, not just regenerate ([49c9898](https://github.com/nmorgowicz-org/llama-monitor/commit/49c98981a2d47e9601bead370d79eee55bbcb7a5))
* **chat:** update erotic storyteller system prompt ([49c9898](https://github.com/nmorgowicz-org/llama-monitor/commit/49c98981a2d47e9601bead370d79eee55bbcb7a5))
* **ui:** fix connection lost modal spacing ([49c9898](https://github.com/nmorgowicz-org/llama-monitor/commit/49c98981a2d47e9601bead370d79eee55bbcb7a5))
* **ui:** remove infinite animations from settings modal fields ([47768f1](https://github.com/nmorgowicz-org/llama-monitor/commit/47768f1375eb93eb49bcb1e2507d83fff348be48))
* **ui:** restore min-height to allow content expansion ([9ad98c2](https://github.com/nmorgowicz-org/llama-monitor/commit/9ad98c2454d1c33f577feff3ed7aeaa480682e83))

## [0.17.0](https://github.com/nmorgowicz-org/llama-monitor/compare/v0.16.0...v0.17.0) (2026-05-07)


### Features

* **agent:** add quick upgrade button to remote agent update indicator ([27618bf](https://github.com/nmorgowicz-org/llama-monitor/commit/27618bf3ad70bc0355446ca4efd9c7e25548b6db))
* **ui:** add breathing glow to chat telemetry trigger button ([27618bf](https://github.com/nmorgowicz-org/llama-monitor/commit/27618bf3ad70bc0355446ca4efd9c7e25548b6db))


### Bug Fixes

* **agent:** normalize version comparison to strip v prefix from GitHub tag ([27618bf](https://github.com/nmorgowicz-org/llama-monitor/commit/27618bf3ad70bc0355446ca4efd9c7e25548b6db))
* **ui:** pause infinite animations on hover instead of stopping them ([27618bf](https://github.com/nmorgowicz-org/llama-monitor/commit/27618bf3ad70bc0355446ca4efd9c7e25548b6db))

## [0.16.0](https://github.com/nmorgowicz-org/llama-monitor/compare/v0.15.0...v0.16.0) (2026-05-07)


### Features

* add automatic network quality detection with polling adjustment ([b725c81](https://github.com/nmorgowicz-org/llama-monitor/commit/b725c81a1c85315fe068d90a09f9fff06377e972))
* add configurable dashboard WebSocket refresh rate (200ms-10s) ([b725c81](https://github.com/nmorgowicz-org/llama-monitor/commit/b725c81a1c85315fe068d90a09f9fff06377e972))
* **agent:** add remote agent upgrade flow with host key and OS detection ([b725c81](https://github.com/nmorgowicz-org/llama-monitor/commit/b725c81a1c85315fe068d90a09f9fff06377e972))
* **agent:** add remote agent version tracking and update detection ([b725c81](https://github.com/nmorgowicz-org/llama-monitor/commit/b725c81a1c85315fe068d90a09f9fff06377e972))
* **ui:** add chat telemetry popover with pin-to-inline toggle, throughput bars, context ring, and activity rail ([b725c81](https://github.com/nmorgowicz-org/llama-monitor/commit/b725c81a1c85315fe068d90a09f9fff06377e972))
* **ui:** add nav cockpit with live inference state, throughput, context pressure, GPU temp, and sparkline ([b725c81](https://github.com/nmorgowicz-org/llama-monitor/commit/b725c81a1c85315fe068d90a09f9fff06377e972))
* **ui:** add Performance settings tab with refresh rate and network indicator ([b725c81](https://github.com/nmorgowicz-org/llama-monitor/commit/b725c81a1c85315fe068d90a09f9fff06377e972))
* **ui:** elevate dashboard with ambient gradient orbs, typography hierarchy, and gap token standardization ([b725c81](https://github.com/nmorgowicz-org/llama-monitor/commit/b725c81a1c85315fe068d90a09f9fff06377e972))
* **ui:** refresh agent and settings modal styling with widget-card treatment ([b725c81](https://github.com/nmorgowicz-org/llama-monitor/commit/b725c81a1c85315fe068d90a09f9fff06377e972))


### Bug Fixes

* **agent:** log update available message only once on state transition ([b725c81](https://github.com/nmorgowicz-org/llama-monitor/commit/b725c81a1c85315fe068d90a09f9fff06377e972))
* **agent:** only check GitHub releases once per session to prevent rate limit errors ([b725c81](https://github.com/nmorgowicz-org/llama-monitor/commit/b725c81a1c85315fe068d90a09f9fff06377e972))
* **chat:** align chat tabs and accents with dashboard styling ([b725c81](https://github.com/nmorgowicz-org/llama-monitor/commit/b725c81a1c85315fe068d90a09f9fff06377e972))
* **ui:** add prefers-reduced-motion overrides for all dashboard animations ([b725c81](https://github.com/nmorgowicz-org/llama-monitor/commit/b725c81a1c85315fe068d90a09f9fff06377e972))
* **ui:** convert hardcoded Nord colors to CSS variables with light theme coverage ([b725c81](https://github.com/nmorgowicz-org/llama-monitor/commit/b725c81a1c85315fe068d90a09f9fff06377e972))
* **ui:** correct GPU core clock color mapping across clock visuals ([b725c81](https://github.com/nmorgowicz-org/llama-monitor/commit/b725c81a1c85315fe068d90a09f9fff06377e972))
* **ui:** fix spawn local server button on logs empty state ([b725c81](https://github.com/nmorgowicz-org/llama-monitor/commit/b725c81a1c85315fe068d90a09f9fff06377e972))
* **ui:** improve sparkline visibility and stabilize validation captures ([b725c81](https://github.com/nmorgowicz-org/llama-monitor/commit/b725c81a1c85315fe068d90a09f9fff06377e972))
* **ui:** reserve warning styling for health states instead of normal utilization spikes ([b725c81](https://github.com/nmorgowicz-org/llama-monitor/commit/b725c81a1c85315fe068d90a09f9fff06377e972))
* **ui:** standardize dashboard metric cards around a shared surface palette ([b725c81](https://github.com/nmorgowicz-org/llama-monitor/commit/b725c81a1c85315fe068d90a09f9fff06377e972))

## [0.15.0](https://github.com/nmorgowicz-org/llama-monitor/compare/v0.14.0...v0.15.0) (2026-05-05)


### Features

* **agent:** auto-save remote agent token on install and start ([790d673](https://github.com/nmorgowicz-org/llama-monitor/commit/790d6739f6f2b28836cbacda948ce2e72e93a3e2))
* **chat:** add chat tab pinning, drag-to-reorder, persona/template menus, export/import flows, and edit/regenerate ([790d673](https://github.com/nmorgowicz-org/llama-monitor/commit/790d6739f6f2b28836cbacda948ce2e72e93a3e2))
* **chat:** add message timestamps with dates and SillyTavern integration link ([790d673](https://github.com/nmorgowicz-org/llama-monitor/commit/790d6739f6f2b28836cbacda948ce2e72e93a3e2))
* **chat:** add timeout adjustment actions and retry/dismiss recovery for chat failures ([790d673](https://github.com/nmorgowicz-org/llama-monitor/commit/790d6739f6f2b28836cbacda948ce2e72e93a3e2))
* **chat:** improve compaction with dynamic budgets, auto-compact post-response trigger, overflow guard, and structured summaries ([790d673](https://github.com/nmorgowicz-org/llama-monitor/commit/790d6739f6f2b28836cbacda948ce2e72e93a3e2))
* **ui:** add model metadata (param count, trained context) to decoding config card ([790d673](https://github.com/nmorgowicz-org/llama-monitor/commit/790d6739f6f2b28836cbacda948ce2e72e93a3e2))
* **ui:** add remote logs empty-state messaging and refresh README screenshots ([790d673](https://github.com/nmorgowicz-org/llama-monitor/commit/790d6739f6f2b28836cbacda948ce2e72e93a3e2))
* **ui:** redesign context window card with gauge/fleet views, chat-derived pressure, and most-recent-chat gauge ([790d673](https://github.com/nmorgowicz-org/llama-monitor/commit/790d6739f6f2b28836cbacda948ce2e72e93a3e2))
* **ui:** refresh dashboard cards, hardware metrics, sparkline indicators, navigation, and status treatments ([790d673](https://github.com/nmorgowicz-org/llama-monitor/commit/790d6739f6f2b28836cbacda948ce2e72e93a3e2))
* **ui:** replace agent dropdown menu with hover status tooltip ([790d673](https://github.com/nmorgowicz-org/llama-monitor/commit/790d6739f6f2b28836cbacda948ce2e72e93a3e2))


### Bug Fixes

* **agent:** write remote agent tokens to user home dirs and preserve unrelated settings on setup finish ([790d673](https://github.com/nmorgowicz-org/llama-monitor/commit/790d6739f6f2b28836cbacda948ce2e72e93a3e2))
* **api:** rename chat tab fields to camelCase to prevent duplicate-field panic ([790d673](https://github.com/nmorgowicz-org/llama-monitor/commit/790d6739f6f2b28836cbacda948ce2e72e93a3e2))
* **chat:** fix nav arrows visibility, settings panel opening, textarea auto-size, and unread message badge ([790d673](https://github.com/nmorgowicz-org/llama-monitor/commit/790d6739f6f2b28836cbacda948ce2e72e93a3e2))
* **chat:** preserve persona state, fix resend/edit flows, timeout rollback, and full-width edit layouts ([790d673](https://github.com/nmorgowicz-org/llama-monitor/commit/790d6739f6f2b28836cbacda948ce2e72e93a3e2))
* **security:** implement DOMPurify XSS sanitization and escape HTML in unsafe render paths ([790d673](https://github.com/nmorgowicz-org/llama-monitor/commit/790d6739f6f2b28836cbacda948ce2e72e93a3e2))
* **ui:** correct context usage calculations, persist pressure, and improve derived-context fallbacks ([790d673](https://github.com/nmorgowicz-org/llama-monitor/commit/790d6739f6f2b28836cbacda948ce2e72e93a3e2))
* **ui:** format GPU SCLK values, fix clock card spacing, and tighten sidebar layout ([790d673](https://github.com/nmorgowicz-org/llama-monitor/commit/790d6739f6f2b28836cbacda948ce2e72e93a3e2))
* **ui:** resolve modal navigation, export menu bugs, and visual regressions after ES module refactor ([790d673](https://github.com/nmorgowicz-org/llama-monitor/commit/790d6739f6f2b28836cbacda948ce2e72e93a3e2))
* **ui:** restore endpoint status, active button states, scroll button position, and hide empty sidebar badge ([790d673](https://github.com/nmorgowicz-org/llama-monitor/commit/790d6739f6f2b28836cbacda948ce2e72e93a3e2))

## [0.14.0](https://github.com/nmorgowicz-org/llama-monitor/compare/v0.13.0...v0.14.0) (2026-05-02)


### Features

* **ui:** remove the legacy window facade and delete app.js startup wiring ([beee69e](https://github.com/nmorgowicz-org/llama-monitor/commit/beee69e3044296e1f78b82f577e95f6e63f9a4a3))


### Bug Fixes

* **ui:** improve modal interaction and accessibility for settings, models, and remote-agent flows ([beee69e](https://github.com/nmorgowicz-org/llama-monitor/commit/beee69e3044296e1f78b82f577e95f6e63f9a4a3))


### Performance Improvements

* **ui:** defer non-critical frontend modules to reduce startup work ([beee69e](https://github.com/nmorgowicz-org/llama-monitor/commit/beee69e3044296e1f78b82f577e95f6e63f9a4a3))
* **ui:** optimize frontend bootstrap and rendering hot paths ([beee69e](https://github.com/nmorgowicz-org/llama-monitor/commit/beee69e3044296e1f78b82f577e95f6e63f9a4a3))

## [0.13.0](https://github.com/nmorgowicz-org/llama-monitor/compare/v0.12.0...v0.13.0) (2026-05-01)


### Features

* **ui:** refactor monolithic app.js into 22 ES modules ([005fe0f](https://github.com/nmorgowicz-org/llama-monitor/commit/005fe0f983bbcf4f9c588d53cae57ba889cbdf94))


### Bug Fixes

* **chat:** default auto-compaction on restored tabs ([005fe0f](https://github.com/nmorgowicz-org/llama-monitor/commit/005fe0f983bbcf4f9c588d53cae57ba889cbdf94))

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
