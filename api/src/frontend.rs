use axum::http::StatusCode;
use std::path::PathBuf;
use tower_http::services::ServeDir;

const FALLBACK_HTML: &str = r#"<!DOCTYPE html>
<html><head><title>AstrBotRS</title></head>
<body><h1>AstrBotRS</h1>
<p>Dashboard frontend not deployed.</p>
<p>Build the original Vue 3 dashboard and copy <code>dashboard/dist/</code> to <code>data/dist/</code>.</p>
<pre>
  cd astrbot_refactor/dashboard
  npm install -g pnpm
  pnpm install && pnpm build
  cp -r dist /workspace/temp/astrbot_rs/data/dist
</pre>
</body></html>"#;

pub struct FrontendService {
    dist_dir: PathBuf,
}

impl FrontendService {
    pub fn new(dist_dir: PathBuf) -> Self {
        Self { dist_dir }
    }

    pub fn into_router(self) -> axum::Router {
        let dist_exists = self.dist_dir.join("index.html").exists();
        if !dist_exists {
            tracing::warn!(
                "Frontend dist not found at {:?}. Dashboard UI will not be available.",
                self.dist_dir
            );
            return axum::Router::new().fallback(|| async {
                (StatusCode::OK, FALLBACK_HTML)
            });
        }

        let serve_dir = ServeDir::new(&self.dist_dir)
            .append_index_html_on_directories(true)
            .precompressed_gzip()
            .precompressed_br();

        axum::Router::new()
            .fallback_service(serve_dir)
    }
}
