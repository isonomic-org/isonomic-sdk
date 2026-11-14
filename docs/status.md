# SDK: status

Newest first. History before 14 Nov 2026 is in `isonomic-desktop/docs/architecture/sdk/status.md`; this repo's own session owns the SDK from here.

## 2026-11-14 · The app crate and hello-app (S0, PR 3)

**Built**

- `isonomic-app`: the `App` trait exactly as specified (`manifest`, `build`, `open_link`, `search`, `on_unload`), `AppContext`, `Settings` over a `SettingsStore`, `Attention`, and the window side (`register`, `Registration`, `LoadedApp`, `Tab`).
- `testing::TestHost`: the in-memory engine on its own Tokio runtime, a token granted from the app's manifest, a headless GPUI window, and `wait_until`, which wakes on the view's notifications.
- `examples/hello-app`: lists models, streams one reply, raises attention when done, answers search and deep links. Four headless window tests.
- `docs/guide/build-an-app.md`.

**Verified**

- Locally on Linux, against the same GPUI snapshot the UI crate pins: fmt, clippy `-D warnings`, 50 tests, 10 consecutive green runs. An engine-style consumer of `proto`, `client` and `testkit` still builds with no `../isonomic-ui`.
- CI against the real Isonomic UI checkout is waiting on two things only Omar can do: the `UI_DEPLOY_KEY` secret, and moving `.github/ci-next.yml` to `.github/workflows/ci.yml` (the connector cannot write workflow files). The new workflow checks out the UI at `UI_REF`, installs GPUI's Linux packages, and adds the append-only fixture check.

**Decisions**

- **Hooks reach the app through its root view.** `build` returns `AnyView`, as specified; when that view is `Entity<Self>`, the window downcasts it to call `open_link`, `search` and `on_unload`. No second handle is needed.
- **Settings live behind a `SettingsStore` the window supplies**, keyed by app id, values as plain JSON. There is no settings interface on the wire yet; when the engine stores them (over `sql` or its own interface), only the store changes, not apps.
- **The app crate depends on Isonomic UI with default features off.** It needs only GPUI's types; apps turn on the components they use.
- **The SDK applies the UI crate's patch for GPUI's macros**, so macro output resolves through Isonomic UI exactly as in the UI repo.
- **`Cargo.lock` is no longer committed.** With GPUI it is about 180 KB, too large for the write tooling (the inference repo made the same call). The UI repo's lockfile pins GPUI's own tree.
- **hello-app is a workspace member** at `examples/hello-app`, so CI builds and tests it with everything else.

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
