use astrbot_core::lifecycle::CoreLifecycle;
use tracing::info;

use crate::app::create_router;

pub async fn start_server(core: &CoreLifecycle) -> Result<(), Box<dyn std::error::Error>> {
    let addr = {
        let config = core.config.read().await;
        format!("{}:{}", config.dashboard.host, config.dashboard.port)
    };

    let app = create_router(core);

    let listener = tokio::net::TcpListener::bind(&addr).await?;
    info!("Dashboard listening on http://{addr}");

    axum::serve(listener, app).await?;
    Ok(())
}
