use std::sync::Arc;
use reqwest::{Client, header};
use tokio::sync::Semaphore;
use tokio::fs::{File, OpenOptions};
use tokio::io::{AsyncWriteExt, AsyncSeekExt, SeekFrom};
use anyhow::Result;
use futures_util::StreamExt;
use crate::buffer::BufferPool;
use crate::error::DownloadResult;
use crate::strategies::StrategyExecutor;
use crate::config::Strategy;

pub struct ParallelStrategy {
    client: Client,
    semaphore: Arc<Semaphore>,
    buffer_pool: Arc<BufferPool>,
}

impl ParallelStrategy {
    pub fn new(client: Client, semaphore: Arc<Semaphore>, buffer_pool: Arc<BufferPool>) -> Self {
        Self { client, semaphore, buffer_pool }
    }
    
    async fn create_temp_file(&self, size: u64) -> Result<(File, std::path::PathBuf)> {
        let temp_name = format!("leech_{}.tmp", rand::random::<u32>());
        let sys_temp = std::env::temp_dir().join(&temp_name);
        
        let path = match File::create(&sys_temp).await {
            Ok(_) => sys_temp,
            Err(_) => {
                let local_dir = std::env::current_dir()?.join("leech_temp");
                tokio::fs::create_dir_all(&local_dir).await?;
                local_dir.join(&temp_name)
            }
        };

        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(&path)
            .await?;
        
        file.set_len(size).await?;
        Ok((file, path))
    }
}

#[async_trait::async_trait]
impl StrategyExecutor for ParallelStrategy {
    async fn execute(&self, url: &str) -> Result<DownloadResult> {
        let start_time = std::time::Instant::now();
        
        // Use HEAD to get size and check range support
        let head_resp = self.client.head(url).send().await?;
        if !head_resp.status().is_success() {
            return Err(anyhow::anyhow!("HEAD request failed: {}", head_resp.status()));
        }

        let total_size = head_resp.headers()
            .get(header::CONTENT_LENGTH)
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.parse::<u64>().ok())
            .ok_or_else(|| anyhow::anyhow!("Content-Length missing"))?;
            
        let accept_ranges = head_resp.headers()
            .get(header::ACCEPT_RANGES)
            .and_then(|v| v.to_str().ok())
            .map(|v| v == "bytes")
            .unwrap_or(false);

        if !accept_ranges {
            return Err(anyhow::anyhow!("Server does not support range requests"));
        }

        let (_initial_file, temp_path) = self.create_temp_file(total_size).await?;
        drop(_initial_file);
        
        // Dynamic chunk count based on size
        let chunk_count = if total_size < 10 * 1024 * 1024 {
            2
        } else if total_size < 100 * 1024 * 1024 {
            4
        } else {
            8
        };
        
        let chunk_size = (total_size + chunk_count - 1) / chunk_count;
        let mut handles = Vec::new();
        
        for i in 0..chunk_count {
            let start = i * chunk_size;
            if start >= total_size { break; }
            let end = ((i + 1) * chunk_size).min(total_size) - 1;
            
            let permit = self.semaphore.clone().acquire_owned().await?;
            let url = url.to_string();
            let client = self.client.clone();
            let path = temp_path.clone();
            let pool = self.buffer_pool.clone();
            
            handles.push(tokio::spawn(async move {
                let _p = permit;
                let mut file = OpenOptions::new().write(true).open(&path).await?;
                file.seek(SeekFrom::Start(start)).await?;
                
                let res = client.get(&url)
                    .header(header::RANGE, format!("bytes={}-{}", start, end))
                    .send()
                    .await?;
                
                if !res.status().is_success() && res.status() != reqwest::StatusCode::PARTIAL_CONTENT {
                    return Err(anyhow::anyhow!("Chunk download failed: {}", res.status()));
                }

                let mut stream = res.bytes_stream();
                let buf = pool.acquire(64 * 1024);
                
                while let Some(chunk) = stream.next().await {
                    let c = chunk?;
                    file.write_all(&c).await?;
                }
                
                pool.release(buf);
                file.flush().await?;
                Ok::<(), anyhow::Error>(())
            }));
        }

        for h in handles {
            h.await??;
        }
        
        Ok(DownloadResult {
            data: Vec::new(),
            path: Some(temp_path),
            size: total_size,
            duration: start_time.elapsed(),
            strategy: "parallel".to_string(),
            strategy_enum: Strategy::ParallelChunks
        })
    }
}
