# Changelog

Newest first. Each entry names what the engine (isonomic-capsule) and the window (isonomic-desktop) must do when they move `SDK_REF` to it.

## Unreleased: capability tokens, `apps.grant`, tolerant decoding

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
