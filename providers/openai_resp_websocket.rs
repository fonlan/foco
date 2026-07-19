use std::sync::{Arc, Mutex};

use async_stream::stream;
use futures_util::{SinkExt, StreamExt};
use genai::{
    PreparedChatStreamRequest,
    adapter::{OpenAIRespEventDecoder, openai_resp_websocket_create_payload},
    chat::{ChatOptions, ChatRequest, ChatStreamEvent},
};
use tokio_tungstenite::{
    connect_async,
    tungstenite::{
        Message,
        client::IntoClientRequest,
        http::{HeaderName, HeaderValue},
    },
};

use crate::openai_resp_ws_session::{
    AppliedContinuation, LiveWsConnection, OpenAiRespWsRead, OpenAiRespWsSessionTurn,
    OpenAiRespWsWrite, chat_options_fingerprint_json, connection_identity_from_parts,
    continuation_fingerprint, tools_fingerprint_json,
};
use crate::{
    MASKED_AUTHORIZATION_VALUE, NeutralChatStream, PROVIDER_WEBSOCKET_REQUEST_DUMP_FORMAT,
    PROVIDER_WEBSOCKET_REQUEST_DUMP_VERSION, ProviderAuditRequestDump, ProviderConfigError,
    ProviderConnectionConfig, ProviderErrorContext, ProviderHttpHeadersDump,
    ProviderHttpResponseHeadDump, ProviderRequestDumpObserver, ProviderRequestFailure,
    ProviderWebSocketHandshakeDump, ProviderWebSocketRequestDump, ProviderWsSessionContext,
    redact_json_body_credentials, websocket_url_from_responses_http_url,
};

pub(crate) async fn stream_chat_openai_resp_websocket(
    config: &ProviderConnectionConfig,
    chat_request: ChatRequest,
    options: ChatOptions,
    model: genai::ModelIden,
    error_context: ProviderErrorContext,
    capture_details: bool,
    request_observer: Option<ProviderRequestDumpObserver>,
    session_ctx: Option<ProviderWsSessionContext>,
) -> Result<NeutralChatStream, ProviderRequestFailure> {
    match session_ctx {
        Some(ctx) => {
            stream_with_session(
                config,
                chat_request,
                options,
                model,
                error_context,
                capture_details,
                request_observer,
                ctx,
            )
            .await
        }
        None => {
            stream_ephemeral(
                config,
                chat_request,
                options,
                model,
                error_context,
                capture_details,
                request_observer,
            )
            .await
        }
    }
}

async fn stream_ephemeral(
    config: &ProviderConnectionConfig,
    mut chat_request: ChatRequest,
    options: ChatOptions,
    model: genai::ModelIden,
    error_context: ProviderErrorContext,
    capture_details: bool,
    request_observer: Option<ProviderRequestDumpObserver>,
) -> Result<NeutralChatStream, ProviderRequestFailure> {
    // One-shot internal requests: no store, no session reuse, no previous_response_id.
    chat_request.store = Some(false);
    chat_request.previous_response_id = None;

    let prepared = prepare_request_async(
        config,
        chat_request,
        &options,
        model.clone(),
        &error_context,
    )
    .await?;
    let (ws_url, create_payload) = prepared_wire(&prepared)?;
    // Pre-send dump for connect-failure diagnostics (frame_sent=false; observer only on failure).
    let pre_send_dump = (capture_details || request_observer.is_some()).then(|| {
        ProviderAuditRequestDump::from_websocket(websocket_wire_request_dump(
            &ws_url,
            &prepared,
            &create_payload,
            false,
            None,
            config.api_key.as_deref(),
            false,
        ))
    });

    let (write, read, handshake_status, response_status, response_head, handshake) =
        match connect_websocket(
            &ws_url,
            &prepared,
            &error_context,
            pre_send_dump.as_ref(),
            capture_details,
            config.api_key.as_deref(),
        )
        .await
        {
            Ok(parts) => parts,
            Err(mut error) => {
                if let Some(dump) = pre_send_dump {
                    if let Some(observer) = request_observer.as_ref() {
                        observer(&dump);
                    }
                    if capture_details {
                        error.request_dump = Some(dump);
                    }
                }
                return Err(error);
            }
        };

    run_create_stream(
        write,
        read,
        &ws_url,
        &prepared,
        create_payload,
        model,
        &options,
        error_context,
        false,
        handshake,
        config.api_key.as_deref(),
        capture_details,
        request_observer.as_ref(),
        response_status,
        response_head,
        handshake_status,
        None,
        false,
        0,
        0,
        0,
        std::time::Instant::now(),
        0,
    )
    .await
}

async fn stream_with_session(
    config: &ProviderConnectionConfig,
    chat_request: ChatRequest,
    options: ChatOptions,
    model: genai::ModelIden,
    error_context: ProviderErrorContext,
    capture_details: bool,
    request_observer: Option<ProviderRequestDumpObserver>,
    session_ctx: ProviderWsSessionContext,
) -> Result<NeutralChatStream, ProviderRequestFailure> {
    let enable_continuation = session_ctx.enable_continuation;
    let turn = session_ctx
        .registry
        .begin_turn(session_ctx.key.clone())
        .await;

    let connection_identity = connection_identity_from_config(config);
    let tools_fp = tools_fingerprint_json(&chat_request.tools);
    let options_fp = chat_options_fingerprint_json(&options);
    let fingerprint = continuation_fingerprint(
        model.model_name.as_ref(),
        config.base_url.as_deref(),
        config.kind.as_str(),
        chat_request.system.as_deref(),
        &tools_fp,
        connection_identity,
        &options_fp,
    );

    // Only reuse a live socket when Provider connection identity still matches.
    // Mismatch / age-out closes the old socket and clears continuation first so
    // we never send a stale previous_response_id on a rebuilt connection.
    let existing_connection = turn.take_connection_for_identity(connection_identity).await;

    let AppliedContinuation {
        chat_request: continued_request,
        used_previous_response_id,
        full_messages_len,
        full_messages_hash,
        fingerprint,
    } = turn
        .apply_continuation(chat_request, fingerprint, enable_continuation)
        .await;

    if let Some(prev) = used_previous_response_id.as_ref() {
        tracing::info!(
            workspace_id = %turn.key().workspace_id,
            run_affinity_id = %turn.key().run_affinity_id,
            previous_response_id = %prev,
            full_messages_len,
            "openai responses websocket: using previous_response_id continuation"
        );
    }

    let prepared = match prepare_request_async(
        config,
        continued_request,
        &options,
        model.clone(),
        &error_context,
    )
    .await
    {
        Ok(prepared) => prepared,
        Err(error) => {
            turn.commit_failure().await;
            if let Some(conn) = existing_connection {
                turn.return_connection(Some(conn), false).await;
            }
            turn.finish().await;
            return Err(error);
        }
    };

    let (ws_url, create_payload) = match prepared_wire(&prepared) {
        Ok(parts) => parts,
        Err(error) => {
            turn.commit_failure().await;
            if let Some(conn) = existing_connection {
                turn.return_connection(Some(conn), false).await;
            }
            turn.finish().await;
            return Err(error);
        }
    };

    let connection_reused = existing_connection.is_some();
    let pre_send_dump = (capture_details || request_observer.is_some()).then(|| {
        ProviderAuditRequestDump::from_websocket(websocket_wire_request_dump(
            &ws_url,
            &prepared,
            &create_payload,
            connection_reused,
            None,
            config.api_key.as_deref(),
            false,
        ))
    });

    let (write, read, handshake_status, response_status, response_head, handshake, connected_at) =
        match existing_connection {
            Some(LiveWsConnection {
                write,
                read,
                connected_at,
                connection_identity: _,
                handshake_status: established_status,
            }) => {
                // Reused sockets do not re-handshake this turn. Do not fabricate an HTTP
                // response head for wire audit — that would not be observed for this turn.
                // Preserve the connection-level handshake status only for socket bookkeeping.
                let response_status = Arc::new(Mutex::new(None));
                let response_head = None;
                (
                    write,
                    read,
                    established_status,
                    response_status,
                    response_head,
                    None,
                    connected_at,
                )
            }
            None => {
                match connect_websocket(
                    &ws_url,
                    &prepared,
                    &error_context,
                    pre_send_dump.as_ref(),
                    capture_details,
                    config.api_key.as_deref(),
                )
                .await
                {
                    Ok((
                        write,
                        read,
                        handshake_status,
                        response_status,
                        response_head,
                        handshake,
                    )) => (
                        write,
                        read,
                        handshake_status,
                        response_status,
                        response_head,
                        handshake,
                        std::time::Instant::now(),
                    ),
                    Err(mut error) => {
                        turn.commit_failure().await;
                        turn.finish().await;
                        if let Some(dump) = pre_send_dump {
                            if let Some(observer) = request_observer.as_ref() {
                                observer(&dump);
                            }
                            if capture_details {
                                error.request_dump = Some(dump);
                            }
                        }
                        return Err(error);
                    }
                }
            }
        };

    // Observer is notified only after response.create is successfully written (inside run_create_stream).
    run_create_stream(
        write,
        read,
        &ws_url,
        &prepared,
        create_payload,
        model,
        &options,
        error_context,
        connection_reused,
        handshake,
        config.api_key.as_deref(),
        capture_details,
        request_observer.as_ref(),
        response_status,
        response_head,
        handshake_status,
        Some(turn),
        enable_continuation,
        full_messages_len,
        full_messages_hash,
        fingerprint,
        connected_at,
        connection_identity,
    )
    .await
}

fn connection_identity_from_config(config: &ProviderConnectionConfig) -> u64 {
    let overrides_json =
        serde_json::to_string(&config.request_overrides).unwrap_or_else(|_| "[]".to_string());
    let redirects_json =
        serde_json::to_string(&config.model_redirects).unwrap_or_else(|_| "[]".to_string());
    connection_identity_from_parts(
        config.kind.as_str(),
        config.base_url.as_deref(),
        config.api_key.as_deref(),
        config.proxy_url.as_deref(),
        &overrides_json,
        &redirects_json,
    )
}

async fn prepare_request_async(
    config: &ProviderConnectionConfig,
    chat_request: ChatRequest,
    options: &ChatOptions,
    model: genai::ModelIden,
    error_context: &ProviderErrorContext,
) -> Result<PreparedChatStreamRequest, ProviderRequestFailure> {
    let client = config
        .genai_client()
        .map_err(|error| ProviderRequestFailure {
            error,
            request_dump: None,
        })?;

    client
        .prepare_chat_stream_request(model, chat_request, Some(options))
        .await
        .map_err(|source| ProviderRequestFailure {
            error: ProviderConfigError::from_genai_error_with_context(source, error_context),
            request_dump: None,
        })
}

fn prepared_wire(
    prepared: &PreparedChatStreamRequest,
) -> Result<(String, serde_json::Value), ProviderRequestFailure> {
    let ws_url = websocket_url_from_responses_http_url(&prepared.url).map_err(|error| {
        ProviderRequestFailure {
            error,
            request_dump: None,
        }
    })?;
    let create_payload = openai_resp_websocket_create_payload(prepared.payload.clone());
    Ok((ws_url, create_payload))
}

fn build_websocket_request_dump(
    ws_url: &str,
    prepared: &PreparedChatStreamRequest,
    create_payload: &serde_json::Value,
    connection_reused: bool,
    handshake: Option<ProviderWebSocketHandshakeDump>,
    api_key: Option<&str>,
    frame_sent: bool,
    capture_details: bool,
    request_observer: Option<&ProviderRequestDumpObserver>,
    notify: bool,
) -> Option<ProviderAuditRequestDump> {
    if !capture_details && request_observer.is_none() {
        return None;
    }
    let dump = ProviderAuditRequestDump::from_websocket(websocket_wire_request_dump(
        ws_url,
        prepared,
        create_payload,
        connection_reused,
        handshake,
        api_key,
        frame_sent,
    ));
    if notify {
        if let Some(observer) = request_observer {
            observer(&dump);
        }
    }
    // Only keep dump on the stream / failure path when detail capture is enabled.
    capture_details.then_some(dump)
}

/// Extract real HTTP status from a failed WebSocket upgrade (401/429/5xx), if present.
fn websocket_connect_status_code(error: &tokio_tungstenite::tungstenite::Error) -> Option<u16> {
    match error {
        tokio_tungstenite::tungstenite::Error::Http(response) => Some(response.status().as_u16()),
        _ => None,
    }
}

async fn connect_websocket(
    ws_url: &str,
    prepared: &PreparedChatStreamRequest,
    error_context: &ProviderErrorContext,
    wire_request_dump: Option<&ProviderAuditRequestDump>,
    capture_details: bool,
    api_key: Option<&str>,
) -> Result<
    (
        OpenAiRespWsWrite,
        OpenAiRespWsRead,
        u16,
        Arc<Mutex<Option<u16>>>,
        Option<Arc<Mutex<Option<ProviderHttpResponseHeadDump>>>>,
        Option<ProviderWebSocketHandshakeDump>,
    ),
    ProviderRequestFailure,
> {
    let mut request = ws_url
        .into_client_request()
        .map_err(|source| ProviderRequestFailure {
            error: ProviderConfigError::Connection {
                message: format!("{error_context}: invalid WebSocket request: {source}"),
                status_code: None,
            },
            request_dump: wire_request_dump.cloned(),
        })?;

    let mut has_authorization = false;
    for (name, value) in prepared.headers.iter() {
        if name.eq_ignore_ascii_case("host")
            || name.eq_ignore_ascii_case("connection")
            || name.eq_ignore_ascii_case("upgrade")
            || name.eq_ignore_ascii_case("sec-websocket-key")
            || name.eq_ignore_ascii_case("sec-websocket-version")
        {
            continue;
        }
        if name.eq_ignore_ascii_case("authorization") {
            has_authorization = true;
        }
        let header_name: HeaderName = name.parse().map_err(|source| ProviderRequestFailure {
            error: ProviderConfigError::InvalidRequest(format!(
                "invalid WebSocket header name '{name}': {source}"
            )),
            request_dump: wire_request_dump.cloned(),
        })?;
        let header_value =
            HeaderValue::from_str(value).map_err(|source| ProviderRequestFailure {
                error: ProviderConfigError::InvalidRequest(format!(
                    "invalid WebSocket header value for '{name}': {source}"
                )),
                request_dump: wire_request_dump.cloned(),
            })?;
        request.headers_mut().insert(header_name, header_value);
    }
    if !has_authorization && let Some(api_key) = api_key.filter(|value| !value.is_empty()) {
        let header_value =
            HeaderValue::from_str(&format!("Bearer {api_key}")).map_err(|source| {
                ProviderRequestFailure {
                    error: ProviderConfigError::InvalidRequest(format!(
                        "invalid WebSocket authorization header: {source}"
                    )),
                    request_dump: wire_request_dump.cloned(),
                }
            })?;
        request
            .headers_mut()
            .insert(HeaderName::from_static("authorization"), header_value);
    }

    let (ws_stream, response) = match connect_async(request).await {
        Ok(parts) => parts,
        Err(source) => {
            let status_code = websocket_connect_status_code(&source);
            return Err(ProviderRequestFailure {
                error: ProviderConfigError::Connection {
                    message: format!("{error_context}: WebSocket connect failed: {source}"),
                    status_code,
                },
                request_dump: wire_request_dump.cloned(),
            });
        }
    };

    let handshake_status = response.status().as_u16();
    let handshake_headers =
        response
            .headers()
            .iter()
            .fold(ProviderHttpHeadersDump::new(), |mut map, (name, value)| {
                let entry = map.entry(name.as_str().to_string()).or_default();
                if name.as_str().eq_ignore_ascii_case("authorization") {
                    entry.push(MASKED_AUTHORIZATION_VALUE.to_string());
                } else {
                    entry.push(
                        value
                            .to_str()
                            .map(str::to_string)
                            .unwrap_or_else(|_| "[NON_UTF8]".to_string()),
                    );
                }
                map
            });
    let handshake = ProviderWebSocketHandshakeDump {
        status: handshake_status,
        version: "HTTP/1.1".to_string(),
        headers: handshake_headers.clone(),
    };
    let response_status = Arc::new(Mutex::new(Some(handshake_status)));
    let response_head = capture_details.then(|| {
        Arc::new(Mutex::new(Some(ProviderHttpResponseHeadDump {
            status: handshake_status,
            version: "HTTP/1.1".to_string(),
            headers: handshake_headers,
        })))
    });

    let (write, read) = ws_stream.split();
    Ok((
        write,
        read,
        handshake_status,
        response_status,
        response_head,
        Some(handshake),
    ))
}

async fn run_create_stream(
    mut write: OpenAiRespWsWrite,
    read: OpenAiRespWsRead,
    ws_url: &str,
    prepared: &PreparedChatStreamRequest,
    create_payload: serde_json::Value,
    model: genai::ModelIden,
    options: &ChatOptions,
    error_context: ProviderErrorContext,
    connection_reused: bool,
    handshake: Option<ProviderWebSocketHandshakeDump>,
    api_key: Option<&str>,
    capture_details: bool,
    request_observer: Option<&ProviderRequestDumpObserver>,
    response_status: Arc<Mutex<Option<u16>>>,
    response_head: Option<Arc<Mutex<Option<ProviderHttpResponseHeadDump>>>>,
    handshake_status: u16,
    turn: Option<OpenAiRespWsSessionTurn>,
    enable_continuation: bool,
    full_messages_len: usize,
    full_messages_hash: u64,
    fingerprint: u64,
    connected_at: std::time::Instant,
    connection_identity: u64,
) -> Result<NeutralChatStream, ProviderRequestFailure> {
    let create_text =
        serde_json::to_string(&create_payload).map_err(|source| ProviderRequestFailure {
            error: ProviderConfigError::InvalidRequest(format!(
                "failed to serialize response.create: {source}"
            )),
            request_dump: build_websocket_request_dump(
                ws_url,
                prepared,
                &create_payload,
                connection_reused,
                handshake.clone(),
                api_key,
                false,
                capture_details,
                request_observer,
                false,
            ),
        })?;

    if let Err(source) = write.send(Message::Text(create_text.into())).await {
        if let Some(turn) = turn {
            turn.commit_failure().await;
            turn.return_connection(None, false).await;
            turn.finish().await;
        }
        // Frame never left the client; dump is diagnostic only (frame_sent=false).
        let wire_request_dump = build_websocket_request_dump(
            ws_url,
            prepared,
            &create_payload,
            connection_reused,
            handshake,
            api_key,
            false,
            capture_details,
            request_observer,
            true,
        );
        // Only report an HTTP status when this turn actually observed a handshake.
        let status_code = response_status.lock().ok().and_then(|slot| *slot);
        return Err(ProviderRequestFailure {
            error: ProviderConfigError::Connection {
                message: format!("{error_context}: failed to send response.create: {source}"),
                status_code,
            },
            request_dump: wire_request_dump,
        });
    }

    // Real wire: response.create was written successfully.
    let wire_request_dump = build_websocket_request_dump(
        ws_url,
        prepared,
        &create_payload,
        connection_reused,
        handshake,
        api_key,
        true,
        capture_details,
        request_observer,
        true,
    );

    let mut decoder = OpenAIRespEventDecoder::from_chat_options(model, options);
    let stream_error_context = error_context.with_phase("reading provider stream");
    let turn_slot = Arc::new(tokio::sync::Mutex::new(turn));
    let halves = Arc::new(tokio::sync::Mutex::new(Some((
        write,
        read,
        connected_at,
        connection_identity,
        handshake_status,
    ))));
    let outcome = Arc::new(Mutex::new(StreamTurnOutcome {
        response_id: None,
        keep_connection: true,
        completed_ok: false,
        previous_response_not_found: false,
    }));
    // If the consumer drops the stream mid-turn, still return/close the socket.
    let drop_guard = StreamFinalizeGuard {
        turn_slot: Arc::clone(&turn_slot),
        halves: Arc::clone(&halves),
        outcome: Arc::clone(&outcome),
        enable_continuation,
        full_messages_len,
        full_messages_hash,
        fingerprint,
        armed: true,
    };

    let event_stream = {
        let outcome = Arc::clone(&outcome);
        let halves = Arc::clone(&halves);
        let mut drop_guard = Some(drop_guard);
        stream! {
            yield Ok(ChatStreamEvent::Start);

            loop {
                let next = {
                    let mut guard = halves.lock().await;
                    let Some((_, read, _, _, _)) = guard.as_mut() else {
                        break;
                    };
                    read.next().await
                };
                match next {
                    Some(Ok(Message::Text(text))) => {
                        mark_previous_not_found_if_needed(&outcome, text.as_str());
                        let decode = {
                            // decoder is not Send across await with lock held; use local
                            decoder.decode_json_to_chat_event(text.as_str())
                        };
                        match decode {
                            Ok(Some(event)) => {
                                note_stream_event(&outcome, &event);
                                let done = matches!(event, ChatStreamEvent::End(_));
                                yield Ok(event);
                                if done {
                                    break;
                                }
                            }
                            Ok(None) => {}
                            Err(error) => {
                                mark_stream_broken(&outcome);
                                yield Err(error);
                                break;
                            }
                        }
                    }
                    Some(Ok(Message::Binary(bytes))) => {
                        let Ok(text) = std::str::from_utf8(&bytes) else {
                            continue;
                        };
                        mark_previous_not_found_if_needed(&outcome, text);
                        match decoder.decode_json_to_chat_event(text) {
                            Ok(Some(event)) => {
                                note_stream_event(&outcome, &event);
                                let done = matches!(event, ChatStreamEvent::End(_));
                                yield Ok(event);
                                if done {
                                    break;
                                }
                            }
                            Ok(None) => {}
                            Err(error) => {
                                mark_stream_broken(&outcome);
                                yield Err(error);
                                break;
                            }
                        }
                    }
                    Some(Ok(Message::Ping(payload))) => {
                        let send_result = {
                            let mut guard = halves.lock().await;
                            if let Some((write, _, _, _, _)) = guard.as_mut() {
                                write.send(Message::Pong(payload)).await
                            } else {
                                break;
                            }
                        };
                        if let Err(error) = send_result {
                            mark_stream_broken(&outcome);
                            yield Err(genai::Error::WebStream {
                                model_iden: decoder.model_iden().clone(),
                                cause: format!("failed to reply to WebSocket ping: {error}"),
                                error: Box::new(error),
                            });
                            break;
                        }
                    }
                    Some(Ok(Message::Pong(_))) => {}
                    Some(Ok(Message::Frame(_))) => {}
                    Some(Ok(Message::Close(_))) | None => {
                        mark_stream_broken(&outcome);
                        if let Some(end) = decoder.finish_on_eof_to_chat_event() {
                            note_stream_event(&outcome, &end);
                            yield Ok(end);
                        }
                        break;
                    }
                    Some(Err(error)) => {
                        mark_stream_broken(&outcome);
                        yield Err(genai::Error::WebStream {
                            model_iden: decoder.model_iden().clone(),
                            cause: error.to_string(),
                            error: Box::new(error),
                        });
                        break;
                    }
                }
            }

            if let Some(guard) = drop_guard.take() {
                guard.finalize_now().await;
            }
        }
    };

    Ok(NeutralChatStream {
        stream: Box::pin(event_stream),
        error_context: stream_error_context,
        wire_request_dump,
        response_status,
        response_head,
        saw_response_event: false,
        final_response_dump: None,
    })
}

struct StreamFinalizeGuard {
    turn_slot: Arc<tokio::sync::Mutex<Option<OpenAiRespWsSessionTurn>>>,
    halves: Arc<
        tokio::sync::Mutex<
            Option<(
                OpenAiRespWsWrite,
                OpenAiRespWsRead,
                std::time::Instant,
                u64,
                u16,
            )>,
        >,
    >,
    outcome: Arc<Mutex<StreamTurnOutcome>>,
    enable_continuation: bool,
    full_messages_len: usize,
    full_messages_hash: u64,
    fingerprint: u64,
    armed: bool,
}

impl StreamFinalizeGuard {
    async fn finalize_now(mut self) {
        self.armed = false;
        finalize_turn(
            &self.turn_slot,
            &self.outcome,
            &self.halves,
            self.enable_continuation,
            self.full_messages_len,
            self.full_messages_hash,
            self.fingerprint,
        )
        .await;
    }
}

impl Drop for StreamFinalizeGuard {
    fn drop(&mut self) {
        if !self.armed {
            return;
        }
        let turn_slot = Arc::clone(&self.turn_slot);
        let halves = Arc::clone(&self.halves);
        let outcome = Arc::clone(&self.outcome);
        let enable_continuation = self.enable_continuation;
        let full_messages_len = self.full_messages_len;
        let full_messages_hash = self.full_messages_hash;
        let fingerprint = self.fingerprint;
        // Best-effort async cleanup when the stream is dropped early.
        tokio::spawn(async move {
            finalize_turn(
                &turn_slot,
                &outcome,
                &halves,
                enable_continuation,
                full_messages_len,
                full_messages_hash,
                fingerprint,
            )
            .await;
        });
    }
}

struct StreamTurnOutcome {
    response_id: Option<String>,
    keep_connection: bool,
    completed_ok: bool,
    previous_response_not_found: bool,
}

fn mark_previous_not_found_if_needed(outcome: &Arc<Mutex<StreamTurnOutcome>>, text: &str) {
    if text_looks_like_previous_response_not_found(text)
        && let Ok(mut slot) = outcome.lock()
    {
        slot.previous_response_not_found = true;
        slot.completed_ok = false;
    }
}

fn mark_stream_broken(outcome: &Arc<Mutex<StreamTurnOutcome>>) {
    if let Ok(mut slot) = outcome.lock() {
        slot.keep_connection = false;
        slot.completed_ok = false;
    }
}

fn note_stream_event(outcome: &Arc<Mutex<StreamTurnOutcome>>, event: &ChatStreamEvent) {
    let Ok(mut slot) = outcome.lock() else {
        return;
    };
    if let ChatStreamEvent::End(end) = event {
        slot.response_id = end.captured_response_id.clone();
        slot.completed_ok = end.captured_response_id.is_some() && !slot.previous_response_not_found;
        if !slot.completed_ok && slot.previous_response_not_found {
            slot.keep_connection = true;
        }
    }
}

fn text_looks_like_previous_response_not_found(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    lower.contains("previous_response_not_found")
        || (lower.contains("previous response") && lower.contains("not found"))
}

async fn finalize_turn(
    turn_slot: &Arc<tokio::sync::Mutex<Option<OpenAiRespWsSessionTurn>>>,
    outcome: &Arc<Mutex<StreamTurnOutcome>>,
    halves: &Arc<
        tokio::sync::Mutex<
            Option<(
                OpenAiRespWsWrite,
                OpenAiRespWsRead,
                std::time::Instant,
                u64,
                u16,
            )>,
        >,
    >,
    enable_continuation: bool,
    full_messages_len: usize,
    full_messages_hash: u64,
    fingerprint: u64,
) {
    let taken = {
        let mut guard = halves.lock().await;
        guard.take()
    };
    let Some((write, read, connected_at, connection_identity, handshake_status)) = taken else {
        // Already finalized.
        return;
    };

    let turn = {
        let mut slot = turn_slot.lock().await;
        slot.take()
    };

    let Some(turn) = turn else {
        let mut conn = LiveWsConnection {
            write,
            read,
            connected_at,
            connection_identity,
            handshake_status,
        };
        let _ = conn.write.close().await;
        return;
    };

    let (response_id, completed_ok, keep_connection, previous_not_found) = outcome
        .lock()
        .map(|slot| {
            (
                slot.response_id.clone(),
                slot.completed_ok,
                slot.keep_connection,
                slot.previous_response_not_found,
            )
        })
        .unwrap_or((None, false, false, false));

    if previous_not_found || !completed_ok {
        turn.commit_failure().await;
        if previous_not_found {
            tracing::info!(
                workspace_id = %turn.key().workspace_id,
                run_affinity_id = %turn.key().run_affinity_id,
                "openai responses websocket: previous_response_not_found; cleared continuation"
            );
        }
    } else {
        turn.commit_success(
            response_id,
            full_messages_len,
            full_messages_hash,
            fingerprint,
            enable_continuation,
        )
        .await;
    }

    let conn = LiveWsConnection {
        write,
        read,
        connected_at,
        connection_identity,
        handshake_status,
    };
    turn.return_connection(Some(conn), keep_connection && completed_ok)
        .await;
    turn.finish().await;
}

fn websocket_wire_request_dump(
    ws_url: &str,
    prepared: &PreparedChatStreamRequest,
    create_payload: &serde_json::Value,
    connection_reused: bool,
    handshake: Option<ProviderWebSocketHandshakeDump>,
    api_key: Option<&str>,
    frame_sent: bool,
) -> ProviderWebSocketRequestDump {
    let create_frame = serde_json::to_string(create_payload)
        .map(|body| redact_json_body_credentials(&body))
        .ok();

    let mut headers = ProviderHttpHeadersDump::new();
    for (name, value) in prepared.headers.iter() {
        let entry = headers.entry(name.clone()).or_default();
        if name.eq_ignore_ascii_case("authorization") {
            entry.push(MASKED_AUTHORIZATION_VALUE.to_string());
        } else {
            entry.push(value.clone());
        }
    }
    // genai prepare may leave Authorization to the HTTP send path; surface the
    // same credential boundary for WebSocket wire audit (always masked).
    let has_authorization = headers
        .keys()
        .any(|name| name.eq_ignore_ascii_case("authorization"));
    if !has_authorization && api_key.is_some() {
        headers.insert(
            "authorization".to_string(),
            vec![MASKED_AUTHORIZATION_VALUE.to_string()],
        );
    }

    let create_frame_encoding = create_frame.as_ref().map(|_| "utf8".to_string());
    ProviderWebSocketRequestDump {
        format: PROVIDER_WEBSOCKET_REQUEST_DUMP_FORMAT.to_string(),
        version: PROVIDER_WEBSOCKET_REQUEST_DUMP_VERSION,
        url: ws_url.to_string(),
        headers,
        create_frame,
        create_frame_encoding,
        frame_sent,
        connection_reused,
        handshake,
    }
}
