//! Decoding that survives a peer newer than this build (design rule 6:
//! unknown fields and events are ignored, not fatal).
//!
//! Unknown fields are already ignored by every type. What remains is an
//! unknown method, reply or event: decoding the frame as a typed message
//! fails, but the frame is intact, so the connection goes on.

use serde_json::Value;

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
        Err(failure) => failure,
    };
    let bad = |text: String| WireError::new(ErrorCode::BadRequest, text);
    let Ok(Value::Object(map)) = serde_json::from_slice::<Value>(body) else {
        return FromClient::Garbage(bad(failure.to_string()));
    };
    let id = map.get("id").and_then(Value::as_u64);
    let method = map.get("method").and_then(Value::as_str);
    match (map.get("type").and_then(Value::as_str), id, method) {
        (Some("request"), Some(id), Some(method)) if !Call::METHODS.contains(&method) => {
            FromClient::Rejected {
                id,
                error: WireError::new(
                    ErrorCode::UnknownMethod,
                    format!("this engine has no method {method}"),
                ),
            }
        }
        (Some("request"), Some(id), method) => FromClient::Rejected {
            id,
            error: bad(shaped(method, failure)),
        },
        (_, _, method) => FromClient::Garbage(bad(shaped(method, failure))),
    }
}

/// The message for a request that did not decode: serde's own, which can
/// quote what was sent, except for the recovery kit's methods, whose
/// params may hold the person's words. Those get fixed text.
fn shaped(method: Option<&str>, failure: serde_json::Error) -> String {
    match method {
        Some(method) if method.starts_with("identity.kit_") => format!(
            "{method}: the request does not have the shape this method takes \
             (what was sent is not repeated, as it may hold recovery words)"
        ),
        _ => failure.to_string(),
    }
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
