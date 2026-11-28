# `agents` interface

Agent sessions the engine's host runs for the app: Chat's conversations first, then every assistant panel. Each method maps one to one onto a harness-host call (isonomic-harness `docs/host-api.md`). Every example is a frozen fixture in `crates/proto/fixtures/v0/agents.jsonl`.

Needs the `agents` capability. The host decides what an app reaches: its own sessions, presets and artifacts only. Another app's ids answer `not_found`, exactly as if they did not exist. The shell reaches every app's.

## Types

The session event schema is harness-core's, re-exported as `isonomic_proto::agents::schema` at the commit the SDK pins (`harness-core` in the workspace `Cargo.toml`). Apps import only the SDK. `welcome` carries the schema the engine writes, `{ version, revision }`; it shows up in `Client::live().schema`. A newer revision of the same version still reads: an unknown event or block type decodes as `Unknown` and should be rendered generically.

Arguments and results (`CreateSession`, `SessionSummary`, `PendingApproval`, …) are the SDK's own, in `isonomic_proto::agents`.

## Methods

| Method | Params | Reply |
| --- | --- | --- |
| `sessions.create` | `{ preset, title?, location_opt_in?, model? }` | `session_created { session }`; `model` runs the session on that model in place of the preset's |
| `sessions.list` | `{ archived?, preset?, text?, cursor?, limit? }` | `sessions { items: [SessionSummary], next? }` |
| `sessions.open` | `{ session }` | `session_snapshot { summary, turn_open?, approvals, questions, awaiting_client }` |
| `sessions.fork` | `{ session, at: {before_turn\|after_turn: turn}, reason? }` | `session_created { session }` |
| `sessions.archive` | `{ session, archived }` | `done {}` |
| `sessions.rename` | `{ session, title }` | `done {}` |
| `sessions.send` | `{ session, content: [Block], client_ref? }` | `accepted { queued }` |
| `sessions.continue` | `{ session }` | `done {}`; extends the last sealed partial reply |
| `sessions.cancel` | `{ session }` | `done {}`; safe to repeat |
| `sessions.purge` | `{ app? }` | `purged { sessions, events, blobs }`: every session of the calling app (`{}`), or of the app the shell names; the shell must name one |
| `sessions.subscribe` | `{ session, from_seq? }` | `session_stream { subscription }`, then `session_event` and `live_delta` events |
| `sessions.unsubscribe` | `{ subscription }` | `unsubscribed {}` |
| `approvals.list` | `{ app? }` | `approvals [{ session, app, request }]`; the shell may narrow to one `app` |
| `approvals.decide` | `{ session, approval, decision: allow\|deny\|{text}, user_action? }` | `done {}` |
| `approvals.user_action` | `{ session, approval, app? }` | `user_action { token }`. Shell only: the window mints the token when the person acts and hands it to `app` (the shell's own when absent). An app asks for it with `AppContext::user_action` (`docs/guide/build-an-app.md`) |
| `questions.answer` | `{ session, question, selected?, text?, skipped? }` | `done {}` |
| `tools.register_client` | `{ session, specs: [ToolSpec] }` | `done {}`; replaces the app's client tools for that session |
| `tools.result` | `{ session, call, content?, error? }` | `done {}` |
| `presets.list` | `{}` | `presets [{ reference, owner?, name, diagnostics }]` |
| `presets.get` | `{ preset }` | `preset { reference, owner?, text, diagnostics }` |
| `presets.put` | `{ text }` (TOML) | `preset_saved { reference }` |
| `artifacts.get` | `{ artifact, version?, offset?, length? }` | `artifact { meta, bytes, offset? }`: at most `CHUNK_BYTES` (512 KiB) of the body from `offset`, base64; `meta.bytes` is the whole size. Reading at or past the end of a non-empty body is `bad_request`. `Agents::artifact` reads every chunk and refuses a reply whose `offset` or version does not match what it asked for |
| `artifacts.list` | `{ session }` | `artifacts [ArtifactEvent]` |

```json
{"type":"request","id":86,"token":"tok-chat-1","method":"sessions.send","params":{"session":"s-1","content":[{"type":"text","text":"Plan three days in Lisbon"}]}}
{"type":"response","id":86,"result":{"accepted":{"queued":false}}}
{"type":"event","subscription":21,"event":"live_delta","data":{"session":"s-1","turn":"t-3","step":"p-1","after_seq":12,"block":0,"kind":"text","text":"Sintra is "}}
```

Approval, question and call ids are minted per session, so two sessions can each hold an `a-1`. The calls that answer one name the session as well: the target is (session, id). Calls without parameters still take `params: {}`, so a filter can be added later without a new method.

## Events

A stream's last event is `session_ended { session, reason }` (`reason: purged`): `SessionStream::next` returns `SessionUpdate::Ended(reason)`, then `None`. A `None` with no `Ended` before it means the connection dropped, so subscribe again from `last_seq()`; after `Ended`, do not, since the session is gone. If a sequence number is ever skipped (a malformed event the client could not read, say), `next` returns `SessionUpdate::Gap { after, next }` instead of the event; subscribe again from `last_seq()` to read the missing ones.


A subscription first replays the durable events after `from_seq`, then sends new ones as they are written, with no gap or duplicate at the seam. Each `session_event` is one `SessionEvent` (`session`, `seq`, `at`, `turn?`, `step?`, then `type` and its fields). To resume after a reconnect, subscribe from the last `seq` seen (`SessionStream::last_seq`).

`live_delta` events carry reply text as it streams. They are lossy and may be dropped under load; the durable `assistant` event that follows is the truth, so drop a step's deltas when it arrives.

A client tool call arrives as a `tool.call` event with `kind: client`; answer it with `tools.result`. While the app is gone, the session's status is `awaiting_client` (a `session.status` event, and `awaiting_client` in `sessions.open`).

## Rules

- **Approvals need a person.** `approvals.decide` takes the user-action token the window minted when the person acted. The shell and the session's first-party app may decide; Views and third-party code only request (`not_allowed_to_decide`). Words that are not a clear yes are a denial.
- **Client tools are the app's own.** `tools.register_client` applies to the calling app's token and that session only; `tools.result` is accepted only from the app that registered the tool.
- **The person's token.** `approvals.user_action` mints a token for one approval in one session, for one decider (the app named, or the shell). It is good only for that decision and only until the approval is decided, by anyone; it carries no other power. An engine may also expire it sooner; the testkit never does.
- **A session's model is the app's to pick, within its manifest.** `sessions.create { model }` follows the rule of `models.stream`: an app names only a model its `models` capability allows (`capability_denied` otherwise, from `proto::auth`; a remote one also needs `remote`). An unknown model answers `unknown_model`, one that cannot chat `unsupported`, one that is not installed `model_unavailable`. Absent, the preset's model runs, as before. The same rule holds for a preset the app stored itself: every model it names (`[model] id` and `title_model`) must be one the manifest allows, or `sessions.create` answers `capability_denied`. Built-in presets are the engine's choice and are not checked.
- **Only the shell names another app.** `app` on `sessions.purge` and `approvals.list` works as on `events`: an app may name only itself (`capability_denied` otherwise, from `proto::auth`), and the shell may name any. The shell's purge with no `app` is `bad_request`, never "purge everything". An app that misspells `app` sends a key the engine ignores, so the call reads as `{}` and acts on the app's own sessions: tolerant decoding working as designed, not a way past the rule.
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

The testkit serves every agents call with harness-host's `MemoryHost` (at the commit the workspace pins), behind the same client API, so apps test sessions without the engine. The token check comes first, through `proto::auth`; the host then keeps apps apart exactly as the engine's does. Every granted app is first-party.

`TestEngine::agents()` scripts the engine side:

- `reply(text)` queues the model's next reply. With nothing queued it answers "OK.". `with(|host| ..)` reaches the rest of the host: `host.model.calls(..)` for tool calls, `host.model.push(..)` for any model events.
- Each call runs what it started to completion before it answers, so a scripted turn has ended by the time `sessions.send` replies. The clock moves only with `advance(ms)`, which also runs timers, timeouts and the watchdog.
- `user_action(app, session, approval)` mints the token the window would when the person acts; pass it to `decide`.
- `disconnect(app)` and `reconnect(app)` take an app's client tools away and bring them back.
- `log(session)` reads the events the host stored.

A purged session's stream sends nothing more; the wire has no end-of-stream event for it.

The host keys approvals, questions and client tool calls by (session, id), so equal ids in two sessions never meet; a call naming the wrong session answers `not_found`. For `app` on `sessions.purge` and `approvals.list`, `proto::auth` refuses what the rule above forbids before the host sees the call. The shell's purge acts for the app it names.

The conformance suite's `agents.` group runs against this host and the engine's (`docs/conformance.md`).
