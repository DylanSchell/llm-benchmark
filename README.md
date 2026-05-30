# LLM Benchmark Runner

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
docker build -f docker/Dockerfile.runner -t llm-benchmark/runner:latest .
```

### 2. Configure

Create a `config.yaml` in the project root (copy from `config.yaml.example` if available):

```yaml
benchmark_path: ../polyglot-benchmark
parallelism: 4

docker:
  image: llm-benchmark/runner:latest
  memory: 2g
  timeout: 300

output:
  results_dir: ./benchmark-results
  log_level: INFO
```

### 3. Build the Application

```bash
cargo build --release
```

This builds the unified launcher binary:

| Binary | Purpose |
|--------|---------|
| `llm-benchmark` | Unified launcher for all commands |

To build individual components separately:

```bash
cargo build --release --package benchmark-web       # Web dashboard
cargo build --release --package benchmark-cli       # CLI runner (standalone)
cargo build --release --package benchmark-reporter  # Report generator (standalone)
cargo build --release --package benchmark-token-report  # Token stats (standalone)
```

### 4. Using the Launcher

The `llm-benchmark` launcher provides all commands in a single binary:

```bash
./target/release/llm-benchmark --help
```

**Run Benchmarks:**

```bash
# Run all Java exercises with the reference agent
./target/release/llm-benchmark run --language java

# Run Python exercises with Claude Code
./target/release/llm-benchmark run --agent claude --model sonnet --language python

# Run multiple languages
./target/release/llm-benchmark run --language java,python,rust

# Verbose mode (shows live output from the agent)
./target/release/llm-benchmark run --language java --verbose

# Retry — re-run exercises even if results already exist
./target/release/llm-benchmark run --language java --retry
```

**Generate Reports:**

```bash
# Full markdown report
./target/release/llm-benchmark report

# Token statistics report
./target/release/llm-benchmark token-report

# Token stats with filters
./target/release/llm-benchmark token-report --agent claude --language java --details
```

**Web Dashboard:**

The web server is fully integrated into the launcher with all templates and static files embedded in the binary:

```bash
./target/release/llm-benchmark web --port 8081
```

Access the dashboard at **http://localhost:8081**.

All Tera templates and CSS are compiled into the binary, so there's no filesystem dependency for resources.

---

## Project Structure

```
crates/
  benchmark-types/          # Shared types: Config, ExerciseResult, Agent traits
  benchmark-core/           # Core logic: DockerClient, ExerciseRunner, Agents
benchmark-cli/              # CLI benchmark runner
benchmark-web/              # Axum web server with REST API + SSE streaming
benchmark-token-report/     # Token statistics report tool
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

| Key | Type | Default                             | Description |
|-----|------|-------------------------------------|-------------|
| `benchmark_path` | string | `../polyglot-benchmark`             | Path to the polyglot exercise repo |
| `parallelism` | int | 1                                   | Number of concurrent exercises |
| `docker.image` | string | `llm-benchmark/runner:latest` | Docker image for exercise containers |
| `docker.memory` | string | `2g`                                | Container memory limit |
| `docker.timeout` | int | 300                                 | Container execution timeout (seconds) |
| `output.results_dir` | string | `./benchmark-results`               | Directory for result files |
| `output.log_level` | string | `INFO`                              | Logging level |

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
