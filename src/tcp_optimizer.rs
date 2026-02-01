use reqwest::Client;
use std::time::Duration;
use crate::config::DownloadConfig;

pub struct OptimizedClientBuilder {
    config: DownloadConfig,
}

impl OptimizedClientBuilder {
    pub fn new(config: DownloadConfig) -> Self {
        Self { config }
    }

    pub fn build(self) -> Client {
        tracing::debug!("🛠️ Building Optimized Client");

        let mut builder = reqwest::Client::builder()
            .timeout(self.config.timeout)
            .tcp_nodelay(true)
            .pool_idle_timeout(self.config.pool_idle_timeout.unwrap_or(Duration::from_secs(90)))
            .pool_max_idle_per_host(32)
            .user_agent(&self.config.user_agent)
            .tcp_keepalive(Some(Duration::from_secs(60)));

        // Apply bypass logic
        let bypass = crate::bypass::BypassSystem::new();
        builder = bypass.apply_bypass_logic(builder);

        builder.build()
            .expect("Client build failed")
    }
}
