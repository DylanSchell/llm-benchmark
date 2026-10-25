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

## Before a build

A two-arch build needs real disk headroom, and the Docker VM — not the host — is what runs out.
Check it before blaming the Dockerfile:

```bash
docker run --rm alpine df -h /      # the VM's own free space, not the host's
docker system df                    # images / build cache / volumes, with reclaimable column
```

Under a few GB free, expect failures. Reclaim in this order, least destructive first: stopped
containers, build cache (`docker builder prune -f`), dangling images (`docker image prune -f`),
then superseded runner tags (`:1.4.0` style) — leave `:latest` and the current version tag, because
local benchmark runs pull them. `docker image prune -a` deletes every image no container uses,
including unrelated services on the same daemon; never run it without saying so first.

## When the build fails

**apt "invalid signature" is a full disk, not a key.** This combination:

```
W: GPG error: http://deb.debian.org/debian bookworm InRelease: At least one invalid signature was encountered.
E: The repository 'http://deb.debian.org/debian bookworm InRelease' is not signed.
E: You don't have enough free space in /var/cache/apt/archives/.
```

means the VM ran out of space and apt wrote truncated index files; the signature complaint is the
symptom, and the free-space line appears only in the step that gets far enough to download
packages. Fix the disk, then retry — earlier layers are cached, so a retry resumes mid-Dockerfile.

Do **not** "fix" this by refreshing Debian archive keys or re-pinning the base image. Verified the
hard way: the pinned `bellsoft/liberica-openjdk-debian:17` digest verifies today's `bookworm`
`InRelease` fine once there is room, and a `gpgv` run pointing at one keyring file under
`/usr/share/keyrings/` proves nothing — apt trusts `/etc/apt/trusted.gpg.d/*.asc`, which is not the
same set. Reproduce on a fresh container (`docker run --rm debian:bookworm-slim apt-get update`) before
editing anything under `docker/`; if a clean container succeeds, nothing under `docker/` is wrong.

## Traps

- Forgetting the version bump is the common failure: `docker-verify` fails on the PR, which is the
  point. Do not edit `runner.lock` by hand — only a real build can refresh it.
- The coupling runs the other way too: `Cargo.toml` **is** the image tag, so bumping the version for
  a change that never touches `docker/` (an AoC fixture, say) still requires a real build to refresh
  `runner.lock`, or CI's `docker-inputs` job is red on every push. Decide the bump knowing that.
- The base image is pinned **by digest** so a rebuild cannot pick up a different JDK underneath.
  Checking for a newer digest is `docker buildx imagetools inspect …`.
- A pin that changes what an agent prints or how it exits changes the benchmark results, not just
  the image. Say so in the PR description.
