use reqwest::Client;
use anyhow::Result;
use std::sync::Arc;
use tokio::fs::File;
use tokio::io::AsyncWriteExt;
use futures_util::StreamExt;
use crate::buffer::BufferPool;
use crate::error::DownloadResult;
use crate::strategies::StrategyExecutor;
use crate::config::Strategy;

pub struct StreamStrategy {
    client: Client,
    buffer_pool: Arc<BufferPool>,
}

impl StreamStrategy {
    pub fn new(client: Client, buffer_pool: Arc<BufferPool>) -> Self {
        Self { client, buffer_pool }
    }
}

#[async_trait::async_trait]
impl StrategyExecutor for StreamStrategy {
    async fn execute(&self, url: &str) -> Result<DownloadResult> {
        let start = std::time::Instant::now();
        let res = self.client.get(url).send().await?;
        
        if !res.status().is_success() {
            return Err(anyhow::anyhow!("HTTP Status: {}", res.status()));
        }

        let temp_name = format!("stream_{}.tmp", rand::random::<u32>());
        let sys_temp = std::env::temp_dir().join(&temp_name);
        
        let (mut file, path) = match File::create(&sys_temp).await {
            Ok(f) => (f, sys_temp),
            Err(_) => {
                let local_dir = std::env::current_dir()?.join("leech_temp");
                tokio::fs::create_dir_all(&local_dir).await?;
                let path = local_dir.join(&temp_name);
                (File::create(&path).await?, path)
            }
        };

        let mut stream = res.bytes_stream();
        let mut size = 0;
        let buf = self.buffer_pool.acquire(64 * 1024);
        // We don't actually use the buffer pool's BytesMut here for write_all because
        // reqwest returns its own Bytes, but we acquired it to "reserve" memory if needed
        // and to keep the interface consistent. 
        // A better way would be to copy stream chunks into the buffer then write,
        // but write_all(&bytes) is already efficient.
        
        while let Some(chunk) = stream.next().await {
            let c = chunk?;
            file.write_all(&c).await?;
            size += c.len() as u64;
        }
        
        self.buffer_pool.release(buf);
        file.flush().await?;
        
        Ok(DownloadResult { 
            data: vec![], 
            path: Some(path), 
            size, 
            duration: start.elapsed(), 
            strategy: "stream".to_string(),
            strategy_enum: Strategy::StreamToDisk
        })
    }
}
