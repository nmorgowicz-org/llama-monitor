use std::sync::Arc;

use bytes::Bytes;
use warp::Filter;

use crate::config::AppConfig;
use crate::state::AppState;

use super::super::common::{ApiCtx, ApiRoute, box_reply, check_api_token, unauthorized_api_token};
use super::super::upstream::{
    build_upstream_client, prepare_inference_request, send_upstream_request_with_retry,
};

pub(crate) fn routes(ctx: ApiCtx) -> ApiRoute {
    let chat = api_chat(ctx.state.clone(), ctx.config.clone());
    let chat_abort = api_chat_abort(ctx.state.clone(), ctx.config.clone());

    chat.map(box_reply)
        .or(chat_abort.map(box_reply))
        .unify()
        .boxed()
}

fn api_chat(
    state: AppState,
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "chat")
        .and(warp::post())
        .and(warp::header::optional::<String>("authorization"))
        .and(warp::body::content_length_limit(2 * 1024 * 1024))
        .and(warp::body::bytes())
        .and_then(move |auth: Option<String>, body: Bytes| {
            let cfg = app_config.clone();
            let state = state.clone();
            async move {
                if !check_api_token(&auth, &cfg) {
                    return Ok(unauthorized_api_token());
                }
                let prepared = prepare_inference_request(&state).await?;
                let client = build_upstream_client(std::time::Duration::from_secs(120))?;
                let request_body = prepared.map_chat_body(&body)?;
                let url = prepared.url.clone();

                let resp = send_upstream_request_with_retry(|| {
                    prepared.authenticate(
                        client
                            .post(&url)
                            .header("Content-Type", "application/json")
                            .body(request_body.clone()),
                    )
                })
                .await?;
                let is_sse = resp
                    .headers()
                    .get(reqwest::header::CONTENT_TYPE)
                    .and_then(|value| value.to_str().ok())
                    .is_some_and(|value| value.starts_with("text/event-stream"));
                let backend = prepared.backend;
                let permit = prepared.permit;

                let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
                let stream = tokio_stream::wrappers::UnboundedReceiverStream::new(rx);

                tokio::spawn(async move {
                    let _permit = permit;
                    let mut stream = resp.bytes_stream();
                    let mut buf = String::new();

                    if !is_sse {
                        const MAX_NON_STREAM_BYTES: usize = 2 * 1024 * 1024;
                        let mut body = Vec::new();
                        while let Some(chunk) = next_chunk_or_disconnect(&tx, &mut stream).await {
                            match chunk {
                                Ok(bytes) if body.len() + bytes.len() <= MAX_NON_STREAM_BYTES => {
                                    body.extend_from_slice(&bytes);
                                }
                                Ok(_) => {
                                    let _ = tx.send(Ok::<_, warp::Error>(
                                        warp::sse::Event::default().data(
                                            r#"{"error":"Non-streaming response exceeded 2 MiB"}"#,
                                        ),
                                    ));
                                    return;
                                }
                                Err(error) => {
                                    let _ = tx.send(Ok::<_, warp::Error>(
                                        warp::sse::Event::default().data(error_event_data(&error)),
                                    ));
                                    return;
                                }
                            }
                        }
                        let data =
                            normalize_non_stream_response(backend, &body).unwrap_or_else(|| {
                                r#"{"error":"Upstream returned malformed non-streaming JSON"}"#
                                    .to_string()
                            });
                        let _ =
                            tx.send(Ok::<_, warp::Error>(warp::sse::Event::default().data(data)));
                        let _ = tx.send(Ok::<_, warp::Error>(
                            warp::sse::Event::default().data("[DONE]"),
                        ));
                        return;
                    }

                    const MAX_SSE_BUFFER_BYTES: usize = 2 * 1024 * 1024;
                    loop {
                        let chunk = next_chunk_or_disconnect(&tx, &mut stream).await;
                        let Some(chunk) = chunk else {
                            break;
                        };
                        match chunk {
                            Ok(bytes) => {
                                if tx.is_closed() {
                                    return;
                                }

                                buf.push_str(&String::from_utf8_lossy(&bytes));
                                if buf.len() > MAX_SSE_BUFFER_BYTES {
                                    let _ = tx.send(Ok::<_, warp::Error>(
                                        warp::sse::Event::default().data(
                                            r#"{"error":"Upstream SSE event exceeded 2 MiB"}"#,
                                        ),
                                    ));
                                    return;
                                }

                                while let Some(pos) = buf.find('\n') {
                                    let line = buf[..pos].to_string();
                                    buf = buf[pos + 1..].to_string();

                                    if let Some(data) = normalize_sse_line(backend, &line) {
                                        let _ = tx.send(Ok::<_, warp::Error>(
                                            warp::sse::Event::default().data(data),
                                        ));
                                    }
                                }
                            }
                            Err(e) => {
                                let _ = tx.send(Ok::<_, warp::Error>(
                                    warp::sse::Event::default().data(error_event_data(&e)),
                                ));
                                break;
                            }
                        }
                    }
                    if !buf.is_empty()
                        && let Some(data) = normalize_sse_line(backend, &buf)
                    {
                        let _ =
                            tx.send(Ok::<_, warp::Error>(warp::sse::Event::default().data(data)));
                    }
                });

                Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(warp::sse::reply(
                    stream,
                )))
            }
        })
}

fn normalize_sse_line(backend: crate::inference::InferenceBackend, line: &str) -> Option<String> {
    let data = line
        .strip_suffix('\r')
        .unwrap_or(line)
        .strip_prefix("data:")?
        .trim();
    (!data.is_empty())
        .then(|| normalize_sse_data(backend, data))
        .flatten()
}

fn api_chat_abort(
    state: AppState,
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "chat" / "abort")
        .and(warp::post())
        .and(warp::header::optional::<String>("authorization"))
        .and(warp::body::content_length_limit(1024))
        .and(warp::body::bytes())
        .and_then(move |auth: Option<String>, body: Bytes| {
            let cfg = app_config.clone();
            let state = state.clone();
            async move {
                if !check_api_token(&auth, &cfg) {
                    return Ok(unauthorized_api_token());
                }
                let request_id = parse_abort_request(&body).map_err(warp::reject::custom)?;
                let Some(request_id) = request_id else {
                    return Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(
                        warp::reply::json(&serde_json::json!({
                            "ok": true,
                            "cancelled": false,
                            "mode": "local_only"
                        })),
                    ));
                };
                validate_request_id(&request_id).map_err(warp::reject::custom)?;
                let session = state
                    .get_active_session()
                    .ok_or_else(warp::reject::not_found)?;
                let port = match session.mode {
                    crate::state::SessionMode::Spawn { port, .. } => port,
                    crate::state::SessionMode::Attach { .. } => {
                        return Ok(Box::new(warp::reply::json(&serde_json::json!({
                            "ok": true, "cancelled": false, "mode": "local_only",
                            "reason": "native cancellation is unavailable for attached sessions"
                        }))));
                    }
                };
                let adapter = state
                    .backend
                    .lock()
                    .map_err(|error| {
                        warp::reject::custom(super::super::ApiError::internal(error.to_string()))
                    })?
                    .clone();
                let Some(adapter) = adapter.filter(|adapter| {
                    super::super::upstream::adapter_matches_backend(session.backend, adapter)
                }) else {
                    return Ok(Box::new(warp::reply::json(&serde_json::json!({
                        "ok": true, "cancelled": false, "mode": "local_only"
                    }))));
                };
                if !adapter.capabilities().cancellation {
                    return Ok(Box::new(warp::reply::json(&serde_json::json!({
                        "ok": true, "cancelled": false, "mode": "local_only",
                        "reason": "active runtime does not advertise native cancellation"
                    }))));
                }
                adapter
                    .cancel_request(port, &request_id)
                    .await
                    .map_err(|error| {
                        warp::reject::custom(super::super::ApiError::gateway(format!(
                            "Native request cancellation failed: {error}"
                        )))
                    })?;
                Ok(Box::new(warp::reply::json(&serde_json::json!({
                    "ok": true, "cancelled": true, "mode": "native"
                }))))
            }
        })
}

fn normalize_sse_data(backend: crate::inference::InferenceBackend, data: &str) -> Option<String> {
    if data == "[DONE]" || backend == crate::inference::InferenceBackend::LlamaCpp {
        return Some(data.to_string());
    }
    let mut value: serde_json::Value = serde_json::from_str(data).ok()?;
    if let Some(delta) = value
        .get_mut("choices")
        .and_then(|choices| choices.as_array_mut())
        .and_then(|choices| choices.first_mut())
        .and_then(|choice| choice.get_mut("delta"))
        .and_then(|delta| delta.as_object_mut())
        && !delta.contains_key("reasoning_content")
        && let Some(reasoning) = delta
            .get("reasoning")
            .cloned()
            .or_else(|| delta.get("reasoning_text").cloned())
    {
        delta.insert("reasoning_content".to_string(), reasoning);
    }
    serde_json::to_string(&value).ok()
}

fn normalize_non_stream_response(
    backend: crate::inference::InferenceBackend,
    body: &[u8],
) -> Option<String> {
    let mut value: serde_json::Value = serde_json::from_slice(body).ok()?;
    if let Some(choice) = value
        .get_mut("choices")
        .and_then(|choices| choices.as_array_mut())
        .and_then(|choices| choices.first_mut())
        .and_then(|choice| choice.as_object_mut())
        && !choice.contains_key("delta")
        && let Some(message) = choice.remove("message")
    {
        choice.insert("delta".to_string(), message);
    }
    let serialized = serde_json::to_string(&value).ok()?;
    normalize_sse_data(backend, &serialized)
}

async fn next_chunk_or_disconnect<S, T>(
    sender: &tokio::sync::mpsc::UnboundedSender<T>,
    stream: &mut S,
) -> Option<S::Item>
where
    S: futures_util::Stream + Unpin,
{
    use futures_util::StreamExt;
    tokio::select! {
        () = sender.closed() => None,
        chunk = stream.next() => chunk,
    }
}

fn error_event_data(error: &impl std::fmt::Display) -> String {
    serde_json::json!({"error": error.to_string()}).to_string()
}

fn validate_request_id(request_id: &str) -> Result<(), super::super::ApiError> {
    if request_id.is_empty()
        || request_id.len() > 128
        || !request_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
    {
        return Err(super::super::ApiError::new(
            warp::http::StatusCode::BAD_REQUEST,
            "Invalid upstream request ID",
        ));
    }
    Ok(())
}

fn parse_abort_request(body: &[u8]) -> Result<Option<String>, super::super::ApiError> {
    if body.is_empty() {
        return Ok(None);
    }
    #[derive(serde::Deserialize)]
    struct AbortRequest {
        request_id: String,
    }
    serde_json::from_slice::<AbortRequest>(body)
        .map(|request| Some(request.request_id))
        .map_err(|error| {
            super::super::ApiError::new(
                warp::http::StatusCode::BAD_REQUEST,
                format!("Invalid chat abort request: {error}"),
            )
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rapid_sse_normalizes_reasoning_and_preserves_usage_tools_and_finish() {
        let data = r#"{"id":"chatcmpl-1","choices":[{"delta":{"reasoning":"think","tool_calls":[{"index":0}]},"finish_reason":"tool_calls"}],"usage":{"prompt_tokens":2,"completion_tokens":3}}"#;
        let normalized =
            normalize_sse_data(crate::inference::InferenceBackend::RapidMlx, data).unwrap();
        let value: serde_json::Value = serde_json::from_str(&normalized).unwrap();
        assert_eq!(value["choices"][0]["delta"]["reasoning_content"], "think");
        assert!(value["choices"][0]["delta"]["tool_calls"].is_array());
        assert_eq!(value["choices"][0]["finish_reason"], "tool_calls");
        assert_eq!(value["usage"]["completion_tokens"], 3);

        let usage_only = normalize_sse_data(
            crate::inference::InferenceBackend::RapidMlx,
            r#"{"choices":[],"usage":{"prompt_tokens":4,"completion_tokens":5}}"#,
        )
        .unwrap();
        let usage: serde_json::Value = serde_json::from_str(&usage_only).unwrap();
        assert_eq!(usage["usage"]["prompt_tokens"], 4);
    }

    #[test]
    fn malformed_rapid_chunks_are_dropped_without_changing_llama_forwarding() {
        assert!(normalize_sse_data(crate::inference::InferenceBackend::RapidMlx, "{bad").is_none());
        assert_eq!(
            normalize_sse_data(crate::inference::InferenceBackend::LlamaCpp, "{bad").as_deref(),
            Some("{bad")
        );
        assert_eq!(
            normalize_sse_data(crate::inference::InferenceBackend::RapidMlx, "[DONE]").as_deref(),
            Some("[DONE]")
        );
        let final_without_newline = normalize_sse_line(
            crate::inference::InferenceBackend::RapidMlx,
            r#"data:{"choices":[],"usage":{"completion_tokens":7}}"#,
        )
        .unwrap();
        let final_value: serde_json::Value = serde_json::from_str(&final_without_newline).unwrap();
        assert_eq!(final_value["usage"]["completion_tokens"], 7);
    }

    #[test]
    fn non_stream_response_maps_message_reasoning_usage_and_tools_to_one_chunk() {
        let normalized = normalize_non_stream_response(
            crate::inference::InferenceBackend::RapidMlx,
            br#"{"id":"chatcmpl-nonstream","choices":[{"message":{"content":"answer","reasoning_content":"thought","tool_calls":[{"id":"call-1"}]},"finish_reason":"stop"}],"usage":{"prompt_tokens":1,"completion_tokens":2}}"#,
        )
        .unwrap();
        let value: serde_json::Value = serde_json::from_str(&normalized).unwrap();
        assert_eq!(value["choices"][0]["delta"]["content"], "answer");
        assert_eq!(value["choices"][0]["delta"]["reasoning_content"], "thought");
        assert!(value["choices"][0]["delta"]["tool_calls"].is_array());
        assert_eq!(value["usage"]["completion_tokens"], 2);
        assert!(
            normalize_non_stream_response(
                crate::inference::InferenceBackend::RapidMlx,
                b"successful but invalid JSON"
            )
            .is_none()
        );
    }

    #[test]
    fn cancellation_ids_are_bounded_and_path_safe() {
        assert!(validate_request_id("chatcmpl-safe_1.2").is_ok());
        assert!(validate_request_id("").is_err());
        assert!(validate_request_id("../escape").is_err());
        assert!(validate_request_id(&"x".repeat(129)).is_err());
    }

    #[test]
    fn abort_body_preserves_empty_compatibility_and_rejects_bad_json_shapes() {
        assert_eq!(parse_abort_request(b"").unwrap(), None);
        assert_eq!(
            parse_abort_request(br#"{"request_id":"chatcmpl-1"}"#).unwrap(),
            Some("chatcmpl-1".to_string())
        );
        for malformed in [
            b"not-json".as_slice(),
            br#"{}"#.as_slice(),
            br#"[]"#.as_slice(),
            br#"{"request_id":7}"#.as_slice(),
        ] {
            let error = parse_abort_request(malformed).unwrap_err();
            assert_eq!(error.status, warp::http::StatusCode::BAD_REQUEST);
        }
    }

    #[tokio::test]
    async fn receiver_disconnect_stops_waiting_on_upstream_immediately() {
        let (sender, receiver) = tokio::sync::mpsc::unbounded_channel::<()>();
        let mut upstream = futures_util::stream::pending::<Result<bytes::Bytes, reqwest::Error>>();
        drop(receiver);
        let result = tokio::time::timeout(
            std::time::Duration::from_millis(50),
            next_chunk_or_disconnect(&sender, &mut upstream),
        )
        .await
        .expect("disconnect must not wait for another upstream byte");
        assert!(result.is_none());
    }

    #[test]
    fn stream_errors_are_always_valid_json() {
        let data = error_event_data(&"upstream \\\"failure\\\"\nnext line");
        let value: serde_json::Value = serde_json::from_str(&data).unwrap();
        assert_eq!(value["error"], "upstream \\\"failure\\\"\nnext line");
    }
}
