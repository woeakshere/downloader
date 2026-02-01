use lol_html::{element, HtmlRewriter, Settings};
use std::cell::RefCell;
use std::rc::Rc;
use crate::error::DownloadError;

pub struct StaticAnalyzer {
    // We can add more configuration here if needed
}

impl StaticAnalyzer {
    pub fn new() -> Self {
        Self {}
    }

    pub fn extract_links(&self, html: &str, selectors: &[&str]) -> Result<Vec<String>, DownloadError> {
        let links = Rc::new(RefCell::new(Vec::new()));
        
        let mut element_content_handlers = Vec::new();
        
        for selector in selectors {
            let links_clone = Rc::clone(&links);
            element_content_handlers.push(
                element!(selector, move |el| {
                    if let Some(href) = el.get_attribute("href") {
                        links_clone.borrow_mut().push(href);
                    } else if let Some(src) = el.get_attribute("src") {
                        links_clone.borrow_mut().push(src);
                    } else if let Some(content) = el.get_attribute("content") {
                        links_clone.borrow_mut().push(content);
                    }
                    Ok(())
                })
            );
        }

        let mut rewriter = HtmlRewriter::new(
            Settings {
                element_content_handlers,
                ..Settings::default()
            },
            |_: &[u8]| {}
        );

        rewriter.write(html.as_bytes()).map_err(|e| DownloadError::Anyhow(e.to_string()))?;
        rewriter.end().map_err(|e| DownloadError::Anyhow(e.to_string()))?;

        let result = links.borrow().clone();
        Ok(result)
    }

    /// A more advanced extraction that can handle simple JS-like patterns in HTML
    pub fn extract_with_js_patterns(&self, html: &str, patterns: &[&str]) -> Vec<String> {
        let mut results = Vec::new();
        for pattern in patterns {
            if let Ok(re) = regex::Regex::new(pattern) {
                for caps in re.captures_iter(html) {
                    if let Some(m) = caps.get(1) {
                        results.push(m.as_str().to_string());
                    } else {
                        results.push(caps.get(0).unwrap().as_str().to_string());
                    }
                }
            }
        }
        results
    }

    /// Lightweight JS Scraper: Extracts and "evaluates" simple JS assignments and variables
    pub fn scrape_js_vars(&self, html: &str) -> std::collections::HashMap<String, String> {
        let mut vars = std::collections::HashMap::new();
        
        // Match common JS variable assignments: var/let/const name = "value";
        let re = regex::Regex::new(r#"(?:var|let|const)\s+([a-zA-Z0-9_$]+)\s*=\s*["']([^"']+)["']"#).unwrap();
        for caps in re.captures_iter(html) {
            vars.insert(caps[1].to_string(), caps[2].to_string());
        }

        // Match object properties: "key": "value" or key: "value"
        let re_obj = regex::Regex::new(r#"["']?([a-zA-Z0-9_$]+)["']?\s*:\s*["']([^"']+)["']"#).unwrap();
        for caps in re_obj.captures_iter(html) {
            vars.insert(caps[1].to_string(), caps[2].to_string());
        }

        vars
    }

    /// Attempts to reconstruct a URL from JS fragments
    pub fn reconstruct_url_from_js(&self, html: &str) -> Option<String> {
        let vars = self.scrape_js_vars(html);
        
        // Look for common URL parts
        let base = vars.get("base_url").or(vars.get("baseUrl")).or(vars.get("host"));
        let path = vars.get("file_path").or(vars.get("path")).or(vars.get("slug"));
        let token = vars.get("token").or(vars.get("k")).or(vars.get("auth"));

        if let (Some(b), Some(p)) = (base, path) {
            let mut url = format!("{}{}", b, p);
            if let Some(t) = token {
                if !url.contains('?') {
                    url.push_str(&format!("?token={}", t));
                } else {
                    url.push_str(&format!("&token={}", t));
                }
            }
            return Some(url);
        }
        None
    }
}
