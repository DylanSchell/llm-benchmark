# Result Format Documentation

This document describes the file formats used for storing benchmark results.

---

## Directory Structure

Results are stored in a hierarchical directory structure:

```
results/
└── {agent}-{model}/
    ├── result_{agent}_{language}_{exercise}.json   # Exercise result
    ├── trace_{language}_{exercise}.jsonl           # Agent trace (pi session log)
    ├── trace_{language}_{exercise}.html            # The same trace, rendered (pi only)
    └── log_pi_{language}_{exercise}.jsonl          # pi's own log files (pi only)
```

### Example

```
results/pi-claude-sonnet-5/
├── result_pi_java_series.json
├── result_pi_python_hello-world.json
├── trace_java_series.jsonl
└── trace_java_series.html
```

---

## Result Directory Naming

Result directories are named `{agent}-{model}`:

- **agent**: `reference`, `pi` or `claude`
- **model**: the model label from `--model` or `model` in `config.yaml`. The reference agent always
  records `reference` here, because it does not call a model at all

### Examples

```
pi-claude-sonnet-5/       # pi against a Claude model
pi-qwen36-35b-a3b/        # pi against a local model
reference-reference/      # the reference baseline
```

There is no sequence number. Runs of the same agent-model pair share one directory and
accumulate results in it, which is why `attempts` exists and why `--retry` is a separate flag.

---

## Exercise Result JSON

Each exercise execution produces a `result_{agent}_{language}_{exercise}.json` file.

### Schema

Serialized from `AgentResult` (`crates/benchmark-types/src/agent/mod.rs`). A current record,
verbatim from a real run:

```json
{
  "attempts": 1,
  "cachedInputTokens": 0,
  "containerId": "bench-55b9aff1be2e",
  "duration": 4.915,
  "endTime": "2026-09-14T11:36:55.752601+00:00",
  "exerciseName": "series",
  "exitCode": 0,
  "input_tokens": 0,
  "language": "java",
  "model": "reference",
  "output": "\n",
  "output_tokens": 0,
  "startTime": "2026-09-14T11:36:50.836231+00:00",
  "success": true,
  "trace": null,
  "uncachedInputTokens": 0
}
```

### Fields

| Field | Type | Notes |
|---|---|---|
| `exerciseName` | string | Exercise name, e.g. `series` |
| `language` | string | `java`, `python`, `go`, … |
| `success` | bool | Whether the test command exited 0 |
| `exitCode` | int | Exit code of the test command |
| `output` | string | Combined stdout/stderr |
| `duration` | float | Seconds. Stored internally as milliseconds, serialized as seconds |
| `startTime` / `endTime` | string | ISO 8601, e.g. `2026-09-14T11:36:50.836231+00:00` |
| `errorMessage` | string | **Absent entirely** when there is no error — not `null` |
| `containerId` | string | Container that ran it, e.g. `bench-55b9aff1be2e` |
| `attempts` | int | `1` on a first run; incremented by `--retry` |
| `model` | string | Model label, or `reference` for the reference agent |
| `trace` | string or null | Path to the trace file, when one was written |
| `input_tokens` | int | Prompt tokens consumed |
| `output_tokens` | int | Completion tokens produced |
| `cachedInputTokens` | int | Input tokens served from the provider's cache |
| `uncachedInputTokens` | int | Input tokens that were not cached |

Things that have caught people out:

- **There is no `type` field and no `agent` field.** An earlier revision of this document
  claimed both. The agent is encoded in the directory (`pi-qwen3-coder/`) and the file name
  (`result_pi_java_series.json`).
- **The trace field is `trace`, not `traceFile`.**
- **Only `errorMessage` is omitted when empty.** `trace` is written as `null`; an older record
  may carry `""`.
- **The token fields mix naming conventions** — `input_tokens` and `output_tokens` but
  `cachedInputTokens` and `uncachedInputTokens` — because that is literally what the serde
  attributes say. Both spellings are accepted when reading.
- **Two generations of records exist on disk.** Current ones carry `containerId` and the token
  fields and use ISO 8601 timestamps; files written before those fields were added carry epoch
  seconds as numbers and omit them. `AgentResult` reads both: every field beyond the original
  set has a `default`, and `deserialize_timestamp` converts a number to RFC 3339 (treating a
  value over `1e12` as milliseconds). Anything you write yourself should do the same.

### Success Example

The record above is a success. The reference agent's `output` is often just a newline and its
token counts are zero, because it copies the reference solution rather than calling a model.

### Failure Example

An older-generation failure, verbatim — note the epoch timestamps and the absent
`containerId`/token fields:

```json
{
  "exerciseName": "matrix",
  "language": "go",
  "success": false,
  "exitCode": 2,
  "output": "\n# matrix [matrix.test]\n./matrix_test.go:280:17: cannot use New(\"1 2 3 10 11\\n4 5 6 11 12\") (value of type *Matrix) as type Matrix in assignment\nFAIL\tmatrix [build failed]\n",
  "duration": 95.093747,
  "startTime": 1777200670.546966,
  "endTime": 1777200765.640713,
  "errorMessage": "# matrix [matrix.test]\n./matrix_test.go:280:17: cannot use New(…) as type Matrix in assignment\nFAIL\tmatrix [build failed]\n",
  "trace": "",
  "model": "qwen36-35b-a3b-q4-q4kv-no-thinking",
  "attempts": 1
}
```

On a failure, `errorMessage` usually repeats the tail of `output` — the reporter prefers
`errorMessage` when present.

## Trace File Format (JSONL)

Written by agent runs. The file is the container's pi session log, copied out verbatim, so it is
**pi's own format**, not a benchmark invention. It is JSON Lines — one JSON object per line — and
it is a **tree, not a transcript**: every entry carries `id` and `parentId`, so a revised message
appends a new node instead of rewriting an old one. Version 3 is what the current runner writes.

The first line is always `session`:

```json
{"type":"session","version":3,"id":"019e2605-726e-70fb-9365-4d885efb33b5","timestamp":"2026-05-14T10:25:51.726Z","cwd":"/workspace"}
```

### Event types

| `type` | Fields | Meaning |
|---|---|---|
| `session` | `version`, `id`, `timestamp`, `cwd` | Once, first. `version` is the session-format version (currently `3`) |
| `model_change` | `id`, `parentId`, `timestamp`, `provider`, `modelId` | The model the session runs against |
| `thinking_level_change` | `id`, `parentId`, `timestamp`, `thinkingLevel` | pi's thinking level: `off`, `minimal`, `low`, `medium`, `high`, `xhigh` |
| `message` | `id`, `parentId`, `timestamp`, `message` | A user, assistant or tool-result turn |
| `custom` / `custom_message` | `id`, `parentId`, `timestamp`, … | Agent-specific extras |

Everything except `session` carries `id` and `parentId`; `parentId` is `null` only at the root of
a chain. Timestamps are **ISO 8601 strings** (`2026-05-14T10:25:51.765Z`), not epoch numbers.

### The `message` entry

It wraps a `{role, content}` object. `role` is one of `user`, `assistant` or `toolResult`, and
`content` is an array of typed parts:

```json
{"type":"message","id":"2cd3b56a","parentId":"07ef48a3","timestamp":"2026-05-14T10:25:51.765Z",
 "message":{"role":"assistant","content":[
   {"type":"text","text":"Let me list the files."},
   {"type":"toolCall","id":"XubtUWl2H9gqFKCZA1uVrMRWVzGkFdBY","name":"ls","arguments":{}}]}}
```

| Part `type` | Meaning |
|---|---|
| `text` | Natural language, from the user or the model |
| `toolCall` | A tool invocation: `id`, `name` (`ls`, `Read`, `Write`, …) and `arguments` |

### Reading a trace

```bash
# Every tool call, compact
jq -c 'select(.type=="message") | .message.content[]? | select(.type=="toolCall")' trace_java_series.jsonl

# Only the assistant's prose
jq -r 'select(.type=="message" and .message.role=="assistant")
       | .message.content[]? | select(.type=="text") | .text' trace_java_series.jsonl

# How many turns before the model stopped
jq -s '[.[] | select(.type=="message")] | length' trace_java_series.jsonl
```

To follow one branch, start at any entry and walk `parentId` back to the `session` line. Because
the log is append-only, two runs of the same exercise can be compared line by line — that is what
`compare` in the dashboard does.

**Note:** the reference agent writes no trace, so its `trace` field is `null` and the file is
absent. A trace exists only for agent runs.

## Aggregated Results

`llm-benchmark report` walks the results directory and writes one markdown file — `results.md` by
default, in the current directory:

```bash
llm-benchmark report --results-dir ../benchmark-results --output results.md
```

Both options have defaults (`--results-dir` is `../benchmark-results`, `--output` is `results.md`),
so a bare `llm-benchmark report` works from the repository root.

### Report Structure (`results.md`)

Every heading in the report is level 1; there is no nesting. Four groups of tables follow, in this
order:

| # | Heading | One row per | Sorted by |
| --- | --- | --- | --- |
| 1 | `# Benchmark Results Summary` | one results directory (`{agent}-{model}`) | result count, then success rate |
| 2 | `# Success rates per exercise` | one `{exercise}_{language}` | success rate |
| 3 | `# {benchmark}` (repeated) | one exercise within that benchmark | input order |
| 4 | `# {exercise}_{language}` (repeated) | one model that ran that exercise | duration, ascending |

```markdown
# Benchmark Results Summary

| Benchmark | Total Results | Success | Failed | Completion % | Total Duration | Input / Cached / Output |
|-----------|---------------|---------|--------|---------------|----------------|---------------------------|
| [pi-qwen36-35b-a3b-coding](#pi-qwen36-35b-a3b-coding) | 225 | 224 | 1 | 99.6% | 5h 33m 51s | 0 / 0 / 1.1M |

# Success rates per exercise

| Exercise | Total Results | Success | Failed | Completion % | Total Duration | Input / Cached / Output |
|----------|---------------|---------|--------|---------------|----------------|---------------------------|
| [proverb_python](#proverb_python) | 38 | 38 | 0 | 100.0% | 18m 8s | 0 / 0 / 0 |

# pi-qwen36-35b-a3b-coding

| Exercise | Success | Duration | Input / Cached / Output |
|----------|---------|----------|---------------------------|
| [say_javascript](#say_javascript) | ✅ | 17s | 0 / 0 / 1.1M |

# series_java

| Model | Success | Duration | Input / Cached / Output |
|-------|---------|----------|---------------------------|
| [pi-qwen36-35b-a3b-q4-no-thinking](#pi-qwen36-35b-a3b-q4-no-thinking) | ✅ | 18s | 0 / 0 / 0 |

*Generated by BenchmarkResultAnalyzer*
```

The example rows above are real, taken from a report over a full results directory. The one thing
not reproduced from a live file is the tokens column header: older reports show `Tokens` where the
current generator writes `Input / Cached / Output`.

**Details that are easy to get wrong:**

- **The token columns are uncached input / cached input / output,** despite the header reading
  `Input / Cached / Output` — the first value the generator passes is `uncachedInputTokens`. Values
  above 1000 are abbreviated to `K`/`M`/`G`.
- **The success markers are `✅` and `❌`, plus `⏰`.** `⏰` appears only in the per-benchmark tables,
  for a failure that ran at least 7199 s — just under the 2-hour timeout. The per-exercise tables
  use `✅`/`❌` only.
- **Names and anchors mangle `.` to `_` and `:` to `-`,** and every row links to its own anchor.
- **The report aggregates the whole results directory,** so one file mixes agents and models
  together. There is no per-agent report and no filtering option.
- **The footer is a leftover name.** `BenchmarkResultAnalyzer` is only a string the reporter writes;
  there is no such type in the codebase. The generator is the `benchmark-reporter` crate, which is
  what `llm-benchmark report` calls.
- **`results.md` is generated output and is gitignored,** not a source file.

---

## File Naming Conventions

| Pattern | Example | Description |
|---------|---------|-------------|
| `result_{agent}_{language}_{exercise}.json` | `result_pi_java_series.json` | Exercise execution result |
| `trace_{language}_{exercise}.jsonl` | `trace_java_series.jsonl` | Agent trace (pi session log) |
| `trace_{language}_{exercise}.html` | `trace_java_series.html` | The same trace, rendered |
| `log_pi_{language}_{exercise}.jsonl` | `log_pi_java_series.jsonl` | pi's own logs |

**Rules:**
- Agent: `reference`, `pi` or `claude` — in the result file name only, not the trace name
- Language: lowercase (`java`, `python`, `javascript`)
- Exercise: the upstream exercise name (`two-fer`, `hello-world`)
- Extensions: `.json` for results, `.jsonl` for traces and pi logs, `.html` for rendered traces

---

## Parsing Results

### jq Example

```bash
# One line per result
jq -r '"\\(.exerciseName)\\t\\(.language)\\t\\(.success)\\t\\(.duration)s"' results/*/result_*.json

# Everything that failed, with the reason
jq -r 'select(.success|not) | "\\(.exerciseName): \\(.errorMessage // "(no message)")"' results/*/result_*.json

# Token totals per model
jq -s 'group_by(.model) | map({model: .[0].model,
         input:  (map(.input_tokens)  | add),
         output: (map(.output_tokens) | add)})' results/*/result_*.json
```

The same thing in Rust, using the type from `benchmark-types`:

```rust
let text = std::fs::read_to_string(path)?;
let result: benchmark_types::agent::AgentResult = serde_json::from_str(&text)?;
if result.success {
    println!("{} passed in {:.1}s", result.exercise_name, result.duration_seconds());
} else if let Some(err) = &result.error_message {
    eprintln!("{} failed: {}", result.exercise_name, err);
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

with open("results/pi-claude-sonnet-5/trace_java_series.jsonl") as f:
    for line in f:
        event = json.loads(line)
        if event["type"] != "message":
            continue
        msg = event["message"]
        for part in msg["content"]:
            if part["type"] == "toolCall":
                print(f"{msg['role']} called {part['name']} at {event['timestamp']}")
```

### Timestamps

Result records come in two generations, and both are read into the same struct:

| Generation | `startTime` / `endTime` on disk | After parsing |
|---|---|---|
| Current | ISO 8601 string: `"2026-09-14T11:36:50.836231+00:00"` | Unchanged |
| Older records | Epoch seconds as a number: `1777200670.546966` | Converted to RFC 3339 by `deserialize_timestamp` |

A number above `1e12` is treated as milliseconds. Anything reading these files directly should
handle both shapes rather than assuming one.

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

## Migrating older result files

There is no directory migration to perform: older and newer records sit side by side in the same
directories and are read by the same struct. What differs is the record shape, described under
[Fields](#fields) and [Timestamps](#timestamps): newer files add `containerId`, `attempts` and the
token fields, and write ISO 8601 timestamps where the older ones wrote epoch seconds.
`AgentResult` gives every added field a default and normalizes numeric timestamps, so a directory
holding both generations needs no special handling.

---

## Related Documentation

- [Architecture Overview](ARCHITECTURE.md)
- [API Documentation](API.md)
- [Configuration Reference](CONFIGURATION.md)
- [Developer Guide](DEVELOPER.md)
