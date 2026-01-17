use serde::Deserialize;
use std::env;
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

    /// Load configuration from environment variables
    pub fn from_env() -> anyhow::Result<Self> {
        let url = env::var("IMMICH_URL")
            .map_err(|_| anyhow::anyhow!("IMMICH_URL environment variable is required"))?;

        let api_key = env::var("IMMICH_API_KEY")
            .map_err(|_| anyhow::anyhow!("IMMICH_API_KEY environment variable is required"))?;

        let albums_str = env::var("IMMICH_ALBUMS").map_err(|_| {
            anyhow::anyhow!("IMMICH_ALBUMS environment variable is required (comma-separated)")
        })?;

        let albums: Vec<String> = albums_str
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();

        if albums.is_empty() {
            return Err(anyhow::anyhow!(
                "IMMICH_ALBUMS must contain at least one album ID"
            ));
        }

        let base_path = env::var("STORAGE_PATH").unwrap_or_else(|_| "./data".to_string());
        let host = env::var("SERVER_HOST").unwrap_or_else(|_| "0.0.0.0".to_string());
        let port: u16 = env::var("SERVER_PORT")
            .unwrap_or_else(|_| "3000".to_string())
            .parse()
            .unwrap_or(3000);

        let delete_removed: bool = env::var("SYNC_DELETE_REMOVED")
            .unwrap_or_else(|_| "false".to_string())
            .parse()
            .unwrap_or(false);

        let parallel_downloads: usize = env::var("SYNC_PARALLEL_DOWNLOADS")
            .unwrap_or_else(|_| "4".to_string())
            .parse()
            .unwrap_or(4);

        let parallel_conversions: usize = env::var("SYNC_PARALLEL_CONVERSIONS")
            .unwrap_or_else(|_| "2".to_string())
            .parse()
            .unwrap_or(2);

        Ok(Config {
            immich: ImmichConfig {
                url,
                auth: AuthConfig::ApiKey { api_key },
                albums,
            },
            storage: StorageConfig {
                base_path: PathBuf::from(base_path),
                original_dir: env::var("STORAGE_ORIGINAL_DIR")
                    .unwrap_or_else(|_| default_original_dir()),
                avif_dir: env::var("STORAGE_AVIF_DIR").unwrap_or_else(|_| default_avif_dir()),
                db_name: env::var("STORAGE_DB_NAME").unwrap_or_else(|_| default_db_name()),
            },
            server: ServerConfig { host, port },
            sync: SyncConfig {
                delete_removed,
                parallel_downloads,
                parallel_conversions,
            },
        })
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
