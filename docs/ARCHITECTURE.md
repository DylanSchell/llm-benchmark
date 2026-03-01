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

## Package Structure

```
src/main/java/com/benchmark/
├── BenchmarkRunner.java          # Main orchestration class
├── CliEntryPoint.java            # CLI entry point
├── CliArgs.java                  # CLI arguments record
│
├── agent/                        # Agent implementations
│   ├── LanguageHandler.java      # Strategy interface
│   ├── LanguageHandlerRegistry.java
│   ├── ReferenceAgent.java       # Reference implementation agent
│   ├── ClaudeAgent.java          # Claude Code CLI agent
│   └── handlers/                 # Language-specific handlers
│       ├── JavaHandler.java
│       ├── GoHandler.java
│       ├── JavaScriptHandler.java
│       ├── PythonHandler.java
│       ├── RustHandler.java
│       └── CppHandler.java
│
├── config/                       # Configuration management
│   ├── Config.java               # Main configuration class
│   ├── ConfigLoader.java         # YAML loader
│   ├── DockerConfig.java         # Docker settings
│   └── OutputConfig.java         # Output settings
│
├── docker/                       # Docker integration
│   └── DockerClient.java         # Container management
│
├── exception/                    # Exception hierarchy
│   ├── BenchmarkException.java
│   ├── BenchmarkExecutionException.java
│   ├── ExerciseNotFoundException.java
│   └── DockerExecutionException.java
│
├── exercise/                     # Exercise handling
│   ├── Exercise.java             # Exercise domain model
│   ├── ExerciseRunner.java       # Single exercise execution
│   └── LanguageExercise.java     # Language-specific exercise
│
├── model/                        # Domain models
│   ├── ExerciseResult.java       # Execution result
│   ├── BenchmarkSession.java     # Web session state
│   └── ...                       # Other models
│
├── persistence/                  # Persistence layer
│   └── ResultPersister.java      # File-based storage
│
└── web/                          # Web layer
    ├── config/                   # Spring configuration
    │   └── WebConfig.java
    ├── controller/               # REST controllers
    │   ├── BenchmarkController.java
    │   ├── ResultController.java
    │   ├── ExerciseController.java
    │   └── QueueController.java
    ├── domain/                   # Web-specific models
    │   ├── BenchmarkSession.java
    │   └── BenchmarkQueueItem.java
    └── service/                  # Service layer
        ├── BenchmarkService.java
        ├── SessionManager.java
        ├── BenchmarkExecutor.java
        ├── QueueProcessor.java
        └── ResultService.java
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

**Purpose:** Encapsulate language-specific operations behind a common interface.

```java
public interface LanguageHandler {
    void copyReference(Exercise exercise, Path tempDir) throws IOException;
    void copyTests(Exercise exercise, Path sourceDir, Path destDir) throws IOException;
    List<String> getTestCommand(Exercise exercise);
    void patchTests(Path tempWorkDir) throws IOException;
}
```

**Benefits:**
- Easy to add new language support
- Each handler knows only about its language
- No if/else chains in ReferenceAgent

### 2. Factory Pattern - Agent Creation

**Purpose:** Create agents without exposing instantiation logic.

```java
public interface AgentFactory {
    Agent createAgent(String agentType, Config config);
}

// Implementations:
ReferenceAgentFactory, ClaudeAgentFactory
```

**Benefits:**
- Type-safe agent creation
- Easy to add new agent types
- Testable (can mock factories)

### 3. Facade Pattern - BenchmarkService

**Purpose:** Provide simplified interface to complex subsystem.

```java
@Service
public class BenchmarkService {
    private final SessionManager sessionManager;
    private final BenchmarkExecutor benchmarkExecutor;
    private final QueueProcessor queueProcessor;
    
    // Simple facade methods delegate to specialized services
}
```

**Benefits:**
- Clean separation of concerns
- Easier to understand API
- Services can evolve independently

### 4. Command Pattern - Exercise Execution

**Purpose:** Encapsulate execution as commands with rollback support.

```java
// Exercise execution is encapsulated with:
// - Pre-execution setup (copy files, prepare environment)
// - Execution (run agent, run tests)
// - Post-execution (cleanup, persist results)
```

### 5. Observer Pattern - Progress Tracking

**Purpose:** Stream progress updates to multiple subscribers.

```java
// OutputConsumer allows streaming output to:
// - Console (CLI mode)
// - SSE stream (web UI)
// - Log file
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
  image: claude-benchmark/runner:latest
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
│   Java Application  │
│   └── BenchmarkRunner.main()
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
│   Spring Boot Application   │
│  ┌───────────────────────┐  │
│  │    Embedded Tomcat    │  │
│  │  ┌─────────────────┐  │  │
│  │  │ REST Controllers│  │  │
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
