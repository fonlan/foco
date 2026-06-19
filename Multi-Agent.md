# Foco Multi-Agent 分阶段实现计划

## 1. 文档用途

本文档用于指导 Foco 多 Agent 能力的分阶段实现。所有实施项、验证项和阶段退出条件均使用可勾选任务表示。

执行原则：

- 按阶段顺序推进；未满足当前阶段退出条件前，不进入依赖它的后续阶段。
- 每个阶段保持可编译、可测试、可回退，不提交只有数据结构而没有明确使用路径的半成品。
- SQLite 是 Agent 运行状态、任务队列和消息的事实来源；Tokio Channel 只负责进程内唤醒。
- 单个 AgentInstance 同时最多执行一个任务；并发只发生在不同实例之间。
- 不引入静默 fallback、静默重试、自动修正模型配置或隐式创建实例。
- 运行时任务队列与现有 ToDo graph 是两个不同概念，不复用表、状态或 API。

## 2. 最终目标

- 每个 AgentDefinition 可以配置独立 Provider、模型、模型参数、系统提示词和工具权限。
- 同一个 AgentDefinition 可以创建多个拥有独立上下文、队列和生命周期的 AgentInstance。
- AgentInstance 的任务严格顺序执行，不同实例可以受控并发。
- Agent 之间可以发送消息、委派任务、等待结果和转移未执行任务。
- 任务、消息、等待关系、执行尝试和事件可以在后端重启后恢复或明确标记为中断。
- Coordinator 的最终输出进入主聊天；Worker 的私有上下文和中间输出不污染主聊天历史。
- 所有模型调用继续使用现有 provider-neutral request/event 流、Provider 代理、审计、脱敏和上下文压缩能力。
- Web UI 可以配置 Agent、启用 Team、查看实例与队列，并执行暂停、取消、重试和转移操作。

## 3. 明确不进入首版的能力

- 跨进程或跨机器运行 Agent。
- Redis、RabbitMQ 或其它外部消息代理。
- 自动扩缩容 AgentInstance。
- 用户注入任意 Rust 路由函数。
- 广播消息和无边界 fan-out。
- 可抢占优先级队列。
- 等待子任务期间继续执行当前实例队列中的后续任务。
- 自动重试中断或失败的任务。
- 自动合并多个 Agent 的文件修改。
- 修改 AgentDefinition 后热更新既有实例。

## 4. 固定架构决策

### 4.1 配置与运行时边界

- AgentDefinition 保存到全局严格配置，供多个 workspace 复用。
- AgentTeam、AgentInstance、AgentTask、AgentMessage、AgentEvent 和私有上下文保存到 workspace 的 `.foco/foco.sqlite`。
- AgentInstance 创建时保存 AgentDefinition 的完整运行配置快照，但不保存 Provider 凭据、API key 或代理凭据。
- AgentDefinition 被修改或删除后，已创建实例继续使用原快照；新实例只能使用当前仍存在且有效的定义。
- Multi-Agent 按 chat 显式启用；未启用的既有单 Agent chat 保持当前行为。

### 4.2 执行与调度边界

- 现有 ChatRun/模型工具循环抽取为可复用的 AgentRunExecutor，单 Agent 与 Multi-Agent 共享核心执行器。
- Scheduler 通过 SQLite 事务领取任务，通过全局并发许可限制同时运行的模型任务数量。
- Scheduler wake channel 使用有界、可合并的全局唤醒信号；任意唤醒都会扫描全部可运行任务，因此 channel 满不会丢失具体任务。
- `waiting` 任务保持实例队首所有权，但不占用模型并发许可。
- 运行中任务在进程异常退出后变为 `interrupted`，必须由用户或 Agent 显式重试。

### 4.3 协作语义

- `send_message` 只发送信息，不创建执行任务。
- `delegate_task` 创建子任务并返回 task ID。
- `wait_tasks` 持久化等待条件并暂停当前执行尝试，不在工具调用中长期阻塞 Tokio task。
- `transfer_task` 只允许转移 `queued` 任务；`running` 或 `waiting` 任务必须先显式取消或完成。
- Worker 完成任务后更新任务结果并发出事件；结果不是伪装成普通 Message 的 `TaskType::Result`。
- 普通 Message 不打断正在进行的模型响应，只在目标实例下一个模型 turn 边界注入上下文。

### 4.4 Workspace 并发边界

- 首版允许模型推理、检索和只读工具跨实例并发。
- 可能修改 workspace 的工具通过 workspace mutation lease 串行执行。
- command 和无法确定副作用的工具按 mutation/unknown 处理，不假设其只读。
- 真正的并行代码修改通过后续独立 worktree 阶段实现。

## 5. 阶段依赖

```text
阶段 0  契约冻结
  ↓
阶段 1  AgentDefinition 配置
  ↓
阶段 2  Workspace 持久化模型
  ↓
阶段 3  AgentRunExecutor 抽取
  ↓
阶段 4  单实例持久调度垂直切片
  ↓
阶段 5  私有上下文与 Prompt
  ↓
阶段 6  消息、委派与 Agent 工具
  ↓
阶段 7  持久等待、转移与恢复
  ↓
阶段 8  多实例、路由与并发
  ↓
阶段 9  Workspace 修改协调
  ↓
阶段 10 SSE、API 与前端 UI
  ↓
阶段 11 安全、可观测性与发布验证
  ↓
阶段 12 独立 worktree（后续增强）
```

---

## 阶段 0：冻结领域契约与兼容边界

### 目标

在开始迁移和执行器重构前，固定 ID、状态机、错误语义和现有 chat 兼容策略。

### 0.1 领域术语与 ID

- [x] 定义 `AgentDefinitionId`、`AgentTeamId`、`AgentInstanceId`、`AgentTaskId`、`AgentMessageId`、`AgentAttemptId` 强类型 ID，避免使用 name 作为路由键。
- [x] 明确 AgentDefinition 是配置模板，AgentInstance 是独立运行身份，AgentTask 是持久工作项，AgentMessage 是非工作型通信。
- [x] 明确 Coordinator 也是普通 AgentInstance，仅通过 team role 区分，不创建另一套执行模型。
- [x] 明确 Agent runtime task 与现有 ToDo graph task 完全隔离。
- [x] 约定所有跨 API、SSE、审计和工具调用的 ID 序列化格式。

### 0.2 状态机

- [x] 固定 AgentInstance 状态：`idle`、`running`、`waiting`、`paused`、`draining`、`stopped`、`failed`。
- [x] 固定 AgentTask 状态：`queued`、`running`、`waiting`、`completed`、`failed`、`cancelled`、`interrupted`。
- [x] 固定 AgentAttempt 状态：`running`、`suspended`、`completed`、`failed`、`cancelled`、`interrupted`。
- [x] 为每个状态写出允许的进入状态、退出状态和拒绝条件。
- [x] 明确 `waiting` 仍占据实例队首，后续 queued task 不可越过。
- [x] 明确失败不会自动进入重试，重试是显式状态迁移并创建新 attempt。

### 0.3 兼容策略

- [x] 明确既有 chat 默认保持 single-agent 模式，不自动创建 team。
- [x] 明确启用 Multi-Agent 时必须选择有效的 Coordinator AgentDefinition。
- [x] 明确 team 模式下聊天模型由 Coordinator 快照决定，前端模型选择器不得静默覆盖它。
- [x] 明确关闭 team 前必须先处理 running、waiting 和 queued task。
- [x] 明确删除 chat 时级联删除 team runtime 数据，同时将需保留的 `llm_requests` 外键置空。

### 0.4 错误契约

- [x] 定义结构化 Agent domain error，至少包含 code、phase、message、retryable 和非敏感诊断字段。
- [x] 明确 Provider 错误继续遵守 model、adapter、base URL、proxy 状态诊断规则，不暴露凭据、query 或 fragment。
- [x] 明确队列冲突、无效状态迁移、实例上限、循环等待和 mutation lease 冲突均返回显式错误。
- [x] 约定 `anyhow` 仅用于应用边界聚合，domain、store、API 和工具层使用可匹配的错误类型。

### 0.5 阶段验证

- [x] 为所有状态迁移建立表驱动单元测试清单。
- [x] 为 single-agent 与 team-agent 两条入口建立兼容测试清单。
- [x] 评审并确认本阶段没有需要保留的开放架构问题。

### 阶段 0 退出条件

- [x] ID、状态机、错误和兼容语义已经形成稳定的 Rust 类型/API 设计。
- [x] 后续阶段不需要通过隐式 fallback 才能兼容既有 chat。
- [x] 领域术语在后端、数据库、SSE、工具 schema 和前端命名中保持一致。

### Phase 0 实现记录

- 领域契约位于 `agent/lib.rs`，作为 `foco-agent` crate 的公共 Rust API。
- ID 统一序列化为带实体前缀的纯字符串；状态和错误 code/phase 统一序列化为 `snake_case`。
- AgentTask 只能通过显式 `Retry` 从 `failed`、`cancelled` 或 `interrupted` 回到 `queued`；后续 store 实现必须同时创建新 attempt。
- Provider 错误继续复用现有脱敏诊断契约；Agent domain error 不携带 Provider 凭据或原始 URL。
- Phase 0 只冻结领域契约，不创建配置、SQLite schema、Scheduler、API 或 UI 半成品。

---

## 阶段 1：AgentDefinition 全局配置

**阶段状态：已完成（2026-06-19）**

### 目标

实现独立模型、系统提示词、工具权限和实例限制的严格配置，但暂不启动 Agent runtime。

### 1.1 配置模型

- [x] 在全局配置中增加 `agentDefinitions`，并保持严格 JSON schema 和未知字段报错。
- [x] 为 AgentDefinition 增加稳定 ID、revision、name、description、providerId、modelId、model options、systemPrompt、allowedTools、maxInstances。
- [x] 增加是否允许创建实例、是否允许委派、允许创建哪些 AgentDefinition 等最小权限配置；不实现通用 ACL 引擎。
- [x] 保证配置快照只引用 Provider ID，不复制 API key、Authorization header、cookie、代理凭据或 URL 凭据。
- [x] 为系统提示词、名称、描述和实例上限设置明确长度/范围限制。
- [x] 要求 name 在 UI 展示范围内大小写不敏感唯一，但所有运行时路由仍使用 ID。

### 1.2 Provider 与模型校验

- [x] 保存 AgentDefinition 时校验 provider 存在且配置有效。
- [x] 校验 model 存在，并具备 context window 和 max output tokens。
- [x] 校验 model options 类型和取值范围，不静默修正用户配置。
- [x] 校验 Provider 代理配置仍满足现有类型、scheme、URL 和凭据规则。
- [x] 删除 Provider 或 Model 配置时，显式拒绝会使 AgentDefinition 失效的操作，或要求先修改相关定义。

### 1.3 工具权限

- [x] AgentDefinition 只保存 Tool ID/name，不复制 ToolDefinition schema。
- [x] 保存时校验所有 allowedTools 均存在于运行时工具目录。
- [x] 明确 Agent 协作工具是否始终可用或受单独权限控制，并形成统一策略。
- [x] 对不允许委派或创建实例的 Agent，在工具目录暴露或工具执行阶段返回明确权限错误。
- [x] 保持现有文件、命令、图、ToDo、MCP 和 web 工具安全约束不变。

### 1.4 配置 API

- [x] 增加 AgentDefinition 列表、创建、更新和删除 API。
- [x] 创建定义时由服务端生成 ID 和初始 revision。
- [x] 每次影响运行配置的更新由服务端递增 revision。
- [x] 删除定义时允许既有实例继续使用快照，但阻止创建新实例。
- [x] API 响应不得包含 Provider 密钥、密码 hash 或其它敏感配置。

### 1.5 测试

- [x] 覆盖合法 AgentDefinition 的序列化与反序列化。
- [x] 覆盖未知字段、缺失字段、无效 Provider、无效 Model 和非法 model options。
- [x] 覆盖重复名称、无效工具、超长提示词和实例上限越界。
- [x] 覆盖 revision 更新与删除后实例快照兼容策略。
- [x] 覆盖配置 API 不泄露敏感字段。

### 阶段 1 退出条件

- [x] 可以通过后端 API 完整管理 AgentDefinition。
- [x] 无效模型、Provider、工具和权限配置均在保存时明确失败。
- [x] 尚未启用 runtime 时，现有 single-agent chat 行为完全不变。

### Phase 1 实现记录

- AgentDefinition 保存在全局严格配置的 `agentDefinitions` 中；服务端生成带前缀 ID，初始 revision 为 1，更新时递增。
- 配置只保存 Provider、Model、Tool 和可创建 AgentDefinition 的 ID；模型参数限定为 provider-neutral 请求当前支持的 thinking level 与 max output tokens，不复制任何 Provider 凭据。
- `allowedTools` 保存时与内置、Memory 和当前 MCP 运行时工具目录做精确名称校验；Agent 协作工具使用独立权限策略，不进入 `allowedTools`。
- CRUD API 为 `GET /api/agent-definitions` 与 `POST /api/agent-definitions/create|update|delete`；删除被定义引用的 Provider、Model 或目标 AgentDefinition 会明确失败。
- Phase 1 只增加配置和 API，不创建 Team、Instance、Scheduler 或 Agent runtime；现有 single-agent chat 路径未改动。

---

## 阶段 2：Workspace 持久化模型与迁移

**阶段状态：已完成（2026-06-19）**

### 目标

建立 team、instance、task、dependency、message、attempt、event 和私有上下文的持久化基础。

### 2.1 Schema 设计

- [x] 在 `store/workspace_schema.rs` 中增加多 Agent schema/migration 常量。
- [x] 增加 `agent_teams`，记录 chat、Coordinator、状态、并发上限和创建时间。
- [x] 增加 `agent_instances`，记录 definition ID/revision、无敏感信息的配置快照、角色、状态、队列序号和调度时间。
- [x] 增加 `agent_tasks`，记录 owner、origin、parent、sequence、状态、输入、结果、结构化错误和时间字段。
- [x] 增加 `agent_task_dependencies`，记录等待任务与被等待任务的关系及等待模式。
- [x] 增加 `agent_messages`，记录 sender、receiver、related task、replyTo、kind、内容和消费状态。
- [x] 增加 `agent_attempts`，记录任务每次执行/恢复尝试及其状态、开始结束时间和中断原因。
- [x] 增加 `agent_events`，记录 team 内单调递增事件序号、实体关联、事件类型、脱敏 payload 和时间。
- [x] 增加 `agent_context_entries` 与必要的 context snapshot 结构，避免 Worker 私有历史写入主 chat messages。
- [x] 为 `llm_requests` 增加 nullable team、instance、task、attempt 关联字段，并使用 `ON DELETE SET NULL` 保留审计。

### 2.2 约束与索引

- [x] 为 `(instance_id, sequence)` 增加唯一约束。
- [x] 使用约束或事务保证每实例同时最多一个 `running`/`waiting` task。
- [x] 为 runnable task、team event sequence、未读 message 和 task dependency 查询增加必要索引。
- [x] 为所有状态字段增加受控值校验，拒绝未知状态。
- [x] 保证 task parent、dependency、message receiver 和 context owner 必须属于同一 team。
- [x] 保证一个 team 只有一个 Coordinator。
- [x] 保证 chat 删除时级联删除 team runtime、私有上下文和事件，但保留脱离关联后的 LLM 审计。

### 2.3 Records 与 Store API

- [x] 在 `store/workspace_records.rs` 增加记录/DTO，并由 `store/workspace.rs` re-export 保持入口稳定。
- [x] 实现事务化创建 team 与 Coordinator instance。
- [x] 实现事务化分配 instance task sequence 并入队。
- [x] 实现原子领取 runnable task 的 lease/compare-and-set 操作。
- [x] 实现 task 完成、失败、取消、中断、等待和恢复的条件更新。
- [x] 实现 message 写入、读取、标记已消费和按上下文游标查询。
- [x] 实现 dependency 写入、满足判断、删除和循环检查所需查询。
- [x] 实现 append-only agent event 写入与按 event sequence 查询。
- [x] 实现 startup reconciliation 查询，定位遗留 running attempt/task。

### 2.4 Migration 与备份

- [x] Schema 升级前继续在 `<workspace>/.foco/backups` 创建数据库备份。
- [x] 验证空数据库、旧版本数据库和已有大量 chat 数据的迁移路径。
- [x] 迁移不得为既有 chat 隐式创建 AgentTeam。
- [x] 迁移失败必须保留原数据库和明确错误，不创建部分可用 schema。
- [x] 更新 schema version 和相关迁移测试。

### 2.5 测试

- [x] 覆盖完整 schema 创建和旧版本迁移。
- [x] 覆盖并发入队时 sequence 唯一且严格递增。
- [x] 覆盖两个 Scheduler 领取同一 task 时只有一个成功。
- [x] 覆盖跨 team parent/dependency/message 关联被拒绝。
- [x] 覆盖 chat 删除后的级联和 `llm_requests` 审计保留。
- [x] 覆盖重启 reconciliation 能准确识别 interrupted attempt。

### 阶段 2 退出条件

- [x] 所有 Agent runtime 状态均可只通过 SQLite 重建。
- [x] 不依赖内存 Channel、HashMap 或 Tokio task 才能判断任务状态。
- [x] Schema migration、备份和 chat 删除语义符合现有 workspace 存储约定。

### Phase 2 实现记录

- Workspace schema 升级到 v10，增加 Team、Instance、Task、Dependency、Message、Attempt、Event、Context Entry 和 Context Snapshot 表；同 Team 归属通过复合外键约束，Coordinator、活动 task 和活动 attempt 通过部分唯一索引约束。
- Team/Coordinator 创建、task/message/event 序号分配、task 入队和领取均使用 SQLite `IMMEDIATE` 事务；runnable 扫描以 SQLite 为事实来源，wake channel 不需要携带任务状态。
- Store API 提供 Team 全量重建、严格 FIFO runnable 查询、条件状态迁移、消息游标与消费、依赖满足/循环检查、append-only event、私有上下文和 startup reconciliation 查询。
- AgentDefinition 快照只序列化 Phase 1 的无凭据配置并拒绝敏感字段；Agent event payload 入库前脱敏。
- `llm_requests` 增加 nullable Agent 关联并校验 chat/team/instance/task/attempt 归属；删除 chat 时 runtime 级联删除，审计行及其已置空关联继续保留。
- 迁移测试覆盖空库、v7、v9、500 条既有 chat、迁移失败回滚与备份；并发测试覆盖 sequence 分配和双 Scheduler 领取竞争。

---

## 阶段 3：抽取可复用 AgentRunExecutor

**阶段状态：已完成（2026-06-19）**

### 目标

将现有模型/工具循环与 HTTP/SSE/chat 编排解耦，使 single-agent 和 team-agent 共享同一个执行核心。

### 3.1 执行器输入输出

- [x] 定义 `AgentRunContext`，包含 chat、team、instance、task、attempt、workspace、配置快照和取消信号。
- [x] 定义 `AgentRunInput`，包含 provider-neutral messages、当前任务、未读消息和恢复信息。
- [x] 定义 `AgentRunEvent`，覆盖 reasoning、text、usage、completion、error、tool call、tool result 和 control outcome。
- [x] 定义 `AgentRunOutcome`，区分 completed、failed、cancelled 和 suspended。
- [x] 保证所有事件带可选 team/instance/task/attempt ID，single-agent 可保持这些字段为 null。

### 3.2 抽取执行循环

- [x] 将 provider-neutral 请求构建、stream 处理和工具 continuation 循环收敛到 `agent` crate 定义的执行器边界，并由唯一的 Foco run task 适配应用依赖。
- [x] 保留现有最大工具轮数与达到上限后的上下文压缩恢复逻辑。
- [x] 保留重复工具调用循环检测。
- [x] 保留 Provider stream 错误的非敏感诊断上下文。
- [x] 保留现有 Provider 请求重试配置；AgentRunExecutor 自身不增加隐式重试或创建新 attempt。
- [x] 保留 tool timeout、命令终止和同 turn 文件编辑冲突规则。
- [x] 不让 AgentRunExecutor 直接依赖 Web SSE sender；通过事件 sink/callback 交付事件。

### 3.3 Audit 集成

- [x] AgentRunExecutor 创建 LLM 审计时写入 chat 和可选 team/instance/task/attempt ID。
- [x] 请求、响应、stream 和工具 payload 继续在入库前脱敏。
- [x] Coordinator 与 Worker 使用同一套模型、Provider、耗时、速率和首 token 延迟计算。
- [x] Worker 审计可被 UI 查询，但不作为主聊天 assistant message 展示。

### 3.4 Single-Agent 回归

- [x] 将现有 single-agent ChatRun 改为调用 AgentRunExecutor。
- [x] 保持 single-agent SSE 顺序、消息持久化、取消和错误行为不变。
- [x] 保持 Markdown、reasoning、工具调用和完成指标的前端事件形状不变，除新增 nullable Agent 标识外。
- [x] 保持现有 memory retrieval 在 chat 入库和 start SSE 后发生的时序。
- [x] 保持 ContextPreview 的 memory 内联行为不变。

### 3.5 测试

- [x] 将执行器单元测试覆盖到文本、工具、错误、取消、压缩和重复循环路径。
- [x] 证明同一 fixture 经旧入口语义与新 AgentRunExecutor 产生等价事件序列。
- [x] 覆盖 Agent ID 字段为空时的 single-agent 审计和 SSE。
- [x] 覆盖含 Agent ID 时审计正确关联且敏感字段仍被脱敏。

### 阶段 3 退出条件

- [x] 所有现有 single-agent 测试继续通过。
- [x] AgentRunExecutor 不依赖 Multi-Agent Scheduler 即可独立测试。
- [x] Multi-Agent 后续实现不需要复制模型或工具循环。

### Phase 3 实现记录

- `agent/executor.rs` 提供 provider-neutral 的运行上下文、输入、关联 ID、取消信号、事件封装、结果状态、事件 emitter/sink 和不依赖 Scheduler 的 `AgentRunExecutor`。
- Foco 专属的 Provider、Hook、Memory、工具和持久化依赖通过 `FocoAgentRunTask` 适配；single-agent 入口已经只通过该执行器驱动同一条现有模型/工具 continuation 循环，后续 team-agent 直接复用该 run task。
- Web SSE 仍由 `ActiveChatRunRegistration` 负责；执行器只投递带 Agent 关联信息的泛型事件，因此没有引入 SSE、HTTP 或 workspace store 依赖。
- ChatRun 取消信号同步桥接到 AgentRun 与工具取消 token；工具轮数上限、压缩恢复、重复调用检测、timeout、文件冲突、Memory 时序和 ContextPreview 路径未改变。
- LLM 审计插入路径现在写入可选 team、instance、task、attempt ID；single-agent 保持四个字段为 null，既有脱敏和指标计算路径继续复用。
- 验证通过 `foco-agent` 26 项测试与 `foco-app` 122 项测试，包含事件顺序等价、sink 失败取消、Agent 关联审计、压缩和重复工具循环回归。

---

## 阶段 4：单实例持久调度垂直切片

**阶段状态：已完成（2026-06-19）**

### 目标

完成一个显式启用的 AgentTeam、一个 Coordinator、一个 FIFO 队列和一个持久 Scheduler 的端到端路径。

### 4.1 Team 生命周期

- [x] 增加为 chat 启用 AgentTeam 的服务与 API，要求指定 Coordinator AgentDefinition ID。
- [x] 在一个事务中创建 team、Coordinator instance 和无敏感信息的配置快照。
- [x] 增加读取 team/instance/task snapshot 的 API。
- [x] 增加 pause、resume、drain、stop Team/Instance 的明确操作。
- [x] 阻止在存在 active/queued task 时直接删除实例或关闭 team。
- [x] Team 模式启用后，用户消息写入 chat 后创建 Coordinator task，而不是直接启动模型调用。

### 4.2 Scheduler 基础

- [x] 实现全局 AgentScheduler 生命周期并随后端启动和优雅关闭。
- [x] 使用 bounded `tokio::sync::mpsc` 发送无 payload 或可合并的全局 wake 信号。
- [x] Scheduler 启动时扫描全部 workspace 中的 runnable task。
- [x] 每次入队、完成、取消、恢复和实例 resume 后触发 scheduler wake。
- [x] Scheduler 查询 SQLite 领取任务，而不是把任务 payload 放进 Channel。
- [x] 使用全局 Semaphore 限制同时运行的 AgentRun 数量。
- [x] 使用结构化并发跟踪活跃 attempt，后端关闭时显式 cancel/drain。

### 4.3 Coordinator 执行

- [x] Scheduler 为 Coordinator task 创建 AgentAttempt。
- [x] 构建 AgentRunContext 并调用 AgentRunExecutor。
- [x] 将 Coordinator 文本和工具事件按现有顺序流向主聊天 SSE。
- [x] 成功时完成 task/attempt，并将最终 assistant output 写入主聊天历史。
- [x] 失败或取消时同步更新 task、attempt、instance 和 team event。
- [x] 当前任务结束后自动唤醒 Scheduler 领取同实例下一个 queued task。

### 4.4 FIFO 与背压

- [x] 使用持久 sequence 保证单实例严格 FIFO。
- [x] 为每个 team、instance 和 chat 设置明确的最大 queued task 数。
- [x] 队列满时拒绝新任务并返回明确错误，不覆盖或丢弃旧任务。
- [x] 证明 Coordinator running 时提交的新用户消息只入队，不并行启动第二个 Coordinator run。

### 4.5 重启与中断

- [x] 后端启动 reconciliation 将遗留 running attempt 标记为 interrupted。
- [x] 对应 task 进入 interrupted，而不是自动重新入队。
- [x] instance 从 running/waiting 恢复到可解释的 paused/failed 状态。
- [x] 提供显式 retry API：保留 task ID，创建新 attempt，并记录 retry event。
- [x] retry 前重新校验实例、定义快照和 workspace 是否仍有效。

### 4.6 测试

- [x] 覆盖单 Coordinator 多任务 FIFO。
- [x] 覆盖并发 wake 信号不会重复执行任务。
- [x] 覆盖 Channel 满时 SQLite 任务仍被后续扫描执行。
- [x] 覆盖全局并发许可和优雅关闭。
- [x] 覆盖进程重启后的 interrupted 与显式 retry。
- [x] 覆盖 single-agent chat 与 team-agent chat 可以同时运行且互不污染。

### 阶段 4 退出条件

- [x] 用户可以显式启用一个只有 Coordinator 的 Team 并连续发送多条任务。
- [x] 后端重启后不丢失 queued task，也不会静默重放 active task。
- [x] single-agent 和 team-agent 共享 AgentRunExecutor，但各自入口行为明确。

### Phase 4 实现记录

- `app/runtime/agent_scheduler.rs` 提供随后端启动的全局持久 Scheduler：容量 1 的可合并 wake channel 只传唤醒信号，任务始终从各 workspace SQLite 扫描并事务领取；全局 Semaphore 与 `JoinSet` 分别限制并发和负责优雅收敛。
- `app/http/agents.rs` 提供 Team 启用、team/instance/task snapshot、Team/Instance 生命周期和 task cancel/retry API；Team 与 Coordinator snapshot 继续由 Phase 2 的单事务 Store API 创建，不保存 Provider 凭据。
- Team chat 的 queue 入口以 Coordinator snapshot 作为 model/provider 权威，持久化 user message 对应的 Coordinator task；同实例 sequence、队首查询和领取 CAS 保证严格 FIFO，team、instance、chat 的 queued 上限均为 64。
- Coordinator task 继续通过 `FocoAgentRunTask` 和 `AgentRunExecutor` 运行，以 task ID 复用主聊天 SSE/run-event/assistant 历史路径，并把 team、instance、task、attempt ID 写入 LLM audit；single-agent 入口保持原有直接 ChatRun 行为。
- 启动 reconciliation 将遗留 running/waiting task 与 attempt 标记为 interrupted、实例置为 paused，保留 queued task；显式 retry 保留 task ID，重新校验 workspace 与定义快照后由下一次领取创建新 attempt。
- 针对性测试覆盖 Team API、queue/cancel、队列上限、严格 FIFO、竞争领取、wake 合并、重启中断和显式 retry；single-agent AgentRun/SSE 回归与 Agent 关联 audit 测试继续通过。

---

## 阶段 5：实例私有上下文、Prompt 与 Memory 集成

**阶段状态：已完成（2026-06-19）**

### 目标

使每个 AgentInstance 拥有独立、持久、可压缩的上下文，并按固定层级构建提示词。

### 5.1 Prompt 分层

- [x] 固定 Prompt 顺序：Foco 基础系统提示词 → AgentDefinition 系统提示词 → workspace AGENTS/配置提示词 → Team 协议与实例身份 → 当前任务与消息。
- [x] 为 Team 协议注入 instance ID、definition ID、角色、允许的协作工具和运行限制。
- [x] 任务、parent task 摘要、dependency 结果和未读消息使用运行时消息层，不拼接进永久系统提示词。
- [x] AgentDefinition systemPrompt 为空、超长或读取失败时明确报错，不静默使用其它提示词。
- [x] Prompt cache key 纳入 definition revision、实例配置快照、memory resolved 内容和 Team 协议版本。

### 5.2 私有上下文

- [x] Worker/Coordinator 的私有 context entries 按 instance 和 sequence 持久化。
- [x] Coordinator 面向用户的最终消息同时进入主 chat history；Worker 输出只进入私有上下文和 task result。
- [x] 未读 AgentMessage 只注入目标实例，并在成功构建 turn 后更新消费游标。
- [x] 已完成任务保留结构化结果和摘要，不无限复制完整历史。
- [x] 实例 reset 必须显式创建新的 context generation，不修改历史审计。

### 5.3 上下文压缩

- [x] 复用现有 runtime tool-state 压缩能力，并按 instance 隔离。
- [x] 达到工具 continuation 上限时只压缩当前实例的工具协议状态。
- [x] 增加跨任务摘要策略，确保实例长期使用时上下文不会无限增长。
- [x] 压缩失败必须使当前 task 明确失败，不丢弃无法恢复的上下文。
- [x] context snapshot 必须关联 instance、task、attempt 和构建版本。

### 5.4 Memory 行为

- [x] Coordinator 保持 chat memory 在任务可见/start 事件之后解析的现有时序。
- [x] Worker 可读取同 chat 的 global/workspace/chat memory，但 memoryResolved 事件必须带 instance/task ID。
- [x] Worker 的自动 memory extraction 默认关闭，避免多个实例重复写入同一事实。
- [x] 只有 Coordinator 的用户可见完成结果进入现有自动提取路径。
- [x] ContextPreview 对 Coordinator 保持 memory 内联；Worker 暂不提供独立 Preview，除非后续 UI 明确需要。

### 5.5 测试

- [x] 覆盖两个同 Definition 实例拥有不同 context history。
- [x] 覆盖修改 AgentDefinition 后既有实例继续使用旧快照。
- [x] 覆盖 Prompt 分层顺序与 workspace AGENTS 注入。
- [x] 覆盖 Worker 输出不会写入主 chat history。
- [x] 覆盖 memory retrieval 时序、cache key 重算和 Worker 不自动提取 memory。
- [x] 覆盖实例上下文压缩与工具轮数恢复。

### 阶段 5 退出条件

- [x] 每个实例都可以在多个顺序任务间保持独立连续上下文。
- [x] 主聊天只显示 Coordinator 用户可见输出，不包含 Worker 私有历史。
- [x] Prompt、Memory、压缩和审计均可按 instance/task 追踪。

### Phase 5 实现记录

- `app/runtime/agent_scheduler.rs` 在 Scheduler 领取任务后补齐 Agent prompt 层：验证 AgentDefinition `system_prompt`，按实例快照注入定义提示词和 Team protocol，并将当前 task 与未读 AgentMessage 作为运行时消息层传入 `AgentRunExecutor`。
- Team protocol 记录协议版本、team/chat/instance/task/attempt ID、definition ID/revision、角色、协作权限、允许的运行时工具、队列上限和工具轮数上限；`AgentRole::as_str` 提供稳定 `snake_case` 角色值。
- 每个 task 完成后写入 `agent_context_entries` 并生成带 version、team protocol version、instance、task、attempt、generation 和 build version 的 `agent_context_snapshots`；snapshot 只保留有界摘要，避免跨任务完整历史无限增长。
- Worker 运行继续写 LLM audit、tool call/result 和 task result，但 `persist_chat_result` 不写主 chat assistant message，也不排自动 memory extraction；Coordinator 保持原主聊天输出和自动抽取路径。
- 未读 AgentMessage 只读取目标实例未消费消息，成功构建 prompt 后标记 consumed，并作为 `AgentRunInput.unread_messages` 传给共享执行器。
- Deferred memory retrieval 继续在 start 事件后执行；`memoryResolved` SSE 增加可选 team/instance/task ID，prompt cache key 显式哈希 AgentDefinition、Team protocol 和 resolved memory 内容。
- `reset_context` 实例 action 通过 `reset_agent_instance_context` 显式递增 `context_generation`，拒绝仍有 queued/running/waiting task 的实例，并保留旧 generation 的 context 与审计历史。
- 针对性测试覆盖 Agent prompt cache key、Worker 不写主聊天且不抽取 memory、context reset generation、既有 memory 时序和 runtime tool-state 压缩回归；全量 workspace 测试继续通过。

---

## 阶段 6：Agent 消息、任务委派与内置工具

**阶段状态：已完成（2026-06-19）**

### 目标

实现 Agent 之间的持久消息和子任务委派，但暂不实现复杂等待与转移。

### 6.1 Agent 工具家族

- [x] 新增 `tools/agent_tools.rs`，`tools/lib.rs` 只保留注册、公共类型、调度入口和共享 helper。
- [x] 实现 `agent_list`，返回当前 Team 可见的 definitions、instances、状态和队列摘要。
- [x] 实现 `agent_get_task`，按权限返回 task 状态、结果和结构化错误。
- [x] 实现 `agent_send_message`，持久化点对点消息。
- [x] 实现 `agent_delegate_task`，创建带 origin/parent 的子任务。
- [x] 实现 `agent_cancel_task`，先支持取消 queued 子任务。
- [x] 所有 schema 遵守 strict JSON schema、properties 全部 required、可选值用 null，并包含 `timeoutMs`。

### 6.2 消息语义

- [x] Message receiver 只接受 instance ID，不接受 name 或 broadcast。
- [x] Message kind 首版只支持 notification 和 reply，不把 message 当作任务执行。
- [x] 为同一 sender/receiver 提供稳定顺序字段。
- [x] 目标实例正在运行时，不修改已发出的 Provider 请求；消息保持未读直到该实例下一次 prompt 构建边界注入。
- [x] 目标实例空闲时，普通消息保持未读，不自动唤醒 LLM。
- [x] Message 内容、事件和 UI 通知入库/输出前执行敏感字段脱敏。

### 6.3 委派语义

- [x] `agent_delegate_task` 要求显式 target instance 或 target definition，二者不能同时为空或同时非空。
- [x] 指定 instance 时校验它属于当前 team、可接收任务且未 stopped。
- [x] 指定 definition 时首版只允许已有实例；无实例时明确报错，不自动创建。
- [x] 子任务记录 origin instance、parent task 和 correlation ID。
- [x] 委派返回 task ID 和选中的 instance ID，不等待任务完成。
- [x] Worker 完成后写入 task result，并记录带 parent/origin 关联的 task completion event。
- [x] 子任务失败不会自动使 parent 失败，由 parent 后续恢复逻辑决定如何处理。

### 6.4 权限与限制

- [x] 普通运行时工具继续按 definition 快照 `allowedTools` 过滤；Agent 协作工具不写入 `allowedTools`，统一按 `AgentPermissions` 暴露和执行。
- [x] 限制单个 task 可创建的子任务数量。
- [x] 限制最大委派深度。
- [x] 限制单个 message 和 task input/result 的持久化大小。
- [x] 拒绝跨 team 消息、委派和 task 查询。
- [x] 所有权限失败和上限失败返回稳定错误 code。

### 6.5 事件与审计

- [x] 记录 message created/consumed 事件。
- [x] 记录 task delegated/queued/started/completed/failed/cancelled 事件。
- [x] Agent 工具调用继续写入现有 tool call/result 审计。
- [x] 事件 payload 不重复存储完整敏感 Prompt 或 Provider 请求体。

### 6.6 测试

- [x] 覆盖 active instance 与 idle instance 的消息消费差异。
- [x] 覆盖显式实例委派、definition 委派和无实例报错。
- [x] 覆盖跨 team、无权限、超深度、超数量和超 payload 限制。
- [x] 覆盖 Worker 成功、失败和取消后的 parent completion event。
- [x] 覆盖工具 schema 的 OpenAI Responses strict 兼容性。

### 阶段 6 退出条件

- [x] Coordinator 可以向 Worker 委派任务并异步查询结果。
- [x] Agent 可以发送持久点对点消息且消息不会造成隐式模型运行。
- [x] 所有协作操作可通过 task/message/event/audit 完整追踪。

### Phase 6 实现记录

- `tools/agent_tools.rs` 定义 `agent_list`、`agent_get_task`、`agent_send_message`、`agent_delegate_task` 和 `agent_cancel_task` 的 strict JSON schema；所有 properties 均 required，可选值使用 `null`，并统一包含 `timeoutMs`。
- `app/runtime/agent_scheduler.rs` 在 Team Agent prompt 中追加协作工具定义：`send_message` 始终可用，delegate/cancel 按 `canDelegate` 暴露；普通运行时工具仍按 AgentDefinition 快照 `allowedTools` 过滤。
- `app/runtime/tool_execution.rs` 在 runtime 层执行 Agent 工具，基于 `AgentToolContext` 校验 team/instance/task association、协作权限和可见性，所有失败通过结构化 `{ code, error }` tool result 返回。
- `agent_send_message` 只接受 receiver instance ID，支持 `notification` 与 `reply`，写入 `agent_messages` 并记录 `message_created`；普通消息不创建 task、不唤醒 Scheduler，目标实例下一次 prompt 构建时读取未消费消息并记录 `message_consumed`。
- `agent_delegate_task` 支持显式 target instance 或 target definition 二选一；definition 路由只选择既有 runnable instance，不自动创建实例；子任务持久化 origin instance、parent task、correlation ID，并立即返回 task ID 与目标 instance ID。
- `agent_cancel_task` 首版只取消当前任务委派出的 queued child task；running/waiting/completed 等状态明确拒绝，取消结果写入 task error 并记录 `task_cancelled`。
- Scheduler 在任务领取和完成路径记录 `task_started`、`task_completed`、`task_failed`、`task_cancelled` 等事件，task outcome event payload 带 origin/parent 关联；Worker 输出继续写 task result 与私有上下文，不污染主 chat history。
- Workspace schema 升级到 v11，`agent_messages.kind` 收敛为 `notification`/`reply`，迁移时重建相关外键表，旧 `response` 映射为 `reply`，其它旧值映射为 `notification`。
- 持久化层对 Agent event payload、message content 和 task outcome 执行敏感信息脱敏或大小限制；message、delegated input、task result/error 均有固定上限。
- 新增 Phase 6 针对性测试覆盖 message 顺序/脱敏/显式消费、child task origin/parent/cancel、definition lookup 只返回既有实例、Agent tool strict schema 和 task outcome 上限；目标 crate 测试全部通过。

---

## 阶段 7：持久等待、恢复、转移与死锁检测

**阶段状态：已完成（2026-06-19）**

### 目标

实现真正可恢复的 fan-out/fan-in，而不是在 Tokio 工具调用中长期阻塞等待。

### 7.1 `agent_wait_tasks` 控制结果

- [x] 为 AgentRunExecutor 增加可持久化的 `suspend` control outcome。
- [x] 实现 `agent_wait_tasks`，首版只支持等待全部指定 task 完成。
- [x] 工具调用校验目标 task 属于当前 team，且当前 task 有权限等待它们。
- [x] 持久化 wait dependency、deadline、pending tool call ID 和恢复所需上下文。
- [x] 将当前 task/attempt 更新为 waiting/suspended 后释放模型并发许可并结束 Tokio 执行任务。
- [x] 不在 `agent_wait_tasks` 内部通过 `recv().await` 持有长期运行的 worker task。

### 7.2 恢复协议

- [x] 所有 dependency 完成或 deadline 到期时，由事务将 waiting task 标记为可恢复。
- [x] Scheduler 为恢复创建新 attempt，但保持原 task ID 和实例队首所有权。
- [x] 恢复时生成与原 pending tool call 配对的 tool result，包含各 dependency 的状态、结果或错误。
- [x] 恢复后继续调用 AgentRunExecutor，而不是从头重新执行用户任务。
- [x] 等待超时时不自动取消子任务；将当前状态返回 Agent，由其显式决定。
- [x] 重启后可以从 SQLite 重建等待条件并在依赖满足时恢复。

### 7.3 死锁检测

- [x] 在新增 wait dependency 前检查 task dependency cycle。
- [x] 将实例 FIFO 队首占用纳入 wait-for graph。
- [x] 检测 A 等待 B，而 B 又等待排在 A 当前任务之后的 A-instance task 等队列级死锁。
- [x] 检测失败时拒绝进入 waiting，并向 Agent 返回可诊断但不泄密的错误。
- [x] 为死锁图查询设置规模限制和 timeout，超时明确失败。

### 7.4 Task 转移

- [x] 实现 `agent_transfer_task` 工具与 API。
- [x] 只允许转移 queued task。
- [x] 在一个事务中修改 owner、分配目标实例新 sequence、记录 transfer event 并唤醒 Scheduler。
- [x] running、waiting、completed、failed、cancelled、interrupted task 的转移请求明确失败。
- [x] 转移时重新校验目标实例工具权限、definition 状态和队列上限。
- [x] 保留 origin、parent 和历史 owner event，不重写审计历史。

### 7.5 取消与重试

- [x] queued task 取消后从可运行查询中排除。
- [x] running task 取消通过 attempt cancellation token 通知 AgentRunExecutor 和工具运行时。
- [x] waiting task 取消时删除/终止等待关系，并显式决定是否级联取消子任务。
- [x] 将 cascade 作为明确参数，不使用隐式默认。
- [x] failed/interrupted task 只有显式 retry 才重新进入队列。
- [x] retry 保留 task ID、增加 attempt，并在上下文中注入前次失败摘要，避免假装首次执行。

### 7.6 测试

- [x] 覆盖一个 parent 并行委派多个 child 后等待全部完成并恢复。
- [x] 覆盖 child success、failure、cancel 和 timeout 的恢复结果。
- [x] 覆盖等待期间后续任务无法越过队首。
- [x] 覆盖后端重启后 waiting task 正确恢复。
- [x] 覆盖 task dependency cycle 和 queue-level deadlock。
- [x] 覆盖 queued transfer 的序号、权限、上限和审计。
- [x] 覆盖 running/waiting transfer 明确失败。
- [x] 覆盖取消与显式 retry 不重复执行已完成 attempt。

### 阶段 7 退出条件

- [x] Coordinator 能持久化地 fan-out 多个任务、暂停、等待并 fan-in 结果。
- [x] 等待不占用模型许可或长期 Tokio worker。
- [x] 进程重启不会丢失等待关系，也不会自动重放有副作用的 attempt。

### Phase 7 实现记录

- `agent/executor.rs` 的 `AgentRunOutcome::Suspended` 作为持久 suspend control outcome；`app/main.rs` 在收到 `agent_wait_tasks` tool result 的 suspend control 后结束当前执行轮次，由 Scheduler 统一落库。
- `tools/agent_tools.rs` 新增 `agent_wait_tasks` 和 `agent_transfer_task` strict schema；`app/runtime/tool_execution.rs` 执行时校验 team/task 可见性、权限、等待目标数量、deadline、dependency cycle 和同实例队首死锁，并持久化 wait dependency、pending tool call ID 与 deadline。
- Workspace schema 升级到 v12：`agent_task_dependencies` 增加 `pending_tool_call_id` 与 `deadline_at`，`agent_attempts_one_active_per_task_idx` 收敛为仅限制 running attempt，使 suspended 历史 attempt 与恢复新 attempt 可以共存。
- `store/workspace.rs` 的 runnable/claim/resume 查询将 dependency 的 completed/failed/cancelled/interrupted 视为可恢复终态，并支持 deadline 到期；`resume_satisfied_agent_tasks` 在事务内将 waiting task 恢复为 queued，并把实例从 waiting 释放回 idle/draining。
- `app/runtime/agent_scheduler.rs` 增加 deadline 轮询、waiting task 恢复扫描和 `task_resumed` event；恢复时为原 pending tool call 生成 provider-neutral assistant tool-call 与 tool result 配对消息，同时在当前任务 JSON 中保留 resume 摘要。
- 启动 reconciliation 只中断遗留 running task/attempt；waiting task 保留在 SQLite，后续 dependency 满足或 deadline 到期后由 Scheduler 恢复，不自动重放有副作用的 active attempt。
- `agent_transfer_task` 和 task transfer API 只接受 queued task，在事务中更新 owner、分配目标实例 sequence、校验 team/instance 状态与队列上限，保留 origin/parent 和历史事件，不重写审计历史。
- task action cancel/retry 保持显式状态转换：running 通过现有 active run cancellation 通知执行器和工具，waiting cancel 必须显式提供 `cascade`，并在事务中清理 wait dependency；retry 保留 task ID，恢复 queued，由下一次 claim 创建新 attempt，并在 prompt 中注入前次 result/error 摘要。
- 新增 Phase 7 针对性 store 测试覆盖 wait dependency 持久化、dependency 完成恢复、deadline 恢复、新 attempt 创建、queued transfer、running transfer 拒绝、waiting cancel 清理 dependency 和 retry 保留前次错误；既有 dependency cycle 测试继续覆盖 cycle 拒绝。

---

## 阶段 8：多实例、路由、并发与生命周期

### 目标

允许同一 AgentDefinition 创建多个独立实例，并在实例间实现受控并发。

### 8.1 实例创建

- [ ] 实现用户 API 与 `agent_create_instances` 工具。
- [ ] 每次创建要求 definition ID、count 和明确的运行限制参数。
- [ ] 所有实例保存相同 definition revision 的独立配置快照。
- [ ] 每个实例创建独立 context generation、queue sequence 和状态。
- [ ] 创建前检查 definition、team、task 和全局实例上限。
- [ ] count 部分失败时整个事务回滚，不返回部分创建结果。

### 8.2 路由策略

- [ ] 支持指定 instance ID 精确路由。
- [ ] 支持指定 definition ID 路由到现有实例。
- [ ] definition 路由使用 least-pending，并使用 lastScheduledAt/instance ID 做稳定 tie-break。
- [ ] paused、draining、stopped、failed 实例不参与新任务路由。
- [ ] waiting 实例的当前任务计入负载，且不能运行新任务。
- [ ] 路由决定写入 task/event，保证问题可审计和复现。
- [ ] 不实现自定义 Rust router、随机路由或自动创建实例。

### 8.3 调度公平性

- [ ] 全局 Scheduler 在不同 team/instance 之间实现稳定公平选择，避免单一 chat 长队列长期饿死其它 team。
- [ ] 每个实例同一时间只获得一个 task lease。
- [ ] 全局模型并发限制与单 Provider 并发/速率错误保持独立；Provider 错误不触发自动重试。
- [ ] Scheduler 完成一次领取后及时让出执行，避免持有 workspace store 锁跨 await。
- [ ] 记录 queue wait time、run time 和 scheduler latency。

### 8.4 实例生命周期

- [ ] pause：保留队列，不领取新任务；当前任务行为必须由 API 参数明确决定。
- [ ] resume：恢复领取任务并触发 scheduler wake。
- [ ] drain：拒绝新任务，等待当前任务和现有队列完成。
- [ ] stop：要求明确选择取消或转移 queued task，并处理 active task。
- [ ] reset context：仅创建新 context generation，不删除旧审计、任务和消息。
- [ ] delete：只有实例无 active/queued/waiting task 时允许，或先完成显式迁移流程。

### 8.5 资源限制

- [ ] 增加全局最大并发 AgentRun。
- [ ] 增加单 Team 最大实例数和最大 queued task 数。
- [ ] 增加单 Definition 在 Team 内最大实例数。
- [ ] 增加最大委派深度、单任务子任务数、消息数和持久化 payload 大小。
- [ ] 增加单 Team token/请求预算的统计基础；首版超限时停止新 run，不做模型降级。
- [ ] 所有限制达到时写入明确事件和错误。

### 8.6 测试

- [ ] 覆盖同 Definition 多实例使用同快照但不同上下文和队列。
- [ ] 覆盖不同实例并行、同一实例严格串行。
- [ ] 覆盖 least-pending 的稳定选择和不可用实例过滤。
- [ ] 覆盖全局并发上限、公平性和队列等待指标。
- [ ] 覆盖 pause/resume/drain/stop/reset/delete 生命周期。
- [ ] 覆盖实例和任务上限，不出现部分创建或静默丢弃。

### 阶段 8 退出条件

- [ ] 同一 AgentDefinition 可以安全创建多个独立实例并并行执行任务。
- [ ] 路由结果确定、可审计，实例状态变化不会破坏 FIFO。
- [ ] 资源上限可以阻止递归委派和实例爆炸。

---

## 阶段 9：Workspace 修改协调与工具副作用

### 目标

在共享工作目录模式下防止多个 Agent 同时执行不可控的修改操作。

### 9.1 Tool effect 分类

- [ ] 为运行时工具增加内部 `ToolEffect`：`read_only`、`workspace_mutation`、`external_or_unknown`。
- [ ] file write/edit 工具标记为 workspace mutation。
- [ ] command 工具默认标记为 external_or_unknown，不解析命令猜测只读性。
- [ ] MCP 工具未声明可信 effect 时按 external_or_unknown 处理。
- [ ] graph/query、web fetch 和明确只读工具标记为 read_only。
- [ ] ToolEffect 只作为调度安全元数据，不改变对模型暴露的 strict schema。

### 9.2 Mutation lease

- [ ] 为每个 workspace 实现 mutation lease，Coordinator 和所有 Worker 共用。
- [ ] read-only 工具不获取 mutation lease，可跨实例并发。
- [ ] mutation/unknown 工具在执行前获取 lease，结束、超时、取消或 panic 时可靠释放。
- [ ] 不持有数据库事务等待 mutation lease。
- [ ] lease 等待遵守工具 timeoutMs；超时返回明确错误。
- [ ] 记录 lease owner instance/task/tool call 和等待时间，供诊断与 UI 展示。

### 9.3 文件与命令安全

- [ ] 保持文件工具 workspace-relative、canonicalize 后仍位于 workspace 内的约束。
- [ ] 保持 edit_file 必须先读取最新内容、oldStr 精确匹配和默认唯一匹配。
- [ ] 跨实例 stale edit 明确失败，不自动重新读取并套用修改。
- [ ] 同一实例/turn 的 edit/write 冲突规则保持不变。
- [ ] command timeout 继续终止进程树，并保持 Windows Job Object/KillOnDrop 行为。
- [ ] mutation lease 不替代 Provider、MCP、hook 和文件工具原有安全校验。

### 9.4 测试

- [ ] 覆盖两个实例的 read-only 工具可并行。
- [ ] 覆盖两个实例的 mutation 工具严格串行。
- [ ] 覆盖 lease 等待 timeout、取消和 panic 后释放。
- [ ] 覆盖 stale edit、命令超时和跨 workspace 路径逃逸仍明确失败。
- [ ] 覆盖 MCP unknown effect 不绕过 mutation lease。

### 阶段 9 退出条件

- [ ] 共享 workspace 中不存在两个并发运行的 mutation/unknown 工具。
- [ ] 并行读取和模型推理不受不必要的全局串行限制。
- [ ] 并发冲突以明确错误呈现，不产生静默覆盖或自动合并。

---

## 阶段 10：SSE、API 与前端 UI

### 目标

让用户可以配置 Agent、启用 Team、查看执行过程并安全管理实例和任务。

### 10.1 SSE 协议

- [ ] 定义 teamCreated/teamStateChanged 事件。
- [ ] 定义 instanceCreated/instanceStateChanged/instanceDeleted 事件。
- [ ] 定义 taskQueued/taskStarted/taskWaiting/taskResumed/taskCompleted/taskFailed/taskCancelled/taskTransferred 事件。
- [ ] 定义 messageCreated/messageConsumed 事件。
- [ ] 所有 Agent 事件携带 team event sequence、instance/task/attempt ID 和脱敏 payload。
- [ ] Coordinator 原有 reasoning/text/tool/usage 事件继续按到达顺序进入 assistant bubble。
- [ ] Worker stream 事件进入 Agent 面板，不混入主 assistant bubble。
- [ ] SSE 重连后通过 snapshot + event sequence 恢复，不重复展示已消费事件。

### 10.2 Runtime API

- [ ] 增加 chat Team 启用、读取、停止 API。
- [ ] 增加实例创建、暂停、恢复、drain、stop、reset 和删除 API。
- [ ] 增加 task 列表、详情、取消、重试和转移 API。
- [ ] 增加 message 列表和任务依赖查询 API。
- [ ] 所有 mutation API 使用明确 action endpoint 或严格 request enum，不使用含糊 patch。
- [ ] 所有 API 验证 workspace/chat/team 归属和操作时状态。
- [ ] API 返回 Provider/model 标签但不返回凭据或完整敏感 Prompt。

### 10.3 AgentDefinition 设置页

- [ ] 在全局设置中增加 Agents 页面。
- [ ] 支持创建、编辑、复制和删除 AgentDefinition。
- [ ] 支持选择 Provider、Model、模型参数、系统提示词和 allowed tools。
- [ ] 显示 definition ID、revision、maxInstances 和权限限制。
- [ ] 修改定义时明确提示不会影响既有实例。
- [ ] 删除仍被运行实例快照引用的定义时显示影响说明，但不伪造外键阻止。
- [ ] 使用 `lucide-react` 作为唯一图标来源。

### 10.4 Chat Team 入口

- [ ] 在 chat 中增加显式启用 Multi-Agent 的入口，并要求选择 Coordinator definition。
- [ ] Team 模式下模型选择器显示 Coordinator 模型来源，不允许静默覆盖。
- [ ] 用户发送消息后立即显示 queued/running 状态，即使 Coordinator 正忙。
- [ ] 保留原有附件、发送、停止生成和聊天标签交互。
- [ ] 全局设置、统计等全局视图不显示 Team runtime 面板。

### 10.5 Agent 面板

- [ ] 在现有右侧 Git/ToDo 交互模型中增加 Agents tab，避免破坏性布局漂移。
- [ ] 移动端通过底部覆盖面板展示 Agents，不常驻 workspace 侧边栏。
- [ ] 显示 Team 状态、全局并发使用量和 mutation lease owner。
- [ ] 按 Definition 分组显示实例、模型、状态、当前任务、队列长度和上下文 generation。
- [ ] 显示 task parent/dependency、result/error、attempt 和审计摘要。
- [ ] 显示 Agent 间消息与关联 task，但默认折叠完整私有上下文。
- [ ] 提供创建实例、暂停、恢复、取消、重试、转移、drain 和 stop 控件。
- [ ] 危险操作显示准确影响范围，不使用含糊的确认文案。

### 10.6 前端测试

- [ ] 将 AgentDefinition 设置测试放入独立 feature 级 `web/app-*.test.tsx` 文件。
- [ ] 将 Team 启用、实例列表、任务队列和控制操作测试拆成独立 feature 测试。
- [ ] 复用 `web/test-utils/app-test-harness.tsx` 的 fixture、mock fetch、stream controller 和 renderApp。
- [ ] 覆盖 SSE 乱序/重连、重复 event sequence 和 Worker 事件不进入主 bubble。
- [ ] 覆盖桌面右侧 tab 与移动端底部覆盖行为。
- [ ] 覆盖键盘操作、焦点、ARIA label、错误提示和 loading 状态。
- [ ] 避免把纯函数或单组件行为追加回巨型 App 测试文件。

### 阶段 10 退出条件

- [ ] 用户无需直接调用 API 即可完成 Agent 配置和 Team 日常管理。
- [ ] Coordinator、Worker、任务、消息和错误在 UI 中归属清晰。
- [ ] 新 UI 不破坏 workspace 侧边栏、聊天、Git/ToDo 和移动端现有交互。

---

## 阶段 11：安全、可观测性、性能与发布验证

### 目标

完成跨模块加固、端到端验证、文档同步和首版发布门槛。

### 11.1 安全与脱敏

- [ ] 审计 AgentDefinition 快照，确认不包含 Provider 密钥、密码 hash、cookie 或代理凭据。
- [ ] 对 Agent task/message/result/event/hook notification 执行敏感字段脱敏。
- [ ] 验证 URL 凭据、query、fragment 不进入 Provider 错误和 AgentEvent。
- [ ] 验证 Agent 工具无法跨 workspace、chat 或 team 查询和修改数据。
- [ ] 验证 Worker 的工具权限不能被 Prompt、Message 或 task payload 提升。
- [ ] 验证 hook 输入输出和 UI 通知不会因 Agent metadata 泄露敏感内容。

### 11.2 可观测性

- [ ] 记录 team/instance/task/attempt 生命周期 tracing span。
- [ ] 指标至少包括 queue length、queue wait、run duration、scheduler latency、token usage、tool duration、mutation lease wait 和失败分类。
- [ ] 指标标签避免使用 task ID、message 内容等高基数字段。
- [ ] UI 能从同一 LLM 审计记录展示 model、provider、耗时、输出速率和首 token 延迟。
- [ ] 日志写入现有 `logs/foco-YYYY-MM-DD.log`，不创建单独 Agent 日志体系。
- [ ] Scheduler 卡住、无 runnable task、lease 冲突和 reconciliation 结果具有可诊断日志。

### 11.3 性能与压力测试

- [ ] 测试大量 idle instance 不会各自占用永久 Tokio worker。
- [ ] 测试大量 queued task 的索引查询、领取和 event 写入性能。
- [ ] 测试 bounded wake channel 合并信号时不会遗漏 SQLite runnable task。
- [ ] 测试多个 workspace/team 的调度公平性。
- [ ] 测试长上下文、频繁消息和大量 task result 下的压缩与数据库大小增长。
- [ ] 在真实性能数据证明需要前，不引入 DashMap、flume 或 crossbeam。

### 11.4 故障注入

- [ ] 在 Provider stream、工具调用、任务完成事务和 SSE 发送各阶段注入失败。
- [ ] 在 task 已产生工具副作用但完成事务前模拟后端退出，确认任务标记 interrupted 且不自动重放。
- [ ] 在 waiting dependency 完成与 parent resume 之间模拟退出，确认恢复幂等。
- [ ] 在 transfer、cancel、retry 和实例 stop 并发发生时验证状态一致性。
- [ ] 在 mutation lease owner 异常结束时验证 lease 释放。
- [ ] 在 workspace 数据库迁移失败时验证备份和原库不被破坏。

### 11.5 文档与项目约定

- [ ] 更新根目录 `AGENTS.md`，记录新增模块、配置、运行时、工具、存储、API、UI 和验证约定。
- [ ] 记录 Multi-Agent 开发/调试方式和必要环境变量；不硬编码端口或配置目录。
- [ ] 记录 AgentDefinition、Team、Instance、Task 和 Message 的用户语义。
- [ ] 记录 shared workspace 模式的串行 mutation 限制。
- [ ] 记录 interrupted task 不会自动重试的原因和恢复流程。
- [ ] 记录首版明确不支持的广播、自动扩缩容、优先级和分布式运行。

### 11.6 分层验证命令

- [ ] 运行针对 store migration/repository 的 Rust 测试。
- [ ] 运行针对 AgentRunExecutor、Scheduler、等待恢复和 mutation lease 的 Rust 测试。
- [ ] 运行 app API/SSE 集成测试。
- [ ] 运行 `cargo fmt --all -- --check`。
- [ ] 运行 `cargo check --workspace`。
- [ ] 运行 `cargo test --workspace`。
- [ ] 运行 `npm run test -w web`。
- [ ] 运行 `npm run typecheck -w web`。
- [ ] 运行 `npm test` 完成全量验证。
- [ ] 运行 `npm run build:release`。
- [ ] 在 Windows release 条件具备时运行 `npm run test:release-smoke:windows`。
- [ ] 运行 diff/格式检查，确认无无关文件、生成物或敏感数据进入变更。

### 阶段 11 退出条件

- [ ] 所有阶段测试、全量测试、类型检查和 release 构建通过。
- [ ] 故障注入证明任务不会丢失、重复执行或静默覆盖 workspace 修改。
- [ ] 安全审计证明 Agent 配置、消息、事件、日志和 LLM 审计不泄露敏感字段。
- [ ] 文档、AGENTS 约定和实际实现一致。
- [ ] Multi-Agent 首版具备可发布、可恢复、可审计的完整闭环。

---

## 阶段 12：实例隔离 worktree（后续增强）

### 目标

在 shared workspace mutation 串行模式稳定后，支持多个 Agent 真正并行修改代码。

### 12.1 Execution workspace 抽象

- [ ] 为 AgentInstance 增加 execution workspace mode：shared 或 isolated worktree。
- [ ] 通过 Foco 内部 Git 能力管理 worktree，不依赖外部 git 命令。
- [ ] 创建实例时显式选择隔离模式，不自动从 shared 切换。
- [ ] 隔离 workspace 路径不得暴露 Windows `\\?\` verbatim 前缀给配置或 UI。
- [ ] code graph、文件工具、命令和 Git API 全部绑定实例 execution root。

### 12.2 Worktree 生命周期

- [ ] 创建隔离 worktree 时记录 base revision、分支和实例关联。
- [ ] 实例停止时不得静默删除存在未合并变更的 worktree。
- [ ] 提供状态、diff、保留、归档和显式删除操作。
- [ ] 删除前验证路径位于 Foco 管理目录，避免递归删除越界。
- [ ] 后端异常退出后能够发现并恢复孤立 worktree 元数据。

### 12.3 结果合并

- [ ] Worker 完成实现任务时返回结构化变更摘要、base revision 和 diff 标识。
- [ ] Coordinator 或用户显式发起合并。
- [ ] 合并冲突明确返回，不自动选择一方或覆盖 shared workspace。
- [ ] 合并前验证 shared workspace 当前 revision 与 Worker base revision 的关系。
- [ ] 合并后的 code graph、Git diff 和 UI 状态及时刷新。

### 12.4 安全与测试

- [ ] 覆盖两个 isolated instance 真正并行写入不同 worktree。
- [ ] 覆盖冲突、shared workspace 已变化、实例中断和 orphan recovery。
- [ ] 覆盖路径校验、Windows 路径规范化和内部目录 ignore 规则。
- [ ] 覆盖未合并变更不会被 stop/delete 静默清理。
- [ ] 更新 `AGENTS.md`、运行命令和 release smoke 验证说明。

### 阶段 12 退出条件

- [ ] 多实例可以在物理隔离的工作目录中并行修改代码。
- [ ] 所有合并和删除操作均由用户或 Agent 显式发起且可审计。
- [ ] shared workspace 模式继续可用，并保持其原有安全语义。

---

## 6. 首版发布范围

首版必须完成阶段 0 至阶段 11。阶段 12 属于后续增强，不阻塞首版发布。

首版完成定义：

- [ ] 可以配置多个使用不同 Provider/Model/System Prompt 的 AgentDefinition。
- [ ] 可以在一个 chat 中创建 Team、Coordinator 和多个 Worker 实例。
- [ ] 同实例任务严格串行，不同实例受全局限制并发。
- [ ] 可以发送消息、委派、等待、取消、重试和转移 queued task。
- [ ] 后端重启不丢 queued/waiting 数据，不自动重放 interrupted task。
- [ ] 主聊天、Worker 私有上下文、SSE 和审计归属明确。
- [ ] shared workspace 修改有 mutation lease，不出现跨实例并发写入。
- [ ] UI、API、数据库迁移、安全、测试和文档形成完整闭环。

## 7. 实施过程中的强制评审点

- [x] 阶段 0 后评审状态机与 single-agent 兼容策略。
- [x] 阶段 2 后评审 schema、删除语义、迁移和审计保留。
- [x] 阶段 3 后评审 AgentRunExecutor 是否真正消除执行循环复制。
- [x] 阶段 4 后进行第一次端到端垂直切片演示。
- [ ] 阶段 7 后专项评审等待恢复、Provider tool protocol 和死锁检测。
- [ ] 阶段 9 后专项评审 command/MCP 副作用与 mutation lease。
- [ ] 阶段 10 后进行桌面、移动端和 SSE 重连体验评审。
- [ ] 阶段 11 后完成安全、故障注入和发布评审。
