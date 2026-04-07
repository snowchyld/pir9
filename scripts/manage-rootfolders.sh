#!/usr/bin/env bash
#
# manage-rootfolders.sh — Bulk root folder management for pir9
#
# Usage:
#   ./scripts/manage-rootfolders.sh <series|anime|movie> <paths-file>
#
# The paths file should contain one folder path per line.
# Lines starting with # and empty lines are ignored.
#
# Environment:
#   PIR9_URL     Base URL (default: http://localhost:8989)
#   PIR9_API_KEY API key (optional, sent as X-Api-Key header)

set -euo pipefail

# --- Config ---
PIR9_URL="${PIR9_URL:-http://localhost:8989}"
API_BASE="${PIR9_URL}/api/v5"

# --- Arg parsing ---
if [[ $# -lt 2 ]]; then
  echo "Usage: $0 <series|anime|movie> <paths-file>"
  echo ""
  echo "  Content types: series, anime, movie"
  echo "  paths-file:    text file with one folder path per line"
  echo ""
  echo "Environment:"
  echo "  PIR9_URL       Base URL (default: http://localhost:8989)"
  echo "  PIR9_API_KEY   API key (optional)"
  exit 1
fi

CONTENT_TYPE="$1"
PATHS_FILE="$2"

case "$CONTENT_TYPE" in
  series|anime|movie) ;;
  *)
    echo "Error: content type must be one of: series, anime, movie"
    exit 1
    ;;
esac

if [[ ! -f "$PATHS_FILE" ]]; then
  echo "Error: file not found: $PATHS_FILE"
  exit 1
fi

# Check dependencies
for cmd in curl jq; do
  if ! command -v "$cmd" &>/dev/null; then
    echo "Error: $cmd is required but not installed"
    exit 1
  fi
done

# --- Helpers ---
curl_opts=(-s -f)
if [[ -n "${PIR9_API_KEY:-}" ]]; then
  curl_opts+=(-H "X-Api-Key: ${PIR9_API_KEY}")
fi

api_get() {
  curl "${curl_opts[@]}" -H "Content-Type: application/json" "$API_BASE$1"
}

api_post() {
  curl "${curl_opts[@]}" -X POST -H "Content-Type: application/json" -d "$2" "$API_BASE$1"
}

# --- Step 1: Get existing root folders ---
echo "Fetching existing root folders from ${PIR9_URL}..."

existing_json=$(api_get "/rootfolder") || {
  echo "Error: failed to reach pir9 API at ${PIR9_URL}"
  echo "  Check PIR9_URL and ensure pir9 is running"
  exit 1
}

# Build set of existing paths for this content type
existing_paths=$(echo "$existing_json" | jq -r --arg ct "$CONTENT_TYPE" \
  '.[] | select(.contentType == $ct) | .path')

# --- Step 2: Process paths file ---
added=0
existed=0
failed=0
total=0

echo ""
echo "Processing paths for content type: $CONTENT_TYPE"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"

while IFS= read -r line; do
  # Skip empty lines and comments
  [[ -z "$line" || "$line" =~ ^[[:space:]]*# ]] && continue

  # Trim whitespace
  path="$(echo "$line" | sed 's/^[[:space:]]*//;s/[[:space:]]*$//')"
  [[ -z "$path" ]] && continue

  total=$((total + 1))

  # Check if already exists
  if echo "$existing_paths" | grep -qxF "$path"; then
    echo "  EXISTS  $path"
    existed=$((existed + 1))
    continue
  fi

  # Add as new root folder
  body=$(jq -n --arg p "$path" --arg ct "$CONTENT_TYPE" \
    '{path: $p, contentType: $ct}')

  if result=$(api_post "/rootfolder" "$body" 2>&1); then
    echo "  ADDED   $path"
    added=$((added + 1))
  else
    echo "  FAILED  $path"
    failed=$((failed + 1))
  fi

done < "$PATHS_FILE"

# --- Step 3: Fetch updated root folders and count unmapped ---
echo ""
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "Summary"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "  Paths in file:    $total"
echo "  Already existed:  $existed"
echo "  Newly added:      $added"
if [[ $failed -gt 0 ]]; then
  echo "  Failed:           $failed"
fi

echo ""
echo "Checking for unimported folders..."

updated_json=$(api_get "/rootfolder") || {
  echo "Error: failed to fetch updated root folders"
  exit 1
}

# Filter to our content type and show unmapped counts per root folder
total_unmapped=0

echo ""
echo "Root folders ($CONTENT_TYPE):"
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"

echo "$updated_json" | jq -r --arg ct "$CONTENT_TYPE" \
  '.[] | select(.contentType == $ct) | "\(.path)\t\(.unmappedFolders | length)"' | \
while IFS=$'\t' read -r folder_path unmapped_count; do
  total_unmapped=$((total_unmapped + unmapped_count))
  if [[ "$unmapped_count" -gt 0 ]]; then
    echo "  $folder_path — $unmapped_count unimported"
  else
    echo "  $folder_path — all imported"
  fi
done

# Calculate total separately (subshell in pipe above loses variable)
total_unmapped=$(echo "$updated_json" | jq --arg ct "$CONTENT_TYPE" \
  '[.[] | select(.contentType == $ct) | .unmappedFolders | length] | add // 0')

echo ""
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo "Total unimported folders ($CONTENT_TYPE): $total_unmapped"

if [[ "$total_unmapped" -gt 0 ]]; then
  echo ""
  echo "Use the pir9 web UI to import these folders:"
  case "$CONTENT_TYPE" in
    series) echo "  ${PIR9_URL}/series/add" ;;
    anime)  echo "  ${PIR9_URL}/anime/add" ;;
    movie)  echo "  ${PIR9_URL}/movies/add" ;;
  esac
fi
