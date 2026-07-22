# 代码图谱语义升级：影响盘点与变更矩阵

状态：Phase 1 基线盘点。本文记录当前调用链和后续将 edge 语义从“同名 identifier”收敛为可信调用、引用与跨文件模块关系时的受影响边界。

## 当前端到端链路

| 环节 | 当前实现 | 语义升级影响 |
| --- | --- | --- |
| 文件发现与增量准备 | `graph/lib.rs::index_workspace` 发现文件、读取内容、按 hash 跳过未变文件 | 模块解析可能需要 workspace 级上下文；仍须在普通数据库 permit 外解析，不能把整轮扫描包进数据库连接。 |
| Tree-sitter 抽取 | `extract_file` → `collect_symbols_and_imports` → `collect_references` | 将拆分为按语言的局部抽取器；当前 identifier 名称匹配仅保留为明确标注的 legacy/heuristic 基线。 |
| SQLite replace transaction | `WorkspaceDatabase::replace_code_graph_file_index` 先清单文件旧行，再写 hash、parse status、symbols、imports、references、edges 与 FTS | schema 新列/索引和跨文件 resolver 输出必须在同一 replace/协调 transaction 中保持一致；不得留下指向已删除 symbol 的 edge。 |
| 查询 API | `find_code_graph_symbols`、`code_graph_callers`、`code_graph_callees`、`code_graph_references`、`code_graph_related_files` | callers/callees 当前查全部 edge；未来必须强制 `edge.edge_kind = 'calls'`，并为 imports/candidate 关系提供明确查询语义。 |
| Agent 工具 JSON | `tools/graph_tools.rs` 将 store records 转为 JSON，并经过统一输出预算 | 可新增可选 metadata/confidence 字段；不应替换或重命名已有 `edgeKind`、source、target、`truncated` 等字段。 |
| 本地初始化与 watcher | `app/runtime/code_graph.rs` 在最近 7 天活跃本地 workspace 后台初始化；首次聊天 prompt assembly 也可懒初始化；`foco_graph::start_code_graph_watcher` 维护 watcher | schema/抽取升级须支持冷 workspace 初始化、增量 watcher 更新和失效文件删除；不可仅在启动全量路径验证。 |
| 远程 SSH sidecar | `app/remote_workspace.rs::classify_tool_route` 将全部 graph 工具标为 `SidecarLocal`；首次远端图谱工具请求会经 `ensure_sidecar_code_graph` 同步执行 `index_workspace` 并启动 `start_code_graph_watcher`，随后 sidecar 在远端 workspace 使用同一工具与 workspace DB | graph 抽取、migrations 和查询必须随 sidecar 生效；迁移要覆盖首次初始化、watcher 续跑和已初始化热路径；主进程不能以本地图谱回退代替远端结果。 |

## 当前 callers/callees 核对

`WorkspaceDatabase::code_graph_callers` 的条件仅为 `edge.target_symbol_id = ?1`；`code_graph_callees` 仅为 `edge.source_symbol_id = ?1`。二者都调用 `code_graph_symbol_relations`，其 SQL 没有 `edge_kind` 过滤。

因此当前 `graph_find_callers`/`graph_find_callees` 不是调用图查询：它们会暴露所有 `references` 边。后续 SQL 收敛的最小约束是：

```sql
WHERE edge.target_symbol_id = ?1 AND edge.edge_kind = 'calls'
```

或相应的 `source_symbol_id` 条件。该改变必须与 calls 抽取器、索引和工具回归测试同一阶段提交，不能单独过滤导致工具突然返回空结果。

## 数据库与迁移边界

- 当前 workspace schema 版本为 **43**：`store/workspace.rs::MIGRATIONS` 的最后一项是 `MIGRATION_043`。图谱基础表在 `MIGRATION_001`：`code_graph_files`、`code_graph_symbols`、`code_graph_edges`、`code_graph_references`、`code_graph_imports`；v43 为 symbols 增加 qualified/visibility/metadata 并清空旧图谱，FTS 在后续 migration 建立。
- 迁移入口在 `WorkspaceDatabase::open_or_create` 的 workspace migration 流程，受 workspace database gate 管理。图谱工具、索引器和远程 sidecar 都经此入口打开 workspace DB；不得 raw-open 绕过 gate 或迁移锁。
- `graph/lib.rs::index_workspace` 仅短暂打开数据库以读取 hash、批量替换（默认 16 文件）和删除 stale 文件。这一“解析 permit 外、短事务写入”的约束应保持。
- `replace_code_graph_file_index` 目前用 transaction 维护文件级 replace。若跨文件 edge 不再只属于源文件，需要在设计中定义 source-file 删除、target-file 重建、module resolver 重跑时的失效策略，避免残留边。

## 测试与消费者影响

| 范围 | 已有/受影响验证 | 后续必须补充 |
| --- | --- | --- |
| 图谱抽取 | `graph/lib.rs` 单测覆盖增量索引、删除、语言解析和数据库 gate | 使用 `graph/tests/fixtures/semantic_baseline` 固定 Rust、TS/TSX、ERROR、遮蔽和 alias/re-export 基线。 |
| Store | `store/tests/workspace_database.rs` 覆盖 migration 与 workspace DB 行为 | 新 schema migration、metadata JSON 默认/校验、edge-kind 查询索引和跨文件失效 transaction。 |
| 工具 | `tools/graph_tools.rs` 的查询、ambiguous symbol 和输出预算逻辑 | callers/callees 只消费 `calls`；references 与 candidate/unresolved metadata 的 JSON 兼容和软限。 |
| App 本地 | `app/runtime/code_graph.rs` 与 `app/tests/mod.rs` 覆盖活跃 workspace 与懒初始化 | watcher 修改、初始化失败、schema 升级后的冷/热 workspace 重建。 |
| App 远程 | `app/remote_workspace.rs` 的 route 分类和 `ensure_sidecar_code_graph` 首次触发索引/watcher 路径 | sidecar migration、首次图谱请求的 index + watcher、远程多文件图谱查询以及主进程不回退本地图谱。 |
| Spec / Dream 消费者 | `app/spec_runtime.rs` 消费 file/symbol 摘要；Dream 也写入图谱 fixture 数据 | 新 metadata 不应破坏现有 summary；若要消费可信影响关系，必须选择性接入而非假定全部 edge 都是 calls。 |

## 分阶段实施顺序

1. Phase 1：本契约、影响矩阵与回归夹具；不改变索引结果。
2. Schema：为 edge provenance/confidence、unresolved/candidate 表达与 `calls` 查询索引设计 migration，并保留 legacy 数据兼容读取。
3. 抽取器：先 Rust、TypeScript/TSX/JavaScript 的 call/import 局部抽取，再接入 module resolver；Python/Go 后续扩展。
4. 查询：待 `calls` 数据可靠后同时收紧 callers/callees SQL，补齐 `references`、imports 和影响分析的语义输出。
5. 远程与性能：在 sidecar workspace 执行相同测试矩阵；以固定 release fixture 观察吞吐和数据库写入成本，不引入机器相关阈值。

## 非回归约束

- 不改变 Agent 工具输出预算和既有分页/截断规则。
- 不在前端做 Tree-sitter、模块解析或跨文件影响计算。
- 不把同名、遮蔽、方法重名等候选结果升级为确定调用。
- 不只修本地：远程 sidecar 的 workspace-local 图谱路径必须同步验证。
