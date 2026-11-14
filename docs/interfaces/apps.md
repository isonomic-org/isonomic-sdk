# `apps` interface

Shell only. Records a first-party app's approved manifest and issues the capability token the app's calls carry. An app token calling `apps.*` is refused with `capability_denied`.

## `apps.grant`

Params: `{ manifest }`. Reply: `granted { app, token }`.

```json
{"type":"request","id":10,"method":"apps.grant","params":{"manifest":{"id":"com.isonomic.hello","name":"Hello","version":"0.1.0","icon":"sparkle","capabilities":{"models":{"any":true}}}}}
{"type":"response","id":10,"result":{"granted":{"app":"com.isonomic.hello","token":"tok-hello-1"}}}
```

Granting the same app again replaces its manifest and keeps its earlier tokens valid. Tokens are meant to survive an engine restart (requested of the engine; see `docs/status.md`).

## The manifest

| Field | Meaning |
| --- | --- |
| `id` | Reverse-DNS, at least three parts: `com.isonomic.chat` |
| `name`, `version`, `icon` | Display name, `major.minor.patch`, an icon name from the Isonomic UI set |
| `capabilities` | What the app may reach; absent means denied (below) |
| `schemas` | SQL schemas the app creates: `[{"name":"chat"}]` |

| Capability | Shape | Grants |
| --- | --- | --- |
| `sql` | `{"schemas":["other_app"]}` | Read access to other apps' schemas (its own are implied) |
| `models` | `{"any":true}` or `{"ids":["chat-small"],"remote":true}` | The named models, or any; remote providers only with `remote` |
| `connections` | `[{"provider":"calendar"}]` | Linked accounts of those providers |
| `egress` | `["example.com"]` | Outbound HTTP to those hosts, once the user approves |
| `blobs` | `{"max_bytes":10485760}` | The blob store, up to that size per blob |
| `agents` | — | Reserved. A manifest that sets it is refused |

## Errors

| Code | When |
| --- | --- |
| `invalid_manifest` | The manifest breaks a rule in `Manifest::validate` |
| `capability_denied` | An app token called a shell-only method, or a method its manifest does not allow |
| `unknown_token` | The token was never issued or was revoked |

```json
{"type":"response","id":11,"error":{"code":"invalid_manifest","message":"version \"1.2\" is not major.minor.patch"}}
{"type":"response","id":12,"error":{"code":"capability_denied","message":"app com.isonomic.hello may not call system.shutdown"}}
```
