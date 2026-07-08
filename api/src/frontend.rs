use axum::response::{IntoResponse, Response};

const INDEX_HTML: &str = "<!DOCTYPE html><html><head><title>AstrBotRS</title></head><body><h1>AstrBotRS</h1><p>Dashboard frontend not deployed. Build the original Vue 3 dashboard and copy dist/ to data/dist/.</p></body></html>";

pub async fn serve_static() -> impl IntoResponse {
    Response::builder()
        .header("Content-Type", "text/html")
        .body(axum::body::Body::from(INDEX_HTML))
        .unwrap()
}

pub async fn serve_frontend() -> impl IntoResponse {
    serve_static().await
}
