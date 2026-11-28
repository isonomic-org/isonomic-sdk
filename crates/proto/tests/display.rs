// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: Apache-2.0

//! Display widgets on the wire: the schema revision they arrived in, and
//! tolerance of kinds and widgets this build does not know.

use isonomic_proto::agents::display::{display_spec, validate};
use isonomic_proto::agents::schema::{Event, ToolKind};
use isonomic_proto::agents::{SchemaVersion, SessionEvent};

#[test]
fn the_engine_reports_schema_revision_one() {
    assert_eq!(SchemaVersion::CURRENT.revision, 1);
}

#[test]
fn an_unknown_kind_or_widget_decodes() {
    let line = r#"{"session":"s-1","seq":3,"at":1,"turn":"t-1","step":"p-1","type":"tool.call","call":"c-1","tool":"show_map","kind":"hologram","input":{}}"#;
    let event: SessionEvent = serde_json::from_str(line).unwrap();
    let Event::ToolCall(call) = &event.event else {
        panic!("not a tool call");
    };
    assert_eq!(call.kind, ToolKind::Other("hologram".into()));
    assert_eq!(serde_json::to_string(&event).unwrap(), line);
    let line = line.replace("hologram", "display");
    let event: SessionEvent = serde_json::from_str(&line).unwrap();
    let Event::ToolCall(call) = &event.event else {
        panic!("not a tool call");
    };
    assert_eq!(call.kind, ToolKind::Display);
    assert!(display_spec(&call.tool).is_none());
    assert!(validate(&call.tool, &call.input).is_err());
}
