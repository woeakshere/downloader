use reqwest::header::{HeaderMap, HeaderValue, USER_AGENT, ACCEPT, ACCEPT_LANGUAGE, REFERER};
use rand::{seq::SliceRandom, Rng};
use std::collections::HashMap;
use std::sync::Arc;
use trust_dns_resolver::TokioAsyncResolver;
use trust_dns_resolver::config::{ResolverConfig, ResolverOpts};

pub struct BypassSystem {
    user_agents: Vec<String>,
    languages: Vec<String>,
    resolver: Arc<TokioAsyncResolver>,
}

impl BypassSystem {
    pub fn new() -> Self {
        // Use Cloudflare DNS-over-HTTPS
        let resolver = TokioAsyncResolver::tokio(
            ResolverConfig::cloudflare(),
            ResolverOpts::default(),
        );

        Self {
            // FIX: Use ONLY the User-Agent that matches strategies/stream.rs
            // This prevents "Signature Mismatch" (403) between extraction and download.
            user_agents: vec![
                "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36".to_string(),
            ],
            languages: vec![
                "en-US,en;q=0.9".to_string(),
            ],
            resolver: Arc::new(resolver),
        }
    }

    pub async fn resolve_secure(&self, host: &str) -> Option<std::net::IpAddr> {
        self.resolver.lookup_ip(host).await.ok()?.iter().next()
    }

    pub fn generate_headers(&self, url: &str) -> HeaderMap {
        let mut headers = HeaderMap::new();
        // Removed rng for User-Agent to ensure consistency
        
        let ua = &self.user_agents[0]; // Always use the matching Chrome UA
        let lang = &self.languages[0];

        headers.insert(USER_AGENT, HeaderValue::from_str(ua).unwrap());
        headers.insert(ACCEPT, HeaderValue::from_static("text/html,application/xhtml+xml,application/xml;q=0.9,image/avif,image/webp,image/apng,*/*;q=0.8,application/signed-exchange;v=b3;q=0.7"));
        headers.insert(ACCEPT_LANGUAGE, HeaderValue::from_str(lang).unwrap());
        
        // Referer spoofing
        if let Ok(parsed_url) = url::Url::parse(url) {
            if let Some(host) = parsed_url.host_str() {
                headers.insert(REFERER, HeaderValue::from_str(&format!("https://{}/", host)).unwrap());
            }
        }

        // Anti-fingerprinting & Bypass headers
        // Standard Chrome Headers
        headers.insert("Sec-Ch-Ua", HeaderValue::from_static("\"Not_A Brand\";v=\"8\", \"Chromium\";v=\"120\", \"Google Chrome\";v=\"120\""));
        headers.insert("Sec-Ch-Ua-Mobile", HeaderValue::from_static("?0"));
        headers.insert("Sec-Ch-Ua-Platform", HeaderValue::from_static("\"Windows\""));
        
        // FIX: Removed strict Sec-Fetch-* headers and X-Forwarded-For
        // These often cause 403s on Google Video servers which check IP binding.
        headers.insert("Upgrade-Insecure-Requests", HeaderValue::from_static("1"));
        headers.insert("DNT", HeaderValue::from_static("1")); 
        
        headers
    }

    pub fn apply_bypass_logic(&self, client_builder: reqwest::ClientBuilder) -> reqwest::ClientBuilder {
        client_builder
            .danger_accept_invalid_certs(true)
            .no_proxy()
            .cookie_store(true) // Important: Keeps cookies between Extraction and Download
    }

    pub fn get_random_timezone(&self) -> String {
        "UTC".to_string()
    }

    pub fn get_spoofed_location(&self) -> HashMap<String, String> {
        let mut loc = HashMap::new();
        loc.insert("lat".to_string(), "40.7128".to_string());
        loc.insert("lon".to_string(), "-74.0060".to_string());
        loc.insert("city".to_string(), "New York".to_string());
        loc
    }
}
