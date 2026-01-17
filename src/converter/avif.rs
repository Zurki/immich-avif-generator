use crate::config::Config;
use crate::db::models::SyncedImage;
use anyhow::{Context, Result};
use futures::stream::{self, StreamExt};
use image::DynamicImage;
use ravif::{Encoder, Img};
use rgb::RGBA8;
use sqlx::SqlitePool;
use std::path::{Path, PathBuf};
use tracing::{debug, info, warn};

pub struct AvifConverter {
    pool: SqlitePool,
    config: Config,
}

#[derive(Debug)]
pub struct ConversionResult {
    pub converted: usize,
    pub skipped: usize,
    pub failed: usize,
}

impl AvifConverter {
    pub fn new(pool: SqlitePool, config: Config) -> Self {
        Self { pool, config }
    }

    pub async fn convert_all(&self) -> Result<ConversionResult> {
        let unconverted = SyncedImage::get_unconverted(&self.pool).await?;

        info!("Found {} images to convert", unconverted.len());

        let mut result = ConversionResult {
            converted: 0,
            skipped: 0,
            failed: 0,
        };

        let results: Vec<_> = stream::iter(unconverted)
            .map(|image| {
                let pool = self.pool.clone();
                let avif_base = self.config.avif_path();
                async move { Self::convert_image(&pool, &image, &avif_base).await }
            })
            .buffer_unordered(self.config.sync.parallel_conversions)
            .collect()
            .await;

        for conversion_result in results {
            match conversion_result {
                Ok(true) => result.converted += 1,
                Ok(false) => result.skipped += 1,
                Err(e) => {
                    warn!("Conversion failed: {}", e);
                    result.failed += 1;
                }
            }
        }

        info!(
            "Conversion complete: {} converted, {} skipped, {} failed",
            result.converted, result.skipped, result.failed
        );

        Ok(result)
    }

    async fn convert_image(
        pool: &SqlitePool,
        image: &SyncedImage,
        avif_base: &Path,
    ) -> Result<bool> {
        let original_path = match &image.original_path {
            Some(p) => PathBuf::from(p),
            None => return Ok(false),
        };

        if !original_path.exists() {
            warn!("Original file not found: {:?}", original_path);
            return Ok(false);
        }

        let avif_path = avif_base
            .join(&image.album_id)
            .join(format!("{}.avif", image.id));

        if avif_path.exists() {
            debug!("AVIF already exists: {:?}", avif_path);
            SyncedImage::mark_converted(pool, &image.id, avif_path.to_str().unwrap_or("")).await?;
            return Ok(false);
        }

        info!("Converting: {} -> {:?}", image.filename, avif_path);

        let original_path_clone = original_path.clone();
        let avif_path_clone = avif_path.clone();

        let result = tokio::task::spawn_blocking(move || {
            Self::do_conversion(&original_path_clone, &avif_path_clone)
        })
        .await?;

        match result {
            Ok(()) => {
                SyncedImage::mark_converted(pool, &image.id, avif_path.to_str().unwrap_or(""))
                    .await?;
                Ok(true)
            }
            Err(e) => Err(e),
        }
    }

    fn do_conversion(source: &Path, dest: &Path) -> Result<()> {
        let img = image::open(source).context("Failed to open source image")?;

        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let rgba = Self::to_rgba(&img);
        let width = img.width() as usize;
        let height = img.height() as usize;

        let pixels: Vec<RGBA8> = rgba
            .chunks(4)
            .map(|c| RGBA8::new(c[0], c[1], c[2], c[3]))
            .collect();

        let img_ref = Img::new(&pixels[..], width, height);

        let encoder = Encoder::new()
            .with_quality(100.0)
            .with_speed(4)
            .with_alpha_quality(100.0);

        let result = encoder
            .encode_rgba(img_ref)
            .context("Failed to encode AVIF")?;

        std::fs::write(dest, result.avif_file)?;

        debug!("Converted {:?} to {:?}", source, dest);
        Ok(())
    }

    fn to_rgba(img: &DynamicImage) -> Vec<u8> {
        img.to_rgba8().into_raw()
    }
}
