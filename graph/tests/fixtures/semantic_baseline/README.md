# 语义回归基线夹具

这些 fixture 固定“当前行为”和“目标行为”之间的差异，供后续 Rust、TypeScript/TSX/JavaScript 抽取器与模块解析器平滑切换。它们不是大型快照：测试应断言精确 edge kind、端点、metadata、候选集和查询结果。

| Fixture | 覆盖 | 当前基线 | 目标语义 |
| --- | --- | --- | --- |
| `rust_workspace` | 局部函数调用、变量读取、同名函数、嵌套遮蔽、`use ... as` alias、跨文件 module、ERROR 文件 | 同文件 identifier 同名匹配产生 `references`；同名可误连；`use` 只保存 import 行；ERROR 文件无抽取结果 | 可验证局部调用为 `calls/tree_sitter/exact`；遮蔽精确绑定；alias 经 module resolver 连接或显式 unresolved/candidate。 |
| `typescript_workspace` | `.ts` import alias、re-export、`.tsx` 消费者与跨文件 module 路径 | import/re-export 只保存 import 行，不产生跨文件 edge | imports 和 re-export 可表示 `imports/module_resolver`，调用按 resolution 写 `calls` 或 candidate/unresolved。 |
| `python_workspace` | Python class/function/import/direct call | 专用 Python extractor 的 stable IR golden 与 SQLite 持久化 | 覆盖 `from ... import ... as ...`、qualified name、calls、位置。 |
| `go_workspace` | Go package/import/function/method/type/direct call | 专用 Go extractor 的 stable IR golden 与 SQLite 持久化 | selector call 仅保留未解析 reference，不猜测 receiver 类型。 |
| `performance_rust_workspace` | 固定中等规模 Rust 源码、连续局部调用和多个模块 | 可用于观察全量与增量索引、file prepare（读/哈希/识别/抽取）、SQLite persistence、resolver 与 caller/callee 查询成本 | release 测试还会从它派生 32 个重命名模块作为大型临时 execution root；不设机器相关时间阈值。 |

手工运行 release 性能样本：

```text
cargo test -p foco-graph semantic_fixture_release_performance --release -- --ignored --nocapture
```

测试会输出一行 `code_graph_release_benchmark`，分别包含 `medium` 与 `large` 样本。记录
`files`、`bytes`、冷启动 `cold_index_ms`、file prepare（读/哈希/识别/抽取）、SQLite
持久化、resolver、增量索引和 caller/callee 查询耗时。这里的临时 execution root 与隔离
worktree 使用相同的图谱数据库布局（`<root>/.foco/foco.sqlite`）；该样本不启动 watcher，
因此 `cold_index_ms` 只代表 SQLite 图谱可查询前的索引时间。Git 元数据不参与 graph 索引，
因此不把它纳入性能变量。

运行 Foco App 的 release 验证时，结构化日志中的 `code graph execution-root initialization`
事件另外提供 `queue_wait_ms` 和 `prewarm_to_ready_ms`，前者覆盖进程级并发 gate 排队时间，后者
覆盖从预热触发到 watcher Ready 的总时间。若 medium/large 的实际数据表明全量预热不可接受，应
另开计划评估“仅复制 Code Graph 专属表 + content-hash 校正”；本方案不复制 workspace SQLite，
也不复制 Chat、Plan、Spec 或 Todo 数据。

未来改动应更新测试中的明确预期，而不是删除当前基线。候选结果必须带 provenance/confidence；无法唯一解析的跨文件目标必须保留为 unresolved/candidate，不能猜测为 exact。
