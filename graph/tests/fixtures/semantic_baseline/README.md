# 语义回归基线夹具

这些 fixture 固定“当前行为”和“目标行为”之间的差异，供后续 Rust、TypeScript/TSX/JavaScript 抽取器与模块解析器平滑切换。它们不是大型快照：测试应断言精确 edge kind、端点、metadata、候选集和查询结果。

| Fixture | 覆盖 | 当前基线 | 目标语义 |
| --- | --- | --- | --- |
| `rust_workspace` | 局部函数调用、变量读取、同名函数、嵌套遮蔽、`use ... as` alias、跨文件 module、ERROR 文件 | 同文件 identifier 同名匹配产生 `references`；同名可误连；`use` 只保存 import 行；ERROR 文件无抽取结果 | 可验证局部调用为 `calls/tree_sitter/exact`；遮蔽精确绑定；alias 经 module resolver 连接或显式 unresolved/candidate。 |
| `typescript_workspace` | `.ts` import alias、re-export、`.tsx` 消费者与跨文件 module 路径 | import/re-export 只保存 import 行，不产生跨文件 edge | imports 和 re-export 可表示 `imports/module_resolver`，调用按 resolution 写 `calls` 或 candidate/unresolved。 |
| `performance_rust_workspace` | 固定中等规模 Rust 源码、连续局部调用和多个模块 | 可用于观察当前索引吞吐和 SQLite 写入成本 | 用作后续 release 比较样本，不设机器相关时间阈值。 |

手工运行性能样本：

```text
cargo test -p foco-graph semantic_fixture_performance_baseline --release -- --ignored --nocapture
```

未来改动应更新测试中的明确预期，而不是删除当前基线。候选结果必须带 provenance/confidence；无法唯一解析的跨文件目标必须保留为 unresolved/candidate，不能猜测为 exact。
