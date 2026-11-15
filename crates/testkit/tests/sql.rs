//! The sql interface against the scripted database: typed results, writes,
//! batches, live queries, schema rules, failures, and the capability check.

use std::time::Duration;

use isonomic_client::{ClientError, LiveQuery};
use isonomic_proto::manifest::{Schema, SqlAccess};
use isonomic_proto::sql::{Column, Statement};
use isonomic_proto::{Call, Capabilities, ErrorCode, Manifest, Rows, SqlValue};
use isonomic_testkit::TestEngine;

fn app(id: &str, own: &[&str], sql: Option<SqlAccess>) -> Manifest {
    Manifest {
        id: id.into(),
        name: "Sql app".into(),
        version: "0.1.0".into(),
        icon: "table".into(),
        capabilities: Capabilities {
            sql,
            ..Default::default()
        },
        schemas: own
            .iter()
            .map(|s| Schema {
                name: s.to_string(),
            })
            .collect(),
    }
}

fn chat() -> Manifest {
    let reads = SqlAccess {
        schemas: vec!["contacts".into()],
    };
    app("com.isonomic.chat", &["chat", "chat_cache"], Some(reads))
}

fn count(n: i64) -> Rows {
    Rows {
        columns: vec![Column {
            name: "n".into(),
            kind: "integer".into(),
        }],
        rows: vec![vec![SqlValue::Int(n)]],
    }
}

fn code<T: std::fmt::Debug>(result: Result<T, ClientError>) -> ErrorCode {
    match result {
        Err(ClientError::Engine(error)) => error.code,
        other => panic!("expected an engine error, got {other:?}"),
    }
}

async fn changed(live: &mut LiveQuery) -> Rows {
    let next = tokio::time::timeout(Duration::from_secs(5), live.changed()).await;
    next.expect("a change within 5 s").expect("open").clone()
}

#[tokio::test(flavor = "multi_thread")]
async fn queries_return_typed_rows_and_writes_are_recorded() {
    let engine = TestEngine::new();
    let sql = engine.app_client(&chat()).sql();
    let threads = "select id, title from threads where archived = $1";
    let rows = Rows {
        columns: vec![
            Column {
                name: "id".into(),
                kind: "integer".into(),
            },
            Column {
                name: "title".into(),
                kind: "text".into(),
            },
        ],
        rows: vec![
            vec![42.into(), "Lisbon".into()],
            vec![41.into(), SqlValue::Null],
        ],
    };
    engine.sql().respond("chat", threads, rows.clone());
    let got = sql.query(threads, vec![false.into()]).await.unwrap();
    assert_eq!(got, rows);
    assert_eq!(got.get(0, "title"), Some(&SqlValue::from("Lisbon")));
    // Unscripted queries return nothing rather than failing.
    assert_eq!(
        sql.query("select 1", vec![]).await.unwrap(),
        Rows::default()
    );

    let update = "update threads set title = $1 where id = $2";
    engine.sql().affect("chat", update, 1);
    let changed = sql.execute(update, vec!["Porto".into(), 42.into()]).await;
    assert_eq!(changed.unwrap(), 1);
    let last = engine.sql().statements().pop().unwrap();
    assert_eq!(last.app.as_deref(), Some("com.isonomic.chat"));
    assert_eq!(last.schema, "chat");
    assert_eq!(last.sql, update);
    assert_eq!(last.params, vec!["Porto".into(), SqlValue::Int(42)]);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_batch_is_all_or_nothing() {
    let engine = TestEngine::new();
    let sql = engine.app_client(&chat()).sql();
    let insert = Statement {
        sql: "insert into messages (body) values ($1)".into(),
        params: vec!["hi".into()],
    };
    let bump = Statement {
        sql: "update threads set updated = now()".into(),
        params: vec![],
    };
    engine.sql().affect("chat", &insert.sql, 1);
    engine.sql().affect("chat", &bump.sql, 3);
    let counts = sql.batch(vec![insert.clone(), bump.clone()]).await.unwrap();
    assert_eq!(counts, [1, 3]);
    assert_eq!(engine.sql().statements().len(), 2);

    engine.sql().fail(&bump.sql, "deadlock detected");
    let failed = sql.batch(vec![insert, bump]).await;
    assert_eq!(code(failed), ErrorCode::SqlError);
    assert_eq!(
        engine.sql().statements().len(),
        2,
        "nothing from the failed batch"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn a_live_query_follows_its_result_until_dropped() {
    let engine = TestEngine::new();
    let sql = engine.app_client(&chat()).sql();
    let query = "select count(*) as n from messages";
    engine.sql().respond("chat", query, count(7));
    let mut live = sql.live(query, vec![]).await.unwrap();
    assert_eq!(live.rows(), &count(7));
    // Spacing does not matter; another schema's change does not reach it.
    engine.sql().respond("chat_cache", query, count(99));
    engine
        .sql()
        .respond("chat", "select  count(*) as n\nfrom messages", count(8));
    assert_eq!(changed(&mut live).await, count(8));
    assert_eq!(live.rows(), &count(8));

    let subscription = live.subscription();
    drop(live);
    let client = engine.app_client(&chat());
    let again = client.call(Call::SqlUnsubscribe { subscription }).await;
    assert_eq!(code(again), ErrorCode::UnknownSubscription);
}

#[tokio::test(flavor = "multi_thread")]
async fn schemas_own_listed_and_foreign() {
    let engine = TestEngine::new();
    let sql = engine.app_client(&chat()).sql();
    // Its own second schema: read and write.
    assert!(sql
        .in_schema("chat_cache")
        .execute("delete from t", vec![])
        .await
        .is_ok());
    // A listed schema: read only.
    let contacts = sql.in_schema("contacts");
    assert!(contacts.query("select * from people", vec![]).await.is_ok());
    let write = contacts.execute("delete from people", vec![]).await;
    assert_eq!(code(write), ErrorCode::CapabilityDenied);
    // Anyone else's: nothing.
    let notes = sql.in_schema("notes");
    assert_eq!(
        code(notes.query("select 1", vec![]).await),
        ErrorCode::CapabilityDenied
    );
    assert_eq!(
        code(notes.live("select 1", vec![]).await),
        ErrorCode::CapabilityDenied
    );
    // The shell names a schema and reaches it.
    let shell = engine.client().sql();
    assert_eq!(
        code(shell.query("select 1", vec![]).await),
        ErrorCode::BadRequest
    );
    assert!(shell
        .in_schema("notes")
        .query("select 1", vec![])
        .await
        .is_ok());
}

#[tokio::test(flavor = "multi_thread")]
async fn sql_errors_and_the_capability_check() {
    let engine = TestEngine::new();
    let sql = engine.app_client(&chat()).sql();
    engine.sql().fail(
        "insert into nope values (1)",
        "relation \"nope\" does not exist",
    );
    let failed = sql.execute("insert into nope values (1)", vec![]).await;
    assert_eq!(code(failed), ErrorCode::SqlError);

    let without = engine.app_client(&app("com.isonomic.docs", &["docs"], None));
    let calls = [
        Call::SqlQuery {
            sql: "select 1".into(),
            params: vec![],
            schema: None,
        },
        Call::SqlExecute {
            sql: "delete from t".into(),
            params: vec![],
            schema: None,
        },
        Call::SqlBatch {
            statements: vec![],
            schema: None,
        },
        Call::SqlSubscribe {
            sql: "select 1".into(),
            params: vec![],
            schema: None,
        },
        Call::SqlUnsubscribe { subscription: 1 },
    ];
    for call in calls {
        let method = call.method();
        assert_eq!(
            code(without.call(call).await),
            ErrorCode::CapabilityDenied,
            "{method}"
        );
    }
    // A capability but no schema of its own: nothing to run in.
    let homeless = engine.app_client(&app("com.isonomic.x", &[], Some(SqlAccess::default())));
    assert_eq!(
        code(homeless.sql().query("select 1", vec![]).await),
        ErrorCode::CapabilityDenied
    );
}
