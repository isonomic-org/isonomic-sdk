# Versioning

## The wire

- **Protocol version 0** is spoken today. The first message, `hello`, lists the versions a client speaks; `welcome` names the one chosen. The engine supports the current version and the previous one.
- **Every message shape is frozen** by a fixture: one line of JSON in `crates/proto/fixtures/v<version>/<interface>.jsonl`, exactly as it travels. A test decodes and re-encodes every line and requires the same bytes.
- **Additive by default.** New methods, replies, events, error codes and optional fields are added without a version bump. New fixture lines may be added freely.
- **Unknown is not fatal.** Unknown fields are ignored. An unknown error code decodes as `unrecognized`. A client skips an event kind it does not know; a reply kind it does not know fails only that call. An engine answers a method it does not know with `unknown_method` and the request's own id (`proto::lenient`).
- **A shape change is a version bump.** CI refuses any edit to an existing fixture line. To change a shape: raise `PROTOCOL_VERSION`, add `fixtures/v<new>/`, keep the old version decodable in the engine, and announce it in the changelog.

## The Rust API

- `Call`, `Reply`, `Event`, `ErrorCode` and `auth::Interface` are `#[non_exhaustive]`: a `match` on them needs a `_` arm, so new variants do not break consumers.
- A change that does break Rust code (a new field on a struct a consumer builds by hand, say) is listed under **Breaking for Rust** in the changelog, with the fix.

## Releases

Every merged change that other repos see gets a `CHANGELOG.md` entry naming the commit to put in `SDK_REF`.
