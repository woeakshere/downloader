use reqwest::Client;
use anyhow::Result;
use std::sync::Arc;
use crate::buffer::BufferPool;
use crate::error::DownloadResult;
use crate::strategies::StrategyExecutor;
use crate::config::Strategy;

pub struct MemoryStrategy {
    client: Client,
    buffer_pool: Arc<BufferPool>,
}

impl MemoryStrategy {
    pub fn new(client: Client, buffer_pool: Arc<BufferPool>) -> Self {
        Self { client, buffer_pool }
    }
}

#[async_trait::async_trait]
impl StrategyExecutor for MemoryStrategy {
    async fn execute(&self, url: &str) -> Result<DownloadResult> {
        let start = std::time::Instant::now();
        let res = self.client.get(url).send().await?;
        
        if !res.status().is_success() {
            return Err(anyhow::anyhow!("HTTP Status: {}", res.status()));
        }

        let len = res.content_length().unwrap_or(0) as usize;
        let mut data = self.buffer_pool.acquire(len);
        
        let full = res.bytes().await?;
        data.extend_from_slice(&full);
        
        let data_vec = data.to_vec();
        // Return the buffer to the pool
        self.buffer_pool.release(data);
        
        Ok(DownloadResult { 
            data: data_vec, 
            path: None, 
            size: full.len() as u64, 
            duration: start.elapsed(), 
            strategy: "memory".to_string(),
            strategy_enum: Strategy::MemoryBuffered
        })
    }
}
