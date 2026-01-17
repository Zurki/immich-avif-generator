use serde::Deserialize;
use std::path::PathBuf;

#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    pub immich: ImmichConfig,
    pub storage: StorageConfig,
    pub server: ServerConfig,
    #[serde(default)]
    pub sync: SyncConfig,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ImmichConfig {
    pub url: String,
    #[serde(flatten)]
    pub auth: AuthConfig,
    pub albums: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "auth_type")]
pub enum AuthConfig {
    #[serde(rename = "api_key")]
    ApiKey { api_key: String },
    #[serde(rename = "oauth")]
    OAuth {
        client_id: String,
        client_secret: String,
        token_url: String,
        auth_url: String,
        redirect_uri: String,
    },
}

#[derive(Debug, Clone, Deserialize)]
pub struct StorageConfig {
    pub base_path: PathBuf,
    #[serde(default = "default_original_dir")]
    pub original_dir: String,
    #[serde(default = "default_avif_dir")]
    pub avif_dir: String,
    #[serde(default = "default_db_name")]
    pub db_name: String,
}

fn default_original_dir() -> String {
    "original".to_string()
}

fn default_avif_dir() -> String {
    "avif".to_string()
}

fn default_db_name() -> String {
    "db.sqlite".to_string()
}

#[derive(Debug, Clone, Deserialize)]
pub struct ServerConfig {
    #[serde(default = "default_host")]
    pub host: String,
    #[serde(default = "default_port")]
    pub port: u16,
}

fn default_host() -> String {
    "127.0.0.1".to_string()
}

fn default_port() -> u16 {
    3000
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct SyncConfig {
    #[serde(default)]
    pub delete_removed: bool,
    #[serde(default = "default_parallel_downloads")]
    pub parallel_downloads: usize,
    #[serde(default = "default_parallel_conversions")]
    pub parallel_conversions: usize,
}

fn default_parallel_downloads() -> usize {
    4
}

fn default_parallel_conversions() -> usize {
    2
}

impl Config {
    pub fn load(path: &str) -> anyhow::Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let config: Config = toml::from_str(&content)?;
        Ok(config)
    }

    pub fn original_path(&self) -> PathBuf {
        self.storage.base_path.join(&self.storage.original_dir)
    }

    pub fn avif_path(&self) -> PathBuf {
        self.storage.base_path.join(&self.storage.avif_dir)
    }

    pub fn db_path(&self) -> PathBuf {
        self.storage.base_path.join(&self.storage.db_name)
    }
}
