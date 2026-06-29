# 失败 Phase 换模型重试实现边界

## 目标

本阶段只确认边界，不改业务逻辑。目标是让后续实现失败 phase 的默认重试和换模型重试时，不覆盖旧失败记录，不污染 plan 全局模型配置，并尽量复用现有 Plan runtime、Agent dispatch 和前端模型选择控件。

## 当前路径

### 数据结构

- Plan schema 在 `store/workspace_schema.rs` 的 `MIGRATION_020` 中定义：`plans` 保存整体状态，`plan_phases` 保存当前/最近一次 phase 运行摘要，`plan_steps` 保存步骤状态。
- `PlanPhaseRecord` 当前只有单份运行字段：`implementation_chat_id`、`agent_team_id`、`agent_task_id`、`commit_id`、`merge_attempt_count`、`error_message`、`started_at`、`completed_at`。
- 已有 `agent_attempts` 表，但它是 Agent task 的尝试记录，不是 Plan phase 的 run history。Plan phase 当前只指向一个 `agent_task_id`，不会保留多次 phase retry 的历史索引。
- `llm_requests` 已保存 `provider_id`、`model_id` 和 `agent_attempt_id`，但 phase API/record 没有直接暴露本次 phase 使用的 provider/model/thinking level。

### 失败和重试流转

- phase 派发入口是 `app/plan_runtime.rs::transition_plan_action`，`start`/`resume` 会先走 `WorkspaceDatabase::transition_plan`，再走 `dispatch_plan_phase`。
- `dispatch_plan_phase` 通过 `plan_runner_model_selection` 选择模型，然后调用 `queue_chat_message_internal` 创建 chat、Agent team 和 Agent task，最后用 `attach_plan_phase_run` 写回 phase 当前运行字段。
- phase task 完成时，`sync_plan_phase_for_agent_task` 根据 `AgentTaskStatus` 调用：成功走 `complete_plan_phase_run`；失败、取消、中断走 `fail_plan_phase_run`。
- phase 完成后的 worktree commit 失败会直接 `fail_plan_phase_by_id`。共享 workspace merge 失败另有 `try_begin_plan_phase_merge_attempt` 和 `attach_plan_phase_merge_run`，目前只用 `merge_attempt_count` 限制一次自动 merge 尝试。
- 当前失败 phase 的前端重试入口在 `web/features/context/ContextPanel.tsx`：`isRetryablePlanPhase` 对 `failed` 或 `running` 且有 `agentTaskId` 的 phase 显示 retry 按钮。
- 前端 retry 调 `web/App.tsx::runPlanPhaseRetry`，实际 POST `/plans/{plan_id}/action`，body 为 `{ action: "start" }`。
- 后端 `start_next_plan_phase` 会选取 `pending/running/failed` 的第一个 phase。若该 phase 是 `failed`，会清空 `implementation_chat_id`、`agent_team_id`、`agent_task_id`、`commit_id`、`merge_attempt_count`、`error_message`，并把 steps 置回 `pending`。这会让 phase 当前字段指向新 run，但旧失败只能靠旧 chat/task 本身残留，Plan API 没有历史列表。

### 模型选择入口

- Plan runner 当前没有 per-plan/per-phase 配置。`plan_runner_model_selection` 选择第一个 enabled、text-output、active provider enabled 的 model，并带上 `model.thinking_level`。
- `QueueChatMessageInput` 已支持 `model_id`、`provider_id`、`thinking_level`，所以 runtime 派发层可以直接接收本次覆盖配置，不需要改 Agent dispatch 的核心参数形状。
- 前端已有可复用模型控件在 `web/features/chat/ChatPanel.tsx`：`ComposerModelProviderMenu` 负责 provider/model 组合选择，`ComposerSimpleSelectMenu` 负责 Thinking。后续可把这些控件抽成 shared 或在 Plan retry dialog 中复用同样的数据结构，不需要新依赖。

## 最小实现方案

ponytail: 只存运行历史和本次覆盖需要的字段；不做批量策略、自动 fallback 链或全局 plan 模型配置。以后要策略系统时，可以在 attempt 表之上新增 policy 表/JSON，而不是先把 runtime 搞复杂。

### Schema

新增 `plan_phase_attempts` 表，保留每次 phase 派发记录：

```sql
CREATE TABLE plan_phase_attempts (
    id TEXT PRIMARY KEY NOT NULL CHECK (id GLOB 'plan-phase-attempt-*'),
    plan_id TEXT NOT NULL REFERENCES plans(id) ON DELETE CASCADE,
    phase_id TEXT NOT NULL REFERENCES plan_phases(id) ON DELETE CASCADE,
    sequence INTEGER NOT NULL CHECK (sequence >= 0),
    trigger TEXT NOT NULL CHECK (trigger IN ('initial', 'retry', 'model_override_retry', 'merge_auto')),
    status TEXT NOT NULL CHECK (status IN ('queued', 'running', 'completed', 'failed', 'cancelled', 'interrupted')),
    provider_id TEXT CHECK (provider_id IS NULL OR length(provider_id) > 0),
    model_id TEXT CHECK (model_id IS NULL OR length(model_id) > 0),
    thinking_level TEXT CHECK (thinking_level IS NULL OR length(thinking_level) > 0),
    implementation_chat_id TEXT REFERENCES chats(id) ON DELETE SET NULL,
    agent_team_id TEXT REFERENCES agent_teams(id) ON DELETE SET NULL,
    agent_task_id TEXT REFERENCES agent_tasks(id) ON DELETE SET NULL,
    commit_id TEXT,
    error_message TEXT,
    started_at TEXT,
    completed_at TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    UNIQUE (phase_id, sequence)
);
```

说明：

- 用 `thinking_level` 存现有代码里的 reasoning effort 名称，UI 文案仍可叫 reasoning effort。
- `implementation_chat_id` 是本次 attempt 的日志/ transcript 入口；`agent_task_id` 是运行锚点。`agent_attempt_id` 不先存，因为 Agent attempt 运行后才产生，而且可从 task 关联查询；后续若查询成本或审计需要明确，可补 nullable 字段。
- `plan_phases` 继续作为当前摘要，避免大范围改现有 Plan 面板；新增 attempts 只作为 history。
- migration 可按当前 `plan_phases.agent_task_id IS NOT NULL` backfill 一条 sequence 0 attempt，状态沿用 phase status，保存当前 chat/team/task/error/commit。历史上已经被旧重试覆盖的更早失败无法恢复，这属于现状限制。

### Store/runtime 边界

- 新增 store 方法负责 begin/attach/complete/fail attempt，或扩展现有 `attach_plan_phase_run`、`complete_plan_phase_run`、`fail_plan_phase_run` 在更新 phase 摘要的同时写 attempts。
- 默认 retry 不应直接复用 `start_next_plan_phase` 的清空逻辑作为历史来源；应先创建新的 attempt，再清空/更新 phase 摘要指向新 attempt。
- 旧失败记录不能 UPDATE 成新 attempt。后续每次 dispatch 都 INSERT 新 `plan_phase_attempts` 行，完成/失败时只 UPDATE 对应 `agent_task_id` 的 attempt 行。
- merge 自动尝试可以记录为 `trigger = 'merge_auto'`；现有 `merge_attempt_count` 可继续保留为一次自动 merge guard。

### API 边界

保留现有 `POST /api/workspaces/{workspace_id}/plans/{plan_id}/action` 作为兼容入口。新增更明确的 phase retry API：

```http
POST /api/workspaces/{workspace_id}/plans/{plan_id}/phases/{phase_id}/retry
Content-Type: application/json

{
  "providerId": "openai",
  "modelId": "gpt-4.1",
  "thinkingLevel": "high"
}
```

参数规则：

- body 可为空或 `{}`，表示默认重试，使用 `plan_runner_model_selection` 当前默认选择。
- `modelId`/`providerId` 必须成对有效：provider 存在且 enabled，model 存在且 enabled，model 支持 text output，model.providerIds 包含 provider。
- `thinkingLevel` 可为 `null` 或空字符串，表示使用模型默认；非空值用现有 thinking levels 校验。
- 覆盖只进入本次 attempt 和 `QueueChatMessageInput`，不写回 `config.models[*].thinking_level` 或 provider/model 全局配置。
- 只允许对 `failed` phase 重试；当前 UI 对 `running` phase 显示 retry 更像“刷新/重开”遗留行为，换模型重试入口应收窄到 failed，避免并发覆盖当前运行。

### 前端边界

- 默认重试：失败 phase 上保留现有 `RefreshCw` 按钮，调用新 retry API，body 为空。
- 换模型重试：失败 phase 上新增明确按钮或菜单项，不用长按；打开 dialog/popover，复用现有 provider/model/thinking 数据和选择逻辑。
- Plan phase summary 可先展示当前 attempt 摘要；attempt history 可在 phase 展开区追加紧凑列表，列出 sequence、status、model/provider/thinking、error、chat/task 入口。
- 旧失败记录通过 attempts 展示，当前 phase 的 `errorMessage` 只代表最近/当前摘要。

## 后续最小检查

- store 单测：失败 phase retry 后旧 attempt 保持 `failed`，新 attempt sequence +1，phase 当前字段指向新 task。
- runtime 单测或轻量集成：override provider/model/thinking 进入 `QueueChatMessageInput`，但不修改 `GlobalConfig`。
- 前端单测：failed phase 显示默认重试和换模型重试入口，running phase 不显示换模型重试。
