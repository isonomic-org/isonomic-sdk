//! The connection itself, on a raw stream: the handshake, and a peer newer
//! or sloppier than the engine (unknown methods and fields, a bad frame)
//! failing one request without ending the connection.

use std::time::Duration;

use isonomic_client::BoxedStream;
use isonomic_proto::io::{read_message, write_message};
use isonomic_proto::{
    Call, ClientMessage, ErrorCode, Outcome as Answer, Reply, ServerMessage, PROTOCOL_VERSION,
};
use tokio::io::AsyncWriteExt;

use crate::{ensure, Outcome, Target};

async fn open(target: &Target) -> Result<BoxedStream, String> {
    (target.connector)()
        .await
        .map_err(|e| format!("connect: {e}"))
}

async fn send(stream: &mut BoxedStream, message: &ClientMessage) -> Outcome {
    write_message(stream, message)
        .await
        .map_err(|e| format!("write: {e}"))
}

/// Write one frame of raw bytes, as a newer or broken client would.
async fn send_raw(stream: &mut BoxedStream, body: &[u8]) -> Outcome {
    let mut frame = (body.len() as u32).to_be_bytes().to_vec();
    frame.extend_from_slice(body);
    stream
        .write_all(&frame)
        .await
        .map_err(|e| format!("write: {e}"))?;
    stream.flush().await.map_err(|e| format!("write: {e}"))
}

/// The next message, skipping events.
async fn next(stream: &mut BoxedStream) -> Result<ServerMessage, String> {
    loop {
        let read = tokio::time::timeout(Duration::from_secs(5), read_message(stream)).await;
        match read {
            Err(_) => return Err("no message within 5 s".into()),
            Ok(Err(error)) => return Err(format!("read: {error}")),
            Ok(Ok(None)) => return Err("the engine closed the connection".into()),
            Ok(Ok(Some(ServerMessage::Event { .. }))) => continue,
            Ok(Ok(Some(message))) => return Ok(message),
        }
    }
}

fn hello(versions: Vec<u32>) -> ClientMessage {
    ClientMessage::Hello {
        versions,
        client: "isonomic-conformance".into(),
    }
}

/// Greet and expect a welcome.
async fn greeted(target: &Target) -> Result<BoxedStream, String> {
    let mut stream = open(target).await?;
    send(&mut stream, &hello(vec![PROTOCOL_VERSION])).await?;
    match next(&mut stream).await? {
        ServerMessage::Welcome { .. } => Ok(stream),
        other => Err(format!("expected welcome, got {other:?}")),
    }
}

/// Expect an error with `code` in answer to request `id`.
async fn expect_error(stream: &mut BoxedStream, id: u64, code: ErrorCode) -> Outcome {
    match next(stream).await? {
        ServerMessage::Response {
            id: got,
            outcome: Answer::Error(error),
        } if got == id && error.code == code => Ok(()),
        other => Err(format!("expected {code:?} for request {id}, got {other:?}")),
    }
}

/// A status request on this connection still works.
async fn still_answers(stream: &mut BoxedStream, id: u64) -> Outcome {
    let request = ClientMessage::Request {
        id,
        token: None,
        call: Call::SystemStatus,
    };
    send(stream, &request).await?;
    match next(stream).await? {
        ServerMessage::Response {
            id: got,
            outcome: Answer::Result(Reply::Status(_)),
        } if got == id => Ok(()),
        other => Err(format!("the connection stopped answering: {other:?}")),
    }
}

/// `hello` with the client's versions gets a `welcome` naming the highest
/// version both speak, and the agent session schema the engine writes.
pub async fn welcome(target: Target) -> Outcome {
    let mut stream = open(&target).await?;
    send(&mut stream, &hello(vec![PROTOCOL_VERSION, 9_999])).await?;
    match next(&mut stream).await? {
        ServerMessage::Welcome {
            version,
            engine,
            schema,
        } => {
            ensure(version == PROTOCOL_VERSION, || {
                format!("engine {engine} chose version {version}, not {PROTOCOL_VERSION}")
            })?;
            ensure(schema.is_some(), || {
                format!("engine {engine} did not name its session event schema")
            })
        }
        other => Err(format!("expected welcome, got {other:?}")),
    }
}

/// No version in common: `unsupported_version`.
pub async fn unsupported_version(target: Target) -> Outcome {
    let mut stream = open(&target).await?;
    send(&mut stream, &hello(vec![9_999])).await?;
    expect_error(&mut stream, 0, ErrorCode::UnsupportedVersion).await
}

/// A request before `hello`: `hello_required`, answered with its id.
pub async fn hello_required(target: Target) -> Outcome {
    let mut stream = open(&target).await?;
    let early = ClientMessage::Request {
        id: 3,
        token: None,
        call: Call::SystemStatus,
    };
    send(&mut stream, &early).await?;
    expect_error(&mut stream, 3, ErrorCode::HelloRequired).await
}

/// A method this engine lacks fails alone, with `unknown_method`.
pub async fn unknown_method(target: Target) -> Outcome {
    let mut stream = greeted(&target).await?;
    let body =
        br#"{"type":"request","id":7,"method":"weather.forecast","params":{"city":"Lisbon"}}"#;
    send_raw(&mut stream, body).await?;
    expect_error(&mut stream, 7, ErrorCode::UnknownMethod).await?;
    still_answers(&mut stream, 8).await
}

/// Fields the engine does not know are ignored, at the top and in params.
pub async fn unknown_fields(target: Target) -> Outcome {
    let mut stream = greeted(&target).await?;
    let body = br#"{"type":"request","id":9,"method":"blobs.info","params":{"handle":"conformance-none","colour":"blue"},"trace":"abc"}"#;
    send_raw(&mut stream, body).await?;
    // The shell asked about a handle that does not exist: decoded and run.
    expect_error(&mut stream, 9, ErrorCode::UnknownBlob).await?;
    still_answers(&mut stream, 10).await
}

/// A frame that is not a message: `bad_request` with id 0, and the
/// connection goes on.
pub async fn bad_frame(target: Target) -> Outcome {
    let mut stream = greeted(&target).await?;
    send_raw(&mut stream, b"this is not json").await?;
    expect_error(&mut stream, 0, ErrorCode::BadRequest).await?;
    still_answers(&mut stream, 11).await
}
