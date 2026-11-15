# `system` interface

Shell only: the window's view of the engine. Every example is a frozen fixture in `crates/proto/fixtures/v0/system.jsonl`.

| Method | Params | Reply |
| --- | --- | --- |
| `system.status` | — | `status` (a `SystemStatus`) |
| `system.subscribe` | — | `subscribed { subscription, snapshot }`, then `service_status` events |
| `system.unsubscribe` | `{ subscription }` | `unsubscribed {}` |
| `system.logs` | `{ service, limit? }` | `logs { service, lines }` (default 50 lines) |
| `system.profile` | — | `profile` (a `HardwareProfile`) |
| `system.shutdown` | — | `shutting_down {}`; the engine then stops in order |
| `system.retry` | `{ service }` | `retrying {}`; the engine clears the service's restart window and starts it again. The outcome (`starting`, then `healthy` or `degraded`) arrives as `service_status` events |

```json
{"type":"request","id":2,"method":"system.subscribe"}
{"type":"response","id":2,"result":{"subscribed":{"subscription":7,"snapshot":{"aggregate":"starting","services":[{"name":"postgres","tier":1,"state":"starting","restart_count":0,"last_healthy_unix":null}],"cold_start_ms":null}}}}
{"type":"event","subscription":7,"event":"service_status","data":{"name":"postgres","tier":1,"state":"healthy","restart_count":2,"last_healthy_unix":1700000000}}
```

Errors: `unknown_service` (logs or retry of a service that does not exist), `not_degraded` (retry of a service that is not degraded), `unknown_subscription`, `capability_denied` (any app token).

In the testkit, `retries()` lists the services retried, and `on_retry(service, outcome)` scripts where a retry ends after `starting`. Unscripted, the service stays `starting` until the test calls `set_state`.

The client keeps one status subscription open itself and folds its events into `Client::live()`; views await that watch instead of calling `system.status`.
