# Developer Guide

This guide provides everything you need to contribute to the Claude Benchmark Runner.

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

### Clone Polyglot Benchmark

```bash
git clone https://github.com/Aider-AI/polyglot-benchmark ../polyglot-benchmark
```

### Configure Environment

1. Copy the example configuration:

```bash
cp config.example.yaml config.yaml
```

2. Edit `config.yaml` with your settings:

```yaml
benchmark:
  path: ../polyglot-benchmark

docker:
  image: llm-benchmark/runner:latest
  memory: 2g

output:
  results_dir: ./results
```

3. Build the Docker runner image:

```bash
cd docker
docker build -t llm-benchmark/runner:latest -f Dockerfile.runner .
cd ..
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
docker build -t llm-benchmark/runner:latest -f docker/Dockerfile.runner .
```

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

The benchmark supports multiple languages through the **Strategy Pattern** with `LanguageHandler` implementations.

### Step 1: Create Language Handler

Create a new handler in `src/main/java/com/benchmark/agent/handlers/`:

```java
package io.schell.llm.benchmark.agent.handlers;

import io.schell.llm.benchmark.agent.LanguageHandler;
import io.schell.llm.benchmark.exercise.Exercise;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.StandardCopyOption;
import java.util.List;

public class RubyHandler implements LanguageHandler {
    private static final Logger logger = LoggerFactory.getLogger(RubyHandler.class);

    @Override
    public String getLanguage() {
        return "ruby";
    }

    @Override
    public void copyReference(Exercise exercise, Path tempDir) throws IOException {
        // Copy reference implementation files
        for (Path refPath : exercise.getReferencePath()) {
            if (refPath == null || !Files.exists(refPath)) continue;
            
            String fileName = refPath.getFileName().toString();
            Path destFile = tempDir.resolve(fileName);
            Files.copy(refPath, destFile, StandardCopyOption.REPLACE_EXISTING);
            logger.info("Copied Ruby reference file: {}", fileName);
        }
    }

    @Override
    public void copyTests(Exercise exercise, Path sourceDir, Path destDir) throws IOException {
        // Copy test files
        for (Path testPath : exercise.getTestPath()) {
            String fileName = testPath.getFileName().toString();
            Path destFile = destDir.resolve(fileName);
            Files.copy(testPath, destFile, StandardCopyOption.REPLACE_EXISTING);
            logger.info("Copied Ruby test file: {}", fileName);
        }
    }

    @Override
    public List<String> getTestCommand(Exercise exercise) {
        // Return command to run tests
        return List.of("bundle", "exec", "rspec");
    }

    @Override
    public void patchTests(Path tempWorkDir) throws IOException {
        // Remove skip/ignore annotations if needed
        logger.debug("No test patching needed for Ruby");
    }
}
```

### Step 2: Register Handler

Update `LanguageHandlerRegistry` in `benchmark-core`:

```rust
// In benchmark-core/src/handlers/registry.rs
pub struct LanguageHandlerRegistry {
    handlers: HashMap<String, Box<dyn LanguageHandler>>,
}

impl LanguageHandlerRegistry {
    pub fn new() -> Self {
        let mut registry = Self { handlers: HashMap::new() };
        
        // Register all built-in handlers
        registry.register(Box::new(JavaHandler));
        registry.register(Box::new(GoHandler));
        registry.register(Box::new(JavaScriptHandler));
        registry.register(Box::new(PythonHandler));
        registry.register(Box::new(RustHandler));
        registry.register(Box::new(CppHandler));
        registry.register(Box::new(RubyHandler));  // Add new handler
        
        info!("Registered {} language handlers", registry.handlers.len());
        registry
    }
    
    pub fn register(&mut self, handler: Box<dyn LanguageHandler>) {
        let language = handler.get_language().to_string();
        self.handlers.insert(language, handler);
    }
}
```

### Step 3: Update Docker Image

Add language runtime to `docker/Dockerfile.runner`:

```dockerfile
# Install Ruby
RUN apt-get update && apt-get install -y ruby-full bundler && rm -rf /var/lib/apt/lists/*
```

Rebuild the Docker image:

```bash
docker build -t llm-benchmark/runner:latest -f docker/Dockerfile.runner .
```

### Step 4: Test Your Handler

```bash
cargo test --package benchmark-core ruby_handler
```

---

## Adding New Agents

Agents implement the `Agent` trait and are created via a factory function.

### Step 1: Create Agent Implementation

```rust
// In benchmark-core/src/agents/gemini_agent.rs
use crate::agents::Agent;
use crate::config::Config;
use crate::exercise::Exercise;
use crate::result::AgentResult;
use tracing::info;

pub struct GeminiAgent {
    config: Config,
    model: String,
}

impl GeminiAgent {
    pub fn new(config: &Config, model: &str) -> Self {
        Self {
            config: config.clone(),
            model: model.to_string(),
        }
    }
}

impl Agent for GeminiAgent {
    fn run(&self, exercise: &Exercise, exercise_dir: &Path, result_dir: &Path) -> Result<AgentResult> {
        info!("Running Gemini agent for {} in {}", exercise.name, exercise.language);
        
        // Implement agent logic here
        // 1. Prepare prompt with exercise description
        // 2. Call Gemini API
        // 3. Write solution to exercise directory
        // 4. Return result
        
        Ok(AgentResult {
            exercise_name: exercise.name.clone(),
            success: true,
            // ... other fields
        })
    }

    fn agent_type(&self) -> &str {
        "gemini"
    }
}
```

### Step 2: Register Agent

Add the agent to the factory function in `benchmark-core/src/agents/mod.rs`:

```rust
pub fn create_agent(agent_type: &str, config: &Config) -> Result<Box<dyn Agent>> {
    match agent_type {
        "reference" => Ok(Box::new(ReferenceAgent::new(config))),
        "claude" => Ok(Box::new(ClaudeAgent::new(config))),
        "gemini" => Ok(Box::new(GeminiAgent::new(config, "gemini-pro"))),  // Add new agent
        _ => Err(BenchmarkError::UnknownAgent(agent_type.to_string())),
    }
}
```

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

Enable verbose Docker output:

```java
// In DockerClient.java
ProcessBuilder pb = new ProcessBuilder("docker", "run", "--rm", "-it", ...);
pb.redirectErrorStream(true);
```

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

1. Update version in `pom.xml`
2. Update changelog
3. Create release branch
4. Run full test suite
5. Build and push Docker image
6. Create Git tag
7. Publish release

---

## Related Documentation

- [Architecture Overview](ARCHITECTURE.md)
- [API Documentation](API.md)
- [Configuration Reference](CONFIGURATION.md)
- [Result Format](RESULT_FORMAT.md)

---

**Version:** 1.0  
**Last Updated:** 2026-02-28  
**Maintained by:** Development Team
