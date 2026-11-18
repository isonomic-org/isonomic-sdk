// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: Apache-2.0

//! Decoding that survives a peer newer than this build (design rule 6:
//! unknown fields and events are ignored, not fatal).
//!
//! Unknown fields are already ignored by every type. What remains is an
//! unknown method, reply or event: decoding the frame as a typed message
//! fails, but the frame is intact, so the connection goes on.

use serde_json::Value;

use crate::frame::DecodeFailure;
use crate::{Call, ClientMessage, ErrorCode, ServerMessage, WireError};

/// A frame the engine read from a client.
#[derive(Debug)]
pub enum FromClient {
    Message(ClientMessage),
    /// A request the engine cannot run. Answer it with this id and error,
    /// so the caller is not left waiting.
    Rejected {
        id: u64,
        error: WireError,
    },
    /// Not a request at all. Answer with id 0.
    Garbage(WireError),
}

pub fn decode_client(body: &[u8]) -> FromClient {
    let failure = match serde_json::from_slice::<ClientMessage>(body) {
        Ok(message) => return FromClient::Message(message),
        Err(failure) => DecodeFailure::of(&failure),
    };
    // No message below quotes the frame: it may hold the person's recovery
    // words, in any field, in the method name, or as the whole frame. Only
    // serde's category and place are said, and a method name only when it
    // has a method's form.
    let bad = |text: String| WireError::new(ErrorCode::BadRequest, text);
    let not_a_message = || bad(format!("the frame is not a client message ({failure})"));
    let Ok(Value::Object(map)) = serde_json::from_slice::<Value>(body) else {
        return FromClient::Garbage(not_a_message());
    };
    let id = map.get("id").and_then(Value::as_u64);
    let method = map.get("method").and_then(Value::as_str);
    match (map.get("type").and_then(Value::as_str), id, method) {
        (Some("request"), Some(id), Some(method)) if !Call::METHODS.contains(&method) => {
            let text = match method_form(method) {
                true => format!("this engine has no method {method}"),
                false => "this engine has no such method".to_string(),
            };
            FromClient::Rejected {
                id,
                error: WireError::new(ErrorCode::UnknownMethod, text),
            }
        }
        (Some("request"), Some(id), _) => FromClient::Rejected {
            id,
            error: bad(format!(
                "the request does not have the shape its method takes ({failure})"
            )),
        },
        _ => FromClient::Garbage(not_a_message()),
    }
}

/// Whether `method` looks like a method name, and so may be repeated: 1 to
/// 64 of `a`-`z`, `0`-`9`, `_` and `.`.
fn method_form(method: &str) -> bool {
    (1..=64).contains(&method.len())
        && method
            .bytes()
            .all(|b| matches!(b, b'a'..=b'z' | b'0'..=b'9' | b'_' | b'.'))
}

/// A frame the client read from the engine.
#[derive(Debug)]
pub enum FromServer {
    Message(ServerMessage),
    /// A response this build cannot read, probably a reply kind from a
    /// newer engine. The pending call with this id should fail.
    UnreadableResponse {
        id: u64,
    },
    /// Anything else unreadable, such as an event kind from a newer engine.
    /// Skip it.
    Skip,
}

pub fn decode_server(body: &[u8]) -> FromServer {
    if let Ok(message) = serde_json::from_slice::<ServerMessage>(body) {
        return FromServer::Message(message);
    }
    let Ok(Value::Object(map)) = serde_json::from_slice::<Value>(body) else {
        return FromServer::Skip;
    };
    match (
        map.get("type").and_then(Value::as_str),
        map.get("id").and_then(Value::as_u64),
    ) {
        (Some("response"), Some(id)) => FromServer::UnreadableResponse { id },
        _ => FromServer::Skip,
    }
}
