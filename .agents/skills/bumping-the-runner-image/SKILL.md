---
name: bumping-the-runner-image
description: Change, re-pin, or rebuild the Docker runner image. Use when touching docker/pins.env, docker/agents.env, Dockerfile.runner.debian, docker/runner.lock, or the agent CLI versions inside the image.
---

# Bumping the Runner Image

The runner image is the container each exercise is benchmarked *inside*: language toolchains plus
the agent CLIs under test. It is a separate artifact from the binary and is versioned by
`Cargo.toml` — the same number tags the binary, the image and the release.

## Files

| File | Role |
| --- | --- |
| `docker/pins.env` | Every externally-sourced version. The single source of truth. |
| `docker/agents.env` | Which optional agents the image packages. A hashed build input. |
| `docker/pin-agents.sh` | Re-pins npm packages (and pi's binary checksums), bumps the version, refreshes the lock. |
| `docker/runner.lock` | Generated. Records version + input hash of the last build. |
| `docker/Dockerfile.runner.debian` | One image for all languages. |

The Dockerfile's build args deliberately carry **no defaults**: a bare `docker build` must fail
loudly rather than silently produce an unpinned image. Always build through `./build.sh`.

## Re-pinning

```bash
./docker/pin-agents.sh --dry-run     # what would move
./docker/pin-agents.sh               # patch bump; majors are reported and skipped
./docker/pin-agents.sh --allow-major # only when you have read the changelog
./docker/pin-agents.sh --check       # lint: nothing unpinned (CI runs this too)
```

`pin-agents.sh` also bumps `Cargo.toml` and refreshes `Cargo.lock` and `docker/runner.lock`,
because **any pin change is a runner-image change**. Flipping `INSTALL_CLAUDE` in `agents.env`
counts the same way: it changes the input hash.

Claude Code is off by default — Anthropic's licence grants no redistribution, so an image that
contains it cannot be published. Keep it off in anything that ships.

## Verify

```bash
./build.sh docker-verify                     # inputs vs runner.lock (CI's docker-inputs job)
./build.sh docker-build                      # host platform
./build.sh docker-build --arch linux/amd64,linux/arm64
```

## Traps

- Forgetting the version bump is the common failure: `docker-verify` fails on the PR, which is the
  point. Do not edit `runner.lock` by hand — only a real build can refresh it.
- The base image is pinned **by digest** so a rebuild cannot pick up a different JDK underneath.
  Checking for a newer digest is `docker buildx imagetools inspect …`.
- A pin that changes what an agent prints or how it exits changes the benchmark results, not just
  the image. Say so in the PR description.
