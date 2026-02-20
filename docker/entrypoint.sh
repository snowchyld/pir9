#!/bin/bash
# Entrypoint script for pir9
# Runs as the pir9 user (UID/GID 1000, baked into the image)

set -e

echo "Starting pir9 as user pir9 ($(id pir9))"

# Ensure data directories have correct ownership (volumes may be root-owned on first run)
for dir in /config /data /logs /backups /app/cache; do
    if [ -d "$dir" ]; then
        chown -R pir9:pir9 "$dir" 2>/dev/null || true
    fi
done

# Run as pir9 user
exec gosu pir9 /app/pir9 "$@"
