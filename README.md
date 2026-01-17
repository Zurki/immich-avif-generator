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
