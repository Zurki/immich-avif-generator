mod config;
mod converter;
mod db;
mod immich;
mod server;
mod sync;

use anyhow::Result;
use clap::{Parser, Subcommand};
use config::Config;
use converter::AvifConverter;
use db::models::SyncedImage;
use immich::{AuthProvider, ImmichClient};
use server::{AppState, create_router};
use sync::SyncService;
use tracing::{Level, info, warn};
use tracing_subscriber::EnvFilter;

#[derive(Parser)]
#[command(name = "avif-generator")]
#[command(about = "Download Immich albums and serve as AVIF images")]
struct Cli {
    /// Path to config file (optional if using environment variables)
    #[arg(short, long)]
    config: Option<String>,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Sync albums from Immich (incremental download)
    Sync,

    /// Convert downloaded images to AVIF format
    Convert,

    /// Start the HTTP server to serve AVIF images
    Serve,

    /// Sync, convert, and serve (all-in-one)
    Run,

    /// Test connection to Immich server
    Ping,

    /// Delete all AVIF images and reconvert from originals
    Reindex,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::builder()
                .with_default_directive(Level::INFO.into())
                .from_env_lossy(),
        )
        .init();

    let cli = Cli::parse();
    let config = match &cli.config {
        Some(path) => Config::load(path)?,
        None => Config::from_env()?,
    };

    tokio::fs::create_dir_all(&config.original_path()).await?;
    tokio::fs::create_dir_all(&config.avif_path()).await?;

    let pool = db::create_pool(&config.db_path()).await?;
    let auth = AuthProvider::from_config(&config.immich.auth)?;
    let client = ImmichClient::new(&config.immich.url, auth);

    match cli.command {
        Commands::Ping => {
            let info = client.ping().await?;
            println!("Connected to Immich server version: {}", info.version);
        }

        Commands::Sync => {
            let sync_service = SyncService::new(client, pool, config);
            let result = sync_service.sync_all().await?;
            println!(
                "Sync complete: {} downloaded, {} skipped, {} failed",
                result.downloaded, result.skipped, result.failed
            );
        }

        Commands::Convert => {
            let converter = AvifConverter::new(pool, config);
            let result = converter.convert_all().await?;
            println!(
                "Conversion complete: {} converted, {} skipped, {} failed",
                result.converted, result.skipped, result.failed
            );
        }

        Commands::Serve => {
            serve(pool, config).await?;
        }

        Commands::Run => {
            info!("Starting sync...");
            let sync_service = SyncService::new(client, pool.clone(), config.clone());
            let sync_result = sync_service.sync_all().await?;
            info!(
                "Sync complete: {} downloaded, {} skipped",
                sync_result.downloaded, sync_result.skipped
            );

            info!("Starting conversion...");
            let converter = AvifConverter::new(pool.clone(), config.clone());
            let convert_result = converter.convert_all().await?;
            info!(
                "Conversion complete: {} converted, {} skipped",
                convert_result.converted, convert_result.skipped
            );

            info!("Starting server...");
            serve(pool, config).await?;
        }

        Commands::Reindex => {
            info!("Starting reindex...");

            // Step 1: Delete all AVIF files from disk
            let avif_path = config.avif_path();
            info!("Deleting AVIF directory: {:?}", avif_path);
            if avif_path.exists() {
                match tokio::fs::remove_dir_all(&avif_path).await {
                    Ok(_) => info!("Deleted AVIF directory"),
                    Err(e) => warn!("Failed to delete AVIF directory: {}", e),
                }
            }
            // Recreate the empty AVIF directory
            tokio::fs::create_dir_all(&avif_path).await?;

            // Step 2: Clear conversion data in database
            let cleared = SyncedImage::clear_all_conversions(&pool).await?;
            info!("Cleared conversion data for {} images", cleared);

            // Step 3: Re-run conversion
            info!("Starting conversion...");
            let converter = AvifConverter::new(pool, config);
            let result = converter.convert_all().await?;
            println!(
                "Reindex complete: {} converted, {} skipped, {} failed",
                result.converted, result.skipped, result.failed
            );
        }
    }

    Ok(())
}

async fn serve(pool: sqlx::SqlitePool, config: Config) -> Result<()> {
    let state = AppState {
        pool,
        avif_path: config.avif_path(),
    };

    let app = create_router(state);
    let addr = format!("{}:{}", config.server.host, config.server.port);

    info!("Starting server on http://{}", addr);
    println!("Server running at http://{}", addr);

    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
