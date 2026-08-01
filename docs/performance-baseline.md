# 持续 CPU 与能耗基线采集

本文件是「降低 Foco 持续 CPU 与能耗占用」的 Phase 1 测量协议。它用于先分离后端、浏览器渲染进程和短命子进程的成本，再决定是否修改代码图 watcher、前端 streaming、调度器或命令监控。不得把 `cargo`、Vite、Node、`rg` 或 Plan build 子进程的 CPU 合并计入 `foco` 后端。

采集只写入工作区 `.foco/perf-baselines/`，该目录不应提交。结果文件不包含源码、聊天内容、工具输入/输出、环境变量或凭据。

## 固定前提

1. 使用同一 checkout、同一台机器、接电状态和同一浏览器版本；传入 `--browser-version`。采集器会记录 macOS 版本、芯片、内存、浏览器版本和提交 SHA 到 run 的 `metadata.txt`。
2. 通过 `npm run build:release` 构建；后端必须由 `./target/release/foco` 启动，而不是 `cargo run`。浏览器使用正常的 release web 资源，关闭 DevTools 录制以外的扩展。
3. 每个场景单独运行一次 5 分钟的进程采样；需要稳定结论时运行 3 次，报告中位数，并保留每次原始 CSV。
4. 启动采样前用 `ps -axo pid,ppid,%cpu,command` 记录树。传给采集脚本的是 `foco` 后端 PID 与浏览器 renderer PID；脚本会在每个样本把子进程单列，避免归因混淆。

```text
zsh scripts/perf/capture-macos-process-baseline.zsh \
  --name idle-prewarmed --pids <foco-pid>,<chrome-renderer-pid> \
  --duration-seconds 300 --browser-version "Chrome 140.0.0.0" --powermetrics
```

`process-samples.csv` 保存累计 CPU time；采集器按相邻样本的 CPU-time delta 生成 `cpu-interval-samples.csv`，再产生每 PID 的平均/P95 CPU `process-summary.csv` 和根及其存活子进程合计的 `group-summary.csv`。进程首次出现的样本没有前一时点，故不参与 CPU 百分比计算。`powermetrics-tasks.txt` 使用 `tasks` + `--show-process-energy`，提供同一窗口的 macOS task wakeup/process-energy 代理；它是系统级输出，应按 PID/进程名与 CSV 对照，不能把无关任务归给 Foco。

Rust CPU flame profile 使用 `samply record -- ./target/release/foco ...`，或 Instruments 的 Time Profiler；浏览器在对应 renderer 的 Foco 标签页单独录 Chrome Performance 和 React Profiler。把二者的导出文件置于本次 run 目录，不上传含可能页面数据的 trace。

## 四个必须场景

| 场景 | 预处理与动作 | 结束时必须保留的工件 |
| --- | --- | --- |
| `idle-prewarmed` | 打开目标 workspace，等初始图谱日志和 execution root 预热完成后不操作 5 分钟；工作区保留 `target/`、`node_modules/`。 | 进程 CSV、powermetrics、后端 samply/Instruments trace。 |
| `chat-reasoning` | 用固定模型和固定提示启动一轮会产生 streaming reasoning 的聊天；等待完成。浏览器只录 Foco 标签页。 | 进程 CSV、Chrome Performance、React Profiler commit 导出、后端 trace。 |
| `source-save-burst` | 对一个已索引源码文件连续修改并保存 10 次（间隔 200ms），之后等待 3 秒让 debounce 收敛。 | 进程 CSV、代码图结构化日志摘录、后端 trace。 |
| `plan-two-roots-build` | 创建两个 execution root，启动各自的构建，并持续到两者结束；分别记录 Foco、构建工具和其子进程。 | 进程 CSV、代码图日志、Plan/后端 trace。 |

浏览器 profile 需标记从「发送/保存/启动构建」到结束的用户时间线。React Profiler 报告 `commit count`、最大 commit 时长、P95 commit 时长，并把 Markdown/消息列表相关 flame 作为独立截图或导出保存。

## 代码图诊断字段

Phase 1 为初始索引和 watcher refresh 增加了有界结构化字段。普通保存后过滤日志中 `code graph watcher refreshed workspace index`，必须记录：

```text
index_scope index_reason scanned_files indexed_files unchanged_files
file_prepare_duration_us sqlite_persistence_duration_us resolver_duration_us
watcher_events_received watcher_relevant_events watcher_filtered_events
watcher_debounce_resets watcher_receive_timeouts watcher_refreshes
```

目前 refresh 的 `index_scope` 明确为 `full_workspace`，因此一次普通保存如果出现 `scanned_files` 接近 workspace 全量文件数，即已直接证实全工作区扫描。`watch_event_queue=std_mpsc_unbounded` 与 `watch_event_queue_overflow_observable=false` 也明确当前通道没有可观测的容量溢出；Phase 1 不为了采集数据而改变队列语义。watcher 停止时仅再写一条累计计数日志，空闲的 100ms receive timeout 不会逐次写日志。

## 归因表与冻结阈值

每个 run 在 `metadata.txt` 旁创建下面的表。`before` 必须填本机实测值，不能以开发构建、`cargo`/Vite CPU 或猜测代替；`target` 是后续阶段的验收上限。未进入 Time Profiler/Chrome top hotspot 的行维持 `not selected`，不得据此重构。

| 分类 | 证据 | before | 后续 target |
| --- | --- | --- | --- |
| 代码图扫描/解析 | release trace + `file_prepare_duration_us` 与 `scanned_files` | 实测平均/P95 CPU、每次索引分项 | 同场景 P95 CPU 不高于 before；普通保存不应再无证据触发全量扫描。 |
| SQLite/resolver | release trace + SQLite/resolver 分项 | 实测每次分项和 P95 CPU | 不高于 before；只有 profile 证明其为 top hotspot 才改。 |
| 前端 render/Markdown | Chrome Performance + React Profiler | commit count、P95 commit、renderer P95 CPU | 不高于 before；仅 profile 证明后改计时/渲染。 |
| 调度器扫描 | 后端 trace + scheduler wake/scan 日志 | 平均/P95 CPU、每分钟 scan 数 | 不高于 before；保持现有恢复 deadline 语义。 |
| 命令监控 | 后端 trace + 子进程 CSV | Foco 与真实子进程各自平均/P95 CPU | 不把子进程计入 Foco；仅 profile 证明后调整轮询。 |

能耗代理是同一窗口中 `powermetrics` 的 task CPU wakeups 和 energy impact；报告必须注明它不是跨机器可比较的焦耳读数。若 `powermetrics` 无权限，run 标记为 `energy_proxy=not_collected`，不得用估算值补填。

## 图谱 release 微基线

除端到端场景外，保持以下 release microbenchmark 作为 graph 分项回归参考：

```text
cargo test -p foco-graph semantic_fixture_release_performance --release -- --ignored --nocapture
cargo test -p foco-app release_execution_root_prewarm_reports_trigger_to_ready --release -- --ignored --nocapture
```

保留输出中的 `code_graph_release_benchmark` 和 `code_graph_execution_root_release_benchmark` 行。它们不是浏览器/能耗替代品，而是用来解释端到端 profile 中 code graph 的扫描、SQLite 和 resolver 成本。

### 本工作树的首个 release 图谱样本

2026-08-01 在本 worktree 运行第一条命令得到如下可复跑的微基线（单次，故不作为跨机器阈值）：

| fixture | files / bytes | cold index | file prepare | SQLite persistence | resolver | 单文件增量 |
| --- | --- | --- | --- | --- | --- | --- |
| medium | 3 / 2,690 | 80 ms | 2,961 μs | 3,485 μs | 2,164 μs | 8 ms |
| large | 35 / 107,544 | 100 ms | 16,170 μs | 37,534 μs | 4,030 μs | 12 ms |

两组增量样本均 `indexed_files=1`；medium 的增量 prepare/SQLite/resolver 分别为
103/2,073/2,021 μs，large 为 614/2,920/3,627 μs。该样本证明基准命令、release
构建和分项输出可用；它不包含浏览器 renderer、真实闲置 5 分钟、`powermetrics` 或用户模型
调用，四场景进程/能耗基线仍须按本文件的受控现场流程采集，不能伪造或从微基准外推。
