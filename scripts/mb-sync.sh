#!/usr/bin/env bash
# MusicBrainz dataset sync/process script
# Usage:
#   ./scripts/mb-sync.sh                    # Full sync (download + process all datasets)
#   ./scripts/mb-sync.sh process            # Process only (use cached .tar.xz files)
#   ./scripts/mb-sync.sh process release    # Process only releases
#   ./scripts/mb-sync.sh status             # Show sync status
#   ./scripts/mb-sync.sh cancel             # Cancel running sync

set -euo pipefail

MB_CONTAINER="${MB_CONTAINER:-pir9-musicbrainz}"
MB_URL="http://localhost:8991"

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
    echo "Starting full sync (download + process)..."
    if [ -n "$dataset" ]; then
      docker exec "$MB_CONTAINER" curl -s -X POST "$MB_URL/api/sync" \
        -H 'Content-Type: application/json' \
        -d "{\"datasets\":[\"${dataset}.tar.xz\"]}" | jq .
    else
      docker exec "$MB_CONTAINER" curl -s -X POST "$MB_URL/api/sync" \
        -H 'Content-Type: application/json' \
        -d '{"datasets":[]}' | jq .
    fi
    ;;

  process)
    echo "Processing cached datasets (no download)..."
    if [ -n "$dataset" ]; then
      docker exec "$MB_CONTAINER" curl -s -X POST "$MB_URL/api/process" \
        -H 'Content-Type: application/json' \
        -d "{\"datasets\":[\"${dataset}.tar.xz\"]}" | jq .
    else
      docker exec "$MB_CONTAINER" curl -s -X POST "$MB_URL/api/process" \
        -H 'Content-Type: application/json' \
        -d '{"datasets":[]}' | jq .
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
    echo "Usage: $0 {sync|process|status|cancel|stats|datasets|logs} [dataset]"
    echo ""
    echo "Commands:"
    echo "  sync [dataset]     Full sync (download + process). Dataset: artist, release-group, release, etc."
    echo "  process [dataset]  Process cached .tar.xz files only (no download)"
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
