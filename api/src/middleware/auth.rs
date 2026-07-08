use axum::{
    extract::{Request, State},
    http::StatusCode,
    middleware::Next,
    response::{IntoResponse, Response},
};
use jsonwebtoken::{decode, DecodingKey, Validation};

use crate::app::AppState;

#[derive(Debug, serde::Deserialize)]
struct Claims {
    sub: String,
    exp: usize,
}

pub async fn require_auth(
    State(state): State<AppState>,
    mut req: Request,
    next: Next,
) -> Response {
    let path = req.uri().path();
    if path.ends_with("/auth/login") || path.ends_with("/operations/health") {
        return next.run(req).await;
    }

    let token = req
        .headers()
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .map(|s| s.to_string());

    let Some(token) = token else {
        return (StatusCode::UNAUTHORIZED, "missing authorization header").into_response();
    };

    let secret = state.jwt_secret.read().await.clone();
    match decode::<Claims>(
        &token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &Validation::default(),
    ) {
        Ok(data) => {
            req.extensions_mut().insert(data.claims.sub);
            next.run(req).await
        }
        Err(e) => {
            (StatusCode::UNAUTHORIZED, format!("invalid token: {e}")).into_response()
        }
    }
}
