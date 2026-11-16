# Changelog

Newest first. Each entry names what the engine (isonomic-capsule) and the window (isonomic-desktop) must do when they move `SDK_REF` to it.










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

## Unreleased: Isonomic UI's features through the SDK (isonomic-sdk #27, isonomic-chat O8)

**Wire:** no change.

**For apps and the window:** `isonomic-app` forwards Isonomic UI's features. `component` (`ui::component`) and `assets` (`ui::assets`) are on by default, so a consumer that drops its own isonomic-ui dependency keeps both with no feature line. The new `extra` feature re-exports `isonomic-ui-extra` as `isonomic_app::extra` and turns on `component`.

- The window: `isonomic-app = { path = "../isonomic-sdk/crates/app" }`; the defaults cover it.
- Chat: `isonomic-app = { path = "../isonomic-sdk/crates/app", features = ["extra"] }`, and `isonomic_ui_extra::…` becomes `isonomic_app::extra::…`.
- A consumer that wants neither sets `default-features = false`.
## Unreleased: harness at f061211

**Wire:** no change; the session event schema is the same.

**Build:** `harness-core` and `harness-host` move to isonomic-harness `f0612113f6dcc8e11528642137e6dd9b24f48165`. It brings continue after a title step, live deltas that wake subscribers, `Subscription::next_update()`, and a host keyed by (session, id) for approvals, questions and client tool calls. Every consumer declares harness with exactly this URL and rev, or cargo builds two copies of harness-core:

```toml
harness-core = { git = "ssh://git@github.com/isonomic-org/isonomic-harness.git", rev = "f0612113f6dcc8e11528642137e6dd9b24f48165" }
```

**For the engine:** the host calls take the session before the id: `decide_approval(caller, session, id, decision, token)`, `answer_question(caller, session, id, answer)` and `client_tool_result(caller, session, call, outcome)`. `purge(caller, app)` takes the wire's `app`, which is the caller's own when absent. Stream with `next_update()` to send `live_delta`s as they come.

**For apps' tests:** `FakeAgents::user_action(app, session, approval)` takes the session. The testkit streams deltas as they arrive, no longer held until the next durable event.

## Unreleased: installing local models (isonomic-sdk #21)

**Wire (additive, still version 0):** methods `models.download { model }` → `downloading { subscription, model, total_bytes? }` with `download_progress { model, received_bytes, total_bytes?, outcome?, error? }` events, `models.download_cancel { model }` → `cancelled {}` (or `not_downloading`, a new error code), and `models.recommended { profile? }` → `recommended { model, profile, size_bytes? }`. Sizes are absent when unknown, never 0. `ModelState` gains `not_installed` and `downloading`. All three methods are shell only.

**For the engine:**
- Serve `models.download`, `models.download_cancel` and `models.recommended` (`docs/interfaces/models.md`, "Installing a model"). `proto::auth` already refuses them to app tokens.
- Own the download, so it outlives the window. Attach a second `models.download` to the running one, starting with its current progress. End every subscriber with `installed`, `failed` (with the error) or `cancelled`.
- Report `not_installed` and `downloading` in `models.info`, and refuse streams and embeddings on a model that is not installed with `model_unavailable`.
- Answer `models.recommended` from the hardware profile. Below V1's 16 GiB floor, answer `insufficient_memory` with `required_bytes` the floor and `available_bytes` the machine's total physical memory.
- Leave `total_bytes` and `size_bytes` out when the size is unknown; answer `models.download_cancel` with nothing running with `not_downloading`.

**For the window:** `Models::recommended(profile)`, `download(model)` → `DownloadStream` (`total_bytes`, `next`), and `cancel_download(model)`, on the shell's client. After a reconnect, call `download` again to pick the download back up. In tests, `TestEngine::set_profile` and `models().uninstall`, `script_download` and `recommend`.

## Unreleased: Isonomic UI through the SDK

**Wire:** no change.

**For apps and the window:** Isonomic UI now comes through the SDK, as `isonomic_app::ui`, at the commit in the SDK's `UI_REF` (4c912f6), compiled into the window once.

- Drop `isonomic-ui` from your dependencies and delete your own `UI_REF`.
- Write `use isonomic_app::ui as gpui;` where you had `extern crate isonomic_ui as gpui;`, and `gpui::` where you wrote `isonomic_ui::`. The `use` goes in every module that derives a component or runs `#[gpui::test]`.
- Keep the `../isonomic-ui` checkout, but at the SDK's `UI_REF`, and keep the `[patch.crates-io] gpui-pre-macros` line in your workspace root.
- `isonomic-app` features `editor` and `node-graph` reserve `isonomic_app::editor` and `isonomic_app::node_graph` for the shared toolkits; both are empty for now.

## Unreleased: `test-support` builds outside this workspace

**Wire:** no change.

**For apps:** `isonomic-app`'s `test-support` feature now turns on Tokio's multi-thread runtime, which `TestHost` uses. An app crate outside this workspace no longer needs to add it (requested by isonomic-chat, O6).
## Unreleased: the testkit's agent host, and the `agents.` conformance group

**Wire:** no change.

**Build:** the testkit now depends on `harness-host` (isonomic-harness, the same pinned commit as `harness-core`).

**For the engine:** run the conformance suite's new `agents.` checks against the engine host (`docs/conformance.md`). They need no model: none sends input, and each purges what it created.

**For the window and apps:** agents calls in tests now work against `TestEngine` instead of answering `unknown_method`. `TestEngine::agents()` scripts the model, the clock and the person (`docs/interfaces/agents.md`, Testing).

## Unreleased: the `agents` interface (wire and client)

**Build:** the proto now depends on `harness-core` (isonomic-harness, pinned by commit in the workspace `Cargo.toml`) for the agent session event types it re-exports. Cargo fetches it over SSH. Every repository that builds the proto needs read access to isonomic-harness: in CI, add `harness: true` to the shared workflow call (needs the `HARNESS_DEPLOY_KEY` org secret).

**Wire (additive, still version 0)**

- 22 methods in `sessions.*`, `approvals.*`, `questions.answer`, `tools.*`, `presets.*` and `artifacts.*` (`docs/interfaces/agents.md`); replies `session_created`, `sessions`, `session_snapshot`, `done`, `accepted`, `purged`, `session_stream`, `approvals`, `presets`, `preset`, `preset_saved`, `artifact`, `artifacts`; events `session_event` and `live_delta`; error codes `not_found`, `invalid_preset`, `class_not_supported`, `not_at_turn_boundary`, `continue_refused`, `not_allowed_to_decide`, `missing_user_action`, `already_decided`, `session_failed`, `invalid_tools`.
- `approvals.decide`, `questions.answer` and `tools.result` name the session as well as the id: ids are minted per session. `sessions.purge`, `approvals.list` and `presets.list` take `params` (`{}` is fine); purge and approvals.list take an optional `app`, which only the shell may set to another app (`capability_denied`, checked in `proto::auth`); the shell must name the app it purges (`bad_request` otherwise). `Agents::purge(app)` and `Agents::approvals(app)` pass it.
- `welcome` gains `schema { version, revision }`, the session event schema the engine writes. Optional: an older engine or client simply leaves it out.
- The `agents` capability is open: a manifest may set it (`AgentsAccess {}`), and `ManifestError::AgentsReserved` is gone.

**For the engine** (Part A of the agents brief): serve each method by calling harness-host after checking the token, mapping the SDK's argument and result types to the host's and `HostError` to the error codes above (`Unauthorized` → `capability_denied`, `Store` → `internal`, `Model` → `model_unavailable`). Put `SchemaVersion::CURRENT` in `welcome`.

**For the window:** `Client::agents()` and `AppClient::agents()`: every call, and `subscribe` → `SessionStream` (`next`, `last_seq`), which unsubscribes when dropped. `Live::schema`. `Reply::SessionSnapshot`, `Reply::Preset` and `Reply::Artifact` are boxed.

## Unreleased: Isonomic UI at 4c912f6

**Wire:** no change.

**For the engine and the window:** move `../isonomic-ui` to `4c912f61a1ea80cdf10d369fe7cf013ddbc2228a`, the commit in `UI_REF`. Every V1 repo pins this commit.

Earlier entries (S0: `system.retry`, conformance, sql, events, blobs, the app crate, models, tokens) are in `docs/CHANGELOG-archive.md`.
