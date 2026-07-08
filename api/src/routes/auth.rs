use std::time::{SystemTime, UNIX_EPOCH};

use axum::{extract::State, http::StatusCode, response::IntoResponse, Json, Router};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};

use crate::app::AppState;

#[derive(Debug, Serialize, Deserialize)]
struct Claims {
    sub: String,
    exp: usize,
    iat: usize,
}

#[derive(Debug, Deserialize)]
struct LoginRequest {
    username: String,
    password: String,
}

fn jwt_secret(state: &AppState) -> String {
    state
        .config
        .blocking_read()
        .dashboard
        .jwt_secret
        .clone()
        .unwrap_or_else(|| "astrbot_default_secret_change_me".to_string())
}

async fn login(
    State(state): State<AppState>,
    Json(req): Json<LoginRequest>,
) -> impl IntoResponse {
    let (valid_username, valid_password) = {
        let config = state.config.read().await;
        (
            config
                .dashboard
                .username
                .clone()
                .unwrap_or_else(|| "astrbot".to_string()),
            config
                .dashboard
                .password
                .clone()
                .unwrap_or_else(|| "astrbot".to_string()),
        )
    };

    if req.username != valid_username || req.password != valid_password {
        return (
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({"error": "invalid credentials"})),
        );
    }

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as usize;

    let claims = Claims {
        sub: req.username,
        exp: now + 86400,
        iat: now,
    };

    let secret = jwt_secret(&state);
    match encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    ) {
        Ok(token) => (
            StatusCode::OK,
            Json(serde_json::json!({"token": token})),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": format!("jwt: {e}")})),
        ),
    }
}

async fn verify(State(state): State<AppState>, headers: axum::http::HeaderMap) -> impl IntoResponse {
    let token = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .unwrap_or("")
        .to_string();

    let secret = jwt_secret(&state);
    match decode::<Claims>(
        &token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &Validation::default(),
    ) {
        Ok(data) => (
            StatusCode::OK,
            Json(serde_json::json!({"username": data.claims.sub})),
        ),
        Err(_) => (
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({"error": "invalid token"})),
        ),
    }
}

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/login", axum::routing::post(login))
        .route("/verify", axum::routing::get(verify))
}
