// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! The client against an engine newer than itself: unknown events are
//! skipped, an unknown reply fails only its own call, and the connection
//! survives both.

use std::sync::Arc;
use std::time::Duration;

use isonomic_client::{BoxedStream, Client, ClientError, Connector, Options};
use isonomic_proto::io::{read_message, write_message};
use isonomic_proto::{ClientMessage, PROTOCOL_VERSION};
use tokio::io::AsyncWriteExt;

/// Write one raw JSON frame, as a newer engine would.
async fn raw(stream: &mut tokio::io::DuplexStream, json: &str) {
    stream
        .write_all(&(json.len() as u32).to_be_bytes())
        .await
        .unwrap();
    stream.write_all(json.as_bytes()).await.unwrap();
}

/// An engine that greets, answers the status subscription, then sends an
/// unknown event, and answers each request with a reply kind from the
/// future, except `system.status`, which it answers properly.
fn newer_engine() -> Connector {
    Arc::new(|| {
        Box::pin(async {
            let (client_side, mut engine) = tokio::io::duplex(64 * 1024);
            tokio::spawn(async move {
                let _hello: Option<ClientMessage> = read_message(&mut engine).await.unwrap();
                raw(&mut engine, &format!(r#"{{"type":"welcome","version":{PROTOCOL_VERSION},"engine":"9.9","motd":"new"}}"#)).await;
                while let Ok(Some(message)) = read_message::<_, ClientMessage>(&mut engine).await {
                    let ClientMessage::Request { id, call, .. } = message else {
                        continue;
                    };
                    match call.method() {
                        "system.subscribe" => {
                            raw(&mut engine, &format!(r#"{{"type":"response","id":{id},"result":{{"subscribed":{{"subscription":1,"snapshot":{{"aggregate":"healthy","services":[],"cold_start_ms":null}}}}}}}}"#)).await;
                            raw(&mut engine, r#"{"type":"event","subscription":1,"event":"weather","data":{"sunny":true}}"#).await;
                        }
                        "system.status" => {
                            let reply = isonomic_proto::ServerMessage::Response {
                                id,
                                outcome: isonomic_proto::Outcome::Result(
                                    isonomic_proto::Reply::Status(isonomic_proto::SystemStatus {
                                        aggregate: isonomic_proto::ServiceState::Healthy,
                                        services: vec![],
                                        cold_start_ms: Some(1),
                                    }),
                                ),
                            };
                            write_message(&mut engine, &reply).await.unwrap();
                        }
                        _ => raw(
                            &mut engine,
                            &format!(
                                r#"{{"type":"response","id":{id},"result":{{"forecast":{{}}}}}}"#
                            ),
                        )
                        .await,
                    }
                }
            });
            Ok(Box::new(client_side) as BoxedStream)
        })
    })
}

#[tokio::test(flavor = "multi_thread")]
async fn a_newer_engine_does_not_break_an_older_client() {
    let client = Client::start(
        &tokio::runtime::Handle::current(),
        newer_engine(),
        Options::new("old"),
    );
    let mut live = client.live();
    tokio::time::timeout(
        Duration::from_secs(5),
        live.wait_for(|v| v.system.is_some()),
    )
    .await
    .unwrap()
    .unwrap();

    assert!(matches!(
        client.profile().await,
        Err(ClientError::UnexpectedReply)
    ));
    // Still connected, and still answering.
    assert!(client.live().borrow().connected);
    assert_eq!(client.status().await.unwrap().cold_start_ms, Some(1));
}
