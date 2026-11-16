# Changelog

Newest first. Each entry names what the engine (isonomic-capsule) and the window (isonomic-desktop) must do when they move `SDK_REF` to it.










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

## Unreleased: stricter scrubbing (quoted text at any length, the person's ID)

**Wire:** no change. The report format is unchanged; `system.macos` stays format 1's key.

**For the window and the engine**, adopting `isonomic-reports` at this commit:
- Quoted text of any length becomes `[text removed]` in every scrubbed field, not only past 40 characters. So a short typed value never survives, and nor does a short quoted key name.
- The person's ID becomes `[id removed]` in either form (`did:iso:…`, `isonomic:…`) at any length.
- Add the ID to a report only when the person ticks the box, outside the scrubbed fields.

## Unreleased: problem reports, shared (`isonomic-reports`), and `AppContext::report`

**Wire:** no change.

**New crate `isonomic-reports`**, which builds without Isonomic UI (`docs/reports.md`): format 1's `Report` with both sides' kinds and components, `scrub`, `backtrace::frames` and `short_path`, and the service-log `filter`s.

**For the engine** (isonomic-capsule `crates/reports`):
- Depend on `isonomic-reports` and delete your copies of `scrub.rs`, `frames`/`short_path`, the format types in `lib.rs`, and the generic parts of `filter.rs`.
- Keep `Source::of` and your model-server prefix list: call `filter::database_log`, `filter::allow_listed(lines, YOUR_PREFIXES)` and `filter::any_log`.
- `Component::Engine`, `Service` and `ModelServer` and `Kind::ServiceFailed` and `ModelServerFailed` are as you had them. `System.macos` is now `os_version`, still written as `macos`.
- Keep the store, the reporter, the log ring, the panic hook and the watchers.

**For the window** (isonomic-desktop `crates/reports`):
- Depend on `isonomic-reports` and delete your `scrub.rs`, your `frames`/`short_path`, and the format types.
- **The scrubber gains the engine's key rule**, so tokens, digests and long IDs in window reports are now removed.
- `Report` gains no field you lack. `System.macos` is `os_version`.
- Keep the store, the setting, the guard, the watchdog and the panic hook with `blame`.
- Handle `WindowRequest::Report(Problem)`: add the app's id and version and write an `app_error`. The SDK already keeps each app to one a minute.

**For apps:** `AppContext::report(Problem { summary, detail })`, and `TestHost::reports()` in tests.

## Unreleased: Isonomic UI at b4c9670

**Wire:** no change.

**For the engine and the window:** move `../isonomic-ui` to `b4c967050e0f0b91d880b96ec2e33dd9ea6019ee`, the commit in `UI_REF`. Additive since 2e3116b:
- `extra::run_status_bar::{RunStatusBar, RunStatus}`;
- `PromptComposer::set_attach(false, cx)` to hide attaching;
- focus fixes in four components outside version 1.

**For apps:** the UI's macros now find `isonomic-app` by themselves and expand to `isonomic_app::ui::` paths. So `use isonomic_app::ui as gpui;` is no longer needed: import from `isonomic_app::ui` and write `#[ui::test]`. Two things must hold, and both do:
- `isonomic-app` exposes the whole of Isonomic UI as `ui`;
- the app's workspace root carries the SDK's `gpui-pre-macros` patch line.

The old alias still works. hello-app now uses `isonomic_app::ui` directly.

## Unreleased: the person's Isonomic ID (`identity`, needs-master)

**Wire (additive, still version 0):** the shell-only interface `identity`. `identity.get {}` answers `identity { id, created }`, and the new error code is `identity_unavailable`. `proto::auth` refuses the interface to every app token. Nothing else about identity is on the wire in version 1 (`docs/interfaces/identity.md`).

**For the engine:**
- Serve `identity.get` from the shell's connection.
- Create the ID the first time it is needed, and answer `created: true` on that call only. Every later call, from any connection, answers the same `id` with `created: false`.
- When the key store or anything else stops you making or reading the ID, answer `identity_unavailable` with a message in plain words, and make nothing.
- Pass the new conformance check `identity.stable`; `tokens.shell_only` and `tokens.every_interface` now cover `identity.get` too.

**For the window:**
- Call `Client::identity()`, at first run and whenever it shows the ID.
- Show `id` exactly as given, and offer to copy it.
- On `identity_unavailable`, show the message as written and let the person retry.
- In tests: `TestEngine::identity()` with `set_id`, `refuse`, `recover` and `forget`.

## Unreleased: Isonomic UI at 2e3116b (V1 components pass QA)

**Wire:** no change.

**For the engine and the window:** move `../isonomic-ui` to `2e3116bfb3c39388db58c81cf898e63f3e8092cb`, the commit in `UI_REF`. Every app takes the UI through the SDK at this pin. All 86 version 1 stories pass QA.

- **API:** additive only, `Combobox::accessibility_label` and `Toggle::accessibility_label`.
- **Behaviour:** `Accordion::on_toggle_click` fires when a trigger toggles. Tab skips disabled text fields. An icon-only `Button` with a tooltip is named by it. A focused `Toggle` draws a focus ring. The prompt composer wraps its controls below about 430 px wide.

## Unreleased: harness at ae4f1a7, and every model a stored preset names

**Wire:** no change; the session event schema is the same.

**Build:** harness moves to `ae4f1a7ebe6fdde701534ceadddab987a1b40bcc`:
- chat preset version 3, with shorter answers and `max_output_tokens = 1024`;
- a compaction budget that follows the model's window, so no request is larger than the window;
- a message too long on its own ends its turn `failed` with `context.too_large`.

Every consumer declares it exactly so:

```toml
harness-core = { git = "ssh://git@github.com/isonomic-org/isonomic-harness.git", rev = "ae4f1a7ebe6fdde701534ceadddab987a1b40bcc" }
```

**For the engine:** a stored preset's `title_model` follows the app's manifest as its `[model] id` does (`agents.model_rules` checks both). Check every name against the manifest before looking any up.

## Unreleased: the model picker, and the agents follow-ups (#30)

**Build:** harness moves to `9a3809f376e5f7307727f01fad0973b64b8d9e9d` (the create-time model override and its follow-ups, and chat preset version 2). Every consumer declares it exactly so:

```toml
harness-core = { git = "ssh://git@github.com/isonomic-org/isonomic-harness.git", rev = "9a3809f376e5f7307727f01fad0973b64b8d9e9d" }
```

**Wire (additive, still version 0)**

- `ModelInfo.recommended` (omitted when false), in `models.list` and `models.info`.
- `sessions.create { model? }`: the session runs on that model. `proto::auth` lets an app name only a model its `models` capability allows (`capability_denied`), and `agents::model_name` trims it and refuses a blank name, one over 128 characters or 200 bytes, or one with a control character (`bad_request`). `SessionSummary.model` reports it (absent for the preset's model).
- `session_ended { session, reason }`, the last event of a session stream (`reason: purged`).
- `approvals.user_action { session, approval, app? }` → `user_action { token }`, shell only.
- `artifacts.get { offset?, length? }` and `artifact { offset? }`: at most `CHUNK_BYTES` a reply.

**For the engine:**
- Set `recommended` on the model `models.recommended` would name.
- On `sessions.create { model }`: answer `unknown_model`, `unsupported`, `model_unavailable`, or `capability_denied` for a remote model without `remote`; otherwise pass the model to the host. Report it in `SessionSummary.model`.
- End a session stream with `session_ended` when the host's subscription ends.
- Serve `approvals.user_action` from the UI broker (`proto::auth` refuses it to app tokens).
- Chunk `artifacts.get` like `blobs.get`.

**For the window:**
- Mint the token with `Agents::user_action(session, approval, app)` when the person acts on an approval, and hand it to the app.
- Supply `AppContext::with_window(impl WindowRequests)` and run the download step on `WindowRequest::InstallModel`.

**For apps:**
- `CreateSession { model }`, `SessionSummary.model` and `ModelInfo.recommended` for the picker.
- `cx.request(WindowRequest::InstallModel { model })` turns "not installed" into the window's download step.
- `SessionStream::next` returns `None` after `session_ended`, and reports `SessionUpdate::Gap` on a skipped sequence number.
- In tests: `TestHost::requests()`.

Earlier entries (from the agents interface back to S0: `system.retry`, conformance, sql, events, blobs, the app crate, models, tokens) are in `docs/CHANGELOG-archive.md`.
