#!/bin/sh
# Worker health check: verify heartbeat file is recent (< 60 seconds old)
HEARTBEAT_FILE="/tmp/pir9-worker-heartbeat"
if [ ! -f "$HEARTBEAT_FILE" ]; then
    exit 1
fi
LAST=$(cat "$HEARTBEAT_FILE")
NOW=$(date +%s)
DIFF=$((NOW - LAST))
if [ "$DIFF" -gt 60 ]; then
    exit 1
fi
exit 0
