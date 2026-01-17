use crate::db::models::{Album, SyncedImage};
use axum::{
    extract::{Path, State},
    http::{header, StatusCode},
    response::{IntoResponse, Response},
    routing::get,
    Json, Router,
};
use serde::Serialize;
use sqlx::SqlitePool;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::fs::File;
use tokio::io::AsyncReadExt;
use tower_http::cors::{Any, CorsLayer};
use tracing::error;

#[derive(Clone)]
pub struct AppState {
    pub pool: SqlitePool,
    pub avif_path: PathBuf,
}

#[derive(Serialize)]
struct AlbumListResponse {
    albums: Vec<AlbumInfo>,
}

#[derive(Serialize)]
struct AlbumInfo {
    id: String,
    name: String,
    image_count: i64,
}

#[derive(Serialize)]
struct ImageListResponse {
    album_id: String,
    album_name: String,
    images: Vec<ImageInfo>,
}

#[derive(Serialize)]
struct ImageInfo {
    id: String,
    filename: String,
    url: String,
}

#[derive(Serialize)]
struct ImageMetadata {
    id: String,
    filename: String,
    album_id: String,
    file_size: Option<i64>,
    synced_at: Option<String>,
    converted_at: Option<String>,
}

pub fn create_router(state: AppState) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    Router::new()
        .route("/", get(root))
        .route("/albums", get(list_albums))
        .route("/albums/{album_id}", get(get_album))
        .route("/images/{image_id}", get(serve_image))
        .route("/images/{image_id}/metadata", get(get_image_metadata))
        .layer(cors)
        .with_state(Arc::new(state))
}

async fn root() -> &'static str {
    "AVIF Generator API"
}

async fn list_albums(
    State(state): State<Arc<AppState>>,
) -> Result<Json<AlbumListResponse>, AppError> {
    let albums = Album::get_all(&state.pool).await?;

    let album_infos: Vec<AlbumInfo> = albums
        .into_iter()
        .map(|a| AlbumInfo {
            id: a.id,
            name: a.name,
            image_count: a.asset_count.unwrap_or(0),
        })
        .collect();

    Ok(Json(AlbumListResponse {
        albums: album_infos,
    }))
}

async fn get_album(
    State(state): State<Arc<AppState>>,
    Path(album_id): Path<String>,
) -> Result<Json<ImageListResponse>, AppError> {
    let album = Album::get_by_id(&state.pool, &album_id)
        .await?
        .ok_or_else(|| AppError::NotFound("Album not found".to_string()))?;

    let images = SyncedImage::get_by_album(&state.pool, &album_id).await?;

    let image_infos: Vec<ImageInfo> = images
        .into_iter()
        .filter(|img| img.avif_path.is_some())
        .map(|img| ImageInfo {
            url: format!("/images/{}", img.id),
            id: img.id,
            filename: img.filename,
        })
        .collect();

    Ok(Json(ImageListResponse {
        album_id: album.id,
        album_name: album.name,
        images: image_infos,
    }))
}

async fn serve_image(
    State(state): State<Arc<AppState>>,
    Path(image_id): Path<String>,
) -> Result<Response, AppError> {
    let image = SyncedImage::get_by_id(&state.pool, &image_id)
        .await?
        .ok_or_else(|| AppError::NotFound("Image not found".to_string()))?;

    let avif_path = image
        .avif_path
        .ok_or_else(|| AppError::NotFound("AVIF not yet converted".to_string()))?;

    let path = PathBuf::from(&avif_path);
    if !path.exists() {
        return Err(AppError::NotFound(
            "AVIF file not found on disk".to_string(),
        ));
    }

    let mut file = File::open(&path).await.map_err(|e| {
        error!("Failed to open file {:?}: {}", path, e);
        AppError::Internal("Failed to read image".to_string())
    })?;

    let mut contents = Vec::new();
    file.read_to_end(&mut contents).await.map_err(|e| {
        error!("Failed to read file {:?}: {}", path, e);
        AppError::Internal("Failed to read image".to_string())
    })?;

    Ok((
        [
            (header::CONTENT_TYPE, "image/avif"),
            (header::CACHE_CONTROL, "public, max-age=31536000, immutable"),
        ],
        contents,
    )
        .into_response())
}

async fn get_image_metadata(
    State(state): State<Arc<AppState>>,
    Path(image_id): Path<String>,
) -> Result<Json<ImageMetadata>, AppError> {
    let image = SyncedImage::get_by_id(&state.pool, &image_id)
        .await?
        .ok_or_else(|| AppError::NotFound("Image not found".to_string()))?;

    Ok(Json(ImageMetadata {
        id: image.id,
        filename: image.filename,
        album_id: image.album_id,
        file_size: image.file_size,
        synced_at: image.synced_at.map(|d| d.to_rfc3339()),
        converted_at: image.converted_at.map(|d| d.to_rfc3339()),
    }))
}

#[derive(Debug)]
pub enum AppError {
    NotFound(String),
    Internal(String),
    Database(sqlx::Error),
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            AppError::NotFound(msg) => (StatusCode::NOT_FOUND, msg),
            AppError::Internal(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg),
            AppError::Database(e) => {
                error!("Database error: {}", e);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Database error".to_string(),
                )
            }
        };

        (status, Json(serde_json::json!({ "error": message }))).into_response()
    }
}

impl From<sqlx::Error> for AppError {
    fn from(e: sqlx::Error) -> Self {
        AppError::Database(e)
    }
}

impl From<anyhow::Error> for AppError {
    fn from(e: anyhow::Error) -> Self {
        AppError::Internal(e.to_string())
    }
}
