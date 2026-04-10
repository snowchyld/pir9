# syntax=docker/dockerfile:1
# Dockerfile for pir9-imdb service
# Multi-stage build with cargo-chef for optimal dependency caching

# ============================================================================
# Stage 1: Install cargo-chef
# ============================================================================
FROM rust:1.93-bookworm AS chef

RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    cargo install cargo-chef

WORKDIR /app

# ============================================================================
# Stage 2: Analyze dependencies and create recipe
# ============================================================================
FROM chef AS planner

COPY Cargo.toml Cargo.lock* ./
COPY src ./src

RUN cargo chef prepare --recipe-path recipe.json

# ============================================================================
# Stage 3: Build dependencies (cached until Cargo.toml/lock change)
# ============================================================================
FROM chef AS builder

# Cook dependencies from recipe (this layer is cached)
COPY --from=planner /app/recipe.json recipe.json
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    cargo chef cook --release --recipe-path recipe.json

# Copy source and build application
COPY Cargo.toml Cargo.lock* ./
COPY src ./src
COPY migrations ./migrations

RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    cargo build --release

# ============================================================================
# Stage 4: Minimal runtime image
# ============================================================================
FROM debian:bookworm-slim

WORKDIR /app

# Install runtime dependencies (no libssl needed — using rustls)
RUN apt-get update && apt-get install -y \
    ca-certificates \
    curl \
    && rm -rf /var/lib/apt/lists/*

# Copy binary
COPY --from=builder /app/target/release/pir9-imdb /app/pir9-imdb

# Copy migrations for runtime execution
COPY --from=builder /app/migrations /app/migrations

# Environment
ENV PIR9_IMDB_PORT=8990
ENV RUST_LOG=info,pir9_imdb=debug

EXPOSE 8990

# Run as non-root user for security
RUN groupadd -g 1000 pir9 && useradd -u 1000 -g pir9 -s /bin/false -d /app pir9 \
    && chown -R pir9:pir9 /app
USER pir9

CMD ["/app/pir9-imdb"]
