// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! Each interface doing its job for an app that holds the capability, and
//! keeping apps apart: blobs, events, subscriptions and sql schemas.

use std::time::Duration;

use isonomic_client::EventStream;
use isonomic_proto::blobs::CHUNK_BYTES;
use isonomic_proto::manifest::{BlobAccess, SqlAccess};
use isonomic_proto::{AppEvent, Call, Capabilities, ErrorCode, Json, Reply};
use serde_json::json;

use crate::{ensure, manifest, ok, refused, Outcome, Target};

fn blobs(max_bytes: Option<u64>) -> Capabilities {
    Capabilities {
        blobs: Some(BlobAccess { max_bytes }),
        ..Default::default()
    }
}

/// A store, read back, describe, delete cycle on the app's own blob.
pub async fn blobs_round_trip(target: Target) -> Outcome {
    let app = target.app(manifest("blobs", blobs(None), &[])).await?;
    let bytes = b"conformance".to_vec();
    let stored = ok(app.blobs().put("text/plain", &bytes).await, "put")?;
    ensure(
        stored.size == bytes.len() as u64 && stored.complete(),
        || format!("put reported {stored:?}"),
    )?;
    let (info, back) = ok(app.blobs().get(&stored.handle).await, "get")?;
    ensure(back == bytes, || "get returned other bytes".into())?;
    ensure(info.content_type == "text/plain", || {
        format!("content type came back as {}", info.content_type)
    })?;
    ok(app.blobs().info(&stored.handle).await, "info")?;
    ok(app.blobs().delete(&stored.handle).await, "delete")?;
    let gone = app.blobs().get(&stored.handle).await;
    refused(gone, ErrorCode::UnknownBlob, "get after delete")
}

/// A blob larger than one chunk goes up in pieces and comes back whole.
pub async fn blobs_chunked(target: Target) -> Outcome {
    let app = target
        .app(manifest("blobs-chunked", blobs(None), &[]))
        .await?;
    let size = CHUNK_BYTES as usize + CHUNK_BYTES as usize / 2;
    let bytes: Vec<u8> = (0..size).map(|i| (i % 251) as u8).collect();
    let stored = ok(
        app.blobs().put("application/octet-stream", &bytes).await,
        "put",
    )?;
    let (_, back) = ok(app.blobs().get(&stored.handle).await, "get")?;
    let result = ensure(back == bytes, || {
        format!(
            "{} bytes in, {} different bytes out",
            bytes.len(),
            back.len()
        )
    });
    ok(app.blobs().delete(&stored.handle).await, "delete")?;
    result
}

/// One app's handle means nothing to another.
pub async fn blobs_private(target: Target) -> Outcome {
    let owner = target
        .app(manifest("blobs-owner", blobs(None), &[]))
        .await?;
    let other = target
        .app(manifest("blobs-other", blobs(None), &[]))
        .await?;
    let stored = ok(owner.blobs().put("text/plain", b"mine").await, "put")?;
    let read = other.blobs().get(&stored.handle).await;
    let deleted = other.blobs().delete(&stored.handle).await;
    ok(
        owner.blobs().delete(&stored.handle).await,
        "the owner deletes",
    )?;
    refused(read, ErrorCode::UnknownBlob, "another app reads it")?;
    refused(deleted, ErrorCode::UnknownBlob, "another app deletes it")
}

/// Over the manifest's `max_bytes`: `too_large`, whether in one call or
/// announced up front.
pub async fn blobs_max_bytes(target: Target) -> Outcome {
    let app = target
        .app(manifest("blobs-small", blobs(Some(4)), &[]))
        .await?;
    let one_call = app.blobs().put("text/plain", b"12345").await;
    refused(one_call, ErrorCode::TooLarge, "a 5-byte put")?;
    let announced = Call::BlobsCreate {
        content_type: "text/plain".into(),
        size: 5,
    };
    refused(
        app.call(announced).await,
        ErrorCode::TooLarge,
        "a 5-byte create",
    )?;
    let fits = ok(app.blobs().put("text/plain", b"1234").await, "a 4-byte put")?;
    ok(app.blobs().delete(&fits.handle).await, "delete")?;
    Ok(())
}

async fn next(stream: &mut EventStream) -> Result<AppEvent, String> {
    match tokio::time::timeout(Duration::from_secs(5), stream.next()).await {
        Ok(Some(event)) => Ok(event),
        Ok(None) => Err("the event stream closed".into()),
        Err(_) => Err("no event within 5 s".into()),
    }
}

fn data(value: serde_json::Value) -> Json {
    Json::from(value)
}

fn none() -> Capabilities {
    Capabilities::default()
}

/// What an app publishes reaches its own subscribers, with its sender, and
/// a topic filter holds.
pub async fn events_round_trip(target: Target) -> Outcome {
    let app = target.app(manifest("events", none(), &[])).await?;
    let mut filtered = ok(
        app.events().subscribe(&["conformance.b"]).await,
        "subscribe",
    )?;
    let mut all = ok(app.events().subscribe(&[]).await, "subscribe to all")?;
    let first = data(json!({"n": 1}));
    let second = data(json!({"n": 2}));
    let publish = |topic: &'static str, data: Json| {
        let events = app.events();
        async move { events.publish(topic, data, Some("tab-1")).await }
    };
    ok(publish("conformance.a", first.clone()).await, "publish a")?;
    ok(publish("conformance.b", second.clone()).await, "publish b")?;
    let a = next(&mut all).await?;
    ensure(a.topic == "conformance.a" && a.data == first, || {
        format!("expected conformance.a first, got {a:?}")
    })?;
    ensure(a.sender.as_deref() == Some("tab-1"), || {
        format!("the sender came back as {:?}", a.sender)
    })?;
    let b = next(&mut filtered).await?;
    ensure(b.topic == "conformance.b" && b.data == second, || {
        format!("the filter let through {b:?}")
    })
}

/// Another app's events on the same topic never arrive.
pub async fn events_scoped(target: Target) -> Outcome {
    let mine = target.app(manifest("events-mine", none(), &[])).await?;
    let theirs = target.app(manifest("events-theirs", none(), &[])).await?;
    let mut stream = ok(mine.events().subscribe(&[]).await, "subscribe")?;
    let topic = "conformance.shared";
    let foreign = theirs.events();
    let sent = foreign.publish(topic, data(json!("theirs")), None).await;
    ok(sent, "the other app publishes")?;
    let own = mine.events();
    ok(
        own.publish(topic, data(json!("mine")), None).await,
        "the app publishes",
    )?;
    let event = next(&mut stream).await?;
    ensure(event.data == data(json!("mine")), || {
        format!("another app's event arrived: {event:?}")
    })
}

/// The shell names an app's scope and reaches its subscribers.
pub async fn events_shell_reaches_app(target: Target) -> Outcome {
    let spec = manifest("events-shell", none(), &[]);
    let id = spec.id.clone();
    let app = target.app(spec).await?;
    let mut stream = ok(app.events().subscribe(&[]).await, "subscribe")?;
    let call = Call::EventsPublish {
        topic: "conformance.from-shell".into(),
        data: data(json!({"theme": "dark"})),
        sender: Some("shell".into()),
        app: Some(id),
    };
    ok(target.shell.call(call).await, "the shell publishes")?;
    let event = next(&mut stream).await?;
    ensure(event.topic == "conformance.from-shell", || {
        format!("got {event:?}")
    })
}

/// Only the token that opened a subscription can end it, even on the same
/// connection.
pub async fn owner_only(target: Target) -> Outcome {
    let owner = target.app(manifest("subs-owner", none(), &[])).await?;
    let other = target.app(manifest("subs-other", none(), &[])).await?;
    let mut stream = ok(owner.events().subscribe(&[]).await, "subscribe")?;
    let subscription = stream.subscription();
    let theft = other.call(Call::EventsUnsubscribe { subscription }).await;
    refused(
        theft,
        ErrorCode::UnknownSubscription,
        "another app unsubscribes",
    )?;
    let events = owner.events();
    let still = events
        .publish("conformance.still", Json::default(), None)
        .await;
    ok(still, "publish")?;
    let event = next(&mut stream).await?;
    ensure(event.topic == "conformance.still", || {
        format!("got {event:?}")
    })?;
    match owner.call(Call::EventsUnsubscribe { subscription }).await {
        Ok(Reply::Unsubscribed {}) => Ok(()),
        other => Err(format!("the owner could not unsubscribe: {other:?}")),
    }
}

/// Own schemas for anything, listed schemas for reads only, nothing else;
/// the shell names its schema.
pub async fn sql_schemas(target: Target) -> Outcome {
    let reads = SqlAccess {
        schemas: vec!["conformance_listed".into()],
    };
    let caps = Capabilities {
        sql: Some(reads),
        ..Default::default()
    };
    let app = target
        .app(manifest("sql", caps, &["conformance_sql"]))
        .await?;
    let sql = app.sql();
    ok(
        sql.query("select 1", vec![]).await,
        "a read in its own schema",
    )?;
    let listed = sql.in_schema("conformance_listed");
    let write = listed.execute("delete from t", vec![]).await;
    refused(
        write,
        ErrorCode::CapabilityDenied,
        "a write to a listed schema",
    )?;
    let foreign = sql.in_schema("conformance_foreign");
    let read = foreign.query("select 1", vec![]).await;
    refused(read, ErrorCode::CapabilityDenied, "a read elsewhere")?;
    let live = foreign.live("select 1", vec![]).await;
    refused(live, ErrorCode::CapabilityDenied, "a live query elsewhere")?;

    let homeless = Capabilities {
        sql: Some(SqlAccess::default()),
        ..Default::default()
    };
    let app = target.app(manifest("sql-homeless", homeless, &[])).await?;
    let result = app.sql().query("select 1", vec![]).await;
    refused(result, ErrorCode::CapabilityDenied, "no schema of its own")?;

    let result = target.shell.sql().query("select 1", vec![]).await;
    refused(result, ErrorCode::BadRequest, "the shell names no schema")
}
