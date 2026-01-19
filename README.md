# AVIF Generator

Syncs images from Immich albums and converts them to AVIF format.

## Usage with Docker (Coolify/Environment Variables)

Set these environment variables:

| Variable | Required | Default | Description |
|----------|----------|---------|-------------|
| `IMMICH_URL` | Yes | - | Immich server URL |
| `IMMICH_API_KEY` | Yes | - | Immich API key |
| `IMMICH_ALBUMS` | Yes | - | Comma-separated album UUIDs |
| `STORAGE_PATH` | No | `/app/data` | Data storage path |
| `SERVER_HOST` | No | `0.0.0.0` | Server bind address |
| `SERVER_PORT` | No | `3000` | Server port |
| `SYNC_DELETE_REMOVED` | No | `false` | Delete local files when removed from album |
| `SYNC_PARALLEL_DOWNLOADS` | No | `4` | Parallel download count |
| `SYNC_PARALLEL_CONVERSIONS` | No | `2` | Parallel conversion count |

## Usage with Docker Compose

```yaml
services:
  avif-generator:
    build: .
    ports:
      - "3000:3000"
    volumes:
      - ./data:/app/data
    environment:
      - IMMICH_URL=https://your-immich-server.com
      - IMMICH_API_KEY=your-api-key
      - IMMICH_ALBUMS=album-uuid-1,album-uuid-2
```

## Usage with Config File

```bash
cp config.example.toml config.toml
# Edit config.toml with your settings
avif-generator --config config.toml run
```

## Commands

```bash
avif-generator run      # Sync, convert, and start server
avif-generator sync     # Sync only
avif-generator convert  # Convert only
avif-generator serve    # Start server only
avif-generator ping     # Test Immich connection
```

## API Routes

| Method | Endpoint | Description |
|--------|----------|-------------|
| GET | `/` | Health check, returns "AVIF Generator API" |
| GET | `/albums` | List all synced albums |
| GET | `/albums/:album_id` | Get images in an album (paginated) |
| GET | `/images/:image_id` | Serve AVIF image file |
| GET | `/images/:image_id/metadata` | Get image metadata |

### Pagination

The `/albums/:album_id` endpoint supports pagination with query parameters:

| Parameter | Default | Description |
|-----------|---------|-------------|
| `offset` | `0` | Number of images to skip |
| `limit` | `20` | Number of images to return (max 100) |

Example:
```
GET /albums/abc123                    # First 20 images
GET /albums/abc123?offset=20          # Images 21-40
GET /albums/abc123?offset=0&limit=50  # First 50 images
```

Response includes pagination info:
```json
{
  "album_id": "abc123",
  "album_name": "My Album",
  "images": [...],
  "pagination": {
    "total": 150,
    "offset": 0,
    "limit": 20,
    "has_more": true
  }
}
```
