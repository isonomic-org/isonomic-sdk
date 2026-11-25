// Copyright 2026 Anud Inc.
// SPDX-License-Identifier: Apache-2.0

//! Method names and interfaces of every call.

use crate::{auth, Call};

impl Call {
    /// Every method this build knows, in interface order.
    pub const METHODS: &'static [&'static str] = &[
        "system.status",
        "system.subscribe",
        "system.unsubscribe",
        "system.logs",
        "system.profile",
        "system.shutdown",
        "system.retry",
        "apps.grant",
        "models.list",
        "models.info",
        "models.stream",
        "models.cancel",
        "models.embed",
        "models.download",
        "models.download_cancel",
        "models.recommended",
        "blobs.put",
        "blobs.create",
        "blobs.write",
        "blobs.get",
        "blobs.info",
        "blobs.delete",
        "events.publish",
        "events.subscribe",
        "events.unsubscribe",
        "sql.query",
        "sql.execute",
        "sql.batch",
        "sql.subscribe",
        "sql.unsubscribe",
        "sessions.create",
        "sessions.list",
        "sessions.open",
        "sessions.fork",
        "sessions.archive",
        "sessions.rename",
        "sessions.send",
        "sessions.continue",
        "sessions.cancel",
        "sessions.purge",
        "sessions.subscribe",
        "sessions.unsubscribe",
        "approvals.list",
        "approvals.decide",
        "questions.answer",
        "tools.register_client",
        "tools.result",
        "presets.list",
        "presets.get",
        "presets.put",
        "artifacts.get",
        "artifacts.list",
        "approvals.user_action",
        "identity.get",
        "identity.kit_export",
        "identity.kit_restore",
        "identity.kit_saved",
        "identity.kit_status",
    ];

    /// The method name on the wire, for logs and the Activity tab.
    pub fn method(&self) -> &'static str {
        match self {
            Call::SystemStatus => "system.status",
            Call::SystemSubscribe => "system.subscribe",
            Call::SystemUnsubscribe { .. } => "system.unsubscribe",
            Call::SystemLogs { .. } => "system.logs",
            Call::SystemProfile => "system.profile",
            Call::SystemShutdown => "system.shutdown",
            Call::SystemRetry { .. } => "system.retry",
            Call::AppsGrant { .. } => "apps.grant",
            Call::ModelsList => "models.list",
            Call::ModelsInfo { .. } => "models.info",
            Call::ModelsStream(_) => "models.stream",
            Call::ModelsCancel { .. } => "models.cancel",
            Call::ModelsEmbed { .. } => "models.embed",
            Call::ModelsDownload(_) => "models.download",
            Call::ModelsDownloadCancel(_) => "models.download_cancel",
            Call::ModelsRecommended(_) => "models.recommended",
            Call::BlobsPut { .. } => "blobs.put",
            Call::BlobsCreate { .. } => "blobs.create",
            Call::BlobsWrite { .. } => "blobs.write",
            Call::BlobsGet { .. } => "blobs.get",
            Call::BlobsInfo { .. } => "blobs.info",
            Call::BlobsDelete { .. } => "blobs.delete",
            Call::EventsPublish { .. } => "events.publish",
            Call::EventsSubscribe { .. } => "events.subscribe",
            Call::EventsUnsubscribe { .. } => "events.unsubscribe",
            Call::SqlQuery { .. } => "sql.query",
            Call::SqlExecute { .. } => "sql.execute",
            Call::SqlBatch { .. } => "sql.batch",
            Call::SqlSubscribe { .. } => "sql.subscribe",
            Call::SqlUnsubscribe { .. } => "sql.unsubscribe",
            Call::SessionsCreate(_) => "sessions.create",
            Call::SessionsList(_) => "sessions.list",
            Call::SessionsOpen(_) => "sessions.open",
            Call::SessionsFork(_) => "sessions.fork",
            Call::SessionsArchive(_) => "sessions.archive",
            Call::SessionsRename(_) => "sessions.rename",
            Call::SessionsSend(_) => "sessions.send",
            Call::SessionsContinue(_) => "sessions.continue",
            Call::SessionsCancel(_) => "sessions.cancel",
            Call::SessionsPurge(_) => "sessions.purge",
            Call::SessionsSubscribe(_) => "sessions.subscribe",
            Call::SessionsUnsubscribe { .. } => "sessions.unsubscribe",
            Call::ApprovalsList(_) => "approvals.list",
            Call::ApprovalsDecide(_) => "approvals.decide",
            Call::QuestionsAnswer(_) => "questions.answer",
            Call::ToolsRegisterClient(_) => "tools.register_client",
            Call::ToolsResult(_) => "tools.result",
            Call::PresetsList(_) => "presets.list",
            Call::PresetsGet(_) => "presets.get",
            Call::PresetsPut(_) => "presets.put",
            Call::ArtifactsGet(_) => "artifacts.get",
            Call::ArtifactsList(_) => "artifacts.list",
            Call::ApprovalsUserAction(_) => "approvals.user_action",
            Call::IdentityGet(_) => "identity.get",
            Call::IdentityKitExport(_) => "identity.kit_export",
            Call::IdentityKitRestore(_) => "identity.kit_restore",
            Call::IdentityKitSaved(_) => "identity.kit_saved",
            Call::IdentityKitStatus(_) => "identity.kit_status",
        }
    }

    pub fn interface(&self) -> auth::Interface {
        use auth::Interface;
        match self {
            Call::SystemStatus
            | Call::SystemSubscribe
            | Call::SystemUnsubscribe { .. }
            | Call::SystemLogs { .. }
            | Call::SystemProfile
            | Call::SystemShutdown
            | Call::SystemRetry { .. } => Interface::System,
            Call::AppsGrant { .. } => Interface::Apps,
            Call::ModelsList
            | Call::ModelsInfo { .. }
            | Call::ModelsStream(_)
            | Call::ModelsCancel { .. }
            | Call::ModelsEmbed { .. }
            | Call::ModelsDownload(_)
            | Call::ModelsDownloadCancel(_)
            | Call::ModelsRecommended(_) => Interface::Models,
            Call::BlobsPut { .. }
            | Call::BlobsCreate { .. }
            | Call::BlobsWrite { .. }
            | Call::BlobsGet { .. }
            | Call::BlobsInfo { .. }
            | Call::BlobsDelete { .. } => Interface::Blobs,
            Call::EventsPublish { .. }
            | Call::EventsSubscribe { .. }
            | Call::EventsUnsubscribe { .. } => Interface::Events,
            Call::SqlQuery { .. }
            | Call::SqlExecute { .. }
            | Call::SqlBatch { .. }
            | Call::SqlSubscribe { .. }
            | Call::SqlUnsubscribe { .. } => Interface::Sql,
            Call::SessionsCreate(_)
            | Call::SessionsList(_)
            | Call::SessionsOpen(_)
            | Call::SessionsFork(_)
            | Call::SessionsArchive(_)
            | Call::SessionsRename(_)
            | Call::SessionsSend(_)
            | Call::SessionsContinue(_)
            | Call::SessionsCancel(_)
            | Call::SessionsPurge(_)
            | Call::SessionsSubscribe(_)
            | Call::SessionsUnsubscribe { .. }
            | Call::ApprovalsList(_)
            | Call::ApprovalsDecide(_)
            | Call::QuestionsAnswer(_)
            | Call::ToolsRegisterClient(_)
            | Call::ToolsResult(_)
            | Call::PresetsList(_)
            | Call::PresetsGet(_)
            | Call::PresetsPut(_)
            | Call::ArtifactsGet(_)
            | Call::ArtifactsList(_)
            | Call::ApprovalsUserAction(_) => Interface::Agents,
            Call::IdentityGet(_)
            | Call::IdentityKitExport(_)
            | Call::IdentityKitRestore(_)
            | Call::IdentityKitSaved(_)
            | Call::IdentityKitStatus(_) => Interface::Identity,
        }
    }

    /// Calls only the window makes, whatever an app's capabilities: the
    /// model install calls, and minting a person's user-action token.
    pub fn window_only(&self) -> bool {
        self.installs_models() || matches!(self, Call::ApprovalsUserAction(_))
    }

    /// `models.download`, `download_cancel` and `recommended`: the shell's
    /// alone, like `system.*`, whatever the app's capabilities.
    pub fn installs_models(&self) -> bool {
        matches!(
            self,
            Call::ModelsDownload(_) | Call::ModelsDownloadCancel(_) | Call::ModelsRecommended(_)
        )
    }
}
