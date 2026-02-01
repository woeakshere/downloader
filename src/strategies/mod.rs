pub mod parallel;
pub mod memory;
pub mod stream;
use async_trait::async_trait;
use anyhow::Result;
use crate::error::DownloadResult;

#[async_trait]
pub trait StrategyExecutor: Send + Sync {
    async fn execute(&self, url: &str) -> Result<DownloadResult>;
}