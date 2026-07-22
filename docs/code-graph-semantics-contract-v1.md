# 代码图谱语义契约 v1

状态：已接受（Phase 1 基线）

本文冻结 Foco 代码图谱从当前 Tree-sitter 名称匹配实现迁移到语义边模型时的对外含义、置信度表达和兼容边界。它是后续 schema、抽取器和查询改造的依据；**本文不会改变当前索引结果或工具 JSON**。

## 目标与非目标

代码图谱服务于 Agent 的局部导航和影响分析：定位定义、解释引用、寻找可信 callers/callees，以及发现跨文件关联。它不是面向可视化的“全宇宙依赖图”。

本轮及后续演进保持计算和持久化在 Rust 后端与 workspace SQLite 中完成。明确**不采用 CodeGraph 的 N-API / flat-buffer ABI**：该 ABI 解决 TypeScript 与 Rust 间高频数据传输的问题；Foco 没有对应的 JS/Rust 图谱计算热路径，加入一层内部序列化协议只会增加维护、版本协调和故障面。前端只消费现有工具/API 返回的结构化记录，不承担图谱重计算。

第一优先级语言是 Rust、TypeScript、TSX 与 JavaScript；Python、Go 在共享模型稳定后扩展。语言不支持或解析失败时，系统必须保留可诊断状态，而不是伪造语义关系。

## 位置与标识约定

- `path`：相对 workspace 根的规范化、斜杠分隔路径；不得是机器绝对路径。
- 所有 `startLine`、`startColumn`、`endLine`、`endColumn` 均为 Tree-sitter `Point` 直接转换，**从 0 开始**，并采用半开区间 `[start, end)`。
- symbol 的稳定数据库主键可在一次 replace transaction 后变化；跨索引、跨会话引用应使用 `path + kind + name + start/end position` 作为定位键，而不能把整数 ID 当作持久外部身份。
- 空位置表示语法节点没有可可靠映射的位置，不能用 `0` 伪装为文件首字符。

## 节点模型

当前核心节点为：

| 节点 | 语义 | 当前存储 |
| --- | --- | --- |
| `file` | 被发现、已知语言或已记录解析状态的 workspace 文件 | `code_graph_files` |
| `symbol` | 由语言抽取器识别的声明，例如 function、method、class、variable | `code_graph_symbols` |
| `reference` | 源码中的一个名称使用位置；可暂时没有目标 symbol | `code_graph_references` |

后续可扩展 module、package、external-symbol 等逻辑节点，但不要求为了表达 unresolved 目标而预先落库一个虚假的 `symbol`。

## Edge kind 稳定集合

`code_graph_edges.edge_kind` 从 v1 起使用以下小集合。值为小写 ASCII 字符串；消费者遇到未知值必须保留记录并按“未知关系”显示，不能把它转为 `calls`。

| edge kind | source → target | 含义 | 何时可声明 |
| --- | --- | --- | --- |
| `contains` | file/module/symbol → contained symbol | 结构性归属，不代表运行时或名称解析 | 父子语法结构可精确确定时 |
| `calls` | 可调用 symbol → 被调用 symbol | 一个 call/new expression 经解析后指向目标 callable | 语法调用成立；目标为局部唯一绑定时是 `exact`，关联/成员近似必须标为 `heuristic` |
| `references` | symbol/reference owner → 被引用 symbol | 非调用名称使用，例如变量、类型、常量或保守的名称绑定 | 已能表示引用，且目标精度符合 metadata 声明时 |
| `imports` | file/module → module 或 imported symbol | 源文件声明了 import/use/export-from/re-export 依赖 | 语法 import 可识别；目标 module 可暂时 unresolved |

为未来预留但本阶段不生成的 kind：`extends`、`implements`、`type_of`、`returns`。新增 kind 必须先更新本文、迁移策略和工具兼容测试。

## 证据、来源与置信度 metadata

每条目标模型 edge 的 `metadata_json` 必须是 JSON object，最小契约如下。字段可新增，已定义字段不得改变含义。

```json
{
  "semanticVersion": 1,
  "provenance": "tree_sitter",
  "confidence": "exact",
  "resolution": {
    "status": "resolved",
    "candidates": []
  }
}
```

`provenance` 枚举：

- `tree_sitter`：仅由当前文件的语法树与可验证局部结构得出。
- `module_resolver`：由语言模块规则、import/export/re-export 路径解析得出。
- `heuristic`：有意保留的近似推断；必须配合非 `exact` 置信度。

`confidence` 枚举：

- `exact`：有唯一、可验证目标。
- `candidate`：存在有限候选集，尚不能唯一绑定。
- `heuristic`：基于名称、目录或其他非语义证据推断。

`resolution.status` 为 `resolved`、`candidate` 或 `unresolved`。`candidate` 时 `candidates` 必须列出稳定定位键；`unresolved` 时可提供 `unresolvedTarget`，例如：

```json
{
  "semanticVersion": 1,
  "provenance": "module_resolver",
  "confidence": "candidate",
  "resolution": {
    "status": "unresolved",
    "candidates": [],
    "unresolvedTarget": {
      "kind": "module",
      "specifier": "./format",
      "fromPath": "src/index.ts"
    }
  }
}
```

不确定性是图谱数据的一部分：解析器不得将同名、遮蔽、方法同名或外部依赖猜测为 `exact`。

## 历史兼容基线（schema v42 及以前）

截至 workspace schema v42，`graph/lib.rs` 会收集当前文件的 symbol，再把 identifier 文本按名称映射到一个同文件 symbol。若 identifier 不等于该目标的声明名称位置，它会创建 `code_graph_references` 行，并在有包含 symbol 时写入 `edge_kind = "references"`。

这意味着当前结果具有以下限制：

- 同名 symbol 会在名称映射中互相覆盖，引用可能指向后收集到的声明。
- 嵌套作用域遮蔽未建模，外层/内层同名变量或函数可能误连。
- method 同名、字段访问、宏与语言特定调用语法未经过调用上下文判定。
- import 行已保存，但不会解析到其他文件 symbol；因此不存在可信跨文件 edge。
- parser 有 `ERROR` 节点时当前文件在 `code_graph_parse_status.status` 中标记为 `error`，且不写 symbol、import、reference 或 edge。
- 当前边 metadata 是空对象 `{}`，没有 provenance 或 confidence，因而不能被解读为 exact。

这些限制是历史行为，不是 v1 语义承诺。schema v43 会清空旧 code graph 文件、hash、FTS 数据和边，令下一次惰性索引以新提取器重建，绝不混用旧 `references` 调用近似与新 `calls`。`graph/tests/fixtures/semantic_baseline` 保留可复现样本。

## 当前实现（schema v43）

- Rust、TypeScript、TSX 与 JavaScript 使用专用 Tree-sitter walker；有 `ERROR` 的文件维持安全策略：仅记录 parse status，不写部分事实。
- symbols 持久化 `qualified_name`、`visibility` 与 `metadata_json`；局部声明以词法 scope 解析，遮蔽局部变量会阻止生成错误的强调用边。
- `calls` 仅从 Rust `call_expression`/`method_call_expression` 与 TS-family `call_expression`/`new_expression` 生成。非调用属性访问与普通变量读取不生成 calls。
- Rust `use`、TS-family named/default/namespace import 及 `export ... from` 会写入 imports 的 module、imported_symbol、alias；尚未解析的跨文件目标不会凭名称产生 calls。
- 当前文件内可验证的调用写 `tree_sitter`/`exact`；关联函数和成员调用的有限近似写 `heuristic`/`heuristic`。两者都必须在 metadata 中公开 provenance、confidence 与 resolution。

## 查询兼容策略

在 schema v43，`graph_find_callers` 与 `graph_find_callees` 只查询 `edge_kind = 'calls'`，并把关系表述为静态 call-site approximation，而非运行时追踪。

迁移策略：

1. schema v43 migration 先清除历史图谱，再由既有 lazy/background 初始化按需重建。
2. callers/callees 的 SQL 必须限定 `edge.edge_kind = 'calls'`，工具文档和结果都说明其为静态近似。
3. `graph_find_references` 保持 occurrence 查询；不能重新把 callers/callees 退化为全部边。
4. `graph_find_children` 仅返回一层 `contains` 子节点，支持 kind 过滤，不递归展开。
5. `graph_related_files` 的 caller/callee 分支只基于 calls；共享 import 保持独立 relation。

工具的字段形状保持向后兼容。后续若需要输出 edge metadata，应新增可选字段；旧客户端不依赖它时仍可读取原有 `edgeKind`、source 和 target。

## 迁移不变量

- workspace SQLite 仍是真源；文件 replace 必须在同一 transaction 中清理并重写该文件的图谱行。
- `metadata_json` 保持合法 JSON object，写入时使用显式默认值，不接受任意标量。
- 跨文件 unresolved target 不得用错误的本地 symbol ID 占位；以 metadata 的 `unresolvedTarget`/`candidates` 表达。
- 局部 Tree-sitter 抽取与模块解析可以分阶段运行，但任何写入必须携带其来源与置信度。
- 当前工具输出预算、数据库 ordinary gate 和远程 sidecar 的 workspace-local 运行模型不因语义升级而放宽。

## 验证基线

Phase 1 的夹具覆盖 Rust 与 TypeScript/TSX 多文件 module 场景、函数调用、变量读取、同名 symbol、嵌套遮蔽、import alias、re-export 以及含 `ERROR` 的文件。固定中等规模 Rust 样本仅用于 release 手工基准，不设置机器相关的耗时阈值。

后续实现应优先以精确断言验证 edge kind、端点、metadata provenance/confidence 与候选集合；避免用大规模快照掩盖语义变化。
