# Changelog

Newest first. Each entry names what the engine (isonomic-capsule) and the window (isonomic-desktop) must do when they move `SDK_REF` to it.










## Unreleased: display widgets, harness at 982d9fa (#55, needs-master)

- **Harness:** `harness-core` and `harness-host` move to 982d9fa (harness #58, merge of 928f2e6): display widgets and session event schema revision 1, additive (tool kind `display`, `tool.result.display`). `SchemaVersion::CURRENT.revision` is now 1. `ToolResult` gains `display`, so a struct literal of it needs `display: None`.
- **Proto:** `isonomic_proto::agents::display` re-exports the specs (`DisplaySpec`, `DISPLAY_SPECS`, `display_spec`, `display_tool_spec`, `validate`, `display_capable`), and `agents` re-exports `ToolKind`. No new wire types or methods; fixture lines added for a display call, its `shown` and `fallback` results, and `tools.result` on it answering `not_found`.
- **Test engine:** `tools.result` for a display call answers `not_found`; `FakeAgents::set_profile` runs the host under another hardware profile (`standard` by default).
- **Engine:** pin harness 982d9fa with this SDK_REF; answer `tools.result` for a display call `not_found`. **Window:** nothing to do. Apps render display calls (`docs/interfaces/agents.md`, Display tools).

## Unreleased: an app asks the window for the person's token (#64)

- **App crate:** `AppContext::user_action(session, approval)` asks the window for the user-action token `approvals.decide` needs, for an approval the person answered in the app's own view. The window supplies a `UserActionSource` (`AppContext::with_user_actions`) and owns the policy: it decides when the call is made, not when the future is polled. `ShellUserActions::new(shell_agents, acting)` is a ready-made source: `acting(app)` says whether the person is acting in that app's focused tab right now, and it mints with `approvals.user_action { app }`. Refusals are `UserActionError` with a stable `code()`: `no_user_input`, `unavailable`, `engine` (with the engine's `ErrorCode`), `disconnected`.
- **Test host:** `TestHost` answers `user_action` through the testkit only inside `host.user_input(|| ..)`, the stand-in for the app's input handler, and refuses with `no_user_input` elsewhere (so a token asked for in a later future fails in tests as in the window). `set_user_input(true)` holds it on; `user_actions_asked()` lists who asked.
- **No wire change:** `approvals.user_action` already exists and stays shell only.
- **Window:** install a source on each app's context, `cx.with_user_actions(Arc::new(ShellUserActions::new(client.agents(), acting)))`, where `acting(app)` is true only while that app's tab is on screen and focused and a click or key press is being handled, and at most once per input event. Without one, apps get `unavailable` and approvals answered in an app stay pending.
- **Engine:** nothing to do.

## Unreleased: harness at d7d8ab6

- **Harness:** `harness-core` and `harness-host` move to d7d8ab6, harness main after its #56: the host leaves a tool out of a new session when `Tool::offered(app)` is false, and the connected-app tools forward to `Connections::offered(owner)`. No wire change. Engines and windows must pin this same rev.

## Unreleased: connections follow-ups, harness at ef14f17

- **Client:** `connections().remove(id)` drops the id from `changes()` once the engine answers `done`; before, a removed connection stayed there until the next reconnect.
- **Test engine:** a lagged status subscriber skips what it missed instead of ending the stream.
- **Docs:** credentials go only in a connection's `secret`, never in its `url`.
- **Harness:** `harness-core` and `harness-host` move to ef14f17, harness main after its #55: the connected-app tools (`connections`, `connection_read`, `connection_act`) are in harness-toolkit over `services::Connections`, and the chat preset (version 5) allows them in place of `app_read` and `app_act`. No harness-core or host API change. Engines and windows must pin this same rev.

## Unreleased: tool server connections, the model setup, harness at 077909e (needs-master)

**Wire (additive, still version 0):**
- New interface `connections`, shell only (`auth::Interface::Connections`):
  - `connections.list {}` answers `connections [Connection]`;
  - `connections.add { name, key?, server, secret? }` answers `connection Connection`;
  - `connections.sign_in { id }` answers `sign_in { url }`;
  - `connections.remove { id }` answers `done {}`;
  - `connections.tools { id }` answers `connection_tools [{ name, description }]`.
- `server` is `{ kind: remote, url }` or `{ kind: local, command, args }`. `state` is `needs_sign_in`, `connecting`, `ready` or `failed`.
- New event `connection` (the whole `Connection`), sent to the shell on its `system.subscribe` subscription whenever one changes state.
- New `models` methods, shell only: `models.catalogue {}` answers `catalogue [CatalogueEntry]` (a `ModelDetail` flattened, with `about?`, `publisher?`, `fits` and `fit_reason?`, a sentence on why it is or isn't recommended for this machine); `models.remove { id }` answers `done {}`.
- New error codes: `connection_not_found`, `sign_in_failed`, `not_installed`, `download_running`.
- Fixtures: `connections.jsonl` lines 1–24, `models.jsonl` lines 59–70. Specs: `docs/interfaces/connections.md`, `docs/interfaces/models.md`.

**Harness:** `harness-core` and `harness-host` move to 077909e (the chat preset allows the engine tools `app_read` and `app_act`; no API change). **Engines and windows must pin this same harness rev** as the SDK, or the preset the testkit checks differs from the one the engine runs.

**For the engine:** move `SDK_REF` and the harness rev here. Answer the seven methods; `proto::auth` already refuses them to app tokens. Mint stable connection ids. Keep secrets out of logs: `ConnectionSecret`'s `Debug` prints `[secret]`; log calls only through `Debug`, never as JSON, and never log `reveal()`. Publish `connection` on every shell status subscription when a state changes. Refuse `models.remove` while a download runs. Run the conformance suite; `connections.round_trip`, `connections.unknown_id`, `models.catalogue_local` and `models.remove_unknown` are new.

**For the window:**
- Settings, connections: `client.connections()` with `list`, `add`, `sign_in` (open the `url` it answers), `remove`, `tools`, and `changes()`, a watch of the newest state per id. Pick the logo by `key`. Show `error` and every `sign_in_failed` message as written.
- Settings, models: `client.models().catalogue()` lists the rows; `remove(id)` uninstalls (cancel a running download first).
- Treat `unknown_method` as an engine that needs updating.

**Breaking for Rust:** none. `Call`, `Reply`, `Event`, `ErrorCode` and `Interface` are `#[non_exhaustive]`; so are the new `Server` and `ConnectionState`.

**Testing:** `TestEngine::connections()` (`list`, `secret`, `set_state`, `set_tools`, `fail_sign_ins`), `SIGN_IN_PAGE`, and `TestEngine::models().describe(model, about, publisher)`.

## Unreleased: the recovery kit (`identity.kit_*`, needs-master)

**Wire (additive, still version 0):**
- New methods, all shell only: `identity.kit_export { person_asked }`, `identity.kit_restore { kit, words, replace_unused }`, `identity.kit_saved {}` and `identity.kit_status {}`.
- New replies: `recovery_kit { id, words, kit }`, `kit_restored { id, words, kit, kit_made, note }` and `kit_status { saved_at, exported_at }`. `kit_saved` answers `done {}`.
- New error code: `recovery_refused`.
- The kit document never holds the words.
- `note` is the engine's plain message about the spent kit and anything set aside.
- Fixtures: `identity.jsonl` lines 144–152.
- New crate `isonomic-words` (no UI): the standard 2048-word list, pinned by its published SHA-256, with `is_word`, `completions`, `complete` and the 24-word checksum `check`.
- Spec: `docs/interfaces/identity.md`.

**For the engine:** move `SDK_REF` here and answer the four methods (its plan, isonomic-capsule "Worker reports" at 22:29 UTC, with the words kept out of the document). `proto::auth` already refuses them to app tokens. Refuse an export without `person_asked: true`, and every refusal, with `recovery_refused` and plain words. Write `note` in plain words. Take the word list from `isonomic-words` (or hold your own copy to `LIST_SHA256`). Never log the words: `RecoveryWords`' `Debug` is redacted, and so is the `kit` in `KitRestore`'s `Debug` (a person can pick the wrong file). Log a call or reply only through `Debug`, never as JSON, and never log `reveal()`. Answer a client's frame through `lenient::decode_client` and never quote client input in the engine's own errors. `FrameError::Json` now holds a `DecodeFailure` (category, line and column) instead of serde's error. Run the conformance suite; `identity.kit_apps_refused` and `identity.kit_export_needs_person` are new.

**For the window:**
- `client.kit_export(true)` only after the person asks. Show the words, save `kit` as a file, then call `kit_saved()` when the person confirms.
- Check each word as it's typed with `isonomic_words::{is_word, completions, complete}`, and all 24 with `isonomic_words::check` before restoring.
- `client.kit_restore(KitRestore { kit, words, replace_unused })` from the "I have a recovery kit" path. Show its `note` as written. Its new words and kit replace the spent one, and the person saves them again.
- `kit_status()` drives the "not saved yet" reminder.
- Show every `recovery_refused` message as written. Treat an `unknown_method` answer as an engine that needs updating.

**Testing:** `TestEngine::identity()` adds `set_now`, `set_in_use` and `kit_generation`, and `isonomic_testkit::kit_words(n)` gives a generation's words (public test vectors).

## Unreleased: Isonomic UI at fa70c9b

**Wire:** no change. Additive only.

**For Chat and the window:** move `../isonomic-ui` to `fa70c9b016514a73495f946cfd34e2bc5d153007`, the commit in `UI_REF`.
- A list indent per kind on `TextView`.
- `MessageActionBar::pager_quiet`.

Move once to this commit, and adopt the shell header (below) in the same change.

## Unreleased: the shell header (the rail folded into an app's sidebar)

**Wire:** no change. Additive to `isonomic-app`, plus one change to `test-support`, below.

**What's new:**
- `App::hosts_shell_header() -> bool`, `false` by default.
- `Registration::hosts_shell_header()`.
- `AppContext`:
  - for the app: `shell_header(window, cx) -> AnyElement`, `shell_inset() -> Pixels`, `set_sidebar(Sidebar)` and `sidebar()`;
  - for the window: `with_shell(Arc<dyn Shell>)` and `sidebar_changes() -> watch::Receiver<Sidebar>`.
- The new `shell` module: `Sidebar { Expanded, Collapsed }`, the `Shell` trait (`header(app, sidebar, window, cx)`, `inset(sidebar)`) and `ProductWordmark` (feature `extra`).
- `TestHost::shell_headers()`, and `OpenApp::sidebar` and `hosts_shell_header`.

**`test-support` change:** `test-support` now turns on `extra`. `TestHost::open` now initialises Isonomic UI (`ui::init`) once per test, as the window does.

**For Chat:**
1. `fn hosts_shell_header() -> bool { true }`.
2. In the sidebar's top row:
   - expanded: `toggle`, then `app.shell_header(window, cx)`;
   - collapsed: the header, then the toggle under it.
   - In both, pad by `app.shell_inset()`.
3. Call `app.set_sidebar(Sidebar::Collapsed | Expanded)` whenever the sidebar folds or opens.
4. Remove Chat's own wordmark.

**For the window:**
1. For each registration, `registration.hosts_shell_header()` decides whether to draw the 56-px rail.
2. Give each context a shell before opening the app:
   - one app: `cx.with_shell(Arc::new(isonomic_app::shell::ProductWordmark::of(&manifest)))`;
   - more than one: its own `impl Shell`, the ghost button with a caret and the menu of apps and Home.
   - `inset(sidebar)` returns the traffic-light room.
3. Subscribe with `cx.sidebar_changes()` before handing over the context, and draw the collapsed form and the traffic-light position from it.
4. Turn on `isonomic-app`'s `extra` feature for `ProductWordmark`.

Apps that leave `hosts_shell_header` false keep the rail.

## Unreleased: Isonomic UI at 2b8a583 (the composer)

**Wire:** no change. Additive only.

**For Chat and the window:** move `../isonomic-ui` to `2b8a583e499b8f142f87a3b1c26620edb612b851`, the commit in `UI_REF`.
- New: `PromptComposer::set_quiet_focus`.
- The comfortable composer is 96 high when empty.
- The escape hint reads "Esc".
- `Ban` and the composer's photos item use the icon set's own drawings.

## Unreleased: Isonomic UI at 527d955 (stable ids)

**Wire:** no change.

**For Chat and the window:** move `../isonomic-ui` to `527d955a9b2a79187961fa4be195a779b91164be`, the commit in `UI_REF`. Sidebar rows and menu items take an owner-set id. Model picker ids derive from the picker's id (`model_picker::list_id`, `row_id`).

**Not purely additive.** Tests that find elements by id need updating:
- Palette rows were `command-<id>-<index>`; they are now `command-<command id>`, with a `-meta` element.
- Picker rows were `("model", index)` and the list `"models"`; both now derive from the picker's id.
- `PopupMenuItem::Item` gained `id`, so patterns and literals that name every field need it.

No SDK code, test or example names these ids.

## Unreleased: Isonomic UI at a1b30e1 (the wordmark)

**Wire:** no change. Additive only.

**For Chat and the window:** move `../isonomic-ui` to `a1b30e15f315cb197acb6fd7237a457fbf7bc8d2`, the commit in `UI_REF`.
- The bundled sans gains its Light file.
- `Wordmark`, in `isonomic_ui_extra::wordmark`. The brand rule: "Isonomic" in the sans at Regular, a product's name after it in Light, same size and colour, never in the serif.
- Apps reach it as `isonomic_app::extra::wordmark::Wordmark`, with feature `extra`.
- The window's `ui` crate re-exports `isonomic_app::ui::*` only. To reach `Wordmark`, it turns on `isonomic-app`'s `extra` feature and re-exports `isonomic_app::extra`.

## Unreleased: Isonomic UI at 0a8f39d (the serif)

**Wire:** no change. Additive only.

**For Chat and the window:** move `../isonomic-ui` to `0a8f39dc05ef46b7363ad18664ab694d37c2974f`, the commit in `UI_REF`.
- A bundled serif family. Apps name it as `isonomic_app::ui::assets::SERIF` and read it from `cx.theme().serif_font_family`, for display text such as a wordmark or a greeting.
- `Clipboard::icon_size` and `MessageActionBar::icon_size`.

`isonomic_app::ui::init` (Isonomic UI's `init`) registers the bundled sans, mono and serif before the theme resolves its families. The window already calls it; an app needs nothing more. hello-app's `tests/fonts.rs` checks both points against `isonomic-app` alone.

## Unreleased: Isonomic UI at ecd21d1

**Wire:** no change.

**For Chat and the window:** move `../isonomic-ui` to `ecd21d1cfa22eb206d41882e8837e5ffff7d0c47`, the commit in `UI_REF`. Palette rows take any catalog icon, `list_indent` reaches its value, and icon buttons get `icon_size`.

**Not purely additive**, one point: the public field `PaletteCommand::icon` is now the icon's path (`Option<SharedString>`). Calls to `.icon(..)` are unaffected; struct literals and reads of the field need the new type.

## Unreleased: report ids

**Wire:** no change. Report format 1 gains an optional top-level `id`; older files stay valid.

**What changes:** `Report` has `id: Option<String>`, set by `Report::new` and written as the top-level `id`. It is never scrubbed. The new `isonomic_reports::id` module has `new_id`, `is_id`, `report_id(doc, file_name, capsule_id)` and `derived_id`. A struct literal of `Report` needs `id`.

**For the window:** its sender sends `id::report_id(&doc, file_name, capsule_id)`, so the receiver can drop duplicates.

**For the engine:** its fifth identity step uses the same `report_id` for its stored reports.

## Unreleased: Isonomic UI at 4f1207b (the options Chat needs)

**Wire:** no change.

**For Chat and the window:** move `../isonomic-ui` to `4f1207b811a3d0a2eb16a6ad48e9d1d8db496871`, the commit in `UI_REF`. New options to match the design:
- sidebar padding and focusable rows;
- `PopupMenuItem::destructive()`;
- command palette setters;
- `PromptComposer::set_comfortable`;
- model picker options, with list-box roles on rows;
- text view list styling;
- the message action bar's size;
- the pager's position and labels.

**Not purely additive**, three points:
- `PaletteCommand` gained the public fields `icon` and `meta`, so struct literals need both.
- `PopupMenuItem::Item` gained `destructive`, so patterns and literals that name every field need it.
- The component `IconName` now derives `Debug`, `PartialEq` and `Eq`; a local impl of those for it now conflicts.

Nothing in the SDK uses these.

## Unreleased: Isonomic UI at 66e93d3

**Wire:** no change.

**For the window and Chat:** move `../isonomic-ui` to `66e93d359e02166da1237370571f8966f0ce6403`, the commit in `UI_REF`. The run status bar now matches the design:
- `RunStatus::Running` and `Done` are new, so `RunStatus::ALL` has seven entries;
- buttons are a list, through `RunStatusButton`, and `.elapsed(..)` is new;
- the default words for Stopped changed;
- the ban icon is fixed.

**Not purely additive:** code that matches `RunStatus` exhaustively needs arms for `Running` and `Done`, and tests that compare the Stopped words need updating. Nothing in the SDK matches on it.


Earlier entries: from stricter scrubbing back to the model picker in `docs/CHANGELOG-archive-2.md`; from the agents interface back to S0 (`system.retry`, conformance, sql, events, blobs, the app crate, models, tokens) in `docs/CHANGELOG-archive.md`.
