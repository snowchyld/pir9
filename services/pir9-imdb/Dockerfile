# Build stage
FROM rust:1.93-bookworm AS builder

WORKDIR /app

# Copy manifests
COPY Cargo.toml Cargo.lock* ./

# Create dummy src for caching dependencies
RUN mkdir src && echo "fn main() {}" > src/main.rs

# Build dependencies only
RUN cargo build --release && rm -rf src

# Copy actual source
COPY src ./src
COPY migrations ./migrations

# Build the application
RUN touch src/main.rs && cargo build --release

# Runtime stage
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    curl \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Copy binary
COPY --from=builder /app/target/release/pir9-imdb /app/pir9-imdb

# Copy migrations for runtime execution
COPY --from=builder /app/migrations /app/migrations

# Environment
ENV PIR9_IMDB_PORT=8990
ENV RUST_LOG=info,pir9_imdb=debug

EXPOSE 8990

CMD ["/app/pir9-imdb"]
