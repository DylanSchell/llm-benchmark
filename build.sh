#!/usr/bin/env bash
# Build scripts for llm-benchmark (Rust workspace)
# Replaces Maven-based build pipeline.

set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"

usage() {
    cat >&2 <<EOF
Usage: $(basename "$0") <command> [options]

Commands:
  docker-build   Build the runner Docker image (default)
  docker-run     Run a command inside the runner container
  cargo          Pass-through to cargo
  test           Run all workspace tests
  clean          Clean build artifacts

Options:
  --arch         Target architecture for Docker build (linux/amd64 or linux/arm64)
  --tag          Image tag (default: llm-benchmark/runner:latest)
EOF
    exit 1
}

docker_build() {
    local tag="${TAG:-llm-benchmark/runner:latest}"
    local arch="${ARCH:-}"
    local platform_args=()
    if [[ -n "$arch" ]]; then
        platform_args=(--platform "$arch")
        echo "Building Docker image ${tag} (${arch})..." >&2
    else
        echo "Building Docker image ${tag} (host arch)..." >&2
    fi
    docker buildx build \
        "${platform_args[@]}" \
        --tag "${tag}" \
        -f docker/Dockerfile.runner.debian \
        "${SCRIPT_DIR}/docker"
}

docker_run() {
    local tag="${TAG:-llm-benchmark/runner:latest}"
    docker run --rm -it "${tag}" "$@"
}

cargo_cmd() {
    cargo "$@"
}

test_all() {
    cargo test --workspace
}

clean_artifacts() {
    rm -rf target
    echo "Cleaned build artifacts." >&2
}

# Parse options (can appear before or after the command)
ARCH=""
TAG=""
COMMAND=""
while [[ $# -gt 0 ]]; do
    case "$1" in
        --arch) ARCH="$2"; shift 2 ;;
        --tag)  TAG="$2";   shift 2 ;;
        docker-build|build|docker-run|run|cargo|test|clean)
            COMMAND="$1"
            shift
            ;;
        *)      break ;;
    esac
done

case "$COMMAND" in
    docker-build|build)   docker_build ;;
    docker-run|run)       docker_run "$@" ;;
    cargo)                cargo_cmd "$@" ;;
    test)                 test_all ;;
    clean)                clean_artifacts ;;
    *)                    usage ;;
esac
