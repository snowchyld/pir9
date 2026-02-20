#!/bin/bash
# PostToolUse hook: lightweight security scan on edited files
# Only outputs when findings exist — zero noise on clean files

INPUT=$(cat)
FILE_PATH=$(echo "$INPUT" | jq -r '.tool_input.file_path // empty')

if [[ -z "$FILE_PATH" ]]; then
  exit 0
fi

# Semgrep on all files it supports — don't filter by language
OUTPUT=$(semgrep --config auto --quiet "$FILE_PATH" 2>/dev/null | grep -v '^$' | head -20)
if [[ -n "$OUTPUT" ]]; then
  echo "⚠ semgrep: $(basename "$FILE_PATH")"
  echo "$OUTPUT"
fi

# Cargo audit when dependency files change
if [[ "$FILE_PATH" == */Cargo.toml ]]; then
  AUDIT=$(cd /home/drew/dev/pir9 && cargo audit 2>/dev/null | grep -E "^(Crate:|Version:|Warning:|ID:| +→)" | head -15)
  if [[ -n "$AUDIT" ]]; then
    echo "⚠ cargo audit:"
    echo "$AUDIT"
  fi
fi

exit 0
