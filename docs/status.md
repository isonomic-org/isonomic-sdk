# SDK: status

Newest first. History before 14 Nov 2026 is in `isonomic-desktop/docs/architecture/sdk/status.md`; this repo's own session owns the SDK from here.









## 2026-11-28 · Permissions, remembered approvals and kits on the wire; harness at ab016a2 (#59, needs-master)

Issue 2 of the tool kits build ("Tool kits: architecture v0.3"). Differences from the design are in `docs/kits-deviations.md`.

- **Built:** the `permissions` interface (grant, list, revoke, request), `remember` on `approvals.decide`, `kits` in the manifest's agents capability checked by `proto::auth`, the code `not_linkable`, `isonomic_proto::agents::kits`, the client's `permissions()`, the test engine's in-memory grants with a scratch root, and three conformance checks with the token-stripping canary.
- **Harness** moves to ab016a2 (harness #61). Schema revision stays 1.
- **Waiting on the harness's files kit:** the kit tool specs in the re-export, and the conformance checks `permissions.revoked`, `kits.remember` and `kits.offered`. Each needs a kit tool the host can call, and the chat preset naming a kit; neither is on harness main yet. They also need a decision: no conformance check sends input today, and all three need a tool call.
- **Open, for the owner:** the design does not say how the window learns that an app called `permissions.request`, nor how the window mints the user-action token for it (`approvals.user_action` is bound to a session and an approval). Neither was invented here. The test engine records requests for tests to read and mints the token from its scripting handle.
- Labelled needs-master: it adds to the wire and moves the pinned harness.

## 2026-11-28 · Display widgets re-exported; harness at 982d9fa (#55)

- Harness moves to 982d9fa (harness #58, display widgets, schema revision 1). `isonomic_proto::agents::display` is the one place apps read the specs. The test engine answers `tools.result` on a display call `not_found`, and `FakeAgents::set_profile` tests the profile gate.
- Labelled needs-master: it moves the pinned session event schema.

## 2026-11-28 · An app asks the window for the person's token (#64)

- `AppContext::user_action(session, approval)` over a window-supplied `UserActionSource`; `ShellUserActions` for the window; the test host answers it and can refuse. Stable refusal codes `no_user_input`, `unavailable`, `engine` (with the engine's `ErrorCode`), `disconnected`. No wire change.
- The window decides at the moment of the call, so an app must call it inside the input handler and await afterwards. A kit that calls it inside a future started later will be refused in the window.
- Labelled needs-master: it is the approvals policy surface between app and window, though the wire is unchanged.

## 2026-11-27 · Harness at d7d8ab6

- Harness moves to d7d8ab6 (harness #56): a tool can decline a new session (`Tool::offered`), and the connected-app tools decline an app with nothing connected (`Connections::offered`, true by default). No harness-core change, no SDK type or wire change. Engines and windows must pin this same rev.

## 2026-11-27 · Connections follow-ups from the independent review; harness at ef14f17

- `connections().remove(id)` takes the id out of `changes()` on `done`.
- The test engine's status stream survives a lagged receiver.
- `docs/interfaces/connections.md`: credentials go only in `secret`.
- Harness moves to ef14f17 (harness #55): the connected-app tools live in harness-toolkit, and the chat preset allows them instead of `app_read` and `app_act`, which the engine was to build itself against the plan. No SDK type changes.

## 2026-11-27 · Tool server connections and the model setup (needs-master)

The orchestrator's three asks, in one change: the `connections` interface, `models.catalogue` with `models.remove`, and harness at 077909e.

- **`connections`**, shell only for every call: `list`, `add`, `sign_in`, `remove`, `tools`, the `connection` event, and the codes `connection_not_found` and `sign_in_failed`.
- **The event's subscription.** The ask said to send `connection` "on the same subscription the shell uses for `download_progress`". A download's progress comes on the subscription its own `models.download` opens, and `sign_in`'s reply opens none. So the event goes on the shell's status subscription (`system.subscribe`), the one the client always holds. Changes then reach the window whatever caused them, not only a sign-in. The client folds them into `Connections::changes()`, a watch of the newest state per id, emptied on disconnect.
- **The secret.** `ConnectionSecret` follows `RecoveryWords`: `Debug` prints `[secret]`, `Deserialize` refuses in fixed text, and `Serialize` stays transparent. `words_never_echo.rs` also sends broken `connections.add` frames holding a secret.
- **Unknown kinds.** `Server` and `ConnectionState` are `#[non_exhaustive]`, with `Unrecognized` for a newer engine's kind, so `connections.list` never fails over one.
- **`models.catalogue` and `models.remove`** are window-only calls, like the install calls. `remove` adds the codes `not_installed` and `download_running`. A remote model answers `unsupported`.
- **Harness** moves to 077909e: the chat preset allows `app_read` and `app_act`. No SDK test depended on the preset's tool list.
- **Conformance:** `connections.round_trip`, `connections.unknown_id`, `models.catalogue_local` and `models.remove_unknown`. `tokens.shell_only` and `tokens.every_interface` read the new fixtures.
- **Changelog:** the entries from stricter scrubbing back to the model picker moved to `docs/CHANGELOG-archive-2.md`, to keep the changelog under the size limit.

Not merged here: it waits for the independent reviewer.

## 2026-11-25 · Isonomic UI at 6e0c4a5

`UI_REF` moves to 6e0c4a5: the stopped run status draws a filled stop, and `ThinkingIndicator` gains `elapsed`, `finished`, `reasoning` (a keyboard disclosure), `default_expanded` and `on_toggle`. Additive only. Nothing in the SDK uses them, and every test passes on it.

## 2026-11-16 · The recovery kit on the wire (needs-master)

`identity.kit_export`, `kit_restore`, `kit_saved` and `kit_status`, all shell only, with the new error code `recovery_refused`. These are the shapes from the engine's plan, with the orchestrator's decision applied: **the kit document holds the ID, the key log and a fingerprint of the recovery key, never the words.**

- **The restore reply** carries `note`, the engine's plain message, so the wording lives in one place.
- **The word list:** the new crate `isonomic-words` bundles the standard 2048-word English list. It's pinned by its published SHA-256 (`2f5eed53…dbda`), and a test holds the file to that hash. It gives `is_word`, `completions`, `complete` and the 24-word checksum `check`. The engine has no list yet, so it takes this one. The test engine checks words with it too.
- **The words:** `identity::RecoveryWords`, a JSON array.
  - Its `Debug` prints only a count, so every call, reply and assertion that holds words prints redacted.
  - `Serialize` stays transparent, so a JSON log would print the words (documented).
  - Its hand-written `Deserialize` refuses in fixed text.
  - `decode_client` never quotes an undecodable frame, whatever it holds: only fixed text, serde's category, and the line and column. An unknown method is never named back, since words joined by dots look like one. The strict decoder's `FrameError` says the same and no more, in `Display` and in `Debug`.
- **First review (DO NOT MERGE), fixed in the second push:**
  - A string-valued `words` was echoed in the `bad_request` message. Params sent as one string were echoed too, which the review did not list.
  - New lines named the computer by its maker's product name; they now say "this computer".
  - The test engine's document had no check; it now has one, and an edited ID is refused.
  - The generation now uses `checked_add`.
  - `sha2` is a workspace dependency.
  - Tests cover every leak path found.
- **Second review (DO NOT MERGE), fixed in the third push:**
  - The kit-only guard guessed the method from the frame. A duplicate `method`, the frame as a string or an array, words in a non-kit request, in `hello` or in the method name all still came back.
  - Every undecodable frame is now content-free, and the strict path too.
  - `words_never_echo.rs` sends all of these; against the second push's decoder it fails.
  - Fixture 149's new kit carries its own check, and the last test name says "computer".
  - Constant-time comparison and zeroing on drop are a follow-up for the engine: isonomic-capsule #81.
- **Fixtures** use only the word-list standard's public 256-bit test vectors.
- **The test engine's kit:** deterministic (public vectors by generation, its own JSON document, an injected kit clock). It restores across test engines and refuses wrong words, damaged or spent kits, a used computer, and an unused one without `replace_unused`.
- **Conformance:** the checks `identity.kit_apps_refused` and `identity.kit_export_needs_person`. No check exports a kit.
- **Consumers on the old wire:** an older engine answers `unknown_method`. An older client never sends these calls, and would read `recovery_refused` as `unrecognized`.

Not merged here: it waits for the independent reviewer.

## 2026-11-16 · Isonomic UI at fa70c9b

`UI_REF` moves to fa70c9b: a list indent per kind on `TextView` and `MessageActionBar::pager_quiet`. Additive only. Nothing in the SDK uses them, and every test passes on it.

## 2026-11-16 · The shell header

The window's rail can fold into an app's sidebar (the window worker's sketch, isonomic-desktop's "Worker reports" at 12:54 UTC). An app opts in with `App::hosts_shell_header`. It places `AppContext::shell_header` beside its sidebar toggle and reports folding with `set_sidebar`. The window supplies the element through the `Shell` trait (`with_shell`) and follows `sidebar_changes`.

- **One app:** `shell::ProductWordmark` gives "Isonomic <name>" through `Wordmark::product` when expanded, and the app's icon when collapsed.
- **No flicker:** the opt-in is static, so the window decides on its rail before the first frame. Apps that don't opt in keep the rail.
- **hello-app:** it hosts the header, with a toggle, and tests it.
- **`TestHost`:** it supplies the one-app header and records each draw.
- **Deviations:**
  - `TestHost::open` now runs `ui::init` once, because a component that reads the theme (the wordmark) panicked without it. The window has always done this.
  - `test-support` turns on `extra`.
  - The `request` and `report` docs on `AppContext` were swapped; they're fixed.
  - **CI ran out of disk** linking this change's two new test binaries. Every test binary links the whole interface stack, so:
    - `crates/app/tests/{extra,shell}.rs` are now modules of `context.rs`;
    - hello-app's `tests/{fonts,shell}.rs` are now modules of `window.rs`;
    - That leaves two such binaries for the app crate and hello-app, where main had four.
    - Building dependencies without debug info was tried and dropped. It rebuilt every dependency next to CI's restored cache, and the cache save then ran out of disk. Trimming debug info needs a fresh cache, which means a change to the shared workflow.

## 2026-11-16 · Isonomic UI at 2b8a583

`UI_REF` moves to 2b8a583. New: `PromptComposer::set_quiet_focus`. The comfortable composer is 96 high when empty, the escape hint reads "Esc", and `Ban` and the photos item use the set's own drawings. Additive only. Nothing in the SDK uses them, and every test passes on it.

## 2026-11-16 · Isonomic UI at 527d955

`UI_REF` moves to 527d955: stable ids for sidebar rows, menu items, palette rows (`command-<command id>`) and model picker rows (from the picker's id). The changed ids and `PopupMenuItem::Item`'s new `id` touch nothing in the SDK, its tests or hello-app. Every test passes on it.

## 2026-11-16 · Isonomic UI at a1b30e1

`UI_REF` moves to a1b30e1: the sans in Light, and `Wordmark` in `isonomic_ui_extra::wordmark`. Additive only.

- `Wordmark` reaches apps as `isonomic_app::extra::wordmark::Wordmark`. The new `crates/app/tests/extra.rs` draws one in a test window through that path.
- hello-app's fonts test now also checks that the bundled files carry the sans in Light.
- The older changelog entries, from the agents interface down, moved to `docs/CHANGELOG-archive.md` to keep the changelog under the size limit.

## 2026-11-16 · Isonomic UI at 0a8f39d

`UI_REF` moves to 0a8f39d: a bundled serif (`Theme::serif_font_family`, `assets::SERIF`) and an `icon_size` on the clipboard and the message action bar. Additive only.

The serif reaches apps through `isonomic_app::ui`. Isonomic UI's `init` registers the bundled files. hello-app's new `tests/fonts.rs` checks that the files carry all three families and that the theme names the serif. GPUI's headless test platform lists no fonts, so the test checks the files handed to `add_fonts`, not the platform's list. Under that platform the sans also resolves to a system font, so the test doesn't assert it.

## 2026-11-16 · Isonomic UI at ecd21d1

`UI_REF` moves to ecd21d1: palette rows take any catalog icon, `list_indent` reaches its value, icon buttons get `icon_size`. The one non-additive point, `PaletteCommand::icon` as the icon's path, touches nothing in the SDK. hello-app and every test pass on it.

## 2026-11-16 · Report ids

Format 1 gains an optional top-level `id` (32 hex characters, made by `Report::new`, never scrubbed). `id::report_id` returns it, or for an older file derives a stable one from the file name and the Capsule's id. There is no random-number dependency: the bits come from the standard library's random hasher keys, so an id is unique, not secret.

## 2026-11-16 · Isonomic UI at 4f1207b

`UI_REF` moves to 4f1207b, with the options Chat needs to match the design. The three non-additive points (`PaletteCommand`'s `icon` and `meta`, `PopupMenuItem::Item`'s `destructive`, `IconName`'s derives) touch nothing in the SDK. hello-app and every test pass on it, and a crate outside the workspace still builds against `isonomic-app` alone.

## 2026-11-16 · Isonomic UI at 66e93d3

`UI_REF` moves to 66e93d3: the run status bar matches the design (`Running`, `Done`, `RunStatusButton`, `.elapsed`), and the ban icon is fixed. No SDK code matches `RunStatus`. hello-app and every test pass on it, and a crate outside the workspace still names `RunStatusBar` through `isonomic_app::extra`.

Older entries: 16 Nov, from the shared problem reports back to the model picker, in `docs/status-archive-2.md`; 15 Nov and S0 (requests, Isonomic UI through the SDK, installing models, the agents interface and its testkit host, system.retry, conformance, sql, events, blobs, the app crate, models, tokens) in `docs/status-archive.md`.
