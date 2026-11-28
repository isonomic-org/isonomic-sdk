// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: GPL-3.0-or-later

//! The recovery kit, checked without ever exporting one: every
//! `identity.kit_*` call is refused to an app, however much it may do, and
//! an export the person did not ask for is refused to the shell too.
//!
//! The only words sent are the word-list standard's public all-zero test
//! vector, and only in calls that must be refused.

use isonomic_proto::identity::{KitExport, KitRestore, RecoveryWords};
use isonomic_proto::{Call, ErrorCode};

use crate::{manifest, refused, tokens, Outcome, Target};

fn test_vector() -> RecoveryWords {
    let mut words = vec!["abandon".to_string(); 23];
    words.push("art".into());
    RecoveryWords::new(words)
}

/// An app with every capability is refused all four kit calls.
pub async fn apps_refused(target: Target) -> Outcome {
    let app = target
        .app(manifest("recovery-kit", tokens::everything(), &[]))
        .await?;
    let calls = [
        Call::IdentityKitExport(KitExport { person_asked: true }),
        Call::IdentityKitRestore(KitRestore {
            kit: "{}".into(),
            words: test_vector(),
            replace_unused: false,
        }),
        Call::IdentityKitSaved(Default::default()),
        Call::IdentityKitStatus(Default::default()),
    ];
    for call in calls {
        let method = call.method();
        refused(app.call(call).await, ErrorCode::CapabilityDenied, method)?;
    }
    Ok(())
}

/// The shell's export without `person_asked` is refused, and hands out no
/// words.
pub async fn export_needs_person(target: Target) -> Outcome {
    refused(
        target.shell.kit_export(false).await,
        ErrorCode::RecoveryRefused,
        "identity.kit_export without person_asked",
    )
}
