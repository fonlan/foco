use std::sync::{Arc, Mutex};

use async_stream::stream;
use futures_util::{SinkExt, StreamExt};
use genai::{
    PreparedChatStreamRequest,
    adapter::{OpenAIRespEventDecoder, openai_resp_websocket_create_payload},
    chat::{ChatOptions, ChatStreamEvent},
};
use tokio_tungstenite::{
    connect_async,
    tungstenite::{
        Message,
        client::IntoClientRequest,
        http::{HeaderName, HeaderValue},
    },
};

use crate::{
    MASKED_AUTHORIZATION_VALUE, NeutralChatStream, PROVIDER_WIRE_REQUEST_DUMP_FORMAT,
    PROVIDER_WIRE_REQUEST_DUMP_VERSION, ProviderConfigError, ProviderConnectionConfig,
    ProviderErrorContext, ProviderHttpHeadersDump, ProviderHttpResponseHeadDump,
    ProviderRequestDumpObserver, ProviderRequestFailure, ProviderWireRequestDump,
    redact_json_body_credentials, websocket_url_from_responses_http_url,
};

pub(crate) async fn stream_chat_openai_resp_websocket(
    config: &ProviderConnectionConfig,
    chat_request: genai::chat::ChatRequest,
    options: ChatOptions,
    model: genai::ModelIden,
    error_context: ProviderErrorContext,
    capture_details: bool,
    request_observer: Option<ProviderRequestDumpObserver>,
) -> Result<NeutralChatStream, ProviderRequestFailure> {
    let client = config
        .genai_client()
        .map_err(|error| ProviderRequestFailure {
            error,
            request_dump: None,
        })?;

    let prepared = client
        .prepare_chat_stream_request(model.clone(), chat_request, Some(&options))
        .await
        .map_err(|source| ProviderRequestFailure {
            error: ProviderConfigError::from_genai_error_with_context(source, &error_context),
            request_dump: None,
        })?;

    let ws_url = websocket_url_from_responses_http_url(&prepared.url).map_err(|error| {
        ProviderRequestFailure {
            error,
            request_dump: None,
        }
    })?;

    let create_payload = openai_resp_websocket_create_payload(prepared.payload.clone());
    let wire_request_dump = if capture_details || request_observer.is_some() {
        Some(websocket_wire_request_dump(
            &ws_url,
            &prepared,
            &create_payload,
        ))
    } else {
        None
    };

    if let Some(dump) = wire_request_dump.as_ref()
        && let Some(observer) = request_observer.as_ref()
    {
        observer(dump);
    }

    let mut request =
        ws_url
            .as_str()
            .into_client_request()
            .map_err(|source| ProviderRequestFailure {
                error: ProviderConfigError::Connection {
                    message: format!("{error_context}: invalid WebSocket request: {source}"),
                    status_code: None,
                },
                request_dump: wire_request_dump.clone(),
            })?;

    for (name, value) in prepared.headers.iter() {
        // Host/connection headers are managed by the WebSocket client.
        if name.eq_ignore_ascii_case("host")
            || name.eq_ignore_ascii_case("connection")
            || name.eq_ignore_ascii_case("upgrade")
            || name.eq_ignore_ascii_case("sec-websocket-key")
            || name.eq_ignore_ascii_case("sec-websocket-version")
        {
            continue;
        }
        let header_name: HeaderName = name.parse().map_err(|source| ProviderRequestFailure {
            error: ProviderConfigError::InvalidRequest(format!(
                "invalid WebSocket header name '{name}': {source}"
            )),
            request_dump: wire_request_dump.clone(),
        })?;
        let header_value =
            HeaderValue::from_str(value).map_err(|source| ProviderRequestFailure {
                error: ProviderConfigError::InvalidRequest(format!(
                    "invalid WebSocket header value for '{name}': {source}"
                )),
                request_dump: wire_request_dump.clone(),
            })?;
        request.headers_mut().insert(header_name, header_value);
    }

    let (ws_stream, response) =
        connect_async(request)
            .await
            .map_err(|source| ProviderRequestFailure {
                error: ProviderConfigError::Connection {
                    message: format!("{error_context}: WebSocket connect failed: {source}"),
                    status_code: None,
                },
                request_dump: wire_request_dump.clone(),
            })?;

    let handshake_status = response.status().as_u16();
    let response_status = Arc::new(Mutex::new(Some(handshake_status)));
    let response_head = capture_details.then(|| {
        Arc::new(Mutex::new(Some(ProviderHttpResponseHeadDump {
            status: handshake_status,
            version: "HTTP/1.1".to_string(),
            headers: response.headers().iter().fold(
                ProviderHttpHeadersDump::new(),
                |mut map, (name, value)| {
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
                },
            ),
        })))
    });

    let (mut write, mut read) = ws_stream.split();
    let create_text =
        serde_json::to_string(&create_payload).map_err(|source| ProviderRequestFailure {
            error: ProviderConfigError::InvalidRequest(format!(
                "failed to serialize response.create: {source}"
            )),
            request_dump: wire_request_dump.clone(),
        })?;

    write
        .send(Message::Text(create_text.into()))
        .await
        .map_err(|source| ProviderRequestFailure {
            error: ProviderConfigError::Connection {
                message: format!("{error_context}: failed to send response.create: {source}"),
                status_code: Some(handshake_status),
            },
            request_dump: wire_request_dump.clone(),
        })?;

    let mut decoder = OpenAIRespEventDecoder::from_chat_options(model, &options);
    let stream_error_context = error_context.with_phase("reading provider stream");

    let event_stream = stream! {
        yield Ok(ChatStreamEvent::Start);

        loop {
            match read.next().await {
                Some(Ok(Message::Text(text))) => {
                    match decoder.decode_json_to_chat_event(text.as_str()) {
                        Ok(Some(event)) => {
                            let done = matches!(event, ChatStreamEvent::End(_));
                            yield Ok(event);
                            if done {
                                break;
                            }
                        }
                        Ok(None) => {}
                        Err(error) => {
                            yield Err(error);
                            break;
                        }
                    }
                }
                Some(Ok(Message::Binary(bytes))) => {
                    let Ok(text) = std::str::from_utf8(&bytes) else {
                        continue;
                    };
                    match decoder.decode_json_to_chat_event(text) {
                        Ok(Some(event)) => {
                            let done = matches!(event, ChatStreamEvent::End(_));
                            yield Ok(event);
                            if done {
                                break;
                            }
                        }
                        Ok(None) => {}
                        Err(error) => {
                            yield Err(error);
                            break;
                        }
                    }
                }
                Some(Ok(Message::Ping(payload))) => {
                    if let Err(error) = write.send(Message::Pong(payload)).await {
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
                    if let Some(end) = decoder.finish_on_eof_to_chat_event() {
                        yield Ok(end);
                    } else if !decoder.is_done() {
                        // EOF without partial capture still surfaces as stream end without Complete
                        // at the Neutral layer when no End event is produced.
                    }
                    break;
                }
                Some(Err(error)) => {
                    yield Err(genai::Error::WebStream {
                        model_iden: decoder.model_iden().clone(),
                        cause: error.to_string(),
                        error: Box::new(error),
                    });
                    break;
                }
            }
        }

        let _ = write.close().await;
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

fn websocket_wire_request_dump(
    ws_url: &str,
    prepared: &PreparedChatStreamRequest,
    create_payload: &serde_json::Value,
) -> ProviderWireRequestDump {
    let body = serde_json::to_string(create_payload)
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

    let body_encoding = body.as_ref().map(|_| "utf8".to_string());
    ProviderWireRequestDump {
        format: PROVIDER_WIRE_REQUEST_DUMP_FORMAT.to_string(),
        version: PROVIDER_WIRE_REQUEST_DUMP_VERSION,
        method: "WEBSOCKET".to_string(),
        url: ws_url.to_string(),
        headers,
        body,
        body_encoding,
    }
}
