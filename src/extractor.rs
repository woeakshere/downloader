use crate::error::DownloadError;
use regex::Regex;
use reqwest::Client;
use serde::{Deserialize, Serialize};

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
        
        // 1. Check if it's already a direct link
        if self.is_direct_link(&lower) {
            return Ok(ExtractedMedia {
                url: input_url.to_string(),
                title: None,
                is_direct: true,
            });
        }

        // 2. Try to find a matching platform in our rules
        if let Some(rules) = &self.rules {
            for category in &rules.categories {
                for platform in &category.platforms {
                    if platform.domains.iter().any(|d| lower.contains(d)) {
                        tracing::info!("🔍 Using extraction rules for platform: {}", platform.name);
                        match self.extract_with_rules(input_url, platform).await {
                            Ok(media) => return Ok(media),
                            Err(e) => {
                                tracing::warn!("⚠️ Rule-based extraction failed for {}: {:?}", platform.name, e);
                            }
                        }
                    }
                }
            }
        }

        // 3. Platform-specific hardcoded fallbacks
        if lower.contains("mediafire.com") {
            return self.extract_mediafire(input_url).await;
        }

        // 4. Generic extraction (last resort)
        match self.generic_extract(input_url).await {
            Ok(media) => Ok(media),
            Err(_) => {
                self.search_for_media_links(input_url).await
            }
        }
    }

    async fn search_for_media_links(&self, url: &str) -> Result<ExtractedMedia, DownloadError> {
        let resp = self.client.get(url)
            .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")
            .send().await.map_err(DownloadError::NetworkError)?;
        let html = resp.text().await.map_err(|e| DownloadError::Anyhow(e.to_string()))?;

        // Improved regex to find direct media links in HTML
        let media_regex = Regex::new(r#"(https?://[^\s"'<>]+?\.(?:mp4|mkv|mp3|zip|pdf|exe|dmg|rar|7z|tar\.gz|iso|mov|avi)[^\s"'<>]*)"#).unwrap();
        
        if let Some(caps) = media_regex.captures(&html) {
            let direct_url = caps.get(1).unwrap().as_str().to_string();
            tracing::info!("✅ Found media link via generic search: {}", direct_url);
            return Ok(ExtractedMedia {
                url: direct_url,
                title: None,
                is_direct: false,
            });
        }

        Err(DownloadError::Anyhow("Could not find any media links on the page".to_string()))
    }

    async fn extract_with_rules(&self, url: &str, platform: &PlatformConfig) -> Result<ExtractedMedia, DownloadError> {
        let mut request = self.client.get(url);
        let ua = platform.ua.as_deref().unwrap_or("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36");
        request = request.header("User-Agent", ua);

        let resp = request.send().await.map_err(DownloadError::NetworkError)?;
        let html = resp.text().await.map_err(|e| DownloadError::Anyhow(e.to_string()))?;

        for pattern in &platform.patterns {
            if let Ok(re) = Regex::new(pattern) {
                if let Some(caps) = re.captures(&html) {
                    let mut direct_url = if caps.len() > 1 {
                        caps.get(1).unwrap().as_str().to_string()
                    } else {
                        caps.get(0).unwrap().as_str().to_string()
                    };

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
        let extensions = [
            ".mp4", ".m3u8", ".zip", ".pdf", ".exe", ".dmg", ".mkv", ".avi", ".mp3", 
            ".rar", ".7z", ".tar.gz", ".iso", ".mov", ".wav", ".flac", ".epub"
        ];
        extensions.iter().any(|ext| url.ends_with(ext) || url.contains(&format!("{ext}?")))
    }

    async fn extract_mediafire(&self, url: &str) -> Result<ExtractedMedia, DownloadError> {
        let resp = self.client.get(url)
            .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")
            .send().await.map_err(DownloadError::NetworkError)?;
        let html = resp.text().await.map_err(|e| DownloadError::Anyhow(e.to_string()))?;

        // Mediafire often hides the link in various ways. Let's try multiple patterns.
        let patterns = [
            r#"href="(https?://download[^"]+\.mediafire\.com/[^"]+)""#,
            r#"aria-label="Download file"\s+href="([^"]+)""#,
            r#"onclick="location\.href='(https?://download[^']+)'""#,
            r#"https?://download[0-9]+\.mediafire\.com/[^\s"']+"#
        ];

        for pattern in patterns {
            let re = Regex::new(pattern).unwrap();
            if let Some(caps) = re.captures(&html) {
                let direct_url = if caps.len() > 1 {
                    caps.get(1).unwrap().as_str().to_string()
                } else {
                    caps.get(0).unwrap().as_str().to_string()
                };
                tracing::info!("✅ Extracted Mediafire URL: {}", direct_url);
                return Ok(ExtractedMedia {
                    url: direct_url,
                    title: None,
                    is_direct: false,
                });
            }
        }
        
        Err(DownloadError::Anyhow("Could not find Mediafire download link".to_string()))
    }

    async fn generic_extract(&self, url: &str) -> Result<ExtractedMedia, DownloadError> {
        let resp = self.client.head(url)
            .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")
            .send().await;
            
        if let Ok(head) = resp {
            if let Some(ct) = head.headers().get(reqwest::header::CONTENT_TYPE) {
                let ct_str = ct.to_str().unwrap_or("");
                if ct_str.contains("text/html") {
                    return Err(DownloadError::Anyhow("URL points to HTML, need extraction".to_string()));
                }
                if ct_str.contains("video/") || ct_str.contains("audio/") || ct_str.contains("application/") {
                    return Ok(ExtractedMedia {
                        url: url.to_string(),
                        title: None,
                        is_direct: true,
                    });
                }
            }
        }

        Ok(ExtractedMedia {
            url: url.to_string(),
            title: None,
            is_direct: true,
        })
    }
}
