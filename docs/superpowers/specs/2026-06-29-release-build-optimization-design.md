# Release 构建优化设计

## 目标

缩短日常 `npm run build:release`，同时保留一个显式的最高优化发布入口。不得改变最终程序功能、前端资源嵌入方式或现有完整类型检查覆盖。

实测基线：前端构建约 21 秒；前端重写 `web/dist` 后，`foco-app` 在 fat LTO 和单 codegen unit 下优化/链接超过 11 分钟，并因运行中的 `target/release/foco.exe` 被 Windows 锁定而最终失败。

## 方案比较

1. **推荐：日常 ThinLTO，正式分发保留 fat LTO。** `release` 使用 ThinLTO 和并行 codegen；新增 `dist` profile 保留 fat LTO、单 codegen unit。收益最大，且没有依赖或架构变化。
2. **只优化前端和失败提示。** 风险最低，但无法解决超过 11 分钟的 Rust 主瓶颈。
3. **完全关闭 LTO或取消资源嵌入。** 构建更快，但会改变发布性能/体积或单文件分发模型，当前没有必要。

采用方案 1，并叠加低风险的生产类型检查收窄和 Windows 快速失败。

## 设计

### Rust profiles

- `[profile.release]` 改为 `lto = "thin"`、`codegen-units = 16`，保留 `opt-level = 3`、`panic = "abort"`、`strip = true`。
- 新增 `[profile.dist]`，继承 `release`，覆盖为 `lto = "fat"`、`codegen-units = 1`。
- `build:release` 继续产出 `target/release/foco.exe`；新增 `build:dist`，产出 `target/dist/foco.exe`。

### 前端 typecheck

- 新增 `web/tsconfig.build.json`，继承现有配置，只把 `main.tsx` 作为生产入口。其传递依赖仍会被 TypeScript 检查，但根级测试、测试 setup 和 Vite 配置不进入 release 构建。
- `web` 的 `build` 使用生产配置；现有 `typecheck` 保持不变，继续覆盖测试文件。

### Release runner 与错误处理

- 新增一个小型 Node runner，复用 Node 标准库，顺序执行 Windows 锁检查、前端构建和对应 Cargo profile。
- Windows 下在构建开始前检查当前仓库目标路径的 `foco.exe` 是否正在运行；命中时明确报错并立即退出。非 Windows 不执行该检查。
- runner 只接受普通 release 或 `--dist`，未知参数直接失败，不做静默 fallback。
- `--check-only` 只执行参数解析与 Windows 锁检查，供快速验证，不启动前端或 Cargo 构建。

### 不在本次实施

- 不删除现有 `target`；清理 209GB debug 缓存属于用户可选的破坏性操作。
- 不引入 `sccache`、新 npm 依赖或外部 linker。
- 不改变 `rust-embed`、资源部署方式或 Vite chunk 策略。
- 不在仓库里硬编码共享 `CARGO_TARGET_DIR`；worktree 共享缓存应由运行时统一注入，另行处理。

## 验证

- Node runner 自检：release/dist 参数映射、未知参数拒绝。
- `npm run typecheck -w web`：完整 TypeScript 检查仍通过。
- `npm run build -w web`：生产 typecheck 与 Vite 构建通过。
- `cargo check --profile release -p foco-app` 与 `cargo check --profile dist -p foco-app`：两个 profile 配置有效。
- 在 `target/release/foco.exe` 运行时执行 runner 的 check-only 路径，确认快速拒绝且不触发前端/Cargo 构建。
- 最后检查 `git diff --check` 和 tracked diff，确保不夹带已有提交或无关改动。
