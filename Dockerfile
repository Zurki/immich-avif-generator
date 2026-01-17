# Build stage
FROM rust:1.84-bookworm AS builder

WORKDIR /app

# Install dependencies for image processing
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    nasm \
    cmake \
    && rm -rf /var/lib/apt/lists/*

# Copy all source files - bust cache by copying everything together
COPY . .

# Build the application
RUN cargo build --release

# Runtime stage
FROM debian:bookworm-slim

WORKDIR /app

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    && rm -rf /var/lib/apt/lists/*

# Copy the binary from builder
COPY --from=builder /app/target/release/avif-generator /usr/local/bin/avif-generator

# Create data directory
RUN mkdir -p /app/data

# Environment variables for configuration
ENV IMMICH_URL=""
ENV IMMICH_API_KEY=""
ENV IMMICH_ALBUMS=""
ENV STORAGE_PATH="/app/data"
ENV SERVER_HOST="0.0.0.0"
ENV SERVER_PORT="3000"
ENV SYNC_DELETE_REMOVED="false"
ENV SYNC_PARALLEL_DOWNLOADS="4"
ENV SYNC_PARALLEL_CONVERSIONS="2"
ENV RUST_LOG="info"

EXPOSE 3000

CMD ["avif-generator", "run"]
