use axum::{
    extract::Query,
    response::Json,
    routing::get,
    Router,
};
use leech_core::{LeechEngine, Strategy};
use serde::{Deserialize, Serialize};
use std::time::Instant;
use std::net::SocketAddr;

#[derive(Deserialize)]
struct DownloadParams {
    url: String,
    strategy: Option<String>,
}

#[derive(Serialize)]
struct SpeedResult {
    status: String,
    title: Option<String>,
    file_size_mb: f64,
    speed_mbps: f64,
    duration_secs: f64,
    strategy: String,
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    let app = Router::new()
        .route("/", get(root))
        .route("/test", get(trigger_download));

    let port = std::env::var("PORT").unwrap_or_else(|_| "8000".to_string());
    let addr: SocketAddr = format!("0.0.0.0:{}", port).parse().unwrap();

    tracing::info!("🚀 Leech Server listening on {}", addr);
    
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

async fn root() -> &'static str {
    "Leech Engine Online.\nSupported: Instagram, Mega, Mediafire, YouTube, etc.\nEndpoints: /test?url=...&strategy=[memory|stream|parallel]"
}

async fn trigger_download(Query(params): Query<DownloadParams>) -> Json<SpeedResult> {
    tracing::info!("📥 Incoming Request: {}", params.url);
    
    let engine = LeechEngine::new();
    let start = Instant::now();

    let preferred = params.strategy.and_then(|s| match s.to_lowercase().as_str() {
        "memory" => Some(Strategy::MemoryBuffered),
        "stream" => Some(Strategy::StreamToDisk),
        "parallel" => Some(Strategy::ParallelChunks),
        _ => None,
    });

    // The engine now handles extraction internally via LinkExtractor
    match engine.download(&params.url, preferred).await {
        Ok(res) => {
            let duration = start.elapsed().as_secs_f64();
            let size_mb = res.size as f64 / 1024.0 / 1024.0;
            let speed = if duration > 0.0 { size_mb / duration } else { 0.0 };

            // Cleanup temp file if it exists
            if let Some(path) = res.path {
                let _ = tokio::fs::remove_file(path).await;
            }

            Json(SpeedResult {
                status: "Success".to_string(),
                title: None, // We could pass this from engine if we update DownloadResult
                file_size_mb: (size_mb * 100.0).round() / 100.0,
                speed_mbps: (speed * 100.0).round() / 100.0,
                duration_secs: (duration * 100.0).round() / 100.0,
                strategy: res.strategy,
            })
        }
        Err(e) => {
            tracing::error!("❌ Download Failed: {:?}", e);
            Json(SpeedResult {
                status: format!("Error: {}", e),
                title: None,
                file_size_mb: 0.0,
                speed_mbps: 0.0,
                duration_secs: 0.0,
                strategy: "Failed".to_string(),
            })
        }
    }
}
