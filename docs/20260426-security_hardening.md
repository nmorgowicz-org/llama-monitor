# Security Hardening — Findings & Remediation

**Date:** 2026-04-26
**Scope:** `sensor_bridge` local HTTP server, llama-monitor agent mode, dashboard web server, install/uninstall scripts
**Reviewed by:** Claude + Qwen Code (automated review)

---

## Overview

Fifteen findings were identified during a comprehensive review of the sensor_bridge component, the remote agent server, and the main dashboard web server. Several allow remote code execution via SSH when the dashboard is network-accessible. Findings are ordered by practical severity.

---

## Finding 1 — Agent bearer token not constant-time compared

**File:** `src/agent.rs:158`  
**Severity:** Low  
**Category:** Timing side-channel

### Description

The token check uses Rust's built-in string equality:

```rust
value == format!("Bearer {expected}")
```

String equality in Rust short-circuits on the first differing byte. An attacker who can make many requests and measure response times with nanosecond precision could use a timing oracle to recover the token one character at a time.

In practice this requires low-latency access to the agent port and hundreds of thousands of requests, making it realistic only on a LAN or co-located server. It is not exploitable over the internet through normal network jitter.

### Remediation

Use a constant-time comparison. Add the `subtle` crate (no transitive dependencies):

```toml
# Cargo.toml
subtle = "2"
```

```rust
use subtle::ConstantTimeEq;

let provided = value
    .strip_prefix("Bearer ")
    .unwrap_or("")
    .as_bytes();
let valid = provided.ct_eq(expected.as_bytes()).into();
if !valid {
    return Err(warp::reject::custom(AgentAuthError));
}
```

---

## Finding 2 — Agent communicates over plain HTTP (no TLS)

**File:** `src/agent.rs:242`, `src/agent.rs:699`  
**Severity:** Medium  
**Category:** Credential / data exposure

### Description

The remote agent serves metrics and accepts bearer tokens over unencrypted HTTP. On any network path shared with other hosts (LAN, Wi-Fi, managed switch with port mirroring), an observer can:

- Capture the bearer token and replay it indefinitely.
- Read all metrics in transit (CPU/GPU/RAM telemetry).

The default install command binds to `0.0.0.0:7779`, which is reachable from any host on the network.

### Remediation

**Short-term (recommended):** Document that the agent port must be protected by a network-level control. The SSH tunnel approach is the lowest-friction option:

```bash
# On the dashboard host, forward local 7779 → remote 7779 over SSH
ssh -N -L 7779:127.0.0.1:7779 user@remote-host
# Configure the dashboard to use http://127.0.0.1:7779 as the agent URL
```

With this in place, the bearer token travels inside the encrypted SSH session and the agent can safely bind to `127.0.0.1` only.

**Long-term:** Add optional TLS to the agent (`--agent-tls-cert` / `--agent-tls-key`). warp supports TLS via `warp::serve(...).tls()`. A self-signed certificate with cert-pinning in the dashboard client is sufficient — CA validation is not necessary for a private point-to-point deployment.

---

## Finding 3 — TOCTOU race on elevated install scripts

**Files:** `src/lhm.rs:178`, `src/agent.rs` (uninstall script drop)  
**Severity:** Low–Medium  
**Category:** Local privilege escalation

### Description

The sensor_bridge install and uninstall flows write a PowerShell script to a predictable path in `%TEMP%` and then invoke it with elevated privileges via `Start-Process -Verb RunAs`:

```rust
let script_path = std::env::temp_dir().join("llama_monitor_sb_install.ps1");
std::fs::write(&script_path, &script)?;
// UAC elevation triggered here — window between write and exec
Command::new("powershell")
    .args(["-Command",
           &format!("Start-Process powershell.exe -Verb RunAs -ArgumentList '... -File \"{script_path_str}\"'")])
    .spawn()?;
```

A local attacker who can write to `%TEMP%` (any process running as the same user) can replace the script between the `fs::write` and the `Start-Process` invocation. Because the elevated process runs the attacker's script, this is a local privilege escalation to SYSTEM.

The window is narrow (milliseconds) but it is a real TOCTOU.

### Remediation

**Option 3 (implemented):** Use inline script execution via PowerShell's `-EncodedCommand` parameter. This eliminates the temp file entirely, removing the attack surface.

```rust
use base64::{Engine as _, engine::general_purpose::STANDARD};

// Encode the script content as base64
let encoded = STANDARD.encode(script.as_bytes());

// Execute inline — no temp file, no race condition
std::process::Command::new("powershell")
    .args([
        "-NoProfile",
        "-Command",
        &format!(
            "Start-Process powershell.exe -Verb RunAs -ArgumentList '-NoProfile -ExecutionPolicy Bypass -EncodedCommand \"{encoded}\"'"
        ),
    ])
    .spawn()
    .map_err(|e| format!("Failed to launch UAC prompt: {e}"))?;
```

**Why this is better:**
- No temp file → no TOCTOU window
- No file permissions to manage
- No cleanup needed
- Script content is opaque (base64-encoded) during transit

**Trade-off:** The script content is visible in the process list briefly, but this is acceptable for a local elevation prompt that requires user interaction.

---

## Finding 4 — sensor_bridge `lastJson` field is not thread-safe (C#)

**File:** `sensor_bridge/Program.cs:35`  
**Severity:** Low  
**Category:** Data race / undefined behavior

### Description

`lastJson` is a plain `string` field written by a `System.Threading.Timer` callback and read by the main HTTP handler loop without synchronization:

```csharp
string lastJson = CollectSensors(computer);   // initialized on main thread

var timer = new Timer(_ => {
    lastJson = CollectSensors(computer);       // written on timer thread
}, ...);

// main loop
var bytes = Encoding.UTF8.GetBytes(lastJson); // read on main thread
```

.NET guarantees that reference-type assignments are atomic on both 32-bit and 64-bit runtimes, so a torn reference (pointing to garbage memory) cannot occur. However, without a memory barrier the JIT or CPU is free to keep a cached copy of the old reference in a register, meaning the main loop may serve data that is multiple refresh cycles stale. This is a benign data race in practice, but it is undefined behavior under the C# memory model.

### Remediation

Mark the field `volatile`:

```csharp
volatile string lastJson = CollectSensors(computer);
```

`volatile` in C# inserts a memory barrier on every read and write, ensuring the main loop always observes the most recently committed value from the timer thread. No locking overhead is introduced.

---

## Finding 5 — sensor_bridge runs as SYSTEM with no authentication

**File:** `sensor_bridge/Program.cs`, `src/lhm.rs` (install task)  
**Severity:** Inherent (by design) — documented for awareness  
**Category:** Privilege + information exposure

### Description

The scheduled task runs `sensor_bridge.exe` as the `SYSTEM` account — the highest privilege level on Windows. LibreHardwareMonitor requires this to load its kernel driver and read CPU package temperature. The HTTP listener binds to `127.0.0.1:7780` only, so the surface is limited to local processes.

Any process running on the machine (including low-privilege user processes and sandboxed applications) can connect to `127.0.0.1:7780` and receive the full sensor JSON without credentials. The data includes CPU model, motherboard model, and all temperature readings.

There is no path from read-only sensor data to code execution. However, if a future version of LibreHardwareMonitor or the .NET runtime introduced a memory-corruption vulnerability in the HTTP handling path, a local attacker could craft a malicious HTTP request to achieve SYSTEM code execution.

### Remediation

**Short-term:** No action required. The loopback binding adequately limits network exposure, and the data served is hardware telemetry only (no user data, no file system access beyond LHM internals).

**Long-term mitigations to consider:**
- Add Windows ACL-based access control to the socket using `HttpListener.AuthenticationSchemes` (NTLM/Negotiate) so only the `llama-monitor` process user can connect. This is complex to implement correctly.
- Run the HTTP server component under a lower-privilege account (e.g., `NT SERVICE\SensorBridge`) and start the LHM kernel driver initialization separately under SYSTEM, then hand off a read-only handle. This is a significant architectural change.
- Document clearly in user-facing materials that `sensor_bridge.exe` runs as SYSTEM and what data it exposes.

---

## Finding 6 — sensor_bridge HTTP handler is single-threaded

**File:** `sensor_bridge/Program.cs:47`  
**Severity:** Low  
**Category:** Denial of service (local)

### Description

The main loop calls `listener.GetContext()` synchronously and handles one request at a time. A client that connects and then reads the response slowly (or not at all) will stall all subsequent callers until the connection times out.

```csharp
while (true)
{
    var ctx = listener.GetContext(); // blocks until a request arrives
    // ... write response to ctx.Response.OutputStream
    ctx.Response.Close();            // blocks until client reads or closes
}
```

Since the server binds to loopback, only local processes can trigger this. A malicious local process could hold the connection open and prevent `llama-monitor` from reading fresh sensor data.

### Remediation

Dispatch each request to the thread pool:

```csharp
while (true)
{
    var ctx = listener.GetContext();
    ThreadPool.QueueUserWorkItem(_ =>
    {
        try
        {
            var bytes = Encoding.UTF8.GetBytes(lastJson);
            ctx.Response.ContentType = "application/json";
            ctx.Response.ContentLength64 = bytes.Length;
            ctx.Response.OutputStream.Write(bytes, 0, bytes.Length);
            ctx.Response.Close();
        }
        catch { }
    });
}
```

Set a short write timeout on the response stream to bound how long a slow client can hold a thread.

---

## Finding 7 — sensor_bridge accepts any HTTP method

**File:** `sensor_bridge/Program.cs:51`  
**Severity:** Informational  
**Category:** Protocol correctness

### Description

The handler returns sensor JSON for `POST`, `DELETE`, `PUT`, and any other HTTP method, not just `GET`. This is not exploitable but deviates from expected HTTP semantics and could cause confusion for monitoring or proxy tooling.

### Remediation

```csharp
if (ctx.Request.HttpMethod != "GET")
{
    ctx.Response.StatusCode = 405; // Method Not Allowed
    ctx.Response.Close();
    continue;
}
```

---

## Finding 8 — `/info` endpoint exposes executable path and PID

**File:** `src/agent.rs:185`  
**Severity:** Informational  
**Category:** Information disclosure (authenticated)

### Description

The `/info` route, which is behind authentication, returns the full filesystem path to the agent executable and the process ID:

```rust
"executable": std::env::current_exe().ok().map(|p| p.to_string_lossy()),
"pid": std::process::id(),
```

This information is useful for legitimate management but becomes an asset for an attacker who has obtained the bearer token — they can determine the exact install path without needing to enumerate the filesystem.

### Remediation

No change required given the endpoint is authenticated. Document that the bearer token should be treated as a secret with the same sensitivity as SSH credentials. If a future "read-only" vs. "admin" token split is introduced, the `/info` endpoint should require the admin token.

---

## Finding 9 — Command injection via user-controlled install paths

**File:** `src/agent.rs` lines ~1385-1386, ~1411-1432, ~698-706
**Severity:** Critical
**Category:** Remote code execution (via SSH)

### Description

User-controlled `install_path` and `install_dir` parameters from the web API are interpolated directly into shell commands without sanitization or escaping:

```rust
RemoteOs::Unix | RemoteOs::Macos => format!("mv {temp_path} {install_path}"),
```

```rust
let command = format!(
    "powershell.exe -NoProfile -NonInteractive -Command \"$ErrorActionPreference = 'Stop'; \
if (!(Test-Path '{dir}')) {{ New-Item -ItemType Directory -Path '{dir}' -Force | Out-Null }}; \
...",
    dir = install_dir,
    // ...
);
```

An attacker who controls the install path via `/api/remote-agent/install` can inject arbitrary commands executed on the remote host via SSH. For example, setting `install_path` to `"; rm -rf /; #` on Unix or `"; cmd.exe /c malicious-command; #` on Windows would execute the injected payload.

The `default_start_command_for_os` function has the same issue:

```rust
RemoteOs::Unix | RemoteOs::Macos => format!(
    "nohup {install_path} --agent --agent-host 0.0.0.0 --agent-port {REMOTE_AGENT_DEFAULT_PORT} > ~/.config/llama-monitor/agent.log 2>&1 &"
),
```

### Remediation

**Layered defense recommended.** Shell escaping alone is insufficient due to nested interpretation contexts (Rust string → SSH shell → PowerShell → elevated PowerShell). Each layer has its own escaping rules, and fixing only the Rust layer does not guarantee safety.

**Layer 1 — Input validation (most effective):** Restrict install paths to a known-safe character set and allowed directories:

```rust
fn validate_install_path(path: &str) -> Result<(), Error> {
    let p = Path::new(path);

    // Must be absolute
    if !p.is_absolute() {
        return Err(Error::new("Path must be absolute"));
    }

    // Must not contain shell metacharacters
    if path.chars().any(|c| ";|&$`'\"(){}[]!#~<>*?".contains(c)) {
        return Err(Error::new("Path contains invalid characters"));
    }

    // Must not target suspicious directories
    let forbidden = ["/tmp", "/var", "/etc", "C:\\Windows"];
    if forbidden.iter().any(|f| p.starts_with(f)) {
        return Err(Error::new("Path not allowed"));
    }

    Ok(())
}
```

**Layer 2 — Argument vectors (where possible):** Bypass the shell entirely for local execution:

```rust
// Instead of:
format!("mv {temp_path} {install_path}")

// Use:
Command::new("mv")
    .arg(&temp_path)
    .arg(&install_path)
    .output()?;
```

This is not always possible for SSH commands (which require a single command string), but should be used wherever the execution is local.

**Layer 3 — Platform-appropriate escaping (for SSH commands):** When string interpolation is unavoidable:

```rust
// Add dependency
// Cargo.toml: shlex = "1"

fn shell_quote_path(path: &str, os: RemoteOs) -> String {
    match os {
        RemoteOs::Unix | RemoteOs::Macos => {
            shlex::quote(path).into_owned()
        }
        RemoteOs::Windows => {
            // PowerShell single quotes are literal; escape embedded quotes by doubling
            format!("'{}'", path.replace(''', "''"))
        }
    }
}

// Usage:
RemoteOs::Unix | RemoteOs::Macos => {
    format!("mv {} {}", shell_quote_path(temp_path, os), shell_quote_path(install_path, os))
}
```

**Layer 4 — Base64 encoding (for arbitrary data):** When paths could contain any character:

```rust
fn ssh_safe_path(path: &str, os: RemoteOs) -> String {
    let encoded = base64::encode(path.as_bytes());
    match os {
        RemoteOs::Unix | RemoteOs::Macos => {
            format!("$(echo '{}' | base64 -d)", encoded)
        }
        RemoteOs::Windows => {
            format!("$( [System.Text.Encoding]::UTF8.GetString([System.Convert]::FromBase64String('{}')) )", encoded)
        }
    }
}
```

### Caveats

- `shlex::quote` wraps in single quotes — may behave differently in non-interactive shells (rare with SSH)
- PowerShell single-quote escaping is incomplete if the string is later interpolated into a double-quoted context
- Multiple quoting layers (Rust → SSH → PowerShell → elevated PowerShell) compound the risk — each layer must be handled correctly

**Bottom line:** Input validation (Layer 1) is the single most effective control. Shell escaping (Layer 3) is a defense-in-depth measure, not a standalone fix.

---

## Finding 10 — Main web server binds to 0.0.0.0 with no authentication

**Files:** `src/main.rs:168, 223`, `src/web/api.rs` (all routes)
**Severity:** High
**Category:** Unauthenticated access / network exposure

### Description

The dashboard server binds to all network interfaces with no authentication on 40+ API endpoints:

```rust
warp::serve(routes).run(([0, 0, 0, 0], port)).await;
```

Any machine on the local network can:
- Start/stop llama-server instances via `/api/start-llama`
- Browse the entire filesystem via `/api/browse`
- Read/write presets and settings
- Execute SSH commands to remote hosts (if SSH agent is configured)
- Kill processes via `/api/kill-llama`
- Install remote agents on arbitrary hosts via `/api/remote-agent/install`

Contrast with the agent mode (`src/agent.rs:167-186`) which correctly implements optional Bearer token auth via `--agent-token`. The dashboard has no equivalent mechanism.

### Remediation

**Short-term (recommended):** Default the bind address to `127.0.0.1` and add a `--host` or `--bind` CLI flag for explicit network exposure:

```rust
let bind_addr: IpAddr = args.get("host")
    .map(|h| h.parse().expect("Invalid --host"))
    .unwrap_or_else(|| Ipv4Addr::new(127, 0, 0, 1).into());
```

**Long-term:** Add an optional `--api-token` flag similar to `--agent-token` for the dashboard API. This would allow secure remote access when needed, while still protecting against unauthenticated access on exposed networks.

---

## Finding 11 — SSRF / open proxy via chat endpoint

**File:** `src/web/api.rs:1233-1260`
**Severity:** High
**Category:** Server-side request forgery

### Description

The `/api/chat` endpoint acts as an open HTTP proxy to any port on localhost:

```rust
let port = query.get("port")
    .and_then(|p| p.parse::<u16>().ok())
    .unwrap_or_else(|| { /* defaults to 8080 */ });
let url = format!("http://127.0.0.1:{port}/v1/chat/completions");
// ...
client.post(&url).body(body.to_vec()).send().await
```

An attacker can use this to:
- Probe internal services on arbitrary ports (e.g., database ports, admin panels)
- Access cloud metadata endpoints (if reachable via localhost)
- Bypass network restrictions that block direct access to internal ports

While restricted to `127.0.0.1`, this still exposes all localhost services to the web UI.

### Remediation

Whitelist allowed ports or restrict to only the configured llama-server port:

```rust
let allowed_ports: HashSet<u16> = [8080, 8081, 8082, 8192]
    .iter().cloned().collect();
let port = query.get("port")
    .and_then(|p| p.parse::<u16>().ok())
    .unwrap_or(8080);
if !allowed_ports.contains(&port) {
    return Err(warp::reject::custom(BadRequest("Port not allowed")));
}
```

Alternatively, derive the port from the active session configuration rather than accepting it as user input.

---

## Finding 12 — SSRF via attach endpoint

**File:** `src/web/api.rs:1575-1650`
**Severity:** Medium
**Category:** Server-side request forgery

### Description

The `/api/attach` endpoint accepts an arbitrary `endpoint` URL and makes HTTP health checks to it:

```rust
let endpoint: String = match payload.get("endpoint") {
    // ...
};
// Pre-attach health check
let client = reqwest::Client::builder()
    .timeout(std::time::Duration::from_secs(5))
    .build()?;
let server_up = client.get(&endpoint).send().await.is_ok();
```

This could be used for:
- Internal network reconnaissance (probing internal IPs)
- Port scanning (testing reachability of internal services)
- Accessing internal services not directly reachable from the attacker's machine

The 5-second timeout limits the impact but does not prevent the information disclosure.

### Remediation

Validate that the endpoint is a reasonable llama-server URL (e.g., restrict to common ports, or require it to be on a known subnet):

```rust
fn validate_llama_endpoint(url: &str) -> Result<(), ApiError> {
    let parsed = url::Url::parse(url)?;
    let host = parsed.host_str().ok_or(BadRequest("No host"))?;

    // Allow localhost, 127.0.0.1, and private IP ranges
    match host.parse::<IpAddr>() {
        Ok(ip) if ip.is_loopback() | ip.is_private() => Ok(()),
        _ => Err(BadRequest("Endpoint must be local or private")),
    }
}
```

---

## Finding 13 — Missing HTTP security headers

**File:** `src/web/mod.rs`
**Severity:** Medium
**Category:** Missing security controls
**Status:** ✅ Fixed and released in v0.9.2

### Description

No security headers are set on any response. Specifically missing:
- `Content-Security-Policy` — No XSS mitigation
- `X-Frame-Options` — No clickjacking protection
- `X-Content-Type-Options` — MIME sniffing not disabled
- `Strict-Transport-Security` — No HSTS (though less relevant for local-only)
- `Referrer-Policy` — No referrer control

### Remediation

**Initial approach (incorrect):** Adding custom middleware via warp filters. Warp lacks true response middleware, making this approach unworkable without touching every route.

**Correct approach:** Use `warp-helmet` crate — an actively maintained security middleware that wraps all routes and sets security headers automatically.

**CSP customization required:** `Helmet::default()` sets restrictive CSP that blocks the app's external dependencies. The app requires:
- Inline `onclick` handlers (100+ instances)
- External CDN scripts (`cdn.jsdelivr.net` for marked.js)
- External fonts/styles (`fonts.googleapis.com`, `fonts.gstatic.com`)
- Data URIs for SVG images

Customized CSP to permit all legitimate sources while keeping other security headers active.

```rust
use warp_helmet::{Helmet, HelmetFilter, ContentSecurityPolicy};

// In build_routes():
let csp = ContentSecurityPolicy::new()
    .default_src(vec!["'self'", "data:"])
    .script_src(vec!["'self'", "'unsafe-inline'", "https://cdn.jsdelivr.net"])
    .style_src(vec!["'self'", "'unsafe-inline'", "https://fonts.googleapis.com"])
    .font_src(vec!["'self'", "https://fonts.gstatic.com"])
    .img_src(vec!["'self'", "data:", "https:"]);
let helmet: HelmetFilter = Helmet::new().add(csp).try_into().unwrap();
helmet.wrap(routes)
```
```

---

## Finding 14 — Agent token is optional with no hard fail

**File:** `src/agent.rs:178-198`
**Severity:** High
**Category:** Authentication bypass

### Description

The `--agent-token` is optional. When not provided, the agent server logs a warning but still serves metrics without any authentication:

```rust
if agent_token.is_empty() {
    eprintln!("[warn] No agent token configured — metrics are unauthenticated");
    // Server still starts and serves requests
}
```

The warning is printed to stderr but does not prevent the server from starting. An attacker on the same network can read system and GPU metrics from any agent instance without a token. Combined with Finding 2 (no TLS), the token — if one were set — would also be transmitted in plaintext.

### Remediation

Consider making the token required when the agent binds to `0.0.0.0`:

```rust
if agent_token.is_empty() && agent_host != "127.0.0.1" {
    eprintln!("[error] --agent-token is required when binding to non-loopback addresses");
    process::exit(1);
}
```

Alternatively, add a `--agent-token-required` flag that defaults to `true` when not binding to localhost.

---

## Finding 15 — Insecure temporary file handling across multiple locations

**Files:** `src/lhm.rs:174, 221`; `src/agent.rs:1051, 1069, 1158, 1280`
**Severity:** High
**Category:** Symlink attack / local privilege escalation

### Description

Multiple locations use predictable temp filenames beyond the TOCTOU already documented in Finding 3:

```rust
// lhm.rs:174
let script_path = std::env::temp_dir().join("llama_monitor_sb_install.ps1");

// agent.rs:1051
let local_tmp = std::env::temp_dir().join("llama_monitor_agent_uninstall.bat");

// agent.rs:1069
let local_tmp = std::env::temp_dir().join("llama_monitor_agent_install.bat");
```

Issues:
- **Predictable names** enable symlink attacks — a local attacker can create a symlink at the expected path, causing the application to write to or overwrite arbitrary files
- **No atomic write** operations — write-then-rename pattern is used in some places but not consistently
- **Scripts with sensitive data** (paths, commands) are not cleaned up immediately after use

### Remediation

Use the `tempfile` crate for secure temporary file creation with random names and automatic cleanup:

```rust
// Add dependency
// Cargo.toml: tempfile = "3"

use tempfile::NamedTempFile;

// Creates a file with a random name, deleted on drop
let mut script = NamedTempFile::new_in(std::env::temp_dir())?;
script.write_all(script_contents.as_bytes())?;
let script_path = script.path().to_path_buf();

// Keep the file alive beyond the NamedTempFile scope if needed
let _file_handle = script.keep()?.1;
```

This ensures unique filenames per invocation and automatic cleanup when the handle goes out of scope.

---

## Recommended Fix Priority

| # | Finding | Priority | Effort |
|---|---------|----------|--------|
| 9 | Command injection via install paths | **Critical** | Medium (`shlex::quote`) |
| 10 | Dashboard binds to 0.0.0.0, no auth | **High** | Low (`--host` flag) |
| 11 | SSRF via chat endpoint | **High** | Low (port whitelist) |
| 14 | Agent token optional, no hard fail | **High** | Low (require on 0.0.0.0) |
| 15 | Insecure temp files (multiple) | **High** | Low (`tempfile` crate) |
| 2 | No TLS — token/data in plaintext | **High** | High (TLS) or Low (VPN doc) |
| 12 | SSRF via attach endpoint | **Medium** | Low (URL validation) |
| 3 | TOCTOU on install script temp files | **Medium** | Low (random filename) |
| 1 | Non-constant-time token comparison | **Low** | Low (`subtle` crate) |
| 4 | `lastJson` data race (C#) | **Low** | Trivial (`volatile`) |
| 6 | Single-threaded sensor_bridge handler | **Low** | Low (threadpool) |
| 7 | sensor_bridge accepts all HTTP methods | **Informational** | Trivial |
| 5 | sensor_bridge runs as SYSTEM, no auth | **Inherent** | Architectural |
| 8 | `/info` leaks exe path and PID | **Informational** | N/A (behind auth) |

---

## Remediation Checklist

- [x] **#9** Command injection via install paths — Fixed with input validation + shell quoting; validated end-to-end against remote host
- [x] **#10** Dashboard binds to 0.0.0.0, no auth — Added `--host` flag defaulting to `127.0.0.1` and `--basic-auth` (PR #90)
- [x] **#11** SSRF via chat endpoint — Removed user-controlled `port` parameter; endpoint derived from active session (PR #96)
- [x] **#14** Agent token optional, no hard fail — Auto-generate token on first run, persist to config dir, log for dashboard pairing (PR #96)
- [x] **#15** Insecure temp files (multiple) — `extract_archive` migrated to `tempfile::Builder` for random names + auto-cleanup
- [x] **#2** No TLS — token/data in plaintext — Cert infrastructure in place (certs.rs), CA distribution via install payload, dashboard accepts self-signed certs
- [x] **#12** SSRF via attach endpoint — Validate scheme (http/https only) and restrict to private/loopback IPs
- [x] **#13** Missing HTTP security headers — Added `warp-helmet` with custom CSP to allow inline handlers and CDN scripts
- [x] **#3** TOCTOU on install script temp files — Eliminated temp file entirely via inline `-EncodedCommand`
- [ ] **#1** Non-constant-time token comparison — Add `subtle` crate
- [ ] **#4** `lastJson` data race (C#) — Add `volatile` keyword
- [ ] **#6** Single-threaded sensor_bridge handler — Use thread pool
- [ ] **#7** sensor_bridge accepts all HTTP methods — Return 405 for non-GET
- [ ] **#5** sensor_bridge runs as SYSTEM, no auth — Document; consider lower-privilege account
- [ ] **#8** `/info` leaks exe path and PID — Document; consider admin-only token

---

## Out of Scope

- SSH host key validation in `src/remote_ssh.rs` was reviewed and found to be **correctly implemented** with constant-time comparison and a trust store.
- LibreHardwareMonitor itself is a third-party dependency; its security posture is not covered here.
