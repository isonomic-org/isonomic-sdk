# Problem reports

`isonomic-reports` holds the problem-report format and the scrubbing rules that the window and the engine share. It builds without Isonomic UI. Each side keeps what differs: where reports are stored, the setting that allows them, the panic hook, and which of its services follows which log filter.

## Format 1

One JSON document per report, written by `Report::to_json(&build, &system, home, unix_secs)`:

| Field | What |
| --- | --- |
| `format` | `1` |
| `kind` | `panic`, `hang`, `app_failed`, `app_error` (the window); `panic`, `service_failed`, `model_server_failed` (the engine) |
| `component` | `{ id, version? }`: `window`, an app's id, `engine`, `engine/<service>` or `engine/<runtime>` |
| `product` | `{ version, build, arch, pins: { sdk, chat, engine } }` |
| `system` | `{ macos, profile, memory_gb, capsule_format }` (`macos` is format 1's key for the OS version; `System::os_version` in Rust) |
| `when` | `{ unix_secs, uptime_secs }` |
| `message` | Scrubbed, cut at 300 characters |
| `backtrace` | Function names and crate-relative places (`backtrace::frames`), scrubbed |
| `log_tail` | Up to `LOG_LINES` (50) lines, filtered for their source, scrubbed |
| `tab` | The tab on screen (the window), else `null` |
| `service` | The engine's `service_failed`: `{ name, version, restarts, last_exit }` |
| `model` | The engine's `model_server_failed`: `{ id, runtime, restarts, last_exit }` |

## Scrubbing (`scrub`)

Every text field goes through `scrub::line` (the message through `scrub::message`):

- Paths under the home folder become `~/…`.
- Quoted text (`"…"` or `'…'`) of any length becomes `[text removed]`, so a short typed value goes too. An apostrophe in prose pairs with the next one, so prose can lose a little more than it must.
- The person's ID becomes `[id removed]` whatever its length: `did:iso:` or the readable `isonomic:` form, with the characters that follow. The window adds the ID back only when the person asks, outside the scrubbed fields.
- A run of 24 or more key characters (letters, digits, `_ - + / =`) holding at least 6 digits becomes `[key removed]`. That covers tokens, keys and digests. Model and file names have too few digits to match.
- A message is cut at 300 characters.

Service logs are filtered further, as below.

## Service logs (`filter`)

- `database_log(lines)` keeps only `LOG`, `WARNING`, `ERROR`, `FATAL` and `PANIC` lines, as their severity and first sentence. Statement, detail, context, hint and query lines, and the lines that continue them, are dropped.
- `allow_listed(lines, prefixes)` keeps only lines that start with one of `prefixes`. A model server's prefixes are the engine's to give.
- `any_log(lines)` keeps every line.
- All three remove quoted text of any length and cut each line at 200 characters. The supervisor's own lines, which carry no `[stdout] ` or `[stderr] ` tag, are kept.
- `last_exit(lines)` reads the supervisor's exit line as `exit code N` or `signal N`.

## From an app

`AppContext::report(Problem { summary, detail })` hands a handled problem to the window as `WindowRequest::Report`, at most one per app per `REPORT_INTERVAL` (a minute). It returns `true` when the window took it; a refusal doesn't use up the minute. The window adds the app's id and version, writes an `app_error` report with this crate, and shows the person the notice. In tests, `TestHost::reports()` lists what apps reported.

## Tests

`crates/reports/tests/planted.rs` plants a prompt, a statement's value, a home path, a token and a person's ID (both forms) in a message, in a database log, in a model server's log and in another service's log. None comes out, short ones included.
