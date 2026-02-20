#!/bin/bash
# Entrypoint script for pir9
# Handles PUID/PGID for proper volume permissions (like linuxserver.io containers)

set -e

PUID=${PUID:-1000}
PGID=${PGID:-1000}

echo "Starting pir9 with UID=$PUID GID=$PGID"

# Create pir9 group and user with specified IDs if they don't match
if [ "$(id -u pir9 2>/dev/null)" != "$PUID" ]; then
    # Delete existing pir9 user if exists
    userdel pir9 2>/dev/null || true
    groupdel pir9 2>/dev/null || true

    # Create group with specified GID
    groupadd -g "$PGID" pir9

    # Create user with specified UID
    useradd -u "$PUID" -g pir9 -s /bin/false pir9
fi

# Ensure directories exist and have correct ownership
for dir in /config /data /logs /backups /app/cache; do
    mkdir -p "$dir"
    chown -R pir9:pir9 "$dir"
done

# Run as pir9 user
exec gosu pir9 /app/pir9 "$@"
