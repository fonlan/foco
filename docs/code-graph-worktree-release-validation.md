# Plan Worktree Code Graph Release Validation

本文件记录隔离 worktree Code Graph 预热的发布前测量方法。测量目标是确认预热不会阻塞首轮 LLM，
Graph 工具只会在索引初始化期间等待，并且多个 execution root 同时预热不会超过进程级上限。

## Release 性能样本

在干净工作树中运行：

```text
cargo test -p foco-graph semantic_fixture_release_performance --release -- --ignored --nocapture
cargo test -p foco-app release_execution_root_prewarm_reports_trigger_to_ready --release -- --ignored --nocapture
```

该测试对固定的中等 Rust fixture 和由其派生的大型临时 execution root 执行：

1. 冷启动索引。
2. 单文件增量更新。
3. resolver-only pass。
4. caller/callee SQLite 查询。

输出的 `code_graph_release_benchmark` 字段如下：

| 字段 | 含义 |
| --- | --- |
| `files` / `bytes` | 扫描文件数与输入字节数。 |
| `cold_index_ms` | 从冷索引开始到图谱 SQLite 可查询的总时间，不含 watcher 启动。 |
| `cold_file_prepare_us` | 读文件、哈希、语言识别和解析/抽取耗时。 |
| `cold_sqlite_persistence_us` | 图谱 SQLite 批写和 stale cleanup 耗时。 |
| `cold_resolver_us` | 跨文件 import/resolution 耗时。 |
| `incremental_*` | 修改单个文件后的相同分项。 |
| `resolver_only_ms` / `*_query_ms` | resolver-only 与典型查询成本。 |

不使用跨机器时间阈值。将相同机器、相同 checkout 的 `medium` 和 `large` 输出与发布候选或后续
实现比较，关注回归而非绝对值。第二条命令会真实调用 execution-root 预热、启动 watcher 并等待
registry `Ready`，输出 `code_graph_execution_root_release_benchmark` 的 `prewarm_to_ready_ms`；这才是
从预热触发到 watcher Ready 的端到端样本。

## 预热、并发和取消回归

运行聚焦覆盖：

```text
cargo test -p foco-app code_graph:: -- --nocapture
cargo test -p foco-app worktree_code_graph -- --nocapture
cargo test -p foco-app remote_sidecar_isolated_worktree_code_graph_indexes_execution_root_not_canonical -- --nocapture
cargo test -p foco-app remote_finalize_fast_forward_releases_graph_and_worktree -- --nocapture
```

`agent_scheduler` 在确定 `tool_workspace_path` 后只调用异步预热函数，不等待索引完成；因此首轮
LLM 与预热并行。Graph 工具通过 readiness gate 等待同一 execution root，短轮询会检查 deadline
和 cancellation token。`init_gate_never_grants_more_than_configured_permits` 验证活跃初始化数不超过
内部常量，且 queued work 不持有 workspace DB permit。

在 release 或手工运行中，采集结构化日志的下列字段：

```text
code graph execution-root initialization started: queue_wait_ms (仅进程级 init gate 等待)
code graph execution-root initialization completed: prewarm_to_ready_ms
initialized code graph index: file_prepare_duration_us sqlite_persistence_duration_us resolver_duration_us
```

这些日志只记录计数、耗时、execution root 路径和调用标签，不记录源码或数据库内容。绝对路径来自
既有 execution-root 诊断；发布采集时不应将完整日志上传到不受信任的服务。

## 发布级检查

```text
cargo fmt --all -- --check
cargo test -p foco-graph
cargo clippy -p foco-app -p foco-graph --all-targets --all-features --no-deps -- -D warnings
git diff --check
```

本验证不改变 HTTP、SSE、前端或公开工具 schema，也不引入 SQLite migration。若 release 数据证明
全量预热仍不可接受，后续研究仅可复制 Code Graph 专属表并在写入前校验 content hash；不得复制整个
workspace SQLite 或聊天、Plan、Spec、Todo 数据。
