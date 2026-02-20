---
name: docker-infra
description: Modify Docker and deployment configuration
user-invocable: true
allowed-tools:
  - Read
  - Write
  - Edit
  - Grep
  - Glob
  - Bash
---

# Docker & Deployment Infrastructure

You are modifying pir9's Docker or deployment configuration.

## Key Files

- `Dockerfile` — Multi-stage build (cargo-chef → planner → builder → runtime)
- `docker-compose.yml` — Production multi-container setup
- `docker-compose.simple.yml` — Simple single-container setup
- `docker-compose.synology-worker.yml` — Synology NAS worker deployment
- `docker/entrypoint.sh` — Container entrypoint script
- `docker/nginx.conf` — Reverse proxy configuration
- `Makefile` — Build shortcuts (`make release`, `make push`, `make deploy`)

## Build Architecture

The Dockerfile uses **multi-stage builds with cargo-chef** for efficient layer caching:

1. **chef** — Install cargo-chef
2. **planner** — Analyze dependencies (`cargo chef prepare`)
3. **builder** — Build dependencies separately, then build app
4. **runtime** — Minimal image with just the binary

Uses `--mount=type=cache` for cargo registry and git cache acceleration.

## Security Requirements

- **Always include `USER` directive** — semgrep will flag containers running as root
- **No secrets in Dockerfile** — use environment variables or mounted secrets
- **Health checks** on all service containers
- **Read-only root filesystem** where possible

## Registry

- Image: `reg.pir9.org:2443/pir9:latest`
- Push with: `make push` or `docker push reg.pir9.org:2443/pir9:latest`

## Deployment Modes

### Production (multi-container)
```bash
docker compose --profile production up -d
```
- pir9 API + scheduler
- PostgreSQL (if configured)
- Redis (if distributed mode)
- Nginx reverse proxy

### Simple (single-container)
```bash
docker compose -f docker-compose.simple.yml up -d
```
- All-in-one with SQLite

### Worker (Synology NAS)
```bash
docker compose -f docker-compose.synology-worker.yml up -d
```
- Scan worker only, connects to server via Redis

## Makefile Targets

| Target | Description |
|--------|-------------|
| `make dev-api` | Build Rust API locally |
| `make dev-frontend` | Build frontend |
| `make release` | Full build and push to registry |
| `make push` | Push image to registry |
| `make deploy` | Quick deploy to running containers |

## Conventions

- Base image: `rust:1.93-bookworm` for builder, `debian:bookworm-slim` for runtime
- Include `ca-certificates` and `libssl-dev` in runtime image for HTTPS
- Label images with version and build date
- Use `.dockerignore` to exclude `target/`, `node_modules/`, etc.
