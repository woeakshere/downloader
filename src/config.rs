use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum Strategy {
    /// Small files (< 5MB) kept in memory.
    MemoryBuffered,
    /// Medium/Linear files streamed to disk.
    StreamToDisk,
    /// Large files (> 10MB) downloaded in parallel chunks.
    ParallelChunks,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DownloadConfig {
    pub max_concurrent_chunks: usize,
    pub buffer_pool_size: usize,
    pub chunk_size: usize,
    pub timeout: Duration,
    pub pool_idle_timeout: Option<Duration>,
    pub max_retries: u32,
    pub user_agent: String,
    pub debug: bool,
}

impl Default for DownloadConfig {
    fn default() -> Self {
        Self {
            max_concurrent_chunks: 8,
            buffer_pool_size: 64,
            chunk_size: 128 * 1024, // 128KB chunks
            timeout: Duration::from_secs(30),
            pool_idle_timeout: Some(Duration::from_secs(90)),
            max_retries: 3,
            user_agent: "LeechEngine/1.0.0 (Faster, Less-Memory Downloader)".to_string(),
            debug: false,
        }
    }
}
