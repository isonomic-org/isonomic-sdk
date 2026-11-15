//! The blobs interface against the fake: small and chunked round trips,
//! ownership, limits, half-written blobs, slow and failing calls, and the
//! token check on every method.

use std::time::{Duration, Instant};

use isonomic_client::ClientError;
use isonomic_proto::blobs::CHUNK_BYTES;
use isonomic_proto::manifest::BlobAccess;
use isonomic_proto::{Bytes, Call, Capabilities, ErrorCode, Manifest, Reply, WireError};
use isonomic_testkit::{Interface, TestEngine};

fn app(id: &str, blobs: Option<BlobAccess>) -> Manifest {
    Manifest {
        id: id.into(),
        name: "Blob app".into(),
        version: "0.1.0".into(),
        icon: "document".into(),
        capabilities: Capabilities {
            blobs,
            ..Default::default()
        },
        schemas: vec![],
    }
}

fn allowed() -> Option<BlobAccess> {
    Some(BlobAccess::default())
}

fn code<T: std::fmt::Debug>(result: Result<T, ClientError>) -> ErrorCode {
    match result {
        Err(ClientError::Engine(error)) => error.code,
        other => panic!("expected an engine error, got {other:?}"),
    }
}

fn bytes(len: usize) -> Vec<u8> {
    (0..len).map(|i| (i % 251) as u8).collect()
}

#[tokio::test(flavor = "multi_thread")]
async fn a_small_blob_round_trips_in_one_call() {
    let engine = TestEngine::new();
    let blobs = engine
        .app_client(&app("com.isonomic.chat", allowed()))
        .blobs();
    let info = blobs.put("text/plain", b"hello").await.unwrap();
    assert_eq!((info.size, info.written), (5, 5));
    assert_eq!(info.content_type, "text/plain");
    let (again, data) = blobs.get(&info.handle).await.unwrap();
    assert_eq!(again, info);
    assert_eq!(data, b"hello");
    assert_eq!(engine.blobs().stored(&info.handle).unwrap(), b"hello");
    let methods: Vec<String> = engine.calls().into_iter().map(|c| c.method).collect();
    let blob_calls: Vec<&String> = methods.iter().filter(|m| m.starts_with("blobs.")).collect();
    assert_eq!(blob_calls, ["blobs.put", "blobs.get"]);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_large_blob_is_written_and_read_in_chunks() {
    let engine = TestEngine::new();
    let blobs = engine
        .app_client(&app("com.isonomic.chat", allowed()))
        .blobs();
    let original = bytes(CHUNK_BYTES as usize * 2 + 1000);
    let info = blobs.put("application/pdf", &original).await.unwrap();
    assert!(info.complete());
    assert_eq!(info.size, original.len() as u64);
    let (_, data) = blobs.get(&info.handle).await.unwrap();
    assert!(data == original, "the bytes read back differ");
    let methods: Vec<String> = engine.calls().into_iter().map(|c| c.method).collect();
    let count = |m: &str| methods.iter().filter(|x| *x == m).count();
    assert_eq!(count("blobs.create"), 1);
    assert_eq!(count("blobs.write"), 3);
    assert_eq!(count("blobs.get"), 3);
}

#[tokio::test(flavor = "multi_thread")]
async fn ranges_info_and_delete() {
    let engine = TestEngine::new();
    let client = engine.app_client(&app("com.isonomic.chat", allowed()));
    let info = client.blobs().put("text/plain", b"hello").await.unwrap();
    let call = Call::BlobsGet {
        handle: info.handle.clone(),
        offset: 1,
        length: Some(3),
    };
    match client.call(call).await.unwrap() {
        Reply::BlobData { offset, data, .. } => {
            assert_eq!((offset, data), (1, Bytes(b"ell".to_vec())))
        }
        other => panic!("unexpected {other:?}"),
    }
    assert_eq!(client.blobs().info(&info.handle).await.unwrap(), info);
    client.blobs().delete(&info.handle).await.unwrap();
    assert_eq!(
        code(client.blobs().get(&info.handle).await),
        ErrorCode::UnknownBlob
    );
    assert_eq!(engine.blobs().count(), 0);
}

#[tokio::test(flavor = "multi_thread")]
async fn one_app_cannot_see_another_apps_blob() {
    let engine = TestEngine::new();
    let chat = engine
        .app_client(&app("com.isonomic.chat", allowed()))
        .blobs();
    let docs = engine
        .app_client(&app("com.isonomic.docs", allowed()))
        .blobs();
    let info = chat.put("text/plain", b"private").await.unwrap();
    assert_eq!(code(docs.get(&info.handle).await), ErrorCode::UnknownBlob);
    assert_eq!(code(docs.info(&info.handle).await), ErrorCode::UnknownBlob);
    assert_eq!(
        code(docs.delete(&info.handle).await),
        ErrorCode::UnknownBlob
    );
    // The shell can read it, to show it in the window.
    let (_, data) = engine.client().blobs().get(&info.handle).await.unwrap();
    assert_eq!(data, b"private");
}

#[tokio::test(flavor = "multi_thread")]
async fn limits_from_the_manifest_and_the_engine() {
    let engine = TestEngine::new();
    let capped = Some(BlobAccess {
        max_bytes: Some(10),
    });
    let small = engine.app_client(&app("com.isonomic.chat", capped)).blobs();
    assert!(small.put("text/plain", b"0123456789").await.is_ok());
    assert_eq!(
        code(small.put("text/plain", b"0123456789a").await),
        ErrorCode::TooLarge
    );
    let big = engine
        .app_client(&app("com.isonomic.docs", allowed()))
        .blobs();
    engine.blobs().set_limit(1000);
    assert_eq!(
        code(big.put("text/plain", &bytes(1001)).await),
        ErrorCode::TooLarge
    );
    engine.blobs().set_limit(CHUNK_BYTES * 4);
    let over = bytes(CHUNK_BYTES as usize * 4 + 1);
    assert_eq!(
        code(big.put("text/plain", &over).await),
        ErrorCode::TooLarge
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn a_half_written_blob_cannot_be_read_and_writes_stay_in_order() {
    let engine = TestEngine::new();
    let client = engine.app_client(&app("com.isonomic.chat", allowed()));
    let call = Call::BlobsCreate {
        content_type: "image/png".into(),
        size: 10,
    };
    let Reply::Blob(info) = client.call(call).await.unwrap() else {
        panic!("expected a blob")
    };
    assert_eq!(
        code(client.blobs().get(&info.handle).await),
        ErrorCode::Incomplete
    );
    let write = |offset: u64, data: &[u8]| Call::BlobsWrite {
        handle: info.handle.clone(),
        offset,
        data: Bytes(data.to_vec()),
    };
    let out_of_order = client.call(write(4, b"abcd")).await;
    assert_eq!(code(out_of_order), ErrorCode::BadRequest);
    client.call(write(0, b"abcdef")).await.unwrap();
    assert_eq!(
        code(client.call(write(6, b"ghijk")).await),
        ErrorCode::TooLarge
    );
    client.call(write(6, b"ghij")).await.unwrap();
    let (_, data) = client.blobs().get(&info.handle).await.unwrap();
    assert_eq!(data, b"abcdefghij");
}

#[tokio::test(flavor = "multi_thread")]
async fn slow_and_failing_calls_then_recovery() {
    let engine = TestEngine::new();
    let blobs = engine
        .app_client(&app("com.isonomic.chat", allowed()))
        .blobs();
    engine.slow(Interface::Blobs, Duration::from_millis(150));
    let started = Instant::now();
    blobs.put("text/plain", b"slow").await.unwrap();
    assert!(started.elapsed() >= Duration::from_millis(150));
    engine.fail(
        Interface::Blobs,
        WireError::new(ErrorCode::Internal, "disk full"),
    );
    assert_eq!(
        code(blobs.put("text/plain", b"x").await),
        ErrorCode::Internal
    );
    engine.recover(Interface::Blobs);
    let started = Instant::now();
    blobs.put("text/plain", b"fast").await.unwrap();
    assert!(started.elapsed() < Duration::from_millis(150));
}

#[tokio::test(flavor = "multi_thread")]
async fn every_method_needs_the_blobs_capability() {
    let engine = TestEngine::new();
    let owner = engine
        .app_client(&app("com.isonomic.chat", allowed()))
        .blobs();
    let info = owner.put("text/plain", b"hi").await.unwrap();
    let none = engine.app_client(&app("com.isonomic.docs", None));
    let handle = info.handle.clone();
    let calls = [
        Call::BlobsPut {
            content_type: "text/plain".into(),
            data: Bytes(b"hi".to_vec()),
        },
        Call::BlobsCreate {
            content_type: "text/plain".into(),
            size: 2,
        },
        Call::BlobsWrite {
            handle: handle.clone(),
            offset: 0,
            data: Bytes(b"hi".to_vec()),
        },
        Call::BlobsGet {
            handle: handle.clone(),
            offset: 0,
            length: None,
        },
        Call::BlobsInfo {
            handle: handle.clone(),
        },
        Call::BlobsDelete { handle },
    ];
    for call in calls {
        let method = call.method();
        assert_eq!(
            code(none.call(call).await),
            ErrorCode::CapabilityDenied,
            "{method}"
        );
    }
    assert_eq!(engine.blobs().count(), 1);
}
