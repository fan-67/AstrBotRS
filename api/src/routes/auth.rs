use std::collections::HashMap;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use axum::{extract::State, http::StatusCode, response::IntoResponse, Json, Router};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;

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

fn jwt_secret(state: &AppState) -> Arc<RwLock<String>> {
    state.jwt_secret.clone()
}

async fn get_secret(state: &AppState) -> String {
    jwt_secret(state).read().await.clone()
}

static LOGIN_ATTEMPTS: std::sync::LazyLock<std::sync::Mutex<HashMap<String, (u32, u64)>>> =
    std::sync::LazyLock::new(Default::default);

fn check_rate_limit(ip: &str) -> bool {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let mut attempts = LOGIN_ATTEMPTS.lock().unwrap();
    let entry = attempts.entry(ip.to_string()).or_insert((0, now));
    if now - entry.1 > 60 {
        *entry = (1, now);
        return true;
    }
    if entry.0 >= 5 {
        return false;
    }
    entry.0 += 1;
    true
}

async fn login(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
    Json(req): Json<LoginRequest>,
) -> impl IntoResponse {
    let ip = headers
        .get("x-forwarded-for")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("unknown");
    if !check_rate_limit(ip) {
        return (
            StatusCode::TOO_MANY_REQUESTS,
            Json(serde_json::json!({"error": "too many login attempts, try again later"})),
        );
    }

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

    let secret = get_secret(&state).await;
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

    let secret = get_secret(&state).await;
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
