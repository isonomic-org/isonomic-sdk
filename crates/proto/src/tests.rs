// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Frozen examples of the v0 wire format. If one of these strings has to
//! change, the protocol version has to change with it.

use super::*;

fn json<T: Serialize>(value: &T) -> String {
    serde_json::to_string(value).unwrap()
}

fn status() -> ServiceStatus {
    ServiceStatus {
        name: "postgres".into(),
        tier: 1,
        state: ServiceState::Healthy,
        restart_count: 2,
        last_healthy_unix: Some(1_700_000_000),
    }
}

#[test]
fn client_messages_have_a_frozen_shape() {
    let cases = [
        (
            ClientMessage::Hello {
                versions: vec![0],
                client: "isonomic-desktop".into(),
            },
            r#"{"type":"hello","versions":[0],"client":"isonomic-desktop"}"#,
        ),
        (
            ClientMessage::Request {
                id: 1,
                token: None,
                call: Call::SystemStatus,
            },
            r#"{"type":"request","id":1,"method":"system.status"}"#,
        ),
        (
            ClientMessage::Request {
                id: 2,
                token: None,
                call: Call::SystemLogs {
                    service: "postgres".into(),
                    limit: Some(20),
                },
            },
            r#"{"type":"request","id":2,"method":"system.logs","params":{"service":"postgres","limit":20}}"#,
        ),
        (
            ClientMessage::Request {
                id: 3,
                token: None,
                call: Call::SystemUnsubscribe { subscription: 7 },
            },
            r#"{"type":"request","id":3,"method":"system.unsubscribe","params":{"subscription":7}}"#,
        ),
    ];
    for (message, wire) in cases {
        assert_eq!(json(&message), wire);
        assert_eq!(
            serde_json::from_str::<ClientMessage>(wire).unwrap(),
            message
        );
    }
}

#[test]
fn server_messages_have_a_frozen_shape() {
    let snapshot = SystemStatus {
        aggregate: ServiceState::Healthy,
        services: vec![status()],
        cold_start_ms: Some(180),
    };
    let service = r#"{"name":"postgres","tier":1,"state":"healthy","restart_count":2,"last_healthy_unix":1700000000}"#;
    let cases = [
        (
            ServerMessage::Welcome {
                version: 0,
                engine: "0.0.1".into(),
                schema: None,
            },
            r#"{"type":"welcome","version":0,"engine":"0.0.1"}"#.to_string(),
        ),
        (
            ServerMessage::Response { id: 1, outcome: Outcome::Result(Reply::Status(snapshot)) },
            format!(
                r#"{{"type":"response","id":1,"result":{{"status":{{"aggregate":"healthy","services":[{service}],"cold_start_ms":180}}}}}}"#
            ),
        ),
        (
            ServerMessage::Response {
                id: 4,
                outcome: Outcome::Error(WireError::new(
                    ErrorCode::UnknownService,
                    "no service named cache",
                )),
            },
            r#"{"type":"response","id":4,"error":{"code":"unknown_service","message":"no service named cache"}}"#
                .to_string(),
        ),
        (
            ServerMessage::Event { subscription: 7, event: Event::ServiceStatus(status()) },
            format!(r#"{{"type":"event","subscription":7,"event":"service_status","data":{service}}}"#),
        ),
        (
            ServerMessage::Response { id: 5, outcome: Outcome::Result(Reply::ShuttingDown {}) },
            r#"{"type":"response","id":5,"result":{"shutting_down":{}}}"#.to_string(),
        ),
    ];
    for (message, wire) in cases {
        assert_eq!(json(&message), wire);
        assert_eq!(
            serde_json::from_str::<ServerMessage>(&wire).unwrap(),
            message
        );
    }
}

#[test]
fn negotiation_picks_the_highest_common_version() {
    assert_eq!(negotiate(&[0], &[0]), Some(0));
    assert_eq!(negotiate(&[0, 1, 2], &[1, 0]), Some(1));
    assert_eq!(negotiate(&[3], &[0, 1]), None);
    assert_eq!(negotiate(&[], &[0]), None);
}

#[test]
fn frames_carry_their_length_and_refuse_oversize() {
    let frame = frame::encode(&ClientMessage::Request {
        id: 1,
        token: None,
        call: Call::SystemProfile,
    })
    .unwrap();
    let len = frame::body_len([frame[0], frame[1], frame[2], frame[3]]).unwrap();
    assert_eq!(len, frame.len() - 4);
    let back: ClientMessage = frame::decode(&frame[4..]).unwrap();
    assert_eq!(
        back,
        ClientMessage::Request {
            id: 1,
            token: None,
            call: Call::SystemProfile
        }
    );

    let too_big = ((MAX_FRAME_BYTES + 1) as u32).to_be_bytes();
    assert!(matches!(
        frame::body_len(too_big),
        Err(frame::FrameError::TooLarge(_))
    ));
    assert!(matches!(
        frame::decode::<ClientMessage>(b"{nope"),
        Err(frame::FrameError::Json(_))
    ));
}

#[cfg(feature = "tokio")]
#[tokio::test]
async fn messages_round_trip_over_a_stream_and_eof_is_clean() {
    let (mut a, mut b) = tokio::io::duplex(4096);
    let hello = ClientMessage::Hello {
        versions: vec![0],
        client: "t".into(),
    };
    io::write_message(&mut a, &hello).await.unwrap();
    let got: Option<ClientMessage> = io::read_message(&mut b).await.unwrap();
    assert_eq!(got, Some(hello));
    drop(a);
    let end: Option<ClientMessage> = io::read_message(&mut b).await.unwrap();
    assert_eq!(end, None);
}

#[test]
fn applying_an_event_updates_the_service_and_the_light() {
    let mut system = SystemStatus {
        aggregate: ServiceState::Healthy,
        services: vec![status()],
        cold_start_ms: None,
    };
    system.apply(ServiceStatus {
        state: ServiceState::Starting,
        restart_count: 3,
        ..status()
    });
    assert_eq!(system.aggregate, ServiceState::Starting);
    assert_eq!(system.services.len(), 1);
    assert_eq!(system.services[0].restart_count, 3);

    system.apply(ServiceStatus {
        name: "cache".into(),
        state: ServiceState::Degraded,
        ..status()
    });
    assert_eq!(system.services.len(), 2);
    assert_eq!(system.aggregate, ServiceState::Degraded);
    assert_eq!(ServiceState::aggregate([]), ServiceState::Stopped);
}
