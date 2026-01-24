use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct Album {
    pub id: String,
    pub name: String,
    pub asset_count: Option<i64>,
    pub last_sync: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct SyncedImage {
    pub id: String,
    pub album_id: String,
    pub filename: String,
    pub checksum: Option<String>,
    pub original_path: Option<String>,
    pub avif_path: Option<String>,
    pub thumbnail_path: Option<String>,
    pub file_size: Option<i64>,
    pub synced_at: Option<DateTime<Utc>>,
    pub converted_at: Option<DateTime<Utc>>,
}

impl Album {
    pub async fn upsert(
        pool: &sqlx::SqlitePool,
        id: &str,
        name: &str,
        asset_count: i64,
    ) -> anyhow::Result<()> {
        sqlx::query(
            r#"
            INSERT INTO albums (id, name, asset_count, last_sync)
            VALUES (?, ?, ?, datetime('now'))
            ON CONFLICT(id) DO UPDATE SET
                name = excluded.name,
                asset_count = excluded.asset_count,
                last_sync = datetime('now')
            "#,
        )
        .bind(id)
        .bind(name)
        .bind(asset_count)
        .execute(pool)
        .await?;
        Ok(())
    }

    pub async fn get_all(pool: &sqlx::SqlitePool) -> anyhow::Result<Vec<Album>> {
        let albums = sqlx::query_as::<_, Album>("SELECT * FROM albums ORDER BY name")
            .fetch_all(pool)
            .await?;
        Ok(albums)
    }

    pub async fn get_by_id(pool: &sqlx::SqlitePool, id: &str) -> anyhow::Result<Option<Album>> {
        let album = sqlx::query_as::<_, Album>("SELECT * FROM albums WHERE id = ?")
            .bind(id)
            .fetch_optional(pool)
            .await?;
        Ok(album)
    }
}

impl SyncedImage {
    pub async fn upsert(
        pool: &sqlx::SqlitePool,
        id: &str,
        album_id: &str,
        filename: &str,
        checksum: Option<&str>,
        original_path: Option<&str>,
        file_size: Option<i64>,
    ) -> anyhow::Result<()> {
        sqlx::query(
            r#"
            INSERT INTO synced_images (id, album_id, filename, checksum, original_path, file_size, synced_at)
            VALUES (?, ?, ?, ?, ?, ?, datetime('now'))
            ON CONFLICT(id) DO UPDATE SET
                album_id = excluded.album_id,
                filename = excluded.filename,
                checksum = excluded.checksum,
                original_path = excluded.original_path,
                file_size = excluded.file_size,
                synced_at = datetime('now')
            "#,
        )
        .bind(id)
        .bind(album_id)
        .bind(filename)
        .bind(checksum)
        .bind(original_path)
        .bind(file_size)
        .execute(pool)
        .await?;
        Ok(())
    }

    pub async fn mark_converted(
        pool: &sqlx::SqlitePool,
        id: &str,
        avif_path: &str,
        thumbnail_path: &str,
    ) -> anyhow::Result<()> {
        sqlx::query(
            r#"
            UPDATE synced_images
            SET avif_path = ?, thumbnail_path = ?, converted_at = datetime('now')
            WHERE id = ?
            "#,
        )
        .bind(avif_path)
        .bind(thumbnail_path)
        .bind(id)
        .execute(pool)
        .await?;
        Ok(())
    }

    pub async fn get_by_id(
        pool: &sqlx::SqlitePool,
        id: &str,
    ) -> anyhow::Result<Option<SyncedImage>> {
        let image = sqlx::query_as::<_, SyncedImage>("SELECT * FROM synced_images WHERE id = ?")
            .bind(id)
            .fetch_optional(pool)
            .await?;
        Ok(image)
    }

    pub async fn get_by_album(
        pool: &sqlx::SqlitePool,
        album_id: &str,
    ) -> anyhow::Result<Vec<SyncedImage>> {
        let images = sqlx::query_as::<_, SyncedImage>(
            "SELECT * FROM synced_images WHERE album_id = ? ORDER BY filename",
        )
        .bind(album_id)
        .fetch_all(pool)
        .await?;
        Ok(images)
    }

    pub async fn get_by_album_paginated(
        pool: &sqlx::SqlitePool,
        album_id: &str,
        offset: i64,
        limit: i64,
    ) -> anyhow::Result<Vec<SyncedImage>> {
        let images = sqlx::query_as::<_, SyncedImage>(
            "SELECT * FROM synced_images WHERE album_id = ? AND avif_path IS NOT NULL ORDER BY filename LIMIT ? OFFSET ?",
        )
        .bind(album_id)
        .bind(limit)
        .bind(offset)
        .fetch_all(pool)
        .await?;
        Ok(images)
    }

    pub async fn count_by_album(pool: &sqlx::SqlitePool, album_id: &str) -> anyhow::Result<i64> {
        let count: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM synced_images WHERE album_id = ? AND avif_path IS NOT NULL",
        )
        .bind(album_id)
        .fetch_one(pool)
        .await?;
        Ok(count.0)
    }

    pub async fn get_unconverted(pool: &sqlx::SqlitePool) -> anyhow::Result<Vec<SyncedImage>> {
        // Include images that either haven't been converted yet, or were converted
        // before thumbnail support was added (thumbnail_path is NULL)
        let images = sqlx::query_as::<_, SyncedImage>(
            "SELECT * FROM synced_images WHERE original_path IS NOT NULL AND (converted_at IS NULL OR thumbnail_path IS NULL)",
        )
        .fetch_all(pool)
        .await?;
        Ok(images)
    }

    pub async fn get_all_synced_ids(pool: &sqlx::SqlitePool) -> anyhow::Result<Vec<String>> {
        let ids: Vec<(String,)> = sqlx::query_as("SELECT id FROM synced_images")
            .fetch_all(pool)
            .await?;
        Ok(ids.into_iter().map(|(id,)| id).collect())
    }

    pub async fn delete_by_id(pool: &sqlx::SqlitePool, id: &str) -> anyhow::Result<()> {
        sqlx::query("DELETE FROM synced_images WHERE id = ?")
            .bind(id)
            .execute(pool)
            .await?;
        Ok(())
    }

    /// Clear conversion data for all images (resets avif_path, thumbnail_path, converted_at)
    pub async fn clear_all_conversions(pool: &sqlx::SqlitePool) -> anyhow::Result<u64> {
        let result = sqlx::query(
            "UPDATE synced_images SET avif_path = NULL, thumbnail_path = NULL, converted_at = NULL",
        )
        .execute(pool)
        .await?;
        Ok(result.rows_affected())
    }
}
