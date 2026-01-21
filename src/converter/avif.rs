use crate::config::{Config, ImageConfig};
use crate::db::models::SyncedImage;
use anyhow::{Context, Result};
use futures::stream::{self, StreamExt};
use image::imageops::FilterType;
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
                let image_config = self.config.image.clone();
                async move { Self::convert_image(&pool, &image, &avif_base, &image_config).await }
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
        image_config: &ImageConfig,
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

        let thumbnail_path = avif_base
            .join(&image.album_id)
            .join(format!("{}_thumb.avif", image.id));

        if avif_path.exists() && thumbnail_path.exists() {
            debug!("AVIF and thumbnail already exist: {:?}", avif_path);
            SyncedImage::mark_converted(
                pool,
                &image.id,
                avif_path.to_str().unwrap_or(""),
                thumbnail_path.to_str().unwrap_or(""),
            )
            .await?;
            return Ok(false);
        }

        info!("Converting: {} -> {:?}", image.filename, avif_path);

        let original_path_clone = original_path.clone();
        let avif_path_clone = avif_path.clone();
        let thumbnail_path_clone = thumbnail_path.clone();
        let config_clone = image_config.clone();

        let result = tokio::task::spawn_blocking(move || {
            Self::do_conversion(
                &original_path_clone,
                &avif_path_clone,
                &thumbnail_path_clone,
                &config_clone,
            )
        })
        .await?;

        match result {
            Ok(()) => {
                SyncedImage::mark_converted(
                    pool,
                    &image.id,
                    avif_path.to_str().unwrap_or(""),
                    thumbnail_path.to_str().unwrap_or(""),
                )
                .await?;
                Ok(true)
            }
            Err(e) => Err(e),
        }
    }

    fn do_conversion(
        source: &Path,
        dest: &Path,
        thumbnail_dest: &Path,
        config: &ImageConfig,
    ) -> Result<()> {
        let img = image::open(source).context("Failed to open source image")?;

        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent)?;
        }

        // Resize main image if needed
        let resized_img = Self::resize_image(&img, config.max_width);

        // Generate and save main AVIF
        Self::encode_and_save(
            &resized_img,
            dest,
            config.quality,
            config.max_file_size,
            config.min_quality,
        )?;
        debug!("Converted {:?} to {:?}", source, dest);

        // Generate and save thumbnail
        let thumbnail_img = Self::resize_image(&img, config.thumbnail_width);
        Self::encode_and_save(
            &thumbnail_img,
            thumbnail_dest,
            config.quality,
            config.max_file_size,
            config.min_quality,
        )?;
        debug!("Created thumbnail {:?}", thumbnail_dest);

        Ok(())
    }

    fn resize_image(img: &DynamicImage, max_width: u32) -> DynamicImage {
        let (width, height) = (img.width(), img.height());

        if width <= max_width {
            return img.clone();
        }

        let ratio = max_width as f32 / width as f32;
        let new_height = (height as f32 * ratio) as u32;

        img.resize(max_width, new_height, FilterType::Lanczos3)
    }

    fn encode_and_save(
        img: &DynamicImage,
        dest: &Path,
        quality: f32,
        max_file_size: u64,
        min_quality: f32,
    ) -> Result<()> {
        let rgba = Self::to_rgba(img);
        let width = img.width() as usize;
        let height = img.height() as usize;

        let pixels: Vec<RGBA8> = rgba
            .chunks(4)
            .map(|c| RGBA8::new(c[0], c[1], c[2], c[3]))
            .collect();

        let img_ref = Img::new(&pixels[..], width, height);

        let mut current_quality = quality;
        let quality_step = 5.0;

        loop {
            let encoder = Encoder::new()
                .with_quality(current_quality)
                .with_speed(4)
                .with_alpha_quality(current_quality);

            let result = encoder
                .encode_rgba(img_ref.clone())
                .context("Failed to encode AVIF")?;

            let file_size = result.avif_file.len() as u64;

            if file_size <= max_file_size {
                if current_quality < quality {
                    info!(
                        "Reduced quality from {} to {} to meet {}MB limit (final size: {} bytes)",
                        quality,
                        current_quality,
                        max_file_size / (1024 * 1024),
                        file_size
                    );
                }
                std::fs::write(dest, result.avif_file)?;
                return Ok(());
            }

            if current_quality <= min_quality {
                warn!(
                    "File size {} bytes exceeds limit of {} bytes even at minimum quality {}. Saving anyway.",
                    file_size, max_file_size, min_quality
                );
                std::fs::write(dest, result.avif_file)?;
                return Ok(());
            }

            debug!(
                "File size {} bytes exceeds limit, reducing quality from {} to {}",
                file_size,
                current_quality,
                (current_quality - quality_step).max(min_quality)
            );
            current_quality = (current_quality - quality_step).max(min_quality);
        }
    }

    fn to_rgba(img: &DynamicImage) -> Vec<u8> {
        img.to_rgba8().into_raw()
    }
}
