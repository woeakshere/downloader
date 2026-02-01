pub mod parallel;
pub mod memory;
pub mod stream;
use async_trait::async_trait;
use anyhow::Result;
use crate::error::DownloadResult;

#[async_trait]
pub trait StrategyExecutor: Send + Sync {
    // UPDATED: Added referer argument to the trait definition
    async fn execute(&self, url: &str, referer: Option<&str>) -> Result<DownloadResult>;
}
