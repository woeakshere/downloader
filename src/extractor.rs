cat > src/extractor.rs <<'EOF'
use crate::error::DownloadError;
use regex::Regex;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ExtractedMedia {
    pub url: String,
    pub title: Option<String>,
    pub is_direct: bool,
}

#[derive(Debug, Deserialize)]
struct ExtractionRules {
    categories: Vec<Category>,
}

#[derive(Debug, Deserialize)]
struct Category {
    platforms: Vec<PlatformConfig>,
}

#[derive(Debug, Deserialize)]
struct PlatformConfig {
    name: String,
    domains: Vec<String>,
    patterns: Vec<String>,
    ua: Option<String>,
}

pub struct LinkExtractor {
    client: Client,
    rules: Arc<Option<ExtractionRules>>,
}

impl LinkExtractor {
    pub fn new(client: Client) -> Self {
        let rules = std::fs::read_to_string("extraction_rules.json")
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok());
        
        if rules.is_some() {
            tracing::info!("🧠 Extraction rules loaded successfully");
        } else {
            tracing::warn!("⚠️ extraction_rules.json not found or invalid");
        }

        Self { 
            client, 
            rules: Arc::new(rules) 
        }
    }

    pub async fn extract(&self, input_url: &str) -> Result<ExtractedMedia, DownloadError> {
        let lower = input_url.to_lowercase();
        
        if self.is_direct_link(&lower) {
            return Ok(ExtractedMedia {
                url: input_url.to_string(),
                title: None,
                is_direct: true,
            });
        }

        if let Some(config) = self.rules.as_ref() {
            for category in &config.categories {
                for platform in &category.platforms {
                    if platform.domains.iter().any(|d| lower.contains(&d.to_lowercase())) {
                        tracing::info!("⚡ Matched Rule: {}", platform.name);
                        return self.extract_with_rule(platform, input_url).await;
                    }
                }
            }
        }

        if lower.contains("mediafire.com") {
            return self.extract_mediafire(input_url).await;
        }

        self.generic_extract(input_url).await
    }

    async fn extract_with_rule(&self, platform: &PlatformConfig, url: &str) -> Result<ExtractedMedia, DownloadError> {
        let mut req = self.client.get(url);
        if let Some(ua) = &platform.ua {
            req = req.header("User-Agent", ua);
        }

        let res = req.send().await.map_err(DownloadError::NetworkError)?;
        let html = res.text().await.map_err(|e| DownloadError::Anyhow(e.to_string()))?;

        for pattern in &platform.patterns {
            if let Ok(re) = Regex::new(pattern) {
                if let Some(caps) = re.captures(&html) {
                    if let Some(match_str) = caps.get(1) {
                        let mut found_url = match_str.as_str().to_string();
                        found_url = found_url.replace(r"\/", "/").replace("\\u0026", "&");
                        return Ok(ExtractedMedia {
                            url: found_url,
                            title: None,
                            is_direct: false, 
                        });
                    }
                }
            }
        }
        Err(DownloadError::Anyhow(format!("No patterns matched for {}", platform.name)))
    }

    fn is_direct_link(&self, url: &str) -> bool {
        let clean = url.split('?').next().unwrap_or(url);
        clean.ends_with(".mp4") || clean.ends_with(".m3u8") || clean.ends_with(".mkv") || clean.ends_with(".zip")
    }

    async fn extract_mediafire(&self, url: &str) -> Result<ExtractedMedia, DownloadError> {
        let resp = self.client.get(url).send().await.map_err(DownloadError::NetworkError)?;
        let html = resp.text().await.map_err(|e| DownloadError::Anyhow(e.to_string()))?;
        let re = Regex::new(r#"href="(https?://download[^"]+)""#).unwrap();
        if let Some(caps) = re.captures(&html) {
            return Ok(ExtractedMedia { url: caps[1].to_string(), title: None, is_direct: false });
        }
        Err(DownloadError::Anyhow("Mediafire link not found".to_string()))
    }

    async fn generic_extract(&self, url: &str) -> Result<ExtractedMedia, DownloadError> {
        Ok(ExtractedMedia { url: url.to_string(), title: None, is_direct: true })
    }
}
EOF
