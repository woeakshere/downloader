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
        // Use Cloudflare DNS-over-HTTPS or similar secure resolver
        let resolver = TokioAsyncResolver::tokio(
            ResolverConfig::cloudflare(),
            ResolverOpts::default(),
        );

        Self {
            user_agents: vec![
                "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36".to_string(),
                "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/119.0.0.0 Safari/537.36".to_string(),
                "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36".to_string(),
                "Mozilla/5.0 (iPhone; CPU iPhone OS 17_1_1 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.1 Mobile/15E148 Safari/604.1".to_string(),
            ],
            languages: vec![
                "en-US,en;q=0.9".to_string(),
                "en-GB,en;q=0.8".to_string(),
                "fr-FR,fr;q=0.7".to_string(),
                "de-DE,de;q=0.6".to_string(),
            ],
            resolver: Arc::new(resolver),
        }
    }

    pub async fn resolve_secure(&self, host: &str) -> Option<std::net::IpAddr> {
        self.resolver.lookup_ip(host).await.ok()?.iter().next()
    }

    pub fn generate_headers(&self, url: &str) -> HeaderMap {
        let mut headers = HeaderMap::new();
        let mut rng = rand::thread_rng();

        let ua = self.user_agents.choose(&mut rng).unwrap();
        let lang = self.languages.choose(&mut rng).unwrap();

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
        headers.insert("Sec-Ch-Ua", HeaderValue::from_static("\"Not_A Brand\";v=\"8\", \"Chromium\";v=\"120\", \"Google Chrome\";v=\"120\""));
        headers.insert("Sec-Ch-Ua-Mobile", HeaderValue::from_static("?0"));
        headers.insert("Sec-Ch-Ua-Platform", HeaderValue::from_static("\"Windows\""));
        headers.insert("Sec-Fetch-Dest", HeaderValue::from_static("document"));
        headers.insert("Sec-Fetch-Mode", HeaderValue::from_static("navigate"));
        headers.insert("Sec-Fetch-Site", HeaderValue::from_static("none"));
        headers.insert("Sec-Fetch-User", HeaderValue::from_static("?1"));
        headers.insert("Upgrade-Insecure-Requests", HeaderValue::from_static("1"));
        
        // Additional bypass headers for ISP/Proxy/Anonymizer detection
        headers.insert("X-Forwarded-For", HeaderValue::from_str(&format!("{}.{}.{}.{}", rng.gen_range(1..255), rng.gen_range(1..255), rng.gen_range(1..255), rng.gen_range(1..255))).unwrap());
        headers.insert("Via", HeaderValue::from_static("1.1 vegur"));
        headers.insert("DNT", HeaderValue::from_static("1")); // Do Not Track
        
        headers
    }

    /// Simulates a bypass for various tests (ISP, DNS, Fingerprinting, etc.)
    pub fn apply_bypass_logic(&self, client_builder: reqwest::ClientBuilder) -> reqwest::ClientBuilder {
        client_builder
            .danger_accept_invalid_certs(true) // Bypass some SSL/TLS blocks
            .no_proxy() // Avoid system proxy leaks if needed, or configure custom ones
            .cookie_store(true) // Handle cookies properly to bypass cookie tests
    }

    pub fn get_random_timezone(&self) -> String {
        let timezones = vec!["UTC", "America/New_York", "Europe/London", "Asia/Tokyo"];
        timezones.choose(&mut rand::thread_rng()).unwrap().to_string()
    }

    pub fn get_spoofed_location(&self) -> HashMap<String, String> {
        let mut loc = HashMap::new();
        loc.insert("lat".to_string(), "40.7128".to_string());
        loc.insert("lon".to_string(), "-74.0060".to_string());
        loc.insert("city".to_string(), "New York".to_string());
        loc
    }
}
