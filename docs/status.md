# SDK: status

Newest first. History before 14 Nov 2026 is in `isonomic-desktop/docs/architecture/sdk/status.md`; this repo's own session owns the SDK from here.









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

## 2026-11-16 · Problem reports, shared (issue #35)

**Built:**
- `isonomic-reports`: format 1, the scrubber (the window's rules plus the engine's key rule), backtrace helpers and service-log filters, taken from the window's and the engine's copies.
- `AppContext::report(Problem)` as `WindowRequest::Report`, one per app per minute, and `TestHost::reports()`.
- 14 unit tests and 5 planted-content tests.

**Decisions**

- **Generic filters.** The engine's copy named its bundled services and a model server's log prefixes. Those are third-party names, so they stay in the engine. The crate takes the prefixes as an argument.
- **Two limits, then closed (16 Nov, the orchestrator's decision):** quoted text now goes at any length, and the person's ID goes in either form at any length (`[id removed]`). `system.macos` stays format 1's key.
- **One trait for the window.** A report is a `WindowRequest` (it is `#[non_exhaustive]`), so the window's handler compiles unchanged and answers `false` until it handles reports.

## 2026-11-16 · Isonomic UI at b4c9670: no alias needed

`UI_REF` moves to b4c9670. The UI's macros now recognise `isonomic-app` as the facade, the follow-up filed from #24. A scratch crate outside the workspace, depending on `isonomic-app` alone with no `gpui` alias, derives components in two modules, runs `#[isonomic_app::ui::test]` and names `extra::run_status_bar::RunStatusBar`. hello-app drops its alias. The guide and the app crate's docs say the alias is optional.

## 2026-11-16 · The person's Isonomic ID (needs-master)

**Built:** the shell-only interface `identity` (`identity.get` → `identity { id, created }`, error `identity_unavailable`), `Client::identity()`, the testkit's `FakeIdentity`, 8 fixture lines, 3 testkit tests and the conformance check `identity.stable`. Omar moved the Isonomic ID into version 1 on 16 Nov.

**Decisions**

- **Its own interface, shell only**, like `system` and `apps`: `Interface::Identity` is in `shell_only()`, so no app capability can reach it.
- **The ID is opaque text.** The engine owns its form, and the window shows it as given.
- **Only this in version 1.** The identity request in `docs/requests.md` (`permission_denied`, `approval_required`) stays scheduled for after version 1.

## 2026-11-16 · Isonomic UI at 2e3116b

`UI_REF` moves to 2e3116b, the QA worker's "V1 components pass QA" commit. Its public API changes since 4c912f6 are additive only. hello-app and every SDK test pass on it, with `component`, `assets` and `extra` all built.

## 2026-11-16 · Every model a stored preset names

**Built:** the testkit holds both models a stored preset names, `[model] id` and `title_model`, to the app's manifest. It checks every name against the manifest before it asks whether any of them runs. `agents.model_rules` gains the title-model case. From the final check of #30. Harness moves to ae4f1a7 (chat preset version 3, window-fit compaction); no SDK test depended on the preset's version or text.

## 2026-11-16 · Wire follow-ups and the model picker (#30, needs-master)

**Built:**
- **`lib.rs` split:** `reply.rs` holds `Reply` and `Event`, `error.rs` holds `WireError` and `ErrorCode`. Paths are unchanged.
- **Model picker:** `ModelInfo.recommended`; `sessions.create { model? }`, passed to the host's create-time override; `SessionSummary.model`. `agents::model_name` trims a name and refuses one over 128 characters (the engine's bound) or 200 bytes (the host's), or with a control character; `proto::auth` and the testkit use it.
- **Harness at 9a3809f:** the create-time model, and chat preset version 2. No SDK test depended on preset version 1.
- **From the #16 review:**
  - `session_ended` closes a session stream.
  - `approvals.user_action` (shell only) mints the person's token for (session, approval, app).
  - `artifacts.get` takes `offset` and `length`, at most `CHUNK_BYTES` a reply, and the client reassembles.
  - `SessionStream` reports a `Gap` instead of skipping silently.
- **App requests (issue #31):** `AppContext::request(WindowRequest::InstallModel { model })`, the window's `WindowRequests`, and `TestHost::requests()`.

**After review:**
- Fixture 95's last chunk now carries the 4096 bytes that end the body.
- An app's own preset is held to the model rule.
- `SessionUpdate::Ended` tells a purge from a dropped connection.
- Chunk reassembly checks each reply's `offset`, version and size.
- Reading past an artifact's end is `bad_request`.
- New fixtures for `unsupported` and the remote refusal on `sessions.create`.
- The token's lifetime is documented.
- Conformance gains `agents.model_rules` and `agents.session_ended`.
- The testkit checks the manifest before it looks a model up, so an app learns nothing about models it may not use.

**Tested against a minimal engine of its own** (`testkit/tests/artifact_chunks.rs`): chunk reassembly, and the refusal of an engine that ignores `offset`. Still without an end-to-end test: `SessionUpdate::Gap`. There is no conformance check for chunking either, because no wire call writes an artifact.

Older entries (15 Nov: requests, Isonomic UI through the SDK, installing models, the agents interface and its testkit host; S0, 14 and 15 Nov: system.retry, conformance, sql, events, blobs, the app crate, models, tokens) are in `docs/status-archive.md`.
