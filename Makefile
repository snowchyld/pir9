# pir9 Makefile
# Simplifies Docker operations for development and deployment

# Registry configuration
REGISTRY ?= nas.drew.red:2443
IMAGE_NAME ?= pir9
VERSION := $(shell grep '^version' Cargo.toml | head -1 | sed 's/.*"\(.*\)"/\1/')
IMAGE_TAG ?= $(VERSION)
FULL_IMAGE = $(REGISTRY)/$(IMAGE_NAME):$(IMAGE_TAG)
LATEST_IMAGE = $(REGISTRY)/$(IMAGE_NAME):latest

# Default target: build, push to registry, and restart
.DEFAULT_GOAL := release-restart

.PHONY: help start stop restart status logs \
        build build-api build-frontend build-all \
        deploy deploy-api deploy-frontend \
        dev dev-api dev-frontend \
        clean clean-all shell-api shell-frontend \
        test lint \
        push pull release release-restart

# Default target
help:
	@echo "pir9 Development Commands"
	@echo "=============================="
	@echo ""
	@echo "Docker Operations:"
	@echo "  make start          - Start all services"
	@echo "  make stop           - Stop all services"
	@echo "  make restart        - Restart all services"
	@echo "  make status         - Show container status"
	@echo "  make logs           - Tail logs from all services"
	@echo "  make logs-api       - Tail API logs only"
	@echo "  make logs-frontend  - Tail frontend logs only"
	@echo ""
	@echo "Build Commands:"
	@echo "  make build          - Build all Docker images"
	@echo "  make build-api      - Build API Docker image"
	@echo "  make build-frontend - Build frontend Docker image"
	@echo "  make build-all      - Full rebuild (no cache)"
	@echo ""
	@echo "Registry & Deployment:"
	@echo "  make release        - Build and push v$(VERSION) to registry"
	@echo "  make release-restart - Build, push v$(VERSION), and restart local"
	@echo "  make push           - Push $(FULL_IMAGE) + :latest"
	@echo "  make pull           - Pull :latest from registry"
	@echo ""
	@echo "Development Builds:"
	@echo "  make dev-api        - Build Rust API locally (release)"
	@echo "  make dev-frontend   - Build frontend locally"
	@echo ""
	@echo "Quick Deploy (no Docker rebuild):"
	@echo "  make deploy         - Deploy both API and frontend"
	@echo "  make deploy-api     - Deploy API binary to running container"
	@echo "  make deploy-frontend- Deploy frontend to running container"
	@echo ""
	@echo "Utilities:"
	@echo "  make shell-api      - Open shell in API container"
	@echo "  make shell-frontend - Open shell in frontend container"
	@echo "  make clean          - Remove stopped containers"
	@echo "  make clean-all      - Remove all containers, images, volumes"
	@echo ""
	@echo "Testing:"
	@echo "  make test           - Run Rust tests"
	@echo "  make lint           - Run linters (cargo clippy + biome)"
	@echo ""
	@echo "Configuration:"
	@echo "  REGISTRY=$(REGISTRY)"
	@echo "  IMAGE=$(FULL_IMAGE) → :latest"

# =============================================================================
# Docker Operations
# =============================================================================

start:
	@echo "Starting services..."
	docker compose up -d
	@echo "Services started. Access at http://localhost:8989"

stop:
	@echo "Stopping services..."
	docker compose down

restart: stop build start

status:
	@docker compose ps

logs:
	docker compose logs -f

logs-api:
	docker logs -f pir9-api

logs-frontend:
	docker logs -f pir9-frontend

# =============================================================================
# Docker Build
# =============================================================================

build: build-api build-frontend
	@echo "All images built successfully"

build-api:
	@echo "Building API Docker image..."
	docker compose build api

build-frontend:
	@echo "Building frontend Docker image..."
	docker compose build frontend

build-all:
	@echo "Full rebuild (no cache)..."
	docker compose build --no-cache

# =============================================================================
# Development Builds (Local)
# =============================================================================

dev-api:
	@echo "Building Rust API (release)..."
	cargo build --release
	@echo "Binary ready at: target/release/pir9"

dev-frontend:
	@echo "Installing frontend dependencies..."
	cd frontend && npm install
	@echo "Building frontend..."
	cd frontend && npm run build
	@echo "Frontend built to: frontend/dist/"

# =============================================================================
# Quick Deploy (updates running containers without full rebuild)
# =============================================================================

deploy: deploy-api deploy-frontend
	@echo "Deployment complete"

deploy-api: dev-api
	@echo "Deploying API to running container..."
	@docker stop pir9-api 2>/dev/null || true
	docker cp target/release/pir9 pir9-api:/app/pir9
	docker start pir9-api
	@echo "API deployed and restarted"

deploy-frontend: dev-frontend
	@echo "Deploying frontend to running container..."
	docker cp frontend/dist/. pir9-frontend:/usr/share/nginx/html/
	docker exec pir9-frontend nginx -s reload
	@echo "Frontend deployed"

# =============================================================================
# Shell Access
# =============================================================================

shell-api:
	docker exec -it pir9-api /bin/bash

shell-frontend:
	docker exec -it pir9-frontend /bin/sh

# =============================================================================
# Cleanup
# =============================================================================

clean:
	@echo "Removing stopped containers..."
	docker compose down --remove-orphans

clean-all:
	@echo "WARNING: This will remove all containers, images, and volumes!"
	@read -p "Are you sure? [y/N] " confirm && [ "$$confirm" = "y" ]
	docker compose down -v --rmi all --remove-orphans

# =============================================================================
# Testing & Linting
# =============================================================================

test:
	cargo test

lint:
	@echo "Running Rust linter..."
	cargo clippy -- -D warnings
	@echo ""
	@echo "Running frontend linter..."
	cd frontend && npm install --silent && npm run lint

# =============================================================================
# Development Shortcuts
# =============================================================================

# Watch mode for frontend development
watch-frontend:
	@echo "Installing frontend dependencies..."
	cd frontend && npm install
	cd frontend && npm run dev

# Run API locally (without Docker)
run-api:
	RUST_LOG=debug cargo run --release

# Database migrations
migrate:
	cargo run -- migrate

# Check what would be built
check:
	cargo check

# =============================================================================
# Registry Operations
# =============================================================================

# Build and tag for registry (version + latest)
docker-build:
	@echo "Building Docker image: $(FULL_IMAGE)"
	docker build -t $(FULL_IMAGE) -f Dockerfile .
	docker tag $(FULL_IMAGE) $(LATEST_IMAGE)

# Push to registry (version + latest)
push:
	@echo "Pushing to registry: $(FULL_IMAGE)"
	docker push $(FULL_IMAGE)
	docker push $(LATEST_IMAGE)
	@echo "Image pushed: $(FULL_IMAGE) + :latest"

# Pull from registry (latest)
pull:
	@echo "Pulling from registry: $(LATEST_IMAGE)"
	docker pull $(LATEST_IMAGE)

# Full release: build and push
release: docker-build push
	@echo ""
	@echo "Release complete: $(FULL_IMAGE) (also tagged :latest)"

# Full release + restart local containers
release-restart: dev-frontend release
	@echo ""
	@echo "Restarting local containers..."
#	docker compose down
	docker compose build
	docker compose up -d
	@echo ""
	@echo "Local containers restarted. Check logs with: make logs"
	./reg/nas.sh
	./reg/nastoo.sh

# Build with specific tag override
release-tag:
	@if [ -z "$(TAG)" ]; then echo "Usage: make release-tag TAG=v1.0.0"; exit 1; fi
	$(MAKE) release IMAGE_TAG=$(TAG)
