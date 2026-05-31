#!/usr/bin/env bash
set -euo pipefail

BASE_PATH="/home/runner/.claude/projects/-workspace/"
CONTAINER="$(docker ps --format '{{.ID}}' | head -n 1)"

if [ -n "${CONTAINER:-}" ]; then
  JSON_LOG="$(docker exec -w /workspace "${CONTAINER}" ls '/home/runner/.claude/projects/-workspace/' 2>/dev/null || true)"
  if [ -n "${JSON_LOG:-}" ]; then
    docker exec "${CONTAINER}" tail -f "${BASE_PATH}${JSON_LOG}"
  else
    echo "Error: no JSON log file found in container" >&2
    exit 1
  fi
else
  echo "Error: no running container found" >&2
  exit 1
fi
