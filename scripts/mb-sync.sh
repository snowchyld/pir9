#!/usr/bin/env bash
# MusicBrainz dataset sync/process script
# Usage:
#   ./scripts/mb-sync.sh                    # Full sync (download + process core datasets)
#   ./scripts/mb-sync.sh sync all           # Full sync ALL datasets (including unused)
#   ./scripts/mb-sync.sh process            # Process only (use cached .tar.xz files)
#   ./scripts/mb-sync.sh process release    # Process only releases
#   ./scripts/mb-sync.sh status             # Show sync status
#   ./scripts/mb-sync.sh cancel             # Cancel running sync
#
# Core datasets (used by pir9): artist, release-group, release
# Unused datasets: recording, label, work, area, series, event, instrument, place

set -euo pipefail

MB_CONTAINER="${MB_CONTAINER:-pir9-musicbrainz}"
MB_URL="http://localhost:8991"

# Only sync datasets that pir9 actually queries (ADR-006, audit #10)
CORE_DATASETS='["artist.tar.xz","release-group.tar.xz","release.tar.xz"]'

cmd="${1:-sync}"
dataset="${2:-}"

case "$cmd" in
  status)
    docker exec "$MB_CONTAINER" curl -s "$MB_URL/api/sync/status" | jq '
      to_entries | map(select(.value | type == "object")) |
      map({dataset: .key, status: .value.status, rows: .value.rowsProcessed, running: .value.isRunning}) |
      sort_by(.dataset)'
    ;;

  cancel)
    docker exec "$MB_CONTAINER" curl -s -X POST "$MB_URL/api/sync/cancel" | jq .
    ;;

  sync)
    if [ "$dataset" = "all" ]; then
      echo "Starting full sync (ALL datasets)..."
      docker exec "$MB_CONTAINER" curl -s -X POST "$MB_URL/api/sync" \
        -H 'Content-Type: application/json' \
        -d '{"datasets":[]}' | jq .
    elif [ -n "$dataset" ]; then
      echo "Starting sync for $dataset..."
      docker exec "$MB_CONTAINER" curl -s -X POST "$MB_URL/api/sync" \
        -H 'Content-Type: application/json' \
        -d "{\"datasets\":[\"${dataset}.tar.xz\"]}" | jq .
    else
      echo "Starting sync (core datasets: artist, release-group, release)..."
      docker exec "$MB_CONTAINER" curl -s -X POST "$MB_URL/api/sync" \
        -H 'Content-Type: application/json' \
        -d "{\"datasets\":$CORE_DATASETS}" | jq .
    fi
    ;;

  process)
    if [ "$dataset" = "all" ]; then
      echo "Processing ALL cached datasets..."
      docker exec "$MB_CONTAINER" curl -s -X POST "$MB_URL/api/process" \
        -H 'Content-Type: application/json' \
        -d '{"datasets":[]}' | jq .
    elif [ -n "$dataset" ]; then
      echo "Processing $dataset..."
      docker exec "$MB_CONTAINER" curl -s -X POST "$MB_URL/api/process" \
        -H 'Content-Type: application/json' \
        -d "{\"datasets\":[\"${dataset}.tar.xz\"]}" | jq .
    else
      echo "Processing core datasets (artist, release-group, release)..."
      docker exec "$MB_CONTAINER" curl -s -X POST "$MB_URL/api/process" \
        -H 'Content-Type: application/json' \
        -d "{\"datasets\":$CORE_DATASETS}" | jq .
    fi
    ;;

  stats)
    docker exec "$MB_CONTAINER" curl -s "$MB_URL/api/stats" | jq .
    ;;

  datasets)
    docker exec "$MB_CONTAINER" curl -s "$MB_URL/api/datasets" | jq .
    ;;

  logs)
    docker logs "$MB_CONTAINER" --tail "${dataset:-50}" -f
    ;;

  *)
    echo "Usage: $0 {sync|process|status|cancel|stats|datasets|logs} [dataset|all]"
    echo ""
    echo "Commands:"
    echo "  sync               Sync core datasets only (artist, release-group, release)"
    echo "  sync all           Sync ALL datasets (including unused)"
    echo "  sync <dataset>     Sync a specific dataset"
    echo "  process            Process cached core datasets only"
    echo "  process all        Process ALL cached datasets"
    echo "  process <dataset>  Process a specific cached dataset"
    echo "  status             Show sync progress for all datasets"
    echo "  cancel             Cancel running sync"
    echo "  stats              Show database statistics"
    echo "  datasets           Show dataset metadata"
    echo "  logs [n]           Follow container logs (default: last 50 lines)"
    echo ""
    echo "Datasets: artist, release-group, release, label, recording, work, area, series, event, instrument, place"
    echo ""
    echo "Data locations:"
    echo "  .tar.xz files:   tmp/musicbrainz_data/"
    echo "  Extracted JSONL:  tmp/musicbrainz_data/extracted/"
    exit 1
    ;;
esac
