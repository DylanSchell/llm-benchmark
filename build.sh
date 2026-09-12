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

# External version pins live in docker/pins.env. The Dockerfile's ARGs deliberately have no
# defaults, so every one of them has to be supplied here — a bare `docker build` fails loudly
# rather than silently producing an unpinned image.
load_pins() {
    local pins="${SCRIPT_DIR}/docker/pins.env"
    if [[ ! -f "$pins" ]]; then
        echo "error: ${pins} not found" >&2
        exit 1
    fi
    # shellcheck disable=SC1090
    set -a
    . "$pins"
    set +a
}

# Build args are derived from this name list, so adding a pin to pins.env plus the Dockerfile
# is enough — this script needs no edit.
PIN_NAMES=(
    BASE_IMAGE
    GO_VERSION FD_VERSION NODE_MAJOR RUST_VERSION UV_VERSION
    GRADLE_VERSION GRADLE_SHA256
    CLAUDE_CODE_VERSION PI_CODING_AGENT_VERSION PI_CAVEMAN_VERSION SUPI_BASH_TIMEOUT_VERSION
    JEST_VERSION BABEL_CORE_VERSION BABEL_PRESET_ENV_VERSION
    EXERCISM_BABEL_PRESET_VERSION EXERCISM_ESLINT_CONFIG_VERSION
    TYPES_JEST_VERSION TYPES_NODE_VERSION BABEL_JEST_VERSION
    CORE_JS_VERSION ESLINT_VERSION
)

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

    load_pins

    local -a build_args=()
    local name
    for name in "${PIN_NAMES[@]}"; do
        build_args+=(--build-arg "${name}=${!name}")
    done

    echo "Pinned: go=${GO_VERSION} rust=${RUST_VERSION} uv=${UV_VERSION} gradle=${GRADLE_VERSION}" >&2
    echo "        claude-code=${CLAUDE_CODE_VERSION} pi=${PI_CODING_AGENT_VERSION}" >&2

    docker buildx build \
        "${platform_args[@]}" \
        "${build_args[@]}" \
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
