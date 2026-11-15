# SDK: status

Newest first. History before 14 Nov 2026 is in `isonomic-desktop/docs/architecture/sdk/status.md`; this repo's own session owns the SDK from here.

## 2026-11-15 · The `sql` interface (S0)

**Built:** `proto::sql` (`SqlValue`, `Rows`, `Column`, `Statement`), five methods and 17 fixture lines; `auth::authorize_schema`; `client::Sql` and `LiveQuery`; a scripted database in the testkit. 5 new integration tests, 2 unit tests; 75 in all.

**Decisions**

- **The testkit runs no SQL.** Its database is scripted per statement (`respond`, `affect`, `fail`) and logs what apps ran. A real engine in tests would tie apps to one SQL dialect's quirks; the script keeps app tests about the app.
- **Live queries send the whole result**, not a diff. Results are small (a thread list, a count); diffs can come later as an optional field.
- **Values are plain JSON** with bytes as `{"bytes":…}` and floats always written with a decimal point, so every value reads back as the kind it was.
- **One schema per call.** The app's first own schema is the default; reads may reach the schemas its manifest lists.

## 2026-11-15 · The `events` interface (S0)

**Built:** `proto::events` (`AppEvent`, topic rules), `Json` for payloads the SDK does not own, three methods and 16 fixture lines; `client::Events` and `EventStream`; a testkit channel per scope shared by every connection. 5 new integration tests, 2 unit tests; 68 in all.

**Decisions**

- **No capability.** An app reaches only its own scope, so there is nothing to grant; the brief's capability list has no events entry. Only the shell may name another app's scope, which `proto::auth` checks.
- **Live only, at most once.** Nothing is stored or replayed; durable state belongs in `sql` and `blobs`. This keeps the engine side a plain broadcast.
- **`Json` is a value, not raw text.** Raw JSON cannot pass through the flattened request and event envelopes, so payloads are parsed values; key order is not kept, and fixtures write keys sorted.

## 2026-11-15 · The `blobs` interface (S0)

**Built**

- `proto::blobs`: `BlobInfo`, `Bytes` (base64 on the wire, with its own encoder so `proto` gains no dependency), `CHUNK_BYTES`. Six methods, three replies, three error codes, 20 fixture lines.
- `proto::auth`: the blobs check, including the manifest's `max_bytes`.
- `client`: `Blobs` (`put`, `get`, `info`, `delete`) splits and joins chunks.
- `testkit`: an in-memory blob store, and per-interface `slow`, `fail` and `recover` for every interface.

**Verified:** fmt, clippy `-D warnings`, 8 new integration tests and 2 unit tests; 61 tests in all.

**Decisions**

- **Chunked, append-only writes.** A frame is 1 MiB, so one call carries 512 KiB at most; larger blobs are created with their size and written in order. Sequential offsets make a retried or lost write visible instead of silently corrupting the blob.
- **A handle belongs to its app.** Another app's handle answers `unknown_blob`, not `capability_denied`, so handles reveal nothing. The shell reads all blobs, to render them in the window.
- **Limits live in two places:** the manifest's `max_bytes`, checked in `proto::auth` so the testkit and the engine agree, and the engine's own limit on top.

## 2026-11-14 · The app crate and hello-app (S0, PR 3)

**Built**

- `isonomic-app`: the `App` trait exactly as specified (`manifest`, `build`, `open_link`, `search`, `on_unload`), `AppContext`, `Settings` over a `SettingsStore`, `Attention`, and the window side (`register`, `Registration`, `LoadedApp`, `Tab`).
- `testing::TestHost`: the in-memory engine on its own Tokio runtime, a token granted from the app's manifest, a headless GPUI window, and `wait_until`, which wakes on the view's notifications.
- `examples/hello-app`: lists models, streams one reply, raises attention when done, answers search and deep links. Four headless window tests.
- `docs/guide/build-an-app.md`.

**Verified**

- Locally on Linux, against the same GPUI snapshot the UI crate pins: fmt, clippy `-D warnings`, 51 tests (one is the xtask's), the suite green 10 times in a row before the xtask and 3 more after. An engine-style consumer of `proto`, `client` and `testkit` still builds with no `../isonomic-ui`.
- CI: `ci.yml` calls the shared workflow with its new `ui` input (isonomic-org/.github#1), GPUI's Linux packages, and `cargo xtask frozen-fixtures`, the append-only fixture check. The UI checkout needs the `UI_DEPLOY_KEY` secret (a read-only deploy key on isonomic-ui).

**Decisions**

- **Hooks reach the app through its root view.** `build` returns `AnyView`, as specified; when that view is `Entity<Self>`, the window downcasts it to call `open_link`, `search` and `on_unload`. No second handle is needed.
- **Settings live behind a `SettingsStore` the window supplies**, keyed by app id, values as plain JSON. There is no settings interface on the wire yet; when the engine stores them (over `sql` or its own interface), only the store changes, not apps.
- **The app crate depends on Isonomic UI with default features off.** It needs only GPUI's types; apps turn on the components they use.
- **The SDK applies the UI crate's patch for GPUI's macros**, so macro output resolves through Isonomic UI exactly as in the UI repo.
- **`Cargo.lock` is no longer committed.** With GPUI it is about 180 KB, too large for the write tooling (the inference repo made the same call). The UI repo's lockfile pins GPUI's own tree.
- **hello-app is a workspace member** at `examples/hello-app`, so CI builds and tests it with everything else.
- **The fixture guard is `cargo xtask frozen-fixtures`**, so it runs through the shared workflow's `xtask` input. On a pull request it fails if any existing line under `crates/proto/fixtures/` was changed or removed.

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
