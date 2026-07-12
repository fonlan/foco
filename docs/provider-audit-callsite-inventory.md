# Provider 调用与审计迁移清单

生产调用面（搜索范围 `app/`，忽略 `.mem`）：

| 路径 | 调用/写入 | 分类 | 当前状态 |
| --- | --- | --- | --- |
| `app/main.rs` agent turn | `stream_chat_with_capture_observer`、running/outcome audit | 主聊天、多轮工具调用、provider retry | Phase 2 已迁移：每个 attempt 先插入空详情 running 行，observers 即时捕获 `provider_request_v1` 与真实 response head，Complete/失败/取消写 `provider_final_response_v1`。 |
| `app/main.rs` audited text/tool helpers | `stream_chat_with_capture_observer` | 标题、Memory、Workspace Spec、Git commit message、模型探测等内部请求 | Phase 2 已迁移：每个 retry 独立审计行，不再用 neutral request 作为详情。模型探测保留独立 requestKind 与主统计排除契约。 |
| `app/prompt/compression.rs` | `stream_chat_with_capture_observer` | LLM context compression | Phase 2 已迁移：running 行先创建，真实 request observer 回写，成功/失败保存最终 envelope；snapshot 与 fallback 语义保持。 |
| `app/hooks.rs` | `stream_chat_with_capture_observer` | Prompt Hook | Phase 1/2 已迁移：running request detail 初始为空，observer 回写真实 request；成功、建流失败、超时和流中断保存最终 envelope。 |
| `app/remote_workspace.rs` | `stream_chat_with_capture_observer` | 远程 Broker | Phase 3 已迁移：每个 broker request 独立审计行，主进程 observer 捕获真实 provider request；成功、失败、取消、sidecar 断连和无 Complete 中断均保存最终 envelope。 |

源码守卫 `app/provider_audit_source_guard.rs` 固定以下边界：

- `app/` 生产代码不再允许 direct `stream_chat(...)`；所有受审计 provider 调用必须经过 capture-aware API。
- 本地主聊天、内部 helper、context compression 与 Prompt Hook 不得把 `NeutralChatRequest`/`hook_request` 序列化结果作为 `request_body_json` 的审计详情信源。
- `app/` 已删除 `serialize_provider_request` 生产 helper；本地运行时只允许 finalized observer 产生 Request 详情。

Phase 4 硬验收测试：

- `providers/lib.rs::tests::captures_finalized_requests_for_four_primary_adapters`：真实本地 HTTP 覆盖 OpenAI Chat、OpenAI Responses、Anthropic、Gemini；逐请求比较 observer dump 与服务端实际 method/path/body，校验 Request headers 仅 Authorization 星号化、其它最终 HeaderMap 值保留、最终 adapter 映射和每个 attempt 只发送一次。
- `providers/lib.rs::tests::captures_final_wire_request_and_only_final_response` 与 `captures_http_response_head_for_non_success_stream`：验证成功和非 2xx 流都保存真实 status/version/response headers，Response headers 同样仅 Authorization 星号化，且 chunk-only sentinel 不进入最终 envelope。
- `app/tests/mod.rs::main_chat_real_http_bytes_persist_as_wire_and_detail_api_returns_wire`：真实主聊天生产路径贯穿 mock provider → finalized observer → SQLite `llm_requests` → AI statistics detail handler，明确断言新请求为 `provider_request_v1` / `provider_final_response_v1` 而非 legacy，且 chunk-only sentinel 不落库。
- `app/tests/mod.rs::main_chat_details_disabled_send_once_without_request_or_response_dump`：详情关闭时仍只发送一次，成功审计统计保留，但 request/response detail 为 `NULL`。

升级 genai fork、修改 adapter、ProviderAuditCapture、主聊天 turn/retry、远程 Broker、SQLite 审计或详情 API 时，必须重跑上述测试；provider 层 fixture 或前端手工 wire fixture 不能替代 App 端到端硬验收。
