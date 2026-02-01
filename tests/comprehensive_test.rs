use leech_core::{LeechEngine, Strategy};
use std::path::Path;

#[tokio::test]
async fn test_memory_strategy() {
    let engine = LeechEngine::new();
    let url = "https://raw.githubusercontent.com/tokio-rs/tokio/master/README.md";
    let result = engine.download(url, Some(Strategy::MemoryBuffered)).await.expect("Memory download failed");
    assert_eq!(result.strategy_enum, Strategy::MemoryBuffered);
    assert!(!result.data.is_empty());
    assert!(result.path.is_none());
}

#[tokio::test]
async fn test_stream_strategy() {
    let engine = LeechEngine::new();
    let url = "https://raw.githubusercontent.com/tokio-rs/tokio/master/README.md";
    let result = engine.download(url, Some(Strategy::StreamToDisk)).await.expect("Stream download failed");
    assert_eq!(result.strategy_enum, Strategy::StreamToDisk);
    assert!(result.path.is_some());
    let path = result.path.unwrap();
    assert!(path.exists());
    tokio::fs::remove_file(path).await.unwrap();
}

#[tokio::test]
async fn test_parallel_strategy_fallback() {
    let engine = LeechEngine::new();
    // This file might not support ranges, so it should fallback to stream
    let url = "https://raw.githubusercontent.com/tokio-rs/tokio/master/README.md";
    let result = engine.download(url, Some(Strategy::ParallelChunks)).await.expect("Parallel download (with fallback) failed");
    // Since it's a small file on GitHub raw, it might not support ranges or the engine might decide to fallback
    assert!(result.size > 0);
}

#[tokio::test]
async fn test_download_to_path() {
    let engine = LeechEngine::new();
    let url = "https://raw.githubusercontent.com/tokio-rs/tokio/master/README.md";
    let dest = "test_readme.md";
    let result = engine.download_to_path(url, dest, None).await.expect("Download to path failed");
    assert!(Path::new(dest).exists());
    assert!(result.size > 0);
    tokio::fs::remove_file(dest).await.unwrap();
}
