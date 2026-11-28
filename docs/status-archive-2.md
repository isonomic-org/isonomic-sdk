# SDK: status, earlier entries (2)

Moved from `docs/status.md` to keep it under the size limit. Newest first. Older still: `docs/status-archive.md`.

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
