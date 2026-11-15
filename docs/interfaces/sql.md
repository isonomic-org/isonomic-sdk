# `sql` interface

An app's own data in the Capsule's database: query and change it with typed rows, run several statements as one transaction, and keep live queries whose result the engine pushes whenever it changes. Chat's thread list, projects and settings live here. Every example is a frozen fixture in `crates/proto/fixtures/v0/sql.jsonl`.

Needs the `sql` capability. An app owns the schemas its manifest declares (`schemas`) and may read and write them; it may only read the other apps' schemas listed under `capabilities.sql.schemas`. A call runs in `schema`, or in the app's first own schema when `schema` is absent. The shell must name a schema and may use any. `proto::auth::authorize_schema` makes this decision for the engine and the testkit alike.

## Methods

| Method | Params | Reply |
| --- | --- | --- |
| `sql.query` | `{ sql, params?, schema? }` | `rows Rows` |
| `sql.execute` | `{ sql, params?, schema? }` | `executed { rows_affected }` |
| `sql.batch` | `{ statements: [{ sql, params? }], schema? }` | `executed_batch { rows_affected: [n] }`, all or nothing |
| `sql.subscribe` | `{ sql, params?, schema? }` | `live_query { subscription, rows }`, then `rows_changed` events |
| `sql.unsubscribe` | `{ subscription }` | `unsubscribed {}` |

Parameters are positional (`$1`, `$2`, …). A value is `null`, `true`/`false`, an integer, a float (always written with a decimal point), text, or `{"bytes":"<base64>"}`. `Rows` is `{ columns: [{ name, type }], rows: [[value]] }`; `type` is the database's own type name, as a string. A live query sends the whole new result on every change; only the token that opened it can end it.

```json
{"type":"request","id":70,"token":"tok-hello-1","method":"sql.query","params":{"sql":"select id, title, pinned from threads where archived = $1 order by updated desc","params":[false]}}
{"type":"response","id":70,"result":{"rows":{"columns":[{"name":"id","type":"integer"},{"name":"title","type":"text"},{"name":"pinned","type":"boolean"}],"rows":[[42,"Trip to Lisbon",true],[41,null,false]]}}}
{"type":"request","id":73,"token":"tok-hello-1","method":"sql.subscribe","params":{"sql":"select count(*) as n from messages"}}
{"type":"response","id":73,"result":{"live_query":{"subscription":14,"rows":{"columns":[{"name":"n","type":"integer"}],"rows":[[7]]}}}}
{"type":"event","subscription":14,"event":"rows_changed","data":{"columns":[{"name":"n","type":"integer"}],"rows":[[8]]}}
```

## Errors

| Code | When |
| --- | --- |
| `capability_denied` | No `sql` capability; no schema of its own; a schema it may not use; a write to a read-only schema |
| `sql_error` | The database refused the statement; the message says why |
| `bad_request` | The shell did not name a schema |
| `unknown_subscription` | Unsubscribing from a live query that ended or is not the caller's |

## Testing

The testkit's database is scripted; it runs no SQL. `TestEngine::sql()` gives `respond(schema, sql, rows)` (also pushed to every live query on that statement), `affect(schema, sql, n)`, `fail(sql, message)` and `statements()`, the log of everything apps ran. Statements match whatever their spacing. Unscripted queries return no rows and unscripted writes change none.
