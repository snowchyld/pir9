#!/usr/bin/env bash
# Deploy pir9 worker to Synology NAS (nastoo.drew.red)
#
# Pulls the latest image and recreates the worker container via
# the Container Manager project at /volume1/docker/p9/compose.yaml.
#
# Usage:
#   ./reg/deploy.sh              # Pull + recreate worker
#   ./reg/deploy.sh --pull-only  # Pull image only (no restart)

set -euo pipefail

REMOTE_HOST="nastoo.drew.red"
REGISTRY="nas.drew.red:2443"
IMAGE="${REGISTRY}/pir9:latest"
PROJECT="pir9"
COMPOSE_FILE="/volume1/docker/p9/compose.yaml"

# Colors
GREEN='\033[0;32m'
NC='\033[0m'

info() { echo -e "${GREEN}[deploy]${NC} $*"; }

# Synology's non-login SSH shell has a near-empty PATH
remote() { ssh "${REMOTE_HOST}" "export PATH=/usr/local/bin:/usr/bin:/usr/sbin:/bin:/sbin:/usr/syno/bin && $1"; }

if [[ "${1:-}" == "--pull-only" ]]; then
    info "Pulling ${IMAGE} on ${REMOTE_HOST}..."
    remote "sudo docker stop pir9-worker"
    remote "sudo docker rm pir9-worker"
    remote "sudo docker pull ${IMAGE}"
    info "Pull complete."
    exit 0
fi

# ── Pull + recreate via Container Manager project ─────────────────────
info "Deploying ${IMAGE} on ${REMOTE_HOST}..."
remote "sudo docker compose -p ${PROJECT} -f ${COMPOSE_FILE} up -d --pull always"

# ── Verify ────────────────────────────────────────────────────────────
sleep 2
remote "sudo docker ps --filter name=pir9-worker --format 'table {{.Names}}\t{{.Status}}\t{{.Image}}'"

info "Deploy complete."
