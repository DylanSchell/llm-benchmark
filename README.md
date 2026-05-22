# Claude Benchmark Runner

A Rust framework for benchmarking autonomous coding agents against the [polyglot exercise suite](https://github.com/Aider-AI/polyglot-benchmark). Agents run exercises inside isolated Docker containers and produce structured results with JSONL trace files.

---

## Quick Start

### Prerequisites

- **Rust 1.75+** (with `cargo`)
- **Docker** (running)
- **polyglot-benchmark repo** — clone it alongside this project:

```bash
git clone https://github.com/Aider-AI/polyglot-benchmark
cd polyglot-benchmark && git checkout main && cd ..
```

### 1. Build the Docker Image

The runner container needs Java, Maven, Gradle, Node.js, Go, Rust, and Claude Code CLI pre-installed:

```bash
docker build -f docker/Dockerfile.runner -t claude-benchmark/runner:latest .
```

### 2. Configure

Create a `config.yaml` in the project root (copy from `config.yaml.example` if available):

```yaml
benchmark_path: ../polyglot-benchmark
parallelism: 4

docker:
  image: claude-benchmark/runner:latest
  memory: 2g
  timeout: 300

output:
  results_dir: ./benchmark-results
  log_level: INFO
```

### 3. Build the Applications

```bash
cargo build --release --workspace
```

This builds all four crates in the workspace:

| Crate | Binary | Purpose |
|-------|--------|---------|
| `benchmark-cli` | `benchmark-cli` | CLI benchmark runner |
| `benchmark-web` | `benchmark-web` | Web dashboard server |
| `benchmark-report` | `benchmark-report` | Token statistics report |
| `benchmark-reporter` | `benchmark-reporter` | Full markdown report generator |

### 4. Run the Web Application

```bash
cargo run --release --package benchmark-web
# or directly:
./target/release/benchmark-web config.yaml
```

Access the dashboard at **http://localhost:8081**.

Override the port via environment variable or CLI:

```bash
SERVER_PORT=9090 ./target/release/benchmark-web config.yaml
```

### 5. Run the CLI Benchmark Runner

The CLI runner executes exercises against a chosen agent (reference, claude, or pi):

```bash
# Run all Java exercises with the reference agent
cargo run --release --package benchmark-cli -- --language java

# Run a single exercise
cargo run --release --package benchmark-cli -- --language rust --exercise two-fer

# Run Python exercises with Claude Code
cargo run --release --package benchmark-cli -- --agent claude --model sonnet --language python

# Run multiple languages
cargo run --release --package benchmark-cli -- --language java,python,rust

# Verbose mode (shows live output from the agent)
cargo run --release --package benchmark-cli -- --language java --verbose

# Retry — re-run exercises even if results already exist
cargo run --release --package benchmark-cli -- --language java --retry

# Override model and results directory
cargo run --release --package benchmark-cli -- \
  --model haiku \
  --results-dir ./my-results \
  --language javascript
```

**CLI Options:**

| Flag | Default | Description |
|------|---------|-------------|
| `--config` | `config.yaml` | Path to config file |
| `--model` | from config | Model name override |
| `--results-dir` | from config | Results directory override |
| `--language` | `java` | Comma-separated languages |
| `--exercise` | *(none)* | Run a single exercise by name |
| `--agent` | `reference` | Agent: `reference`, `claude`, or `pi` |
| `--verbose` | off | Show live token stream output |
| `--retry` | off | Re-run exercises even if results exist (increments attempts) |

### 6. Run the Reporting Applications

Two separate report tools are available:

**Token Statistics Report** (`benchmark-report`):

```bash
cargo run --release --package benchmark-report -- \
  --results-dir ./benchmark-results
```

| Flag | Default | Description |
|------|---------|-------------|
| `--results-dir, -r` | `../benchmark-results` | Results directory to analyze |
| `--agent, -a` | *(all)* | Filter by agent name |

**Full Markdown Report** (`benchmark-reporter`):

```bash
cargo run --release --package benchmark-reporter
# or directly:
./target/release/benchmark-reporter
```

Reads from `../benchmark-results` by default and generates a `results.md` file with summary tables, per-exercise success rates, and per-model breakdowns.

---

## Project Structure

```
crates/
  benchmark-types/          # Shared types: Config, ExerciseResult, Agent traits
  benchmark-core/           # Core logic: DockerClient, ExerciseRunner, Agents
benchmark-cli/              # CLI benchmark runner
benchmark-web/              # Axum web server with REST API + SSE streaming
benchmark-report/           # Token statistics report tool
benchmark-reporter/         # Full markdown report generator
docker/
  Dockerfile.runner         # Container image with build tools
config.yaml                 # Configuration file
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

- **ReferenceAgent** — Copies the reference implementation, runs tests. Validates that exercises are well-formed.
- **ClaudeAgent** — Invokes Claude Code CLI inside the Docker container to solve exercises.
- **PiAgent** — Invokes the Pi coding agent inside the Docker container.

---

## Results

Results are stored in `{results_dir}/{agent}-{model}/` directories:

```
benchmark-results/sonnet-1/
├── result_claude_java_two-fer.json      # Exercise result
├── trace_java_two-fer.jsonl             # Agent interaction trace (JSONL)
├── result_reference_python_hello-world.json
└── trace_python_hello-world.jsonl
```

Individual result files contain: exercise name, language, success status, exit code, output, duration, timestamps, and error messages. Trace files are JSONL with structured agent events (messages, usage, etc.).

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

---

## Configuration Reference

All configuration lives in `config.yaml`:

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `benchmark_path` | string | `../polyglot-benchmark` | Path to the polyglot exercise repo |
| `parallelism` | int | 1 | Number of concurrent exercises |
| `docker.image` | string | `claude-benchmark/runner:latest` | Docker image for exercise containers |
| `docker.memory` | string | `2g` | Container memory limit |
| `docker.timeout` | int | 300 | Container execution timeout (seconds) |
| `output.results_dir` | string | `./benchmark-results` | Directory for result files |
| `output.log_level` | string | `INFO` | Logging level |

---

## Contributing

1. Fork the repository
2. Create a feature branch
3. Make your changes
4. Run tests: `cargo test --workspace`
5. Submit a pull request

---

## License

MIT License - See LICENSE file for details

---

**Version:** 0.1.0  
**Last Updated:** 2026-05-22
