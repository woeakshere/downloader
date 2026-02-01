use crate::error::DownloadError;
use regex::Regex;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use url::Url;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ExtractedMedia {
    pub url: String,
    pub title: Option<String>,
    pub is_direct: bool,
}

#[derive(Debug, Deserialize)]
struct PlatformConfig {
    name: String,
    domains: Vec<String>,
    patterns: Vec<String>,
    ua: Option<String>,
}

#[derive(Debug, Deserialize)]
struct Category {
    platforms: Vec<PlatformConfig>,
}

#[derive(Debug, Deserialize)]
struct ExtractionRules {
    categories: Vec<Category>,
}

pub struct LinkExtractor {
    client: Client,
    rules: Option<ExtractionRules>,
}

impl LinkExtractor {
    pub fn new(client: Client) -> Self {
        let rules = std::fs::read_to_string("extraction_rules.json")
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok());
        
        Self { client, rules }
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

        // Try to find a matching platform in our rules
        if let Some(rules) = &self.rules {
            for category in &rules.categories {
                for platform in &category.platforms {
                    if platform.domains.iter().any(|d| lower.contains(d)) {
                        tracing::info!("🔍 Using extraction rules for platform: {}", platform.name);
                        return self.extract_with_rules(input_url, platform).await;
                    }
                }
            }
        }

        // Fallback to legacy or generic extraction
        if lower.contains("mediafire.com") {
            return self.extract_mediafire(input_url).await;
        }

        self.generic_extract(input_url).await
    }

    async fn extract_with_rules(&self, url: &str, platform: &PlatformConfig) -> Result<ExtractedMedia, DownloadError> {
        let mut request = self.client.get(url);
        if let Some(ua) = &platform.ua {
            request = request.header("User-Agent", ua);
        } else {
            request = request.header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/119.0.0.0 Safari/537.36");
        }

        let resp = request.send().await.map_err(DownloadError::NetworkError)?;
        let html = resp.text().await.map_err(|e| DownloadError::Anyhow(e.to_string()))?;

        for pattern in &platform.patterns {
            if let Ok(re) = Regex::new(pattern) {
                if let Some(caps) = re.captures(&html) {
                    let mut direct_url = caps.get(1).unwrap().as_str().to_string();
                    
                    // Basic unescaping
                    direct_url = direct_url
                        .replace("\\u0026", "&")
                        .replace("&amp;", "&")
                        .replace("\\/", "/");

                    tracing::info!("✅ Extracted URL using pattern: {}", pattern);
                    return Ok(ExtractedMedia {
                        url: direct_url,
                        title: None,
                        is_direct: false,
                    });
                }
            }
        }

        Err(DownloadError::Anyhow(format!("Could not extract media from {} using provided rules", platform.name)))
    }

    fn is_direct_link(&self, url: &str) -> bool {
        url.ends_with(".mp4") || url.ends_with(".m3u8") || url.ends_with(".zip") || 
        url.ends_with(".pdf") || url.ends_with(".exe") || url.ends_with(".dmg") ||
        url.ends_with(".mkv") || url.ends_with(".avi") || url.ends_with(".mp3")
    }

    async fn extract_mediafire(&self, url: &str) -> Result<ExtractedMedia, DownloadError> {
        let resp = self.client.get(url).send().await.map_err(DownloadError::NetworkError)?;
        let html = resp.text().await.map_err(|e| DownloadError::Anyhow(e.to_string()))?;

        let re = Regex::new(r#"href="(https?://download[^"]+)""#).unwrap();
        if let Some(caps) = re.captures(&html) {
            let direct_url = caps.get(1).unwrap().as_str().to_string();
            return Ok(ExtractedMedia {
                url: direct_url,
                title: None,
                is_direct: false,
            });
        }
        Err(DownloadError::Anyhow("Could not find Mediafire download link".to_string()))
    }

    async fn generic_extract(&self, url: &str) -> Result<ExtractedMedia, DownloadError> {
        Ok(ExtractedMedia {
            url: url.to_string(),
            title: None,
            is_direct: true,
        })
    }
}
