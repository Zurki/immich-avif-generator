use crate::config::Config;
use crate::db::models::SyncedImage;
use crate::immich::{AssetResponse, ImmichClient};
use anyhow::Result;
use futures::stream::{self, StreamExt};
use sqlx::SqlitePool;
use std::collections::HashSet;
use tracing::{debug, info, warn};

pub struct SyncService {
    client: ImmichClient,
    pool: SqlitePool,
    config: Config,
}

#[derive(Debug)]
pub struct SyncResult {
    pub downloaded: usize,
    pub skipped: usize,
    pub failed: usize,
    pub removed: usize,
}

impl SyncService {
    pub fn new(client: ImmichClient, pool: SqlitePool, config: Config) -> Self {
        Self {
            client,
            pool,
            config,
        }
    }

    pub async fn sync_all(&self) -> Result<SyncResult> {
        let mut total_result = SyncResult {
            downloaded: 0,
            skipped: 0,
            failed: 0,
            removed: 0,
        };

        let albums = self.client.get_albums().await?;
        info!("Found {} accessible albums", albums.len());

        for album in &albums {
            info!("Syncing album: {} ({})", album.album_name, album.id);
            match self.sync_album(&album.id).await {
                Ok(result) => {
                    total_result.downloaded += result.downloaded;
                    total_result.skipped += result.skipped;
                    total_result.failed += result.failed;
                    total_result.removed += result.removed;
                }
                Err(e) => {
                    warn!("Failed to sync album {} ({}): {}", album.album_name, album.id, e);
                    total_result.failed += 1;
                }
            }
        }

        info!(
            "Sync complete: {} downloaded, {} skipped, {} failed, {} removed",
            total_result.downloaded,
            total_result.skipped,
            total_result.failed,
            total_result.removed
        );

        Ok(total_result)
    }

    pub async fn sync_album(&self, album_id: &str) -> Result<SyncResult> {
        let album = self.client.get_album(album_id).await?;

        crate::db::models::Album::upsert(
            &self.pool,
            &album.id,
            &album.album_name,
            album.asset_count,
        )
        .await?;

        let existing_ids: HashSet<String> = SyncedImage::get_all_synced_ids(&self.pool)
            .await?
            .into_iter()
            .collect();

        let images: Vec<&AssetResponse> = album.assets.iter().filter(|a| a.is_image()).collect();

        let remote_ids: HashSet<String> = images.iter().map(|a| a.id.clone()).collect();

        let mut result = SyncResult {
            downloaded: 0,
            skipped: 0,
            failed: 0,
            removed: 0,
        };

        let assets_to_sync: Vec<&AssetResponse> = images
            .into_iter()
            .filter(|asset| {
                !existing_ids.contains(&asset.id)
                    || self.needs_update(&asset.id, &asset.checksum, &existing_ids)
            })
            .collect();

        info!(
            "Album '{}': {} images to sync out of {}",
            album.album_name,
            assets_to_sync.len(),
            album.asset_count
        );

        let results: Vec<_> = stream::iter(assets_to_sync)
            .map(|asset| async move { self.download_asset(album_id, asset).await })
            .buffer_unordered(self.config.sync.parallel_downloads)
            .collect()
            .await;

        for download_result in results {
            match download_result {
                Ok(true) => result.downloaded += 1,
                Ok(false) => result.skipped += 1,
                Err(e) => {
                    warn!("Download failed: {}", e);
                    result.failed += 1;
                }
            }
        }

        if self.config.sync.delete_removed {
            for id in existing_ids.difference(&remote_ids) {
                if let Ok(Some(image)) = SyncedImage::get_by_id(&self.pool, id).await {
                    if image.album_id == album_id {
                        debug!("Removing deleted image: {}", id);
                        if let Some(path) = &image.original_path {
                            let _ = tokio::fs::remove_file(path).await;
                        }
                        if let Some(path) = &image.avif_path {
                            let _ = tokio::fs::remove_file(path).await;
                        }
                        SyncedImage::delete_by_id(&self.pool, id).await?;
                        result.removed += 1;
                    }
                }
            }
        }

        Ok(result)
    }

    fn needs_update(&self, _asset_id: &str, _checksum: &str, _existing: &HashSet<String>) -> bool {
        false
    }

    async fn download_asset(&self, album_id: &str, asset: &AssetResponse) -> Result<bool> {
        let original_dir = self.config.original_path();
        let dest_path = original_dir.join(album_id).join(&asset.original_file_name);

        if dest_path.exists() {
            debug!("Skipping existing file: {:?}", dest_path);
            return Ok(false);
        }

        info!("Downloading: {}", asset.original_file_name);
        let size = self.client.download_asset(&asset.id, &dest_path).await?;

        SyncedImage::upsert(
            &self.pool,
            &asset.id,
            album_id,
            &asset.original_file_name,
            Some(&asset.checksum),
            Some(dest_path.to_str().unwrap_or("")),
            Some(size as i64),
        )
        .await?;

        Ok(true)
    }
}
