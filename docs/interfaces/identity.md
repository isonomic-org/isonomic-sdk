# `identity` interface

The person's Isonomic ID, which the window shows. The engine creates it the first time it is needed (in practice at first run) and keeps it. Every example is a frozen fixture in `crates/proto/fixtures/v0/identity.jsonl`.

**Shell only.** `proto::auth` refuses it to every app token with `capability_denied`, whatever the app's capabilities. Apps never see the ID.

The recovery kit (`identity.kit_*`, below) lets the person take the ID to another Mac. Grants, activity logs and keys are not on the wire.

## Methods

| Method | Params | Reply |
| --- | --- | --- |
| `identity.get` | `{}` | `identity { id, created }` |
| `identity.kit_export` | `{ person_asked }` | `recovery_kit { id, words, kit }` |
| `identity.kit_restore` | `{ kit, words, replace_unused }` | `kit_restored { id, words, kit, kit_made, note }` |
| `identity.kit_saved` | `{}` | `done {}` |
| `identity.kit_status` | `{}` | `kit_status { saved_at, exported_at }` |

- `id` is the form a person reads and copies. Treat it as opaque text: show it as given, and don't parse it.
- `created` is `true` only on the call that made the ID. Every later call, from any connection, answers `false` with the same `id`. The call is idempotent once the ID exists.

```json
{"type":"request","id":140,"method":"identity.get","params":{}}
{"type":"response","id":140,"result":{"identity":{"id":"isonomic:7Q4K-M2XD-9PLV","created":true}}}
{"type":"request","id":141,"method":"identity.get","params":{}}
{"type":"response","id":141,"result":{"identity":{"id":"isonomic:7Q4K-M2XD-9PLV","created":false}}}
```

## The recovery kit

A kit is two parts the person keeps apart: 24 words they write down, and a kit document the window saves as a file. Both are needed to restore. **The document never holds the words.** Anyone who found the file alone couldn't take the ID.

- **`kit_export`** returns the ID, the words and the document. It is refused (`recovery_refused`) unless `person_asked` is `true`. The window sets that only when the person asked for the kit. The words leave the engine only in this reply.
- **`kit` is opaque text.** It is UTF-8 JSON holding the ID, the key log and a fingerprint of the recovery key. Save it and send it back as given; don't parse it.
- **`kit_restore`** takes the document and the words, and checks them in order:
  1. the document verifies;
  2. the words are well formed (on the list, and the checksum holds);
  3. the words are this kit's;
  4. this Mac may take the ID.
  
  A restore **spends the old kit**. The reply's new `words` and `kit` replace it, and the window asks the person to save them. `kit_made` (Unix seconds) is when the restored kit was made; changes made on another Mac after that are set aside. `note` is the engine's plain words about it: that the old kit no longer works, and anything set aside. The window shows it as written, so the wording lives in one place.
- **`replace_unused`:** a Mac with no ID always restores. A Mac whose ID has no app data yet restores only with `replace_unused: true`, which moves the old ID aside (never deletes it). A Mac whose ID has apps and chats is refused.
- **`kit_saved`** records that the person confirmed saving the kit. `kit_status` gives `saved_at` and `exported_at`, in Unix seconds, or `null`. A restore resets `saved_at`, because the old kit is spent.
- **Typing the words in:** the window checks entry with the `isonomic-words` crate before it calls `kit_restore`. It holds the standard 2048-word English list, pinned by its published SHA-256 (`LIST_SHA256`, which a test holds the bundled file to).
  - `is_word(w)` checks one word.
  - `completions(prefix)` and `complete(prefix)` suggest words as the person types. Four letters always settle a word.
  - `check(&words)` returns `Problem::{Count, NotAWord { position }, Checksum}`. It checks that there are 24 words, each on the list, with the last carrying the checksum of the rest. Trim and lowercase first.
  - Problems name a position, never a word. The engine uses the same list and still does the real verification.
- **The words in code:** `identity::RecoveryWords` holds them. Its `Debug` prints only a count, so a call, reply or error that holds words can be logged. Read them with `reveal()` only to show them or send them back. Never log `reveal()`.

```json
{"type":"request","id":144,"method":"identity.kit_export","params":{"person_asked":true}}
{"type":"request","id":145,"method":"identity.kit_export","params":{"person_asked":false}}
{"type":"response","id":145,"error":{"code":"recovery_refused","message":"Isonomic shows your recovery kit only when you ask for it."}}
{"type":"request","id":146,"method":"identity.kit_saved","params":{}}
{"type":"response","id":146,"result":{"done":{}}}
{"type":"response","id":147,"result":{"kit_status":{"saved_at":1794850060,"exported_at":1794850000}}}
```

The fixtures' words are the word-list standard's public 256-bit test vectors, never real-looking words. The full replies are in the fixture file.

**An older engine** has none of these methods. It answers `unknown_method` ("this engine has no method identity.kit_export"), so `Client::kit_export` and the others fail with `ClientError::Engine` and that code, and the window can say the engine needs updating. **An older client** never sends them. If it met `recovery_refused`, it would read it as `ErrorCode::Unrecognized`.

## Errors

| Code | When |
| --- | --- |
| `identity_unavailable` | The engine could not make or read the ID (the key store refused, say). The `message` is plain words for the window to show as written. Nothing was made; try again later |
| `recovery_refused` | A kit call the engine refused: an export the person did not ask for, words that are not the kit's, a damaged or spent kit, a Mac whose ID is in use, or `kit_saved` before any export. The `message` is plain words to show as written. Nothing was changed |
| `capability_denied` | An app token, on any `identity` method |

```json
{"type":"response","id":142,"error":{"code":"identity_unavailable","message":"The key store did not let Isonomic save your ID. Unlock it and try again."}}
```

## Client

On the shell's `Client` only (`AppClient` has none of these):
- `identity()` returns `Identity { id, created }`.
- `kit_export(person_asked)` returns `RecoveryKit`.
- `kit_restore(KitRestore)` returns `KitRestored`.
- `kit_saved()` returns `()`.
- `kit_status()` returns `KitStatus`.

## Testing

`TestEngine::identity()` scripts it:

- `set_id(id)` sets the ID; the default is `isonomic_testkit::DEFAULT_IDENTITY`.
- `refuse(message)` answers `identity_unavailable` with that message until `recover()`.
- `forget()` starts again as on a first run.
- `made()` says whether the ID exists.
- **The kit** is deterministic. Kit generation *n*'s words are `isonomic_testkit::kit_words(n)`, a public test vector. A restore makes generation *n* + 1. The document is the test engine's own JSON, so a kit exported from one `TestEngine` restores on another. `set_now(secs)` sets the kit clock (`DEFAULT_KIT_NOW` by default), `set_in_use(true)` makes restore refuse as on a Mac with apps and chats, and `kit_generation()` reads the current generation. It checks the words with `isonomic-words` ("don't add up" when the checksum fails), and every restore's `note` is `KIT_RESTORE_NOTE`.

The conformance checks `identity.kit_apps_refused` and `identity.kit_export_needs_person` cover the refusals. No check ever exports a kit. The check `identity.stable` asks the engine for the ID twice: the second time on a connection of its own. It must be the same, non-blank, and `created: false` the second time.
