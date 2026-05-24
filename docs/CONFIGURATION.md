# Configuration Reference

This document describes all configuration options for the Claude Benchmark Runner.

---

## Configuration File Location

The configuration is loaded from `config.yaml` in the project root directory:

```
/Users/dylan/Developer/llm-benchmark/config.yaml
```

You can specify a custom config file location using the CLI:

```bash
./target/release/llm-benchmark --config=/path/to/custom-config.yaml run --language java
```

---

## Complete Configuration Example

```yaml
# ===========================================
# Claude Benchmark Runner Configuration
# ===========================================

# Benchmark settings
benchmark:
  # Path to the polyglot-benchmark repository
  path: ../polyglot-benchmark
  
  # Number of concurrent exercise executions
  parallelism: 4

# Docker container settings
docker:
  # Docker image for running exercises
  image: llm-benchmark/runner:latest
  
  # Container memory limit (e.g., "2g", "512m")
  memory: 2g
  
  # Execution timeout in seconds
  timeout: 300
  
  # Additional environment variables
  environment:
    - ANTHROPIC_MODEL=sonnet
    - ANTHROPIC_API_KEY=${ANTHROPIC_API_KEY}

# Output and logging settings
output:
  # Directory for storing results
  results_dir: ../benchmark-results
  
  # Log level (DEBUG, INFO, WARN, ERROR)
  log_level: INFO
  
  # Enable trace file generation
  generate_traces: true

# Agent configurations
agents:
  reference:
    enabled: true
    
  claude:
    enabled: true
    cli_path: /usr/local/bin/claude
    model: sonnet
    timeout: 600
    max_tokens: 4096
```

---

## Configuration Sections

### Benchmark Settings

| Property | Type | Default | Description |
|----------|------|---------|-------------|
| `benchmark.path` | string | `../polyglot-benchmark` | Path to the polyglot-benchmark repository |
| `benchmark.parallelism` | int | `4` | Number of concurrent exercise executions |

**Example:**
```yaml
benchmark:
  path: /opt/polyglot-benchmark
  parallelism: 8
```

---

### Docker Settings

| Property | Type | Default                       | Description |
|----------|------|-------------------------------|-------------|
| `docker.image` | string | `llm-benchmark/runner:latest` | Docker image for execution containers |
| `docker.memory` | string | `2g`                          | Container memory limit |
| `docker.timeout` | int | `300`                         | Execution timeout in seconds |
| `docker.environment` | string[] | `[]`                          | Additional environment variables |

**Memory Limits:**
- `512m` - 512 MB (lightweight exercises)
- `2g` - 2 GB (recommended default)
- `4g` - 4 GB (memory-intensive languages like Java)

**Timeout Values:**
- `60` - 1 minute (quick tests only)
- `300` - 5 minutes (recommended default)
- `600` - 10 minutes (for Claude agent)
- `1800` - 30 minutes (complex exercises)

**Example:**
```yaml
docker:
  image: llm-benchmark/runner:v1.2.0
  memory: 4g
  timeout: 600
  environment:
    - ANTHROPIC_MODEL=sonnet
    - NODE_ENV=test
```

---

### Output Settings

| Property | Type | Default | Description |
|----------|------|---------|-------------|
| `output.results_dir` | string | `../benchmark-results` | Directory for result files |
| `output.log_level` | string | `INFO` | Logging verbosity |
| `output.generate_traces` | boolean | `true` | Generate JSONL trace files |

**Log Levels:**
- `DEBUG` - Verbose debugging information
- `INFO` - General informational messages (recommended)
- `WARN` - Warning messages only
- `ERROR` - Error messages only

**Example:**
```yaml
output:
  results_dir: /data/benchmark-results
  log_level: DEBUG
  generate_traces: true
```

---

### Agent Configurations

#### Reference Agent

| Property | Type | Default | Description |
|----------|------|---------|-------------|
| `agents.reference.enabled` | boolean | `true` | Enable reference agent |

**Example:**
```yaml
agents:
  reference:
    enabled: true
```

#### Claude Agent

| Property | Type | Default | Description |
|----------|------|---------|-------------|
| `agents.claude.enabled` | boolean | `true` | Enable Claude agent |
| `agents.claude.cli_path` | string | `/usr/local/bin/claude` | Path to Claude CLI binary |
| `agents.claude.model` | string | `sonnet` | Default model to use |
| `agents.claude.timeout` | int | `600` | Agent execution timeout (seconds) |
| `agents.claude.max_tokens` | int | `4096` | Maximum tokens in response |

**Available Models:**
- `sonnet` - Claude 3.5 Sonnet (recommended for coding)
- `haiku` - Claude 3 Haiku (faster, less capable)
- `opus` - Claude 3 Opus (most capable, slower)

**Example:**
```yaml
agents:
  claude:
    enabled: true
    cli_path: /opt/claude/bin/claude
    model: sonnet
    timeout: 900
    max_tokens: 8192
```

---

## Environment Variable Substitution

Configuration values can reference environment variables using `${VARIABLE_NAME}` syntax:

```yaml
docker:
  environment:
    - ANTHROPIC_API_KEY=${ANTHROPIC_API_KEY}
    - GITHUB_TOKEN=${GITHUB_TOKEN:-default_token}
```

**Syntax:**
- `${VAR}` - Use value of `VAR` or fail if not set
- `${VAR:-default}` - Use value of `VAR` or `default` if not set
- `${VAR:-required}` - Use value of `VAR` or throw error if not set

---

## Validation Rules

### Required Properties

The following properties are required and will cause startup failure if missing:

```yaml
benchmark:
  path: <required>

docker:
  image: <required>
```

### Value Constraints

| Property | Valid Range | Constraint |
|----------|-------------|------------|
| `benchmark.parallelism` | 1-32 | Must be positive integer |
| `docker.timeout` | 60-3600 | Must be between 1 minute and 1 hour |
| `docker.memory` | Any valid Docker memory format | e.g., "512m", "2g" |
| `output.log_level` | DEBUG, INFO, WARN, ERROR | Case-insensitive |

---

## Configuration Classes

### Config.java

```java
public record Config(
    BenchmarkConfig benchmark,
    DockerConfig docker,
    OutputConfig output,
    Map<String, AgentConfig> agents
) {}
```

### DockerConfig.java

```java
public record DockerConfig(
    String image,
    String memory,
    int timeout,
    List<String> environment
) {}
```

### OutputConfig.java

```java
public record OutputConfig(
    String resultsDir,
    String logLevel,
    boolean generateTraces
) {}
```

---

## Example Configurations

### Minimal Configuration

```yaml
benchmark:
  path: ../polyglot-benchmark

docker:
  image: llm-benchmark/runner:latest

output:
  results_dir: ./results
```

### Production Configuration

```yaml
benchmark:
  path: /opt/polyglot-benchmark
  parallelism: 8

docker:
  image: llm-benchmark/runner:v1.2.0
  memory: 4g
  timeout: 600
  environment:
    - ANTHROPIC_API_KEY=${ANTHROPIC_API_KEY}
    - NODE_ENV=production

output:
  results_dir: /data/benchmark-results
  log_level: WARN
  generate_traces: true

agents:
  reference:
    enabled: true
  
  claude:
    enabled: true
    cli_path: /opt/claude/bin/claude
    model: sonnet
    timeout: 900
    max_tokens: 8192
```

### Development Configuration

```yaml
benchmark:
  path: ../polyglot-benchmark
  parallelism: 2

docker:
  image: llm-benchmark/runner:latest
  memory: 2g
  timeout: 300

output:
  results_dir: ./dev-results
  log_level: DEBUG
  generate_traces: true

agents:
  reference:
    enabled: true
  
  claude:
    enabled: false  # Disable Claude for unit testing
```

---

## Configuration Validation

The application validates configuration on startup. Common validation errors:

### Missing Required Properties

```
ConfigurationError: Missing required property 'benchmark.path'
```

**Solution:** Add the missing property to config.yaml

### Invalid Parallelism Value

```
ConfigurationError: benchmark.parallelism must be between 1 and 32, got 50
```

**Solution:** Set parallelism to a value between 1 and 32

### Invalid Memory Format

```
ConfigurationError: Invalid docker.memory format: 'invalid'
```

**Solution:** Use valid Docker memory format (e.g., "2g", "512m")

---

## Runtime Configuration Changes

### Environment Variables Override

You can override specific configuration values using environment variables:

```bash
export BENCHMARK_PATH=/custom/path
export DOCKER_MEMORY=4g
export OUTPUT_LOG_LEVEL=DEBUG
```

Format: `SECTION_PROPERTY` (uppercase, underscore-separated)

### CLI Overrides

CLI arguments take precedence over config file:

```bash
./target/release/llm-benchmark run --language java --verbose
```

---

## Best Practices

### 1. Use Environment Variables for Secrets

```yaml
# ❌ Don't hardcode secrets
docker:
  environment:
    - ANTHROPIC_API_KEY=sk-ant-...

# ✅ Use environment variable substitution
docker:
  environment:
    - ANTHROPIC_API_KEY=${ANTHROPIC_API_KEY}
```

### 2. Adjust Parallelism Based on Resources

```yaml
# For development machines (8GB RAM)
benchmark:
  parallelism: 2

# For dedicated benchmark servers (32GB+ RAM)
benchmark:
  parallelism: 8
```

### 3. Set Appropriate Timeouts

```yaml
# Quick validation runs
docker:
  timeout: 120

# Full benchmark runs with Claude agent
docker:
  timeout: 600
```

### 4. Use Separate Result Directories

```yaml
# Development
output:
  results_dir: ./dev-results

# Production
output:
  results_dir: /data/benchmark-results/$(date +%Y-%m-%d)
```

---

## Troubleshooting

### Configuration Not Loading

**Symptom:** Application starts with default values

**Check:**
1. Config file exists at expected location
2. File is valid YAML (use `yamllint` to validate)
3. No typos in property names

### Environment Variable Not Substituted

**Symptom:** `${VAR}` appears literally in config

**Check:**
1. Environment variable is set: `echo $VAR`
2. Syntax is correct: `${VAR}` not `$VAR` or `${VAR}`

### Invalid Memory Limit

**Symptom:** Docker fails to start containers

**Check:**
1. Format is valid: "512m", "2g", etc.
2. Value doesn't exceed system limits

---

## Related Documentation

- [Architecture Overview](ARCHITECTURE.md)
- [API Documentation](API.md)
- [Developer Guide](DEVELOPER.md)

---

**Version:** 1.0  
**Last Updated:** 2026-02-28
