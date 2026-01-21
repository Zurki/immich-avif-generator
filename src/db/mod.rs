pub mod models;

use anyhow::Result;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePool, SqlitePoolOptions};
use std::path::Path;
use std::str::FromStr;

pub async fn create_pool(db_path: &Path) -> Result<SqlitePool> {
    let db_url = format!("sqlite:{}?mode=rwc", db_path.display());

    let options = SqliteConnectOptions::from_str(&db_url)?.create_if_missing(true);

    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect_with(options)
        .await?;

    run_migrations(&pool).await?;

    Ok(pool)
}

async fn run_migrations(pool: &SqlitePool) -> Result<()> {
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS albums (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            asset_count INTEGER DEFAULT 0,
            last_sync DATETIME
        )
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS synced_images (
            id TEXT PRIMARY KEY,
            album_id TEXT NOT NULL,
            filename TEXT NOT NULL,
            checksum TEXT,
            original_path TEXT,
            avif_path TEXT,
            thumbnail_path TEXT,
            file_size INTEGER,
            synced_at DATETIME,
            converted_at DATETIME,
            FOREIGN KEY (album_id) REFERENCES albums(id)
        )
        "#,
    )
    .execute(pool)
    .await?;

    // Migration: add thumbnail_path column if it doesn't exist
    sqlx::query(
        r#"
        ALTER TABLE synced_images ADD COLUMN thumbnail_path TEXT
        "#,
    )
    .execute(pool)
    .await
    .ok(); // Ignore error if column already exists

    sqlx::query(
        r#"
        CREATE INDEX IF NOT EXISTS idx_synced_images_album
        ON synced_images(album_id)
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE INDEX IF NOT EXISTS idx_synced_images_checksum
        ON synced_images(checksum)
        "#,
    )
    .execute(pool)
    .await?;

    Ok(())
}
