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
    // UPDATED: Implementation accepts referer
    async fn execute(&self, url: &str, referer: Option<&str>) -> Result<DownloadResult> {
        let start_time = std::time::Instant::now();
        let ua = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36";
        
        let ref_header = referer.unwrap_or(url);

        // Use HEAD to get size and check range support
        let head_resp = self.client.head(url)
            .header("User-Agent", ua)
            .header("Referer", ref_header) // UPDATED
            .send().await?;
            
        if !head_resp.status().is_success() {
            // Fallback to GET if HEAD is not allowed
            let get_resp = self.client.get(url)
                .header("User-Agent", ua)
                .header("Referer", ref_header) // UPDATED
                .header("Range", "bytes=0-0")
                .send().await?;
            
            if !get_resp.status().is_success() && get_resp.status() != reqwest::StatusCode::PARTIAL_CONTENT {
                return Err(anyhow::anyhow!("Initial request failed: {}", get_resp.status()));
            }
            
            let total_size = get_resp.headers()
                .get("Content-Range")
                .and_then(|v| v.to_str().ok())
                .and_then(|s| s.split('/').last())
                .and_then(|s| s.parse::<u64>().ok())
                .or_else(|| {
                    get_resp.headers()
                        .get(header::CONTENT_LENGTH)
                        .and_then(|v| v.to_str().ok())
                        .and_then(|s| s.parse::<u64>().ok())
                })
                .ok_or_else(|| anyhow::anyhow!("Content-Length missing"))?;
            
            self.run_parallel(url, total_size, start_time, ref_header).await
        } else {
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
            
            self.run_parallel(url, total_size, start_time, ref_header).await
        }
    }
}

impl ParallelStrategy {
    // UPDATED: Helper now accepts ref_header string
    async fn run_parallel(&self, url: &str, total_size: u64, start_time: std::time::Instant, ref_header: &str) -> Result<DownloadResult> {
        let (_initial_file, temp_path) = self.create_temp_file(total_size).await?;
        drop(_initial_file);
        
        let chunk_count = if total_size < 10 * 1024 * 1024 {
            2
        } else if total_size < 100 * 1024 * 1024 {
            4
        } else {
            8
        };
        
        let chunk_size = (total_size + chunk_count - 1) / chunk_count;
        let mut handles = Vec::new();
        let ua = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36";
        
        // Clone referer for use in loop
        let referer_str = ref_header.to_string();

        for i in 0..chunk_count {
            let start = i * chunk_size;
            if start >= total_size { break; }
            let end = ((i + 1) * chunk_size).min(total_size) - 1;
            
            let permit = self.semaphore.clone().acquire_owned().await?;
            let url = url.to_string();
            let client = self.client.clone();
            let path = temp_path.clone();
            let pool = self.buffer_pool.clone();
            let referer_val = referer_str.clone(); // Clone for closure
            
            handles.push(tokio::spawn(async move {
                let _p = permit;
                let mut file = OpenOptions::new().write(true).open(&path).await?;
                file.seek(SeekFrom::Start(start)).await?;
                
                let res = client.get(&url)
                    .header(header::RANGE, format!("bytes={}-{}", start, end))
                    .header("User-Agent", ua)
                    .header("Referer", referer_val) // UPDATED: Use referer
                    .send()
                    .await?;
                
                if !res.status().is_success() && res.status() != reqwest::StatusCode::PARTIAL_CONTENT {
                    return Err(anyhow::anyhow!("Chunk download failed: {}", res.status()));
                }

                let mut stream = res.bytes_stream();
                while let Some(chunk) = stream.next().await {
                    let c = chunk?;
                    file.write_all(&c).await?;
                }
                
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
