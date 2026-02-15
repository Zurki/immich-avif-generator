use super::auth::AuthProvider;
use super::types::{AlbumResponse, AssetResponse, ServerInfo};
use anyhow::{Context, Result};
use reqwest::Client;
use std::path::Path;
use tokio::fs::File;
use tokio::io::AsyncWriteExt;
use tracing::{debug, info};

pub struct ImmichClient {
    client: Client,
    base_url: String,
    auth: AuthProvider,
}

impl ImmichClient {
    pub fn new(base_url: &str, auth: AuthProvider) -> Self {
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(300))
            .build()
            .expect("Failed to create HTTP client");

        let base_url = base_url.trim_end_matches('/').to_string();

        Self {
            client,
            base_url,
            auth,
        }
    }

    async fn request_builder(&self, url: &str) -> Result<reqwest::RequestBuilder> {
        let (header_name, header_value) = self.auth.get_auth_header().await?;
        Ok(self.client.get(url).header(&header_name, &header_value))
    }

    pub async fn ping(&self) -> Result<ServerInfo> {
        let url = format!("{}/api/server/version", self.base_url);
        let response = self
            .request_builder(&url)
            .await?
            .send()
            .await?
            .error_for_status()
            .context("Failed to ping Immich server")?;

        let info: ServerInfo = response.json().await?;
        info!("Connected to Immich server version: {}", info.version);
        Ok(info)
    }

    pub async fn get_albums(&self) -> Result<Vec<AlbumResponse>> {
        let url = format!("{}/api/albums", self.base_url);
        debug!("Fetching owned albums from {}", url);

        let owned_response = self
            .request_builder(&url)
            .await?
            .send()
            .await?
            .error_for_status()
            .context("Failed to fetch owned albums")?;
        let owned: Vec<AlbumResponse> = owned_response.json().await?;
        debug!("Found {} owned albums", owned.len());

        let shared_url = format!("{}/api/albums?shared=true", self.base_url);
        debug!("Fetching shared albums from {}", shared_url);

        let shared_response = self
            .request_builder(&shared_url)
            .await?
            .send()
            .await?
            .error_for_status()
            .context("Failed to fetch shared albums")?;
        let shared: Vec<AlbumResponse> = shared_response.json().await?;
        debug!("Found {} shared albums", shared.len());

        // Merge and deduplicate by album ID
        let mut seen = std::collections::HashSet::new();
        let mut albums = Vec::new();
        for album in owned.into_iter().chain(shared) {
            if seen.insert(album.id.clone()) {
                albums.push(album);
            }
        }

        debug!("Total unique albums: {}", albums.len());
        Ok(albums)
    }

    pub async fn get_album(&self, album_id: &str) -> Result<AlbumResponse> {
        let url = format!("{}/api/albums/{}", self.base_url, album_id);
        debug!("Fetching album {} from {}", album_id, url);

        let response = self
            .request_builder(&url)
            .await?
            .send()
            .await?
            .error_for_status()
            .context(format!("Failed to fetch album {}", album_id))?;

        let album: AlbumResponse = response.json().await?;
        debug!(
            "Album '{}' has {} assets",
            album.album_name, album.asset_count
        );
        Ok(album)
    }

    pub async fn get_asset(&self, asset_id: &str) -> Result<AssetResponse> {
        let url = format!("{}/api/assets/{}", self.base_url, asset_id);
        debug!("Fetching asset metadata for {}", asset_id);

        let response = self
            .request_builder(&url)
            .await?
            .send()
            .await?
            .error_for_status()
            .context(format!("Failed to fetch asset {}", asset_id))?;

        let asset: AssetResponse = response.json().await?;
        Ok(asset)
    }

    pub async fn download_asset(&self, asset_id: &str, dest_path: &Path) -> Result<u64> {
        let url = format!("{}/api/assets/{}/original", self.base_url, asset_id);
        debug!("Downloading asset {} to {:?}", asset_id, dest_path);

        let (header_name, header_value) = self.auth.get_auth_header().await?;

        let response = self
            .client
            .get(&url)
            .header(&header_name, &header_value)
            .send()
            .await?
            .error_for_status()
            .context(format!("Failed to download asset {}", asset_id))?;

        if let Some(parent) = dest_path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        let mut file = File::create(dest_path).await?;
        let bytes = response.bytes().await?;
        let size = bytes.len() as u64;
        file.write_all(&bytes).await?;
        file.flush().await?;

        debug!("Downloaded {} bytes to {:?}", size, dest_path);
        Ok(size)
    }
}
