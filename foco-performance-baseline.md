# Foco 性能优化基线记录（阶段 0）

> 记录时间：2026-06-25 · 分支 `main` @ `86205d4` · `npm run build -w web`（Vite + Rolldown）

本文件为优化前基线，供阶段 1-8 完成后对比。

## 1. 构建产物总量

| 类别 | 数量 | raw | gzip |
| --- | --- | --- | --- |
| JS | 85 | 17623.3 kB | 4295.3 kB |
| CSS | 3 | 294.0 kB | 52.7 kB |
| 其他（svg/字体等） | 60 | 1166.9 kB | — |
| **合计** | **148** | **19084.3 kB** | **4348.0 kB（JS+CSS）** |

## 2. 关键 vendor chunk 体积

| chunk | raw | gzip |
| --- | --- | --- |
| `vendor-monaco-*.js` | 4084.9 kB | 1034.5 kB |
| `vendor-monaco-*.css` | 142.6 kB | 22.1 kB |
| `vendor-markdown-*.js` | 418.7 kB | 124.3 kB |
| `vendor-charts-*.js` | 377.7 kB | 109.4 kB |
| `vendor-terminal-*.js` | 333.6 kB | 84.2 kB |
| `vendor-terminal-*.css` | 3.8 kB | 1.0 kB |
| `vendor-react-*.js`（三段合计） | 174.3 kB | 54.8 kB |

> 入口 `index-*.js` 638.9 kB / gzip 140.9 kB；另有 mermaid 多个 diagram 分包、`cytoscape.esm` 435 kB、`chunk-NNHCCRGN` 593 kB 等大块。

## 3. 首屏 `modulepreload`（dist/index.html）

入口脚本：`/assets/index-Or_l4ulV.js`

当前 `modulepreload`（**问题点**：重 chunk 仍被首屏预加载）：

- `rolldown-runtime-*.js`
- ⚠️ `vendor-monaco-*.js`（4 MB）
- ⚠️ `vendor-charts-*.js`
- ⚠️ `vendor-markdown-*.js`
- ⚠️ `vendor-terminal-*.js`
- `vendor-react-*.js` ×3

`stylesheet`：`vendor-monaco-*.css`、`vendor-terminal-*.css`、`index-*.css`

**结论**：阶段 1 的目标即移除 `vendor-monaco / vendor-charts / vendor-markdown / vendor-terminal` 四项首屏 `modulepreload`，让其按需加载。

## 4. React Profiler 记录（待手动采集）

以下需在浏览器中用 React DevTools Profiler 录制，无法自动完成。建议各录一段并 export 到本目录：

- [ ] 首次打开聊天视图（`profiler-baseline-open.json`）
- [ ] 长会话流式输出（`profiler-baseline-stream.json`）
- [ ] 输入框连续输入（`profiler-baseline-typing.json`）

观察并在此记录：每个流式 delta 后是否跟随 commit 的组件 —
`ChatPanel` / `MarkdownContent` / `ContextPanel`。

> 采集方法：`npm run dev -w web` → React DevTools → Profiler tab → 录制 → 执行对应操作 → 停止 → 右上角导出。

## 5. 验收对照（阶段 8 回填）

| 指标 | 基线 | 阶段 1 后 |
| --- | --- | --- |
| 总 asset 数 | 148 | — |
| 总 raw | 19084.3 kB | — |
| 总 gzip(JS+CSS) | 4348.0 kB | — |
| 入口 `index-*.js` raw | 638.9 kB | 588.9 kB |
| 首屏 modulepreload 含 monaco | 是 | **否** ✓ |
| 首屏 modulepreload 含 charts | 是 | **否** ✓ |
| 首屏 modulepreload 含 terminal | 是 | **否** ✓ |
| 首屏 modulepreload 含 markdown | 是 | 是（首屏聊天必需，保留） |
| 入口静态 import monaco/charts/terminal | 是 | **否（0）** ✓ |

### 阶段 1 关键修复说明

- TerminalPanel / ScheduledTasksPage → `React.lazy` + `Suspense`。
- 统计图表（recharts）抽到 `features/stats/StatCharts.tsx` 懒加载；`ContextMiniBarChart` 改为纯 CSS 条形，彻底移出聊天首屏的 recharts 依赖。
- 移除 `vite.config` 里 `vendor-charts` 手动分组——recharts 现仅经动态 import 拆分，不再被入口静态引用（此前 recharts 内置的 `react-is` 被该分组捕获、成为入口静态依赖）。
- monaco modulepreload 根因：`__vitePreload`（虚拟模块 `\0vite/preload-helper.js`）被 Rolldown 放进 `vendor-monaco`，入口为所有动态 import 静态引用它，从而拖入 4MB chunk。新增分组把 preload-helper 归入 `vendor-react`（首屏本就加载），`vendor-monaco` 保持单 chunk 隔离（规避 Emitter 循环 import 问题）且不再被入口静态引用。

