# `models` interface

Chat completions, model listing and embeddings, through the engine's model gateway. Local models only by default; remote providers are opt-in per app (manifest) and per user (the global switch). Every example is a frozen fixture in `crates/proto/fixtures/v0/models.jsonl`.

Needs the `models` capability. An app sees and uses only the models its manifest names (`ids`), or any (`any: true`); remote models only with `remote: true`.

## Methods

| Method | Params | Reply |
| --- | --- | --- |
| `models.list` | — | `models [ModelInfo]`, filtered to what the caller may use |
| `models.info` | `{ model }` | `model ModelDetail` |
| `models.stream` | `StreamRequest` | `stream { subscription, model }`, then `model_chunk` events |
| `models.cancel` | `{ subscription }` | `cancelled {}`; the stream's last chunk then has `stop: cancelled` |
| `models.embed` | `{ model?, input: [text] }` | `embeddings { model, vectors }`, one per input, in order |

`model` left out means the user's default chat (or embeddings) model; the reply names the one chosen.

```json
{"type":"request","id":21,"method":"models.info","params":{"model":"chat-small"}}
{"type":"response","id":21,"result":{"model":{"id":"chat-small","name":"Chat Small","context_length":8192,"capabilities":["chat"],"size_bytes":2684354560,"license":"Test License","state":"stopped"}}}
```

## Streaming

`StreamRequest`: `messages` (each `role` of `system`, `user`, `assistant` or `tool`, with `content`; assistant messages may carry `tool_calls`, tool messages a `tool_call_id`), and optionally `model`, `tools`, `temperature`, `max_tokens` and `reasoning`. `reasoning` asks the model to think before answering where it can; left out, the engine decides (off on slow machines). Tool schemas and arguments are JSON text.

```json
{"type":"request","id":22,"token":"tok-hello-1","method":"models.stream","params":{"model":"chat-small","messages":[{"role":"system","content":"Be brief."},{"role":"user","content":"What time is it in Lisbon?"}],"tools":[{"name":"clock","description":"The time in a city","schema":"{\"type\":\"object\",\"properties\":{\"city\":{\"type\":\"string\"}}}"}],"temperature":0.5,"max_tokens":512,"reasoning":true}}
{"type":"response","id":22,"result":{"stream":{"subscription":9,"model":"chat-small"}}}
{"type":"event","subscription":9,"event":"model_chunk","data":{"reasoning":"The user wants the time."}}
{"type":"event","subscription":9,"event":"model_chunk","data":{"tool_calls":[{"index":0,"id":"call-1","name":"clock","arguments":"{\"city\":"}]}}
{"type":"event","subscription":9,"event":"model_chunk","data":{"tool_calls":[{"index":0,"arguments":"\"Lisbon\"}"}]}}
{"type":"event","subscription":9,"event":"model_chunk","data":{"usage":{"input_tokens":31,"output_tokens":12,"reasoning_tokens":6,"cost_micros":0},"stop":"tool_calls"}}
```

A `model_chunk` may carry any of: `text`, `reasoning`, `tool_calls` (deltas: the first for an `index` has `id` and `name`, later ones append to `arguments`), `usage`, and `stop`. The chunk with `stop` is the last.

| `stop` | Meaning |
| --- | --- |
| `end` | The answer is complete |
| `max_tokens` | The `max_tokens` limit was reached |
| `tool_calls` | Run the tools it called, then send their results in a new stream |
| `cancelled` | `models.cancel`, or the user's remote switch cut a remote stream |
| `error` | The model failed mid-stream; the chunk's `error` says why |

```json
{"type":"event","subscription":11,"event":"model_chunk","data":{"stop":"error","error":{"code":"model_unavailable","message":"the model worker crashed"}}}
```

Only the app that opened a stream can cancel it. Dropping a `ModelStream` in the client cancels it.

## Errors

| Code | When |
| --- | --- |
| `capability_denied` | No `models` capability, a model the manifest does not name, or a remote model without `remote` |
| `unknown_model` | No such model, or no default of the kind asked for |
| `unsupported` | The model cannot do this (embeddings from a chat model) |
| `insufficient_memory` | The memory gate refused; `memory` carries `required_bytes`, `available_bytes` and `reason` |
| `model_unavailable` | The worker is degraded or crashed |
| `remote_disabled` | The model is remote and the user's switch is off |
| `unknown_subscription` | `models.cancel` for a stream that ended or is not the caller's |

```json
{"type":"response","id":27,"error":{"code":"insufficient_memory","message":"not enough free memory to load the model without thrashing; try a smaller model or a remote one","memory":{"required_bytes":21474836480,"available_bytes":6442450944,"reason":"not enough free memory to load the model without thrashing; try a smaller model or a remote one"}}}
```

## Testing

`TestEngine::models()` scripts the fake: `add`, `script` (steps of chunks, `Hold(gate)` and `Fail`), `refuse` and `admit`, `fail` and `recover`, `set_remote_enabled`, and `requests` / `cancels` for assertions. Without a script, `chat-small` echoes the last user message word by word.
