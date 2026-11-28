// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Backtraces as function names and crate-relative places: never a path
//! on disk.

/// Backtraces are cut to this many frames.
pub const FRAMES: usize = 40;

/// The backtrace as function names and crate-relative locations.
pub fn frames(text: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for raw in text.lines() {
        let line = raw.trim();
        if let Some(at) = line.strip_prefix("at ") {
            if let Some(last) = out.last_mut() {
                last.push_str(&format!(" ({})", short_path(at)));
            }
        } else if let Some((number, function)) = line.split_once(": ") {
            if number.chars().all(|c| c.is_ascii_digit()) {
                out.push(function.to_string());
            }
        }
        if out.len() >= FRAMES {
            break;
        }
    }
    out
}

/// A source path cut to its crate: `…/crates/broker/src/conn.rs:12:5`
/// becomes `broker/src/conn.rs:12`. Never a path on disk.
pub fn short_path(path: &str) -> String {
    let trimmed = match path.rsplit_once(':') {
        Some((head, col)) if col.chars().all(|c| c.is_ascii_digit()) && head.contains(':') => head,
        _ => path,
    };
    match trimmed.rfind("/src/") {
        Some(at) => {
            let before = &trimmed[..at];
            let krate = before.rsplit('/').next().unwrap_or(before);
            format!("{krate}{}", &trimmed[at..])
        }
        None => trimmed.rsplit('/').next().unwrap_or(trimmed).to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TRACE: &str = "   0: std::backtrace::Backtrace::force_capture
             at /rustc/abc/library/std/src/backtrace.rs:312:13
   1: broker::conn::serve
             at /Users/omar/Code/isonomic-capsule/crates/broker/src/conn.rs:70:21";

    #[test]
    fn frames_keep_functions_and_crate_relative_places_only() {
        assert_eq!(
            frames(TRACE),
            [
                "std::backtrace::Backtrace::force_capture (std/src/backtrace.rs:312)",
                "broker::conn::serve (broker/src/conn.rs:70)",
            ]
        );
    }

    #[test]
    fn a_path_without_src_keeps_only_its_file() {
        assert_eq!(short_path("/Users/omar/x/build.rs:3:1"), "build.rs:3");
    }
}
