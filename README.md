# isonomic-sdk

The one contract between the Isonomic window and the Isonomic engine, over a Unix socket.

| Crate | What it is |
| --- | --- |
| `isonomic-proto` | Versioned wire types. Wire format v0 is length-prefixed JSON, so traffic is readable. Also the rule for where a Capsule and its socket live. |
| `isonomic-client` | Async client: one multiplexed connection, reconnect, and a live status view that callers await instead of polling. |
| `isonomic-testkit` | An in-memory engine that speaks the real protocol, for tests. |

Spec and status live in `isonomic-desktop/docs/architecture/sdk/`.

## Develop

```
cargo fmt --all
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
```

`isonomic-capsule` and `isonomic-desktop` use this repo as a sibling checkout (`../isonomic-sdk`). Their CI checks it out at the commit in their `SDK_REF` file.
