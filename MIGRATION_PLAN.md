# Java to Rust Migration Plan

## Overview

Convert the Java benchmark application to Rust. Create a new Rust project in `rust/` directory with feature parity to the existing Java version.

## Architecture

```
rust/
├── Cargo.toml
├── src/
│   ├── main.rs              # CLI with subcommands (run, analyze)
│   ├── config/              # YAML config loading (serde_yaml)
│   │   └── mod.rs
│   ├── agent/               # ReferenceAgent, ClaudeAgent
│   │   └── mod.rs
│   ├── docker/              # Docker client
│   │   └── mod.rs
│   ├── exercise/            # Exercise running logic
│   │   └── mod.rs
│   ├── model/               # Serialization structs (JSON/YAML)
│   │   └── mod.rs
│   ├── cli/                 # Argument parsing (clap)
│   │   └── mod.rs
│   └── lib.rs               # Library core (shared between bin subcommands)
```

## Key Design Decisions

| Aspect | Approach |
|--------|----------|
| **CLI** | Single binary `benchmark` with `run` and `analyze` subcommands |
| **Docker** | Keep via subprocess calls or `bollard`/`docker-rs` crate |
| **Config** | Keep YAML with `serde_yaml` |
| **Concurrency** | `tokio` async runtime for parallel exercise execution |
| **Serialization** | `serde` with derive macros for JSON/YAML |
| **Logging** | `tracing` crate |

## Dependencies (Cargo.toml)

```toml
[package]
name = "benchmark"
version = "0.1.0"
edition = "2021"

[dependencies]
tokio = { version = "1", features = ["full"] }
serde = { version = "1.0", features = ["derive"] }
serde_yaml = "0.9"
serde_json = "1.0"
clap = { version = "4", features = ["derive"] }
anyhow = "1.0"
tracing = "0.1"
tracing-subscriber = "0.3"
```

## Phases

### Phase 1: Project Setup
- [x] Create `rust/` directory
- [x] Initialize Cargo project
- [x] Set up directory structure
- [x] Add dependencies to Cargo.toml

### Phase 2: Config Module
- [x] Create config structs matching Java Config hierarchy
- [x] Implement ConfigLoader in Rust
- [x] Load config.yaml

### Phase 3: Model Layer
- [x] Define Exercise, ExerciseResult structs
- [x] Define log entry models (LogEntry, Message, Usage, etc.)
- [x] Implement JSON serialization/deserialization

### Phase 4: Docker Integration
- [x] Create DockerClient wrapper
- [x] Implement container run with volume mounts
- [x] Handle exec results and logs

### Phase 5: Agent Implementations
- [x] Implement ReferenceAgent (copy reference implementation)
- [x] Implement ClaudeAgent (invoke Claude Code CLI)
- [x] Agent trait/interface for common behavior

### Phase 6: Exercise Runner
- [x] Exercise discovery logic
- [x] Parallel execution with tokio
- [x] Result collection and persistence

### Phase 7: Result Analyzer
- [x] Parse result JSON files
- [x] Generate results.md report
- [x] Token usage and success rate calculations

### Phase 8: CLI Layer
- [x] Set up clap with subcommands
- [x] Implement `benchmark run` command
- [x] Implement `benchmark analyze` command

## Migration Complete

The Java benchmark application has been fully migrated to Rust. The project structure:

```
rust/
├── Cargo.toml
├── src/
│   ├── main.rs              # CLI entrypoint with clap subcommands
│   ├── lib.rs               # Library core
│   ├── config/mod.rs        # YAML config loading
│   ├── agent/mod.rs         # ReferenceAgent, ClaudeAgent
│   ├── docker/mod.rs        # Docker client wrapper
│   ├── exercise/mod.rs      # Exercise discovery and parallel execution
│   ├── model/mod.rs         # Serialization structs
│   ├── analyzer/mod.rs      # Result analysis and report generation
│   └── cli/mod.rs           # CLI module
```

Usage:
- `cargo run -- run --config config.yaml` - Run benchmark
- `cargo run -- analyze --results-dir ../benchmark-results --output results.md` - Analyze results
- [ ] Set up clap with subcommands
- [ ] Implement `benchmark run` command
- [ ] Implement `benchmark analyze` command

## Backward Compatibility

- Result JSON format can change slightly
- `results.md` output should remain compatible with existing analysis

## Remaining Work Items

### Critical: Missing Core Functionality

- [x] Integrate ClaudeAgent into benchmark flow
- [x] Implement result saving to JSON files
- [x] Implement `printSummary()` method

### High: Limited Language Support

- [x] Extend `copy_reference_impl()` for more languages
  - Rust, JavaScript/TypeScript, Python support added

- [ ] Extend file finders for non-Java languages
  - `find_source_file()`: Handle Rust, Go, Node, Python paths
  - `find_test_file()`: Handle various test directory structures
  - `find_reference_file()`: Handle language-specific reference paths
  - Location: `rust/src/exercise/mod.rs`

- [x] Extend `is_exercise_directory()` check
  - Added: `pom.xml`, `package.json`, `Cargo.toml`, etc.

- [x] Extend `get_test_command()` for more build systems
  - Added: Rust, Python, Ruby, C# support

### Medium: Missing CLI and Utilities

- [x] Add CLI flags to match Java version

- [ ] Implement `resultFileExists()` method
  - Check if result file already exists before running

- [ ] Implement `resultFileSuccess()` method
  - Parse existing result to check if it succeeded

### Low: Missing Model Types

- [ ] Add `StringContent` struct to model module
- [ ] Add `AssistantError` struct to model module
- [ ] Add `ContentListDeserializer` functionality

### Analyzer Improvements

- [x] Implement `archiveClaudeProjects()` in analyzer

## References

- Original Java code: `src/main/java/com/benchmark/`
- Config file: `config.yaml`
- Docker image: `docker/Dockerfile.runner`