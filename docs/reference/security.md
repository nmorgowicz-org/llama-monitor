# Security Reference

This document covers dashboard-side authentication, token handling, local recovery, and where
security-related configuration is stored on disk.

## Dashboard Access Modes

Llama Monitor supports four dashboard access states:

- `No Auth`
  - Default behavior.
  - Intended for local-only use on `127.0.0.1`.
- `Form Login`
  - Shows the in-app sign-in shell before protected routes are available.
  - Uses an HttpOnly session cookie after a successful login.
- `Basic Auth`
  - Uses the browser’s native username/password challenge.
- `Both`
  - Accepts either HTTP Basic credentials or a valid in-app form session.

Users can manage config-backed dashboard auth from:

- `Settings → Security → Dashboard Access`

CLI flags remain available for startup-managed auth:

- `--basic-auth user:password`
- `--form-auth user:password`

If startup flags are present, the Security tab shows that the instance is CLI-managed and treats
the in-app dashboard-access controls as read-only.

## Stored Files

Security-relevant local files include:

- `auth-config.json`
  - Stores dashboard auth mode, username, and hashed password.
- `api-token`
  - General admin bearer token for protected UI/API operations.
- `db-admin-token`
  - Elevated bearer token for destructive database operations.
- `tls-config.json`
  - Persisted TLS mode and ACME configuration.
- `encryption-key`
  - Local key used for at-rest encryption of other sensitive values.

On Unix, llama-monitor hardens these files to owner-only permissions (`0600`) at startup.

## Password Storage

Dashboard passwords stored in `auth-config.json` are:

- hashed, not stored in plaintext
- not returned by the UI API
- used for both Basic Auth and form-login verification when config-managed auth is enabled

The auth config is separate from `ui-settings.json` so security settings do not get mixed into the
general dashboard preferences payload.

## Password Change Flow

When a user is already signed in:

1. Open `Settings → Security`.
2. Go to `Dashboard Access`.
3. Choose the desired auth mode.
4. Enter a username.
5. Enter a new password.
6. If replacing an existing password, provide the current password too.
7. Click `Save Dashboard Access`.

Behavior:

- Changing the password invalidates existing form-auth sessions.
- Changing the mode without changing the password keeps the existing stored hash.
- Setting both modes to off disables dashboard auth and clears `auth-config.json`.

## Locked-Out Recovery

There is intentionally no unauthenticated “forgot password” web endpoint.

If a user is locked out of config-managed dashboard auth:

1. Run:

   ```bash
   llama-monitor --clear-auth-config
   ```

2. Restart llama-monitor.
3. Open the local dashboard.
4. Reconfigure `Dashboard Access` from the Security tab.

This keeps recovery local to the machine instead of exposing a network-visible reset action.

## Migration From Older Builds

Older builds only supported dashboard auth through startup flags.

On newer builds:

- If `--basic-auth` and/or `--form-auth` are present and compatible, llama-monitor seeds
  `auth-config.json` automatically on first run.
- Runtime behavior still honors the live startup flags first.
- Once users remove the flags, the stored config-backed auth can take over without a manual reset.

If `--basic-auth` and `--form-auth` use different credentials, the app does not auto-migrate them
into `auth-config.json` because the persisted dashboard-access UI uses a single shared account.

## Related Docs

- [CLI Flags](cli-flags.md)
- [API Reference](api.md)
- [TLS Architecture](tls-architecture.md)
