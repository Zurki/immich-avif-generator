# Build stage - use AMD64 platform to avoid ARM-specific compilation issues
FROM --platform=linux/amd64 rust:1.85-bookworm AS builder

WORKDIR /app

# Install dependencies for image processing
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    nasm \
    cmake \
    && rm -rf /var/lib/apt/lists/*

# Copy manifests
COPY Cargo.toml Cargo.lock ./

# Create dummy source to cache dependencies
RUN mkdir src && \
    echo "fn main() {}" > src/main.rs && \
    cargo build --release && \
    rm -rf src

# Copy actual source code
COPY src ./src

# Build the application (touch main.rs to force rebuild)
RUN touch src/main.rs && cargo build --release

# Runtime stage
FROM --platform=linux/amd64 debian:bookworm-slim

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

# Copy example config (user should mount their own config)
COPY config.example.toml /app/config.example.toml

# Set environment variables
ENV RUST_LOG=info

# Expose the default port
EXPOSE 3000

# Default command runs the full pipeline
CMD ["avif-generator", "--config", "/app/config.toml", "run"]
