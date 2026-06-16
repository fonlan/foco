use std::path::Path;

use foco_store::workspace::{
    TodoGraphFilter, TodoGraphRecord, TodoGraphTask, TodoGraphTaskPatch, WorkspaceDatabase,
};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::{
    DEFAULT_TODO_GRAPH_TIMEOUT_MS,
    errors::{ToolRuntimeError, tool_timeout_ms},
    parse_arguments,
};

pub(crate) fn create_todo_graph(
    workspace_path: &Path,
    chat_id: Option<&str>,
    arguments: Value,
) -> Result<Value, ToolRuntimeError> {
    let request: CreateTodoGraphInput = parse_arguments(arguments)?;
    let timeout_ms = tool_timeout_ms(request.timeout_ms, DEFAULT_TODO_GRAPH_TIMEOUT_MS)?;
    let chat_id = required_chat_id(chat_id)?;
    let mut database = open_todo_graph_database(workspace_path)?;
    let graph = database.upsert_todo_graph(
        chat_id,
        request
            .tasks
            .into_iter()
            .map(todo_graph_task_from_input)
            .collect(),
    )?;

    Ok(todo_graph_json(graph, timeout_ms))
}

pub(crate) fn update_todo_graph(
    workspace_path: &Path,
    chat_id: Option<&str>,
    arguments: Value,
) -> Result<Value, ToolRuntimeError> {
    let request: UpdateTodoGraphInput = parse_arguments(arguments)?;
    let timeout_ms = tool_timeout_ms(request.timeout_ms, DEFAULT_TODO_GRAPH_TIMEOUT_MS)?;
    let chat_id = required_chat_id(chat_id)?;
    let patch = TodoGraphTaskPatch {
        title: request.patch.title,
        status: request.patch.status,
        depends_on: request.patch.depends_on,
        acceptance: request.patch.acceptance,
        summary: request.patch.summary,
        subtasks: request
            .patch
            .subtasks
            .map(|tasks| tasks.into_iter().map(todo_graph_task_from_input).collect()),
    };
    let mut database = open_todo_graph_database(workspace_path)?;
    let graph = database.update_todo_graph_task(chat_id, &request.task_id, patch)?;

    Ok(todo_graph_json(graph, timeout_ms))
}

pub(crate) fn get_todo_graph(
    workspace_path: &Path,
    chat_id: Option<&str>,
    arguments: Value,
) -> Result<Value, ToolRuntimeError> {
    let request: GetTodoGraphInput = parse_arguments(arguments)?;
    let timeout_ms = tool_timeout_ms(request.timeout_ms, DEFAULT_TODO_GRAPH_TIMEOUT_MS)?;
    let chat_id = required_chat_id(chat_id)?;
    let status = request
        .status
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let task_id = request
        .task_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let database = open_todo_graph_database(workspace_path)?;
    let graph = database.filtered_todo_graph(
        chat_id,
        TodoGraphFilter {
            status,
            task_id,
            include_subtasks: request.include_subtasks,
        },
    )?;

    match graph {
        Some(graph) => Ok(todo_graph_json(graph, timeout_ms)),
        None => Ok(json!({
            "chatId": chat_id,
            "tasks": [],
            "exists": false,
            "createdAt": null,
            "updatedAt": null,
            "updatedTask": null,
            "timeoutMs": timeout_ms
        })),
    }
}

fn open_todo_graph_database(workspace_path: &Path) -> Result<WorkspaceDatabase, ToolRuntimeError> {
    WorkspaceDatabase::open_or_create(workspace_path).map_err(ToolRuntimeError::WorkspaceDatabase)
}

fn required_chat_id(chat_id: Option<&str>) -> Result<&str, ToolRuntimeError> {
    chat_id
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            ToolRuntimeError::InvalidArguments(
                "todo graph tools require an active chat".to_string(),
            )
        })
}

fn todo_graph_json(graph: TodoGraphRecord, timeout_ms: u64) -> Value {
    json!({
        "chatId": graph.chat_id,
        "tasks": graph.tasks,
        "exists": true,
        "createdAt": graph.created_at,
        "updatedAt": graph.updated_at,
        "updatedTask": graph.updated_task,
        "timeoutMs": timeout_ms
    })
}

fn todo_graph_task_from_input(task: TodoGraphTaskInput) -> TodoGraphTask {
    let _server_generated_timestamps = (task.created_at, task.updated_at);

    TodoGraphTask {
        id: task.id,
        title: task.title,
        status: task.status,
        depends_on: task.depends_on,
        acceptance: task.acceptance,
        summary: task.summary,
        created_at: String::new(),
        updated_at: String::new(),
        subtasks: task
            .subtasks
            .into_iter()
            .map(todo_graph_task_from_input)
            .collect(),
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateTodoGraphInput {
    tasks: Vec<TodoGraphTaskInput>,
    timeout_ms: Option<u64>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct UpdateTodoGraphInput {
    task_id: String,
    patch: TodoGraphPatchInput,
    timeout_ms: Option<u64>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GetTodoGraphInput {
    status: Option<String>,
    task_id: Option<String>,
    include_subtasks: bool,
    timeout_ms: Option<u64>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct TodoGraphPatchInput {
    title: Option<String>,
    status: Option<String>,
    depends_on: Option<Vec<String>>,
    acceptance: Option<Vec<String>>,
    summary: Option<String>,
    subtasks: Option<Vec<TodoGraphTaskInput>>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct TodoGraphTaskInput {
    id: String,
    title: String,
    status: String,
    depends_on: Vec<String>,
    acceptance: Vec<String>,
    summary: String,
    created_at: Option<String>,
    updated_at: Option<String>,
    subtasks: Vec<TodoGraphTaskInput>,
}
