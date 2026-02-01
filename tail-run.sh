#!/usr/bin/env bash
BASE_PATH=/home/runner/.claude/projects/-workspace/
CONTAINER=$(docker ps --format '{{.ID}}')
if [ "$CONTAINER" != "" ]; then
  JSON_LOG=$(docker exec -w /workspace ${CONTAINER} ls '/home/runner/.claude/projects/-workspace/')
  docker exec "${CONTAINER}" tail -f "$BASE_PATH$JSON_LOG"
fi