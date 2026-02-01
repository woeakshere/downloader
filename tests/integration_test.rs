use leech_core::{LeechEngine, Strategy};
use std::time::Duration;

#[tokio::test]
async fn test_simple_download() {
    let engine = LeechEngine::new();
    // Use a reliable small file for testing
    let url = "https://raw.githubusercontent.com/tokio-rs/tokio/master/README.md";
    let result = engine.download(url, Some(Strategy::MemoryBuffered)).await;
    assert!(result.is_ok());
    let res = result.unwrap();
    assert!(res.size > 0);
    assert!(!res.data.is_empty());
}
