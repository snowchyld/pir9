#!/usr/bin/env bash
# IMDB dataset sync/process script
# Usage:
#   ./scripts/imdb-sync.sh                    # Full sync (download + process all datasets)
#   ./scripts/imdb-sync.sh process            # Process only (use cached TSV files)
#   ./scripts/imdb-sync.sh process title.basics  # Process a specific dataset
#   ./scripts/imdb-sync.sh status             # Show sync status
#   ./scripts/imdb-sync.sh cancel             # Cancel running sync

set -euo pipefail

IMDB_CONTAINER="${IMDB_CONTAINER:-pir9-imdb}"
IMDB_URL="http://localhost:8990"

cmd="${1:-sync}"
dataset="${2:-}"

case "$cmd" in
  status)
    docker exec "$IMDB_CONTAINER" curl -s "$IMDB_URL/api/sync/status" | jq .
    ;;

  cancel)
    docker exec "$IMDB_CONTAINER" curl -s -X POST "$IMDB_URL/api/sync/cancel" | jq .
    ;;

  sync)
    echo "Starting full sync (download + process)..."
    if [ -n "$dataset" ]; then
      docker exec "$IMDB_CONTAINER" curl -s -X POST "$IMDB_URL/api/sync" \
        -H 'Content-Type: application/json' \
        -d "{\"datasets\":[\"$dataset\"]}" | jq .
    else
      docker exec "$IMDB_CONTAINER" curl -s -X POST "$IMDB_URL/api/sync" \
        -H 'Content-Type: application/json' \
        -d '{"datasets":[]}' | jq .
    fi
    ;;

  process)
    echo "Processing cached datasets (no download)..."
    if [ -n "$dataset" ]; then
      docker exec "$IMDB_CONTAINER" curl -s -X POST "$IMDB_URL/api/process" \
        -H 'Content-Type: application/json' \
        -d "{\"datasets\":[\"$dataset\"]}" | jq .
    else
      docker exec "$IMDB_CONTAINER" curl -s -X POST "$IMDB_URL/api/process" \
        -H 'Content-Type: application/json' \
        -d '{"datasets":[]}' | jq .
    fi
    ;;

  datasets)
    docker exec "$IMDB_CONTAINER" curl -s "$IMDB_URL/api/datasets" | jq .
    ;;

  backfill)
    echo "Backfilling episode air dates from TVMaze..."
    limit="${dataset:-1000}"
    docker exec "$IMDB_CONTAINER" curl -s -X POST "$IMDB_URL/api/backfill-air-dates" \
      -H 'Content-Type: application/json' \
      -d "{\"limit\":$limit}" | jq .
    ;;

  logs)
    docker logs "$IMDB_CONTAINER" --tail "${dataset:-50}" -f
    ;;

  *)
    echo "Usage: $0 {sync|process|status|cancel|datasets|backfill|logs} [dataset|limit]"
    echo ""
    echo "Commands:"
    echo "  sync [dataset]     Full sync (download + process)"
    echo "  process [dataset]  Process cached TSV files only (no download)"
    echo "  status             Show sync progress"
    echo "  cancel             Cancel running sync"
    echo "  datasets           Show dataset metadata"
    echo "  backfill [limit]   Backfill episode air dates from TVMaze (default: 1000)"
    echo "  logs [n]           Follow container logs (default: last 50 lines)"
    echo ""
    echo "Datasets: title.basics, title.episodes, title.ratings, name.basics, title.principals"
    echo ""
    echo "Data locations:"
    echo "  TSV/gz files:  tmp/imdb_data/"
    exit 1
    ;;
esac
