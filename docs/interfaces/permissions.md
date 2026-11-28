# `permissions` interface

What the person lets an app's agent sessions reach: linked folders, kit switches ("may this app use the web") and remembered approvals. Each is a grant the engine keeps and checks. The window issues and ends them after the person acts; an app reads its own and may ask the window to put a question to the person. Design: "Tool kits: architecture v0.3"; what differs from it is in `docs/kits-deviations.md`. Every example is a frozen fixture in `crates/proto/fixtures/v0/permissions.jsonl`.

## Methods

| Method | Who | Params | Reply |
| --- | --- | --- | --- |
| `permissions.grant` | Shell only | `{ app, grant: {folder: {path, access}} \| {kit}, session?, until? }` | `permissions Permissions`: everything the app then holds |
| `permissions.list` | An app for itself; the shell for any app | `{ app? }` | `permissions Permissions`. An app's `{}` is its own; the shell must name the app |
| `permissions.revoke` | Shell only | `{ app, permission: {folder: id} \| {kit} \| {approval: id} }` | `done {}`. The next check refuses |
| `permissions.request` | An app, with a user-action token | `{ ask: "folder" \| {kit}, user_action? }` | `done {}`. Asks the window to show the folder picker or a kit's first question; the window decides what to show |

```json
{"type":"request","id":181,"method":"permissions.grant","params":{"app":"com.isonomic.chat","grant":{"folder":{"path":"/Users/ana/Design","access":"read_write"}}}}
{"type":"response","id":181,"result":{"permissions":{"app":"com.isonomic.chat","folders":[{"id":"folder-1","name":"Design","path":"~/Design","access":"read_write","reachable":true}],"kits":[{"kit":"web","state":"not_asked"}]}}}
{"type":"request","id":182,"method":"permissions.grant","params":{"app":"com.isonomic.chat","grant":{"kit":"web"}}}
{"type":"request","id":186,"token":"tok-chat-1","method":"permissions.list","params":{}}
{"type":"response","id":186,"result":{"permissions":{"app":"com.isonomic.chat","folders":[{"id":"folder-1","name":"Design","path":"~/Design","access":"read_write","reachable":true}],"kits":[{"kit":"web","state":"on"}],"approvals":[{"id":"approval-1","effect":"modifies_data","scope":{"kind":"folder","id":"folder-1","label":"Design"},"remember":"session","session":"s-1"},{"id":"approval-2","effect":"modifies_data","scope":{"kind":"folder","id":"folder-1","label":"Design"},"remember":"always"}]}}}
{"type":"request","id":190,"method":"permissions.revoke","params":{"app":"com.isonomic.chat","permission":{"folder":"folder-1"}}}
{"type":"request","id":196,"token":"tok-chat-1","method":"permissions.request","params":{"ask":{"kit":"web"},"user_action":"ua-81"}}
```

## Shapes

- **`Permissions`**: `app`, `folders`, `kits` and `approvals`; an empty list is left out.
- **`folders`**: `id` (the engine's; the scope id of the folder's grants), `name` (what the person and the model call it: `Design/tokens.css` is `tokens.css` in the folder named `Design`), `path` (to show the person), `access` and `reachable` (whether the engine can open it now).
- **`access`**: `read` (read without asking) or `read_write` (a change asks first, until the person remembers an answer). A newer value decodes as `unrecognized`.
- **`kits`**: one `{ kit, state }` per kit the manifest names, in its order, apart from `files`, whose permissions are the linked folders. `state` is `not_asked`, `on` or `off`; a newer value decodes as `unrecognized`.
- **`approvals`**: remembered approvals, `{ id, effect, scope, remember, session? }`. `remember` is `session` (with the `session` it holds for) or `always`. `effect`, `scope.kind` and `remember` are harness-core's open enums: a newer value decodes as `Other`.
- **`session`, `until`** on a grant: hold it for one session, or end it at a time (milliseconds since the Unix epoch).

## Rules

- **Only the window grants and revokes.** `proto::auth` refuses `permissions.grant` and `permissions.revoke` to every app token with `capability_denied`, whatever its manifest says. The window calls them after the person acts.
- **The manifest names the kits.** An app is granted, and so offered, only kits listed under `agents: { kits: [...] }` in its approved manifest. The engine checks `auth::authorize_kit` before it records anything: a kit the manifest does not name (a folder is the `files` kit) answers `capability_denied`. Whatever a preset names, a tool with no grant is never offered.
- **An app sees only its own.** An app naming another app in `permissions.list` is refused with `capability_denied`. An id of one app's folder or approval is `not_found` under any other app's name, as is an app the engine never approved.
- **A request needs the person.** `permissions.request` without a verified user-action token answers `missing_user_action`, and nothing is shown or recorded, so nothing a model writes can raise a question. An app asks only for kits its manifest names (`"folder"` is the `files` kit). The shell never calls it (`bad_request`).
- **Each answer is recorded.** A yes is `permissions.grant`; a no is `permissions.revoke` of the kit, which reads `off` from then on. A request alone changes no state.
- **Some places can never be linked**: the engine's own directory, key stores, other apps' data, system folders and the home folder as a whole. They answer `not_linkable`, and nothing is recorded.
- **Revoking ends it at once.** The next grant check refuses; session approvals that rested on a revoked grant allow nothing.

```json
{"type":"request","id":184,"method":"permissions.grant","params":{"app":"com.isonomic.chat","grant":{"folder":{"path":"/Users/ana","access":"read"}}}}
{"type":"response","id":184,"error":{"code":"not_linkable","message":"The home folder as a whole cannot be linked."}}
{"type":"request","id":194,"token":"tok-chat-1","method":"permissions.grant","params":{"app":"com.isonomic.chat","grant":{"kit":"web"}}}
{"type":"response","id":194,"error":{"code":"capability_denied","message":"app com.isonomic.chat may not call permissions.grant"}}
{"type":"request","id":197,"token":"tok-chat-1","method":"permissions.request","params":{"ask":"folder"}}
{"type":"response","id":197,"error":{"code":"missing_user_action","message":"Asking the person for a permission needs a user action."}}
```

## Errors

| Code | When |
| --- | --- |
| `capability_denied` | An app token called `grant` or `revoke`; an app named another app; a kit the app's manifest does not name; an app without the `agents` capability |
| `not_linkable` | A folder that can never be linked |
| `missing_user_action` | `permissions.request` without the person's verified token |
| `not_found` | No such app, folder, kit or approval for that app |
| `bad_request` | The shell's `permissions.list` without an app; the shell's `permissions.request` |

## Open points

Two things the design leaves unsaid are not on the wire yet, and no engine can complete a `permissions.request` until they are decided:

- **How the window learns of a request.** The call reaches the engine, and the window must then show something. No event or call carries it yet.
- **How the request's token is minted.** `approvals.user_action` mints for one approval of one session; a permission request has neither.

## Test engine

`TestEngine::permissions()`:

- Grants live in the agent host's own in-memory grants, so what the shell grants is what the host checks. A linked folder is a read grant, plus a write grant marked "ask first" for `read_write`; the `web` kit is fetch on the web without asking and act on any site asking first. Other kits are a switch alone.
- `scratch_root()` is the temporary directory. A folder is linkable only from strictly inside it, judged by the path alone: nothing is opened or created. Anything else is `not_linkable`.
- `user_action(app)` mints the person's token for one `permissions.request`; `requests()` lists what the window was asked to show.
- Not modelled: a grant with `session` or `until` answers `unsupported`, and `approvals` is always empty.
