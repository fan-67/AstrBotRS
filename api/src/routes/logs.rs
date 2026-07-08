use axum::{
    extract::State,
    response::{
        sse::{Event, Sse},
        IntoResponse,
    },
    Router,
};
use serde_json::json;
use std::convert::Infallible;

use crate::app::AppState;

async fn stream_logs(
    State(state): State<AppState>,
) -> impl IntoResponse {
    let rx = state.log_broker.subscribe();
    let recent = state.log_broker.recent(100);

    let stream = async_stream::stream! {
        // Send recent logs first
        for entry in recent {
            let data = json!({
                "timestamp": entry.timestamp.to_rfc3339(),
                "level": entry.level,
                "target": entry.target,
                "message": entry.message,
            });
            yield Ok::<_, Infallible>(Event::default().data(data.to_string()));
        }

        // Then stream new logs
        let mut rx = rx;
        while let Ok(entry) = rx.recv().await {
            let data = json!({
                "timestamp": entry.timestamp.to_rfc3339(),
                "level": entry.level,
                "target": entry.target,
                "message": entry.message,
            });
            yield Ok::<_, Infallible>(Event::default().data(data.to_string()));
        }
    };

    Sse::new(stream).keep_alive(
        axum::response::sse::KeepAlive::new()
            .interval(std::time::Duration::from_secs(15))
            .text("keepalive"),
    )
}

async fn recent_logs(State(state): State<AppState>) -> impl IntoResponse {
    let entries = state.log_broker.recent(200);
    let logs: Vec<serde_json::Value> = entries
        .into_iter()
        .map(|e| {
            json!({
                "timestamp": e.timestamp.to_rfc3339(),
                "level": e.level,
                "target": e.target,
                "message": e.message,
            })
        })
        .collect();
    axum::Json(logs)
}

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/stream", axum::routing::get(stream_logs))
        .route("/recent", axum::routing::get(recent_logs))
}
