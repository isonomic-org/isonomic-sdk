# `blobs` interface

Byte objects an app stores and reads back by handle: Chat's attachments first. Every example is a frozen fixture in `crates/proto/fixtures/v0/blobs.jsonl`.

Needs the `blobs` capability. `max_bytes` in the manifest caps each blob; the engine's own limit applies on top. A handle is opaque and belongs to the app that stored it: another app gets `unknown_blob` for it, as if it did not exist. The shell can read every blob, to show it in the window.

## Methods

| Method | Params | Reply |
| --- | --- | --- |
| `blobs.put` | `{ content_type, data }` | `blob BlobInfo` |
| `blobs.create` | `{ content_type, size }` | `blob BlobInfo` with `written: 0` |
| `blobs.write` | `{ handle, offset, data }` | `blob BlobInfo` |
| `blobs.get` | `{ handle, offset, length? }` | `blob_data { info, offset, data }` |
| `blobs.info` | `{ handle }` | `blob BlobInfo` |
| `blobs.delete` | `{ handle }` | `deleted {}` |

`data` is base64. One call carries at most `CHUNK_BYTES` (512 KiB) of raw bytes, which keeps every frame under the 1 MiB limit. A smaller blob is one `blobs.put`. A larger one is a `blobs.create` with its total size, then `blobs.write`s in order: each `offset` must equal the bytes written so far. The blob can be read once `written` equals `size`. `blobs.get` returns at most `CHUNK_BYTES` from `offset`, fewer if `length` asks for fewer. The client does the chunking: `Blobs::put` and `Blobs::get` take and return whole byte vectors.

```json
{"type":"request","id":40,"token":"tok-hello-1","method":"blobs.put","params":{"content_type":"text/plain","data":"aGVsbG8="}}
{"type":"response","id":40,"result":{"blob":{"handle":"blob-1","content_type":"text/plain","size":5,"written":5}}}
{"type":"request","id":41,"token":"tok-hello-1","method":"blobs.create","params":{"content_type":"image/png","size":1048576}}
{"type":"response","id":41,"result":{"blob":{"handle":"blob-2","content_type":"image/png","size":1048576,"written":0}}}
{"type":"request","id":42,"token":"tok-hello-1","method":"blobs.write","params":{"handle":"blob-2","offset":0,"data":"iVBORw0KGgo="}}
{"type":"response","id":42,"result":{"blob":{"handle":"blob-2","content_type":"image/png","size":1048576,"written":8}}}
{"type":"request","id":44,"token":"tok-hello-1","method":"blobs.get","params":{"handle":"blob-1","offset":1,"length":3}}
{"type":"response","id":44,"result":{"blob_data":{"info":{"handle":"blob-1","content_type":"text/plain","size":5,"written":5},"offset":1,"data":"ZWxs"}}}
```

## Errors

| Code | When |
| --- | --- |
| `capability_denied` | No `blobs` capability |
| `too_large` | Over the manifest's `max_bytes` or the engine's limit; a chunk over `CHUNK_BYTES`; a write past the declared size |
| `unknown_blob` | No such handle for this caller, or deleted |
| `incomplete` | `blobs.get` before every byte was written |
| `bad_request` | A `blobs.write` whose `offset` is not the bytes written so far |

```json
{"type":"response","id":48,"error":{"code":"too_large","message":"99999999999 bytes is over the limit of 67108864"}}
{"type":"response","id":49,"error":{"code":"incomplete","message":"blob-2 has 8 of 1048576 bytes"}}
```

## Testing

`TestEngine::blobs()` keeps blobs in memory: `set_limit` lowers the engine's own limit (64 MiB by default), `stored(handle)` reads back what an app stored, `count()` says how many exist. `TestEngine::slow(Interface::Blobs, delay)`, `fail(Interface::Blobs, error)` and `recover` make every blob call slow or fail, as for any interface.
