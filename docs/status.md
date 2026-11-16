# SDK: status

Newest first. History before 14 Nov 2026 is in `isonomic-desktop/docs/architecture/sdk/status.md`; this repo's own session owns the SDK from here.


## 2026-11-16 · Every model a stored preset names

**Built:** the testkit holds both models a stored preset names, `[model] id` and `title_model`, to the app's manifest. It checks every name against the manifest before it asks whether any of them runs. `agents.model_rules` gains the title-model case. From the final check of #30.

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

## 2026-11-15 · Requests from other sessions

`docs/requests.md` collects them. Open there:

- **Say why a call was refused** (Identity, isonomic-id): errors `permission_denied` and `approval_required`, with fixtures. **Accepted, scheduled after version 1**: identity is outside V1, and error codes are additive.

## 2026-11-15 · Isonomic UI's features and extra components through the SDK

**Built:** `isonomic-app` features `component` and `assets` (on by default) and `extra` (`isonomic_app::extra`, from isonomic-ui-extra at the same `UI_REF`). A scratch crate outside the workspace, depending on `isonomic-app` alone with `extra`, names items in `ui::component`, `ui::assets` and `extra`.

**Decisions**

- **Components and assets on by default.** Every app front end is made of them, and the engine never depends on `isonomic-app`, so the cost falls only on UI crates.
- **`extra` is opt-in, next to `ui`.** It is a separate crate with a large surface, so it is its own module (`isonomic_app::extra`) rather than a path under `ui`, which would need a wrapper module in place of the crate re-export.

## 2026-11-15 · Installing local models (isonomic-sdk #21, from the window)

**Built:** `models.download`, `models.download_cancel` and `models.recommended`, all shell only; `download_progress` events; `not_installed` and `downloading` states; `total_bytes` and `size_bytes` optional (absent when unknown); error `not_downloading` for a cancel with nothing running. 29 fixture lines. On the client: `Models::download` → `DownloadStream`, `cancel_download` and `recommended`. In the testkit: downloads (`DownloadStep`), `uninstall`, `recommend`, `set_size` and `TestEngine::set_profile`. 10 tests, including a download resumed after the connection drops. After review: the attach snapshot is taken under the same lock as the subscription, and a cancel after the last step still cancels.

**Decisions**

- **The download is the engine's.** Calling `download` again attaches to the running download rather than starting another, and the first event says where it stands. Cancel takes the model, not a subscription, because several windows can follow one download.
- **Below the floor is `insufficient_memory`**, with the numbers, rather than a new code.
- **Installed is not loaded.** A finished download leaves the model `stopped`; the memory gate still decides on the first request.

## 2026-11-15 · Isonomic UI and the shared toolkits through the SDK (decision, 15 Nov)

**Built:** `isonomic_app::ui` re-exports Isonomic UI; `editor` and `node-graph` features reserve empty `isonomic_app::editor` and `isonomic_app::node_graph`. `hello-app` imports only from the SDK and no longer depends on isonomic-ui. A scratch crate outside the workspace, depending on `isonomic-app` alone, derives components in two modules and runs `#[gpui::test]`.

**Decisions**

- **Imported as `gpui`.** The UI's macros expand to `gpui::` paths when the calling crate does not name isonomic-ui, so `use isonomic_app::ui as gpui;` satisfies them, one `use` per module. Teaching the macros to find `isonomic-app` would drop that line; it is a change in isonomic-ui, not filed yet.
- **One pin.** The SDK's `UI_REF` is the only one. Consumers still need the UI's source at that commit beside the SDK, and the `gpui-pre-macros` patch line in their workspace root.
## 2026-11-15 · The testkit's agent host and the `agents.` conformance group (S0, agents brief Part B)

**Built:** `testkit::agents` serves all 22 agents methods with harness-host's `MemoryHost`, plus `agents_map` (SDK types ⇄ host types, `HostError` → error codes). `TestEngine::agents()` → `FakeAgents` (`reply`, `with`, `advance`, `user_action`, `disconnect`, `reconnect`, `log`). There are 9 testkit tests, and 6 new conformance checks (`agents.presets`, `registry`, `replay`, `isolated`, `purge_own`, `unknown_subscription`). The token-stripping engine fails `agents.isolated`. No proto change.

**Decisions**

- **Each call drives the host to completion.** `MemoryHost` has no runtime; the testkit runs it under one lock with `block_on`, so a scripted turn is over when `send` answers. Streams are Tokio tasks awaiting `Subscription::next`, which the host wakes.
- **Every granted app is first-party.** Manifests carry no app kind yet. View and third-party deciding stays tested in harness-host.
- **Conformance sends no input.** The engine's model is real, so turns stay in testkit tests.

**For consumers** (review of #16, items 10 and 11):

- `wire.welcome` requires `schema`. An engine fails conformance on its next SDK pin bump until its `welcome` sends `SchemaVersion::CURRENT`.
- Declare harness with exactly the workspace's URL and `rev` string (`ssh://git@github.com/isonomic-org/isonomic-harness.git`, `f0612113f6dcc8e11528642137e6dd9b24f48165`). Any other spelling makes cargo build two copies of harness-core, whose types do not match. CI must pass `harness: true` to the shared workflow.

**Gaps, for the next pull request** (review items 6 to 9; 6 to 8 change the wire):

- A purged session's stream goes quiet: no end-of-stream event for session streams.
- No call gives an app the user-action token `approvals.decide` needs.
- `artifacts.get` returns the body inline; bodies over about 750 KB exceed the 1 MiB frame. It should be chunked like blobs.
- The client skips a malformed known event silently, and `SessionStream` does not check sequence continuity.

## 2026-11-15 · The `agents` interface: wire and client (S0, agents brief Part B)

**Built:** `proto::agents` (argument and result types; harness-core's event schema re-exported as `agents::schema`), 22 methods, 72 fixture lines in `agents.jsonl` (every agents error code, and `welcome.schema`) including an event from a newer schema revision, the `schema` field in `welcome`, the `Agents` interface in `proto::auth`, `client::Agents` and `SessionStream`. The method tables moved to `proto/src/calls.rs` to keep `lib.rs` under the size limit. 84 tests.

**Decisions**

- **harness-core by git, pinned.** The brief asks for the event types at a pinned revision; a git dependency keeps one copy of them for every app. It needs SSH access to isonomic-harness wherever the proto builds (the shared workflow's new `harness` input; isonomic-org/.github#4).
- **The SDK's own argument types.** The host's types are not serializable and pull in the loop and policy crates; the proto keeps apps on harness-core alone, and the engine maps.
- **Targets are (session, id).** Approval, question and call ids are per session in the harness, so the calls that answer one also name the session (review of #16; the host side is isonomic-harness #30).
- **Every call takes params.** `sessions.purge`, `approvals.list` and `presets.list` take a struct that accepts `{}`, so a filter can be added without a new method. Purge and approvals.list take `app`, named like `events`' scope: only the shell names another app (checked in `proto::auth`), and the shell's purge must name one.
- **The schema is published, not frozen.** harness schema v1 can still change; a re-pin of `harness-core` may need fixture edits, which means a new protocol version once the agents lines are on main.
- **Unknown stays readable.** A session event or block from a newer revision decodes as `Unknown` (harness-core's catch-all), and a fixture proves it.

**Next:** the testkit's host is harness-host's `MemoryHost`, and conformance gains an `agents.` group, in the next pull request.

Older entries (S0, 14 and 15 Nov: system.retry, conformance, sql, events, blobs, the app crate, models, tokens) are in `docs/status-archive.md`.
