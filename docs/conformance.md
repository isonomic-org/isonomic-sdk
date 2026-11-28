# Conformance

`isonomic-conformance` is the SDK's contract as checks that run against any engine connection. The testkit passes them in this repo's CI; the engine runs the same checks against its own socket. When both pass, an app tested against the testkit behaves the same against the real engine.

## Running it

From a test in the engine, against a test engine (never a user's):

```rust
#[tokio::test(flavor = "multi_thread")]
async fn the_engine_conforms() {
    let engine = start_test_engine().await; // the engine's own helper
    let report = isonomic_conformance::run(isonomic_client::unix_connector(engine.socket())).await;
    println!("{report}");
    report.assert_passed();
}
```

`run_only(connector, |name| name.starts_with("blobs."))` runs a subset while an interface is being built. Each check has 10 s (`CHECK_LIMIT`); the report names every failure and why.

The checks speak the wire alone. They grant their own apps through `apps.grant` (ids under `com.isonomic.conformance`), store and delete small blobs, publish events, add and remove one tool server connection, link and unlink one empty folder, and finish by asking the engine to shut down with an app's token, which it must refuse. They need no engine-side setup. Whatever needs scripted engine state, such as a model's output or a query's rows, is tested in the testkit's own tests instead.

## The checks

| Check | The engine must |
| --- | --- |
| `wire.welcome` | Answer `hello` with `welcome`, the highest version both speak, and the agent session schema it writes |
| `wire.unsupported_version` | Answer `unsupported_version` when there is none |
| `wire.hello_required` | Answer a request sent before `hello` with `hello_required` and its id |
| `wire.unknown_method` | Fail an unknown method alone with `unknown_method`; the connection goes on |
| `wire.unknown_fields` | Ignore fields it does not know, at the top and in `params` |
| `wire.bad_frame` | Answer a frame that is not a message with `bad_request`, id 0; the connection goes on |
| `identity.stable` | Answer `identity.get` with the same non-blank ID every time, on a second connection too, and `created: false` after the first |
| `identity.kit_apps_refused` | Refuse `identity.kit_export`, `kit_restore`, `kit_saved` and `kit_status` to an app holding every capability, with `capability_denied` |
| `identity.kit_export_needs_person` | Refuse the shell's `identity.kit_export` without `person_asked` with `recovery_refused`. No check ever exports a kit |
| `connections.round_trip` | List a connection the shell adds (a remote server at an unused local port) with its name and key, drop it on `connections.remove`, and refuse a second remove with `connection_not_found`. No check signs in |
| `connections.unknown_id` | Refuse `connections.sign_in`, `remove` and `tools` of an id it never minted with `connection_not_found` |
| `models.catalogue_local` | List only local models in `models.catalogue`, each once |
| `models.remove_unknown` | Refuse `models.remove` of a model it does not know with `unknown_model` |
| `tokens.grant` | Grant a valid manifest, including one with the `agents` capability; refuse a bad version or a bad id with `invalid_manifest` |
| `tokens.unknown_token` | Refuse a token it never issued with `unknown_token`, on any interface |
| `tokens.shell_only` | Refuse every `system`, `apps`, `identity` and `connections` method, and `models.download`, `download_cancel`, `recommended`, `catalogue` and `remove`, to an app holding every capability |
| `tokens.every_interface` | Refuse every method of `system`, `apps`, `identity`, `connections`, `models`, `blobs`, `sql` and `agents` to an app with no capabilities. The calls come from the frozen fixtures, and the check fails if a method has none |
| `tokens.named_model` | Refuse a model outside the manifest's list with `capability_denied` |
| `tokens.other_scope` | Refuse an app publishing to, or listening on, another app's events |
| `blobs.round_trip` | Store, read back, describe and delete a blob; then `unknown_blob` |
| `blobs.chunked` | Take a blob larger than one chunk in pieces and return it whole |
| `blobs.private` | Answer another app's reads and deletes of a handle with `unknown_blob` |
| `blobs.max_bytes` | Refuse a blob over the manifest's `max_bytes` with `too_large`, in one call or announced up front |
| `events.round_trip` | Deliver an app's events to its own subscribers with the sender; keep topic filters |
| `events.scoped` | Never deliver another app's events, even on the same topic |
| `events.shell_reaches_app` | Deliver the shell's event to the app whose scope it names |
| `subscriptions.owner_only` | Let only the token that opened a subscription end it, even on the same connection |
| `sql.schemas` | Run reads in the app's own schema; refuse writes to listed schemas, anything in other schemas, and an app with no schema; make the shell name a schema |
| `agents.presets` | List and read the built-in `chat` preset; refuse invalid preset text with `invalid_preset` |
| `agents.registry` | Show a created session in `sessions.list`, and a rename, an archive and a restore in the next read |
| `agents.replay` | Start a session's log with `session.start` at seq 1; replay from a seq starting at the next event |
| `agents.isolated` | Answer another app's open, subscribe, rename and send with `not_found`; refuse purge and `approvals.list` naming another app with `capability_denied`; leave its sessions out of the list |
| `agents.purge_own` | Delete the caller's sessions on purge and leave every other app's |
| `agents.model_rules` | Refuse with `capability_denied` a session on a model outside the app's manifest, whether named on `sessions.create` or reached through a preset the app stored (its reply model or its title model) |
| `agents.session_ended` | End a session's stream with `session_ended` when the session is purged |
| `agents.unknown_subscription` | Refuse unsubscribing from a stream that is not open with `unknown_subscription` |
| `permissions.shell_only` | Refuse `permissions.grant` and `permissions.revoke` to an app whose manifest names the kit, with `capability_denied`, and record nothing |
| `permissions.scoped` | List a folder and a kit the shell granted one app in that app's list only; refuse an app naming another with `capability_denied`; answer a folder id under another app's name with `not_found` |
| `permissions.request_needs_person` | Refuse `permissions.request` without a user-action token, or with one never minted, with `missing_user_action`, and leave the kit `not_asked` |
| `tokens.shutdown_refused` | Refuse `system.shutdown` to an app, and keep answering |

`permissions.scoped` links one empty folder that it makes under the system's temporary directory and removes again; the engine under test must be able to link a folder there.

Three checks the tool kits design names are not here yet: `permissions.revoked` (after a revoke, the next read is refused), `kits.remember` and `kits.offered`. Each needs a kit tool called in a session. The harness this SDK pins has no kit tools yet, and no check sends input today, so they wait for the files kit and for a way to script a tool call over the wire.

## How we know the checks bite

`crates/conformance/tests/testkit.rs` also runs every check through a proxy that strips the token from each request, an engine that ignores tokens. The token, privacy, scope and permissions checks must fail there, and the wire checks must still pass.

## Adding a check

A new interface brings its checks with it: add a function returning `Outcome` to the module for its group, list it in `CHECKS`, and add its row here. A new method needs no new token check: `tokens.every_interface` reads it from its fixture.
