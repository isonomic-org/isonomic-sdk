# SDK: status

Newest first. History before 14 Nov 2026 is in `isonomic-desktop/docs/architecture/sdk/status.md`; this repo's own session owns the SDK from here.

## 2026-11-14 · The `models` interface (S0, PR 2)

**Built**

- `proto::models`: `ModelInfo`, `ModelDetail`, `StreamRequest`, `Message`, `ToolSpec`, `ToolCall`, `ModelChunk`, `ToolCallDelta`, `Usage`, `StopReason`, `MemoryRefusal`. Five methods, five replies, one event, five error codes. Fixtures for all of them.
- `proto::auth`: the models check, plus `authorize_model` for the engine once it has chosen a default.
- `client`: `Models` and `ModelStream`; dropping an unfinished stream cancels it.
- `testkit`: a scriptable fake with two default models, scripted steps with gates, memory refusals, failures, the remote switch, per-token cancel ownership, and request and cancel logs.

**Decisions**

- **Chunks mirror the inference layer's** with app-facing names: `delta` is `text`, `finish_reason` is `stop` (`stop` → `end`, `length` → `max_tokens`). The reasoning field mirrors D4.
- **Tool schemas and arguments are JSON text**, not objects, so the types stay WIT-ready.
- **Capabilities in `ModelInfo` are strings**, as in the inference layer, so a newer engine's capabilities never fail to decode.
- **The memory refusal rides on the error** as an optional `memory` field rather than a new message kind; older clients ignore it.
- **One app cannot cancel another's stream.** Apps share the window's connection, so subscriptions are owned by the token that opened them.
- **`models.list` is filtered** to what the caller may use, so an app's model picker never shows a model it would be refused.

## 2026-11-14 · Tokens, `apps.grant`, tolerant decoding (S0, PR 1)

**Built**

- `proto::manifest`: `Manifest`, `Capabilities` (sql, models, connections, egress, blobs, agents reserved), `Token`, and `Manifest::validate`.
- `proto::auth`: `authorize(caller, call)`, shared by the testkit and the engine.
- `proto::lenient`: decoding that survives a newer peer, on both sides.
- `client`: `AppClient` (a client scoped to one token), generic subscriptions with events routed before the reply is delivered, skipping of unknown events.
- `testkit`: token enforcement on every call, `grant`, `app_client`, `revoke_all`, and a log of every call with its caller.
- Frozen fixtures for `system` and `apps`. The CI step that refuses edits to existing fixture lines is written but not yet in `.github/workflows/`: the connector cannot write workflow files (see PR 3).

**Decisions**

- **A call without a token is the shell's.** The shell is the window on its own Unix socket (user-only permissions). When shell authentication lands with identity, `welcome` will carry a session token and tokenless calls will be refused. App tokens never reach `system` or `apps`.
- **The capability check lives in `proto`** (pure logic, no I/O) so the testkit and the engine cannot disagree.
- **Tokens should survive an engine restart** (derived from the manifest), so open tabs keep working. Filed as a request; the testkit already keeps earlier tokens valid when an app is granted again.
- **Manifests are validated by the engine**: reverse-DNS id, `major.minor.patch`, lowercase schema names, agents absent.
- **`Cargo.lock` stays committed** for now; it will be dropped when the app crate brings GPUI in (too large for the write tooling).
