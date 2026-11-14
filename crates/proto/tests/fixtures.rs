//! The frozen wire shapes. Every line under `fixtures/v<PROTOCOL_VERSION>/`
//! is one message exactly as it travels; each must decode and re-encode to
//! the same bytes. A shape change makes this fail, and CI refuses edits to
//! an existing fixture line, so the only way forward is a new protocol
//! version with its own fixture directory. New lines may be added freely.

use std::path::PathBuf;

use isonomic_proto::{Call, ClientMessage, ServerMessage, PROTOCOL_VERSION};
use serde_json::Value;

fn fixture_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures")
        .join(format!("v{PROTOCOL_VERSION}"))
}

/// (file, line number, text) for every fixture line.
fn fixtures() -> Vec<(String, usize, String)> {
    let mut files: Vec<PathBuf> = std::fs::read_dir(fixture_dir())
        .expect("a fixture directory for the current protocol version")
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "jsonl"))
        .collect();
    files.sort();
    let mut lines = Vec::new();
    for path in files {
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        let text = std::fs::read_to_string(&path).unwrap();
        for (number, line) in text.lines().enumerate() {
            if !line.trim().is_empty() {
                lines.push((name.clone(), number + 1, line.to_string()));
            }
        }
    }
    assert!(!lines.is_empty(), "no fixtures found");
    lines
}

fn from_client(kind: &str) -> bool {
    matches!(kind, "hello" | "request")
}

#[test]
fn every_fixture_round_trips_byte_for_byte() {
    for (file, number, line) in fixtures() {
        let value: Value = serde_json::from_str(&line)
            .unwrap_or_else(|e| panic!("{file}:{number} is not JSON: {e}"));
        let kind = value["type"].as_str().unwrap_or_default().to_string();
        let again = if from_client(&kind) {
            let message: ClientMessage = serde_json::from_str(&line)
                .unwrap_or_else(|e| panic!("{file}:{number} no longer decodes: {e}"));
            serde_json::to_string(&message).unwrap()
        } else {
            let message: ServerMessage = serde_json::from_str(&line)
                .unwrap_or_else(|e| panic!("{file}:{number} no longer decodes: {e}"));
            serde_json::to_string(&message).unwrap()
        };
        assert_eq!(again, line, "{file}:{number} changed shape");
    }
}

#[test]
fn every_method_has_a_fixture_and_the_right_name() {
    let mut seen = Vec::new();
    for (file, number, line) in fixtures() {
        if let Ok(ClientMessage::Request { call, .. }) = serde_json::from_str(&line) {
            let value: Value = serde_json::from_str(&line).unwrap();
            assert_eq!(
                Some(call.method()),
                value["method"].as_str(),
                "{file}:{number}"
            );
            assert!(Call::METHODS.contains(&call.method()), "{file}:{number}");
            seen.push(call.method());
        }
    }
    for method in Call::METHODS {
        assert!(seen.contains(method), "no fixture calls {method}");
    }
}

#[test]
fn interface_files_hold_only_their_own_methods() {
    for (file, number, line) in fixtures() {
        if let Ok(ClientMessage::Request { call, .. }) = serde_json::from_str(&line) {
            let interface = call.interface().name();
            assert_eq!(file, format!("{interface}.jsonl"), "line {number}");
        }
    }
}
