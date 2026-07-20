use std::time::Instant;

use axum::{
    Router,
    body::Body,
    extract::{DefaultBodyLimit, FromRequestParts, Request, State, WebSocketUpgrade},
    http::{HeaderMap, StatusCode, header},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{delete, get, patch, post, put},
};

use serde_json::Value;

use crate::{
    AppState, CHAT_ATTACHMENT_BODY_LIMIT_BYTES, WORKSPACE_LOGO_BODY_LIMIT_BYTES,
    http::assets::static_asset,
};

pub(crate) fn app_router(state: AppState) -> Router {
    let auth_state = state.clone();

    Router::new()
        .route("/api/health", get(crate::http::auth::health))
        .route("/api/auth/status", get(crate::http::auth::auth_status))
        .route("/api/auth/login", post(crate::http::auth::auth_login))
        .route("/api/auth/logout", post(crate::http::auth::auth_logout))
        .route("/api/workspaces", get(crate::http::workspaces::workspaces))
        .route(
            "/api/workspaces/search-chats",
            get(crate::http::workspaces::search_workspace_chats),
        )
        .route(
            "/api/workspaces/add",
            post(crate::http::workspaces::add_workspace),
        )
        .route(
            "/api/workspaces/manual",
            post(crate::http::workspaces::save_workspace_settings),
        )
        .route(
            "/api/workspaces/{workspace_id}",
            delete(crate::http::workspaces::delete_workspace),
        )
        .route(
            "/api/workspaces/order",
            post(crate::http::workspaces::save_workspace_order),
        )
        .route(
            "/api/remote-servers",
            get(crate::http::remote_servers::list_remote_servers),
        )
        .route(
            "/api/remote-servers/create",
            post(crate::http::remote_servers::create_remote_server),
        )
        .route(
            "/api/remote-servers/update",
            post(crate::http::remote_servers::update_remote_server),
        )
        .route(
            "/api/remote-servers/delete",
            post(crate::http::remote_servers::delete_remote_server),
        )
        .route(
            "/api/remote-servers/{server_id}/test",
            post(crate::http::remote_servers::test_remote_server),
        )
        .route(
            "/api/remote-servers/{server_id}/connect",
            post(crate::http::remote_servers::connect_remote_server),
        )
        .route(
            "/api/remote-servers/{server_id}/trust-host-key",
            post(crate::http::remote_servers::trust_remote_server_host_key),
        )
        .route(
            "/api/remote-servers/{server_id}/disconnect",
            post(crate::http::remote_servers::disconnect_remote_server),
        )
        .route(
            "/api/remote-servers/{server_id}/status",
            get(crate::http::remote_servers::remote_server_status),
        )
        .route(
            "/api/remote-servers/{server_id}/sessions",
            get(crate::remote_workspace::remote_workspace_sessions),
        )
        .route(
            "/api/remote-servers/{server_id}/workspaces/{workspace_id}/connect",
            post(crate::remote_workspace::connect_remote_workspace),
        )
        .route(
            "/api/remote-servers/{server_id}/workspaces/{workspace_id}/disconnect",
            post(crate::remote_workspace::disconnect_remote_workspace),
        )
        .route(
            "/api/workspaces/{workspace_id}/chats",
            get(crate::http::workspaces::workspace_chats),
        )
        .route(
            "/api/workspaces/{workspace_id}/files",
            get(crate::http::workspaces::workspace_files),
        )
        .route(
            "/api/workspaces/{workspace_id}/files/children",
            get(crate::http::workspaces::workspace_file_children),
        )
        .route(
            "/api/workspaces/{workspace_id}/files/content",
            post(crate::http::workspaces::workspace_file_content),
        )
        .route(
            "/api/workspaces/{workspace_id}/files/blob",
            get(crate::http::workspaces::workspace_file_blob),
        )
        .route(
            "/api/workspaces/{workspace_id}/files/download",
            get(crate::http::workspaces::workspace_file_download),
        )
        .route(
            "/api/workspaces/{workspace_id}/files/save",
            post(crate::http::workspaces::save_workspace_file),
        )
        .route(
            "/api/workspaces/{workspace_id}/files/delete",
            post(crate::http::workspaces::delete_workspace_file),
        )
        .route(
            "/api/workspaces/{workspace_id}/files/rename",
            post(crate::http::workspaces::rename_workspace_file),
        )
        .route(
            "/api/workspaces/{workspace_id}/logo",
            get(crate::http::workspaces::workspace_logo)
                .post(crate::http::workspaces::save_workspace_logo)
                .delete(crate::http::workspaces::clear_workspace_logo)
                .layer(DefaultBodyLimit::max(WORKSPACE_LOGO_BODY_LIMIT_BYTES)),
        )
        .route(
            "/api/workspaces/{workspace_id}/logo/thumbnail",
            get(crate::http::workspaces::workspace_logo_thumbnail),
        )
        .route(
            "/api/file-picker/roots",
            post(crate::http::file_picker::file_picker_roots),
        )
        .route(
            "/api/file-picker/list",
            post(crate::http::file_picker::file_picker_list),
        )
        .route(
            "/api/file-picker/read-files",
            post(crate::http::file_picker::file_picker_read_files)
                .layer(DefaultBodyLimit::max(CHAT_ATTACHMENT_BODY_LIMIT_BYTES)),
        )
        .route(
            "/api/native/install-ripgrep",
            post(crate::http::workspaces::install_ripgrep),
        )
        .route("/api/settings", get(crate::http::settings::settings))
        .route(
            "/api/update/status",
            get(crate::http::update::update_status),
        )
        .route("/api/update/check", post(crate::http::update::check_update))
        .route(
            "/api/update/settings",
            post(crate::http::update::save_update_settings),
        )
        .route(
            "/api/update/install",
            post(crate::http::update::install_update),
        )
        .route(
            "/api/skill-store/hot",
            get(crate::http::skill_store::skill_store_hot),
        )
        .route(
            "/api/skill-store/browse",
            get(crate::http::skill_store::skill_store_browse),
        )
        .route(
            "/api/skill-store/search",
            get(crate::http::skill_store::skill_store_search),
        )
        .route(
            "/api/skill-store/skills/{skill_id}",
            get(crate::http::skill_store::skill_store_detail),
        )
        .route(
            "/api/skill-store/install",
            post(crate::http::skill_store::skill_store_install),
        )
        .route(
            "/api/skill-store/import-preview",
            post(crate::http::skill_store::skill_store_import_preview),
        )
        .route(
            "/api/skill-store/update",
            post(crate::http::skill_store::skill_store_update),
        )
        .route(
            "/api/skill-store/update-all",
            post(crate::http::skill_store::skill_store_update_all),
        )
        .route(
            "/api/skill-store/translate",
            post(crate::http::skill_store::skill_store_translate),
        )
        .route(
            "/api/settings/general",
            post(crate::http::settings::save_general_settings),
        )
        .route(
            "/api/settings/web-search",
            post(crate::http::settings::save_web_search_settings),
        )
        .route(
            "/api/settings/memory",
            post(crate::http::settings::save_memory_settings),
        )
        .route(
            "/api/settings/spec",
            post(crate::http::settings::save_spec_settings),
        )
        .route(
            "/api/settings/spec/jobs",
            get(crate::http::spec::settings_workspace_spec_jobs),
        )
        .route(
            "/api/settings/plan",
            post(crate::http::settings::save_plan_settings),
        )
        .route(
            "/api/settings/prompts",
            post(crate::http::settings::save_prompt_settings),
        )
        .route(
            "/api/agent-definitions",
            get(crate::http::settings::agent_definitions),
        )
        .route(
            "/api/agent-definitions/create",
            post(crate::http::settings::create_agent_definition),
        )
        .route(
            "/api/agent-definitions/update",
            post(crate::http::settings::update_agent_definition),
        )
        .route(
            "/api/agent-definitions/delete",
            post(crate::http::settings::delete_agent_definition),
        )
        .route("/api/memory", get(crate::http::memory::memory_list))
        .route(
            "/api/memory/manual",
            post(crate::http::memory::create_manual_memory),
        )
        .route(
            "/api/memory/status",
            post(crate::http::memory::update_memory_status),
        )
        .route(
            "/api/memory/enabled",
            post(crate::http::memory::update_memory_enabled),
        )
        .route("/api/memory/edit", post(crate::http::memory::edit_memory))
        .route(
            "/api/memory/forget",
            post(crate::http::memory::forget_memory),
        )
        .route(
            "/api/memory/clear",
            post(crate::http::memory::clear_filtered_memories),
        )
        .route(
            "/api/memory/promote",
            post(crate::http::memory::promote_memory),
        )
        .route(
            "/api/memory/sources",
            get(crate::http::memory::memory_sources),
        )
        .route(
            "/api/memory/extraction/retry",
            post(crate::http::memory::retry_memory_extraction_job),
        )
        .route(
            "/api/memory/extraction/skip",
            post(crate::http::memory::skip_memory_extraction_job),
        )
        .route(
            "/api/memory/dream/run",
            post(crate::http::memory::run_memory_dream),
        )
        .route(
            "/api/memory/dream/jobs",
            get(crate::http::memory::memory_dream_jobs),
        )
        .route(
            "/api/memory/dream/jobs/{job_id}",
            get(crate::http::memory::memory_dream_job),
        )
        .route(
            "/api/memory/dream/jobs/{job_id}/changes",
            get(crate::http::memory::memory_dream_changes),
        )
        .route("/api/hooks", get(crate::http::hooks::hooks_settings))
        .route(
            "/api/hooks/global",
            post(crate::http::hooks::save_global_hooks),
        )
        .route(
            "/api/hooks/workspace",
            post(crate::http::hooks::save_workspace_hooks),
        )
        .route(
            "/api/hooks/import-claude",
            post(crate::http::hooks::import_claude_hooks),
        )
        .route("/api/hooks/test", post(crate::http::hooks::test_hooks))
        .route(
            "/api/scheduled-tasks",
            get(crate::http::scheduled_tasks::scheduled_tasks),
        )
        .route(
            "/api/scheduled-tasks/preview-next-run",
            post(crate::http::scheduled_tasks::preview_scheduled_task_next_run),
        )
        .route(
            "/api/workspaces/{workspace_id}/scheduled-tasks",
            post(crate::http::scheduled_tasks::create_scheduled_task),
        )
        .route(
            "/api/workspaces/{workspace_id}/scheduled-tasks/{task_id}",
            get(crate::http::scheduled_tasks::scheduled_task)
                .patch(crate::http::scheduled_tasks::update_scheduled_task)
                .delete(crate::http::scheduled_tasks::delete_scheduled_task),
        )
        .route(
            "/api/workspaces/{workspace_id}/scheduled-tasks/{task_id}/pause",
            post(crate::http::scheduled_tasks::pause_scheduled_task),
        )
        .route(
            "/api/workspaces/{workspace_id}/scheduled-tasks/{task_id}/resume",
            post(crate::http::scheduled_tasks::resume_scheduled_task),
        )
        .route(
            "/api/workspaces/{workspace_id}/scheduled-tasks/{task_id}/archive",
            post(crate::http::scheduled_tasks::archive_scheduled_task),
        )
        .route(
            "/api/workspaces/{workspace_id}/scheduled-tasks/{task_id}/duplicate",
            post(crate::http::scheduled_tasks::duplicate_scheduled_task),
        )
        .route(
            "/api/workspaces/{workspace_id}/scheduled-tasks/{task_id}/run-now",
            post(crate::http::scheduled_tasks::run_scheduled_task_now),
        )
        .route(
            "/api/workspaces/{workspace_id}/scheduled-tasks/{task_id}/runs",
            get(crate::http::scheduled_tasks::scheduled_task_runs),
        )
        .route(
            "/api/workspaces/{workspace_id}/scheduled-task-runs/{scheduled_run_id}",
            get(crate::http::scheduled_tasks::scheduled_task_run),
        )
        .route(
            "/api/workspaces/{workspace_id}/scheduled-task-runs/{scheduled_run_id}/cancel",
            post(crate::http::scheduled_tasks::cancel_scheduled_task_run),
        )
        .route(
            "/api/workspaces/{workspace_id}/spec",
            get(crate::http::spec::workspace_spec).put(crate::http::spec::save_workspace_spec),
        )
        .route(
            "/api/workspaces/{workspace_id}/spec/settings",
            put(crate::http::spec::save_workspace_spec_settings),
        )
        .route(
            "/api/workspaces/{workspace_id}/spec/generate",
            post(crate::http::spec::generate_workspace_spec),
        )
        .route(
            "/api/workspaces/{workspace_id}/spec/jobs",
            get(crate::http::spec::workspace_spec_jobs),
        )
        .route(
            "/api/workspaces/{workspace_id}/spec/jobs/{job_id}",
            delete(crate::http::spec::delete_failed_workspace_spec_job),
        )
        .route(
            "/api/workspaces/{workspace_id}/spec/jobs/{job_id}/retry",
            post(crate::http::spec::retry_workspace_spec_job),
        )
        .route(
            "/api/workspaces/{workspace_id}/plans",
            get(crate::http::plans::plans).post(crate::http::plans::create_plan),
        )
        .route(
            "/api/workspaces/{workspace_id}/plans/auto-run",
            get(crate::http::plans::plan_auto_run).put(crate::http::plans::set_plan_auto_run),
        )
        .route(
            "/api/workspaces/{workspace_id}/plans/order",
            post(crate::http::plans::save_plan_order),
        )
        .route(
            "/api/workspaces/{workspace_id}/plans/worktrees/audit",
            get(crate::http::plans::plan_worktree_audit),
        )
        .route(
            "/api/workspaces/{workspace_id}/plans/worktrees/cleanup",
            post(crate::http::plans::cleanup_plan_worktree),
        )
        .route(
            "/api/workspaces/{workspace_id}/plans/{plan_id}",
            patch(crate::http::plans::update_plan).delete(crate::http::plans::delete_plan),
        )
        .route(
            "/api/workspaces/{workspace_id}/plans/{plan_id}/action",
            post(crate::http::plans::plan_action),
        )
        .route(
            "/api/workspaces/{workspace_id}/plans/{plan_id}/phases/{phase_id}/retry",
            post(crate::http::plans::retry_plan_phase),
        )
        .route(
            "/api/workspaces/{workspace_id}/plans/{plan_id}/steps/{step_id}/action",
            post(crate::http::plans::plan_step_action),
        )
        .route(
            "/api/workspaces/{workspace_id}/hooks/runs",
            get(crate::http::hooks::hook_runs),
        )
        .route(
            "/api/workspaces/{workspace_id}/hooks/runs/{hook_run_id}",
            get(crate::http::hooks::hook_run_detail),
        )
        .route(
            "/api/providers/manual",
            post(crate::http::settings::save_manual_provider),
        )
        .route(
            "/api/providers/reveal-api-key",
            post(crate::http::settings::reveal_provider_api_key),
        )
        .route(
            "/api/providers/delete",
            post(crate::http::settings::delete_provider),
        )
        .route(
            "/api/providers/test",
            post(crate::http::settings::test_provider),
        )
        .route(
            "/api/providers/models",
            post(crate::http::settings::provider_models),
        )
        .route(
            "/api/providers/models/refresh",
            post(crate::http::settings::refresh_provider_models),
        )
        .route(
            "/api/model-metadata",
            get(crate::http::settings::model_metadata),
        )
        .route(
            "/api/model-metadata/refresh",
            post(crate::http::settings::refresh_model_metadata),
        )
        .route(
            "/api/models/manual",
            post(crate::http::settings::save_manual_model),
        )
        .route(
            "/api/models/route",
            post(crate::http::settings::update_model_route),
        )
        .route("/api/models/test", post(crate::http::settings::test_model))
        .route(
            "/api/models/delete",
            post(crate::http::settings::delete_model),
        )
        .route(
            "/api/mcp/servers/manual",
            post(crate::http::settings::save_mcp_server),
        )
        .route(
            "/api/mcp/servers/delete",
            post(crate::http::settings::delete_mcp_server),
        )
        .route(
            "/api/skills/manual",
            post(crate::http::settings::save_skills),
        )
        .route(
            "/api/skills/refresh",
            post(crate::http::settings::refresh_skills),
        )
        .route(
            "/api/skills/delete",
            post(crate::http::settings::delete_skill),
        )
        .route("/api/ai-statistics", get(crate::http::chat::ai_statistics))
        .route(
            "/api/workspaces/{workspace_id}/chats/{chat_id}/agent-team/enable",
            post(crate::http::agents::enable_agent_team),
        )
        .route(
            "/api/workspaces/{workspace_id}/chats/{chat_id}/agent-team",
            get(crate::http::agents::agent_team_snapshot),
        )
        .route(
            "/api/workspaces/{workspace_id}/agent-team/instances/{instance_id}/transcript",
            get(crate::http::agents::agent_instance_transcript),
        )
        .route(
            "/api/workspaces/{workspace_id}/chats/{chat_id}/agent-team/instances/create",
            post(crate::http::agents::create_agent_instances),
        )
        .route(
            "/api/workspaces/{workspace_id}/chats/{chat_id}/agent-team/action",
            post(crate::http::agents::agent_runtime_action),
        )
        .route(
            "/api/workspaces/{workspace_id}/agent-tasks/{task_id}/action",
            post(crate::http::agents::agent_task_action),
        )
        .route(
            "/api/workspaces/{workspace_id}/chats/{chat_id}/messages/{message_id}/edit",
            post(crate::http::chat::edit_chat_user_message)
                .layer(DefaultBodyLimit::max(CHAT_ATTACHMENT_BODY_LIMIT_BYTES)),
        )
        .route(
            "/api/workspaces/{workspace_id}/chat/queue",
            post(crate::http::chat::queue_chat_message)
                .layer(DefaultBodyLimit::max(CHAT_ATTACHMENT_BODY_LIMIT_BYTES)),
        )
        .route(
            "/api/workspaces/{workspace_id}/chat/stream",
            post(crate::http::chat::stream_chat_response)
                .layer(DefaultBodyLimit::max(CHAT_ATTACHMENT_BODY_LIMIT_BYTES)),
        )
        .route(
            "/api/workspaces/{workspace_id}/chat/runs/{run_id}/stream",
            get(crate::http::chat::subscribe_chat_run),
        )
        .route(
            "/api/workspaces/{workspace_id}/chat/runs/{run_id}/cancel",
            post(crate::http::chat::cancel_chat_run),
        )
        .route(
            "/api/workspaces/{workspace_id}/chat/guidance",
            post(crate::http::chat::add_chat_guidance)
                .layer(DefaultBodyLimit::max(CHAT_ATTACHMENT_BODY_LIMIT_BYTES)),
        )
        .route(
            "/api/workspaces/{workspace_id}/context-usage",
            post(crate::http::chat::context_usage)
                .layer(DefaultBodyLimit::max(CHAT_ATTACHMENT_BODY_LIMIT_BYTES)),
        )
        .route(
            "/api/chat/questions/pending",
            get(crate::http::chat::pending_questions),
        )
        .route(
            "/api/chat/questions/{question_id}/answer",
            post(crate::http::chat::answer_question),
        )
        .route(
            "/api/workspaces/{workspace_id}/ai-statistics/{request_id}",
            get(crate::http::chat::ai_statistics_detail),
        )
        .route(
            "/api/workspaces/{workspace_id}/chats/{chat_id}/messages",
            get(crate::http::chat::chat_messages),
        )
        .route(
            "/api/workspaces/{workspace_id}/chats/{chat_id}/todo-graph",
            get(crate::http::chat::chat_todo_graph),
        )
        .route(
            "/api/workspaces/{workspace_id}/chats/{chat_id}/statistics",
            get(crate::http::chat::chat_statistics),
        )
        .route(
            "/api/workspaces/{workspace_id}/chats/{chat_id}/delete",
            post(crate::http::chat::delete_chat),
        )
        .route(
            "/api/workspaces/{workspace_id}/git/status",
            get(crate::http::git::git_status),
        )
        .route(
            "/api/workspaces/{workspace_id}/git/diff",
            get(crate::http::git::git_diff),
        )
        .route(
            "/api/workspaces/{workspace_id}/git/stage",
            post(crate::http::git::stage_git_file),
        )
        .route(
            "/api/workspaces/{workspace_id}/git/unstage",
            post(crate::http::git::unstage_git_file),
        )
        .route(
            "/api/workspaces/{workspace_id}/git/discard",
            post(crate::http::git::discard_git_file),
        )
        .route(
            "/api/workspaces/{workspace_id}/git/commit",
            post(crate::http::git::commit_staged_changes),
        )
        .route(
            "/api/workspaces/{workspace_id}/git/commit-message",
            post(crate::http::git::generate_commit_message),
        )
        .route(
            "/api/workspaces/{workspace_id}/git/branches",
            get(crate::http::git::git_branches),
        )
        .route(
            "/api/workspaces/{workspace_id}/git/branches/switch",
            post(crate::http::git::switch_git_branch),
        )
        .route(
            "/api/workspaces/{workspace_id}/git/branches/create",
            post(crate::http::git::create_git_branch),
        )
        .route(
            "/api/workspaces/{workspace_id}/terminal/session",
            post(crate::http::terminal::create_terminal_session),
        )
        .route(
            "/api/workspaces/{workspace_id}/terminal/{session_id}/ws",
            get(crate::http::terminal::terminal_socket),
        )
        .route(
            "/api/workspaces/{workspace_id}/preview/sessions",
            post(crate::runtime::create_preview_session),
        )
        .route(
            "/api/workspaces/{workspace_id}/preview/sessions/{token}",
            delete(crate::runtime::release_preview_session),
        )
        .fallback(static_asset)
        .layer(middleware::from_fn_with_state(
            state.clone(),
            remote_workspace_proxy_middleware,
        ))
        .layer(middleware::from_fn_with_state(
            auth_state,
            crate::http::auth::require_auth,
        ))
        // Preview host routing must run outside require_auth so capability tokens
        // alone authorize reads, and before SPA fallback for non-API paths.
        .layer(middleware::from_fn_with_state(
            state.clone(),
            crate::runtime::preview_host_middleware,
        ))
        .layer(middleware::from_fn(log_http_request))
        .with_state(state)
}

/// Proxies workspace-scoped API requests to the remote sidecar when the target
/// workspace is remote.
///
/// Only proxies routes that operate on workspace-local resources: files, git,
/// terminal, spec, plans, code graph, chat/runtime state, agents, schedules, and
/// chat statistics. Settings and provider secrets stay local.
///
/// AI Statistics list (`/api/ai-statistics`) and workspace detail
/// (`/api/workspaces/{id}/ai-statistics/{request_id}`) stay on the main process:
/// real `provider_request_v1` / `provider_final_response_v1` wire lives only in
/// the profile remote-workspace-audit mirror. Sidecar structured mirrors keep
/// detail columns NULL and must not be treated as the dump source of truth.
///
/// ponytail: v1 still buffers request bodies and non-SSE responses in memory.
/// Chat streams are proxied as streams because reverse proxies otherwise wait for
/// the sidecar SSE body to finish and can 504 long-running chats.
async fn remote_workspace_proxy_middleware(
    State(state): State<AppState>,
    request: Request,
    next: Next,
) -> Response {
    let path = request.uri().path();

    let route_method =
        crate::http::workspace_route_contract::WorkspaceRouteMethod::from_http_method(
            request.method().as_str(),
            is_websocket_request(&request),
        );
    let (workspace_id, suffix, request) =
        match route_method.and_then(|method| proxy_workspace_route_path_for_method(path, method)) {
            Some(suffix) => {
                let Some(workspace_id) = extract_workspace_id_from_path(path) else {
                    return next.run(request).await;
                };
                (workspace_id, suffix.to_string(), request)
            }
            None => {
                let (target, request) = match proxy_global_workspace_route(request).await {
                    Ok(result) => result,
                    Err(response) => return response,
                };
                match target {
                    Some((workspace_id, suffix)) => (workspace_id, suffix, request),
                    None => return next.run(request).await,
                }
            }
        };

    if let Err(error) =
        crate::remote_workspace::ensure_remote_workspace_connected(&state, &workspace_id).await
    {
        return error.into_response();
    }

    // Local workspaces fall through. Remote workspaces must have a connected
    // sidecar target; a disconnected or invalid target is never allowed to
    // reach the local workspace handler.
    let (base, token) = match crate::remote_workspace::sidecar_proxy_target(&state, &workspace_id) {
        Ok(crate::remote_workspace::SidecarProxyTarget::Connected { base, token }) => (base, token),
        Ok(crate::remote_workspace::SidecarProxyTarget::Local) => return next.run(request).await,
        Ok(crate::remote_workspace::SidecarProxyTarget::Disconnected) => {
            return crate::ApiError::bad_gateway(format!(
                "remote workspace sidecar is not connected: {workspace_id}"
            ))
            .into_response();
        }
        Err(error) => return error.into_response(),
    };

    // Build the proxied URL - map /api/workspaces/{id}/foo to /api/remote/workspace/foo
    let query = request
        .uri()
        .query()
        .map(|q| format!("?{q}"))
        .unwrap_or_default();
    let proxy_url = format!(
        "{}/api/remote/workspace/{suffix}{query}",
        base.trim_end_matches('/')
    );

    if is_websocket_request(&request) {
        return proxy_websocket_upgrade(request, proxy_url, token).await;
    }

    let is_code_graph_request = suffix.starts_with("code-graph") || suffix.starts_with("graph");

    // Read the request body
    let method = request.method().clone();
    let forwarded_headers = request.headers().clone();
    let mut bytes = match axum::body::to_bytes(request.into_body(), 10 * 1024 * 1024).await {
        Ok(b) => b,
        Err(_) => {
            return Response::builder()
                .status(StatusCode::BAD_REQUEST)
                .body(Body::from("failed to read request body for proxy"))
                .expect("valid response");
        }
    };
    if suffix == "context-usage" {
        bytes = match normalize_remote_context_usage_request(&state, &bytes) {
            Ok(body) => body.into(),
            Err(error) => return error.into_response(),
        };
    }

    // Proxy to sidecar
    let client = reqwest::Client::new();
    let mut req = client
        .request(method, &proxy_url)
        .bearer_auth(&token)
        .body(bytes.to_vec());
    for name in [
        header::CONTENT_TYPE.as_str(),
        header::ACCEPT.as_str(),
        header::ACCEPT_LANGUAGE.as_str(),
        header::CACHE_CONTROL.as_str(),
    ] {
        if let Some(value) = forwarded_headers.get(name) {
            req = req.header(name, value.clone());
        }
    }
    if is_code_graph_request {
        req = req.header("x-foco-ensure-code-graph", "1");
    }
    match req.send().await {
        Ok(resp) => {
            let status = resp.status();
            let headers = resp.headers().clone();
            let is_sse_response = response_is_event_stream(&headers);
            let mut builder = Response::builder().status(status);
            for (name, value) in headers.iter() {
                if should_skip_proxy_response_header(name) {
                    continue;
                }
                builder = builder.header(name, value);
            }
            if is_proxy_sse_path(&suffix) || is_sse_response {
                return builder
                    .body(Body::from_stream(resp.bytes_stream()))
                    .expect("valid streaming proxy response");
            }
            let body_bytes = match resp.bytes().await {
                Ok(b) => b,
                Err(_) => {
                    return Response::builder()
                        .status(StatusCode::BAD_GATEWAY)
                        .body(Body::from("failed to read sidecar proxy response"))
                        .expect("valid response");
                }
            };
            builder
                .body(Body::from(body_bytes.to_vec()))
                .expect("valid proxy response")
        }
        Err(source) => Response::builder()
            .status(StatusCode::BAD_GATEWAY)
            .body(Body::from(format!("sidecar proxy failed: {source}")))
            .expect("valid error response"),
    }
}

/// Resolve the few workspace-scoped APIs whose public URL predates remote
/// workspaces and therefore carries `workspaceId` in a query string or JSON
/// body instead of in `/api/workspaces/{id}/...`.
///
/// Global Memory intentionally remains local: only workspace/chat scopes are
/// eligible for sidecar routing. Rebuilding the request after inspecting the
/// JSON body keeps downstream handlers and proxy forwarding byte-for-byte
/// compatible with the normal route path.
async fn proxy_global_workspace_route(
    request: Request,
) -> Result<(Option<(String, String)>, Request), Response> {
    use crate::http::workspace_route_contract::{
        GlobalWorkspaceIdSource, WorkspaceRouteMethod, global_workspace_route_contract,
        global_workspace_sidecar_suffix,
    };

    let path = request.uri().path();
    let Some(method) = WorkspaceRouteMethod::from_http_method(request.method().as_str(), false)
    else {
        return Ok((None, request));
    };
    let Some(contract) = global_workspace_route_contract(path, method) else {
        return Ok((None, request));
    };
    let Some(sidecar_suffix) = global_workspace_sidecar_suffix(contract, path) else {
        return Ok((None, request));
    };

    match contract.workspace_id_source {
        GlobalWorkspaceIdSource::Query => {
            let query = request
                .uri()
                .query()
                .map(parse_workspace_route_query)
                .unwrap_or_default();
            let Some(workspace_id) = query.get("workspaceId").cloned() else {
                return Ok((None, request));
            };
            if contract.global_memory_scope_stays_local
                && query
                    .get("scope")
                    .is_some_and(|scope| scope.trim().eq_ignore_ascii_case("global"))
            {
                return Ok((None, request));
            }
            Ok((Some((workspace_id, sidecar_suffix.clone())), request))
        }
        GlobalWorkspaceIdSource::JsonBody => {
            let (parts, body) = request.into_parts();
            let bytes = axum::body::to_bytes(body, 10 * 1024 * 1024)
                .await
                .map_err(|_| {
                    Response::builder()
                        .status(StatusCode::BAD_REQUEST)
                        .body(Body::from(
                            "failed to read request body for remote workspace routing",
                        ))
                        .expect("valid response")
                })?;
            let payload: Value = serde_json::from_slice(&bytes).map_err(|_| {
                Response::builder()
                    .status(StatusCode::BAD_REQUEST)
                    .body(Body::from(
                        "invalid JSON request body for remote workspace routing",
                    ))
                    .expect("valid response")
            })?;
            let request = Request::from_parts(parts, Body::from(bytes));
            let Some(workspace_id) = payload
                .get("workspaceId")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|workspace_id| !workspace_id.is_empty())
                .map(ToOwned::to_owned)
            else {
                return Ok((None, request));
            };
            if contract.global_memory_scope_stays_local
                && payload
                    .get("scope")
                    .and_then(Value::as_str)
                    .is_some_and(|scope| scope.trim().eq_ignore_ascii_case("global"))
            {
                return Ok((None, request));
            }
            Ok((Some((workspace_id, sidecar_suffix.clone())), request))
        }
    }
}

fn parse_workspace_route_query(query: &str) -> std::collections::HashMap<String, String> {
    query
        .split('&')
        .filter_map(|entry| entry.split_once('='))
        .map(|(key, value)| (key.to_string(), value.to_string()))
        .collect()
}

fn normalize_remote_context_usage_request(
    state: &AppState,
    body: &[u8],
) -> Result<Vec<u8>, crate::ApiError> {
    let mut payload = serde_json::from_slice::<Value>(body).map_err(|source| {
        crate::ApiError::bad_request(format!("invalid context usage request JSON: {source}"))
    })?;
    let model_id = payload
        .get("modelId")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|model_id| !model_id.is_empty())
        .ok_or_else(|| crate::ApiError::bad_request("model id must not be empty"))?
        .to_string();
    let config = crate::config_snapshot(state)?;
    let (_, provider) = config
        .resolve_active_model_provider(&model_id)
        .map_err(|error| crate::ApiError::bad_request(error.to_string()))?;
    let provider_id = provider.id.clone();
    let payload = payload.as_object_mut().ok_or_else(|| {
        crate::ApiError::bad_request("context usage request must be a JSON object")
    })?;
    payload.insert("providerId".to_string(), Value::String(provider_id));
    serde_json::to_vec(&payload).map_err(|source| {
        crate::ApiError::internal(format!(
            "failed to normalize context usage request: {source}"
        ))
    })
}

fn is_proxy_sse_path(suffix: &str) -> bool {
    suffix == "chat/stream"
        || (suffix.starts_with("chat/runs/") && suffix.ends_with("/stream"))
        || (suffix.starts_with("chat/runs/") && suffix.ends_with("/stream-events"))
}

fn response_is_event_stream(headers: &HeaderMap) -> bool {
    headers
        .get(header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| {
            value
                .split(';')
                .next()
                .is_some_and(|mime| mime.trim().eq_ignore_ascii_case("text/event-stream"))
        })
}

fn should_skip_proxy_response_header(name: &header::HeaderName) -> bool {
    name == header::CONTENT_LENGTH
        || name == header::TRANSFER_ENCODING
        || name == header::CONNECTION
}

fn is_websocket_request(request: &Request) -> bool {
    request
        .headers()
        .get(header::UPGRADE)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value.eq_ignore_ascii_case("websocket"))
}

async fn proxy_websocket_upgrade(request: Request, proxy_url: String, token: String) -> Response {
    let mut parts = request.into_parts().0;
    match WebSocketUpgrade::from_request_parts(&mut parts, &()).await {
        Ok(upgrade) => upgrade
            .on_upgrade(move |client_socket| async move {
                crate::remote_workspace::proxy_websocket_to_sidecar(
                    client_socket,
                    proxy_url,
                    token,
                )
                .await;
            })
            .into_response(),
        Err(rejection) => rejection.into_response(),
    }
}

/// If the request path matches a proxied workspace route, return the path suffix
/// after the workspace_id segment.  Returns None for routes that should stay local.
///
/// Intentionally excludes `ai-statistics`: dump detail is served from the main
/// process audit mirror (`workspace_audit_path` → remote-workspace-audit), not
/// the sidecar mirror (which always reports detail unavailable).
#[cfg(test)]
/// Backward-compatible GET helper used by route tests. Runtime proxying uses
/// [`proxy_workspace_route_path_for_method`] so a shared prefix alone cannot
/// route an undeclared method to the remote sidecar.
pub(crate) fn proxy_workspace_route_path(path: &str) -> Option<&str> {
    proxy_workspace_route_path_for_method(
        path,
        crate::http::workspace_route_contract::WorkspaceRouteMethod::Get,
    )
}

pub(crate) fn proxy_workspace_route_path_for_method(
    path: &str,
    method: crate::http::workspace_route_contract::WorkspaceRouteMethod,
) -> Option<&str> {
    let rest = path.strip_prefix("/api/workspaces/")?;
    let after_id = rest.split_once('/')?.1;
    if crate::http::workspace_route_contract::is_sidecar_workspace_route(path, method) {
        Some(after_id)
    } else {
        None
    }
}

/// Extract workspace_id from /api/workspaces/{workspace_id}/... paths.
fn extract_workspace_id_from_path(path: &str) -> Option<String> {
    let rest = path.strip_prefix("/api/workspaces/")?;
    let workspace_id = rest.split('/').next()?;
    if workspace_id.is_empty() || workspace_id.contains('?') {
        return None;
    }
    Some(workspace_id.to_string())
}

async fn log_http_request(request: axum::extract::Request, next: middleware::Next) -> Response {
    let started_at = Instant::now();
    let method = request.method().clone();
    let path = request.uri().path().to_string();
    let host = request
        .headers()
        .get(header::HOST)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("");
    let host_for_log = if crate::runtime::request_is_preview_host(request.headers()) {
        crate::runtime::redact_preview_host_for_log(host)
    } else {
        host.to_string()
    };
    tracing::info!(%method, %path, host = %host_for_log, "HTTP request started");
    let response = next.run(request).await;
    tracing::info!(
        %method,
        %path,
        host = %host_for_log,
        status = response.status().as_u16(),
        elapsed_ms = started_at.elapsed().as_millis() as u64,
        "HTTP request completed"
    );
    response
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn global_hook_settings_route_uses_the_workspace_id_query_parameter() {
        let request = Request::builder()
            .uri("/api/hooks?workspaceId=remote-workspace")
            .body(Body::empty())
            .expect("valid request");

        let (target, _) = proxy_global_workspace_route(request)
            .await
            .expect("route classification");

        assert_eq!(
            target,
            Some(("remote-workspace".to_string(), "hooks/settings".to_string(),))
        );
    }

    #[tokio::test]
    async fn workspace_memory_mutation_routes_to_the_sidecar() {
        let request = Request::builder()
            .method(axum::http::Method::POST)
            .uri("/api/memory/manual")
            .body(Body::from(
                r#"{"workspaceId":"remote-workspace","scope":"workspace"}"#,
            ))
            .expect("valid request");

        let (target, _) = proxy_global_workspace_route(request)
            .await
            .expect("route classification");

        assert_eq!(
            target,
            Some(("remote-workspace".to_string(), "memory/manual".to_string(),))
        );
    }

    #[tokio::test]
    async fn workspace_memory_dream_changes_route_to_the_sidecar_with_the_job_id() {
        let request = Request::builder()
            .uri("/api/memory/dream/jobs/dream-job-42/changes?workspaceId=remote-workspace")
            .body(Body::empty())
            .expect("valid request");

        let (target, _) = proxy_global_workspace_route(request)
            .await
            .expect("route classification");

        assert_eq!(
            target,
            Some((
                "remote-workspace".to_string(),
                "memory/dream/jobs/dream-job-42/changes".to_string(),
            ))
        );
    }

    #[tokio::test]
    async fn workspace_memory_dream_detail_routes_to_the_sidecar_with_the_job_id() {
        let request = Request::builder()
            .uri("/api/memory/dream/jobs/dream-job-42?workspaceId=remote-workspace")
            .body(Body::empty())
            .expect("valid request");

        let (target, _) = proxy_global_workspace_route(request)
            .await
            .expect("route classification");

        assert_eq!(
            target,
            Some((
                "remote-workspace".to_string(),
                "memory/dream/jobs/dream-job-42".to_string(),
            ))
        );
    }

    #[tokio::test]
    async fn workspace_memory_dream_routes_require_a_declared_method_and_template() {
        let wrong_method = Request::builder()
            .method(axum::http::Method::POST)
            .uri("/api/memory/dream/jobs/dream-job-42/changes?workspaceId=remote-workspace")
            .body(Body::empty())
            .expect("valid request");
        let (target, _) = proxy_global_workspace_route(wrong_method)
            .await
            .expect("route classification");
        assert_eq!(target, None);

        let wrong_template = Request::builder()
            .uri("/api/memory/dream/jobs/dream-job-42/changes/unexpected?workspaceId=remote-workspace")
            .body(Body::empty())
            .expect("valid request");
        let (target, _) = proxy_global_workspace_route(wrong_template)
            .await
            .expect("route classification");
        assert_eq!(target, None);
    }

    #[tokio::test]
    async fn global_memory_dream_detail_stays_on_the_main_process() {
        let request = Request::builder()
            .uri("/api/memory/dream/jobs/dream-job-42?workspaceId=remote-workspace&scope=global")
            .body(Body::empty())
            .expect("valid request");

        let (target, _) = proxy_global_workspace_route(request)
            .await
            .expect("route classification");

        assert_eq!(target, None);
    }

    #[tokio::test]
    async fn global_memory_mutation_remains_on_the_main_process() {
        let request = Request::builder()
            .method(axum::http::Method::POST)
            .uri("/api/memory/manual")
            .body(Body::from(
                r#"{"workspaceId":"remote-workspace","scope":"global"}"#,
            ))
            .expect("valid request");

        let (target, _) = proxy_global_workspace_route(request)
            .await
            .expect("route classification");

        assert_eq!(target, None);
    }
}
