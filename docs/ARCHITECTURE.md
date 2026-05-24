# Architecture Overview

This document describes the architecture of the Claude Benchmark Runner, a framework for benchmarking autonomous coding agents against the polyglot exercise suite.

---

## System Context

```
┌─────────────────────────────────────────────────────────────────┐
│                     Claude Benchmark Runner                      │
│                                                                  │
│  ┌──────────────┐    ┌──────────────┐    ┌──────────────────┐  │
│  │   CLI Mode   │    │ Web UI Mode  │    │  Batch Processing │  │
│  └──────┬───────┘    └──────┬───────┘    └─────────┬────────┘  │
│         │                   │                      │            │
│         └───────────────────┼──────────────────────┘            │
│                             ▼                                   │
│              ┌──────────────────────────┐                       │
│              │   BenchmarkRunner        │                       │
│              │   (Orchestration Layer)  │                       │
│              └──────────┬───────────────┘                       │
└─────────────────────────┼──────────────────────────────────────┘
                          │
          ┌───────────────┼───────────────┐
          ▼               ▼               ▼
   ┌──────────┐    ┌──────────┐    ┌──────────┐
   │  Agent   │    │ Exercise │    │ Docker   │
   │ Layer    │    │ Runner   │    │ Client   │
   └──────────┘    └──────────┘    └──────────┘
```

---

## Component Architecture

### High-Level Components

```
┌─────────────────────────────────────────────────────────────────────┐
│                         Presentation Layer                          │
├─────────────────────────────────────────────────────────────────────┤
│  CliEntryPoint    │  BenchmarkController  │  ResultController       │
│  (CLI Mode)       │  (Web Dashboard)      │  (REST API)             │
│                   │                       │                         │
│  ExerciseController│ QueueController     │  ...                    │
└─────────────────────────────────────────────────────────────────────┘
                              │
┌─────────────────────────────▼───────────────────────────────────────┐
│                         Service Layer                               │
├─────────────────────────────────────────────────────────────────────┤
│  BenchmarkService (Facade)                                          │
│    ├─ SessionManager     - Session lifecycle management             │
│    ├─ BenchmarkExecutor  - Benchmark execution orchestration        │
│    └─ QueueProcessor     - Async queue processing                   │
│                                                                     │
│  ResultService            - Result persistence and retrieval        │
└─────────────────────────────────────────────────────────────────────┘
                              │
┌─────────────────────────────▼───────────────────────────────────────┐
│                         Domain Layer                                │
├─────────────────────────────────────────────────────────────────────┤
│  BenchmarkRunner            - Core orchestration logic              │
│  ExerciseRunner             - Single exercise execution             │
│                                                                     │
│  Agent Layer:                                                       │
│    ├─ ReferenceAgent        - Validates reference implementation   │
│    └─ ClaudeAgent           - Runs Claude Code CLI                 │
│                                                                     │
│  Language Handlers (Strategy Pattern):                              │
│    ├─ JavaHandler           │ GoHandler                            │
│    ├─ JavaScriptHandler     │ PythonHandler                        │
│    ├─ RustHandler           │ CppHandler                           │
└─────────────────────────────────────────────────────────────────────┘
                              │
┌─────────────────────────────▼───────────────────────────────────────┐
│                         Infrastructure Layer                        │
├─────────────────────────────────────────────────────────────────────┤
│  DockerClient              - Container management                   │
│  ResultPersister           - File-based result storage              │
│  ConfigLoader              - YAML configuration loading             │
└─────────────────────────────────────────────────────────────────────┘
```

---

## Package Structure (Rust)

```
crates/
├── benchmark-types/              # Shared types and traits
│   ├── lib.rs
│   ├── config.rs                 # Config, DockerConfig, OutputConfig
│   ├── exercise.rs               # Exercise domain model
│   ├── result.rs                 # ExerciseResult, AgentResult
│   └── agent.rs                  # Agent trait
│
├── benchmark-core/               # Core business logic
│   ├── lib.rs
│   ├── docker_client.rs          # Docker container management
│   ├── exercise_runner.rs        # Single exercise execution
│   ├── benchmark_runner.rs       # Core orchestration
│   ├── handlers/                 # Language-specific handlers
│   │   ├── mod.rs
│   │   ├── registry.rs           # LanguageHandlerRegistry
│   │   ├── language_handler.rs   # Trait interface
│   │   ├── java_handler.rs
│   │   ├── go_handler.rs
│   │   ├── javascript_handler.rs
│   │   ├── python_handler.rs
│   │   ├── rust_handler.rs
│   │   └── cpp_handler.rs
│   └── agents/                   # Agent implementations
│       ├── mod.rs
│       ├── reference_agent.rs
│       └── claude_agent.rs
│
benchmark-cli/                    # CLI binary
│   └── src/main.rs               # CLI entry point (clap)
│
benchmark-web/                    # Web server
│   └── src/
│       ├── main.rs               # Axum server entry point
│       ├── routes.rs             # REST API handlers
│       ├── sse.rs                # Server-sent events
│       └── templates/            # Tera templates (embedded)
│
benchmark-reporter/               # Report generator
└── benchmark-token-report/       # Token statistics tool
```

---

## Data Flow

### Exercise Execution Flow

```
1. User submits benchmark request (CLI or Web)
   │
   ▼
2. BenchmarkRunner creates BenchmarkSession
   │
   ▼
3. ExerciseRunner loads exercises from polyglot-benchmark repo
   │
   ▼
4. For each exercise:
   ┌──────────────────────────────────────────────┐
   │ a. Create temporary working directory        │
   │ b. Copy exercise files (excluding reference) │
   │ c. Agent processes the exercise:             │
   │    - ReferenceAgent: Copies reference impl   │
   │    - ClaudeAgent: Invokes Claude Code CLI    │
   │ d. Run tests in Docker container             │
   │ e. Capture output and results                │
   │ f. Persist result to JSON file               │
   └──────────────────────────────────────────────┘
   │
   ▼
5. Aggregate results and generate report
```

### Language Handler Flow

```
Exercise Runner
      │
      ▼
LanguageHandlerRegistry.getHandler(exercise.language)
      │
      ▼
Selected Handler (e.g., JavaHandler)
      │
      ├── copyReference()     - Copy reference implementation
      ├── copyTests()         - Copy test files
      ├── getTestCommand()    - Return language-specific test command
      └── patchTests()        - Remove @Disabled/#[ignore] annotations
```

---

## Execution Lifecycle

### Session Lifecycle

```
┌─────────────┐     ┌─────────────┐     ┌─────────────┐
│   CREATED   │────▶│  STARTED    │────▶│   RUNNING   │
└─────────────┘     └─────────────┘     └─────────────┘
                                              │
                        ┌─────────────────────┼─────────────────────┐
                        ▼                     ▼                     ▼
                   ┌─────────────┐     ┌─────────────┐     ┌─────────────┐
                   │   PAUSED    │     │ COMPLETED   │     │  FAILED     │
                   └─────────────┘     └─────────────┘     └─────────────┘
                        │                     │                     │
                        ▼                     ▼                     ▼
                   ┌─────────────┐     ┌─────────────┐     ┌─────────────┐
                   │  RESUMED    │     │   CANCELLED │     │   CANCELLED │
                   └─────────────┘     └─────────────┘     └─────────────┘
```

### Queue Processing Lifecycle

```
┌─────────────┐     ┌─────────────┐     ┌─────────────┐
│  ENQUEUED   │────▶│ PROCESSING  │────▶│ COMPLETED   │
└─────────────┘     └─────────────┘     └─────────────┘
        │                  │
        │                  ▼
        │           ┌─────────────┐
        │           │   FAILED    │
        │           └─────────────┘
        │                  │
        └──────────────────┘ (retry or move to dead letter)
```

---

## Key Design Patterns

### 1. Strategy Pattern - Language Handlers

**Purpose:** Encapsulate language-specific operations behind a common trait.

```rust
// In benchmark-core/src/handlers/language_handler.rs
pub trait LanguageHandler: Send + Sync {
    fn get_language(&self) -> &str;
    fn copy_reference(&self, exercise: &Exercise, temp_dir: &Path) -> Result<()>;
    fn copy_tests(&self, exercise: &Exercise, source_dir: &Path, dest_dir: &Path) -> Result<()>;
    fn get_test_command(&self, exercise: &Exercise) -> Vec<String>;
    fn patch_tests(&self, temp_work_dir: &Path) -> Result<()>;
}
```

**Benefits:**
- Easy to add new language support
- Each handler knows only about its language
- Dynamic dispatch via trait objects

### 2. Factory Pattern - Agent Creation

**Purpose:** Create agents via trait objects and configuration.

```rust
// Agents are created via a simple factory function
pub fn create_agent(agent_type: &str, config: &Config) -> Result<Box<dyn Agent>> {
    match agent_type {
        "reference" => Ok(Box::new(ReferenceAgent::new(config))),
        "claude" => Ok(Box::new(ClaudeAgent::new(config))),
        _ => Err(BenchmarkError::UnknownAgent(agent_type.to_string())),
    }
}
```

**Benefits:**
- Simple agent creation
- Easy to add new agent types
- Testable with trait mocks

### 3. Facade Pattern - Benchmark Runner

**Purpose:** Provide simplified interface to complex subsystem.

```rust
// In benchmark-core/src/benchmark_runner.rs
pub struct BenchmarkRunner {
    config: Config,
    docker_client: DockerClient,
    registry: LanguageHandlerRegistry,
}

impl BenchmarkRunner {
    pub fn run_benchmark(&self, languages: &[String], agent: Box<dyn Agent>) -> Result<()> {
        // High-level orchestration
    }
}
```

**Benefits:**
- Clean separation of concerns
- Easier to understand API
- Components can evolve independently

### 4. Command Pattern - Exercise Execution

**Purpose:** Encapsulate execution with proper lifecycle management.

```rust
// Exercise execution flow:
// 1. prepare() - Copy files, create temp directory
// 2. execute() - Run agent, capture output
// 3. cleanup() - Remove temp files
// All wrapped in Result for error handling
```

### 5. Observer Pattern - Progress Tracking

**Purpose:** Stream progress updates to multiple subscribers.

```rust
// In benchmark-web/src/sse.rs
// Uses Axum's SSE (Server-Sent Events) for real-time updates:
// - CLI mode: Direct stdout streaming
// - Web UI: SSE stream with progress events
// - Logging: Structured JSON logs
```

---

## Configuration Architecture

### Hierarchical Configuration

```yaml
# config.yaml
benchmark:
  path: ../polyglot-benchmark     # Path to exercise repo
  parallelism: 4                  # Concurrent executions

docker:
  image: llm-benchmark/runner:latest
  memory: 2g                      # Container memory limit
  timeout: 300                    # Execution timeout (seconds)

output:
  results_dir: ../benchmark-results
  log_level: INFO

agents:
  claude:
    cli_path: /usr/local/bin/claude
    model: sonnet
```

### Configuration Loading Flow

```
YAML File → ConfigLoader → Typed Objects
                            ├── DockerConfig
                            ├── OutputConfig
                            └── AgentConfig
```

---

## Error Handling Strategy

### Exception Hierarchy

```
BenchmarkException (base)
├── BenchmarkExecutionException    # General execution failures
├── ExerciseNotFoundException       # Missing exercise
└── DockerExecutionException        # Docker-related failures
```

### Fail-Fast Principle

- Validation happens early (at startup and before execution)
- Exceptions thrown instead of returning null
- Detailed error messages with context

---

## Testing Strategy

### Unit Tests
- Test individual components in isolation
- Mock external dependencies (Docker, file system)
- Focus on business logic

### Integration Tests
- Test component interactions
- Use test containers for Docker integration
- Verify end-to-end flows

### Test Coverage Targets
- Core logic: > 80%
- Web layer: > 70%
- Infrastructure: > 60%

---

## Deployment Architecture

### Standalone Mode (CLI)
```
┌─────────────────────┐
│   Rust Binary       │
│   └── llm-benchmark │
└─────────────────────┘
         │
    ┌────┴────┐
    ▼         ▼
┌───────┐ ┌──────────┐
│ Docker│ │ Filesystem│
└───────┘ └──────────┘
```

### Web Mode
```
┌─────────────────────────────┐
│   Axum Web Server           │
│  ┌───────────────────────┐  │
│  │    Tokio Runtime      │  │
│  │  ┌─────────────────┐  │  │
│  │  │ REST Handlers   │  │  │
│  │  └─────────────────┘  │  │
│  └───────────────────────┘  │
└─────────────────────────────┘
         │           │
    ┌────┴────┐     ▼
    ▼         ▼   ┌──────────┐
┌───────┐ ┌──────────┐ │ In-Memory│
│ Docker│ │ Filesystem│ │ Sessions │
└───────┘ └──────────┘ └──────────┘
```

---

## Performance Considerations

### Parallelism
- Configurable concurrent exercise execution
- Resource-limited by Docker container settings
- Queue-based processing for controlled throughput

### Memory Management
- Temporary directories cleaned up after each exercise
- Sessions removed after completion/timeout
- Results persisted to disk (not held in memory)

### Timeout Handling
- Per-exercise timeout (configurable)
- Container-level timeout enforcement
- Graceful shutdown on cancellation

---

## Security Considerations

### Docker Isolation
- Exercises run in isolated containers
- Volume mounts limited to exercise directories only
- No network access from containers

### File System Safety
- Temporary directories created per execution
- Cleanup on completion or failure
- No writes outside designated result directory

---

## Future Enhancements

1. **Distributed Execution:** Support for running benchmarks across multiple machines
2. **Database Storage:** Replace file-based persistence with database backend
3. **Real-time Monitoring:** WebSocket-based progress updates
4. **Agent Comparison:** Built-in A/B testing for different agent configurations
5. **Caching:** Cache reference implementations and test results

---

## Related Documentation

- [Configuration Reference](CONFIGURATION.md)
- [API Documentation](API.md)
- [Developer Guide](DEVELOPER.md)
- [Result Format](RESULT_FORMAT.md)

---

**Version:** 1.0  
**Last Updated:** 2026-02-28  
**Maintained by:** Development Team
