# Claude Benchmark Runner

A framework for benchmarking autonomous coding agents against the polyglot exercise suite.

---

## Quick Start

### Prerequisites

- Java 21+
- Maven 3.8+
- Docker

### Build

```bash
mvn package -q
```

### Run (CLI Mode)

```bash
java -jar target/claude-benchmark-1.0-SNAPSHOT.jar \
  --agent=reference \
  --languages=java
```

### Run (Web Mode)

```bash
java -jar target/claude-benchmark-1.0-SNAPSHOT.jar --web
# Access dashboard at http://localhost:8080
```

---

## Documentation

| Document | Description |
|----------|-------------|
| [Architecture](docs/ARCHITECTURE.md) | System architecture and design patterns |
| [API Reference](docs/API.md) | REST API endpoints and usage |
| [Configuration](docs/CONFIGURATION.md) | Configuration options and examples |
| [Developer Guide](docs/DEVELOPER.md) | Contributing, building, testing |
| [Result Format](docs/RESULT_FORMAT.md) | Result file formats and structure |

---

## Features

- **Multi-Language Support**: Java, Python, JavaScript, Go, Rust, C++, and more
- **Multiple Agents**: Reference agent (baseline), Claude Code CLI agent
- **Web Dashboard**: Real-time progress tracking and result visualization
- **Docker Isolation**: Exercises run in isolated containers
- **Parallel Execution**: Configurable concurrent exercise runs
- **Comprehensive Results**: JSON results, JSONL traces, markdown reports

---

## Architecture Overview

```
┌─────────────────────────────────────────────┐
│              Presentation Layer              │
│  CLI │ Web Controllers │ REST API           │
└────────────────┬────────────────────────────┘
                 ▼
┌─────────────────────────────────────────────┐
│               Service Layer                  │
│  BenchmarkService (Facade)                   │
│  ├─ SessionManager                           │
│  ├─ BenchmarkExecutor                        │
│  └─ QueueProcessor                           │
└────────────────┬────────────────────────────┘
                 ▼
┌─────────────────────────────────────────────┐
│               Domain Layer                   │
│  BenchmarkRunner │ ExerciseRunner            │
│  ReferenceAgent │ ClaudeAgent                │
│  LanguageHandlers (Strategy Pattern)         │
└────────────────┬────────────────────────────┘
                 ▼
┌─────────────────────────────────────────────┐
│            Infrastructure Layer              │
│  DockerClient │ ResultPersister │ Config     │
└─────────────────────────────────────────────┘
```

---

## Project Structure

```
src/main/java/com/benchmark/
├── BenchmarkRunner.java          # Main orchestration
├── agent/                        # Agent implementations
│   ├── LanguageHandler.java      # Strategy interface
│   ├── ReferenceAgent.java       # Reference implementation
│   ├── ClaudeAgent.java          # Claude Code CLI agent
│   └── handlers/                 # Language-specific handlers
├── config/                       # Configuration management
├── docker/                       # Docker integration
├── exception/                    # Exception hierarchy
├── exercise/                     # Exercise handling
├── model/                        # Domain models
├── persistence/                  # Persistence layer
└── web/                          # Web layer
    ├── controller/               # REST controllers
    ├── service/                  # Service layer
    └── domain/                   # Web-specific models
```

---

## Refactoring Progress

This project is undergoing a multi-phase refactoring to improve code quality and maintainability.

### Phase 1: ✅ COMPLETE
- ResultPersister extraction
- Inner classes converted to records
- Configuration validation
- AgentFactory interface

### Phase 2: ✅ COMPLETE  
- Java 21 upgrade
- Build warnings fixed
- BenchmarkController split (4 focused controllers)
- CLI entry point extracted
- Error handling improved (exception hierarchy)
- BenchmarkService split (SessionManager, BenchmarkExecutor, QueueProcessor)
- ReferenceAgent refactored with Strategy Pattern (LanguageHandler interface + 6 handlers)

### Phase 3: 🚧 IN PROGRESS
- [x] Documentation added (ARCHITECTURE.md, API.md, CONFIGURATION.md, DEVELOPER.md, RESULT_FORMAT.md)
- [ ] BenchmarkResultAnalyzer refactoring
- [ ] Model class consolidation
- [ ] Comprehensive testing

**Overall Progress:** 11/15 items complete (73%)

See [REFACTORING_PLAN.md](REFACTORING_PLAN.md) for details.

---

## Configuration

Create a `config.yaml` file:

```yaml
benchmark:
  path: ../polyglot-benchmark
  parallelism: 4

docker:
  image: claude-benchmark/runner:latest
  memory: 2g
  timeout: 300

output:
  results_dir: ./results
  log_level: INFO

agents:
  reference:
    enabled: true
  
  claude:
    enabled: true
    cli_path: /usr/local/bin/claude
    model: sonnet
```

See [Configuration Reference](docs/CONFIGURATION.md) for all options.

---

## Results

Results are stored in `results/{model}-{sequence}/` directories:

```
results/sonnet-1/
├── trace_java_two-fer.jsonl      # Agent interaction trace
├── result_java_two-fer.json      # Exercise result
├── trace_python_hello-world.jsonl
└── result_python_hello-world.json
```

See [Result Format](docs/RESULT_FORMAT.md) for details.

---

## Testing

```bash
# Run all tests
mvn test

# Run specific test
mvn test -Dtest=ResultPersisterTest

# Generate coverage report
mvn clean test jacoco:report
```

---

## Contributing

1. Fork the repository
2. Create a feature branch
3. Make your changes
4. Write/update tests
5. Ensure all tests pass
6. Submit a pull request

See [Developer Guide](docs/DEVELOPER.md) for details.

---

## License

MIT License - See LICENSE file for details

---

## Related Projects

- [polyglot-benchmark](https://github.com/Aider-AI/polyglot-benchmark) - Exercise suite
- [Claude Code CLI](https://docs.anthropic.com/claude-code/) - Claude automation framework

---

**Version:** 1.0-SNAPSHOT  
**Last Updated:** 2026-02-28
