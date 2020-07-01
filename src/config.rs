use anyhow::Result;
use serde::Deserialize;
use std::path::{Path, PathBuf};
use tokio::fs;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Config {
    pub port: u16,
    pub database_url: String,
    pub user_agent: Option<String>,
    pub log_level: Option<log::Level>,
    pub token: TokenConfig,
    pub tls: Option<TlsConfig>,
    pub hash: HashConfig,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TokenConfig {
    pub key: String,
    pub duration: i64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TlsConfig {
    pub cert: PathBuf,
    pub key: PathBuf,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HashConfig {
    pub hash_len: Option<u32>,
    pub salt_len: Option<usize>,
    pub lanes: Option<u32>,
    pub mem_cost: Option<u32>,
    pub time_cost: Option<u32>,
    pub secret: Option<String>,
}

pub async fn read<P: AsRef<Path>>(path: P) -> Result<Config> {
    let contents = fs::read_to_string(path).await?;
    Ok(serde_json::from_str(&contents)?)
}
