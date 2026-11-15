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
            Call::SessionsPurge => "sessions.purge",
            Call::SessionsSubscribe(_) => "sessions.subscribe",
            Call::SessionsUnsubscribe { .. } => "sessions.unsubscribe",
            Call::ApprovalsList => "approvals.list",
            Call::ApprovalsDecide(_) => "approvals.decide",
            Call::QuestionsAnswer(_) => "questions.answer",
            Call::ToolsRegisterClient(_) => "tools.register_client",
            Call::ToolsResult(_) => "tools.result",
            Call::PresetsList => "presets.list",
            Call::PresetsGet(_) => "presets.get",
            Call::PresetsPut(_) => "presets.put",
            Call::ArtifactsGet(_) => "artifacts.get",
            Call::ArtifactsList(_) => "artifacts.list",
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
            | Call::ModelsEmbed { .. } => Interface::Models,
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
            | Call::SessionsPurge
            | Call::SessionsSubscribe(_)
            | Call::SessionsUnsubscribe { .. }
            | Call::ApprovalsList
            | Call::ApprovalsDecide(_)
            | Call::QuestionsAnswer(_)
            | Call::ToolsRegisterClient(_)
            | Call::ToolsResult(_)
            | Call::PresetsList
            | Call::PresetsGet(_)
            | Call::PresetsPut(_)
            | Call::ArtifactsGet(_)
            | Call::ArtifactsList(_) => Interface::Agents,
        }
    }
}
