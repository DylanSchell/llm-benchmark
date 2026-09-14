# Developer Guide

This guide provides everything you need to contribute to the LLM Benchmark Runner.

---

## Table of Contents

- [Getting Started](#getting-started)
- [Building the Project](#building-the-project)
- [Running Tests](#running-tests)
- [Adding New Languages](#adding-new-languages)
- [Adding New Agents](#adding-new-agents)
- [Code Style](#code-style)
- [Git Workflow](#git-workflow)
- [Troubleshooting](#troubleshooting)

---

## Getting Started

### Prerequisites

- **Rust 1.75+** (with `cargo`)
- **Docker** (for running exercises)
- **Git**

### Clone the Repository

```bash
git clone https://github.com/your-org/llm-benchmark.git
cd llm-benchmark
```

### Configure Environment

1. Copy the example configuration:

```bash
cp config.example.yaml config.yaml
```

2. Edit `config.yaml` with your settings:

```yaml
docker:
  image: ghcr.io/dylanschell/llm-benchmark-runner:latest
  memory: 2g

output:
  results_dir: ./results
```

3. Build the Docker runner image:

```bash
./build.sh docker-build
```

---

## Building the Project

### Quick Build

```bash
cargo build --release
```

This creates the `llm-benchmark` launcher binary.

### Build Docker Image

```bash
./build.sh docker-build
./build.sh docker-verify   # confirm docker/ inputs match the recorded version
```

The image version is the project version in `Cargo.toml` — one number for the binary, the image tag
and the release tag. Its build args deliberately have no defaults — always build through `build.sh`.
See `docker/README.md`.

---

## Running the Application

### CLI Mode

```bash
# Run reference agent for Java exercises
./target/release/llm-benchmark run --language java

# Run Claude agent for specific exercise
./target/release/llm-benchmark run --agent claude --model sonnet --language python --exercise two-fer
```

### Web Mode

```bash
# Start web server
./target/release/llm-benchmark web --port 8080

# Access dashboard at http://localhost:8080
```

---

## Running Tests

### Run All Tests

```bash
cargo test --workspace
```

### Run Specific Crate Tests

```bash
cargo test --package benchmark-core
```

### Run Single Test

```bash
cargo test --package benchmark-core exercise_runner::tests::test_find_exercise
```

---

## Adding New Languages

A language is not a plugin class. It is three things: a track in the manifest, a test command
the runner recognises, and a toolchain in the runner image.

### Step 1: Add the track to the manifest

Edit `exercises.manifest.yaml`: add the track to `sources`, pinned to a reviewed upstream
commit, and list the exercises to include under `exercises.<language>`.

```yaml
sources:
  ruby: { repo: "https://github.com/exercism/ruby", ref: "<upstream-commit>" }

exercises:
  ruby:
    include:
      - acronym
      - binary-search
```

Third-party exercise content is never committed. `crates/benchmark-exercises/build.rs`
declares the manifest as a `rerun-if-changed` input, so the next build of that crate fetches,
prunes, re-stages `target/exercises-bundle` and refreshes `exercises.lock.yaml`:

```bash
cargo build -p benchmark-exercises     # network required for a track not already cached
```

Commit the regenerated `exercises.lock.yaml`. Setting `LLM_BENCHMARK_EXERCISES_OFFLINE=1`
forbids network access, so it will fail rather than silently fetch. Selection and pruning
rules are documented in `docs/EXERCISE_SELECTION.md`.

### Step 2: Teach the runner the test command

`ReferenceAgent::get_test_command` (`crates/benchmark-core/src/agent/reference.rs`) picks the
command from the build files present, so a track using `pom.xml`, `build.gradle`/`build.gradle.kts`,
`go.mod`, `package.json`, `CMakeLists.txt`, `Cargo.toml`, `Gemfile` or a `.csproj`/`.sln` needs
no change. Anything else needs a branch:

```rust
fn get_test_command<'a>(exercise: &'a Exercise, work_dir: &Path) -> Vec<&'a str> {
    if work_dir.join("pom.xml").exists() {
        vec!["mvn", "test", "-q"]
    // … existing branches …
    } else if work_dir.join("Gemfile").exists() {
        vec!["bundle", "exec", "rake", "test"]
    } else {
        // Add the new language here; keep the error!() for genuinely unknown cases.
    }
}
```

If the track ships skipped tests that must run, add an arm to `run_patch_tests` in
`crates/benchmark-core/src/agent/test_patches.rs`. The existing arms strips `#[ignore]`
(Rust), `@Disabled` (Java) and `xtest(` (JavaScript/TypeScript).

Three places, one per agent surface:

| File | What it decides |
|---|---|
| `crates/benchmark-core/src/agent/reference.rs` | How the reference agent runs the tests |
| `crates/benchmark-core/src/agent/test_patches.rs` | Which skip annotations are stripped |
| `crates/benchmark-core/src/agent/pi.rs` | The per-language "run tests with …" prompt hint |

### Step 3: Update Docker Image

Add the language runtime to `docker/Dockerfile.runner.debian`. If it is fetched from outside
this repository, its version belongs in `docker/pins.env` as a build `ARG` rather than inline:
`docker/pin-agents.sh --check` rejects unpinned installs, and `docker-verify` rejects one whose
pin is not declared.

```dockerfile
# Install Ruby
ARG RUBY_VERSION
RUN apt-get update && apt-get install -y "ruby-full=${RUBY_VERSION}" && rm -rf /var/lib/apt/lists/*
```

Adding a runtime changes the binary↔image contract, so it needs a version bump. Bump the version in
`Cargo.toml` (and refresh `Cargo.lock` with it), then rebuild and check:

```bash
./build.sh docker-build
./build.sh docker-verify
```

See `docker/README.md` for the full bump policy.

### Step 4: Test

```bash
cargo test -p benchmark-exercises-build   # selection, pruning, lock generation
./build.sh docker-build                   # image now carries the runtime
./build.sh test                           # end to end
```

---

## Adding New Agents

An agent is an implementation of the `Agent` trait
(`crates/benchmark-types/src/agent/mod.rs`) plus a variant of the `AgentKind` enum, which is
what the CLI and the dashboard parse `--agent` into.

### Step 1: Create Agent Implementation

```rust
// crates/benchmark-core/src/agent/gemini.rs
use async_trait::async_trait;
use benchmark_types::agent::AgentResult;
use benchmark_types::exercise::Exercise;
use benchmark_types::exercise_source::ExerciseSource;
use std::path::Path;

use crate::docker::DockerClient;

pub struct GeminiAgent {
    docker: DockerClient,
}

impl GeminiAgent {
    pub fn new(docker: DockerClient) -> Self {
        Self { docker }
    }
}

#[async_trait]
impl benchmark_types::agent::Agent for GeminiAgent {
    async fn run_exercise(
        &self,
        exercise: &Exercise,
        source: &dyn ExerciseSource,
        model: &str,
        thinking_level: Option<&str>,
        results_dir: &Path,
    ) -> Result<AgentResult, Box<dyn std::error::Error + Send + Sync>> {
        self.run_exercise_with_timeout(exercise, source, model, thinking_level, results_dir, None).await
    }

    async fn run_exercise_with_timeout(
        &self,
        exercise: &Exercise,
        source: &dyn ExerciseSource,
        model: &str,
        _thinking_level: Option<&str>,
        results_dir: &Path,
        timeout_override_secs: Option<u64>,
    ) -> Result<AgentResult, Box<dyn std::error::Error + Send + Sync>> {
        // 1. Materialise the exercise into a temp dir (exercise_files::materialize_exercise)
        // 2. Build the container command and run it through self.docker
        // 3. Save the result under results_dir and return it
        todo!()
    }

    fn get_name(&self) -> &str {
        "gemini"
    }
}
```

Agents reach the container through `DockerClient`, not the whole `Config`; they take
`&dyn ExerciseSource` so exercise content stays embedded in the binary. Add the module to
`crates/benchmark-core/src/agent/mod.rs` and re-export it there.

### Step 2: Register the agent

Add a variant to `AgentKind` and arms to its `FromStr` and `Display` implementations
(`crates/benchmark-types/src/agent/mod.rs`):

```rust
pub enum AgentKind {
    Reference,
    Claude,
    Pi,
    Gemini,
}

impl AgentKind {
    pub const ALL: &[AgentKind] = &[AgentKind::Reference, AgentKind::Claude, AgentKind::Pi, AgentKind::Gemini];
}
```

Then add a match arm at **each** construction site — there are three, and a missed one is a
runtime panic or an unsupported-agent error on that surface:

| Site | Surface |
|---|---|
| `benchmark-cli/src/runner.rs` (`create_agent`) | `llm-benchmark run` |
| `benchmark-web/src/services/benchmark_executor.rs` | benchmark runs scheduled from the dashboard |
| `crates/benchmark-core/src/lib.rs` | the library entry point |

```rust
// benchmark-cli/src/runner.rs
AgentKind::Gemini => Arc::new(GeminiAgent::new(docker_client)),
```

Finally, a new agent needs its CLI inside the runner image, which means the agent-set decision
in `docker/agents.env` and a version bump — see `docker/README.md`.

---

## Code Style

### Rust Code Style

We follow the [Rust Style Guide](https://doc.rust-lang.org/style-guide/) with minor modifications:

- **Indentation:** 4 spaces (no tabs)
- **Line length:** 120 characters
- **Braces:** Rust standard (same as K&R)
- **Imports:** Grouped by crate, then alphabetically within groups

### Formatting

Use `rustfmt`:

```bash
cargo fmt --all
```

### Linting

Run `clippy` before committing:

```bash
cargo clippy --all-targets -- -D warnings
```

### Naming Conventions

| Element | Convention | Example |
|---------|------------|---------|
| Classes | PascalCase | `BenchmarkRunner` |
| Methods | camelCase | `runExercise()` |
| Fields | camelCase | `dockerClient` |
| Constants | UPPER_SNAKE_CASE | `MAX_RETRIES` |
| Interfaces | Adjective or noun | `LanguageHandler`, `Runnable` |
| Test methods | descriptive_with_underscores | `shouldThrowExceptionWhenInvalidConfig()` |

---

## Git Workflow

### Branch Naming

```
feature/add-ruby-support
fix/docker-timeout-issue
docs/update-api-docs
refactor/extract-service-layer
test/add-integration-tests
```

### Commit Messages

Use conventional commits:

```
feat: Add Ruby language support
fix: Resolve Docker timeout issue
docs: Update API documentation
refactor: Extract BenchmarkExecutor service
test: Add integration tests for web layer
chore: Update dependencies
```

### Pull Request Checklist

- [ ] Code follows style guidelines
- [ ] All tests pass
- [ ] Documentation updated
- [ ] No new compiler warnings
- [ ] Changelog updated (if applicable)

---

## Project Structure

```
src/                        # llm-benchmark launcher binary (run / web / report / token-report)
crates/
  benchmark-types/          # Shared types: Config, Exercise, Agent trait, AgentResult
  benchmark-core/           # Core logic: DockerClient, ExerciseRunner, agents
  benchmark-exercises/      # rust-embed over the staged exercise bundle
  benchmark-exercises-build/ # Build support: fetch, prune, lock, verify
benchmark-cli/              # CLI benchmark runner
benchmark-web/              # Axum web server with REST API + SSE streaming
benchmark-token-report/     # Token statistics report tool
benchmark-reporter/         # Full markdown report generator
docker/
  Dockerfile.runner.debian  # Container image with build tools
  pins.env                  # Pinned tool and agent versions
config.yaml                 # Configuration file
exercises.manifest.yaml     # Exercise inclusion control
```

---

## Debugging

### Enable Debug Logging

Add to `config.yaml`:

```yaml
output:
  log_level: DEBUG
```

Or via CLI:

```bash
./target/release/llm-benchmark run --language java --verbose
```

### Debug Docker Containers

There is no client library to instrument: the runner shells out to the `docker` CLI, and
`crates/benchmark-core/src/docker/client.rs` logs the exact command line it builds.

```bash
# Full `docker run` invocation, container ids, and liveness decisions
./target/release/llm-benchmark run --language java --verbose

# Or raise just the Docker layer's verbosity
RUST_LOG=benchmark_core=debug ./target/release/llm-benchmark run --language java
```

Look for `Executing with memory limit … and volume … : docker run …`, which prints the command
verbatim.

Containers are **not** started with `--rm`, so a run leaves its container behind as
`bench-<random>` until the client cleans it up. That makes the usual tools usable while it is
alive:

```bash
docker ps -a --filter name=bench-      # containers the benchmark created
docker logs <container-id>             # everything the agent saw
docker exec -it <container-id> bash    # inspect the filesystem
```

The materialised exercise is also on the host: each run gets a temporary directory mounted at
`/workspace`, and the agent's traces are written to the results directory.

### IDE Setup

**VS Code:**
1. Install rust-analyzer extension
2. Open workspace folder
3. Configure Cargo.toml if needed

**IntelliJ IDEA:**
1. Open as Rust project (with IntelliJ Rust plugin) or use VS Code

---

## Performance Tuning

### Parallelism

Adjust based on available resources:

```yaml
# Low-end machine (4GB RAM)
benchmark:
  parallelism: 1

# Standard machine (8-16GB RAM)
benchmark:
  parallelism: 4

# High-end server (32GB+ RAM)
benchmark:
  parallelism: 8
```

### Docker Memory

Increase if you see OOM errors:

```yaml
docker:
  memory: 4g
```

---

## Common Issues and Solutions

### Docker Connection Refused

**Symptom:** `Cannot connect to the Docker daemon`

**Solution:**
```bash
# Check Docker is running
docker ps

# Add user to docker group (Linux)
sudo usermod -aG docker $USER
newgrp docker
```

### Test Compilation Fails

**Symptom:** Tests fail to compile in Docker container

**Solution:**
1. Verify exercise has valid test files
2. Check Docker image has required build tools
3. Enable debug logging to see full error

### Out of Memory

**Symptom:** Memory issues with Docker containers

**Solution:**
```bash
# Rust binaries manage memory automatically, adjust Docker container memory
docker:
  memory: 4g
```

---

## Contributing

### How to Contribute

1. Fork the repository
2. Create a feature branch
3. Make your changes
4. Write/update tests
5. Ensure all tests pass
6. Submit a pull request

### Reporting Bugs

Create an issue with:
- Description of the bug
- Steps to reproduce
- Expected vs actual behavior
- Environment details (OS, Java version, Docker version)

### Feature Requests

Create an issue with:
- Clear description of the feature
- Use cases and benefits
- Proposed implementation approach (optional)

---

## Release Process

One number identifies a release — the git tag, the version the binary reports, and the image tag —
and it lives in `Cargo.toml`. Cutting one:

1. Bump `version` under `[workspace.package]` in `Cargo.toml`, and refresh `Cargo.lock` with it.
   `./docker/pin-agents.sh --patch` does both as part of a re-pin; to bump alone:

   ```bash
   cargo metadata --format-version 1 > /dev/null
   ```

   CI builds with `--locked`, so a stale `Cargo.lock` is a hard failure.
2. Update `CHANGELOG.md`.
3. If `docker/` changed, rebuild and publish the image at the same number:

   ```bash
   ./build.sh docker-build --arch linux/amd64,linux/arm64
   ./build.sh docker-push
   ```

   `./build.sh docker-verify` tells you whether this is needed.
4. Tag the same number and push. That is the release:

   ```bash
   git tag "v$(bash -c '. ./build.sh >/dev/null 2>&1; runner_version')"
   git push github main --follow-tags
   ```

   The tag starts `.github/workflows/build.yml`, which builds and tests every platform
   (`linux/{x64,arm64}` and `macos/{arm64,x64}`) and attaches the tarballs plus `SHA256SUMS` to a
   GitHub Release. It refuses to publish if the tag disagrees with `Cargo.toml`.

There is no release branch and no manual build — the workflow is the build.

---

## Related Documentation

- [Architecture Overview](ARCHITECTURE.md)
- [API Documentation](API.md)
- [Configuration Reference](CONFIGURATION.md)
- [Result Format](RESULT_FORMAT.md)

---

