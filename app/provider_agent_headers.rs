//! OpenAIResp Agent correlation header helpers (session/thread mapping).
//!
//! See `docs/agent-openai-request-headers-contract.md`.

use std::path::Path;

use foco_providers::{
    AgentRequestCorrelation, NeutralChatRequest, resolve_agent_session_thread_ids,
};
use foco_store::workspace::WorkspaceDatabase;

use crate::{ApiError, open_workspace_database};

/// Attach OpenAIResp Agent correlation fields onto a provider request.
pub(crate) fn attach_agent_request_correlation(
    request: &mut NeutralChatRequest,
    session_id: &str,
    thread_id: &str,
    client_request_id: &str,
    run_id: Option<&str>,
    workspace_id: Option<&str>,
) {
    let session_id = session_id.trim();
    let thread_id = thread_id.trim();
    let client_request_id = client_request_id.trim();
    if session_id.is_empty() || thread_id.is_empty() || client_request_id.is_empty() {
        return;
    }
    let mut correlation =
        AgentRequestCorrelation::new(session_id, thread_id, client_request_id);
    if let Some(run_id) = run_id.map(str::trim).filter(|value| !value.is_empty()) {
        correlation = correlation.with_run_id(run_id);
    }
    if let Some(workspace_id) = workspace_id
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        correlation = correlation.with_workspace_id(workspace_id);
    }
    request.agent_correlation = Some(correlation);
}

/// Resolve `session-id` / `thread-id` for a chat using workspace SQLite plan bindings.
pub(crate) fn resolve_provider_session_thread_for_chat(
    workspace_path: &Path,
    chat_id: &str,
    parent_chat_id: Option<&str>,
    plan_id_hint: Option<&str>,
) -> Result<(String, String), ApiError> {
    let chat_id = chat_id.trim();
    if chat_id.is_empty() {
        return Ok((String::new(), String::new()));
    }
    if let Some(plan_id) = plan_id_hint.map(str::trim).filter(|value| !value.is_empty()) {
        return Ok(resolve_agent_session_thread_ids(
            chat_id,
            Some(plan_id),
            None,
        ));
    }

    let database = open_workspace_database(workspace_path)?;
    resolve_provider_session_thread_with_database(&database, chat_id, parent_chat_id)
}

pub(crate) fn resolve_provider_session_thread_with_database(
    database: &WorkspaceDatabase,
    chat_id: &str,
    parent_chat_id: Option<&str>,
) -> Result<(String, String), ApiError> {
    let plan_id_for_chat = database
        .plan_id_for_implementation_chat(chat_id)
        .map_err(ApiError::from_workspace_error)?;
    let plan_id_for_parent = match parent_chat_id.map(str::trim).filter(|value| !value.is_empty()) {
        Some(parent_chat_id) if parent_chat_id != chat_id.trim() => database
            .plan_id_for_implementation_chat(parent_chat_id)
            .map_err(ApiError::from_workspace_error)?,
        _ => None,
    };
    Ok(resolve_agent_session_thread_ids(
        chat_id,
        plan_id_for_chat.as_deref(),
        plan_id_for_parent.as_deref(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use foco_providers::resolve_agent_session_thread_ids;

    #[test]
    fn normal_chat_uses_chat_id_for_session_and_thread() {
        let (session, thread) =
            resolve_agent_session_thread_ids("chat-abc", None, None);
        assert_eq!(session, "chat-abc");
        assert_eq!(thread, "chat-abc");
    }

    #[test]
    fn plan_implementation_uses_plan_session() {
        let (session, thread) =
            resolve_agent_session_thread_ids("chat-impl-1", Some("plan-xyz"), None);
        assert_eq!(session, "plan-xyz");
        assert_eq!(thread, "chat-impl-1");
    }

    #[test]
    fn subagent_inherits_plan_session_from_parent() {
        let (session, thread) =
            resolve_agent_session_thread_ids("chat-sub-9", None, Some("plan-xyz"));
        assert_eq!(session, "plan-xyz");
        assert_eq!(thread, "chat-sub-9");
    }

    #[test]
    fn attach_skips_empty_ids() {
        let mut request = NeutralChatRequest {
            model_id: "m".to_string(),
            messages: Vec::new(),
            tools: Vec::new(),
            thinking_level: None,
            max_output_tokens: None,
            prompt_cache_key: None,
            prompt_cache_retention: None,
            agent_correlation: None,
        };
        attach_agent_request_correlation(&mut request, "", "t", "r", None, None);
        assert!(request.agent_correlation.is_none());
        attach_agent_request_correlation(
            &mut request,
            "s",
            "t",
            "req-1",
            Some("run-1"),
            Some("ws-1"),
        );
        let correlation = request.agent_correlation.expect("set");
        assert_eq!(correlation.session_id, "s");
        assert_eq!(correlation.thread_id, "t");
        assert_eq!(correlation.client_request_id, "req-1");
        assert_eq!(correlation.run_id.as_deref(), Some("run-1"));
        assert_eq!(correlation.workspace_id.as_deref(), Some("ws-1"));
    }
}
