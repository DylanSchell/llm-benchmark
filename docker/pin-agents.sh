#!/usr/bin/env bash
# Re-pin the packages the runner image installs, and lint that none of them is unpinned.
#
# Most pins come from npm, but pi is installed from its standalone binary release (so that its
# runtime does not dictate the image's Node version). Re-pinning pi therefore also refreshes
# both of its linux asset checksums from that release's SHA256SUMS file.
#
# Any pin change is a runner-image change, so this script bumps docker/RUNNER_VERSION and
# refreshes docker/runner.lock for you. See docs/specs/runner-image.md.
#
# Usage:
#   pin-agents.sh [--dry-run] [--allow-major] [--patch|--minor|--major]
#   pin-agents.sh --check
#
#   (default)      Resolve the latest published version of every npm pin, update
#                  docker/pins.env, bump the runner version (patch by default) and refresh
#                  docker/runner.lock.
#   --dry-run      Print what would change; write nothing.
#   --allow-major  Also accept updates that cross a major version. Without this, a major jump
#                  is reported and skipped, because it is the kind of change that silently
#                  breaks a track (e.g. jest 29 -> 30, @types/node 20 -> 24).
#   --patch        Bump the patch component of the runner version (default).
#   --minor        Bump the minor component.
#   --major        Bump the major component.
#   --check        Offline lint: every `npm install -g` in the Dockerfile must be pinned to a
#                  variable that is both declared as a build ARG and defined in pins.env, and
#                  every npm pin in pins.env must actually be used. Non-zero exit on failure.
#
# Toolchain pins (GO_VERSION, RUST_VERSION, UV_VERSION, FD_VERSION, GRADLE_VERSION, BASE_IMAGE)
# are not handled here; bump those by hand and refresh the Gradle checksum alongside.

set -euo pipefail

DOCKER_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_DIR="$(cd "${DOCKER_DIR}/.." && pwd)"
PINS="${DOCKER_DIR}/pins.env"
DOCKERFILE="${DOCKER_DIR}/Dockerfile.runner.debian"
VERSION_FILE="${DOCKER_DIR}/RUNNER_VERSION"

# build.sh supplies runner_version / docker_input_hash / write_runner_lock.
# shellcheck source=../build.sh
. "${REPO_DIR}/build.sh"

# pin variable -> npm package. Every entry must appear in the Dockerfile's npm install blocks.
# pi is installed from a GitHub release asset rather than npm, so it is tracked separately:
# the version plus one checksum per published linux architecture. Kept in sync with the
# Dockerfile, which selects between them using buildx's TARGETARCH.
PI_NPM_PACKAGE="@earendil-works/pi-coding-agent"
PI_RELEASE_URL_BASE="https://github.com/earendil-works/pi/releases/download"
PI_RELEASE_PINS=(PI_CODING_AGENT_VERSION PI_SHA256_X64 PI_SHA256_ARM64)

# npm-installed packages. pi is deliberately absent — see PI_RELEASE_PINS above.
NPM_PINS=(
    "CLAUDE_CODE_VERSION=@anthropic-ai/claude-code"
    "PI_CAVEMAN_VERSION=pi-caveman"
    "SUPI_BASH_TIMEOUT_VERSION=@mrclrchtr/supi-bash-timeout"
    "JEST_VERSION=jest"
    "BABEL_CORE_VERSION=@babel/core"
    "BABEL_PRESET_ENV_VERSION=@babel/preset-env"
    "EXERCISM_BABEL_PRESET_VERSION=@exercism/babel-preset-javascript"
    "EXERCISM_ESLINT_CONFIG_VERSION=@exercism/eslint-config-javascript"
    "TYPES_JEST_VERSION=@types/jest"
    "TYPES_NODE_VERSION=@types/node"
    "BABEL_JEST_VERSION=babel-jest"
    "CORE_JS_VERSION=core-js"
    "ESLINT_VERSION=eslint"
)

usage() {
    sed -n '2,/^$/p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//' >&2
    exit "${1:-0}"
}

pin_value() {
    sed -n "s/^$1=//p" "$PINS"
}

set_pin() {
    sed -i.bak "s|^$1=.*|$1=$2|" "$PINS"
    rm -f "${PINS}.bak"
}

# Pull the package specs out of every `npm install -g` invocation, following line
# continuations. Quotes are stripped and the block is tokenised on whitespace, so an unquoted
# or otherwise unpinned package is caught too, not just a quoted one.
dockerfile_npm_specs() {
    awk '/npm install -g/ { grab=1 }
         grab { print }
         grab && $0 !~ /\\$/ { grab=0 }' "$DOCKERFILE" \
        | sed -e 's/\\[[:space:]]*$//' \
              -e 's/^[[:space:]]*RUN[[:space:]]*//' \
              -e 's/npm install -g//' \
              -e 's/"//g' \
        | tr -s '[:space:]' '\n' \
        | sed -e '/^$/d' -e '/^[&|;][&|;]*$/d'
}

lint() {
    local rc=0 spec var entry key specs
    specs="$(dockerfile_npm_specs)"

    if [[ -z "$specs" ]]; then
        echo "FAIL: no npm installs found in ${DOCKERFILE##*/} — parser out of date?" >&2
        return 1
    fi

    while IFS= read -r spec; do
        [[ -z "$spec" ]] && continue
        var="$(printf '%s' "$spec" | sed -n 's/.*@\${\([A-Z0-9_]*\)}$/\1/p')"
        if [[ -z "$var" ]]; then
            echo "FAIL: '${spec}' is not pinned — expected package@\${VAR_VERSION}" >&2
            rc=1
            continue
        fi
        if ! grep -qE "^${var}=" "$PINS"; then
            echo "FAIL: '${spec}' uses \${${var}}, which is not defined in docker/pins.env" >&2
            rc=1
        fi
        if ! grep -qE "^ARG ${var}$" "$DOCKERFILE"; then
            echo "FAIL: '${spec}' uses \${${var}}, which is not declared as an ARG" >&2
            rc=1
        fi
    done <<< "$specs"

    # Every npm pin should actually be used by an install.
    for entry in "${NPM_PINS[@]}"; do
        key="${entry%%=*}"
        if ! printf '%s\n' "$specs" | grep -qF "@\${${key}}"; then
            echo "FAIL: docker/pins.env defines ${key} but no npm install uses it" >&2
            rc=1
        fi
    done

    # Release-asset pins (pi) are not npm installs, so check them on their own terms.
    local var
    for var in "${PI_RELEASE_PINS[@]}"; do
        if ! grep -qE "^${var}=" "$PINS"; then
            echo "FAIL: ${var} is not defined in docker/pins.env" >&2
            rc=1
        fi
        if ! grep -qE "^ARG ${var}$" "$DOCKERFILE"; then
            echo "FAIL: ${var} is not declared as an ARG in ${DOCKERFILE##*/}" >&2
            rc=1
        fi
        if ! grep -qF "\${${var}}" "$DOCKERFILE"; then
            echo "FAIL: ${var} is defined but never used in ${DOCKERFILE##*/}" >&2
            rc=1
        fi
    done

    if (( rc == 0 )); then
        echo "ok: all npm installs are pinned through docker/pins.env"
    fi
    return $rc
}

latest_version() {
    npm view "$1" version 2>/dev/null | tr -d '[:space:]'
}

fetch_url() {
    if command -v curl >/dev/null 2>&1; then
        curl -fsSL "$1"
    elif command -v wget >/dev/null 2>&1; then
        wget -q -O - "$1"
    else
        return 1
    fi
}

# Echoes "<x64-sha256> <arm64-sha256>" for the given pi release, from its SHA256SUMS file.
pi_asset_hashes() {
    local version="$1" sums x64 arm64
    sums="$(fetch_url "${PI_RELEASE_URL_BASE}/v${version}/SHA256SUMS")" || return 1
    x64="$(printf '%s\n' "$sums" | awk '$2 == "pi-linux-x64.tar.gz" {print $1}')"
    arm64="$(printf '%s\n' "$sums" | awk '$2 == "pi-linux-arm64.tar.gz" {print $1}')"
    if [[ -z "$x64" || -z "$arm64" ]]; then
        return 1
    fi
    printf '%s %s' "$x64" "$arm64"
}

same_major() {
    [[ "${1%%.*}" == "${2%%.*}" ]]
}

bump_version() {
    local part="$1" cur major minor patch
    cur="$(runner_version)"
    IFS=. read -r major minor patch <<< "$cur"
    major="${major:-0}"; minor="${minor:-0}"; patch="${patch:-0}"
    case "$part" in
        major) major=$((major + 1)); minor=0; patch=0 ;;
        minor) minor=$((minor + 1)); patch=0 ;;
        *)     patch=$((patch + 1)) ;;
    esac
    printf '%s.%s.%s' "$major" "$minor" "$patch"
}

repin() {
    local dry_run="$1" allow_major="$2" bump_part="$3"
    local -a updates=()
    local entry key pkg current latest u count

    echo "Resolving latest published versions…" >&2
    for entry in "${NPM_PINS[@]}"; do
        key="${entry%%=*}"; pkg="${entry#*=}"
        current="$(pin_value "$key")"
        latest="$(latest_version "$pkg")"
        if [[ -z "$latest" ]]; then
            echo "  warn: could not resolve ${pkg} — skipped" >&2
            continue
        fi
        [[ "$latest" == "$current" ]] && continue
        if ! same_major "$latest" "$current" && [[ "$allow_major" != "1" ]]; then
            echo "  skip: ${pkg} ${current} -> ${latest}  (major jump; re-run with --allow-major)" >&2
            continue
        fi
        echo "  ${key}: ${current} -> ${latest}  (${pkg})" >&2
        updates+=("${key}=${latest}")
    done

    # pi: the version is resolved from npm (it tracks the release), and both asset checksums are
    # refreshed from that release's SHA256SUMS so a version bump cannot leave stale hashes behind.
    local pi_current pi_latest pair
    pi_current="$(pin_value PI_CODING_AGENT_VERSION)"
    pi_latest="$(latest_version "$PI_NPM_PACKAGE")"
    if [[ -n "$pi_latest" && "$pi_latest" != "$pi_current" ]]; then
        if same_major "$pi_latest" "$pi_current" || [[ "$allow_major" == "1" ]]; then
            echo "  PI_CODING_AGENT_VERSION: ${pi_current} -> ${pi_latest}  (release asset)" >&2
            updates+=("PI_CODING_AGENT_VERSION=${pi_latest}")
            if pair="$(pi_asset_hashes "$pi_latest")"; then
                updates+=("PI_SHA256_X64=${pair%% *}")
                updates+=("PI_SHA256_ARM64=${pair##* }")
                echo "    refreshed both linux asset checksums from SHA256SUMS" >&2
            else
                echo "    warn: could not fetch SHA256SUMS for v${pi_latest}; update the checksums by hand" >&2
            fi
        else
            echo "  skip: pi ${pi_current} -> ${pi_latest}  (major jump; re-run with --allow-major)" >&2
        fi
    fi

    count="${#updates[@]}"
    if (( count == 0 )); then
        echo "Nothing to do — every pin is current." >&2
        return 0
    fi

    if [[ "$dry_run" == "1" ]]; then
        echo "Dry run: ${count} pin(s) would change, runner version would bump (${bump_part}). Nothing written." >&2
        return 0
    fi

    for u in "${updates[@]}"; do
        set_pin "${u%%=*}" "${u#*=}"
    done

    local old_version new_version
    old_version="$(runner_version)"
    new_version="$(bump_version "$bump_part")"
    printf '%s\n' "$new_version" > "$VERSION_FILE"

    # pins.env and RUNNER_VERSION are now updated, so the lock can be regenerated. This is the
    # same computation `docker-build` performs after a successful build.
    write_runner_lock "$new_version" "$(docker_input_hash)"

    echo >&2
    echo "Updated ${count} pin(s); runner ${old_version} -> ${new_version}." >&2
    echo "docker/runner.lock refreshed. Rebuild the image with: ./build.sh docker-build" >&2
}

main() {
    local mode="repin" bump="patch" allow_major=0

    while [[ $# -gt 0 ]]; do
        case "$1" in
            --check)       mode="check" ;;
            --dry-run)     mode="dry-run" ;;
            --patch)       bump="patch" ;;
            --minor)       bump="minor" ;;
            --major)       bump="major" ;;
            --allow-major) allow_major=1 ;;
            -h|--help)     usage 0 ;;
            *)             echo "error: unknown argument: $1" >&2; usage 2 ;;
        esac
        shift
    done

    case "$mode" in
        check)   lint ;;
        dry-run) repin 1 "$allow_major" "$bump" ;;
        *)       repin 0 "$allow_major" "$bump" ;;
    esac
}

main "$@"
