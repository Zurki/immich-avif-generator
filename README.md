# AVIF Generator with HTTP-Server

Syncs images from Immich albums and converts them to AVIF format.

## Usage

```bash
# Copy and edit config
cp config.example.toml config.toml

# Run with Docker Compose
docker compose up
```

## Configuration

Edit `config.toml`:

- `immich.url` - Your Immich server URL (use `host.docker.internal` if Immich runs locally)
- `immich.api_key` - Your Immich API key
- `immich.albums` - List of album UUIDs to sync

## Commands

```bash
avif-generator --config config.toml run      # Sync, convert, and start server
avif-generator --config config.toml sync     # Sync only
avif-generator --config config.toml convert  # Convert only
avif-generator --config config.toml serve    # Start server only
```
