# Build an Isonomic app

A first-party Isonomic app is a Rust crate. It depends on `isonomic-app` (this repo), which also hands it Isonomic UI, implements one trait, and is linked into the window, which shows it as a tab. It never opens a socket, a file, a database or a network connection itself: everything goes through the client it is given, under its own capability token.

`examples/hello-app` is the whole of this guide in about 200 lines.

## 1. The crate

```toml
[dependencies]
isonomic-app = { path = "../isonomic-sdk/crates/app" }
isonomic-proto = { path = "../isonomic-sdk/crates/proto" }

[dev-dependencies]
isonomic-app = { path = "../isonomic-sdk/crates/app", features = ["test-support"] }
isonomic-testkit = { path = "../isonomic-sdk/crates/testkit" }
```

The SDK is a sibling checkout pinned by commit (`SDK_REF`). **Isonomic UI comes through the SDK** as `isonomic_app::ui`, at the commit in the SDK's `UI_REF`, and is compiled into the window once. An app's front end is a configuration of those shared components; its back end is its own logic on the SDK. Do not depend on `isonomic-ui` yourself and do not keep a `UI_REF` of your own: the SDK's pin is the only one. GPUI comes only through it; name it nowhere else.

Import the components from `isonomic_app::ui`. The UI's macros (derives, `#[ui::test]`) find it there by themselves, so an app needs no alias:

```rust
use isonomic_app::ui::{self, div, IntoElement, ParentElement, Render, Window};
```

The older `use isonomic_app::ui as gpui;` still works, for code written that way.

The build still needs the UI's source next to the SDK: a checkout of `../isonomic-ui` at the SDK's `UI_REF`, and in your workspace root the same patch line the SDK has, `gpui-pre-macros = { path = "../isonomic-ui/vendor/gpui-pre-macros" }` under `[patch.crates-io]`.

Features of `isonomic-app`, on top of the core it always has:

| Feature | Default | Gives |
| --- | --- | --- |
| `component` | on | `isonomic_app::ui::component`, the styled component library |
| `assets` | on | `isonomic_app::ui::assets`, the icon set and fonts |
| `extra` | off | `isonomic_app::extra`: the prompt composer, the thinking indicator, approval and tool cards and more (turns on `component`) |
| `test-support` | off | `isonomic_app::testing` |

A window or app that uses the defaults needs no feature line; Chat adds `features = ["extra"]`. The shared toolkits come the same way, behind features of `isonomic-app`: `editor` (`isonomic_app::editor`) and `node-graph` (`isonomic_app::node_graph`). Both are reserved and empty until those toolkits exist; an app may turn them on now, and nothing breaks when they fill in.

## 2. The manifest

The manifest says who the app is and what it may reach. The user sees it; the engine derives the app's token from it and refuses every call it does not allow. Ask for the least you need.

```rust
fn manifest() -> Manifest {
    Manifest {
        id: "com.isonomic.hello".into(),     // reverse-DNS, stable forever
        name: "Hello".into(),
        version: "0.1.0".into(),             // major.minor.patch
        icon: "sparkle".into(),              // an Isonomic UI icon name
        capabilities: Capabilities {
            models: Some(ModelAccess { any: true, ..Default::default() }),
            ..Default::default()
        },
        schemas: vec![],                     // SQL schemas it creates
    }
}
```

| Capability | Example | Grants |
| --- | --- | --- |
| `models` | `ModelAccess { any: true, .. }` or `{ ids: vec!["chat-small".into()], remote: false, .. }` | Those models; remote providers only with `remote` |
| `sql` | `SqlAccess { schemas: vec!["contacts".into()] }` | Its own `schemas`, read and write; reading the schemas listed here |
| `blobs` | `BlobAccess { max_bytes: Some(10 << 20) }` | The blob store |
| `events` | none needed | Its own scope's events, always; never another app's |
| `connections` | `vec![ConnectionAccess { provider: "calendar".into() }]` | Linked accounts of that kind |
| `egress` | `vec!["example.com".into()]` | Outbound HTTP to those hosts, once the user approves |
| `agents` | `AgentsAccess {}` | Its own agent sessions, presets and artifacts |

## 3. The trait

```rust
impl App for Hello {
    fn manifest() -> Manifest { /* above */ }

    fn build(cx: AppContext, _window: &mut Window, app: &mut gpui::App) -> AnyView {
        app.new(|view| Hello::new(cx, view)).into()
    }

    fn open_link(&mut self, url: &str) -> bool { /* isonomic://hello/... */ }
    fn search(&mut self, q: &str) -> Vec<SearchHit> { /* for the window's search */ }
    fn on_unload(&mut self) { /* drop caches; the tab is being unloaded */ }
}
```

- `build` is called on first open and again after an unload. Return `Entity<Self>` as the root view: the window routes `open_link`, `search` and `on_unload` to it.
- An app keeps no state the engine does not have. Small machines unload tabs; a reopened tab rebuilds from the engine.

## 4. What the app is given

`AppContext` is cheap to clone; pass it into views.

- `cx.client()`: an `AppClient`. Every call names the app's token. The interfaces hang off it; each is specified in `docs/interfaces/<name>.md`:

  | Interface | From the client | For |
  | --- | --- | --- |
  | `models` | `.models()` | Listing models, streaming a chat reply, embeddings |
  | `sql` | `.sql()` | The app's own schemas: queries, writes, batches, live queries |
  | `blobs` | `.blobs()` | Bytes by handle: attachments, images, exports |
  | `events` | `.events()` | Live messages between the app's own tabs and windows |
- `cx.settings()`: the app's own settings, plain JSON values keyed by name. No app sees another's.
- `cx.set_attention(Attention::Dot)` or `Attention::Count(n)`: the dot on the rail.

Calls are async. The client's connection runs on its own Tokio task; its methods and streams can be awaited from GPUI's executor, so a view does its work in `cx.spawn`:

```rust
cx.spawn(async move |this, cx| {
    let models = app.client().models();
    let list = models.list().await?;
    let mut stream = models.stream(StreamRequest {
        messages: vec![Message::user("Say hello")],
        ..Default::default()
    }).await?;
    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        this.update(cx, |view, cx| { view.reply.push_str(&chunk.text); cx.notify(); })?;
    }
    anyhow::Ok(())
}).detach();
```

The other interfaces work the same way:

```rust
// A thread list that follows the database.
let sql = app.client().sql();
let mut threads = sql.live("select id, title from threads order by updated desc", vec![]).await?;
let rows = threads.rows().clone();
this.update(cx, |view, cx| { view.threads = rows; cx.notify(); })?;
while let Some(rows) = threads.changed().await {
    let rows = rows.clone();
    this.update(cx, |view, cx| { view.threads = rows; cx.notify(); })?;
}

// An attachment: keep the handle, not the bytes.
let stored = app.client().blobs().put("image/png", &bytes).await?;

// Tell the app's other tabs; each skips events carrying its own sender.
let data = Json::new(&json!({ "thread": 42 }))?;
app.client().events().publish("chat.thread-renamed", data, Some(&tab_id)).await?;
```

Never block the foreground thread, and never poll: await the client, and call `cx.notify()` when state changes. Dropping a `ModelStream` cancels it; dropping a `LiveQuery` or an `EventStream` unsubscribes. Copy what you need out of `client().live()` and drop the borrow at once: while one is held, the client cannot update the view.

Errors are typed. `ClientError::Engine(error)` carries a stable `error.code` (`capability_denied`, `insufficient_memory`, ...) and a readable `error.message`; `ClientError::Disconnected` means the engine is restarting, and the client reconnects by itself. `cx.client().live()` tells a view when it is back.

### Asking the window

Some things only the window may do. An app asks with `cx.request(WindowRequest::InstallModel { model })`: the window runs its download step with the person, and the model's state in `models.info` shows the outcome. `request` returns `true` when the window took it on. The window supplies the handler (`AppContext::with_window(impl WindowRequests)`); in tests, `TestHost::requests()` lists every request made.

A handled problem the person should know about goes the same way: `cx.report(Problem { summary, detail })`. Write the summary in plain words, with nothing the person typed. The window adds the app's id and version, scrubs it and shows the problem-report notice. An app reports at most one a minute (`REPORT_INTERVAL`); `report` returns `false` past that. In tests, `TestHost::reports()` (`docs/reports.md`).

### The shell header

The window draws no rail beside an app that hosts its shell header. The app's sidebar carries the window's part instead:

```rust
impl App for Chat {
    fn hosts_shell_header() -> bool { true }
    /* ... */
}

// In the sidebar's render:
let header = self.app.shell_header(window, cx);
let top = div().pt(self.app.shell_inset());
let top = match self.app.sidebar() {
    Sidebar::Expanded => top.flex().flex_row().child(toggle).child(header),
    Sidebar::Collapsed => top.flex().flex_col().child(header).child(toggle),
};

// When the person folds or opens the sidebar:
self.app.set_sidebar(Sidebar::Collapsed);
```

- **Placement.** `cx.shell_header(window, cx)` is the window's element.
  - Expanded, it goes beside the sidebar toggle, which stays at the left of the top row.
  - Collapsed, it goes at the top of the narrow rail, with the toggle under it.
  - Draw it on every render of the sidebar.
- **What the window puts there.** With one app, the plain wordmark in its product form ("Isonomic Chat", Isonomic UI's `Wordmark::product`) when expanded, and the app's icon when collapsed. With more than one, a ghost button with a caret that opens the window's menu of apps and Home.
- **No wordmark of your own.** An app that places the header draws no wordmark itself.
- **The sidebar's state.** `cx.set_sidebar(Sidebar::Expanded | Sidebar::Collapsed)` tells the window. It draws the header for that state and keeps the traffic lights clear. `cx.sidebar()` reads the state back; it is `Expanded` until the app says otherwise.
- **The inset.** `cx.shell_inset()` is the room to leave for the window's controls: at the left of the top row when expanded, above the header when collapsed. Without a window shell it is zero.
- **Opting in.** `App::hosts_shell_header()` defaults to `false`. Such an app keeps the window's rail, so it needn't change. The window reads the flag before its first frame, so the rail never flickers.

In tests, `TestHost` supplies the one-app header (`shell::ProductWordmark`) and records each draw: `host.shell_headers()` lists `(app, Sidebar)`. `open.sidebar` is the window's subscription, and `open.hosts_shell_header` is the flag.

## 5. Agent sessions

With the `agents` capability, `cx.client().agents()` runs agent sessions in the engine's host: create one from a preset, send input, and follow its events. Every method and rule is in `docs/interfaces/agents.md`; the event types are harness-core's, under `isonomic_proto::agents::schema`.

```rust
let agents = app.client().agents();
let session = agents.create("chat").await?;
let mut stream = agents.subscribe(&session, 0).await?;
agents.send_text(&session, "Plan three days in Lisbon").await?;
while let Some(update) = stream.next().await {
    match update {
        SessionUpdate::Delta(delta) => { /* append delta.text to the reply in progress */ }
        SessionUpdate::Event(event) => match &event.event {
            schema::Event::Assistant(reply) => { /* the finished reply: replace the deltas */ }
            schema::Event::TurnEnd(_) => break,
            _ => { /* user, tool calls, approvals, unknown events: render generically */ }
        },
    }
}
```

- **Resume, don't restart.** After a reconnect, subscribe from `stream.last_seq()`: no event is missed or repeated.
- **Deltas are a preview.** They may be dropped under load; the `assistant` event is the truth.
- **Approvals belong to the person.** `decide` needs the user-action token the window gives you when the person clicks; without it the engine refuses.
- **Client tools** (`register_client_tools`) arrive as `tool.call` events with `kind: client`; answer each with `tool_result`.

## 6. Testing

`isonomic_app::testing` (feature `test-support`) runs the app in a headless window against the in-memory engine, under a token granted from the app's own manifest. The testkit enforces that token exactly as the engine does, so a test fails if the app calls anything its manifest does not allow.

```rust
use isonomic_app::ui;

#[ui::test]
fn it_streams_a_reply(cx: &mut TestAppContext) {
    let host = TestHost::new();
    let open = host.open::<Hello>(cx);
    let hello = open.root::<Hello>(cx);
    wait_until(cx, &hello, |hello| hello.phase() == &Phase::Done);
    hello.read_with(cx, |hello, _| assert_eq!(hello.reply(), "You said: Say hello"));
}
```

- `host.engine()` scripts the engine: `models().script(..)` with `Step::Chunk`, `Step::Hold(gate)` and `Step::Fail`, `models().refuse(..)` for the memory gate, `go_offline()` and `go_online()`, and `calls()` to see every call the app made.
- `sql()` is a scripted database that runs no SQL: `respond(schema, sql, rows)` sets a query's result (and pushes it to every live query on it), `affect` sets a write's row count, `fail` makes a statement fail, and `statements()` lists what the app ran.
- `blobs()` is an in-memory store: `stored(handle)`, `count()`, `set_limit(bytes)`. Events need no script; publish from a second client.
- `agents()` is harness-host's `MemoryHost`: `reply(text)` scripts the model, `advance(ms)` moves the clock, `user_action(app, session, approval)` stands in for the person, and `log(session)` reads what was stored (`docs/interfaces/agents.md`).
- `slow(interface, delay)`, `fail(interface, error)` and `recover(interface)` make any interface slow or failing, to test spinners and error states.
- `wait_until` wakes on the view's notifications; it never sleeps on a timer.
- `open.attention` is what the rail would show; `open.sidebar` and `host.shell_headers()` are the shell header's side (above).
- `host.open` initialises Isonomic UI as the window does, so components that need the theme draw.
- No socket, no engine binary, no network: these tests run anywhere, including CI on Linux.

## 7. Registering it in the window

The window links the crate and registers it:

```rust
let apps = [isonomic_app::register::<Hello>(), /* ... */];
```

For each app the window grants a token from the manifest (`client.grant(manifest)`), makes the context (`AppContext::new(id, client.scoped(token), settings)`), and opens the app in a tab (`registration.open(cx, window, app)`, or `Tab::open`). It keeps the `LoadedApp` to route deep links, search and unload, and watches the attention receiver for the rail.

For the shell header:
1. `registration.hosts_shell_header()` says whether to leave out the rail.
2. The window gives the context its header with `AppContext::with_shell(Arc<dyn Shell>)`. `shell::ProductWordmark::of(&manifest)` is the one-app form. With more apps, the window's own `Shell` returns its switcher. Both implement `header(app, sidebar, window, cx)` and `inset(sidebar)`.
3. `cx.sidebar_changes()`, taken before the context is handed to the app, is the window's `watch::Receiver<Sidebar>`.

## Checklist

- [ ] The manifest asks for the least it needs, and `Manifest::validate` passes.
- [ ] No network, file, process or socket code in the crate.
- [ ] No GPUI version named, no `isonomic-ui` dependency and no `UI_REF` of the app's own: components only through `isonomic_app::ui`.
- [ ] Every interaction tested against `TestHost`, including refusals and a disconnect.
- [ ] Live queries and event streams belong to a view and are dropped with it.
