# Changelog

Newest first. Each entry names what the engine (isonomic-capsule) and the window (isonomic-desktop) must do when they move `SDK_REF` to it.

## Unreleased: `test-support` builds outside this workspace

**Wire:** no change.

**For apps:** `isonomic-app`'s `test-support` feature now turns on Tokio's multi-thread runtime, which `TestHost` uses. An app crate outside this workspace no longer needs to add it (requested by isonomic-chat, O6).
## Unreleased: the testkit's agent host, and the `agents.` conformance group

**Wire:** no change.

**Build:** the testkit now depends on `harness-host` (isonomic-harness, the same pinned commit as `harness-core`).

**For the engine:** run the conformance suite's new `agents.` checks against the engine host (`docs/conformance.md`). They need no model: none sends input, and each purges what it created.

**For the window and apps:** agents calls in tests now work against `TestEngine` instead of answering `unknown_method`. `TestEngine::agents()` scripts the model, the clock and the person (`docs/interfaces/agents.md`, Testing).

## Unreleased: the `agents` interface (wire and client)

**Build:** the proto now depends on `harness-core` (isonomic-harness, pinned by commit in the workspace `Cargo.toml`) for the agent session event types it re-exports. Cargo fetches it over SSH. Every repository that builds the proto needs read access to isonomic-harness: in CI, add `harness: true` to the shared workflow call (needs the `HARNESS_DEPLOY_KEY` org secret).

**Wire (additive, still version 0)**

- 22 methods in `sessions.*`, `approvals.*`, `questions.answer`, `tools.*`, `presets.*` and `artifacts.*` (`docs/interfaces/agents.md`); replies `session_created`, `sessions`, `session_snapshot`, `done`, `accepted`, `purged`, `session_stream`, `approvals`, `presets`, `preset`, `preset_saved`, `artifact`, `artifacts`; events `session_event` and `live_delta`; error codes `not_found`, `invalid_preset`, `class_not_supported`, `not_at_turn_boundary`, `continue_refused`, `not_allowed_to_decide`, `missing_user_action`, `already_decided`, `session_failed`, `invalid_tools`.
- `approvals.decide`, `questions.answer` and `tools.result` name the session as well as the id: ids are minted per session. `sessions.purge`, `approvals.list` and `presets.list` take `params` (`{}` is fine); purge and approvals.list take an optional `app`, which only the shell may set to another app (`capability_denied`, checked in `proto::auth`); the shell must name the app it purges (`bad_request` otherwise). `Agents::purge(app)` and `Agents::approvals(app)` pass it.
- `welcome` gains `schema { version, revision }`, the session event schema the engine writes. Optional: an older engine or client simply leaves it out.
- The `agents` capability is open: a manifest may set it (`AgentsAccess {}`), and `ManifestError::AgentsReserved` is gone.

**For the engine** (Part A of the agents brief): serve each method by calling harness-host after checking the token, mapping the SDK's argument and result types to the host's and `HostError` to the error codes above (`Unauthorized` → `capability_denied`, `Store` → `internal`, `Model` → `model_unavailable`). Put `SchemaVersion::CURRENT` in `welcome`.

**For the window:** `Client::agents()` and `AppClient::agents()`: every call, and `subscribe` → `SessionStream` (`next`, `last_seq`), which unsubscribes when dropped. `Live::schema`. `Reply::SessionSnapshot`, `Reply::Preset` and `Reply::Artifact` are boxed.

## Unreleased: Isonomic UI at 4c912f6

**Wire:** no change.

**For the engine and the window:** move `../isonomic-ui` to `4c912f61a1ea80cdf10d369fe7cf013ddbc2228a`, the commit in `UI_REF`. Every V1 repo pins this commit.

## Unreleased: `system.retry`

**Wire (additive, still version 0):** method `system.retry { service }`, reply `retrying {}`, error code `not_degraded`. Requested by the Desktop session (isonomic-sdk #14).

**For the engine:** on `system.retry`, refuse an unknown service (`unknown_service`) or one that is not degraded (`not_degraded`); otherwise call the supervisor's `reset()` and start the service, answering `retrying {}`. The outcome goes out as the usual `service_status` events. `proto::auth` already refuses app tokens.

**For the window:** `Client::retry(service)` for the Retry action on a degraded service in the logs view. Show the result from `Client::live()`, not from the reply.

## Unreleased: the conformance crate

**Wire:** no change.

**For the engine:** add `isonomic-conformance` as a dev-dependency and run `isonomic_conformance::run(unix_connector(<test socket>))` in a test (`docs/conformance.md`). Every check that fails names the rule it broke. `run_only` takes a filter, for interfaces still being built.

**For the window:** nothing to change. `Client::live()` now documents that a borrow of the live view must not be held: while one is, the client cannot update the view and calls wait.

## Unreleased: the `sql` interface

**Wire (additive, still version 0)**

- Methods `sql.query`, `sql.execute`, `sql.batch`, `sql.subscribe`, `sql.unsubscribe` (`docs/interfaces/sql.md`); replies `rows`, `executed`, `executed_batch`, `live_query`; event `rows_changed`; error code `sql_error`.

**For the engine** (filed in isonomic-capsule `docs/requests.md`, R5): a role per app over its own schemas plus read-only grants on the schemas its manifest lists, chosen with `proto::auth::authorize_schema`; positional parameters; batches in one transaction; live queries re-run when a write touches their tables, pushing the whole new result.

**For the window:** `Client::sql()` and `AppClient::sql()`: `query`, `execute`, `batch`, `in_schema`, and `live` → `LiveQuery` (`rows`, `changed`), which unsubscribes when dropped.

## Unreleased: the `events` interface

**Wire (additive, still version 0)**

- Methods `events.publish`, `events.subscribe`, `events.unsubscribe` (`docs/interfaces/events.md`); replies `published`, `listening`; event `app_event`.
- `Json`: any JSON value inside a message, for payloads the SDK does not own.

**Breaking for Rust:** `Event` no longer implements `Eq` (it carries `Json`). `PartialEq` remains.

**For the engine** (filed in isonomic-capsule `docs/requests.md`, R4): one live channel per app scope across every connection, at most once, in order; only the shell may name another app's scope (already checked by `proto::auth`); an unsubscribe is honoured only from the token that subscribed.

**For the window:** `Client::events()` and `AppClient::events()`; `EventStream` unsubscribes when dropped.

## Unreleased: the `blobs` interface

**Wire (additive, still version 0)**

- Methods `blobs.put`, `blobs.create`, `blobs.write`, `blobs.get`, `blobs.info`, `blobs.delete` (`docs/interfaces/blobs.md`). Bytes travel as base64, at most `CHUNK_BYTES` (512 KiB) per call.
- Replies `blob`, `blob_data`, `deleted`. Error codes `unknown_blob`, `too_large`, `incomplete`.

**For the engine** (filed in isonomic-capsule `docs/requests.md`, R3)

- Store blobs per app; another app's handle answers `unknown_blob`. The shell reads all.
- `proto::auth::authorize` already refuses apps without the capability and blobs over the manifest's `max_bytes`; apply the engine's own limit too (`auth::too_large`).
- Writes are append-only: `offset` must equal the bytes written so far. A blob is readable once complete.

**For the window**

- `Client::blobs()` and `AppClient::blobs()`: `put`, `get`, `info`, `delete`, with chunking done for you.

**For tests:** `TestEngine::blobs()`, and `slow` / `fail` / `recover` for any interface.

## Unreleased: the app crate and hello-app

**Wire:** unchanged.

**New**

- `isonomic-app`: the `App` trait, `AppContext` (scoped `AppClient`, `Settings`, `Attention`), `register` / `Registration` / `LoadedApp` / `Tab` for the window, and `testing::TestHost` (feature `test-support`).
- `examples/hello-app` and its headless window tests.
- `UI_REF`: the Isonomic UI commit this repo builds against, checked out as `../isonomic-ui`.

**For the window**

- Link app crates and register them with `isonomic_app::register::<App>()`. Grant each manifest, build `AppContext::new(id, client.scoped(token), settings)`, open with `Registration::open`, and watch the attention receiver for the rail dot (`docs/guide/build-an-app.md`, section 6).
- Supply a `SettingsStore`; `MemorySettings` will do until the engine stores app settings.
- Keep `../isonomic-ui` at the commit in this repo's `UI_REF` (W0 pins the same way).

**For the engine:** nothing. `proto`, `client` and `testkit` still build without the UI checkout.

**For CI:** `ci.yml` calls the shared workflow with `ui: true` (isonomic-ui at `UI_REF`, through the `UI_DEPLOY_KEY` secret), GPUI's Linux libraries, and `cargo xtask frozen-fixtures`, the append-only fixture check. `Cargo.lock` is no longer committed.

## The `models` interface

**`SDK_REF`:** `2d7b9760323a9d5587712add4d8fe07a094fca15` (#4, merged 14 Nov 2026).

**Wire (additive, still version 0)**

- Methods `models.list`, `models.info`, `models.stream`, `models.cancel`, `models.embed` (`docs/interfaces/models.md`).
- Replies `models`, `model`, `stream`, `cancelled`, `embeddings`; event `model_chunk` (text, reasoning, tool-call deltas, usage, stop).
- `error` may carry `memory` (`required_bytes`, `available_bytes`, `reason`) with the new code `insufficient_memory`. Also new: `unknown_model`, `unsupported`, `model_unavailable`, `remote_disabled`.

**Breaking for Rust**

- `Call`, `ClientMessage`, `Reply`, `Outcome` and `ServerMessage` no longer implement `Eq` (temperatures and vectors are floats). `PartialEq` remains.
- `WireError` has a `memory` field: build errors with `WireError::new(code, message)` or `WireError::memory(refusal)`.

**For the engine** (filed in isonomic-capsule `docs/requests.md`, R2)

- Implement the five methods in the model gateway. After choosing a default model, call `proto::auth::authorize_model(caller, model, remote)`.
- Send the `stream` reply before the first chunk, and end every stream with exactly one chunk that has `stop`.
- Only the token that opened a stream may cancel it (apps share the window's connection).

**For the window**

- `Client::models()` and `AppClient::models()`; `ModelStream` (`next`, `cancel`, `text`; dropping it cancels).

## Capability tokens, `apps.grant`, tolerant decoding

**`SDK_REF`:** `32c09ffa29dd11ea0a25c072978b31f6c2b3b794` (#3, merged 14 Nov 2026). Engine requests: isonomic-capsule `docs/requests.md`, R1.

**Wire (additive, still version 0)**

- `request` takes an optional `token`. Without one, the call is the shell's. With one, it is the app's, checked against its manifest.
- New method `apps.grant { manifest }` (shell only), answered `{"granted":{"app":…,"token":…}}`.
- New error codes: `unknown_method`, `unknown_token`, `capability_denied`, `invalid_manifest`. Codes a build does not know decode as `unrecognized`.
- Fixtures for every message in `crates/proto/fixtures/v0/`.

**Breaking for Rust**

- `ClientMessage::Request` has a `token` field: write `token: None` or `..` in patterns.
- `Call`, `Reply`, `Event` and `ErrorCode` are `#[non_exhaustive]`: add a `_` arm. The broker should answer an unhandled call with `ErrorCode::UnknownMethod`.
- `Call::AppsGrant` carries a `Box<Manifest>`.

**New for the engine**

- `proto::auth::authorize(caller, &call)`: the same check the testkit applies. Call it for every request.
- `proto::lenient::decode_client(body)`: read a frame with `io::read_frame`, then decode it with this, so an unknown method is answered with its own id instead of id 0.
- `WireError::new(code, message)`.

**New for the window**

- `Client::grant(manifest)` and `Client::scoped(token)`, which returns the `AppClient` an app is given.
- `Client::subscribe(call)` and `Subscription`, for any call whose reply opens a subscription.
