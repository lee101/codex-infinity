use crate::common::ResponseEvent;
use crate::common::ResponseStream;
use crate::error::ApiError;
use crate::requests::chat_tools::ChatToolMapping;
use crate::requests::chat_tools::restore_tool_call;
use crate::telemetry::SseTelemetry;
use codex_client::StreamResponse;
use codex_protocol::models::ContentItem;
use codex_protocol::models::ReasoningItemContent;
use codex_protocol::models::ResponseItem;
use eventsource_stream::Eventsource;
use futures::Stream;
use futures::StreamExt;
use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::time::Instant;
use tokio::time::timeout;

pub(crate) fn spawn_chat_stream(
    response: StreamResponse,
    idle_timeout: Duration,
    telemetry: Option<Arc<dyn SseTelemetry>>,
    tool_mapping: ChatToolMapping,
) -> ResponseStream {
    let upstream_request_id = response
        .headers
        .get("x-request-id")
        .and_then(|value| value.to_str().ok())
        .map(str::to_string);
    let (tx_event, rx_event) = mpsc::channel(1600);
    tokio::spawn(process_chat_sse(
        response.bytes,
        tx_event,
        idle_timeout,
        telemetry,
        tool_mapping,
    ));
    ResponseStream {
        routing_affinity: None,
        rx_event,
        upstream_request_id,
        interrupt: None,
    }
}

#[derive(Default)]
struct ToolCallState {
    id: Option<String>,
    name: Option<String>,
    arguments: String,
}

pub(crate) async fn process_chat_sse<S>(
    stream: S,
    tx: mpsc::Sender<Result<ResponseEvent, ApiError>>,
    idle_timeout: Duration,
    telemetry: Option<Arc<dyn SseTelemetry>>,
    tool_mapping: ChatToolMapping,
) where
    S: Stream<Item = Result<bytes::Bytes, codex_client::TransportError>> + Unpin,
{
    let mut stream = stream.eventsource();
    let mut text = String::new();
    let mut reasoning = String::new();
    let mut text_started = false;
    let mut reasoning_started = false;
    let mut tool_calls = BTreeMap::<usize, ToolCallState>::new();

    loop {
        let started = Instant::now();
        let polled = timeout(idle_timeout, stream.next()).await;
        if let Some(telemetry) = telemetry.as_ref() {
            telemetry.on_sse_poll(&polled, started.elapsed());
        }
        let event = match polled {
            Ok(Some(Ok(event))) => event,
            Ok(Some(Err(error))) => {
                let _ = tx.send(Err(ApiError::Stream(error.to_string()))).await;
                return;
            }
            Ok(None) => break,
            Err(_) => {
                let _ = tx
                    .send(Err(ApiError::Stream("idle timeout waiting for SSE".into())))
                    .await;
                return;
            }
        };
        let data = event.data.trim();
        if data.is_empty() {
            continue;
        }
        if data == "[DONE]" || data == "DONE" {
            break;
        }
        let Ok(value) = serde_json::from_str::<serde_json::Value>(data) else {
            continue;
        };
        let Some(choices) = value.get("choices").and_then(|value| value.as_array()) else {
            continue;
        };
        for choice in choices {
            if choice.get("finish_reason").and_then(|value| value.as_str()) == Some("length") {
                let _ = tx.send(Err(ApiError::ContextWindowExceeded)).await;
                return;
            }
            let Some(delta) = choice.get("delta") else {
                continue;
            };
            if let Some(chunk) = delta.get("content").and_then(|value| value.as_str()) {
                if !text_started {
                    text_started = true;
                    let _ = tx
                        .send(Ok(ResponseEvent::OutputItemAdded(ResponseItem::Message {
                            encrypted_content: None,
                            status: None,
                            id: None,
                            role: "assistant".into(),
                            content: Vec::new(),
                            phase: None,
                            internal_chat_message_metadata_passthrough: None,
                        })))
                        .await;
                }
                text.push_str(chunk);
                let _ = tx
                    .send(Ok(ResponseEvent::OutputTextDelta(chunk.into())))
                    .await;
            }
            let reasoning_chunk = delta
                .get("reasoning_content")
                .or_else(|| delta.get("reasoning"))
                .and_then(|value| value.as_str());
            if let Some(chunk) = reasoning_chunk {
                if !reasoning_started {
                    reasoning_started = true;
                    let _ = tx
                        .send(Ok(ResponseEvent::OutputItemAdded(
                            ResponseItem::Reasoning {
                                id: None,
                                summary: Vec::new(),
                                content: Some(Vec::new()),
                                encrypted_content: None,
                                internal_chat_message_metadata_passthrough: None,
                            },
                        )))
                        .await;
                }
                reasoning.push_str(chunk);
                let _ = tx
                    .send(Ok(ResponseEvent::ReasoningContentDelta {
                        delta: chunk.into(),
                        content_index: 0,
                    }))
                    .await;
            }
            if let Some(calls) = delta.get("tool_calls").and_then(|value| value.as_array()) {
                for call in calls {
                    let index = call
                        .get("index")
                        .and_then(serde_json::Value::as_u64)
                        .map(|value| value as usize)
                        .unwrap_or(tool_calls.len());
                    let state = tool_calls.entry(index).or_default();
                    if let Some(id) = call.get("id").and_then(|value| value.as_str()) {
                        state.id = Some(id.into());
                    }
                    if let Some(function) = call.get("function") {
                        if let Some(name) = function.get("name").and_then(|value| value.as_str())
                            && !name.is_empty()
                        {
                            state.name = Some(name.into());
                        }
                        if let Some(arguments) =
                            function.get("arguments").and_then(|value| value.as_str())
                        {
                            state.arguments.push_str(arguments);
                        }
                    }
                }
            }
        }
    }

    if !reasoning.is_empty() {
        let _ = tx
            .send(Ok(ResponseEvent::OutputItemDone(ResponseItem::Reasoning {
                id: None,
                summary: Vec::new(),
                content: Some(vec![ReasoningItemContent::ReasoningText {
                    text: reasoning,
                }]),
                encrypted_content: None,
                internal_chat_message_metadata_passthrough: None,
            })))
            .await;
    }
    if !text.is_empty() {
        let _ = tx
            .send(Ok(ResponseEvent::OutputItemDone(ResponseItem::Message {
                encrypted_content: None,
                status: None,
                id: None,
                role: "assistant".into(),
                content: vec![ContentItem::OutputText {
                    text,
                    annotations: None,
                    logprobs: None,
                }],
                phase: None,
                internal_chat_message_metadata_passthrough: None,
            })))
            .await;
    }
    for (index, call) in tool_calls {
        let Some(name) = call.name else { continue };
        let item = restore_tool_call(
            &tool_mapping,
            name,
            call.arguments,
            call.id.unwrap_or_else(|| format!("tool-call-{index}")),
        );
        let _ = tx.send(item.map(ResponseEvent::OutputItemDone)).await;
    }
    let _ = tx
        .send(Ok(ResponseEvent::Completed {
            response_id: String::new(),
            token_usage: None,
            end_turn: None,
            usage_metadata: None,
            reasoning_removals: None,
        }))
        .await;
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::TryStreamExt;
    use tokio_util::io::ReaderStream;

    #[tokio::test]
    async fn parses_deepseek_reasoning_text_and_tool_calls() {
        let body = concat!(
            "data: {\"choices\":[{\"delta\":{\"reasoning_content\":\"think\"}}]}\n\n",
            "data: {\"choices\":[{\"delta\":{\"content\":\"answer\"}}]}\n\n",
            "data: {\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":0,\"id\":\"call-1\",\"function\":{\"name\":\"run\",\"arguments\":\"{}\"}}]}}]}\n\n",
            "data: [DONE]\n\n"
        );
        let stream = ReaderStream::new(std::io::Cursor::new(body))
            .map_err(|error| codex_client::TransportError::Network(error.to_string()));
        let (tx, mut rx) = mpsc::channel(16);
        process_chat_sse(
            stream,
            tx,
            Duration::from_secs(1),
            /*telemetry*/ None,
            ChatToolMapping::new(),
        )
        .await;
        let mut events = Vec::new();
        while let Some(event) = rx.recv().await {
            events.push(event.unwrap());
        }
        assert!(events.iter().any(|event| matches!(
            event,
            ResponseEvent::OutputItemDone(ResponseItem::Reasoning { .. })
        )));
        assert!(events.iter().any(|event| matches!(
            event,
            ResponseEvent::OutputItemDone(ResponseItem::FunctionCall { call_id, .. }) if call_id == "call-1"
        )));
        assert!(matches!(
            events.last(),
            Some(ResponseEvent::Completed { .. })
        ));
    }
}
