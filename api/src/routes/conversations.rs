use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
    Router,
};
use astrbot_db::Conversation;
use serde_json::json;

use crate::app::AppState;

async fn list_conversations(State(state): State<AppState>) -> impl IntoResponse {
    match sqlx::query_as::<_, Conversation>(
        "SELECT id, umo, messages_json, created_at, updated_at FROM conversations ORDER BY updated_at DESC",
    )
    .fetch_all(&state.db_pool)
    .await
    {
        Ok(convs) => (StatusCode::OK, Json(json!(convs))),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": e.to_string()})),
        ),
    }
}

async fn get_conversation(
    State(state): State<AppState>,
    Path(umo): Path<String>,
) -> impl IntoResponse {
    match sqlx::query_as::<_, Conversation>(
        "SELECT id, umo, messages_json, created_at, updated_at FROM conversations WHERE umo = ?",
    )
    .bind(&umo)
    .fetch_optional(&state.db_pool)
    .await
    {
        Ok(Some(conv)) => (StatusCode::OK, Json(json!(conv))),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "conversation not found"})),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": e.to_string()})),
        ),
    }
}

async fn delete_conversation(
    State(state): State<AppState>,
    Path(umo): Path<String>,
) -> impl IntoResponse {
    match sqlx::query("DELETE FROM conversations WHERE umo = ?")
        .bind(&umo)
        .execute(&state.db_pool)
        .await
    {
        Ok(result) if result.rows_affected() > 0 => {
            (StatusCode::OK, Json(json!({"status": "deleted"})))
        }
        Ok(_) => (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "conversation not found"})),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": e.to_string()})),
        ),
    }
}

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/", axum::routing::get(list_conversations))
        .route("/{umo}", axum::routing::get(get_conversation).delete(delete_conversation))
}
