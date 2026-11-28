// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Planted content never reaches a report: a prompt, a statement's value,
//! a home path, a token and a person's ID, planted in a message and in log
//! lines, come out removed whichever filter the lines went through.
//!
//! Quoted text goes at any length, so a short typed value goes too; the
//! person's ID goes in either form, at any length.

use isonomic_reports::filter::{allow_listed, any_log, database_log};
use isonomic_reports::{Build, Component, Kind, Report, System};
use serde_json::Value;

const HOME: &str = "/Users/omar";
const PROMPT: &str = "Plan one day in Lisbon for my sister's birthday next week";
const VALUE: &str = "Lisbon with my sister";
const TOKEN: &str = "iso_tok_7f3a9c21b04e5d6f8a1b2c3d4e5f6a7b";
const ID: &str = "did:iso:3f9c0a7e1b2d4c5f8e6a7b9c0d1e2f3a3f9c0a7e1b2d4c5f8e6a7b9c0d1e2f3a";
const ID_KEY: &str = "3f9c0a7e1b2d4c5f8e6a7b9c0d1e2f3a3f9c0a7e1b2d4c5f8e6a7b9c0d1e2f3a";
const SHORT_ID: &str = "isonomic:7Q4K-M2XD-9PLV";

fn write(report: &Report) -> Value {
    report.to_json(
        &Build::default(),
        &System::default(),
        Some(HOME),
        1_700_000_000,
    )
}

fn assert_clean(doc: &Value) {
    let text = doc.to_string();
    for planted in ["Lisbon", "sister", TOKEN, ID_KEY, "did:iso", "7Q4K", HOME] {
        assert!(!text.contains(planted), "{planted:?} in {text}");
    }
}

/// A message carrying all five, and the ID's short form, as a panic or an
/// app's error might.
fn message() -> String {
    format!(
        "failed on \"{PROMPT}\" with '{VALUE}' at {HOME}/Library/x for {ID} ({SHORT_ID}) using {TOKEN}"
    )
}

fn lines(text: &str) -> Vec<String> {
    text.lines().map(String::from).collect()
}

#[test]
fn a_planted_message_is_scrubbed() {
    let doc = write(&Report::new(Kind::Panic, Component::Window, message()));
    assert_clean(&doc);
    let written = doc["message"].as_str().unwrap();
    assert!(
        written.contains("[text removed]") && written.contains("[key removed]"),
        "{written}"
    );
    assert!(written.contains("~/Library/x"), "{written}");
    assert_eq!(written.matches("[id removed]").count(), 2, "{written}");
}

#[test]
fn a_database_log_keeps_no_value_prompt_token_or_id() {
    let log = lines(&format!(
        "[stderr] 2026-11-16 10:00:00.123 UTC [812] LOG:  database system is ready to accept connections
[stderr] 2026-11-16 10:00:01.004 UTC [907] ERROR:  invalid input syntax for type uuid: \"{VALUE}\"
[stderr] 2026-11-16 10:00:01.004 UTC [907] STATEMENT:  insert into trips (title, note, owner)
[stderr] \tvalues ('{VALUE}', '{PROMPT}', '{ID}')
[stderr] 2026-11-16 10:00:01.005 UTC [907] FATAL:  could not open file {HOME}/Capsule/base/1: token {TOKEN}"
    ));
    let mut report = Report::new(Kind::ServiceFailed, service(), "the database stopped");
    report.log_tail = database_log(&log);
    let doc = write(&report);
    assert_clean(&doc);
    let tail = doc["log_tail"].to_string();
    assert!(tail.contains("LOG: database system is ready"), "{tail}");
    assert!(
        tail.contains("FATAL: could not open file ~/Capsule/"),
        "{tail}"
    );
    assert!(
        !tail.contains("STATEMENT") && !tail.contains("values"),
        "{tail}"
    );
}

#[test]
fn a_model_servers_log_keeps_no_prompt_token_or_id() {
    let log = lines(&format!(
        "[stderr] main: server is listening on http://127.0.0.1:41000
[stderr] slot update_slots: id  0 | task 3 | prompt: {PROMPT}
[stderr] srv  log_server_r: request: POST /v1/chat/completions {TOKEN}
[stderr] error: failed to load model at {HOME}/Capsule/models/x.gguf for {ID}"
    ));
    let mut report = Report::new(
        Kind::ModelServerFailed,
        worker(),
        "the model server stopped",
    );
    report.log_tail = allow_listed(&log, &["main:", "error:"]);
    let doc = write(&report);
    assert_clean(&doc);
    let tail = doc["log_tail"].to_string();
    assert!(
        tail.contains("error: failed to load model at ~/Capsule/models/x.gguf"),
        "{tail}"
    );
    assert!(
        !tail.contains("slot ") && !tail.contains("request:"),
        "{tail}"
    );
}

#[test]
fn any_other_log_keeps_its_lines_but_none_of_the_planted() {
    let log = lines(&format!(
        "[stdout] saved \"{PROMPT}\" for {ID}
[stderr] opened {HOME}/Capsule/cache with {TOKEN}
[stdout] note '{VALUE}'"
    ));
    let mut report = Report::new(Kind::ServiceFailed, service(), message());
    report.log_tail = any_log(&log);
    report.backtrace = vec![format!("app::run ({HOME}/src/main.rs:3) {TOKEN}")];
    assert_clean(&write(&report));
}

fn service() -> Component {
    Component::Service {
        name: "database".into(),
        version: "18.4".into(),
        restarts: 6,
        last_exit: Some(format!("exit code 1 at {HOME}/x")),
    }
}

fn worker() -> Component {
    Component::ModelServer {
        model: "chat-small".into(),
        runtime: "model-server".into(),
        version: "b12387".into(),
        restarts: 5,
        last_exit: None,
    }
}

/// A short typed value goes too: quoted text of any length is removed.
#[test]
fn a_short_quoted_value_in_a_message_is_removed() {
    let message = format!("bad value '{VALUE}'");
    let doc = write(&Report::new(Kind::AppError, Component::Window, message));
    assert_eq!(doc["message"], "bad value '[text removed]'");
}

/// The readable ID is removed in a message and in a log line, short as it is.
#[test]
fn the_short_id_is_removed_wherever_it_is() {
    let message = format!("signed in as {SHORT_ID}");
    let mut report = Report::new(Kind::AppError, Component::Window, message);
    report.log_tail = vec![format!("identity ready: {SHORT_ID}")];
    let doc = write(&report);
    assert_clean(&doc);
    assert_eq!(doc["message"], "signed in as [id removed]");
    assert_eq!(doc["log_tail"][0], "identity ready: [id removed]");
}
