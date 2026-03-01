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

- **Java 21** or later
- **Maven 3.8+**
- **Docker** (for running exercises)
- **Git**

### Clone the Repository

```bash
git clone https://github.com/your-org/claude-benchmark.git
cd claude-benchmark
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
  image: claude-benchmark/runner:latest
  memory: 2g

output:
  results_dir: ./results
```

3. Build the Docker runner image:

```bash
cd docker
docker build -t claude-benchmark/runner:latest -f Dockerfile.runner .
cd ..
```

---

## Building the Project

### Quick Build

```bash
mvn package -q
```

This creates `target/claude-benchmark-1.0-SNAPSHOT.jar`

### Build with Tests

```bash
mvn clean package
```

### Build Without Tests (for development)

```bash
mvn package -DskipTests -q
```

### Build Docker Image

```bash
mvn package -q
docker build -t claude-benchmark/runner:latest -f docker/Dockerfile.runner .
```

---

## Running the Application

### CLI Mode

```bash
# Run reference agent for Java exercises
java -jar target/claude-benchmark-1.0-SNAPSHOT.jar \
  --agent=reference \
  --languages=java

# Run Claude agent for specific exercise
java -jar target/claude-benchmark-1.0-SNAPSHOT.jar \
  --agent=claude \
  --languages=python \
  --exercise=two-fer
```

### Web Mode

```bash
# Start web server
java -jar target/claude-benchmark-1.0-SNAPSHOT.jar --web

# Access dashboard at http://localhost:8080
```

### Development Mode (with Spring Boot)

```bash
mvn spring-boot:run
```

---

## Running Tests

### Run All Tests

```bash
mvn test
```

### Run Specific Test Class

```bash
mvn test -Dtest=ResultPersisterTest
```

### Run Tests with Coverage

```bash
mvn clean test jacoco:report
# Report at target/site/jacoco/index.html
```

### Integration Tests

```bash
mvn verify -Dit.test=DockerIntegrationTest
```

---

## Adding New Languages

The benchmark supports multiple languages through the **Strategy Pattern** with `LanguageHandler` implementations.

### Step 1: Create Language Handler

Create a new handler in `src/main/java/com/benchmark/agent/handlers/`:

```java
package com.benchmark.agent.handlers;

import com.benchmark.agent.LanguageHandler;
import com.benchmark.exercise.Exercise;
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

Update `LanguageHandlerRegistry.java`:

```java
public LanguageHandlerRegistry() {
    // Register all built-in handlers
    register(new JavaHandler());
    register(new GoHandler());
    register(new JavaScriptHandler());
    register(new PythonHandler());
    register(new RustHandler());
    register(new CppHandler());
    register(new RubyHandler());  // Add new handler
    
    logger.info("Registered {} language handlers", handlers.size());
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
docker build -t claude-benchmark/runner:latest -f docker/Dockerfile.runner .
```

### Step 4: Test Your Handler

```bash
mvn test -Dtest=RubyHandlerTest
```

---

## Adding New Agents

Agents implement the `Agent` interface and are created via `AgentFactory`.

### Step 1: Create Agent Implementation

```java
package com.benchmark.agent;

import com.benchmark.config.Config;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

public class GeminiAgent implements Agent {
    private static final Logger logger = LoggerFactory.getLogger(GeminiAgent.class);
    
    private final Config config;
    private final String model;

    public GeminiAgent(Config config, String model) {
        this.config = config;
        this.model = model;
    }

    @Override
    public AgentResult run(Exercise exercise, Path exerciseDir, Path resultDir) {
        logger.info("Running Gemini agent for {} in {}", exercise.getName(), exercise.getLanguage());
        
        // Implement agent logic here
        // 1. Prepare prompt with exercise description
        // 2. Call Gemini API
        // 3. Write solution to exercise directory
        // 4. Return result
        
        return AgentResult.builder()
            .exerciseName(exercise.getName())
            .success(true)
            .build();
    }

    @Override
    public String getAgentType() {
        return "gemini";
    }
}
```

### Step 2: Create Agent Factory

```java
package com.benchmark.agent;

import com.benchmark.config.Config;
import org.springframework.stereotype.Component;

@Component
public class GeminiAgentFactory implements AgentFactory {
    
    @Override
    public Agent createAgent(Config config) {
        return new GeminiAgent(config, "gemini-pro");
    }

    @Override
    public String getAgentType() {
        return "gemini";
    }
}
```

### Step 3: Register Factory

The factory is automatically discovered via Spring's component scanning. Ensure it's in a package scanned by Spring.

---

## Code Style

### Java Code Style

We follow the [Google Java Style Guide](https://google.github.io/styleguide/javaguide.html) with minor modifications:

- **Indentation:** 4 spaces (no tabs)
- **Line length:** 120 characters
- **Braces:** K&R style for classes/methods, Allman for control structures
- **Imports:** Grouped as `java.*`, `javax.*`, third-party, then project packages

### Formatting

Use Maven's formatter plugin:

```bash
mvn formatter:format
```

### Linting

Run checkstyle before committing:

```bash
mvn checkstyle:check
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
src/
├── main/java/com/benchmark/
│   ├── BenchmarkRunner.java          # Main orchestration
│   ├── agent/                        # Agent implementations
│   ├── config/                       # Configuration
│   ├── docker/                       # Docker integration
│   ├── exception/                    # Custom exceptions
│   ├── exercise/                     # Exercise handling
│   ├── model/                        # Domain models
│   ├── persistence/                  # Persistence layer
│   └── web/                          # Web layer
├── test/java/com/benchmark/          # Test classes
└── resources/                        # Config files, templates
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
java -jar target/*.jar --log-level=DEBUG
```

### Debug Docker Containers

Enable verbose Docker output:

```java
// In DockerClient.java
ProcessBuilder pb = new ProcessBuilder("docker", "run", "--rm", "-it", ...);
pb.redirectErrorStream(true);
```

### IDE Setup

**IntelliJ IDEA:**
1. Import as Maven project
2. Enable annotation processing
3. Set Java 21 SDK
4. Configure code style (File → Settings → Editor → Code Style → Java → Set from → Predefined Style → Google)

**VS Code:**
1. Install Extension Pack for Java
2. Open workspace folder
3. Select JDK 21

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

**Symptom:** `java.lang.OutOfMemoryError`

**Solution:**
```bash
# Increase JVM heap for the benchmark runner
java -Xmx4g -jar target/*.jar

# Increase Docker container memory
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
