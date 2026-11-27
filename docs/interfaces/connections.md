# `connections` interface

Shell only: the tool servers the person connects in Settings. A connection is a remote server the engine reaches at a URL, or a local one it starts as a command. The engine owns them: it mints their ids, keeps their sign-ins and secrets, and offers their tools to agent sessions. `proto::auth` refuses every method here to every app token, whatever its capabilities. Every example is a frozen fixture in `crates/proto/fixtures/v0/connections.jsonl`.

## Methods

| Method | Params | Reply |
| --- | --- | --- |
| `connections.list` | `{}` | `connections [Connection]` |
| `connections.add` | `{ name, key?, server, secret? }` | `connection Connection` |
| `connections.sign_in` | `{ id }` | `sign_in { url }`: the page to open in the person's browser. The outcome arrives as a `connection` event |
| `connections.remove` | `{ id }` | `done {}`. The engine stops the server and forgets its sign-in and secret |
| `connections.tools` | `{ id }` | `connection_tools [{ name, description }]` |

```json
{"type":"request","id":171,"method":"connections.add","params":{"name":"Design files","key":"design","server":{"kind":"remote","url":"https://tools.example.com/api"}}}
{"type":"response","id":171,"result":{"connection":{"id":"conn-1","name":"Design files","key":"design","server":{"kind":"remote","url":"https://tools.example.com/api"},"state":"needs_sign_in","tools":0}}}
```

## Shapes

- **`Connection`**: `id` (the engine's, stable), `name` (shown to the person), `key` (the window's own catalogue key, which picks the logo; the engine keeps it as given), `server`, `state`, `error` (with `failed`: plain words to show as given) and `tools` (how many it offers once ready).
- **`server`**, tagged by `kind`:
  - `{ "kind": "remote", "url" }`;
  - `{ "kind": "local", "command", "args": [] }`.
  - A kind from a newer engine decodes as `Server::Unrecognized`, so a list never fails over one.
- **`state`**:
  - `needs_sign_in`: call `connections.sign_in`.
  - `connecting`.
  - `ready`: its tools are available.
  - `failed`: see `error`.
  - A newer state decodes as `unrecognized`.
- **Credentials go only in `secret`.** A server's `url` is shown in replies, events and logs, so it must never hold a user name, password or key (no userinfo, no key in the query).
- **`secret`**: a bearer token or API key the person pasted, for a server that takes one instead of a browser sign-in. `ConnectionSecret`'s `Debug` prints `[secret]`, and its `Deserialize` refuses a malformed value in fixed text, never quoting it. Its `Serialize` is transparent, as the wire needs: never log a call as JSON. No reply ever carries a secret back.

## Changes

The engine sends a `connection` event, carrying the whole `Connection`, whenever one changes state: a sign-in that ends, a server that drops or recovers. It goes to the shell on its status subscription (`system.subscribe`), beside `service_status`, so the window needs no other subscription. An older client skips the event kind.

```json
{"type":"request","id":174,"method":"connections.sign_in","params":{"id":"conn-1"}}
{"type":"response","id":174,"result":{"sign_in":{"url":"https://tools.example.com/authorize?client_id=isonomic&state=4f2c"}}}
{"type":"event","subscription":7,"event":"connection","data":{"id":"conn-1","name":"Design files","key":"design","server":{"kind":"remote","url":"https://tools.example.com/api"},"state":"ready","tools":12}}
```

The client folds these events into `client.connections().changes()`, a watch of the newest state per id. `remove(id)` takes the id out of it once the engine answers, since no event says a connection is gone. It is emptied when the connection to the engine drops; call `list()` again after a reconnect.

## Errors

| Code | When |
| --- | --- |
| `capability_denied` | Any app token |
| `connection_not_found` | `sign_in`, `remove` or `tools` with an id the engine does not hold |
| `sign_in_failed` | The engine could not start a sign-in: a server with no browser sign-in, or one that did not answer. Plain words to show as given |
| `bad_request` | A blank name, or a server kind the engine does not know |

## Client

`client.connections()` gives `list()`, `add(AddConnection)`, `sign_in(id) -> SignInStarted`, `remove(id)`, `tools(id)` and `changes()`. Apps' clients have no such method.

## Testing

The test engine keeps connections in memory and touches no network. `add` answers `needs_sign_in` for a remote server without a secret and `ready`, with no tools, otherwise. `sign_in` of a remote one answers `SIGN_IN_PAGE` followed by the id (`http://127.0.0.1/sign-in/<id>`), then publishes the connection `ready`. A local one answers `sign_in_failed`. `remove` and `tools` refuse unknown ids.

`TestEngine::connections()` gives `list()`, `secret(id)` (what the shell sent), `set_state(id, state, error)` (published as an event), `set_tools(id, tools)` and `fail_sign_ins(Some(message))` (later sign-ins end `failed` with it). Wait for `client.live()` to hold a system snapshot before expecting events: that is when the status subscription is open.
