# Provider 调用与审计迁移清单

生产调用面（搜索范围 `app/`，忽略 `.mem`）：

| 路径 | 调用/写入 | 分类 | 当前状态 |
| --- | --- | --- | --- |
| `app/main.rs` agent turn | `stream_chat_with_capture_observer`、running/outcome audit | 主聊天、多轮工具调用、provider retry | 每个 attempt 先插入空详情 running 行，observers 即时捕获 HTTP `provider_request_v1` 或 WebSocket `provider_websocket_request_v1` 与真实 response/handshake head，Complete/失败/取消写 `provider_final_response_v1`（未捕获则为 NULL）。不再写 run-level `{text,reasoning,...}` / `{error}` / `{cancelled}` 到 `response_body_json`。 |
| `app/main.rs` audited text/tool helpers | `stream_chat_with_capture_observer` | 标题、Memory、Workspace Spec、Git commit message、模型探测等内部请求 | 每个 retry 独立审计行，不再用 Neutral request 作为详情。模型探测保留独立 requestKind 与主统计排除契约。 |
| `app/prompt/compression.rs` | `stream_chat_with_capture_observer` | LLM context compression | running 行先创建，真实 request observer 回写，成功/失败保存最终 envelope；snapshot 与 fallback 语义保持。 |
| `app/hooks.rs` | `stream_chat_with_capture_observer` | Prompt Hook | running request detail 初始为空，observer 回写真实 request；成功、建流失败、超时和流中断保存最终 envelope。 |
| `app/remote_workspace.rs` | `stream_chat_with_capture_observer`（主进程） | 远程 Broker | 每个 broker request 独立审计行，主进程 observer 捕获真实 provider wire；成功、失败、取消、sidecar 断连和无 Complete 中断均落主进程 profile audit mirror。`broker_llm_audit_context` 以 control connection `workspace_id` 为权威，建不上 mirror 硬失败。 |
| `app/remote_workspace.rs` sidecar mirror | `persist_sidecar_llm_audit*` | 远端 SQLite 统计镜像 | **仅**结构化列（usage/final_state/latency 等）；`request_body_json`/`response_body_json` **恒为 NULL**。不得写 completion/error payload、Neutral 或归一化 dump。 |

Store 格式不变量（本地 workspace、主进程 SSH audit mirror、远端 sidecar SQLite 共用）：

- 写入白名单：`request_body_json` 非空仅 `provider_request_v1` 或 `provider_websocket_request_v1` version 1；`response_body_json` 非空仅 `provider_final_response_v1` version 1。
- `merge_audit_detail_for_update`：真实 v1 可覆盖 NULL/非 v1；已有有效 v1 不被 NULL/legacy/normalized/重复 finish 覆盖。
- `open_or_create` 幂等清理非 v1 详情为 NULL（不伪造转 v1）；缺 detail 列的迁移 stub 跳过；并发写锁 busy 时 best-effort 跳过，下次 open 再试。
- 详情关闭：`save_request_response_details=false` 时 request/response detail 为 NULL（不再保留 compact `{cancelled}` 正文）。
- **结构化 `llm_requests.status_code` 与详情开关解耦**：只要观察到真实 HTTP Response head，就写入结构化 `status_code`（本地与 SSH 主进程 `remote-workspace-audit` 同契约）；完整 head dump 与 wire envelope 仍仅在详情开启时保留。无 Response（DNS/TLS/连接失败或响应前取消）→ `status_code` 为 NULL（UI `n/a`）。列表/详情 API **只读**该列，不从 `final_state` 或可选 wire dump 推导，不得硬编码 200。存量：`status_code IS NULL` 且仍保留合法 `provider_final_response_v1` 时一次性从 `http.status`（否则 failed envelope 的 `statusCode` 100–599）回填；metadata `llm_audit_status_code_v1_repaired`；非 v1/已清理/无 head 保持 NULL。
- 归一化状态只保留在 `llm_request_events.normalized_event_json`、run events 与结构化列。

源码守卫 `app/provider_audit_source_guard.rs` 固定以下边界：

- `app/` 生产代码不再允许 direct `stream_chat(...)`；所有受审计 provider 调用必须经过 capture-aware API。
- 本地主聊天、内部 helper、context compression 与 Prompt Hook 不得把 `NeutralChatRequest`/`hook_request` 序列化结果作为 `request_body_json` 的审计详情信源。
- `app/` 已删除 `serialize_provider_request` 生产 helper；本地运行时只允许 finalized observer 产生 Request 详情。
- sidecar 镜像 detail 必须为 NULL；取消路径不得把 compact cancelled JSON 赋给 `response_body_json`。

硬验收测试：

- `providers/lib.rs::tests::captures_finalized_requests_for_four_primary_adapters`：真实本地 HTTP 覆盖 OpenAI Chat、OpenAI Responses、Anthropic、Gemini；逐请求比较 observer dump 与服务端实际 method/path/body，校验 Request headers 仅 Authorization 星号化、其它最终 HeaderMap 值保留、最终 adapter 映射和每个 attempt 只发送一次。
- `providers/lib.rs::tests::captures_final_wire_request_and_only_final_response`, `captures_http_response_head_for_non_success_stream` 与 `connection_failure_before_http_response_does_not_fabricate_response_head`：固定 2xx streaming、非 2xx、收到 response 后失败以及连接建立前失败的可用性边界；真实 status/version/response headers 在 body 消费前捕获，Request/Response 均仅 Authorization 星号化，连接类失败不得伪造 HTTP head。
- `providers/lib.rs` status 聚焦：`http_status_preserves_non_default_success_status_without_detail_dumps`（201 且详情关闭 dump 仍 None）、`http_status_is_captured_without_detail_dumps`（502）、`http_status_survives_stream_decode_failure_after_response_head`、`connection_failure_http_status_is_none_without_response`。
- `providers/lib.rs` WebSocket 审计：`websocket_stream_maps_to_neutral_events_and_closes_cleanly` 断言 `provider_websocket_request_v1`（wss/ws URL、`response.create` frame、`frameSent=true`、Authorization 脱敏、handshake 101）；`websocket_observer_notified_only_after_create_frame_sent_with_frame_sent_true` 断言 Upgrade 收到真实 Bearer 且 observer 仅在 send 成功后收到 `frameSent=true` dump；`websocket_handshake_http_rejection_preserves_status_code` 断言 401 upgrade 保留 status 且 `frameSent=false`；`websocket_session_reuses_connection_and_previous_response_id` 断言复用 turn `connectionReused=true`、无 handshake、不伪造 `http_status`/HTTP head。
- `app/tests/mod.rs::main_chat_real_http_bytes_persist_as_wire_and_detail_api_returns_wire`：真实主聊天生产路径贯穿 mock provider → 双 observers → SQLite `llm_requests` → AI statistics detail handler；仅 1 条 turn 审计、无 `run-` summary row；新记录为 `provider_request_v1` / `provider_final_response_v1`、chunk-only sentinel 不落库。
- `app/tests/mod.rs::main_chat_details_disabled_send_once_without_request_or_response_dump`：详情关闭时仍只发送一次，成功审计统计保留，但 request/response detail 为 `NULL`。
- `app/remote_workspace.rs::remote_ssh_sidecar_chat_turn_persists_real_wire_to_profile_audit_mirror`：真实 `WorkspaceLocation::Ssh` + `remote_sidecar_chat_stream` → control WS → mock provider → 主进程 `profile/.foco/remote-workspace-audit` list/detail；sidecar mirror detail 恒 NULL；同一 broker id 全链路一致；SQLite/list/detail 同一真实 `statusCode`。
- `app/remote_workspace.rs::broker_control_llm_stream_persists_real_provider_wire_and_exposes_same_request_id_to_detail_api`：control WS → `llm.stream` → mock OpenAI → SQLite → Detail API（Ssh 路径）；断言结构化 `status_code`。
- `app/remote_workspace.rs::broker_control_llm_stream_persists_status_code_without_request_response_details`：详情关闭仍写真实 `status_code`（含非默认 2xx），wire dump 为 NULL。
- `app/remote_workspace.rs::broker_control_llm_stream_persists_http_failure_status_code_independently_of_final_state`：HTTP 502 → `final_state=failed` 且 `status_code=502`（业务终态与 HTTP 状态独立）。
- `store/tests/workspace_database.rs::repairs_null_status_code_from_valid_v1_response_wire_once`：存量回填矩阵（合法 succeeded/failed v1、无 head、非 v1、非法 status、已有 status 不覆盖、cleaned detail、二次 open 不重复扫描）。
- `store/tests/workspace_database.rs::rejects_non_v1_audit_details_and_prunes_legacy_on_open`：写入拒绝非 v1；重开清理 `{}`/Neutral/normalized/`{error}`/`{cancelled}`/`legacy_text_v1`；合法 v1 保留。

升级 genai fork、修改 adapter、ProviderAuditCapture、主聊天 turn/retry、远程 Broker、SQLite 审计或详情 API 时，必须重跑上述测试；provider 层 fixture 或前端手工 wire fixture 不能替代 App/SSH 端到端硬验收。

安全与展示契约：

- prepared-request observer 只保证看到发送前最终应用层 `reqwest::Request`；transport/proxy 在该边界之后新增或改写的 headers 不保证可见。
- response-head observer 仅在真实 `reqwest::Response` 已建立时可用，并在 status 校验、SSE 解码和 body 消费前读取 status/version/HeaderMap；DNS/TLS/连接失败不得从错误文本或应用状态伪造 head。
- Request 与 Response 的版本化 HTTP headers 仅把 `Authorization` 写为 `********`。`X-API-Key`、Cookie、Set-Cookie、签名与 token 命名 header 会原样进入本地 workspace SQLite 和详情 UI；这是有意放宽的本地审计安全边界。
- Response UI 仅渲染真实 `provider_*_v1`：status/version、headers JSON 和一个完整最终 response-envelope JSON；不再拆分最终文本、推理、工具调用或 usage 卡片，也不保存原始 SSE/chunk。非 v1 / 已清理详情 → `malformed`/`unavailable`，**不**回显 legacy 正文，**无** `legacy_text_v1` fallback。
- 主进程真实 wire 为唯一详情真源；SSH sidecar 镜像不可替代主进程 dump。
- **远程代理例外（HTTP 路由）**：workspace 代理中间件不得转发 `ai-statistics`。全局列表 `GET /api/ai-statistics` 与 workspace 详情 `GET /api/workspaces/{id}/ai-statistics/{request_id}` 均留在主进程，经 `workspace_audit_path` 读 `profile/.foco/remote-workspace-audit/<workspaceId>`（SSH）或本地 workspace SQLite。Sidecar 的 `/api/remote/workspace/ai-statistics/{request_id}` 可继续返回结构化镜像，但 detail 恒 NULL，**不得**当作 dump 真源。若把详情路由误代理到 sidecar，UI 会恒显示 unavailable（即使主进程 mirror 已有 v1 wire）。
- **历史与开关语义**：修复后仅当主进程已捕获合法 `provider_request_v1` / `provider_final_response_v1` 时详情为 `captured`。历史上未捕获、`save_request_response_details=false`、保留期清理后 detail 为 NULL、或非 v1 被 open 清掉的记录仍正确 `unavailable`/`malformed`，**禁止**用 sidecar normalized 镜像事后伪造恢复。
- 回归：`proxy_workspace_route_path_keeps_ai_statistics_on_main_process`、`remote_ai_statistics_detail_http_reads_main_process_audit_mirror`（真实 App Router，fake sidecar hit=0）。
