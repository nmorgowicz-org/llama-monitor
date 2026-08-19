# External client profiles

Llama Monitor exposes authenticated OpenAI-compatible routes for external
clients. Configure each client with the Llama Monitor base URL, an `api-token`
in the `Authorization: Bearer …` header, and the model name reported by
`GET /api/models`. Keep the client’s streaming and tool-call settings enabled;
the server preserves explicit request fields and only supplies omission-only
defaults.

## OpenCode

Use the OpenAI-compatible chat-completions endpoint for streaming coding turns
and tool calls. Point the provider at the Llama Monitor base URL, configure the
API token, and leave sampling values omitted unless the OpenCode profile owns
them. MCP/tool ordering and tool execution remain client-owned; do not enable
the llama-server Web UI MCP proxy just to serve OpenCode.

## Hermes and OpenClaw

Use the same authenticated OpenAI-compatible endpoint for scheduled or
delegated tool turns. Keep concurrency and context limits explicit in the
client, because the normal server policy is one foreground generation and
queued background work. Their tool/MCP payloads and retry behavior remain
client-owned; inspect the server’s requested/effective diagnostics when a
runtime downgrades a setting.

## SillyTavern

Choose the client mode deliberately:

- Chat Completions sends structured messages; Llama Monitor applies the selected
  backend chat template and returns an OpenAI-compatible stream.
- Text Completions sends a fully rendered prompt to the raw completion route;
  SillyTavern owns instruct formatting, persona, lore/world-info, and prompt
  injection. Do not also apply a server chat template to that request.

The qualified llama.cpp raw routes are `/completion` and legacy
`/v1/completions`. Use the OpenAI-compatible `/v1/completions` contract for
Rapid-MLX where supported. Tool-call template changes are revision-pinned and
candidate discussion fixes are smoke-tested before activation; Version History
activation is an explicit rollback of an installed release.

## Troubleshooting

401/403 responses mean the API token is missing or invalid. A successful
request with `state: unavailable`, `degraded`, or `incompatible` diagnostics is
an honest capability/runtime result, not permission to guess a model property.
For draft/MTP-head artifacts, filename or repository hints may identify a
provisional candidate when those files expose no introspectable head metadata;
the UI labels that evidence inferred and never treats it as confirmed depth.
