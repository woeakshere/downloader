use std::path::Path;
use std::sync::Arc;
use std::time::Instant;
use std::num::NonZeroUsize;
use parking_lot::RwLock;
use anyhow::Result;
use reqwest::Client;
use tokio::sync::Semaphore;
use dashmap::DashMap;
use url::Url;

use crate::buffer::BufferPool;
use crate::config::{DownloadConfig, Strategy};
use crate::error::{DownloadError, DownloadResult};
use crate::metrics::DownloadMetrics;
use crate::strategies::StrategyExecutor;
use crate::tcp_optimizer::OptimizedClientBuilder;
use crate::extractor::LinkExtractor;
use crate::static_analysis::StaticAnalyzer;
use crate::bypass::BypassSystem;

pub struct LeechEngine {
    client: Client,
    config: Arc<DownloadConfig>,
    buffer_pool: Arc<BufferPool>,
    metrics: Arc<DownloadMetrics>,
    active_downloads: Arc<DashMap<String, Instant>>,
    chunk_semaphore: Arc<Semaphore>,
    strategy_cache: Arc<RwLock<lru::LruCache<String, Strategy>>>,
    extractor: Arc<LinkExtractor>,
    static_analyzer: Arc<StaticAnalyzer>,
    bypass_system: Arc<BypassSystem>,
}

impl LeechEngine {
    pub fn new() -> Self {
        Self::with_config(DownloadConfig::default())
    }
    
    pub fn with_config(config: DownloadConfig) -> Self {
        let client = OptimizedClientBuilder::new(config.clone()).build();
        
        let buffer_pool = Arc::new(BufferPool::new(config.buffer_pool_size, config.chunk_size));
        let chunk_semaphore = Arc::new(Semaphore::new(config.max_concurrent_chunks));
        let cache_cap = NonZeroUsize::new(100).expect("Invalid cache capacity");
        let extractor = Arc::new(LinkExtractor::new(client.clone()));
        let static_analyzer = Arc::new(StaticAnalyzer::new());
        let bypass_system = Arc::new(BypassSystem::new());
        
        Self {
            client,
            config: Arc::new(config),
            buffer_pool,
            metrics: Arc::new(DownloadMetrics::new()),
            active_downloads: Arc::new(DashMap::new()),
            chunk_semaphore,
            strategy_cache: Arc::new(RwLock::new(lru::LruCache::new(cache_cap))),
            extractor,
            static_analyzer,
            bypass_system,
        }
    }
    
    pub async fn download(&self, input_url: &str, preferred_strategy: Option<Strategy>) -> Result<DownloadResult, DownloadError> {
        let start_time = Instant::now();
        
        // Phase 1: Native Extraction (No external binaries)
        let media = self.extractor.extract(input_url).await?;
        let url = &media.url;
        
        let parsed_url = Url::parse(url).map_err(|_| DownloadError::InvalidUrl(url.to_string()))?;
        self.active_downloads.insert(url.to_string(), Instant::now());
        
        let initial_strategy = if let Some(p) = preferred_strategy { 
            p 
        } else { 
            self.analyze_url(&parsed_url).await.unwrap_or(Strategy::StreamToDisk)
        };
        
        // UPDATED: Pass input_url as the referer source
        let result = self.execute_with_fallback(url, Some(input_url), initial_strategy).await;
        
        if let Ok(ref res) = result {
             self.metrics.record_download(url, res.size, start_time.elapsed(), &res.strategy_enum);
             if let Some(d) = parsed_url.domain() {
                self.strategy_cache.write().put(d.to_string(), res.strategy_enum);
             }
        }
        
        self.active_downloads.remove(url);
        result
    }

    // UPDATED: Function signature accepts referer
    async fn execute_with_fallback(&self, url: &str, referer: Option<&str>, mut strategy: Strategy) -> Result<DownloadResult, DownloadError> {
        let mut total_attempts = 0;
        let max_retries = self.config.max_retries;

        loop {
            total_attempts += 1;
            let executor: Box<dyn StrategyExecutor> = match strategy {
                Strategy::MemoryBuffered => Box::new(crate::strategies::memory::MemoryStrategy::new(self.client.clone(), self.buffer_pool.clone())),
                Strategy::StreamToDisk => Box::new(crate::strategies::stream::StreamStrategy::new(self.client.clone(), self.buffer_pool.clone())),
                Strategy::ParallelChunks => Box::new(crate::strategies::parallel::ParallelStrategy::new(self.client.clone(), self.chunk_semaphore.clone(), self.buffer_pool.clone())),
            };

            tracing::info!("🔄 Attempt {} using Strategy: {:?}", total_attempts, strategy);

            // UPDATED: Pass referer to execute
            match executor.execute(url, referer).await {
                Ok(mut res) => {
                    res.strategy_enum = strategy; 
                    return Ok(res);
                },
                Err(e) => {
                    tracing::warn!("⚠️ Strategy {:?} failed: {:?}", strategy, e);
                    
                    if total_attempts > max_retries {
                        return Err(DownloadError::Anyhow(format!("Failed after {} attempts. Last error: {:?}", total_attempts, e)));
                    }

                    if strategy == Strategy::ParallelChunks {
                        tracing::info!("⬇️ Downgrading to StreamToDisk (Safer Mode)...");
                        strategy = Strategy::StreamToDisk;
                    }

                    let backoff = 2u64.pow(total_attempts as u32 - 1) * 500;
                    tokio::time::sleep(std::time::Duration::from_millis(backoff)).await;
                }
            }
        }
    }

    pub async fn download_simple(&self, url: &str) -> Result<bytes::Bytes, DownloadError> {
        let result = self.download(url, Some(Strategy::MemoryBuffered)).await?;
        Ok(bytes::Bytes::from(result.data))
    }
    
    pub async fn download_to_path(&self, url: &str, path: impl AsRef<Path>, strategy: Option<Strategy>) -> Result<DownloadResult, DownloadError> {
        let path = path.as_ref().to_path_buf();
        let result = self.download(url, strategy).await?;
        
        if let Some(temp_path) = &result.path {
            if temp_path != &path {
                tokio::fs::rename(temp_path, &path).await.map_err(DownloadError::IoError)?;
            }
        } else if !result.data.is_empty() {
            tokio::fs::write(&path, &result.data).await.map_err(DownloadError::IoError)?;
        }
        Ok(result)
    }

    async fn analyze_url(&self, url: &Url) -> Result<Strategy, DownloadError> {
        if let Some(d) = url.domain() {
            if let Some(cached) = self.strategy_cache.write().get(d) { return Ok(*cached); }
        }
        
        let headers = self.bypass_system.generate_headers(url.as_str());
        let head_result = self.client.head(url.as_str())
            .headers(headers)
            .send().await;
        
        let (len, ranges, is_html) = match head_result {
            Ok(head) => {
                let len = head.headers().get(reqwest::header::CONTENT_LENGTH)
                    .and_then(|v| v.to_str().ok())
                    .and_then(|s| s.parse::<u64>().ok())
                    .unwrap_or(0);
                    
                let ranges = head.headers().get(reqwest::header::ACCEPT_RANGES)
                    .and_then(|v| v.to_str().ok())
                    .map(|v| v == "bytes")
                    .unwrap_or(false);

                let ct = head.headers().get(reqwest::header::CONTENT_TYPE)
                    .and_then(|v| v.to_str().ok())
                    .unwrap_or("");
                let is_html = ct.contains("text/html");

                (len, ranges, is_html)
            },
            Err(_) => (0, false, false),
        };

        if is_html {
            tracing::warn!("⚠️ URL points to an HTML page, not a direct media file: {}", url);
        }
            
        if len == 0 { 
            Ok(Strategy::StreamToDisk) 
        } else if len < 5 * 1024 * 1024 { 
            Ok(Strategy::MemoryBuffered) 
        } else if ranges && len > 10 * 1024 * 1024 { 
            Ok(Strategy::ParallelChunks) 
        } else { 
            Ok(Strategy::StreamToDisk) 
        }
    }
}
