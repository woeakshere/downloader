use crate::error::DownloadError;
use regex::Regex;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;

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
        
        // 1. Direct Link Check
        if self.is_direct_link(&lower) {
            return Ok(ExtractedMedia {
                url: input_url.to_string(),
                title: None,
                is_direct: true,
            });
        }

        // 2. YouTube Special Handling (Native Rust)
        if lower.contains("youtube.com") || lower.contains("youtu.be") {
            return self.extract_youtube(input_url).await;
        }

        // 3. Rule-based Extraction (Instagram, etc.)
        if let Some(rules) = &self.rules {
            for category in &rules.categories {
                for platform in &category.platforms {
                    if platform.domains.iter().any(|d| lower.contains(d)) {
                        tracing::info!("🔍 Using extraction rules for platform: {}", platform.name);
                        match self.extract_with_rules(input_url, platform).await {
                            Ok(media) => return Ok(media),
                            Err(e) => tracing::warn!("⚠️ Rule extraction failed for {}: {:?}", platform.name, e),
                        }
                    }
                }
            }
        }

        // 4. Hardcoded Fallbacks
        if lower.contains("mediafire.com") {
            return self.extract_mediafire(input_url).await;
        }

        // 5. Generic Fallback
        self.search_for_media_links(input_url).await
    }

    /// Native YouTube Extractor (No yt-dlp, Low Memory)
    async fn extract_youtube(&self, url: &str) -> Result<ExtractedMedia, DownloadError> {
        tracing::info!("📺 Starting Native YouTube Extraction...");
        
        let resp = self.client.get(url)
            .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")
            .send().await.map_err(DownloadError::NetworkError)?;
        
        let html = resp.text().await.map_err(|e| DownloadError::Anyhow(e.to_string()))?;

        // Regex to find the JSON blob
        let re = Regex::new(r"var ytInitialPlayerResponse\s*=\s*(\{.+?\});").unwrap();
        
        let json_str = re.captures(&html)
            .and_then(|caps| caps.get(1))
            .map(|m| m.as_str())
            .ok_or_else(|| DownloadError::Anyhow("Could not find ytInitialPlayerResponse JSON".to_string()))?;

        let json: Value = serde_json::from_str(json_str)
            .map_err(|e| DownloadError::Anyhow(format!("JSON Parse Error: {}", e)))?;

        // Extract Title
        let title = json["videoDetails"]["title"].as_str().map(|s| s.to_string());

        // Extract Streaming URL
        // We look in streamingData -> formats (standard) or adaptiveFormats (higher quality/DASH)
        if let Some(formats) = json["streamingData"]["formats"].as_array() {
            for format in formats {
                // Check if 'url' exists (unprotected video)
                if let Some(url) = format["url"].as_str() {
                    tracing::info!("✅ Found unprotected YouTube URL");
                    return Ok(ExtractedMedia {
                        url: url.to_string(),
                        title,
                        is_direct: true,
                    });
                }
                // If 'signatureCipher' exists, the video is protected.
                if format.get("signatureCipher").is_some() {
                    tracing::warn!("🔒 Video is protected by Signature Cipher. This lightweight extractor supports only public videos.");
                    return Err(DownloadError::Anyhow("Encrypted signature not supported in lightweight mode".to_string()));
                }
            }
        }

        Err(DownloadError::Anyhow("No valid streaming URL found in JSON".to_string()))
    }

    async fn extract_with_rules(&self, url: &str, platform: &PlatformConfig) -> Result<ExtractedMedia, DownloadError> {
        let request = self.client.get(url)
            .header("User-Agent", platform.ua.as_deref().unwrap_or("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36"));

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

                    return Ok(ExtractedMedia {
                        url: direct_url,
                        title: None,
                        is_direct: false,
                    });
                }
            }
        }
        Err(DownloadError::Anyhow(format!("Rule extraction failed for {}", platform.name)))
    }

    fn is_direct_link(&self, url: &str) -> bool {
        let extensions = [
            ".mp4", ".m3u8", ".zip", ".pdf", ".exe", ".dmg", ".mkv", ".avi", ".mp3", 
            ".rar", ".7z", ".tar.gz", ".iso", ".mov", ".wav", ".flac", ".epub"
        ];
        extensions.iter().any(|ext| url.ends_with(ext) || url.contains(&format!("{ext}?")))
    }

    async fn extract_mediafire(&self, url: &str) -> Result<ExtractedMedia, DownloadError> {
        // ... (Keep your existing Mediafire logic here)
        Err(DownloadError::Anyhow("Mediafire extraction placeholder".to_string()))
    }

    async fn search_for_media_links(&self, url: &str) -> Result<ExtractedMedia, DownloadError> {
        let resp = self.client.get(url)
            .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")
            .send().await.map_err(DownloadError::NetworkError)?;
        let html = resp.text().await.map_err(|e| DownloadError::Anyhow(e.to_string()))?;

        let media_regex = Regex::new(r#"(https?://[^\s"'<>]+?\.(?:mp4|mkv|mp3|zip|rar)[^\s"'<>]*)"#).unwrap();
        
        if let Some(caps) = media_regex.captures(&html) {
            let direct_url = caps.get(1).unwrap().as_str().to_string();
            return Ok(ExtractedMedia {
                url: direct_url,
                title: None,
                is_direct: false,
            });
        }
        Err(DownloadError::Anyhow("Could not find any media links on the page".to_string()))
    }
}