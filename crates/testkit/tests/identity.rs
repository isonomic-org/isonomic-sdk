// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! The person's Isonomic ID: made once, the same ever after, refusable,
//! and the shell's alone.

use isonomic_client::ClientError;
use isonomic_proto::{Call, Capabilities, ErrorCode, Manifest};
use isonomic_testkit::{TestEngine, DEFAULT_IDENTITY};

#[tokio::test]
async fn the_id_is_made_once_and_the_same_after() {
    let engine = TestEngine::new();
    let shell = engine.client();
    assert!(!engine.identity().made());
    let first = shell.identity().await.unwrap();
    assert_eq!((first.id.as_str(), first.created), (DEFAULT_IDENTITY, true));
    let again = shell.identity().await.unwrap();
    assert_eq!(
        (again.id.as_str(), again.created),
        (DEFAULT_IDENTITY, false)
    );
    assert_eq!(
        engine.client().identity().await.unwrap(),
        again,
        "any window sees it"
    );
    engine.identity().forget();
    assert!(shell.identity().await.unwrap().created, "a first run again");
}

#[tokio::test]
async fn a_scripted_id_and_a_refusal_in_plain_words() {
    let engine = TestEngine::new();
    engine.identity().set_id("isonomic:7Q4K-M2XD-9PLV");
    let shell = engine.client();
    let words = "The key store did not let Isonomic save your ID.";
    engine.identity().refuse(words);
    match shell.identity().await {
        Err(ClientError::Engine(error)) => {
            assert_eq!(error.code, ErrorCode::IdentityUnavailable);
            assert_eq!(error.message, words, "shown as written");
        }
        other => panic!("expected identity_unavailable, got {other:?}"),
    }
    assert!(!engine.identity().made(), "a refusal makes nothing");
    engine.identity().recover();
    let made = shell.identity().await.unwrap();
    assert_eq!(
        (made.id.as_str(), made.created),
        ("isonomic:7Q4K-M2XD-9PLV", true)
    );
}

#[tokio::test]
async fn apps_never_see_the_id() {
    let engine = TestEngine::new();
    let manifest = Manifest {
        id: "com.isonomic.chat".into(),
        name: "Chat".into(),
        version: "0.1.0".into(),
        icon: "chat".into(),
        capabilities: Capabilities::default(),
        schemas: vec![],
    };
    let app = engine.client().scoped(engine.grant(&manifest));
    match app.call(Call::IdentityGet(Default::default())).await {
        Err(ClientError::Engine(error)) => assert_eq!(error.code, ErrorCode::CapabilityDenied),
        other => panic!("expected capability_denied, got {other:?}"),
    }
    assert!(!engine.identity().made(), "an app's call makes nothing");
}

mod kit {
    //! The recovery kit: exported only when the person asked, restored on
    //! another engine as a new kit, refused in plain words, the shell's
    //! alone, and never printed.

    use isonomic_client::ClientError;
    use isonomic_proto::identity::{KitRestore, KitStatus, RecoveryWords};
    use isonomic_proto::{Call, Capabilities, ErrorCode, Manifest};
    use isonomic_testkit::{kit_words, TestEngine, DEFAULT_IDENTITY, DEFAULT_KIT_NOW};

    fn refusal<T: std::fmt::Debug>(result: Result<T, ClientError>) -> String {
        match result {
            Err(ClientError::Engine(error)) => {
                assert_eq!(error.code, ErrorCode::RecoveryRefused, "{}", error.message);
                assert!(
                    error.message.ends_with('.'),
                    "plain words: {}",
                    error.message
                );
                error.message
            }
            other => panic!("expected recovery_refused, got {other:?}"),
        }
    }

    fn restore(kit: &str, words: RecoveryWords, replace_unused: bool) -> KitRestore {
        KitRestore {
            kit: kit.into(),
            words,
            replace_unused,
        }
    }

    #[tokio::test]
    async fn exported_only_when_asked_then_saved() {
        let engine = TestEngine::new();
        let shell = engine.client();
        refusal(shell.kit_export(false).await);
        assert_eq!(shell.kit_status().await.unwrap(), KitStatus::default());
        refusal(shell.kit_saved().await);

        let kit = shell.kit_export(true).await.unwrap();
        assert_eq!(kit.id, DEFAULT_IDENTITY);
        assert_eq!(kit.words.len(), 24);
        assert_eq!(kit.words, kit_words(0));
        assert!(
            !kit.kit.contains(&kit.words.reveal()[23]),
            "the document holds no words"
        );
        let printed = format!("{kit:?}");
        assert!(
            !printed.contains("abandon") && !printed.contains(" art"),
            "{printed}"
        );

        engine.identity().set_now(DEFAULT_KIT_NOW + 60);
        shell.kit_saved().await.unwrap();
        let status = shell.kit_status().await.unwrap();
        assert_eq!(status.exported_at, Some(DEFAULT_KIT_NOW));
        assert_eq!(status.saved_at, Some(DEFAULT_KIT_NOW + 60));
    }

    #[tokio::test]
    async fn a_kit_restores_on_another_computer_as_a_new_kit() {
        let old = TestEngine::new();
        old.identity().set_id("isonomic:7Q4K-M2XD-9PLV");
        let kit = old.client().kit_export(true).await.unwrap();

        let new = TestEngine::new();
        new.identity().set_now(DEFAULT_KIT_NOW + 3600);
        let shell = new.client();
        let restored = shell
            .kit_restore(restore(&kit.kit, kit.words.clone(), false))
            .await
            .unwrap();
        assert_eq!(restored.id, "isonomic:7Q4K-M2XD-9PLV", "the same ID");
        assert_eq!(
            restored.kit_made, DEFAULT_KIT_NOW,
            "when the restored kit was made"
        );
        assert_eq!(restored.note, isonomic_testkit::KIT_RESTORE_NOTE);
        assert_ne!(restored.words, kit.words, "the old kit is spent");
        assert_ne!(restored.kit, kit.kit);
        assert_eq!(new.identity().kit_generation(), 1);
        assert_eq!(
            shell.identity().await.unwrap().id,
            "isonomic:7Q4K-M2XD-9PLV"
        );
        let status = shell.kit_status().await.unwrap();
        assert_eq!(
            (status.saved_at, status.exported_at),
            (None, Some(DEFAULT_KIT_NOW + 3600))
        );

        // The spent kit no longer restores here; the new one does.
        let spent = refusal(
            shell
                .kit_restore(restore(&kit.kit, kit.words.clone(), true))
                .await,
        );
        assert!(spent.contains("replaced by a newer one"), "{spent}");
        shell
            .kit_restore(restore(&restored.kit, restored.words, true))
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn refusals_change_nothing() {
        let old = TestEngine::new();
        let kit = old.client().kit_export(true).await.unwrap();
        let new = TestEngine::new();
        let shell = new.client();

        let wrong = refusal(
            shell
                .kit_restore(restore(&kit.kit, kit_words(3), false))
                .await,
        );
        assert!(wrong.contains("not this kit's"), "{wrong}");
        let mut typo = kit.words.reveal().to_vec();
        typo[23] = "abandon".into();
        let typo = RecoveryWords::new(typo);
        let unsound = refusal(shell.kit_restore(restore(&kit.kit, typo, false)).await);
        assert!(unsound.contains("don't add up"), "{unsound}");
        let damaged = refusal(
            shell
                .kit_restore(restore("{\"format\":", kit.words.clone(), false))
                .await,
        );
        assert!(damaged.contains("damaged"), "{damaged}");

        // Another ID written into a valid kit: the check no longer holds.
        let mut edited: serde_json::Value = serde_json::from_str(&kit.kit).unwrap();
        edited["id"] = "isonomic:someone-else".into();
        let edited = refusal(
            shell
                .kit_restore(restore(&edited.to_string(), kit.words.clone(), false))
                .await,
        );
        assert!(edited.contains("damaged"), "{edited}");

        // A generation at the end of its range, with a check that holds, is
        // refused, not wrapped.
        let mut last: serde_json::Value = serde_json::from_str(&kit.kit).unwrap();
        // The body in the document's own field order, as its check covers.
        let body = format!(
            r#"{{"format":{},"version":{},"id":{},"generation":{},"made":{}}}"#,
            last["format"],
            last["version"],
            last["id"],
            u32::MAX,
            last["made"]
        );
        use sha2::Digest;
        let check: String = sha2::Sha256::digest(body.as_bytes())
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        last["generation"] = u32::MAX.into();
        last["check"] = check.into();
        let words = kit_words(u32::MAX);
        let overflow = refusal(
            shell
                .kit_restore(restore(&last.to_string(), words, false))
                .await,
        );
        assert!(overflow.contains("damaged"), "{overflow}");
        assert!(!new.identity().made(), "nothing was made");

        shell.identity().await.unwrap();
        let unused = refusal(
            shell
                .kit_restore(restore(&kit.kit, kit.words.clone(), false))
                .await,
        );
        assert!(unused.contains("only if you choose to"), "{unused}");
        new.identity().set_in_use(true);
        let in_use = refusal(
            shell
                .kit_restore(restore(&kit.kit, kit.words.clone(), true))
                .await,
        );
        assert!(in_use.contains("apps and chats"), "{in_use}");
        new.identity().set_in_use(false);
        shell
            .kit_restore(restore(&kit.kit, kit.words, true))
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn apps_never_reach_the_kit() {
        let engine = TestEngine::new();
        let manifest = Manifest {
            id: "com.isonomic.chat".into(),
            name: "Chat".into(),
            version: "0.1.0".into(),
            icon: "chat".into(),
            capabilities: Capabilities::default(),
            schemas: vec![],
        };
        let app = engine.client().scoped(engine.grant(&manifest));
        let calls = [
            Call::IdentityKitExport(isonomic_proto::identity::KitExport { person_asked: true }),
            Call::IdentityKitRestore(restore("{}", kit_words(0), true)),
            Call::IdentityKitSaved(Default::default()),
            Call::IdentityKitStatus(Default::default()),
        ];
        for call in calls {
            let method = call.method();
            match app.call(call).await {
                Err(ClientError::Engine(error)) => {
                    assert_eq!(error.code, ErrorCode::CapabilityDenied, "{method}")
                }
                other => panic!("{method}: expected capability_denied, got {other:?}"),
            }
        }
        assert!(!engine.identity().made(), "an app's call makes nothing");
    }
}
