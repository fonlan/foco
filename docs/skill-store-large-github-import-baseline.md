# 大型 GitHub 技能导入 504 基线

采集日期：2026-07-10。

## 真实仓库基线

目标仓库为 `chuspeeism/dashiAI-ppt-skill`。采集时 `main` 指向提交 `69ac66443e36e11cfca4a7f30721dc71a4278d28`（提交时间 `2026-07-10T08:15:10Z`）。递归 tree 未截断，共 421 个条目、359 个 blob。

可重复运行：

```sh
node scripts/skill-store-github-baseline.mjs \
  chuspeeism/dashiAI-ppt-skill \
  69ac66443e36e11cfca4a7f30721dc71a4278d28
```

采集结果：

- 仓库中唯一 `SKILL.md` 为 `skills/dashiai-ppt/SKILL.md`，所以自动发现不会进入多技能歧义错误。
- 技能根目录 `skills/dashiai-ppt` 下有 352 个 blob，总大小 57,984,950 bytes（约 55.3 MiB）。
- 当前预览实现需要 1 次 GitHub recursive-tree 请求，再按路径排序逐个执行 352 次 raw 文件请求，共 353 次 GitHub HTTP 请求。
- 按常见二进制扩展名统计，19 个 PNG 与 186 个 WOFF2 共 205 个文件、23,520,695 bytes（约 22.4 MiB）。这些文件当前也会经过 Reqwest `.text()` 解码并进入 JSON 字符串。
- 59 个文件不小于 100 KiB，16 个不小于 1 MiB，3 个不小于 2 MiB。
- 最大文件包括 4,579,172-byte `generated-metadata.js`、3,573,542-byte `layout-manifest.json`、2,247,468-byte `theme-style-grid.png` 和多张 1–2 MiB PNG。
- 抽样的 2,038,572-byte PNG 经与 Reqwest 等价的 UTF-8 replacement 解码后，序列化 JSON 字符串为 4,863,747 bytes，并产生 853,256 个 replacement characters。这说明预览既下载无须展示的二进制内容，也会显著放大响应。

以上规模数据来自 Git tree 中的 blob `size`，不需要下载全部 55.3 MiB 文件即可复核。脚本在 tree 截断或 `SKILL.md` 数量不是 1 时失败，避免把不完整数据误作基线。

## 退化链路

`POST /api/skill-store/import-preview` 的当前执行顺序是：

1. 输入解析为 GitHub `owner/repo`；目标 URL 可正常得到 `chuspeeism/dashiAI-ppt-skill`，不在输入解析阶段失败。
2. 请求 `git/trees/{branch}?recursive=1`。
3. `find_auto_github_skill_root` 发现唯一技能根目录。
4. 收集该根目录下所有 blob 并排序。
5. 在一个 `for` 循环中对每个 blob 执行 raw GET，并等待 `.send()`、状态检查和 `.text()` 完成后才开始下一个文件。
6. 把全部文件内容放进 `SkillStoreDetailResponse.files`，一次性 JSON 返回浏览器。

本地回归测试 `skill_store_import_preview_large_fixture_serializes_raw_downloads` 用一个唯一 `SKILL.md` 的小型 fixture 给每个 raw 响应添加固定延迟，并断言预览总耗时随文件数线性累积、最大并发 raw 请求为 1。它把问题固定为“发现成功后的串行预览链路”，而不是输入解析或多技能歧义。

## 超时边界

代码库内没有为该端点设置可通过调大数值解决的显式超时：

- `SkillStoreClient` 使用 `reqwest::Client::new()`，没有 request timeout。
- Axum 的 `import-preview` 路由没有 `TimeoutLayer`。
- Vite dev proxy 没有 `timeout`/`proxyTimeout`。
- 前端 `requestJson` 和导入预览调用没有计时器；只有浏览详情切换使用 `AbortController`。

因此应用自身会一直等待串行下载和大 JSON 构造。部署环境看到的 504 是长耗时请求跨越外部代理/网关等待上限的表现；仅提高上游超时会保留 353 次串行请求、二进制文本化和超大响应三个根因。
