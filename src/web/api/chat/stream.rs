use std::sync::Arc;

use bytes::Bytes;
use warp::Filter;

use crate::config::AppConfig;
use crate::state::AppState;

use super::super::common::{ApiCtx, ApiRoute, check_api_token, unauthorized_api_token};
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

fn box_reply<R>(reply: R) -> Box<dyn warp::reply::Reply>
where
    R: warp::Reply + 'static,
{
    Box::new(reply)
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
                let (url, permit) = prepare_inference_request(&state).await?;
                let client = build_upstream_client(std::time::Duration::from_secs(120))?;
                let request_body = body.to_vec();

                let resp = send_upstream_request_with_retry(|| {
                    client
                        .post(&url)
                        .header("Content-Type", "application/json")
                        .body(request_body.clone())
                })
                .await?;

                let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
                let stream = tokio_stream::wrappers::UnboundedReceiverStream::new(rx);

                tokio::spawn(async move {
                    use futures_util::StreamExt;
                    let _permit = permit;
                    let mut stream = resp.bytes_stream();
                    let mut buf = String::new();

                    while let Some(chunk) = stream.next().await {
                        match chunk {
                            Ok(bytes) => {
                                if tx.is_closed() {
                                    return;
                                }

                                buf.push_str(&String::from_utf8_lossy(&bytes));

                                while let Some(pos) = buf.find('\n') {
                                    let line = buf[..pos].to_string();
                                    buf = buf[pos + 1..].to_string();

                                    if let Some(data) = line.strip_prefix("data: ")
                                        && !data.trim().is_empty()
                                    {
                                        let _ = tx.send(Ok::<_, warp::Error>(
                                            warp::sse::Event::default().data(data.to_string()),
                                        ));
                                    }
                                }
                            }
                            Err(e) => {
                                let _ = tx.send(Ok::<_, warp::Error>(
                                    warp::sse::Event::default().data(format!(
                                        "{{\"error\":\"{}\"}}",
                                        e.to_string().replace('"', "'")
                                    )),
                                ));
                                break;
                            }
                        }
                    }
                });

                Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(warp::sse::reply(
                    stream,
                )))
            }
        })
}

fn api_chat_abort(
    _state: AppState,
    app_config: Arc<AppConfig>,
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "chat" / "abort")
        .and(warp::post())
        .and(warp::header::optional::<String>("authorization"))
        .and_then(move |auth: Option<String>| {
            let cfg = app_config.clone();
            async move {
                if !check_api_token(&auth, &cfg) {
                    return Ok(unauthorized_api_token());
                }
                Ok::<Box<dyn warp::reply::Reply>, warp::Rejection>(Box::new(warp::reply::json(
                    &serde_json::json!({"ok": true}),
                )))
            }
        })
}
