# TLS / ACME / mTLS Implementation Plan

Date: 2026-05-16  
Scope:  
- Add optional TLS to the main web UI.  
- Add Let’s Encrypt / ACME integration with DNS-01 challenge (Namecheap first-class).  
- Extend existing mTLS infrastructure for remote-agent.  
- Provide a simple, non-opinionated UX that still satisfies the security audit.

---

## 1. Goals

- Non-intrusive:
  - TLS is fully optional.
  - HTTP remains the default for local-first use.
- Flexible:
  - Support:
    - No TLS (HTTP).
    - Self-signed TLS.
    - User-provided TLS (e.g., from Traefik, internal CA, external ACME).
    - ACME-managed TLS via Let’s Encrypt (DNS-01).
- Secure:
  - Encrypt traffic when exposed (0.0.0.0 or reverse proxy).
  - Add mTLS for remote-agent ↔ main instance.
- Simple:
  - Single, clear “Certificates” flow in Settings.
  - Guided steps for ACME; no need to read ACME docs.
  - Staging mode for safe testing; production when ready.
- Backward-compatible:
  - Existing users and scripts continue to work.
  - No breaking changes to current CLI flags or configs.

This design directly addresses:
- Security audit finding #6 (Basic Auth + no TLS).
- Need for a practical, user-respected TLS model.

---

## 2. High-Level Architecture

2.1 Core components

- Web server (Rust):
  - HTTP or HTTPS listener.
  - Uses TLSConfig to decide:
    - Whether to enable TLS.
    - Which certificate and key to use.
- Cert management layer:
  - Wraps:
    - Self-signed cert generation.
    - Loading user-provided certs.
    - ACME integration (via lego) for Let’s Encrypt.
- ACME integration:
  - Uses lego (Go ACME client) as subprocess:
    - DNS-01 challenge.
    - Namecheap DNS provider.
    - Staging vs production endpoints.
- mTLS for remote-agent:
  - Reuses existing certs.rs infrastructure.
  - Main instance:
    - Serves TLS (ACME, custom, or self-signed).
    - Expects client certs from remote-agents.
  - Remote-agent:
    - Uses its client cert to authenticate.

2.2 ASCII architecture

- User (browser)
    |
    | HTTP or HTTPS
    v
+-----------------+
| llama-monitor   |
| (main instance) |
| - UI + API      |
| - TLS listener  |
| - mTLS CA for   |
|   remote-agent  |
+-----------------+
    |
    | mTLS (client cert required)
    v
+-----------------+
| remote-agent    |
| (on remote host)|
+-----------------+

- ACME interaction (for Let’s Encrypt):

+-----------------+        ACME (HTTPS)        +-----------------+
| llama-monitor   | <========================> | Let’s Encrypt   |
| - ACME client   |                           | (LE/ZeroSSL)    |
+-----------------+                           +-----------------+
      |
      | DNS-01 TXT via provider API
      v
+-----------------+
| DNS provider    |
| (Namecheap, etc)|
+-----------------+

All ACME logic is fully internal to llama-monitor; users only configure via UI.

---

## 3. TLS Modes

We define four mutually exclusive modes:

- NONE:
  - HTTP only.
  - Default for backward compatibility.
  - Suitable for:
    - Local-only (127.0.0.1).
    - Trusted LAN where user explicitly opts-in.

- SELF_SIGNED:
  - Auto-generate a TLS certificate and key.
  - Suitable for:
    - Internal use.
    - Quick encryption without external CA.
  - Browser will show a warning unless user trusts the cert.

- CUSTOM:
  - User provides:
    - cert_path
    - key_path
  - Suitable for:
    - Certs managed externally (Traefik, internal CA, external ACME).
    - Users with existing PKI.

- ACME:
  - Llama-monitor manages:
    - ACME account.
    - Certificate requests.
    - DNS-01 challenge.
    - Renewals.
  - Suitable for:
    - Users who want Let’s Encrypt (or similar) without external tooling.

Constraints:
- Only one mode active at a time.
- Switching modes is safe:
  - Reload TLS listener (graceful).
  - Preserve existing configs; allow rollback.

---

## 4. ACME Design

4.1 Why lego?

- Pros:
  - Single binary, stable.
  - Native Namecheap provider.
  - Clean, scriptable.
  - Already used by many tools (Traefik, pfSense, etc.).
- Cons:
  - We must manage:
    - Invocation.
    - Environment variables.
    - Cert storage.
- Verdict:
  - Best fit for us: predictable, embeddable, no shell weirdness.

4.2 ACME flow (DNS-01, Namecheap)

Steps (from user perspective, high-level):

1) User enables ACME in Settings.
2) User chooses:
   - FQDN (e.g., llama-monitor.example.com).
   - DNS provider: Namecheap.
   - Namecheap credentials:
     - Username
     - API key
     - (Optional) Source IP
   - Environment:
     - Staging (for testing).
     - Production (for real certs).
3) Llama-monitor:
   - Validates inputs.
   - Calls lego to:
     - Register/update ACME account.
     - Request cert for FQDN.
     - Use DNS-01 challenge.
   - lego:
     - Adds TXT record via Namecheap API.
     - Waits for propagation.
     - Lets Encrypt validates.
   - On success:
     - Store cert and key.
     - Switch TLS to use new cert.
4) Renewal:
   - Background job checks expiry.
   - If within renewal window (e.g., 30 days):
     - Repeat same ACME flow.

4.3 Example: Namecheap DNS config (conceptual)

User provides (in UI):

- FQDN: llama-monitor.example.com
- DNS provider: Namecheap
- Environment: Staging (first), then Production
- Validation delay: 300 (seconds)
- Namecheap fields:
  - NAMECHEAP_USERNAME
  - NAMECHEAP_API_KEY
  - NAMECHEAP_SOURCEIP

Internally:
- We map these into lego environment variables.
- We never hardcode real values; we store them securely in config.

Example internal mapping (conceptual, not literal):

- NAMECHEAP_USERNAME → env for lego.
- NAMECHEAP_API_KEY → env for lego.
- NAMECHEAP_SOURCEIP → env for lego.
- DNS propagation delay → lego’s propagation-check config.

We will:
- Show helper text explaining:
  - “Use Staging first to test your DNS/API setup without hitting rate limits.”

4.4 ACME data model (in config)

In ui-settings.json (or a dedicated tls.json), we store:

{
  "tls": {
    "mode": "acme",
    "acme": {
      "enabled": true,
      "fqdn": "llama-monitor.example.com",
      "environment": "production",
      "dns_provider": "namecheap",
      "dns_config": {
        "username": "<from user>",
        "api_key": "<from user>",
        "source_ip": "<optional>"
      },
      "validation_delay": 300,
      "last_renewal": "2026-01-01T00:00:00Z",
      "cert_path": "/path/to/cert.pem",
      "key_path": "/path/to/key.pem"
    }
  }
}

Security:
- API_KEY is sensitive:
  - Stored in config directory.
  - Not logged.
  - Masked in UI.

---

## 5. mTLS for remote-agent

We keep this separate from browser TLS, but use the same underlying TLS infrastructure.

5.1 Goals

- Ensure:
  - Encrypted traffic between main instance and remote-agent.
  - Strong authentication: only known agents can connect.
  - Resistance to spoofing on the LAN.

5.2 Flow

- Main instance:
  - Generates:
    - Root CA.
    - Server certificate.
    - Agent client certificate(s).
  - Configures:
    - TLS listener with server cert.
    - mTLS: require client cert for agent endpoints.

- Remote-agent:
  - Uses:
    - Its client certificate.
    - Root CA to trust the server.
  - Connects back via:
    - mTLS (HTTPS with client auth).

5.3 ASCII

+-----------------+
| llama-monitor   |
| (main instance) |
| - server cert   |
| - CA for agents |
+-----------------+
      ^
      | mTLS (agent client cert)
      |
+-----------------+
| remote-agent    |
| (remote host)   |
+-----------------+

This is already partially implemented in certs.rs; we’ll:
- Formalize it.
- Ensure it composes with ACME/custom TLS for the main UI.

---

## 6. CLI and Config

6.1 Backward-compatible behavior

- If no TLS config:
  - Mode = NONE.
  - HTTP only.
- Existing flags:
  - --host, --port, --basic-auth remain unchanged.
- New flags (optional, for advanced users):
  - --tls
  - --tls-cert
  - --tls-key
  - --tls-self-signed
  - (ACME-specific flags can exist, but primary config is via UI.)

6.2 Data persistence

- Store TLS configuration in:
  - ~/.config/llama-monitor/tls-config.json
- Structure:
  - mode
  - acme config
  - custom cert paths
- Keep it simple and human-editable.

---

## 7. UX: Certificates Modal

We want a flow that is:
- Clear.
- Non-intimidating.
- Self-documenting.
- Better than Proxmox’s current UX.

7.1 Location

- Settings → “Certificates” tab (or section).
- Visible to all users; optional.

7.2 Layout

Top: Status

- “TLS Status:”
  - “Disabled (HTTP only)”
  - “Enabled (Self-signed)”
  - “Enabled (Custom certificate)”
  - “Enabled (Let’s Encrypt – Production)”
- If enabled:
  - Show:
    - Issuer
    - Expiry date
    - Domain(s)
- If 0.0.0.0 is bound and TLS is disabled:
  - Show warning:
    - “Listening on all interfaces without TLS.
       Anyone on your network can access the UI in cleartext.”

Section 1: No TLS

- Label: “HTTP only (no TLS)”
- Button: “Disable TLS”
- Short text:
  - “Recommended for local-only use (127.0.0.1).”

Section 2: Self-signed

- Label: “Self-signed certificate”
- Button: “Generate self-signed certificate”
- Short text:
  - “Useful for internal use. Browser will show a warning unless you trust the certificate.”

Section 3: Let’s Encrypt (ACME)

- Label: “Let’s Encrypt (ACME) – automated certificates”
- Fields:
  - “Domain (FQDN)”
    - Example: llama-monitor.example.com
  - “DNS provider”
    - Dropdown: Namecheap (initially).
  - “Environment”
    - Radio:
      - “Staging (test)”
      - “Production (real)”
    - Helper:
      - “Use Staging first to test your DNS/API setup without hitting rate limits.”
  - “Validation delay (seconds)”
    - Default: 300.
    - Helper:
      - “Time to wait for DNS propagation before validation.”
  - “DNS provider credentials”
    - For Namecheap:
      - “Username”
      - “API Key” (masked)
      - “Source IP” (optional)
- Button: “Request certificate”
- Behavior:
  - Show step-by-step progress:
    - “Contacting Let’s Encrypt…”
    - “Adding DNS TXT record…”
    - “Waiting for propagation…”
    - “Validating domain…”
    - “Certificate installed.”
  - On failure:
    - Show clear error:
      - “DNS challenge failed.”
      - “Check your credentials and DNS provider settings.”
  - On success:
    - Update status to “Enabled (Let’s Encrypt – Production/Staging)”.

Section 4: Custom certificate

- Label: “Use your own certificate”
- Fields:
  - “Certificate file path”
  - “Key file path”
- Button: “Apply”
- Short text:
  - “Use a certificate managed externally (e.g., Traefik, internal CA, or another ACME client).”

Section 5: Renewal and management

- If ACME:
  - Show:
    - “Next renewal check: <date>”
  - Button:
    - “Renew now” (for manual trigger).
- If custom:
  - Show:
    - “Certificate expires: <date>”
  - Short text:
    - “Managed externally; update files as needed.”

7.3 Safety and guidance

- Prevent accidental exposure:
  - If user binds 0.0.0.0 and TLS is disabled:
    - Show:
      - “You are listening on all network interfaces without TLS.
         Your UI and data are accessible in cleartext on your network.”
- Help text:
  - Short, plain language.
  - No requirement to read external docs.

---

## 8. Security Audit Alignment

This design resolves finding #6 (“Basic Auth Weaknesses and Lack of TLS Enforcement”) by:

- Providing:
  - An easy, built-in way to enable TLS (self-signed or ACME).
  - A way to plug in external certificates.
- Keeping:
  - HTTP as default (for local-first, user autonomy).
- Adding:
  - mTLS for remote-agent (strong auth and encryption where it matters).
- Documenting:
  - That Basic Auth is for simple LAN protection.
  - That for stronger protection, users should:
    - Use TLS (native or via reverse proxy).
- Adding:
  - A startup warning when 0.0.0.0 is used without auth.

This is:
- Technically correct.
- Non-opinionated.
- Sufficient to close the finding.

---

## 9. Implementation Phases (Recommended)

Phase 1: Core TLS support
- Add TLSConfig model.
- Add:
  - --tls, --tls-cert, --tls-key, --tls-self-signed flags.
  - Certificates modal (No TLS / Self-signed / Custom).
- Integrate with existing web server.
- Add 0.0.0.0 warning.

Phase 2: ACME integration
- Bundle/use lego.
- Implement:
  - ACME mode in Certificates modal.
  - Namecheap DNS-01 provider.
  - Staging vs production.
  - Renewal job.

Phase 3: mTLS for remote-agent
- Formalize:
  - CA, server cert, agent client certs.
- Ensure:
  - mTLS is enforced for agent endpoints.
- Integrate with main TLS config.

Phase 4: Hardening and docs
- Update:
  - Security audit doc.
  - CLI help and docs.
- Add:
  - Clear warnings and guidance.
  - Example configurations.

---

## 10. For a New Agent: How to Continue or Debug

This section is intentionally explicit so that a fresh AI agent with no prior context can understand:
- Where to look.
- What to change.
- How pieces connect.

If you are a new agent:
- Read this document end-to-end first.
- Then consult the files and modules below.
- Do not invent new config formats; extend existing ones.
- Keep TLS optional and backward-compatible.

10.1 Core Rust files

- src/main.rs:
  - Entry point.
  - Responsible for:
    - Parsing CLI args.
    - Initializing AppState.
    - Starting the web server (HTTP or HTTPS).
  - Relevant for:
    - Integrating TLS listener based on TLSConfig.

- src/cli.rs:
  - CLI argument parsing.
  - Existing flags:
    - --host, --port, --basic-auth, etc.
  - Add (as needed):
    - --tls
    - --tls-cert
    - --tls-key
    - --tls-self-signed
  - Ensure:
    - New flags map to TLSConfig in a backward-compatible way.

- src/config.rs:
  - AppConfig and config loading.
  - Responsibilities:
    - Load base config (ports, paths, tokens).
    - Integrate TLSConfig from tls-config.json.
  - Add:
    - A TLSConfig struct.
    - A load_tls_config() function.
  - TLSConfig fields (minimum):
    - mode: enum { None, SelfSigned, Custom, Acme }
    - custom_cert_path, custom_key_path
    - acme config (FQDN, provider, credentials, environment, validation_delay)
  - Important:
    - If tls-config.json is missing or invalid, default to mode = None.

- src/web/mod.rs:
  - Web server setup and routing.
  - Responsibilities:
    - Build warp filter.
    - Wrap routes with auth and security headers.
  - Relevant for:
    - Starting an HTTPS listener instead of plain HTTP when TLS is enabled.
    - Possibly integrating TLS-aware routing (e.g., HSTS header when TLS is on).

- src/web/api.rs:
  - REST endpoints.
  - New endpoints (for Certificates modal):
    - GET /api/tls/config          – current TLS configuration (non-sensitive).
    - PUT /api/tls/config          – update TLS mode and settings.
    - POST /api/tls/acme/request   – trigger ACME certificate request.
    - POST /api/tls/acme/renew     – manual renewal.
  - Rules:
    - Require api-token where appropriate (align with existing security).
    - Never log raw ACME credentials or private keys.

- src/certs.rs (or src/tls.rs if you create it):
  - Central TLS and certificate logic.
  - Existing behavior:
    - Used for remote-agent TLS/mTLS.
  - Add:
    - generate_self_signed_cert()
    - load_custom_cert(cert_path, key_path)
    - run_acme_request(acme_config)
    - schedule_renewal(acme_config)
  - For ACME:
    - Spawn lego as a subprocess.
    - Set environment variables from acme_config.
    - Capture logs for UI progress and error reporting.
  - For mTLS:
    - Keep CA, server cert, agent client cert generation.
    - Ensure mTLS is enforced for agent endpoints.

- src/agent.rs:
  - Remote-agent logic.
  - Responsibilities:
    - Start/autostart remote-agent.
    - Validate commands.
  - Relevant for:
    - Ensuring mTLS is used when connecting back to the main instance.

- src/state.rs:
  - Global application state.
  - Responsibilities:
    - Track sessions, endpoints, capabilities.
  - Relevant for:
    - Possibly exposing TLS status in internal diagnostics (e.g., via /api/settings or /api/state).

10.2 Config and persistence

- Config directory:
  - ~/.config/llama-monitor/
- New file:
  - tls-config.json
- Rules:
  - Use serde for (de)serialization.
  - Keep it human-readable.
  - On load:
    - If invalid JSON, log warning; fall back to TLS disabled.
  - On save:
    - Atomic writes (write to .tmp, then rename).

10.3 Frontend: Certificates modal

- static/index.html:
  - Contains main layout and modals.
  - Add:
    - A “Certificates” tab/section in the Settings modal.
    - Container elements for:
      - Status
      - Modes
      - ACME fields
      - Custom cert fields

- static/js/features/settings.js:
  - Handles Settings modal logic.
  - Add:
    - Tab for “Certificates”.
    - Wires up:
      - Buttons (Disable TLS, Generate self-signed, Request certificate, Apply).
      - Inputs (FQDN, DNS provider, credentials, environment).
    - Calls:
      - /api/tls/config
      - /api/tls/acme/request
      - /api/tls/acme/renew
    - Displays:
      - Progress steps.
      - Errors in plain language.

- static/js/features/config.js:
  - General config helpers.
  - May be used for:
    - Shared API call patterns.
    - Error handling for TLS endpoints.

- static/js/core/app-state.js:
  - Global app state in the browser.
  - May store:
    - TLS status (enabled/disabled).
    - Whether to show 0.0.0.0 warnings.

10.4 ACME integration notes

- Use lego as a subprocess:
  - Pros:
    - Stable, single binary.
    - Native Namecheap provider.
  - Implementation:
    - On ACME request:
      - Build command:
        - lego run with:
          - --email (can be a synthetic internal email)
          - --accept-tos
          - --dns namecheap
          - --domains <FQDN>
          - --path <internal dir for ACME state>
          - Staging or production endpoint.
      - Set environment:
        - NAMECHEAP_USERNAME
        - NAMECHEAP_API_KEY
        - NAMECHEAP_SOURCEIP (if provided)
      - Stream logs to:
        - Backend logs.
        - UI progress (via WebSocket or polling).
    - On success:
      - lego writes cert.pem and key.pem.
      - We:
        - Update TLSConfig with cert/key paths.
        - Reload TLS listener (graceful).
    - On failure:
      - Show clear error in UI.
      - Do not silently fall back to HTTP if TLS was previously enabled.

- Staging vs production:
  - Staging:
    - Use Let’s Encrypt staging endpoint.
    - Certs are not trusted by browsers.
  - Production:
    - Use real Let’s Encrypt endpoint.
    - Certs are trusted.
  - UX:
    - Default to Staging on first ACME use.
    - Provide a clear “Switch to Production” toggle.

10.5 mTLS and remote-agent

- Existing infrastructure:
  - certs.rs already handles:
    - CA and cert generation.
    - TLS config for remote-agent.
- For a new agent:
  - Inspect:
    - How certs are generated.
    - How remote-agent connects.
  - Ensure:
    - mTLS is required for agent endpoints.
    - Main instance can use:
      - ACME cert (for public trust).
      - Or custom/self-signed cert.
    - While still enforcing mTLS for agents via their client certs.

10.6 Security and audit alignment

- This design resolves finding #6:
  - We:
    - Provide TLS (optional).
    - Provide ACME (Let’s Encrypt).
    - Provide mTLS for remote-agent.
    - Provide warnings when 0.0.0.0 is used without auth.
- For a new agent:
  - When implementing:
    - Do not make TLS mandatory.
    - Do not break existing HTTP workflows.
    - Ensure API keys and private keys:
      - Are never logged.
      - Are masked in the UI.
  - When debugging:
    - Use tls-config.json to understand the current TLS mode.
    - Check logs for:
      - TLS startup messages.
      - ACME/lego logs.
      - mTLS handshake issues.

