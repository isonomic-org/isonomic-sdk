# `events` interface

Messages an app publishes to its own scope, so its tabs and windows stay in step: a thread opened in one window, a setting changed in another. Every example is a frozen fixture in `crates/proto/fixtures/v0/events.jsonl`.

No capability is needed: an app reaches only its own scope, keyed by its app id. Only the shell may name another app's scope (`app`), for example to tell an app's tabs that a setting changed; an app that names any app but itself gets `capability_denied`.

Delivery is live only: at most once, in the publisher's order, to the subscribers present when the message is published. Nothing is stored; a subscriber that falls far behind misses messages. State that must survive belongs in `sql` or `blobs`.

## Methods

| Method | Params | Reply |
| --- | --- | --- |
| `events.publish` | `{ topic, data, sender?, app? }` | `published {}` |
| `events.subscribe` | `{ topics?, app? }` | `listening { subscription }`, then `app_event`s |
| `events.unsubscribe` | `{ subscription }` | `unsubscribed {}` |

`topic` is lowercase letters, digits, dots, dashes and underscores, up to 128 characters. `topics` empty or absent means every topic in the scope. `data` is any JSON; key order inside it is not kept. `sender` is echoed on every delivered event, so a tab can skip its own messages. Only the token that opened a subscription can end it.

```json
{"type":"request","id":60,"token":"tok-hello-1","method":"events.subscribe","params":{"topics":["chat.thread-opened"]}}
{"type":"response","id":60,"result":{"listening":{"subscription":12}}}
{"type":"request","id":62,"token":"tok-hello-1","method":"events.publish","params":{"topic":"chat.thread-opened","data":{"pinned":true,"thread":"t-42"},"sender":"tab-2"}}
{"type":"response","id":62,"result":{"published":{}}}
{"type":"event","subscription":12,"event":"app_event","data":{"topic":"chat.thread-opened","data":{"pinned":true,"thread":"t-42"},"sender":"tab-2"}}
```

## Errors

| Code | When |
| --- | --- |
| `bad_request` | A topic that breaks the naming rule |
| `capability_denied` | An app named another app's scope |
| `unknown_subscription` | Unsubscribing from a subscription that ended or is not the caller's |

## Testing

The testkit keeps one channel per scope across every connection, so two `TestEngine::app_client`s for the same manifest behave like two windows. `TestEngine::slow` and `fail` apply as for any interface.
