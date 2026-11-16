# `identity` interface

The person's Isonomic ID, which the window shows. The engine creates it the first time it is needed (in practice at first run) and keeps it. Every example is a frozen fixture in `crates/proto/fixtures/v0/identity.jsonl`.

**Shell only.** `proto::auth` refuses it to every app token with `capability_denied`, whatever the app's capabilities. Apps never see the ID.

Version 1 carries only this. Grants, activity logs and keys are not on the wire.

## Methods

| Method | Params | Reply |
| --- | --- | --- |
| `identity.get` | `{}` | `identity { id, created }` |

- `id` is the form a person reads and copies. Treat it as opaque text: show it as given, and don't parse it.
- `created` is `true` only on the call that made the ID. Every later call, from any connection, answers `false` with the same `id`. The call is idempotent once the ID exists.

```json
{"type":"request","id":140,"method":"identity.get","params":{}}
{"type":"response","id":140,"result":{"identity":{"id":"isonomic:7Q4K-M2XD-9PLV","created":true}}}
{"type":"request","id":141,"method":"identity.get","params":{}}
{"type":"response","id":141,"result":{"identity":{"id":"isonomic:7Q4K-M2XD-9PLV","created":false}}}
```

## Errors

| Code | When |
| --- | --- |
| `identity_unavailable` | The engine could not make or read the ID (the key store refused, say). The `message` is plain words for the window to show as written. Nothing was made; try again later |
| `capability_denied` | An app token |

```json
{"type":"response","id":142,"error":{"code":"identity_unavailable","message":"Your Mac's keychain did not let Isonomic save your ID. Unlock it and try again."}}
```

## Client

`Client::identity()` returns `Identity { id, created }`. It's on the shell's `Client` only; `AppClient` has no such method.

## Testing

`TestEngine::identity()` scripts it:

- `set_id(id)` sets the ID; the default is `isonomic_testkit::DEFAULT_IDENTITY`.
- `refuse(message)` answers `identity_unavailable` with that message until `recover()`.
- `forget()` starts again as on a first run.
- `made()` says whether the ID exists.

The conformance check `identity.stable` asks the engine for the ID twice: it must be the same, non-blank, and `created: false` the second time.
