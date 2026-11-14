# SDK: status

Newest first. History before 14 Nov 2026 is in `isonomic-desktop/docs/architecture/sdk/status.md`; this repo's own session owns the SDK from here.

## 2026-11-14 · Tokens, `apps.grant`, tolerant decoding (S0, PR 1)

**Built**

- `proto::manifest`: `Manifest`, `Capabilities` (sql, models, connections, egress, blobs, agents reserved), `Token`, and `Manifest::validate`.
- `proto::auth`: `authorize(caller, call)`, shared by the testkit and the engine.
- `proto::lenient`: decoding that survives a newer peer, on both sides.
- `client`: `AppClient` (a client scoped to one token), generic subscriptions with events routed before the reply is delivered, skipping of unknown events.
- `testkit`: token enforcement on every call, `grant`, `app_client`, `revoke_all`, and a log of every call with its caller.
- Frozen fixtures for `system` and `apps`, and a CI check that refuses edits to existing fixture lines.

**Decisions**

- **A call without a token is the shell's.** The shell is the window on its own Unix socket (user-only permissions). When shell authentication lands with identity, `welcome` will carry a session token and tokenless calls will be refused. App tokens never reach `system` or `apps`.
- **The capability check lives in `proto`** (pure logic, no I/O) so the testkit and the engine cannot disagree.
- **Tokens should survive an engine restart** (derived from the manifest), so open tabs keep working. Filed as a request; the testkit already keeps earlier tokens valid when an app is granted again.
- **Manifests are validated by the engine**: reverse-DNS id, `major.minor.patch`, lowercase schema names, agents absent.
- **`Cargo.lock` stays committed** for now; it will be dropped when the app crate brings GPUI in (too large for the write tooling).
