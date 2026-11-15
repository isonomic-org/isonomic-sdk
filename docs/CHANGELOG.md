# Changelog

Newest first. Each entry names what the engine (isonomic-capsule) and the window (isonomic-desktop) must do when they move `SDK_REF` to it.

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
