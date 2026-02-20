#!/bin/bash
# PostToolUse hook: runs Biome lint check on edited frontend files

INPUT=$(cat)
FILE_PATH=$(echo "$INPUT" | jq -r '.tool_input.file_path // empty')

if [[ -z "$FILE_PATH" ]]; then
  exit 0
fi

# Only check frontend TypeScript source files
if [[ "$FILE_PATH" != */frontend/src/*.ts ]]; then
  exit 0
fi

cd /home/drew/dev/pir9/frontend && npx biome check --no-errors-on-unmatched "$FILE_PATH" 2>&1 | tail -30

exit 0
