#!/bin/bash
# Entrypoint script for pir9
# Adjusts the pir9 user UID/GID to match PUID/PGID env vars,
# then drops privileges via gosu.

set -e

PUID=${PUID:-1000}
PGID=${PGID:-1000}

# Adjust group GID if it doesn't match
if [ "$(id -g pir9)" != "$PGID" ]; then
    groupmod -o -g "$PGID" pir9
fi

# Adjust user UID if it doesn't match
if [ "$(id -u pir9)" != "$PUID" ]; then
    usermod -o -u "$PUID" pir9
fi

echo "Starting pir9 as uid=$PUID gid=$PGID"

# Ensure data directories have correct ownership
for dir in /config /data /logs /backups /app/cache; do
    if [ -d "$dir" ]; then
        chown -R pir9:pir9 "$dir" 2>/dev/null || true
    fi
done

# Run as pir9 user (now with the correct UID/GID)
exec gosu pir9 /app/pir9 "$@"
