# isonomic-sdk

The contract between window and engine. The spec is `isonomic-desktop/docs/architecture/sdk/`.

## Rules

1. **The wire format is frozen by tests.** The exact JSON of every message kind is asserted in `proto`. Changing one of those strings means changing the protocol version, and needs Omar's approval.
2. **No engine or window code here.** Types, the client and the test engine only.
3. **No polling.** The client publishes changes on a watch; callers await them.
4. **Hermetic tests only.** No socket, no engine binary, no network: use the test engine.
5. **Small files.** Every source and doc file stays under 20 KB.
6. **Names.** `ISONOMIC_` for environment variables, `com.isonomic.` for identifiers. No other product, repo or account name anywhere.

## Workflow

- One task per branch and pull request. A merge needs green checks and Omar's approval.
- Before pushing: `cargo fmt --all`, `cargo clippy --all-targets --all-features -- -D warnings`, `cargo test --all-features`.
- After a merge here, bump `SDK_REF` in `isonomic-capsule` and `isonomic-desktop` to the new commit.
