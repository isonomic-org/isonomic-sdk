// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: Apache-2.0

//! The client's reassembly of a chunked artifact, against a minimal engine
//! of its own: an honest one, and one that ignores `offset`.

use std::sync::Arc;
use std::time::Duration;

use isonomic_client::{BoxedStream, Client, ClientError, Connector, Options};
use isonomic_proto::agents::{ArtifactEvent, ArtifactVersion};
use isonomic_proto::blobs::CHUNK_BYTES;
use isonomic_proto::io::{read_frame, write_message};
use isonomic_proto::lenient::{decode_client, FromClient};
use isonomic_proto::{Bytes, Call, ClientMessage, Outcome, Reply, ServerMessage, PROTOCOL_VERSION};

const SIZE: usize = 2 * CHUNK_BYTES as usize + 100;

fn body() -> Vec<u8> {
    (0..SIZE).map(|i| (i % 251) as u8).collect()
}

fn meta() -> ArtifactEvent {
    serde_json::from_value(serde_json::json!({
        "artifact": "r-1", "version": 1, "op": "create", "title": "Notes",
        "media_type": "text/plain", "blob": "b-1", "bytes": SIZE, "call": "c-1"
    }))
    .unwrap()
}

/// An engine that serves one artifact: from the offset asked for, or, when
/// `honest` is false, always from 0.
fn engine(honest: bool) -> Connector {
    Arc::new(move || {
        Box::pin(async move {
            let (client_side, engine_side) = tokio::io::duplex(4 << 20);
            tokio::spawn(async move {
                let (mut reader, mut writer) = tokio::io::split(engine_side);
                while let Ok(Some(frame)) = read_frame(&mut reader).await {
                    let answer = match decode_client(&frame) {
                        FromClient::Message(ClientMessage::Hello { .. }) => {
                            ServerMessage::Welcome {
                                version: PROTOCOL_VERSION,
                                engine: "fake".into(),
                                schema: None,
                            }
                        }
                        FromClient::Message(ClientMessage::Request {
                            id,
                            call: Call::ArtifactsGet(get),
                            ..
                        }) => {
                            let offset = if honest { get.offset } else { 0 };
                            let start = offset as usize;
                            let end = (start + CHUNK_BYTES as usize).min(SIZE);
                            let chunk = ArtifactVersion {
                                meta: meta(),
                                bytes: Bytes(body()[start..end].to_vec()),
                                offset,
                            };
                            ServerMessage::Response {
                                id,
                                outcome: Outcome::Result(Reply::Artifact(Box::new(chunk))),
                            }
                        }
                        _ => continue,
                    };
                    if write_message(&mut writer, &answer).await.is_err() {
                        return;
                    }
                }
            });
            Ok(Box::new(client_side) as BoxedStream)
        })
    })
}

async fn read(honest: bool) -> Result<ArtifactVersion, ClientError> {
    let client = Client::start(
        &tokio::runtime::Handle::current(),
        engine(honest),
        Options::new("chunks"),
    );
    let mut live = client.live();
    tokio::time::timeout(Duration::from_secs(5), live.wait_for(|l| l.connected))
        .await
        .expect("connected within 5 s")
        .unwrap();
    drop(live);
    client.agents().artifact(&"r-1".into(), None).await
}

#[tokio::test]
async fn an_artifact_comes_back_whole_from_its_chunks() {
    let whole = read(true).await.unwrap();
    assert_eq!(whole.bytes.0, body());
    assert_eq!(whole.offset, 0);
}

#[tokio::test]
async fn an_engine_that_ignores_offset_is_refused() {
    let refused = read(false).await;
    assert!(
        matches!(refused, Err(ClientError::UnexpectedReply)),
        "{refused:?}"
    );
}
