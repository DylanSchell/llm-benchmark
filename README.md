# LLM Benchmark Runner

A Rust framework for benchmarking autonomous coding agents against a curated [Exercism](https://exercism.org) exercise suite. The exercises are fetched from pinned Exercism tracks and embedded into the binary at build time — no external checkout is required. Agents run exercises inside isolated Docker containers and produce structured results with JSONL trace files.

---

## Requirements

| | Release binary | Build from source |
|---|---|---|
| **Docker** (Docker Desktop, or Docker Engine on Linux) | required | required |
| **An LLM endpoint** (OpenAI- or Anthropic-compatible) | required for `pi` / `claude` — **not** for `reference` | same |
| **Rust** (toolchain pinned in `rust-toolchain.toml`) | not needed | required |
| **Disk** | ~2 GB (runner image) | ~2 GB + build cache |

The runner image is ~1.4 GB compressed and is public, so no registry login is needed. It is pulled automatically on first use, but see [step 3](#3-pull-the-runner-image-recommended) for why you should pull it up front.

### Which agents can you run?

| Agent | Needs an LLM? | Shipped in the published image? |
|---|---|---|
| `reference` | no — copies the known-good solution and runs the tests | yes |
| `pi` | yes | yes |
| `claude` | yes | **no** — see [docker/README.md](docker/README.md) |

The published runner image deliberately excludes Claude Code, because Anthropic licenses it "all rights reserved" and it is therefore not redistributable. Building a `claude`-capable image for your own use is documented in [docker/README.md](docker/README.md).

---

## Quick Start — install a release binary

No clone, no Rust toolchain.

### 1. Download the release

Pick the archive for your platform from the [Releases page](https://github.com/DylanSchell/llm-benchmark/releases/latest):

| Platform | Asset |
|---|---|
| macOS, Apple silicon | `llm-benchmark-1.3.1-macos-arm64.tar.gz` |
| macOS, Intel | `llm-benchmark-1.3.1-macos-x64.tar.gz` |
| Linux, x86-64 | `llm-benchmark-1.3.1-linux-x64.tar.gz` |
| Linux, arm64 | `llm-benchmark-1.3.1-linux-arm64.tar.gz` |

```bash
VERSION=1.3.1
OS=macos        # macos | linux
ARCH=arm64      # arm64 | x64

BASE="https://github.com/DylanSchell/llm-benchmark/releases/download/v${VERSION}"
curl -LO "${BASE}/llm-benchmark-${VERSION}-${OS}-${ARCH}.tar.gz"
```

### 2. Verify and extract

```bash
ASSET="llm-benchmark-${VERSION}-${OS}-${ARCH}.tar.gz"
curl -LO "${BASE}/${ASSET}.sha256"

# macOS ships shasum; Linux ships sha256sum
shasum -a 256 -c "${ASSET}.sha256" || sha256sum -c "${ASSET}.sha256"

tar -xzf "${ASSET}"
cd "llm-benchmark-${VERSION}-${OS}-${ARCH}"
./llm-benchmark --version
```

Use the per-asset `.sha256` file as shown. The release also publishes a combined `SHA256SUMS`, but it lists all four archives, so `shasum -c SHA256SUMS` only verifies cleanly if you downloaded every one of them.

The archive contains four binaries — `llm-benchmark` (the unified launcher used throughout this guide), `benchmark-cli`, `benchmark-reporter` and `benchmark-token-report` — plus `README.md`, `LICENSE` and `THIRD_PARTY_NOTICES`. It does **not** contain a `config.yaml`; you create that in step 4.

### 3. Pull the runner image (recommended)

```bash
docker pull ghcr.io/dylanschell/llm-benchmark-runner:latest
```

This is the image `config.yaml` points at by default. Pulling it is not strictly required — the image is fetched automatically before a run starts — but doing it up front keeps the first benchmark's duration about the benchmark rather than a multi-minute download. The automatic path has its own budget (`docker.pull_timeout`, default 1800 s) and logs what it is doing, so a download no longer competes with the run's own `docker.timeout`.

### 4. Create `config.yaml`

`llm-benchmark run` reads `config.yaml` from the **current working directory**. It does not have to exist: with no file every setting falls back to its default and a warning is logged, which is enough to run a model server on this machine. A small file that sets the knobs worth setting explicitly:

```yaml
# No endpoint appears here on purpose — see "Configuring the LLM endpoint".
docker:
  image: "ghcr.io/dylanschell/llm-benchmark-runner:latest"
  memory: "2g"
  timeout: 3600

output:
  results_dir: "./benchmark-results"   # set explicitly — the code default is ../benchmark-results
  log_level: "INFO"
```

If your model is on this machine, that is the whole configuration: the endpoint is detected and handed to the container. A remote or hosted model needs one more block — read [Configuring the LLM endpoint](#configuring-the-llm-endpoint), where the two places an endpoint can be set are explained and why they are not interchangeable.

A fully commented template ships with the source tree as [`config.example.yaml`](config.example.yaml).

### 5. Smoke test — no LLM required

The `reference` agent copies the known-good solution and runs the exercise's own test suite. It validates Docker, the runner image and the embedded exercises without contacting any model:

```bash
./llm-benchmark run --agent reference --language java --exercise series
```

You should see `Tests passed for exercise: series`. If this fails, the LLM endpoint is not the problem.

### 6. Run an agent against your endpoint

```bash
./llm-benchmark run --agent pi --language python --exercise hello-world --verbose
```

`--verbose` streams the agent's live output. Once that works, drop `--exercise` to run every exercise for that language.

### 7. Web dashboard (optional)

```bash
./llm-benchmark web --port 8081
```

Then open <http://localhost:8081>. Tera templates and static assets are embedded in the binary, so there is no filesystem dependency.

---

## Quick Start — build from source

### 1. Build the runner image (optional)

You do **not** need to build the image: the published multi-arch image is the default in `config.yaml` and is what the release binaries use. Build your own only if you changed something under `docker/`.

The runner container ships Java, Maven, Gradle, Node.js, Go, Rust and the agent CLIs, all version-pinned in `docker/pins.env`:

```bash
./build.sh docker-build
```

The image version tracks `Cargo.toml`, and `./build.sh docker-verify` fails if the two disagree. See [docker/README.md](docker/README.md) for the version bump policy and how to re-pin agents.

### 2. Build the application

```bash
cargo build --release
```

This produces the unified launcher at `./target/release/llm-benchmark`. The build fetches the exercise suite from the pinned Exercism tracks (see `exercises.manifest.yaml`) and caches it under `target/`; it is then embedded in the binary.

To build individual components separately:

```bash
cargo build --release --package benchmark-cli          # CLI runner
cargo build --release --package benchmark-reporter     # Report generator
cargo build --release --package benchmark-token-report # Token stats
```

### 3. Commands

**Run benchmarks:**

```bash
# Run all Java exercises with the reference agent (no LLM needed)
./target/release/llm-benchmark run --language java

# Run Python exercises with the pi agent
./target/release/llm-benchmark run --agent pi --model qwen3-coder --language python

# Several languages at once
./target/release/llm-benchmark run --language java,python,rust

# Verbose mode (live output from the agent)
./target/release/llm-benchmark run --language java --verbose

# Re-run exercises even if results already exist
./target/release/llm-benchmark run --language java --retry
```

**Generate reports:**

```bash
./target/release/llm-benchmark report                      # Full markdown report
./target/release/llm-benchmark token-report                # Token statistics
./target/release/llm-benchmark token-report --agent pi --language java --details
```

**Web dashboard:**

```bash
./target/release/llm-benchmark web --port 8081
```

---

## Configuring the LLM endpoint

Two different processes call the model, and they are configured separately:

| Setting | Read by | Runs | Reachable default |
|---|---|---|---|
| `inference_endpoint` + `api_key` | the benchmark app | on the **host** | auto-detected: the local ports are probed at startup |
| `docker.environment` → `ANTHROPIC_BASE_URL` / `OPENAI_BASE_URL` | the agent CLI (`pi`, `claude`) | **inside the container** | derived from the endpoint above when it is local; otherwise `OPENAI_BASE_URL` → `http://host.docker.internal:8080/v1`; `ANTHROPIC_BASE_URL` unset |

**`inference_endpoint`** is used for exactly one thing: `GET {inference_endpoint}/models`, which populates the model list in the web dashboard. It is OpenAI-style and must include the `/v1` suffix. If it is unreachable the app logs a warning and falls back to a built-in list, so it is never fatal.

**`docker.environment`** is what actually lets an agent reach your model. Since the agent runs *inside a container*, `localhost` there refers to the container itself, not your machine. Use `host.docker.internal` to reach the host.

Both defaults assume a model server on port **8080** of the same machine, which is what the examples below use. `OPENAI_BASE_URL` is pre-filled for you; `ANTHROPIC_BASE_URL` deliberately is not, so that a `claude` run meaning to reach Anthropic's real API is never silently redirected to a local server. Set either explicitly to override the default.

### If the model runs on this machine

The only setup actually required is starting your model server — no endpoint configuration at all:

1. **Nothing configured** ⇒ at startup the tool asks `http://localhost:{8000, 8080, 9931}/v1/models`, in that order, and adopts the first one that returns a model list. (A plain `200` is not enough: the response has to contain a `data` array, so an unrelated app squatting on port 8000 is skipped rather than mistaken for a model server.)
2. Whatever it adopts is **also used for the container**: a `localhost` endpoint becomes `http://host.docker.internal:<same port>/v1`, so `pi` talks to the same server on the same port with no `docker.environment` block. The startup log says which endpoint was chosen, and `--verbose` (or `RUST_LOG=info`) shows the resolution.
3. If nothing answers, the dashboard falls back to its built-in model list, and the container keeps the default `http://host.docker.internal:8080/v1`. Neither is fatal.

An explicit `inference_endpoint` always wins and disables probing. It is still only rewritten for the container when it points at `localhost` (or `127.0.0.1`, `0.0.0.0`, `::1`) — a remote endpoint is left alone, so a hosted `inference_endpoint` cannot silently become the agent's endpoint.

### Worked example — local model server

For Ollama, LM Studio, llama.cpp or vLLM listening on port 8080 of the same machine:

```yaml
inference_endpoint: "http://localhost:8080/v1"   # host-side, for GET /models

docker:
  environment:
    - ANTHROPIC_AUTH_TOKEN: "not-needed"                      # any non-empty value
    - ANTHROPIC_BASE_URL: "http://host.docker.internal:8080"  # Anthropic-style: no /v1
    - OPENAI_BASE_URL: "http://host.docker.internal:8080/v1"  # OpenAI-style: with /v1
    - OPENAI_API_KEY: "not-needed"
```

The `*_AUTH_TOKEN` / `*_API_KEY` values only need to be non-empty for a local server that does not check them. The `OPENAI_BASE_URL` line is optional — it is the built-in default — so a `docker.environment` block that sets only the key, or is left out altogether, still points `pi` at port 8080 on the host.

### Worked example — a hosted provider

```yaml
docker:
  environment:
    - ANTHROPIC_AUTH_TOKEN: "sk-ant-..."
    - ANTHROPIC_MODEL: "claude-sonnet-4-5"
    # omit ANTHROPIC_BASE_URL to talk to the provider's real API
```

### Platform notes for `host.docker.internal`

- **Docker Desktop (macOS and Windows), Colima, OrbStack and Podman Desktop** inject `host.docker.internal` automatically, including for host services bound to `127.0.0.1`. The examples above work as written.
- **Docker Engine on Linux does not.** `host.docker.internal` will not resolve inside the container, and the app passes no `--add-host`, so on Linux point the `*_BASE_URL` values at the host's address on your network instead, for example `http://192.168.1.50:8080`.
- **On Linux, bind your model server to `0.0.0.0`, not `127.0.0.1`.** A container has its own loopback interface, so a host service listening only on `127.0.0.1` is unreachable from a container regardless of which hostname you use.

---

## Project Structure

```
crates/
  benchmark-types/          # Shared types: Config, ExerciseResult, Agent traits
  benchmark-core/           # Core logic: DockerClient, ExerciseRunner, Agents
  benchmark-exercises/      # Embedded Exercism suite (rust-embed)
  benchmark-exercises-build # Build-time fetch + prune of the exercise suite
benchmark-cli/              # CLI benchmark runner
benchmark-web/              # Axum web server with REST API + SSE streaming
benchmark-token-report/     # Token statistics report tool
benchmark-reporter/         # Full markdown report generator
docker/
  Dockerfile.runner.debian  # Runner image with build tools
src/                        # Unified `llm-benchmark` launcher
```

---

## Architecture

```
config.yaml → Config → ExerciseRunner → Agent → DockerClient (runs container)
    → Executes tests (mvn/go/npm test/cargo test/...)
    → Returns AgentResult
    → Saved to results_dir/{agent}-{model}/result_{lang}_{exercise}.json
    → Report tools parse results and generate summaries
```

**Agents:**

- **ReferenceAgent** — copies the reference implementation and runs the tests. Validates that exercises are well-formed; needs no LLM.
- **PiAgent** — invokes the pi coding agent inside the runner container.
- **ClaudeAgent** — invokes the Claude Code CLI inside the runner container. Requires an image built with Claude Code packaged in; the published image does not include it.

---

## Results

Results land in a subdirectory of `output.results_dir` named after the agent and model — `{agent}-{model}`, so `reference-reference` or `pi-qwen3-coder`:

```
benchmark-results/
└── pi-qwen3-coder/
    ├── result_pi_python_hello-world.json   # Per-exercise outcome
    ├── trace_python_hello-world.jsonl      # Agent interaction trace (JSONL)
    ├── trace_python_hello-world.html       # Rendered trace (pi agent)
    └── log_pi_python_hello-world_*.json    # Raw session logs (pi agent)
```

Result files are named `result_{agent}_{language}_{exercise}.json` and traces `trace_{language}_{exercise}.jsonl`. Each result records exercise name, language, success status, exit code, output, duration, timestamps and any error message; traces are JSONL with structured agent events (messages, usage, and so on). See [docs/RESULT_FORMAT.md](docs/RESULT_FORMAT.md) for the schema.

---

## Configuration Reference

`config.yaml` is read from the current working directory; override the path with `--config` or the `CONFIG_PATH` environment variable.

| Key | Type | Default | Description |
|---|---|---|---|
| `parallelism` | int | `1` | Number of concurrent exercises |
| `model` | string | — | Label used in result directory names; overridden by `--model` |
| `inference_endpoint` | string | auto-detected | Host-side OpenAI-compatible base URL, used for `GET /models`. Omit it and the local ports `8000`, `8080`, `9931` are probed at startup |
| `api_key` | string | — | Bearer token sent to `inference_endpoint` |
| `docker.image` | string | `ghcr.io/dylanschell/llm-benchmark-runner:latest` | Runner image |
| `docker.work_dir` | string | `/workspace` | Working directory inside the container |
| `docker.timeout` | int | `300` | Per-exercise container timeout in seconds (minimum 10). Does **not** include pulling the image |
| `docker.pull_timeout` | int | `1800` | Timeout in seconds for pulling the runner image when it is not present locally |
| `docker.per_command_timeout` | int | `600` | Timeout for any single Bash tool call inside the container |
| `docker.memory` | string | `2g` | Container memory limit |
| `docker.environment` | list of maps | derived from `inference_endpoint` | Environment variables injected into the container — this is where the agent's endpoint goes. A local `inference_endpoint` sets `OPENAI_BASE_URL` to the same port on `host.docker.internal`; otherwise it defaults to `http://host.docker.internal:8080/v1` |
| `output.results_dir` | path | `../benchmark-results` | Where results are written. **Set this explicitly.** |
| `output.log_level` | string | `INFO` | Log level |
| `server.port` | int | `8081` | Dashboard port |

Command-line flags override the file: `--config`, `--model`, `--results-dir`, `--language`, `--exercise`, `--agent`, `--verbose`, `--retry`. Recognised environment overrides: `CONFIG_PATH`, `SERVER_PORT`, `PARALLELISM`, `RESULTS_DIR` (the last two apply to `llm-benchmark web`).

> `docs/CONFIGURATION.md` predates the Rust rewrite and still documents the original Java configuration classes; treat the table above as authoritative.

---

## Testing

```bash
# Run all tests
cargo test --workspace

# Run a specific crate's tests
cargo test --package benchmark-core

# Run a single test
cargo test --package benchmark-core -- exercise_runner::tests::test_find_exercise
```

The web UI test suite (`benchmark-web/tests/html_equality.rs`) starts its own server on port 3000 and will fail if anything else already holds that port; set `SERVER_URL` to point at an already-running instance to skip spawning one.

---

## Contributing

1. Fork the repository
2. Create a feature branch
3. Make your changes
4. Run tests: `cargo test --workspace`
5. Submit a pull request

---

## License

MIT — see [`LICENSE`](LICENSE). Third-party components and the embedded Exercism exercise content are attributed in [`THIRD_PARTY_NOTICES`](THIRD_PARTY_NOTICES).

---

**Version:** 1.3.1
**Last Updated:** 2026-09-12
