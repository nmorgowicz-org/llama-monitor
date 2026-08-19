# Browser storage inventory

Captured before any rebrand edits. All existing keys are compatibility
identifiers for 2.x; no key is renamed in the 2.0 implementation.

## Direct literal keys

`csp-collapsed`, `llama-monitor-chat-font`, `llama-monitor-chat-style`,
`llama-monitor-chat-telemetry-pinned`, `llama-monitor-date-format`,
`llama-monitor-enter-to-send`, `llama-monitor-gpu-viz`,
`llama-monitor-group-by-family`, `llama-monitor-last-endpoint`,
`llama-monitor-last-session`, `llama-monitor-preferences`,
`llama-monitor-preset-sort`, `llama-monitor-previous-position`,
`llama-monitor-system-viz`, `llama_monitor_context_notes_intro_hidden`,
`llama_monitor_sidebar_expanded`, `sidebarCollapsed`,
`spawn_wizard_tips_collapsed`, `suggestions_custom_categories`,
`update-dismissed`, and `wizard_view_mode`.

## Named constants requiring module-initialization review

- `llama_monitor_sidebar_width` (`chat-notes.js`)
- `llama-monitor-notifications` (`toast.js`)
- `appNavWidth` (`nav.js`)
- `llama-monitor-chat-focus-mode` (`chat-focus-mode.js`)
- `llama-monitor-log-font-size`, `llama-monitor-log-tail-enabled`,
  `llama-monitor-log-tail-lines` (`dashboard-ws.js`)
- `llama-monitor-models-prefs` (`models.js`)
- `template_autoupdater_lastCheck`, `template_autoupdater_busy`,
  `template_autoupdater_lastStatus` (`template-autoupdater.js`)

`sessionStorage` includes `wizard_view_mode`; the bootstrap order must be
tested before any future migration helper is introduced.
