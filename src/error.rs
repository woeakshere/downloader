use std::path::PathBuf;
use std::time::Duration;
use thiserror::Error;
use crate::config::Strategy;

#[derive(Error, Debug)]
pub enum DownloadError {
    #[error("Network error: {0}")]
    NetworkError(#[from] reqwest::Error),
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
    #[error("Invalid URL: {0}")]
    InvalidUrl(String),
    #[error("Execution error: {0}")]
    Anyhow(String),
    #[error("Unknown error")]
    Unknown,
}

#[derive(Debug, Clone)]
pub struct DownloadResult {
    pub data: Vec<u8>,
    pub path: Option<PathBuf>,
    pub size: u64,
    pub duration: Duration,
    pub strategy: String,
    pub strategy_enum: Strategy, 
}
