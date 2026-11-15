# `agents` interface

Agent sessions the engine's host runs for the app: Chat's conversations first, then every assistant panel. Each method maps one to one onto a harness-host call (isonomic-harness `docs/host-api.md`). Every example is a frozen fixture in `crates/proto/fixtures/v0/agents.jsonl`.

Needs the `agents` capability. The host decides what an app reaches: its own sessions, presets and artifacts only. Another app's ids answer `not_found`, exactly as if they did not exist. The shell reaches every app's.

## Types

The session event schema is harness-core's, re-exported as `isonomic_proto::agents::schema` at the commit the SDK pins (`harness-core` in the workspace `Cargo.toml`). Apps import only the SDK. `welcome` carries the schema the engine writes, `{ version, revision }`; it shows up in `Client::live().schema`. A newer revision of the same version still reads: an unknown event or block type decodes as `Unknown` and should be rendered generically.

Arguments and results (`CreateSession`, `SessionSummary`, `PendingApproval`, …) are the SDK's own, in `isonomic_proto::agents`.

## Methods

| Method | Params | Reply |
| --- | --- | --- |
| `sessions.create` | `{ preset, title?, location_opt_in? }` | `session_created { session }` |
| `sessions.list` | `{ archived?, preset?, text?, cursor?, limit? }` | `sessions { items: [SessionSummary], next? }` |
| `sessions.open` | `{ session }` | `session_snapshot { summary, turn_open?, approvals, questions, awaiting_client }` |
| `sessions.fork` | `{ session, at: {before_turn\|after_turn: turn}, reason? }` | `session_created { session }` |
| `sessions.archive` | `{ session, archived }` | `done {}` |
| `sessions.rename` | `{ session, title }` | `done {}` |
| `sessions.send` | `{ session, content: [Block], client_ref? }` | `accepted { queued }` |
| `sessions.continue` | `{ session }` | `done {}`; extends the last sealed partial reply |
| `sessions.cancel` | `{ session }` | `done {}`; safe to repeat |
| `sessions.purge` | — | `purged { sessions, events, blobs }`: every session of the calling app |
| `sessions.subscribe` | `{ session, from_seq? }` | `session_stream { subscription }`, then `session_event` and `live_delta` events |
| `sessions.unsubscribe` | `{ subscription }` | `unsubscribed {}` |
| `approvals.list` | — | `approvals [{ session, app, request }]` |
| `approvals.decide` | `{ approval, decision: allow\|deny\|{text}, user_action? }` | `done {}` |
| `questions.answer` | `{ question, selected?, text?, skipped? }` | `done {}` |
| `tools.register_client` | `{ session, specs: [ToolSpec] }` | `done {}`; replaces the app's client tools for that session |
| `tools.result` | `{ call, content?, error? }` | `done {}` |
| `presets.list` | — | `presets [{ reference, owner?, name, diagnostics }]` |
| `presets.get` | `{ preset }` | `preset { reference, owner?, text, diagnostics }` |
| `presets.put` | `{ text }` (TOML) | `preset_saved { reference }` |
| `artifacts.get` | `{ artifact, version? }` | `artifact { meta, bytes }` (bytes in base64) |
| `artifacts.list` | `{ session }` | `artifacts [ArtifactEvent]` |

```json
{"type":"request","id":86,"token":"tok-chat-1","method":"sessions.send","params":{"session":"s-1","content":[{"type":"text","text":"Plan three days in Lisbon"}]}}
{"type":"response","id":86,"result":{"accepted":{"queued":false}}}
{"type":"event","subscription":21,"event":"live_delta","data":{"session":"s-1","turn":"t-3","step":"p-1","after_seq":12,"block":0,"kind":"text","text":"Sintra is "}}
```

## Events

A subscription first replays the durable events after `from_seq`, then sends new ones as they are written, with no gap or duplicate at the seam. Each `session_event` is one `SessionEvent` (`session`, `seq`, `at`, `turn?`, `step?`, then `type` and its fields). To resume after a reconnect, subscribe from the last `seq` seen (`SessionStream::last_seq`).

`live_delta` events carry reply text as it streams. They are lossy and may be dropped under load; the durable `assistant` event that follows is the truth, so drop a step's deltas when it arrives.

A client tool call arrives as a `tool.call` event with `kind: client`; answer it with `tools.result`. While the app is gone, the session's status is `awaiting_client` (a `session.status` event, and `awaiting_client` in `sessions.open`).

## Rules

- **Approvals need a person.** `approvals.decide` takes the user-action token the window minted when the person acted. The shell and the session's first-party app may decide; Views and third-party code only request (`not_allowed_to_decide`). Words that are not a clear yes are a denial.
- **Client tools are the app's own.** `tools.register_client` applies to the calling app's token and that session only; `tools.result` is accepted only from the app that registered the tool.
- **Purge is the app's own data.** `sessions.purge` stops the app's running turns, then deletes its sessions' logs, attachments and artifacts. Nothing of any other app's is touched.

## Errors

| Code | When |
| --- | --- |
| `capability_denied` | No `agents` capability |
| `not_found` | No such session, approval, question, preset or artifact for this caller (another app's ids too) |
| `invalid_preset` | `presets.put` text did not validate; the message lists why |
| `class_not_supported` | The preset's class is not run yet (autonomous and delegated, for now) |
| `not_at_turn_boundary` | A fork point inside a turn |
| `continue_refused` | Nothing to continue, or the model cannot continue it |
| `not_allowed_to_decide` / `missing_user_action` / `already_decided` | `approvals.decide` refused |
| `session_failed` | The session failed and takes no more input |
| `invalid_tools` | Specs that are not client tools, or names already taken |
| `unknown_subscription` | Unsubscribing from a stream that ended or is not the caller's |

## Testing

The testkit's host is to be harness-host's `MemoryHost`, behind the same client API, so apps test sessions without the engine. Until it lands (the next pull request), the testkit checks the capability on every agents call and answers the rest `unknown_method`. The conformance suite then gains an `agents.` group that `MemoryHost` and the engine host must both pass.
