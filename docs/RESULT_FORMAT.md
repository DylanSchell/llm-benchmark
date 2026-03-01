# Result Format Documentation

This document describes the file formats used for storing benchmark results.

---

## Directory Structure

Results are stored in a hierarchical directory structure:

```
results/
└── {model}-{sequence}/
    ├── trace_{language}_{exercise}.jsonl      # Agent interaction trace
    ├── result_{language}_{exercise}.json      # Exercise result
    └── ...
```

### Example

```
results/sonnet-1/
├── trace_java_two-fer.jsonl
├── result_java_two-fer.json
├── trace_python_hello-world.jsonl
├── result_python_hello-world.json
├── trace_javascriptexercism.jsonl
└── result_javascript_exercism.json
```

---

## Result Directory Naming

Result directories follow the pattern: `{model}-{sequence}`

- **model**: The AI model used (e.g., `sonnet`, `haiku`, `opus`)
- **sequence**: Incrementing run number for that model

### Example Sequences

```
sonnet-1/     # First run with Sonnet model
sonnet-2/     # Second run with Sonnet model
haiku-1/      # First run with Haiku model
opus-1/       # First run with Opus model
```

The sequence number is determined by scanning existing result directories and incrementing the highest sequence for that model.

---

## Exercise Result JSON

Each exercise execution produces a `result_{language}_{exercise}.json` file.

### Schema

```json
{
  "type": "ExerciseResult",
  "exerciseName": "two-fer",
  "language": "java",
  "agent": "claude",
  "model": "sonnet",
  "success": true,
  "exitCode": 0,
  "duration": 45.234,
  "startTime": "2026-02-28T10:30:00Z",
  "endTime": "2026-02-28T10:30:45Z",
  "output": "...",
  "errorMessage": null,
  "traceFile": "results/sonnet-1/trace_java_two-fer.jsonl"
}
```

### Fields

| Field | Type | Description |
|-------|------|-------------|
| `type` | string | Always `"ExerciseResult"` |
| `exerciseName` | string | Name of the exercise (e.g., "two-fer") |
| `language` | string | Programming language (e.g., "java", "python") |
| `agent` | string | Agent type used ("reference" or "claude") |
| `model` | string | Model name (null for reference agent) |
| `success` | boolean | Whether tests passed |
| `exitCode` | integer | Test command exit code (0 = success) |
| `duration` | number | Execution time in seconds (double) |
| `startTime` | number | Start timestamp as epoch seconds with nanoseconds (e.g., 1772279722.696278) |
| `endTime` | number | End timestamp as epoch seconds with nanoseconds (e.g., 1772279722.696946) |
| `output` | string | Combined stdout/stderr output |
| `errorMessage` | string or null | Error message if failed |
| `traceFile` | string | Path to trace file |

**Note on Timestamps:**  
The `startTime` and `endTime` fields are stored as epoch seconds with fractional nanoseconds (e.g., `1772279722.696278000`). In the web UI, these are automatically converted to ISO 8601 format for display (e.g., "2026-03-01T12:45:30.696Z").

### Success Examples

```json
{
  "type": "ExerciseResult",
  "exerciseName": "two-fer",
  "language": "java",
  "agent": "reference",
  "model": null,
  "success": true,
  "exitCode": 0,
  "duration": 12.5,
  "startTime": 1772279400.123456,
  "endTime": 1772279412.654321,
  "output": "\n[INFO] BUILD SUCCESS\n[INFO] Tests run: 5, Failures: 0\n",
  "errorMessage": null,
  "traceFile": "results/sonnet-1/trace_java_two-fer.jsonl"
}
```

**Note:** Timestamps are epoch seconds with nanoseconds. In the web UI, `1772279400.123456` displays as "2026-03-01T12:43:20.123Z".

### Failure Examples

```json
{
  "type": "ExerciseResult",
  "exerciseName": "hello-world",
  "language": "python",
  "agent": "claude",
  "model": "sonnet",
  "success": false,
  "exitCode": 1,
  "duration": 30.2,
  "startTime": 1772279700.987654,
  "endTime": 1772279730.123456,
  "output": "...\nAssertionError: Expected 'Hello, World!' but got 'Hello'\n...",
  "errorMessage": "Test failed: test_hello_world",
  "traceFile": "results/sonnet-1/trace_python_hello-world.jsonl"
}
```

---

## Trace File Format (JSONL)

Trace files use JSON Lines format (one JSON object per line). Each line represents an event in the agent's interaction.

### Schema

```json
{"role": "user", "content": "...", "timestamp": "2026-02-28T10:30:00Z"}
{"role": "assistant", "content": "...", "timestamp": "2026-02-28T10:30:05Z"}
{"role": "tool_use", "name": "bash", "input": "...", "timestamp": "2026-02-28T10:30:10Z"}
{"role": "tool_result", "name": "bash", "output": "...", "timestamp": "2026-02-28T10:30:15Z"}
```

### Event Types

#### User Message

```json
{
  "role": "user",
  "content": "Implement the two-fer exercise. The function should return 'One for {name}, one for me.' where {name} is the input parameter.",
  "timestamp": 1772279400.123456
}
```

#### Assistant Message

```json
{
  "role": "assistant",
  "content": "I'll implement the two-fer function in Java. Let me start by reading the test file to understand the expected interface.",
  "timestamp": 1772279405.654321
}
```

#### Tool Use (Bash Command)

```json
{
  "role": "tool_use",
  "name": "bash",
  "input": "cat TwoFer.java",
  "timestamp": 1772279410.987654
}
```

#### Tool Result (Command Output)

```json
{
  "role": "tool_result",
  "name": "bash",
  "output": "public class TwoFer {\n    public static String twoFer(String name) {\n        // TODO: implement\n    }\n}",
  "timestamp": 1772279415.123456
}
```

#### Tool Use (File Write)

```json
{
  "role": "tool_use",
  "name": "write_file",
  "input": {
    "path": "TwoFer.java",
    "content": "public class TwoFer {\n    public static String twoFer(String name) {\n        if (name == null || name.isEmpty()) {\n            return \"One for me, one for me\";\n        }\n        return \"One for \" + name + \", one for me\";\n    }\n}"
  },
  "timestamp": 1772279420.654321
}
```

#### Tool Result (File Write)

```json
{
  "role": "tool_result",
  "name": "write_file",
  "output": "File written successfully",
  "timestamp": 1772279425.987654
}
```

#### Thinking Block

```json
{
  "role": "assistant",
  "thinking": "I need to handle the case where name is null or empty, returning 'One for me, one for me' in that case.",
  "timestamp": 1772279403.123456
}
```

### Complete Trace Example

```jsonl
{"role": "user", "content": "Implement the two-fer exercise...", "timestamp": 1772279400.123456}
{"role": "assistant", "thinking": "Let me understand the requirements first...", "timestamp": 1772279401.234567}
{"role": "assistant", "content": "I'll start by reading the test file...", "timestamp": 1772279402.345678}
{"role": "tool_use", "name": "bash", "input": "cat TwoFerTest.java", "timestamp": 1772279403.456789}
{"role": "tool_result", "name": "bash", "output": "@Test\npublic void testTwoFerWithName() {\n    assertEquals(\"One for Sarah, one for me\", TwoFer.twoFer(\"Sarah\"));\n}", "timestamp": 1772279404.567890}
{"role": "assistant", "content": "Now I understand the expected behavior. Let me implement it...", "timestamp": 1772279405.678901}
{"role": "tool_use", "name": "write_file", "input": {"path": "TwoFer.java", "content": "..."}, "timestamp": 1772279410.789012}
{"role": "tool_result", "name": "write_file", "output": "File written successfully", "timestamp": 1772279411.890123}
{"role": "assistant", "content": "Let me run the tests to verify...", "timestamp": 1772279412.901234}
{"role": "tool_use", "name": "bash", "input": "mvn test -q", "timestamp": 1772279413.012345}
{"role": "tool_result", "name": "bash", "output": "[INFO] BUILD SUCCESS\n[INFO] Tests run: 5, Failures: 0", "timestamp": 1772279445.123456}
```

**Note on Trace Timestamps:**  
All `timestamp` fields in trace files are epoch seconds with nanoseconds (e.g., `1772279400.123456`). These represent the exact moment each event occurred during the agent's execution.

---

## Aggregated Results

The `BenchmarkResultAnalyzer` generates a summary report from all result files.

### Report Structure (`results.md`)

```markdown
# Benchmark Results

**Model:** sonnet  
**Run:** 2026-02-28T10:00:00Z  
**Total Exercises:** 20  
**Success Rate:** 95%

## Summary by Language

| Language | Total | Passed | Failed | Success Rate |
|----------|-------|--------|--------|--------------|
| Java     | 5     | 5      | 0      | 100%         |
| Python   | 5     | 5      | 0      | 100%         |
| JavaScript | 5   | 4      | 1      | 80%          |
| Go       | 5     | 5      | 0      | 100%         |

## Detailed Results

### Java - two-fer ✅
- **Duration:** 12.5s
- **Exit Code:** 0

### Java - hello-world ✅
- **Duration:** 8.2s
- **Exit Code:** 0

### JavaScript - exercism ❌
- **Duration:** 45.3s
- **Exit Code:** 1
- **Error:** Test failed: test_exercism_basic
```

---

## File Naming Conventions

| Pattern | Example | Description |
|---------|---------|-------------|
| `trace_{language}_{exercise}.jsonl` | `trace_java_two-fer.jsonl` | Agent interaction trace |
| `result_{language}_{exercise}.json` | `result_java_two-fer.json` | Exercise execution result |

**Rules:**
- Language: lowercase (java, python, javascript)
- Exercise: original exercise name from repo (two-fer, hello-world)
- Extensions: `.jsonl` for traces, `.json` for results

---

## Parsing Results

### Java Example

```java
Path resultDir = Paths.get("results/sonnet-1");

// Read exercise result
String json = Files.readString(resultDir.resolve("result_java_two-fer.json"));
ExerciseResult result = objectMapper.readValue(json, ExerciseResult.class);

if (result.success()) {
    System.out.println("✓ " + result.exerciseName() + " passed!");
} else {
    System.err.println("✗ " + result.exerciseName() + " failed: " + result.errorMessage());
}
```

### Python Example

```python
import json
from pathlib import Path

result_dir = Path("results/sonnet-1")

# Read exercise result
with open(result_dir / "result_java_two-fer.json") as f:
    result = json.load(f)

if result["success"]:
    print(f"✓ {result['exerciseName']} passed!")
else:
    print(f"✗ {result['exerciseName']} failed: {result['errorMessage']}")
```

### Parse Trace File

```python
import json
from pathlib import Path
from datetime import datetime

trace_file = Path("results/sonnet-1/trace_java_two-fer.jsonl")

with open(trace_file) as f:
    for line in f:
        event = json.loads(line)
        if event["role"] == "tool_result":
            # Convert epoch timestamp to readable format
            ts = datetime.fromtimestamp(event['timestamp'])
            print(f"[{ts}] Tool output: {event['output'][:100]}...")
```

### Converting Epoch Timestamps

Timestamps in result files are stored as epoch seconds with nanoseconds. To convert them:

**Python:**
```python
from datetime import datetime

epoch_seconds = 1772279400.123456
dt = datetime.fromtimestamp(epoch_seconds)
print(dt.isoformat())  # "2026-03-01T12:43:20.123456"
```

**Java:**
```java
import java.time.Instant;

double epochSeconds = 1772279400.123456;
long seconds = (long) epochSeconds;
int nanos = (int) ((epochSeconds - seconds) * 1_000_000_000);
Instant instant = Instant.ofEpochSecond(seconds, nanos);
System.out.println(instant.toString());  // "2026-03-01T12:43:20.123456Z"
```

---

## Result Validation

### Validating a Complete Run

A complete benchmark run should have:

1. ✅ Result file for each exercise
2. ✅ Trace file for each exercise (if enabled)
3. ✅ No duplicate files
4. ✅ Consistent naming convention

### Validation Script

```bash
#!/bin/bash
# validate_results.sh

RESULTS_DIR="results/sonnet-1"
EXERCISES=("two-fer" "hello-world" "exercism")
LANGUAGES=("java" "python" "javascript")

for lang in "${LANGUAGES[@]}"; do
    for exercise in "${EXERCISES[@]}"; do
        result_file="$RESULTS_DIR/result_${lang}_${exercise}.json"
        trace_file="$RESULTS_DIR/trace_${lang}_${exercise}.jsonl"
        
        if [ ! -f "$result_file" ]; then
            echo "❌ Missing: $result_file"
        fi
        
        if [ ! -f "$trace_file" ]; then
            echo "⚠️  Missing trace: $trace_file"
        fi
    done
done

echo "Validation complete!"
```

---

## Migration Guide

### From Old Format to New Format

Old format used flat directory structure. Migrate with:

```bash
#!/bin/bash
# migrate_results.sh

OLD_DIR="old-results"
NEW_DIR="results/sonnet-1"
mkdir -p "$NEW_DIR"

for file in "$OLD_DIR"/*.json; do
    if [ -f "$file" ]; then
        filename=$(basename "$file")
        # Extract language and exercise from filename
        # Example: java_two-fer_result.json -> result_java_two-fer.json
        mv "$file" "$NEW_DIR/result_$filename"
    fi
done
```

---

## Related Documentation

- [Architecture Overview](ARCHITECTURE.md)
- [API Documentation](API.md)
- [Configuration Reference](CONFIGURATION.md)
- [Developer Guide](DEVELOPER.md)

---

**Version:** 1.0  
**Last Updated:** 2026-02-28
