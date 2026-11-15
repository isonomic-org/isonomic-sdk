# SDK: status

Newest first. History before 14 Nov 2026 is in `isonomic-desktop/docs/architecture/sdk/status.md`; this repo's own session owns the SDK from here.

## 2026-11-15 · Isonomic UI and the shared toolkits through the SDK (decision, 15 Nov)

**Built:** `isonomic_app::ui` re-exports Isonomic UI; `editor` and `node-graph` features reserve empty `isonomic_app::editor` and `isonomic_app::node_graph`. `hello-app` imports only from the SDK and no longer depends on isonomic-ui. A scratch crate outside the workspace, depending on `isonomic-app` alone, derives components in two modules and runs `#[gpui::test]`.

**Decisions**

- **Imported as `gpui`.** The UI's macros expand to `gpui::` paths when the calling crate does not name isonomic-ui, so `use isonomic_app::ui as gpui;` satisfies them, one `use` per module. Teaching the macros to find `isonomic-app` would drop that line; it is a change in isonomic-ui, not filed yet.
- **One pin.** The SDK's `UI_REF` is the only one. Consumers still need the UI's source at that commit beside the SDK, and the `gpui-pre-macros` patch line in their workspace root.
## 2026-11-15 · The testkit's agent host and the `agents.` conformance group (S0, agents brief Part B)

**Built:** `testkit::agents` serves all 22 agents methods with harness-host's `MemoryHost`, plus `agents_map` (SDK types ⇄ host types, `HostError` → error codes). `TestEngine::agents()` → `FakeAgents` (`reply`, `with`, `advance`, `user_action`, `disconnect`, `reconnect`, `log`). There are 9 testkit tests, and 6 new conformance checks (`agents.presets`, `registry`, `replay`, `isolated`, `purge_own`, `unknown_subscription`). The token-stripping engine fails `agents.isolated`. No proto change.

**Decisions**

- **Each call drives the host to completion.** `MemoryHost` has no runtime; the testkit runs it under one lock with `block_on`, so a scripted turn is over when `send` answers. Streams are Tokio tasks awaiting `Subscription::next`, which the host wakes.
- **Every granted app is first-party.** Manifests carry no app kind yet. View and third-party deciding stays tested in harness-host.
- **Conformance sends no input.** The engine's model is real, so turns stay in testkit tests.

**For consumers** (review of #16, items 10 and 11):

- `wire.welcome` requires `schema`. An engine fails conformance on its next SDK pin bump until its `welcome` sends `SchemaVersion::CURRENT`.
- Declare harness with exactly the workspace's URL and `rev` string (`ssh://git@github.com/isonomic-org/isonomic-harness.git`, `7a73ca695254d415332ef39bae2b100e4f791b6c`). Any other spelling makes cargo build two copies of harness-core, whose types do not match. CI must pass `harness: true` to the shared workflow.

**Gaps, for the next pull request** (review items 6 to 9; 6 to 8 change the wire):

- A purged session's stream goes quiet: no end-of-stream event for session streams.
- No call gives an app the user-action token `approvals.decide` needs.
- `artifacts.get` returns the body inline; bodies over about 750 KB exceed the 1 MiB frame. It should be chunked like blobs.
- The client skips a malformed known event silently, and `SessionStream` does not check sequence continuity.
- Until isonomic-harness #30, the testkit checks the (session, id) target itself.

## 2026-11-15 · The `agents` interface: wire and client (S0, agents brief Part B)

**Built:** `proto::agents` (argument and result types; harness-core's event schema re-exported as `agents::schema`), 22 methods, 72 fixture lines in `agents.jsonl` (every agents error code, and `welcome.schema`) including an event from a newer schema revision, the `schema` field in `welcome`, the `Agents` interface in `proto::auth`, `client::Agents` and `SessionStream`. The method tables moved to `proto/src/calls.rs` to keep `lib.rs` under the size limit. 84 tests.

**Decisions**

- **harness-core by git, pinned.** The brief asks for the event types at a pinned revision; a git dependency keeps one copy of them for every app. It needs SSH access to isonomic-harness wherever the proto builds (the shared workflow's new `harness` input; isonomic-org/.github#4).
- **The SDK's own argument types.** The host's types are not serializable and pull in the loop and policy crates; the proto keeps apps on harness-core alone, and the engine maps.
- **Targets are (session, id).** Approval, question and call ids are per session in the harness, so the calls that answer one also name the session (review of #16; the host side is isonomic-harness #30).
- **Every call takes params.** `sessions.purge`, `approvals.list` and `presets.list` take a struct that accepts `{}`, so a filter can be added without a new method. Purge and approvals.list take `app`, named like `events`' scope: only the shell names another app (checked in `proto::auth`), and the shell's purge must name one.
- **The schema is published, not frozen.** harness schema v1 can still change; a re-pin of `harness-core` may need fixture edits, which means a new protocol version once the agents lines are on main.
- **Unknown stays readable.** A session event or block from a newer revision decodes as `Unknown` (harness-core's catch-all), and a fixture proves it.

**Next:** the testkit's host is harness-host's `MemoryHost`, and conformance gains an `agents.` group, in the next pull request.

## 2026-11-15 · `system.retry` (Desktop request #14)

**Built:** `system.retry { service }` → `retrying {}`, error `not_degraded`; `Client::retry`; the testkit records retries (`retries()`) and scripts their outcome (`on_retry`). Three fixture pairs; 3 new tests; 83 in all. Conformance covers the app-token refusal through the fixture.

**Decision:** retrying a service that is not degraded is refused with `not_degraded`, not answered as a no-op, so a stale Retry button shows an honest error.

## 2026-11-15 · Conformance (S0)

**Built:** `isonomic-conformance`: 22 checks over the wire (handshake and tolerance, tokens, blobs, events, subscriptions, sql schemas), `run`, `run_only` and a `Report`. The testkit passes all 22. A proxy that strips tokens fails every token, privacy and scope check, so the checks bite. 4 integration tests and a doctest; 80 in all.

**Decisions**

- **The wire alone.** Checks grant their own apps through `apps.grant` and assert only what any engine must do, so the engine runs them unchanged. Scripted behaviour (model output, query rows) stays in the testkit's tests.
- **Token checks read the fixtures.** `tokens.every_interface` sends every fixture call with a capability-free token and fails if a method has no fixture, so a new method is covered when its fixture lands.
- **Shutdown last.** An app asking the engine to stop is the most dangerous refusal; it runs at the end, then checks the engine still answers.

**Found:** binding the borrow `watch::Receiver::wait_for` returns blocks the client's actor, and every call then waits. The conformance runner hit it; `Client::live()` now says so.

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
