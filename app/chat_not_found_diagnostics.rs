use std::path::{Path, PathBuf};

use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::ApiError;

pub(crate) const CHAT_NOT_FOUND_DIAGNOSTIC_HEADER: &str = "x-foco-chat-not-found-diagnostic-id";
pub(crate) const CHAT_NOT_FOUND_OPERATION_HEADER: &str = "x-foco-chat-not-found-operation";
pub(crate) const CHAT_NOT_FOUND_PHASE_HEADER: &str = "x-foco-chat-not-found-phase";

/// Server-side correlation fields for a missing-chat failure.
///
/// The full record is emitted only to structured logs. HTTP responses receive
/// [`ChatNotFoundClientDiagnostic`], which deliberately omits workspace and
/// database identity details.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ChatNotFoundDiagnostic {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) diagnostic_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) operation: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) phase: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) workspace_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) chat_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) runtime_topology: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) database_identity: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) queued_user_message_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) run_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) agent_task_id: Option<String>,
}

/// Safe subset attached to an API error. The legacy `error` string remains
/// present, so clients that do not know this field continue to work.
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ChatNotFoundClientDiagnostic {
    pub(crate) diagnostic_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) operation: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) phase: Option<String>,
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum ChatRuntimeTopology {
    Local,
    RemoteSidecar,
}

impl ChatRuntimeTopology {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Local => "local",
            Self::RemoteSidecar => "remote-sidecar",
        }
    }
}

/// Input available at the point a chat lookup fails. All fields are optional
/// because stream recovery and scheduler paths do not always retain the same
/// durable identifiers.
#[derive(Clone, Debug, Default)]
pub(crate) struct ChatNotFoundDiagnosticContext {
    pub(crate) operation: Option<&'static str>,
    pub(crate) phase: Option<&'static str>,
    pub(crate) workspace_id: Option<String>,
    pub(crate) chat_id: Option<String>,
    pub(crate) runtime_topology: Option<ChatRuntimeTopology>,
    pub(crate) database_path: Option<PathBuf>,
    pub(crate) queued_user_message_id: Option<String>,
    pub(crate) run_id: Option<String>,
    pub(crate) agent_task_id: Option<String>,
}

impl ChatNotFoundDiagnosticContext {
    pub(crate) fn local(
        operation: &'static str,
        phase: &'static str,
        workspace_id: impl Into<String>,
        database_path: impl AsRef<Path>,
    ) -> Self {
        Self {
            operation: Some(operation),
            phase: Some(phase),
            workspace_id: Some(workspace_id.into()),
            runtime_topology: Some(ChatRuntimeTopology::Local),
            database_path: Some(database_path.as_ref().to_path_buf()),
            ..Self::default()
        }
    }

    pub(crate) fn remote_sidecar(
        operation: &'static str,
        phase: &'static str,
        workspace_id: impl Into<String>,
        database_path: impl AsRef<Path>,
    ) -> Self {
        Self {
            operation: Some(operation),
            phase: Some(phase),
            workspace_id: Some(workspace_id.into()),
            runtime_topology: Some(ChatRuntimeTopology::RemoteSidecar),
            database_path: Some(database_path.as_ref().to_path_buf()),
            ..Self::default()
        }
    }

    pub(crate) fn with_chat_id(mut self, chat_id: impl Into<String>) -> Self {
        self.chat_id = Some(chat_id.into());
        self
    }

    pub(crate) fn with_queued_user_message_id(mut self, message_id: impl Into<String>) -> Self {
        self.queued_user_message_id = Some(message_id.into());
        self
    }

    pub(crate) fn with_queued_user_message_id_opt(mut self, message_id: Option<String>) -> Self {
        self.queued_user_message_id = message_id;
        self
    }

    pub(crate) fn with_run_id(mut self, run_id: impl Into<String>) -> Self {
        self.run_id = Some(run_id.into());
        self
    }

    pub(crate) fn with_agent_task_id(mut self, task_id: impl Into<String>) -> Self {
        self.agent_task_id = Some(task_id.into());
        self
    }

    pub(crate) fn with_agent_task_id_opt(mut self, task_id: Option<String>) -> Self {
        self.agent_task_id = task_id;
        self
    }

    fn into_diagnostic(self) -> ChatNotFoundDiagnostic {
        ChatNotFoundDiagnostic {
            diagnostic_id: Some(opaque_diagnostic_id()),
            operation: self.operation.map(str::to_string),
            phase: self.phase.map(str::to_string),
            workspace_id: self.workspace_id,
            chat_id: self.chat_id,
            runtime_topology: self
                .runtime_topology
                .map(ChatRuntimeTopology::as_str)
                .map(str::to_string),
            database_identity: self.database_path.as_deref().map(database_identity),
            queued_user_message_id: self.queued_user_message_id,
            run_id: self.run_id,
            agent_task_id: self.agent_task_id,
        }
    }
}

pub(crate) fn api_error_for_missing_chat(context: ChatNotFoundDiagnosticContext) -> ApiError {
    let diagnostic = context.into_diagnostic();
    let client_diagnostic = client_diagnostic(&diagnostic);
    tracing::warn!(
        event = "chat_not_found",
        diagnostic_id = %client_diagnostic.diagnostic_id,
        operation = ?diagnostic.operation,
        phase = ?diagnostic.phase,
        workspace_id = ?diagnostic.workspace_id,
        chat_id = ?diagnostic.chat_id,
        runtime_topology = ?diagnostic.runtime_topology,
        database_identity = ?diagnostic.database_identity,
        queued_user_message_id = ?diagnostic.queued_user_message_id,
        run_id = ?diagnostic.run_id,
        agent_task_id = ?diagnostic.agent_task_id,
        "chat lookup did not find a durable chat record"
    );

    ApiError::bad_request(format!(
        "chat was not found (diagnostic reference: {})",
        client_diagnostic.diagnostic_id
    ))
    .with_chat_not_found_diagnostic(client_diagnostic)
}

fn client_diagnostic(diagnostic: &ChatNotFoundDiagnostic) -> ChatNotFoundClientDiagnostic {
    ChatNotFoundClientDiagnostic {
        diagnostic_id: diagnostic
            .diagnostic_id
            .clone()
            .unwrap_or_else(|| "chat-not-found-unavailable".to_string()),
        operation: diagnostic.operation.clone(),
        phase: diagnostic.phase.clone(),
    }
}

fn database_identity(path: &Path) -> String {
    let normalized = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    let mut hasher = Sha256::new();
    hasher.update(b"foco-chat-not-found-database-v1\0");
    hasher.update(normalized.to_string_lossy().as_bytes());
    let digest = hasher.finalize();
    format!("db-sha256:{digest:x}")
}

fn opaque_diagnostic_id() -> String {
    let mut entropy = [0_u8; 32];
    let mut hasher = Sha256::new();
    hasher.update(b"foco-chat-not-found-diagnostic-v1\0");
    if getrandom::fill(&mut entropy).is_ok() {
        hasher.update(entropy);
    } else {
        // A diagnostic must never prevent the original error from being
        // reported. Hashing the fallback keeps its process details opaque.
        hasher.update(crate::unique_id("chat-not-found-fallback"));
    }
    format!("chat-not-found-{:x}", hasher.finalize())
}

#[cfg(test)]
mod tests {
    use axum::{body::to_bytes, response::IntoResponse};

    use super::*;

    #[test]
    fn database_identity_is_stable_and_does_not_include_the_path() {
        let path = Path::new("/private/workspace/.foco/workspace.sqlite");

        let identity = database_identity(path);

        assert!(identity.starts_with("db-sha256:"));
        assert!(!identity.contains("workspace"));
    }

    #[test]
    fn client_diagnostic_excludes_internal_correlation_fields() {
        let diagnostic = ChatNotFoundDiagnosticContext::local(
            "chat.queue",
            "durable-chat-lookup",
            "workspace-1",
            "/private/workspace/.foco/workspace.sqlite",
        )
        .with_chat_id("chat-1")
        .with_queued_user_message_id("message-1")
        .with_run_id("run-1")
        .with_agent_task_id("task-1")
        .into_diagnostic();

        let serialized = serde_json::to_value(client_diagnostic(&diagnostic)).unwrap();

        assert_eq!(serialized["operation"], "chat.queue");
        assert_eq!(serialized["phase"], "durable-chat-lookup");
        assert!(serialized.get("workspaceId").is_none());
        assert!(serialized.get("chatId").is_none());
        assert!(serialized.get("databaseIdentity").is_none());
        assert!(serialized.get("queuedUserMessageId").is_none());
        assert!(serialized.get("runId").is_none());
        assert!(serialized.get("agentTaskId").is_none());
    }

    #[tokio::test]
    async fn api_error_preserves_legacy_error_and_exposes_only_safe_diagnostic_fields() {
        let response = api_error_for_missing_chat(
            ChatNotFoundDiagnosticContext::remote_sidecar(
                "context.usage",
                "existing-chat-lookup",
                "workspace-1",
                "/private/workspace/.foco/workspace.sqlite",
            )
            .with_chat_id("chat-1")
            .with_queued_user_message_id("message-1")
            .with_run_id("run-1")
            .with_agent_task_id("task-1"),
        )
        .into_response();

        let diagnostic_id = response
            .headers()
            .get(CHAT_NOT_FOUND_DIAGNOSTIC_HEADER)
            .and_then(|value| value.to_str().ok())
            .expect("diagnostic response header")
            .to_string();
        assert_eq!(
            response
                .headers()
                .get(CHAT_NOT_FOUND_OPERATION_HEADER)
                .and_then(|value| value.to_str().ok()),
            Some("context.usage")
        );
        assert_eq!(
            response
                .headers()
                .get(CHAT_NOT_FOUND_PHASE_HEADER)
                .and_then(|value| value.to_str().ok()),
            Some("existing-chat-lookup")
        );

        let body = to_bytes(response.into_body(), 4_096)
            .await
            .expect("error response body");
        let body = serde_json::from_slice::<serde_json::Value>(&body).expect("error JSON");

        assert_eq!(
            body["error"],
            format!("chat was not found (diagnostic reference: {diagnostic_id})")
        );
        assert_eq!(body["diagnostic"]["diagnosticId"], diagnostic_id);
        assert!(
            !body["error"]
                .as_str()
                .is_some_and(|message| message.contains("chat-1"))
        );
        assert_eq!(body["diagnostic"]["operation"], "context.usage");
        assert_eq!(body["diagnostic"]["phase"], "existing-chat-lookup");
        assert!(body["diagnostic"].get("workspaceId").is_none());
        assert!(body["diagnostic"].get("chatId").is_none());
        assert!(body["diagnostic"].get("databaseIdentity").is_none());
        assert!(body["diagnostic"].get("queuedUserMessageId").is_none());
        assert!(body["diagnostic"].get("runId").is_none());
        assert!(body["diagnostic"].get("agentTaskId").is_none());
    }
}
