# Windows migration API qualification handoff

**Purpose:** complete the native Windows application-home migration API and
journal qualification independently from the macOS session.

**Repository:** `nmorgowicz-org/local-llm-foundry`

**Branch:** `feat/rapid-mlx-integration`

**Required commit:** `d711fb1` or a later descendant of that commit.

**Scope:** application-home migration status/preview, authenticated queueing,
copy-first restart execution, interruption/resume, rollback preview/queue, and
cleanup safety. This handoff does not authorize testing against a real profile,
real model inventory, or deleting any user data.

## Why this is a separate handoff

The native binary starts correctly on Windows, but an SSH-launched detached
process is terminated when the SSH session ends. That makes a later HTTP request
unreliable. Run the server and API requests from one persistent Windows
PowerShell session (or a local Windows terminal) so the process lifetime is
controlled and every response can be captured.

## Preconditions

1. Checkout the required branch and verify the commit:

   ```powershell
   git status --short --branch
   git log -1 --oneline
   ```

   The worktree must be clean and the commit must be `d711fb1` or newer.

2. Build the native release binary:

   ```powershell
   cargo build --release
   ```

3. Use a disposable directory below `$env:TEMP` for every scenario. Do not
   point `--config-dir` at the real `%APPDATA%` tree. The migration API's
   normal roots are discovered from Windows known folders, so `--config-dir`
   alone is not a sufficient isolation boundary.

4. Pass the disposable migration root explicitly:

   ```powershell
   --migration-test-root (Join-Path $root "migration-roots")
   ```

   This qualification-only option is accepted only for a directory beneath
   the current user's temp directory. It derives isolated roots beneath that
   directory (`local-llm-foundry` and `llama-monitor`), rejects symlinks and
   non-directory roots, and leaves normal production root discovery unchanged.
   The queue marker, journal, receipt, rollback marker, and cleanup plan stay
   beside those isolated roots. The token files in `--config-dir` are
   encrypted at rest; do not use their raw contents as bearer tokens. For a
   loopback qualification run, obtain the live values from
   `/api/internal/api-token` and `/api/db/admin-token`, then use those values
   only in memory. The first endpoint is unauthenticated on loopback; the
   second accepts the API token and returns the live admin token.

5. Create a receipt directory outside the repository or under the machine-local
   evidence directory described below. Redact tokens, usernames, hostnames, and
   absolute paths before committing any receipt.

## Persistent server procedure

Run the following as a single persistent PowerShell session. Replace the binary
path with the local checkout's `target\release\local-llm-foundry.exe` path and
replace `$cfg` with the scenario's disposable config directory.

```powershell
$scenario = "legacy-preview"
$root = Join-Path $env:TEMP ("foundry-migration-" + [guid]::NewGuid())
$cfg = Join-Path $root "AppData\llama-monitor"
$out = Join-Path $root "stdout.log"
$err = Join-Path $root "stderr.log"
New-Item -ItemType Directory -Force $cfg | Out-Null

# Add scenario fixtures here before starting the process.
$p = Start-Process `
  -FilePath (Resolve-Path ".\target\release\local-llm-foundry.exe") `
  -ArgumentList @("--headless", "--config-dir", $cfg, "--migration-test-root", (Join-Path $root "migration-roots"), "--port", "17880") `
  -PassThru `
  -RedirectStandardOutput $out `
  -RedirectStandardError $err

Start-Sleep -Seconds 15
if ($p.HasExited) { throw "server exited before API probe; inspect stdout/stderr" }

$apiToken = (Get-Content (Join-Path $cfg "api-token") -Raw).Trim()
$adminToken = (Get-Content (Join-Path $cfg "db-admin-token") -Raw).Trim()
$apiHeaders = @{ Authorization = "Bearer $apiToken" }
$adminHeaders = @{ Authorization = "Bearer $adminToken" }

function Invoke-ReceiptRequest($method, $uri, $headers, $body = $null) {
  $started = Get-Date
  try {
    if ($null -eq $body) {
      $response = Invoke-WebRequest -Method $method -Uri $uri -Headers $headers -UseBasicParsing -TimeoutSec 30
    } else {
      $response = Invoke-WebRequest -Method $method -Uri $uri -Headers $headers -ContentType "application/json" -Body ($body | ConvertTo-Json -Compress) -UseBasicParsing -TimeoutSec 30
    }
    [pscustomobject]@{ method=$method; uri=$uri; started=$started.ToString("o"); status=[int]$response.StatusCode; body=$response.Content }
  } catch {
    [pscustomobject]@{ method=$method; uri=$uri; started=$started.ToString("o"); status="error"; body=$_.Exception.Message }
  }
}

$base = "http://127.0.0.1:17880"
Invoke-ReceiptRequest GET "$base/api/app-home-migration/status" $apiHeaders
Invoke-ReceiptRequest GET "$base/api/app-home-migration/preview" $apiHeaders
```

Keep the process alive while requests run. At the end of a scenario, stop it
explicitly and record its exit/liveness state:

```powershell
if (-not $p.HasExited) { Stop-Process -Id $p.Id -Force }
Get-Content $out
Get-Content $err
```

## Required scenarios

### A. Legacy-only preview

- Create only the legacy config root and add a harmless sentinel file plus a
  recognized state marker such as `presets.json`; a sentinel alone is not
  considered application state by the root classifier.
- Start with that root as `--config-dir`.
- `GET /api/app-home-migration/status` must report `legacy_active`.
- `GET /api/app-home-migration/preview` must return a non-null deterministic
  `plan_id` and copy/retention entries.
- No canonical root may be created during preview.
- Record the complete response body and a redacted tree manifest.

### B. Queue and restart execution

- Save the exact preview `plan_id`.
- `POST /api/app-home-migration/queue` with the admin token and:

  ```json
  {
    "plan_id": "<exact preview plan id>",
    "confirmation": "MIGRATE TO LOCAL LLM FOUNDRY"
  }
  ```

- Confirm the queue response and marker are recorded.
- Stop the server normally, restart it with the same disposable root, and
  capture startup stdout/stderr.
- Verify copy-first execution, a receipt, canonical destination creation,
  retained legacy source, and preserved sentinel/hash values.

### C. Interrupted migration and resume

- Use a fixture large enough that the migration has a journal checkpoint.
- Queue the migration, start the controlled restart, and interrupt only after a
  journal checkpoint is visible. Record the exact interruption command/time.
- Capture the tree, journal, queue marker, stdout/stderr, and process exit code.
- Restart with the same root and verify deterministic resume, no duplicate or
  truncated files, source retention, and a final verified receipt.
- If the implementation intentionally classifies a cancellation as
  non-resumable, record that explicit classification and the cleanup result;
  do not relabel it as a resume pass.

### D. Rollback preview and queue

- After a verified migration receipt, call:

  `POST /api/app-home-migration/rollback/preview`

- Confirm the response is receipt-scoped and names only the verified canonical
  destination.
- Queue rollback with the admin token and exact confirmation:

  ```json
  { "confirmation": "ROLL BACK TO LLAMA MONITOR" }
  ```

- Restart under controlled conditions and verify canonical cleanup, legacy
  retention, sentinel/hash preservation, and rollback receipt.

### E. Safety negatives

Record HTTP status and body for each rejection:

- missing or malformed bearer token;
- API token used where db-admin token is required;
- wrong confirmation string;
- stale or altered `plan_id`;
- conflicting roots;
- partial/empty roots;
- permission-denied fixture;
- path traversal or symlink fixture where supported.

No negative case may delete data or create an unrelated destination.

## Receipt layout

Write machine-local raw evidence first, then commit only redacted summaries:

```text
docs/plans/evidence/20260811-local-llm-foundry/phase-12/windows/migration-api/
  machine-manifest.txt
  commands.txt
  status-preview.jsonl
  queue-execution.jsonl
  interruption-resume.jsonl
  rollback.jsonl
  negative-cases.jsonl
  stdout/
  stderr/
  tree-manifests/
  SHA256SUMS
```

Every JSONL record must include timestamp, scenario, commit, endpoint or
command, HTTP status or exit code, redacted response/body, and result. Include
the final tree/hash comparison and explicit cleanup confirmation. Never commit
tokens or unredacted absolute user paths.

## Pass criteria

The migration API gate passes only when A–E have raw receipts, the interrupted
case has an explicit resume or intentional non-resumable classification, no
scenario loses or silently deletes data, and the final disposable process/root
cleanup is confirmed. Update the Phase 12 Windows ledger and the authoritative
rebrand plan only after those receipts exist.

## References

- `docs/plans/20260812-local_llm_foundry-windows-validation-handoff.md`
- `docs/plans/20260811-local_llm_foundry-rebrand.md`
- `docs/reference/api.md` (Application-home migration endpoints)
- `docs/reference/model-library.md`
