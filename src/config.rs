cat > src/config.rs <<'EOF'
use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Strategy {
    MemoryBuffered,
    StreamToDisk,
    ParallelChunks,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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
            chunk_size: 128 * 1024,
            timeout: Duration::from_secs(30),
            pool_idle_timeout: Some(Duration::from_secs(90)),
            max_retries: 3,
            user_agent: "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36".to_string(),
            debug: false,
        }
    }
}
EOF
