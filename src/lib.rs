pub mod buffer;
pub mod config;
pub mod engine;
pub mod extractor;
pub mod error;
pub mod metrics;
pub mod strategies;
pub mod tcp_optimizer;

pub use crate::engine::LeechEngine;
pub use crate::config::{DownloadConfig, Strategy};
pub use crate::error::{DownloadError, DownloadResult};
pub use crate::metrics::DownloadMetrics;
pub use bytes::Bytes;
pub use url::Url;

pub async fn download(url: &str) -> Result<bytes::Bytes, DownloadError> {
    let engine = LeechEngine::new();
    engine.download_simple(url).await
}