# isonomic-sdk

The contract between window and engine, and the platform every Isonomic app is built on. Status and decisions: `docs/status.md`. Original spec: `isonomic-desktop/docs/architecture/sdk/spec.md`.

## Rules

1. **The wire format is frozen by fixtures.** Every message shape is a line in `crates/proto/fixtures/v<version>/`. Add lines freely; never edit one (CI refuses). Changing a shape means a protocol version bump, and needs Omar's approval. See `docs/versioning.md`.
2. **No engine or window code here.** Types, the client, the test engine, and the app crate. Only `crates/app` and `examples/` may depend on Isonomic UI; `proto`, `client`, `testkit` and `conformance` must build without it.
3. **No polling.** The client publishes changes on a watch; callers await them.
4. **Hermetic tests only.** No socket, no engine binary, no network: use the test engine.
5. **Small files.** Every source and doc file stays under 20 KB.
6. **Names.** `ISONOMIC_` for environment variables, `com.isonomic.` for identifiers. No other product, repo or account name anywhere.
7. **Tokens everywhere.** Every app call carries its token; the testkit refuses what the manifest does not allow, through the same `proto::auth` check the engine uses.
8. **Every interface change** gets fixtures, a `docs/interfaces/<name>.md` update and a `docs/CHANGELOG.md` entry.

## Workflow

- One task per branch and pull request. Every PR ends with a status line: ready, blocked, failed or waiting. A merge needs green checks and Omar's go-ahead in chat.
- Before pushing: `cargo fmt --all`, `cargo clippy --all-targets --all-features -- -D warnings`, `cargo test --all-features`.
- After a merge here, bump `SDK_REF` in `isonomic-capsule` and `isonomic-desktop` to the new commit.
