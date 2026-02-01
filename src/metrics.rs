use std::time::Duration;
use dashmap::DashMap;
use crate::config::Strategy;
use std::sync::atomic::{AtomicU64, Ordering};

pub struct DownloadMetrics {
    total_bytes: AtomicU64,
    total_downloads: AtomicU64,
    strategy_counts: DashMap<Strategy, u64>,
}

impl DownloadMetrics {
    pub fn new() -> Self {
        Self {
            total_bytes: AtomicU64::new(0),
            total_downloads: AtomicU64::new(0),
            strategy_counts: DashMap::new(),
        }
    }

    pub fn record_download(&self, _url: &str, size: u64, _duration: Duration, strategy: &Strategy) {
        self.total_bytes.fetch_add(size, Ordering::Relaxed);
        self.total_downloads.fetch_add(1, Ordering::Relaxed);
        *self.strategy_counts.entry(*strategy).or_insert(0) += 1;
        
        let mb = size as f64 / 1024.0 / 1024.0;
        let secs = _duration.as_secs_f64();
        let speed = if secs > 0.0 { mb / secs } else { 0.0 };
        
        tracing::info!(
            "📊 Download Finished: {:.2} MB in {:.2}s ({:.2} MB/s) using {:?}",
            mb, secs, speed, strategy
        );
    }

    pub fn get_total_bytes(&self) -> u64 {
        self.total_bytes.load(Ordering::Relaxed)
    }

    pub fn get_total_downloads(&self) -> u64 {
        self.total_downloads.load(Ordering::Relaxed)
    }
}
