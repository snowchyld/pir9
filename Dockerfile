# Build stage
FROM rust:1.93-slim-bookworm AS builder

WORKDIR /app

# Install dependencies (curl needed for utoipa-swagger-ui build)
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    libsqlite3-dev \
    curl \
    && rm -rf /var/lib/apt/lists/*

# Copy manifests and build script
COPY Cargo.toml Cargo.lock build.rs ./
COPY migration ./migration

# Copy source code
COPY src ./src
COPY migrations ./migrations

# Build release binary with Redis support for distributed scanning
RUN cargo build --release --features redis-events

# Runtime stage
FROM debian:bookworm-slim

WORKDIR /app

# Install runtime dependencies and gosu for proper privilege dropping
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libsqlite3-0 \
    gosu \
    wget \
    && rm -rf /var/lib/apt/lists/*

# Create directories
RUN mkdir -p /config /data /logs /backups /app/cache

# Copy binary from builder
COPY --from=builder /app/target/release/pir9 /app/pir9

# Copy migrations
COPY --from=builder /app/migrations /app/migrations

# Copy entrypoint script
COPY docker/entrypoint.sh /entrypoint.sh
RUN chmod +x /entrypoint.sh

# Expose port
EXPOSE 8989

# Volumes
VOLUME ["/config", "/data", "/logs", "/backups"]

# Environment - defaults matching common host user
ENV PUID=1000
ENV PGID=1000
ENV PIR9_CONFIG_DIR=/config
ENV PIR9_DATA_DIR=/data
ENV PIR9_LOG_DIR=/logs
ENV PIR9_BACKUP_DIR=/backups

# Health check
HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
    CMD wget --no-verbose --tries=1 --spider http://localhost:8989/health || exit 1

# Run via entrypoint (handles PUID/PGID)
ENTRYPOINT ["/entrypoint.sh"]
