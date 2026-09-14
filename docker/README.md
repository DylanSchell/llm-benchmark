# Runner image

The container each exercise is benchmarked inside. It supplies the language toolchains and the
agent CLIs under test. The `llm-benchmark` binary runs on the *host*, materialises an exercise
into a temporary directory, and bind-mounts that at `/workspace`.

The image is a separate artifact from the binary and is versioned separately — see
[Versioning](#versioning).

- Design and rationale: [`docs/specs/runner-image.md`](../docs/specs/runner-image.md)
- Dockerfile: `Dockerfile.runner.debian`
- Version pins: `pins.env`

## Files

| File | Purpose |
| --- | --- |
| `Dockerfile.runner.debian` | The single runner image definition. |
| `pins.env` | Every externally-sourced version. Single source of truth. |
| `agents.env` | Which optional agents the image packages. A hashed build input. |
| `pin-agents.sh` | Re-pins the npm packages and lints that nothing is unpinned. |
| `gradle-dist-hash.py` | Derives the Gradle wrapper cache directory name. |
| `runner.lock` | Generated. Records the version and input hash of the last build. |

The image version is deliberately **not** a file in here — it is the project version in `Cargo.toml`,
because the binary embeds that one at compile time. See [Versioning](#versioning).

## Building

```bash
./build.sh docker-build                            # host platform
./build.sh docker-build --arch linux/arm64         # explicit platform
./build.sh docker-build --arch linux/amd64,linux/arm64   # both, as an OCI index
./build.sh docker-build --tag my/runner:test       # primary tag override
./build.sh docker-build --image ghcr.io/you/fork   # different repository
```

Every build tags the image twice: `<image>:<version>` and `<image>:latest`, where `<version>` is the
project version from `Cargo.toml` and `<image>`
defaults to `ghcr.io/dylanschell/llm-benchmark-runner`. `:latest` is the development convenience
that `config.yaml` expects; use the version tag when you need to record exactly which image
produced a result.

The default name is the *published* name, and `config.yaml`,
`config.example.yaml` and `DockerConfig::default_image()` all default to it, so the image you run
and the image you publish are the same string. Under a local-only name the two could silently
diverge.

Always build through `build.sh`. The Dockerfile's build args deliberately have **no defaults**,
so a bare `docker build` fails loudly instead of quietly producing an unpinned image.

A build is slow by design — it downloads the JDK, Go, Node, Rust, Gradle and the agent CLIs.
The result is cached by layer, so a re-pin usually only rebuilds the layers below it.

### Multi-platform images

`--arch` takes a comma-separated list, so one build can cover both architectures:

```bash
./build.sh docker-build --arch linux/amd64,linux/arm64
```

The result is an OCI **index** in the local image store, and a plain `docker push` publishes every
platform in it — no per-arch staging tags, no separate `buildx --push` path. `docker-push` reports
the platform set it is publishing, and warns when a push would narrow an already multi-platform tag
back to a single platform.

This works only because Docker Desktop stores images with the **containerd** snapshotter — `docker
info` reports driver `overlayfs`, not `overlay2`. With the classic image store a multi-platform
build cannot be loaded locally at all.

Two consequences worth knowing:

- **amd64 layers are built under QEMU emulation** on an arm64 host. The Java- and Node-heavy steps
  — Gradle dependency priming, `npm install` — are far slower than the native build, so building
  both platforms costs considerably more than either alone.
- The base image is pinned by **manifest-list** digest, so that single pin serves both
  architectures, and `JAVA_HOME`/`PATH` come from that image rather than being hardcoded to a
  per-arch path.

## Publishing

**Building never contacts a registry.** Publishing is the separate `docker-push` verb, because
sending several GB to a registry — or publishing an image we have no right to distribute — is not
a build's decision to make:

```bash
./build.sh docker-push
```

It pushes `<image>:<version>` and then `<image>:latest`, and refuses to run unless:

- `docker-verify` passes, so the tag cannot describe inputs that have since changed;
- the image is present locally, so it can never push a stale or unintended one;
- `INSTALL_CLAUDE=0`. Claude Code carries no redistribution right, so an image containing it must
  not be published anywhere — see [Licensing](#licensing). Override deliberately with
  `PUSH_UNLICENSED=1`.

Authenticate first:

```bash
docker login ghcr.io
```

What it publishes is **single-platform**: whichever architecture you built. Shipping both
`linux/amd64` and `linux/arm64` means building and pushing each, or moving to a
`buildx --platform linux/amd64,linux/arm64 --push` build — which cannot also load into the local
image store, so it is a different workflow.

A published package is **private by default**. Container registry storage and bandwidth are
currently free, so visibility is a sharing decision rather than a cost one; change it under
*Package settings → Change visibility* if you want anonymous pulls.

## Versioning

One number identifies the project, and it lives in `Cargo.toml`:

- `llm-benchmark --version` prints it, through clap's `CARGO_PKG_VERSION`;
- `build.sh` tags the image `<image>:<version>`; and
- GitHub Releases are tagged `v<version>`, and the release workflow rejects a tag that disagrees.

`Cargo.toml` is the source of truth because it is the only place Cargo reads a version from at
compile time — the binary embeds it, so nothing else could feed `--version`. A separate
`docker/RUNNER_VERSION` could only ever mirror it, and a mirror is one more thing to forget, so that
file was retired. The consequence is that the image and the binary now move together: a re-pin bumps
the version the binary reports too. That is the intended trade — the two artifacts are consumed as a
pair and share a contract, so one number for both is easier to reason about than a compatibility
table.

`docker/runner.lock` records that version and a SHA-256 over the image's build inputs — every file
under `docker/` except `runner.lock` and documentation — at the last successful build. Because the
hash is a pure function of the inputs, it also identifies the commit an image was built from.

Check it with:

```bash
./build.sh docker-verify
```

which fails when either:

- a `docker/` input changed and `runner.lock` has not been refreshed by a rebuild, or
- the version recorded in `runner.lock` no longer matches `Cargo.toml`.

### Image digest reproducibility

`--provenance=false` keeps the *index* deterministic: given the same layer bytes, the same `docker/`
inputs produce the same manifest and therefore the same digest on every build. This is why the build
passes it.

It does **not** make the layers themselves reproducible, so it does not make a versioned tag immutable.
A rebuild that re-executes an install step (apt, npm, curl) produces different layer bytes — those
tools are not deterministic — and the digest changes with them. Layers are byte-identical only when
BuildKit serves them from its cache. Two rebuilds back to back on a warm cache do come out identical
(verified), but a rebuild after other work may not: the 1.3.2 → 1.4.0 rebuild, with `inputHash`
unchanged, shared only 5 of its 19 layers with the image it replaced.

What `--provenance=false` fixes is a different failure. By default buildx wraps the image in an OCI
*index* carrying a provenance attestation whose payload embeds the build timestamp. That left the image
config and all 19 layers identical between rebuilds while the index digest changed every time — so
re-pushing a versioned tag silently rewrote it to a new digest, breaking anyone pinning by digest.

So: a published tag is stable as long as it is not rebuilt, and `runner.lock`'s `inputHash` is what
tells you whether the inputs actually changed. Digest-pinning an image across a rebuild is not safe.

---

### Bump policy

Bump the version in `Cargo.toml` for:

| Change | Example |
| --- | --- |
| Dockerfile edit | adding a package, reordering layers |
| Agent CLI re-pin | `CLAUDE_CODE_VERSION`, `PI_*`, `SUPI_*` |
| Agent set change | flipping `INSTALL_CLAUDE` in `docker/agents.env` |
| Test-dependency re-pin | `JEST_VERSION`, `EXERCISM_*` |
| Toolchain bump | Go, Rust, uv, fd, Gradle, Node major |
| Base image digest change | new `bellsoft/liberica-openjdk-debian` digest |
| Binary↔image contract change | mount path, npm global root, extension paths |
| Adding or removing a language runtime | a new track |

Do **not** bump for Rust-only refactors, embedded exercise content, or docs and fixtures. The
exercises are compiled into the binary, so they are not part of the image at all.

Use `pin-agents.sh --minor` or `--major` when the change warrants it; a plain re-pin bumps the
patch component.

## Pins

`pins.env` is shell-sourceable `KEY=value` and holds everything fetched from outside this
repository: the base image digest, toolchain versions, the Gradle checksum, the agent CLIs and
the JavaScript test dependencies.

The base image is pinned by **manifest-list digest**, so the same digest is valid for both
`linux/amd64` and `linux/arm64` and a rebuild cannot silently pick up a different JDK.

### Re-pinning

```bash
docker/pin-agents.sh --dry-run    # preview
docker/pin-agents.sh              # apply, bump runner patch version, refresh the lock
docker/pin-agents.sh --minor      # ...bumping the minor component instead
```

A change that crosses a major version is reported and skipped unless `--allow-major` is given.
That guard exists because those are the changes that silently alter benchmark results rather
than failing: `@babel/core` 7→8, `babel-jest`/`@types/jest` 29→30, `eslint` 8→10 and
`@types/node` 20→22 are all currently held back for this reason.

pi is handled alongside the npm pins: bumping its version also refreshes `PI_SHA256_X64` and
`PI_SHA256_ARM64` from the release's `SHA256SUMS`.

Toolchain pins (Go, Rust, uv, fd, Gradle, base image) are **not** re-pinned automatically —
bump those by hand, and refresh `GRADLE_SHA256` alongside `GRADLE_VERSION`:

```bash
curl -fsSL "https://services.gradle.org/distributions/gradle-8.7-bin.zip.sha256"
```

### Linting

```bash
docker/pin-agents.sh --check
```

Offline. Asserts that every `npm install -g` in the Dockerfile is pinned to a variable which is
both declared as a build `ARG` and defined in `pins.env`, and that no pin is unused. It
tokenises the install blocks rather than matching quoted strings, so an unquoted package is
caught too. `docker-verify` runs this as well.

## Agents

The agents are what the benchmark measures, so their versions are the most consequential pins.
They are delivered differently, and the difference matters:

**pi — standalone binary.** Installed from the official bun-compiled release asset
(`pi-linux-x64.tar.gz` / `pi-linux-arm64.tar.gz`, selected with buildx's `TARGETARCH` and verified
against the release's `SHA256SUMS`), then extracted to `/opt/pi`, which is on `PATH`.

The pi *npm* package requires Node ≥22.19.0 and calls `fs.globSync`, while this image pins Node 20
for the Exercism JS track. Installing the standalone binary decouples the two: the bun runtime is
embedded, so pi runs with no Node present at all (verified). Do **not** move pi back to the npm
package without also moving Node to 22.

The archive vendors `photon_rs_bg.wasm`, `assets/`, `theme/`, `export-html/` and `node_modules/`
alongside the executable. That is why it is extracted with `--strip-components=1` into `/opt/pi`
rather than flattened into `/usr/local/bin` — flattening breaks asset resolution.

**claude — native binary via npm, and optional.** `npm install -g @anthropic-ai/claude-code`
pulls a platform package from `optionalDependencies` (`claude-code-linux-arm64`, `…-linux-x64`,
the musl variants, and so on) whose payload is a pre-compiled ELF; npm selects the one matching
the container's architecture. Its `engines` field asks for Node ≥22, but that governs the
install-time wrapper scripts rather than the runtime — the binary itself runs without Node.

It is **not packaged by default**, for licensing reasons (see [Licensing](#licensing)).

### Which agents are packaged

`agents.env` selects them:

| Setting | Image | Publishable |
| --- | --- | --- |
| `INSTALL_CLAUDE=0` (default) | pi only | **yes** |
| `INSTALL_CLAUDE=1` | pi + claude | **no** — see [Licensing](#licensing) |

Because `agents.env` lives under `docker/`, it is part of `docker_input_hash`: changing it moves the
hash, so two images built from the same inputs are guaranteed to contain the same agents. A plain
`--build-arg` could not promise that, which is why the switch is a file.

The choice is recorded twice in the built image — the `com.llm-benchmark.agents` OCI label and
`/etc/llm-benchmark/agents` — so a running container can be identified without inspecting build
history:

```bash
docker image inspect ghcr.io/dylanschell/llm-benchmark-runner:1.2.0 \
  --format '{{index .Config.Labels "com.llm-benchmark.agents"}}'
```

`pin-agents.sh` skips re-pinning an agent that is not packaged, so a pin for content that never
reaches the image cannot force a version bump.

Running a **claude** benchmark against the default image will fail: `claude` is not on `PATH`.
Rebuild with `INSTALL_CLAUDE=1` for local use only.

## Licensing

What this image may and may not be redistributed as.

**The default (pi-only) image is publishable.** Every component permits redistribution provided
notices are retained — and they are:

| Component | Licence |
| --- | --- |
| pi, pi-caveman, supi-bash-timeout | MIT |
| Go | BSD-3-Clause, plus `PATENTS` |
| Gradle, Maven | Apache-2.0 |
| Node.js | MIT |
| Python | PSF |
| Liberica JDK 17 | GPLv2+CE — carries source-offer obligations; BellSoft publishes the source |
| Debian bookworm | per-package, see `/usr/share/doc/*/copyright` |

**Claude Code cannot be redistributed.** `@anthropic-ai/claude-code` ships a one-line licence:

> © Anthropic PBC. All rights reserved. Use is subject to Anthropic's Commercial Terms of Service.

"All rights reserved" grants no redistribution right, so an image containing it must not be
pushed to a registry, public or private. That is why it is off by default.

The Exercism exercise content is **not** in this image at all — it is compiled into the Rust
binary and materialized into the workspace at runtime. Its attribution lives in the repository
root `THIRD_PARTY_NOTICES`.

## Gradle

The Java track's exercises use the Gradle wrapper, which normally downloads its distribution on
first run — not something you want happening mid-benchmark. The image therefore pre-seeds the
wrapper cache.

Gradle names that cache directory `base36(md5(distributionUrl))`, so the path depends on the
wrapper's `distributionUrl`, which the Exercism Java track could change at any time. Rather
than hardcode the value, `gradle-dist-hash.py` computes it during the build and the zip is
verified against `GRADLE_SHA256` before extraction. If the track bumps its wrapper, the build
simply seeds the new location.

## Binary ↔ image contract

The binary and the image must agree on a few things. Changing any of these is a breaking change
and needs a version bump:

- the workspace is mounted at `/workspace`;
- the npm global root is `/usr/lib/node_modules` (hardcoded in `crates/benchmark-core/src/agent/pi.rs`);
- the pi extensions `pi-caveman` (`extensions/caveman.ts`) and `@mrclrchtr/supi-bash-timeout`
  (`src/extension.ts`) exist at those paths within their packages;
- `claude`, `pi`, `go`, `cargo`, `node`, `python3` and `mvn` are on `PATH`. `pi` is `/opt/pi/pi`
  and resolves its vendored assets relative to `/opt/pi`, so that directory has to stay on `PATH`.
  `gradle` is deliberately *not* installed — Java test runs use `mvn` or the exercise's own
  `./gradlew`, and only the wrapper's distribution cache is pre-seeded (see [Gradle](#gradle)).

A running container reports its version:

```bash
docker run --rm ghcr.io/dylanschell/llm-benchmark-runner:latest cat /etc/llm-benchmark/runner-version
docker image inspect ghcr.io/dylanschell/llm-benchmark-runner:latest \
  --format '{{ index .Config.Labels "org.opencontainers.image.version" }}'
```

## Troubleshooting

**`docker build` fails with an empty build arg.** The Dockerfile has no defaults by design.
Build with `./build.sh docker-build`.

**`docker-verify` reports the inputs changed.** You edited something under `docker/`. If the edit
changes image content, bump the version in `Cargo.toml`; either way run `./build.sh docker-build` to
rebuild and refresh `runner.lock`.

**`docker-verify` reports the lock is stale.** The version in `Cargo.toml` no longer matches
`runner.lock`. Run `./build.sh docker-build`.

**A Java exercise tries to download Gradle.** The wrapper's `distributionUrl` no longer matches
what the image seeded. Check `GRADLE_VERSION`, and let the image rebuild the cache.

**pi extensions fail to load.** The npm global root or a package's internal layout moved. See
[Binary ↔ image contract](#binary--image-contract). The extensions are still npm-installed even
though pi itself is not — they are plain TypeScript packages that the binary loads by path.

**The build fails with `computed checksum did NOT match` for `pi.tar.gz`.** `PI_CODING_AGENT_VERSION`
was changed without updating the checksums. Run `docker/pin-agents.sh` to refresh both from the
release's `SHA256SUMS`.

**`claude` is not found inside a container.** The default image is pi-only. Set
`INSTALL_CLAUDE=1` in `docker/agents.env`, rebuild, and use the result locally — it is not
redistributable (see [Licensing](#licensing)).

**`docker-push` fails with `permission_denied` or "token provided does not match expected
scopes".** The registry rejected the credential, not the image. Run `docker login ghcr.io`. Note
that GitHub Packages requires a **classic** personal access token with `write:packages`; a
fine-grained token is refused at the token exchange, before any layer is sent.

**`docker-push` fails with "is not present locally".** The image has not been built under the
current `<image>` name — for example after `--image` was passed to the push but not the build.
Run `./build.sh docker-build` first.
