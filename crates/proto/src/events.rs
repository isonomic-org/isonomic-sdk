// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Data carried by the `events` interface: messages an app publishes to its
//! own scope, so its tabs and windows stay in step.
//!
//! Delivery is live only: at most once, in the publisher's order, to the
//! subscribers present at the time. Nothing is stored; state that must
//! survive belongs in `sql` or `blobs`.

use serde::{Deserialize, Serialize};

use crate::Json;

/// Longest topic name.
pub const MAX_TOPIC: usize = 128;

/// One published message, as a subscriber receives it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AppEvent {
    pub topic: String,
    /// The app's own payload, carried verbatim.
    pub data: Json,
    /// Whatever the publisher passed as `sender`, so a tab can skip its own
    /// messages.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sender: Option<String>,
}

/// Topics are lowercase ASCII letters, digits, dots, dashes and
/// underscores, 1 to [`MAX_TOPIC`] characters, such as `chat.thread-opened`.
pub fn valid_topic(topic: &str) -> bool {
    !topic.is_empty()
        && topic.len() <= MAX_TOPIC
        && topic
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || "._-".contains(c))
}

/// Whether a subscription to `topics` receives `topic`. No topics means
/// every topic in the scope.
pub fn matches(topics: &[String], topic: &str) -> bool {
    topics.is_empty() || topics.iter().any(|t| t == topic)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn topics() {
        for good in ["a", "chat.thread-opened", "settings_changed.v2"] {
            assert!(valid_topic(good), "{good}");
        }
        let long = "a".repeat(MAX_TOPIC + 1);
        for bad in ["", "Chat", "a b", "a/b", long.as_str()] {
            assert!(!valid_topic(bad), "{bad}");
        }
        assert!(matches(&[], "x"));
        assert!(matches(&["x".into()], "x"));
        assert!(!matches(&["y".into()], "x"));
    }
}
