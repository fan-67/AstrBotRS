pub mod auth;
pub mod bots;
pub mod config;
pub mod conversations;
pub mod logs;
pub mod operations;
pub mod plugins;
pub mod providers;
pub mod stats;

use axum::{http::StatusCode, response::IntoResponse, Json};
use serde_json::json;

pub fn api_error(status: StatusCode, message: impl Into<String>) -> impl IntoResponse {
    let body = json!({
        "error": message.into(),
        "code": status.as_u16(),
    });
    (status, Json(body))
}

pub fn api_ok<T: serde::Serialize>(data: T) -> impl IntoResponse {
    (StatusCode::OK, Json(data))
}
