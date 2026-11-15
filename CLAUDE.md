# isonomic-sdk

The contract between window and engine, and the platform every Isonomic app is built on. Status and decisions: `docs/status.md`. Original spec: `isonomic-desktop/docs/architecture/sdk/spec.md`.

## Rules

1. **The wire format is frozen by fixtures.** Every message shape is a line in `crates/proto/fixtures/v<version>/`. Add lines freely; never edit one (CI refuses). Changing a shape means a protocol version bump, and needs Omar's approval. See `docs/versioning.md`.
2. **No engine or window code here.** Types, the client, the test engine, and the app crate. Only `crates/app` and `examples/` may depend on Isonomic UI; `proto`, `client`, `testkit` and `conformance` must build without it.
3. **No polling.** The client publishes changes on a watch; callers await them.
4. **Hermetic tests only.** No socket, no engine binary, no network: use the test engine.
5. **Small files.** Every source and doc file stays under 20 KB.
6. **Names.** `ISONOMIC_` for environment variables, `com.isonomic.` for identifiers. No other product, repo or account name anywhere.
7. **Tokens everywhere.** Every app call carries its token; the testkit refuses what the manifest does not allow, through the same `proto::auth` check the engine uses.
8. **Every interface change** gets fixtures, a `docs/interfaces/<name>.md` update and a `docs/CHANGELOG.md` entry.

## Workflow

- One task per branch and pull request. Every PR ends with a status line: ready, blocked, failed or waiting. A merge follows the standing rules below.
- Before pushing: `cargo fmt --all`, `cargo clippy --all-targets --all-features -- -D warnings`, `cargo test --all-features`.
- After a merge here, tell the orchestrator the new commit so the engine and window workers move their `SDK_REF`.

## Standing rules

Set by Omar for every V1 repository. They hold for every session that works here.

- Only work in isonomic-sdk. If you need a change in another repo, file it in that repo's docs/requests.md or as an issue, and tell the orchestrator.
- Start from main, the open pull requests and docs/status.md. Open pull requests and branches from earlier sessions are yours now: finish, rebase or close them.
- Commit as Omar Qadmiri <omar@isonomic.com>; check `git config user.email` first. Commit and push work in progress often.
- Repos stay private. No license files. Never rename or delete a repo.
- In code, comments, commits, branches, issues and pull requests, never name a third-party product or company, another AI system, or an older product name.
- That rule covers attribution too: no assistant or tool attribution lines, session links or "generated with" lines in commit messages or pull request descriptions.
- Branch from main and open a pull request. Required checks: "rust / fmt, clippy, test (ubuntu-latest)" and "rust / file sizes and names".
- Merge your own pull request once every required check is green. Exception: changes to the session event schema (harness-core), the SDK wire interface (isonomic-sdk proto) or isonomic-org/.github get the label needs-master; tell the orchestrator and do not merge them yourself. An independent reviewer merges those.
- GitHub Actions has a hard monthly budget. Don't trigger macOS runs.
- GitHub GraphQL is blocked in worker sessions (gh pr and gh issue fail); use the built-in GitHub tools or gh api (REST).
- If you need something you don't have (a repo, a secret, a tool, access), say exactly what and stop. Don't mock or guess around it.
- Performance targets are measured on Omar's Intel MacBook Pro. Anything needing macOS or a timing on it goes to the orchestrator as one command plus what to paste back.
- The orchestrator is a session on Omar's Mac. It sends tasks by cross-session message. Report by adding a comment to the open issue titled "Worker reports" in this repo; start each report with what it is and PASS, FAIL, DONE or BLOCKED.
