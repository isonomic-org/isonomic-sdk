# isonomic-sdk

The one contract between the Isonomic window and the Isonomic engine, over a Unix socket.

| Crate | What it is |
| --- | --- |
| `isonomic-proto` | Versioned wire types. Wire format v0 is length-prefixed JSON, so traffic is readable. Manifests, capability tokens and the shared capability check. Also the rule for where a Capsule and its socket live. |
| `isonomic-client` | Async client: one multiplexed connection, reconnect, a live status view that callers await instead of polling, subscriptions, and the token-scoped `AppClient` an app is given. |
| `isonomic-testkit` | An in-memory engine that speaks the real protocol and enforces tokens, with a scriptable fake per interface, for tests. |
| `isonomic-app` | The `App` trait every first-party app implements, the `AppContext` it is given, registration for the window, and a headless test host. The only crate that depends on Isonomic UI. |
| `examples/hello-app` | The smallest app: lists its models and streams one reply, tested in a headless window. |

## Docs

- `docs/guide/build-an-app.md`: the manifest, the trait, the scoped client, testing, registering in the window.
- `docs/interfaces/<name>.md`: every method, event and error, with real JSON.
- `docs/versioning.md`: what may change without a version bump, and how fixtures freeze the rest.
- `docs/CHANGELOG.md`: what each change means for the engine and the window.
- `docs/status.md`: what is built and every decision, newest first.

The SDK's original spec is `isonomic-desktop/docs/architecture/sdk/spec.md`.

## Develop

```
cargo fmt --all
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
```

`isonomic-capsule` and `isonomic-desktop` use this repo as a sibling checkout (`../isonomic-sdk`). Their CI checks it out at the commit in their `SDK_REF` file.

This repo pins Isonomic UI the same way: a sibling checkout (`../isonomic-ui`) at the commit in `UI_REF`. Only the app crate and the example need it; the engine builds `proto`, `client` and `testkit` without it. On Linux, GPUI needs the system packages listed in `.github/workflows/ci.yml`.
