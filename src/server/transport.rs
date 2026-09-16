use super::{Inner, entry::Entry, protocol::*};
use crate::session::Error;
use axum::{
    Json, Router,
    body::Body,
    extract::{
        Path, State, WebSocketUpgrade,
        ws::{Message, WebSocket},
    },
    http::{Request as HttpRequest, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Response as HttpResponse},
    routing::{get, post},
};
use serde_json::{Value, json};
use std::{sync::Arc, time::Duration};

pub(super) fn router(inner: Arc<Inner>) -> Router {
    Router::new()
        .route("/v1/sessions", get(list).post(create))
        .route("/v1/sessions/{id}", get(detail))
        .route("/v1/sessions/{id}/stop", post(stop))
        .route("/v1/sessions/{id}/connect", get(connect))
        .route("/v1/stop", post(end))
        .layer(middleware::from_fn(check_origin))
        .with_state(inner)
}

async fn check_origin(request: HttpRequest<Body>, next: Next) -> HttpResponse {
    if request.headers().contains_key("origin") {
        return (StatusCode::FORBIDDEN, Json(json!({"version":VERSION,"error":{"code":"unsupported_origin","message":"browser-origin requests are not supported"}}))).into_response();
    }
    next.run(request).await
}
fn success(data: impl serde::Serialize) -> HttpResponse {
    Json(json!({"version":VERSION,"data":data})).into_response()
}
fn failure(e: Error) -> HttpResponse {
    (
        StatusCode::BAD_REQUEST,
        Json(json!({"version":VERSION,"error":e})),
    )
        .into_response()
}
async fn list(State(inner): State<Arc<Inner>>) -> HttpResponse {
    let items: Vec<_> = inner
        .list()
        .into_iter()
        .map(|d| json!({"id":d.id,"state":d.state,"created_at":d.created_at}))
        .collect();
    success(items)
}
async fn create(State(inner): State<Arc<Inner>>, body: String) -> HttpResponse {
    let result = serde_json::from_str::<Create>(&body)
        .map_err(|e| Error::new("invalid_config", e))
        .and_then(|c| inner.create(c));
    match result {
        Ok(detail) => success(detail),
        Err(e) => failure(e),
    }
}
async fn detail(State(inner): State<Arc<Inner>>, Path(id): Path<String>) -> HttpResponse {
    match inner.select(&id) {
        Ok(entry) => success(entry.detail()),
        Err(e) => failure(e),
    }
}
async fn stop(State(inner): State<Arc<Inner>>, Path(id): Path<String>) -> HttpResponse {
    match inner.select(&id) {
        Ok(entry) => {
            entry.stop();
            success(entry.detail())
        }
        Err(e) => failure(e),
    }
}
async fn end(State(inner): State<Arc<Inner>>) -> HttpResponse {
    inner.stop(false);
    success(json!({"state":"Stopping"}))
}
async fn connect(
    State(inner): State<Arc<Inner>>,
    Path(id): Path<String>,
    upgrade: WebSocketUpgrade,
) -> HttpResponse {
    match inner.select(&id) {
        Ok(entry) => upgrade.on_upgrade(move |socket| connection(socket, inner, entry)),
        Err(e) => failure(e),
    }
}
async fn connection(mut socket: WebSocket, inner: Arc<Inner>, entry: Arc<Entry>) {
    while let Some(Ok(message)) = socket.recv().await {
        let Message::Text(text) = message else {
            if matches!(message, Message::Close(_)) {
                break;
            }
            continue;
        };
        let decoded = serde_json::from_str::<Request>(&text);
        let (call, result) = match decoded {
            Ok(request) if request.version != VERSION => (
                request.call,
                Result::Error {
                    code: "unsupported_client_version".into(),
                    message: "expected outer version 1".into(),
                },
            ),
            Ok(request) => {
                let result = match request.operation {
                    Operation::Submit { command } => {
                        if inner.directory.lock().unwrap().deadline.is_some() {
                            Error::new("server_stopping", "new work is not accepted").into()
                        } else {
                            entry.submit(command).await
                        }
                    }
                    Operation::Poll { cursor, wait_ms } => {
                        poll(&entry, cursor.as_ref(), wait_ms).await
                    }
                };
                (request.call, result)
            }
            Err(e) => (
                serde_json::from_str::<Value>(&text)
                    .ok()
                    .and_then(|v| v["call"].as_u64())
                    .unwrap_or(0),
                Result::Error {
                    code: "invalid_request".into(),
                    message: e.to_string(),
                },
            ),
        };
        let response = Response {
            version: VERSION,
            call,
            result,
        };
        if socket
            .send(Message::Text(
                serde_json::to_string(&response).unwrap().into(),
            ))
            .await
            .is_err()
        {
            break;
        }
    }
}
async fn poll(entry: &Entry, cursor: Option<&Cursor>, wait_ms: u64) -> Result {
    if wait_ms > 30_000 {
        return Error::new("invalid_wait", "wait_ms must be <= 30000").into();
    }
    // Register before reading, including notifications occurring between the snapshot and await.
    let notified = entry.changed.notified();
    tokio::pin!(notified);
    notified.as_mut().enable();
    let result = entry.poll(cursor);
    if wait_ms == 0 || !matches!(&result, Result::Activity { entries, .. } if entries.is_empty()) {
        return result;
    }
    let _ = tokio::time::timeout(Duration::from_millis(wait_ms), notified).await;
    entry.poll(cursor)
}
