# Spec: Runner Image Consolidation & Determinism (v1)

**Status:** Implemented (T1–T9). T10 outstanding; image-build verification deferred.
**Author:** llm-benchmark contributors
**Date:** 2026-09-12
**Branch:** `feature/runner-image`, **stacked on `feature/embedded-exercises`** and checked out in a
separate `git worktree` (so the benchmark running on the base branch is untouched).

> There are currently **two** runner Dockerfiles, and the one every doc tells you to build
> (Alpine) is legacy *and cannot work*. External agent versions are unpinned, so rebuilding
> the image silently changes the agents under test. This spec consolidates to one image,
> pins its inputs, and defines **when the image version must be bumped** — with a local
> check that enforces it, since the repo has no CI.

---

## Objective

1. **Delete the legacy Alpine runner image** and every artifact that exists only to feed it.
2. **One Dockerfile** (Debian) is the single definition of the runner image.
3. **All external versions pinned in one place**, with tooling to re-pin to latest in one command.
4. **A runner image version** with a documented bump policy, made enforceable by a script.

The image stays a *separate* artifact from the binary: the host binary orchestrates and
mounts a workspace; the image provides the language toolchain **and** the agent CLIs.

## Non-goals

- **Not** coupling the Docker build to the Cargo build. `cargo build` and `build.sh
  docker-build` remain independent entry points. (Deliberately deferred.)
- **Not** making the runner offline-capable in this pass (pre-warmed Maven/npm/pip/Gradle
  caches). Noted as follow-up; it interacts with the version policy, because once caches are
  baked in, *adding exercises* also changes the image.
- **Not** pinning individual apt package versions. Flagged as a risk instead.

## Success Criteria

- [x] `docker/Dockerfile.runner` and `docker/gradle-8.7-bin.zip` do not exist.
- [x] `.gitignore` has no `gradle-8.7-bin.zip` line.
- [x] `docker/Dockerfile.runner.debian` is the only Dockerfile in the repo.
- [ ] `build.sh docker-build` succeeds from a clean checkout with **no untracked prerequisites**.
- [x] Every `npm install -g` / `uv tool install` in the Dockerfile takes its version from
      `docker/pins.env`; no bare unpinned installs remain.
- [x] `build.sh docker-verify` exits **non-zero** when a `docker/` input changes without a
      `RUNNER_VERSION` bump, and **zero** when inputs and version are in sync.
- [x] `docker/pin-agents.sh` re-resolves every pin in one command, prints a diff, and bumps
      the runner patch version.
- [ ] `docker run --rm ghcr.io/dylanschell/llm-benchmark-runner:<VERSION> cat /etc/llm-benchmark/runner-version`
      prints `<VERSION>`.
- [x] `docker/README.md` documents the bump policy table.
- [x] No doc references the Alpine `Dockerfile.runner`, and no doc shows a build context that
      cannot work. (`ROADMAP.md` §6.3 names it only to record that it was removed.)
- [x] No `claude-plugins` / `claude-code-transcripts` install remains.
- [ ] The dead `claude-archive` / `collect_claude_trace` code path is gone.
- [ ] A Java exercise runs to completion in the rebuilt image (validates the Gradle pre-seed).
- [ ] A `pi`-agent exercise runs to completion (validates the extension load paths).

**Outstanding:** the image build itself, the in-image version marker check, and both
reference-agent runs all need a real image build, so they are deferred until the benchmark run
occupying the Docker daemon finishes. The dead `claude-archive` path is T10 (see below) and
`crates/benchmark-core/src/agent/claude.rs` has deliberately not been touched yet.

## Current State (audit — verified, not inferred)

| Artifact | Observed state | Problem |
|---|---|---|
| `docker/Dockerfile.runner` (Alpine, 81 lines) | Legacy; only reachable via a context nobody documents | `COPY gradle-8.7-bin.zip` requires context `docker/`, but `README.md:22` uses context `.` → build fails. The zip is **never extracted** and never on `PATH`. Alpine's npm root is `/usr/local/lib/node_modules`, while `pi.rs` hardcodes `/usr/lib/node_modules` → **the pi extensions could never load**. |
| `docker/Dockerfile.runner.debian` (122 lines) | The one `build.sh` actually builds | Gradle dist hash hardcoded as `bhs2wmbdwecv87pi65oeuq5iu`; `ARG TARGETARCH` declared twice; `fd-find` listed twice; `touch …zip.lck` likely unnecessary. |
| `docker/gradle-8.7-bin.zip` | 134,184,980 B, untracked, ignored by `.gitignore:50` | 128 MiB that exists only for the dead Alpine `COPY`. |
| Agent CLIs | `npm install -g @anthropic-ai/claude-code`, `@earendil-works/pi-coding-agent`, `pi-caveman`, `@mrclrchtr/supi-bash-timeout` | All unpinned → an image rebuild silently changes the agents under test. |
| Test deps (jest/babel/eslint) | Pinned inline in the Dockerfile | Correct, but pinned in a second place, easy to miss on re-pin. |
| `claude-plugins`, `claude-code-transcripts` | Installed but invoked **nowhere** in the repo (see [Dead trace tooling](#dead-trace-tooling-verified)) | Dead. |
| Base image | `bellsoft/liberica-openjdk-debian:17` | Floating tag, no digest → not reproducible. |
| `.dockerignore` | Absent | The 128 MiB zip is uploaded as build context every build. |
| `docker/README.md` | Absent | ROADMAP §6.3 already asks for it. |
| CI / hooks | None (`.github` absent, no Makefile/justfile, only sample hooks) | Any policy must be enforced by a local script. |
| `crates/benchmark-core/src/agent/pi.rs` | Hardcodes `/usr/lib/node_modules` and depends on internal paths `@mrclrchtr/supi-bash-timeout/src/extension.ts` and `pi-caveman/extensions/caveman.ts` | This is a **binary↔image contract**, not just a dependency. |

### Verified hash derivation

The hardcoded Gradle cache directory name is exactly `base36(md5(distributionUrl))`:

```
url     : https://services.gradle.org/distributions/gradle-8.7-bin.zip
md5 hex : c225461986ad5741326ccd56e4273476
base36  : bhs2wmbdwecv87pi65oeuq5iu
hardcoded: bhs2wmbdwecv87pi65oeuq5iu   → MATCH
```

So the constant can be **computed** instead of guessed.

### Dead trace tooling (verified)

`uv tool install claude-code-transcripts` (Dockerfile lines ~99–100) has **no consumer**:

- The package name appears in the repo only on those two install lines — nothing invokes it.
- The only code that looks for Claude **HTML** transcripts is `collect_claude_trace`
  (`crates/benchmark-core/src/agent/claude.rs:86-113`), which reads
  `<temp_dir>/claude-archive/workspace/**/*page*.html`. `claude-archive` is referenced
  **nowhere else in the repository**, so that directory is never created and the function
  always returns `Ok(None)`.
- Its result is then **discarded** at line 249 (`let _trace = Self::collect_claude_trace(...)?;`),
  so even a successful read would have no effect.
- Claude's real trace is raw JSONL from `--output-format stream-json`, parsed directly by
  `benchmark-web/src/services/result_service.rs` (`calculate_claude_tokens`,
  `count_claude_turns_and_tools`) and by `benchmark-token-report`. The only HTML export in
  the codebase is `pi --export`, performed by the **pi** agent (`pi.rs:308`), not by this tool.

Conclusion: this is a leftover from an abandoned HTML-transcript design for the Claude agent.
Dropping it loses nothing automated. The dead `collect_claude_trace` path goes with it.

## Decisions

1. **Do not couple the Docker and Cargo builds** (deferred by the maintainer).
2. **Delete the Alpine image** and the zip/gitignore entries that exist only for it.
   Debian is the single definition.
3. **Pin agent/tool versions**, and ship **re-pin tooling** so pinning is not a chore.
4. **Version the runner image** and document exactly when the version must be bumped.
5. **Drop `claude-plugins`, the commented ralph-wiggum line, and `claude-code-transcripts`** —
   none are invoked by any code path (see [Dead trace tooling](#dead-trace-tooling-verified)),
   and remove the dead `collect_claude_trace` reader that the last of them presumably served.
6. **Tag `:${VERSION}` *and* `:latest`** — `:latest` documented as a dev-only convenience;
   `config.yaml` references it today, so a version-only scheme would break on day one.
7. **Re-pinning auto-bumps the runner patch version** (with `--minor`/`--major`/`--dry-run`),
   because by the policy above a pin change *is* an image change.
8. **Digest-pin the base image**, and treat a digest change as a version-bumping input.
9. **Branch strategy:** stack on `feature/embedded-exercises` rather than branching off `main`,
   to avoid conflicts in the five overlapping files (`README.md`, `ROADMAP.md`, `CHANGELOG.md`,
   `docs/DEVELOPER.md`, `crates/benchmark-core/src/agent/claude.rs`). `main` has no commits
   absent from the base branch, so the base is a clean fast-forward and stacking costs nothing.
10. **Install pi from its standalone bun binary rather than npm.** The pi npm package requires
    Node ≥22.19.0 and calls `fs.globSync`, but the image pins Node 20 for the Exercism JS track,
    so a plain re-pin broke `pi --version` outright. Freezing pi at 0.74.2 (the last Node-20
    release) would only defer the problem. The standalone binary embeds its runtime and is
    published for both linux architectures, so the agent's runtime stops dictating the Node
    version the *exercises* are tested against — which is the more important pin of the two.
11. **Make the packaged agent set an explicit, hashed build input.** Claude Code is excluded by
    default: Anthropic licenses it "all rights reserved" with no redistribution grant, which
    makes any image containing it unpublishable, and the maintainer mostly benchmarks pi. The
    switch lives in `docker/agents.env` rather than in a `--build-arg` so that
    `docker_input_hash` covers it — two images built from the same inputs must contain the same
    agents, so flipping the variant requires a version bump like any other image change.
12. **The image is built under its published name, and publishing is a separate verb.** The build
    tags `ghcr.io/dylanschell/llm-benchmark-runner:${RUNNER_VERSION}` and `:latest`, and
    `config.yaml` plus `DockerConfig::default_image()` default to that same name — so the artifact
    you run and the artifact you publish are the same string, which a local-only name could not
    guarantee. `docker-push` is deliberately not folded into the build: it re-runs `docker-verify`
    first, and refuses to publish an image containing Claude Code, because Anthropic grants no
    redistribution right.
13. **Builds disable buildx provenance, so a published tag is reproducible.** `--provenance=false`
    makes the build emit a plain OCI manifest instead of an index wrapping a provenance attestation
    whose payload embeds the build time. Without it, every rebuild produced a new index digest while
    the image config and all 19 layers stayed identical — so re-pushing a versioned tag silently
    changed it. This is a build-tooling change that does not alter image content, so it needs no
    `RUNNER_VERSION` bump under the policy above.

## Design

### 1. Single Dockerfile

`docker/Dockerfile.runner.debian` becomes the only definition. Rename is *not* required
(avoids churn), but all docs must point at it. Fix during the edit:

- remove the duplicate `ARG TARGETARCH`
- remove the duplicate `fd-find`
- remove `RUN npm install -g claude-plugins` and the commented ralph line
- drop `touch …zip.lck` (the wrapper manages its own lock file)
- pin the base image to a digest — `FROM bellsoft/liberica-openjdk-debian:17@sha256:<digest>` —
  with the digest recorded in `pins.env`; changing it is a version-bumping input (Decision 8)

### 2. Gradle pre-seed without the magic constant

Compute the dist directory name from the distribution URL, and verify the download:

```dockerfile
ARG GRADLE_VERSION=8.7
ARG GRADLE_DIST_URL=https://services.gradle.org/distributions/gradle-${GRADLE_VERSION}-bin.zip

RUN set -eux; \
    hash="$(python3 /usr/local/lib/llm-benchmark/gradle-dist-hash.py "${GRADLE_DIST_URL}")"; \
    dir="/home/runner/.gradle/wrapper/dists/gradle-${GRADLE_VERSION}-bin/${hash}"; \
    mkdir -p "$dir"; \
    curl -fsSL "${GRADLE_DIST_URL}" -o /tmp/gradle-bin.zip; \
    echo "${GRADLE_SHA256}  /tmp/gradle-bin.zip" | sha256sum -c -; \
    unzip -q /tmp/gradle-bin.zip -d "$dir"; \
    touch "$dir/gradle-${GRADLE_VERSION}-bin.zip.ok"; \
    rm /tmp/gradle-bin.zip; \
    chown -R runner:runner /home/runner/.gradle
```

`docker/gradle-dist-hash.py` (~10 lines) implements `md5` → base36. It is the only new
file needed, and it removes a silent-failure coupling: today, if the Exercism Java track
bumps the wrapper, the pre-seed silently stops matching and `./gradlew` downloads at test
time — exactly the class of drift the pinned SHAs were chosen to avoid.

The exercise wrapper properties currently read
`distributionUrl=https\://services.gradle.org/distributions/gradle-8.7-bin.zip`, so the
computed value matches today's constant.

### 3. Pins: `docker/pins.env`

Single source of truth, shell-sourceable (`KEY=value`, no logic):

```
CLAUDE_CODE_VERSION=...
PI_CODING_AGENT_VERSION=...
PI_CAVEMAN_VERSION=...
SUPI_BASH_TIMEOUT_VERSION=...
JEST_VERSION=...
BABEL_CORE_VERSION=...
BABEL_PRESET_ENV_VERSION=...
EXERCISM_BABEL_PRESET_VERSION=...
EXERCISM_ESLINT_CONFIG_VERSION=...
TYPES_JEST_VERSION=...
TYPES_NODE_VERSION=...
BABEL_JEST_VERSION=...
CORE_JS_VERSION=...
ESLINT_VERSION=...
GRADLE_VERSION=...
GO_VERSION=...
FD_VERSION=...
```

`build.sh docker-build` sources this file and passes one `--build-arg` per key. The
Dockerfile declares `ARG X` **without defaults**, so a bare `docker build` fails loudly
rather than silently building an unpinned image. `build.sh docker-build` is the only
supported build path (documented).

### 4. Version: `docker/RUNNER_VERSION` + `docker/runner.lock`

- `docker/RUNNER_VERSION` — a semver (`1.0.0`). Human-owned.
- `docker/runner.lock` — machine-generated: `{ version, inputHash }`, where `inputHash` is a
  sha256 over the sorted `(path, sha256)` of every file under `docker/` (excluding
  `runner.lock` itself).

The framing that makes the policy answerable: **the runner version versions the `docker/`
inputs, not the binary.**

### 5. `build.sh` verbs

| Command | Behaviour |
|---|---|
| `build.sh docker-build [--tag T] [--arch A]` | Sources `pins.env`, passes build args, builds, tags `:${RUNNER_VERSION}` **and** `:latest`, rewrites `runner.lock`. |
| `build.sh docker-verify` | Recomputes `inputHash`. Fails if inputs changed while `RUNNER_VERSION` is unchanged; fails if the lock's version ≠ `RUNNER_VERSION` (i.e. built-but-not-recorded). Also runs the pin lint. |
| `build.sh docker-repin [--minor\|--major]` | Convenience wrapper for `docker/pin-agents.sh`. |

`inputHash` uses `shasum -a 256` with a `sha256sum` fallback (the script runs on the host,
i.e. macOS today).

### 6. Tags and introspection

Tag `:${RUNNER_VERSION}` as canonical, plus `:latest` as a documented convenience alias so
nothing breaks immediately. Bake the version into the image so a run can be traced back to
its environment:

```dockerfile
ARG RUNNER_VERSION
LABEL org.opencontainers.image.version="${RUNNER_VERSION}"
RUN mkdir -p /etc/llm-benchmark && echo "${RUNNER_VERSION}" > /etc/llm-benchmark/runner-version
```

(placed before `USER runner`, so it writes as root).

## Version Bump Policy

Bump when anything that could change benchmark results **without** changing the agent under
test changes:

| Bump | Don't bump |
|---|---|
| Base image tag/digest | Rust binary refactors, reporter/web changes |
| System packages / toolchain versions (JDK, Go, Node, Rust, clang, cmake, boost, maven, Gradle) | Adding/removing **exercise content** (already embedded in the binary) |
| **Any pinned agent version** (claude-code, pi-coding-agent, pi-caveman, supi-bash-timeout) | Test-fixture or docs changes |
| Test-dependency pins (jest/babel/eslint) | |
| Binary↔image **contract**: `/workspace` mount, `.claude`/`.pi` volumes, non-root uid, npm global root `/usr/lib/node_modules`, expected binaries (`pi`, `claude`, `uv`), Gradle pre-seed | |
| **Adding a language** to the manifest (image must gain that toolchain) | |

Two easy-to-miss entries: **adding a language is both a binary and an image change**, and
**any change to the npm global root breaks `pi.rs`'s hardcoded extension paths**.

## Re-pin Tooling — `docker/pin-agents.sh`

- **Default:** resolve `latest` for each npm package (`npm view <pkg> version`) and for the
  `uv` tool, rewrite `pins.env`, print a before/after diff, then **bump `RUNNER_VERSION`
  patch** and refresh `runner.lock` — because per the policy above, a pin change *is* an
  image change. `--minor` / `--major` for deliberate jumps, `--dry-run` to preview.
- **`--check`:** lint — fail if the Dockerfile installs any package without a version
  placeholder, or if `pins.env` and the Dockerfile disagree.
- **`--verify-fresh` (optional):** report which pins are behind `latest` without changing
  anything.

## Removal List (exact)

- `docker/Dockerfile.runner` — delete.
- `docker/gradle-8.7-bin.zip` — delete (134,184,980 B, untracked).
- `.gitignore:50` (`/docker/gradle-8.7-bin.zip`) — delete.
- `docker/Dockerfile.runner.debian:55` (`RUN npm install -g claude-plugins`) — delete.
- `docker/Dockerfile.runner.debian:56` (commented ralph-wiggum line) — delete.
- `docker/Dockerfile.runner.debian:99-100` (comment + `RUN uv tool install claude-code-transcripts`) — delete.
- `crates/benchmark-core/src/agent/claude.rs:86-113` (`collect_claude_trace`) and the discarded
  `let _trace = …` call at line 249 — delete (dead code, see above).
- Duplicate `ARG TARGETARCH`; duplicate `fd-find`; `touch …zip.lck` — delete.
- Doc references to fix: `README.md:22,128`; `docs/DEVELOPER.md:58,77,231,241,413`;
  `ROADMAP.md` §6.3/§6.4.

## Commands

```bash
# Build the runner image (only supported path; passes pin build-args)
./build.sh docker-build
./build.sh docker-build --arch linux/arm64 --tag ghcr.io/dylanschell/llm-benchmark-runner:1.0.0

# Policy guard — fails if docker/ inputs changed without a version bump
./build.sh docker-verify

# Re-pin every agent/tool to latest, bump patch, refresh lock
./build.sh docker-repin
./build.sh docker-repin --minor --dry-run

# Introspect a built image
docker run --rm ghcr.io/dylanschell/llm-benchmark-runner:1.0.0 cat /etc/llm-benchmark/runner-version

# Unchanged
cargo build --release
cargo test --workspace --lib --bins
```

## Project Structure

```
docker/
  Dockerfile.runner.debian   # the only runner image definition
  pins.env                   # single source of truth for external versions
  RUNNER_VERSION             # human-owned semver
  runner.lock                # generated: { version, inputHash }
  pin-agents.sh              # re-pin to latest (+ --check)
  gradle-dist-hash.py        # computes base36(md5(distributionUrl))
  README.md                  # build instructions + bump policy table
docs/specs/
  runner-image.md            # this spec
```

## Testing Strategy

- **Policy guard:** `build.sh docker-verify` — exercised by deliberately editing a file
  under `docker/` and confirming it fails, then bumping and confirming it passes.
- **Pin lint:** `docker/pin-agents.sh --check` — exercised by temporarily removing a
  `@${…_VERSION}` suffix and confirming it fails.
- **Hash derivation:** unit-check `gradle-dist-hash.py` against the known value
  `bhs2wmbdwecv87pi65oeuq5iu` for the 8.7 URL.
- **Image smoke test:** `docker run --rm <img> cat /etc/llm-benchmark/runner-version`,
  plus a Java exercise through the reference agent (validates the Gradle pre-seed) and one
  `pi` exercise (validates extension loading).
- **Regression:** `cargo test --workspace --lib --bins` stays green (no Rust changes expected).

## Boundaries

- **Always:** run `build.sh docker-verify` before committing anything under `docker/`;
  bump `RUNNER_VERSION` when the guard says so; keep pins in `pins.env` only.
- **Ask first:** changing the tag scheme; pinning the base image to a digest (it makes base
  updates a manual, version-bumping chore); any change to `pi.rs`'s hardcoded npm root.
- **Never:** commit the Gradle zip or any other large third-party binary; reintroduce a
  second Dockerfile; add an unpinned `npm install -g`; silently change a pin without a
  version bump.

## Risks & Mitigations

| Risk | Mitigation |
|---|---|
| Removing the Alpine image breaks someone's local workflow | It cannot work (wrong npm root, unextractable zip); docs never matched it anyway. Delete and document. |
| `:latest` keeps being used, defeating versioning | Keep the tag working, but mark it dev-only in `docker/README.md`; `docker-verify` records the version that produced the image. |
| Derived Gradle hash diverges from a future wrapper | The hash is computed from the URL, so it self-corrects. Adding a check that `pins.env`'s `GRADLE_VERSION` matches the wrapper URL in the bundle is a follow-up if desired. |
| Unpinned apt packages / base tag still drift | Digest-pin the base (Ask-first); apt pinning deferred and documented as a known gap. |
| Auto-bump on re-pin is too magic | `--dry-run` and an explicit diff; bump is patch-only by default. |

## Open Questions

None outstanding — all four review questions are resolved (Decisions 6–9).

## Plan

1. **Delete the Alpine path** (Dockerfile, zip, gitignore line, doc refs). Independently
   landable and behaviour-neutral.
2. **Fix the Debian Dockerfile** (duplicates, dead `claude-plugins`/ralph, derived Gradle
   hash + checksum, version label/marker).
3. **Introduce `pins.env`**, parameterise every install, and add `pin-agents.sh`.
4. **Introduce `RUNNER_VERSION` + `runner.lock`**, the `build.sh` verbs, and the guard.
5. **Write `docker/README.md`** (build, contents, bump policy table).
6. **Verify**: guard negative/positive, hash unit check, image smoke test, one Java and one
   `pi` exercise end-to-end.

Steps 1–2 are independent; 3 depends on 2; 4 depends on 3; 5 depends on 4; 6 is last.

## Tasks

- [x] **T1 — Remove the legacy Alpine image**
  - Acceptance: `docker/Dockerfile.runner` gone; no doc references it; `build.sh docker-build` unaffected.
  - Verify: `rg -n 'Dockerfile.runner\b' --glob '!Dockerfile.runner.debian' .` returns nothing but the debian name.
  - Files: `docker/Dockerfile.runner` (delete), `README.md`, `docs/DEVELOPER.md`, `ROADMAP.md`.

- [x] **T2 — Drop the Gradle zip + gitignore line**
  - Acceptance: zip and `.gitignore:50` gone; repo clean.
  - Verify: `git status --short` clean; `ls docker/` has no zip.
  - Files: `docker/gradle-8.7-bin.zip`, `.gitignore`.

- [x] **T3 — Clean the Debian Dockerfile**
  - Acceptance: no duplicate `ARG TARGETARCH`/`fd-find`; `claude-plugins`, the ralph line, and
    `claude-code-transcripts` gone; `.lck` touch gone.
  - Verify: `rg -n 'claude-plugins|ralph|zip.lck|transcripts' docker/Dockerfile.runner.debian` empty.
  - Files: `docker/Dockerfile.runner.debian`.

- [x] **T4 — Derive the Gradle dist hash**
  - Acceptance: hash computed at build time; download checksummed; no hardcoded hash.
  - Verify: `gradle-dist-hash.py` returns `bhs2wmbdwecv87pi65oeuq5iu` for the 8.7 URL; a Java exercise passes end-to-end.
  - Files: `docker/gradle-dist-hash.py` (new), `docker/Dockerfile.runner.debian`.

- [x] **T5 — Add `pins.env` and parameterise installs**
  - Acceptance: every `npm install -g` / `uv tool install` uses a `${…_VERSION}` arg; bare `docker build` fails.
  - Verify: `rg -n 'npm install -g' docker/Dockerfile.runner.debian` shows a `@${` on every package.
  - Files: `docker/pins.env` (new), `docker/Dockerfile.runner.debian`, `build.sh`.

- [x] **T6 — Add `pin-agents.sh`**
  - Acceptance: one command re-pins all, prints a diff, bumps patch; `--check` lints.
  - Verify: `./docker/pin-agents.sh --dry-run` lists current→latest; `--check` passes on a clean tree and fails on a de-pinned edit.
  - Files: `docker/pin-agents.sh` (new).

- [x] **T7 — Add `RUNNER_VERSION`, `runner.lock`, and the guard**
  - Acceptance: `docker-build` tags `:${VERSION}` + `:latest` and writes the lock; `docker-verify` fails on an unversioned `docker/` change.
  - Verify: edit a docker file → `docker-verify` fails; bump version → passes.
  - Files: `docker/RUNNER_VERSION` (new), `docker/runner.lock` (generated), `build.sh`.

- [x] **T8 — Bake the version into the image** (landed with T7 — the Dockerfile `ARG` and the `build.sh` that supplies it have to move together)
  - Acceptance: label + `/etc/llm-benchmark/runner-version` present.
  - Verify: `docker run --rm ghcr.io/dylanschell/llm-benchmark-runner:<VERSION> cat /etc/llm-benchmark/runner-version`.
  - Files: `docker/Dockerfile.runner.debian`.

- [x] **T9 — Write `docker/README.md`**
  - Acceptance: build instructions, image contents, and the bump-policy table.
  - Verify: read-through; links resolve.
  - Files: `docker/README.md` (new), `ROADMAP.md` (§6.3/§6.4 resolved).

- [x] **T10 — Remove the dead Claude HTML-trace path**
  - Acceptance: `collect_claude_trace` and its discarded call are gone; no `claude-archive` reference remains.
  - Verify: `rg -n 'claude-archive|collect_claude_trace' crates/` empty; `cargo test --workspace --lib --bins` green.
  - Files: `crates/benchmark-core/src/agent/claude.rs`.

- [x] **T11 — Install pi from its standalone binary** (not in the original plan; surfaced while
  verifying T9, when the re-pinned pi 0.85.1 crashed on Node 20)
  - Acceptance: pi runs in the image while Node stays at 20; the linux asset is chosen from
    `TARGETARCH` and checksum-verified before extraction; `pin-agents.sh` tracks and refreshes
    both checksums so re-pinning stays a single command.
  - Verify: in the built image with `node` absent, `pi --version` prints the pinned version and
    both `--extension` paths load (`extension_load_errors=0`, versus `1` for a bogus path).
  - Files: `docker/Dockerfile.runner.debian`, `docker/pins.env`, `docker/pin-agents.sh`, `build.sh`.

- [x] **T12 — Make Claude Code packaging optional** (so that a publishable image exists at all)
  - Acceptance: `INSTALL_CLAUDE` in `docker/agents.env` selects the agent set; the image records
    its agents in an OCI label and `/etc/llm-benchmark/agents`; flipping the switch is caught by
    the input-hash guard; `pin-agents.sh` does not re-pin an agent that is not packaged.
  - Verify: a default build reports `agents=pi`, has no `claude` on `PATH`, and still loads both
    pi extensions; `docker-verify` reports `v1.2.0 [pi]`.
  - Files: `docker/agents.env` (new), `docker/Dockerfile.runner.debian`, `build.sh`,
    `docker/pin-agents.sh`, `docker/README.md`.

- [x] **T13 — Build under the published image name; make pushing an explicit verb**
  - Acceptance: a default build tags `ghcr.io/dylanschell/llm-benchmark-runner:{<version>,latest}`;
    `config.yaml`, `config.example.yaml` and `DockerConfig::default_image()` agree with it; the
    repository is overridable with `--image`; `docker-push` is opt-in, verifies the inputs first,
    and refuses to publish an image containing Claude Code.
  - Verify: `./build.sh docker-verify` is unaffected (`build.sh` is not a hashed input, so no
    `RUNNER_VERSION` bump is required); `docker-push` exits 1 with a licensing explanation when
    `INSTALL_CLAUDE=1`, and with "not present locally" for a repository that has no image;
    two consecutive `docker-build` runs from unchanged inputs report the same image digest.
  - Files: `build.sh`, `crates/benchmark-types/src/config/mod.rs`,
    `crates/benchmark-core/src/agent/pi.rs`, `config.yaml`, `config.example.yaml`,
    `docker/README.md`.
