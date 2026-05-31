# LLM Benchmark Improvement Roadmap

A holistic code review of the `llm-benchmark` codebase. Findings are organized
by severity and area, with concrete improvement suggestions.

---

## 1. Architecture & Module Boundaries

### 1.1 Duplicate type definitions (`AgentResult` vs `ExerciseResult`)
**Critical** — Two nearly identical result types exist in separate crates:

- `benchmark_types::agent::AgentResult` (used by agents at runtime)
- `benchmark_types::exercise::ExerciseResult` (used by deserialization/reporting)

`ExerciseResult` has token-tracking fields (`input_tokens`, `output_tokens`,
`cached_input_tokens`, `uncached_input_tokens`, `model`) that `AgentResult`
lacks. These should be unified into a single type that covers both the
runtime path and the reporting path. The `AgentResultBuilder` and
`ExerciseResultBuilder` are also duplicated.

**Suggestion:** Merge into one `BenchmarkResult` in `benchmark-types` with all
fields, using `#[serde(default)]` for optional deserialization fields. Place
the builder in `benchmark-types` only.

### 1.2 Overlapping Docker config types
Two `DockerConfig` types exist:

- `benchmark_types::config::DockerConfig` (deserialized from YAML, uses `Vec<HashMap>` for env)
- `benchmark_core::docker::DockerConfig` (runtime, uses `Option<HashMap>` for env)

Both carry the same fields (image, memory, timeout, work_dir, environment) with
slightly different representations. This forces conversion at every call site
(`runner.rs` line 40–46, `lib.rs` line 50–56).

**Suggestion:** Use a single `DockerConfig` in `benchmark-types` that both
deserializes from YAML and works at runtime. Make `environment` always a
`HashMap<String, String>` (flatten the `Vec<HashMap>` during `Deserialize`).

### 1.3 Agent crate boundary leaks
`benchmark-core` depends on `benchmark-types` (correct), but
`benchmark-web` imports heavily from `benchmark-core` internals like
`exercise_runner::ExerciseRunner`, `persistence::ResultPersister`, and
specific agent types (`ClaudeAgent`, `PiAgent`). The CLI runner (`benchmark-cli`)
also knows about concrete agent types.

**Suggestion:** Define a `BenchmarkFacade` or `BenchmarkEngine` in
`benchmark-core` that exposes a single high-level API: `run_benchmark(config,
opts)` and `analyze_results(dir)`. Consumers (CLI, web) should not need to
construct agents or Docker clients directly.

### 1.4 `benchmark-web/src/lib.rs` is a God function
The `run_web_server` function is **~180 lines** and does configuration
loading, service initialization, template engine setup, static file resolution,
and Axum router construction in one function.

**Suggestion:** Extract:
- Configuration loading → `config::load_app_config()`
- Service wiring → `services::wire_services(config)`
- Server setup → `server::start(app_state, port)`

### 1.5 Unused `parallel` module is a thin wrapper
`benchmark_core::parallel::ParallelExecutor` adds a semaphore-based concurrency
limit on top of `futures::future::join_all`, but `exercise_runner::run_all_exercises`
doesn't use it — it spawns all tasks at once via `tokio::spawn` with no
concurrency control. The `RateLimiter` struct is also never consumed.

**Suggestion:** Either wire `ParallelExecutor` into the exercise runner to
actually enforce `config.parallelism`, or remove the module and use
`tokio::sync::Semaphore` directly where needed.

### 1.6 Hardcoded `"reference"` vs `"claude"` vs `"pi"` agent dispatch
Agent dispatch in `benchmark_core/src/lib.rs:run_benchmark` and
`benchmark-cli/src/runner.rs:create_agent` uses hardcoded string matches.

**Suggestion:** Define an `AgentKind` enum in `benchmark-types` with
`FromStr`/`Deserialize`:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AgentKind {
    Reference,
    Claude,
    Pi,
}
```

Use `AgentKind` in `Agent::kind()` or as a separate method. Eliminate the
stringly-typed dispatch.

---

## 2. Correctness & Error Handling

### 2.1 Mutable-reference-through-immutable-reference pattern
`lib.rs:run_benchmark` creates agents inline and immediately passes `Arc<dyn
Agent>` to the runner, but the `DockerConfig` model update happens through
`set_model()` which takes `&mut self` — and this is called on a clone without
propagating the change back. The comment on line 80 of `exercise_runner/mod.rs`
acknowledges this: "In a production system, we'd store the modified client back."

**Suggestion:** Make `DockerClient` store its config behind `Arc<RwLock<DockerConfig>>`
so model updates are visible to all users. Alternatively, pass the model string
as a parameter to `run_command_with_limits_and_volume_with_callback` so it can
set env vars per-invocation without mutation.

### 2.2 `start_time` serialized differently in ReferenceAgent vs others
`ReferenceAgent::run_tests_in_docker` sets `start_time` to
`duration_ms.to_string()` (a number string like `"1234"`) while all other code
uses RFC3339 format (`"2026-05-30T17:29:10.696Z"`). The `ExerciseResult`
deserializer handles this via `deserialize_timestamp`, but `AgentResult` has no
custom deserializer — it will parse `"1234"` as an RFC3339 timestamp and fail.

**Fix:** Change `ReferenceAgent::run_tests_in_docker` line:
```rust
// Before
.start_time(duration_ms.to_string())
// After
.start_time(chrono::Utc::now().to_rfc3339())
```

### 2.3 Missing `#![deny(unsafe_code)]`
This codebase shells out to Docker extensively. There's no `unsafe` needed, but
there's also no guarantee it stays that way. Rust-familiar Docker interaction
should be safe-only.

**Suggestion:** Add `#![deny(unsafe_code)]` to `benchmark-core` and
`benchmark-types`.

### 2.4 `unwrap()` in production hot paths
Multiple `unwrap()` calls in Docker client and exercise runner:

- `docker/client.rs`: `Channel::send().unwrap()`, `unwrap_or()` on env
- `exercise_runner/mod.rs`: `RwLock::read().unwrap()`, `RwLock::write().unwrap()`
- `persistence/mod.rs`: `unwrap()` on lock acquisition

While `RwLock::read()` panic is rare, it's still a potential crash if poisoned.

**Suggestion:** Replace with `expect("message")` or handle the error with
`anyhow::Context` / `.ok_or_else(|| ...)`. In async contexts, prefer
`tokio::sync::RwLock` which doesn't poison on panic (the lock is simply released).

### 2.5 `run_all_exercises` spawns unconstrained tasks
The exercise runner spawns one `tokio::task` per exercise via
`futures::future::join_all`. With 100+ exercises and `parallelism=1`, this
spawns 100 tasks that all contend for a semaphore — wasting memory and
increasing scheduling overhead.

**Suggestion:** Use a work-stealing queue pattern or `StreamExt::buffered(n)`
with `n = config.parallelism` instead of spawn-all-then-join.

### 2.6 `.DS_Store` and `target/` in version control
The file listing shows `.DS_Store` at the repo root and `target/` build
artifacts. Neither should be tracked.

**Suggestion:** Add `.DS_Store` and `target/` to `.gitignore` (verify they're
present; if already tracked, remove with `git rm --cached`).

---

## 3. Readability & Simplicity

### 3.1 Duplicated exercise file copy logic
Four places copy exercise files to a temp directory with similar-but-not-identical
logic:
- `ClaudeAgent::copy_exercise_files`
- `PiAgent::run_exercise` (inline in the async fn)
- `ReferenceAgent::copy_exercise_files`
- `ReferenceAgent::copy_fresh_tests`

The Pi agent version is inlined directly in `run_exercise` (~40 lines), making
the function enormous (~200 lines). The C++ subdirectory special case is handled
differently in each.

**Suggestion:** Extract a shared `copy_exercise_files(exercise, src, dest)` in
`benchmark-core` that handles all languages (including C++ path rules).
Implement language-specific test-copy logic via a `LanguageHandler` trait.

### 3.2 Giant functions need decomposition
- `PiAgent::run_exercise` — ~250 lines
- `DockerClient::run_command_with_limits_and_volume_with_callback` — ~120 lines
- `PiAgent::collect_pi_trace` — ~140 lines
- `ReferenceAgent::run_exercise` — ~50 lines (better but does many things)
- `ClaudeAgent::run_claude_in_docker` — ~90 lines

**Suggestion:** Apply the "~30 lines per function" rule of thumb. Extract:
- `PiAgent::create_temp_work_dir`
- `PiAgent::copy_files_and_patch`
- `PiAgent::setup_environment`
- `PiAgent::collect_and_save_traces`

### 3.3 Dead/misleading comment in `PiAgent::create_models_json`
Doc comment is duplicated verbatim (lines 50–52 of pi.rs):
```
/// Creates a models.json configuration file for pi inside the working directory.
/// Uses the model parameter instead of Docker config env vars (matches Java behavior).
/// Creates a models.json configuration file for pi inside the working directory.
/// Uses the model parameter instead of Docker config env vars (matches Java behavior).
```

**Fix:** Remove duplicate lines.

### 3.4 `escape_json` recapitulates what `serde_json` already does
`PiAgent::escape_json` manually escapes `\`, `"`, `\n`, `\r`, `\t` — but the
resulting string is injected into a larger JSON string via `format!`. This is
fragile. If `model` contains a `\`, `"`, or `\n`, the manual builder produces
invalid JSON.

**Suggestion:** Build the `models.json` structure as `serde_json::Value` objects
and serialize to string. This eliminates the need for any manual escaping.

Example:
```rust
let models = serde_json::json!({
    "providers": {
        "openai": {
            "baseUrl": base_url,
            "apiKey": api_key,
            "api": "openai-completions",
            "models": [model_obj]
        }
    }
});
fs::write(&models_file, serde_json::to_string_pretty(&models)?)?;
```

### 3.5 `DockerConfig` accessor methods are trivial wrappers
All `DockerConfig` accessors (`image()`, `memory()`, `timeout()`, `work_dir()`,
`environment()`, `per_command_timeout()`) are one-liner wrappers that simply
dereference `Option`. No validation, no transformation. Public field access
with `Option` fields would be equally safe and less code.

**Suggestion:** Make fields `pub(crate)` and remove accessors, or at minimum
inline them as `pub fn` that return the field directly.

### 3.6 Commented-out/placeholder code
- `benchmark-core/src/lib.rs:run_benchmark` comments out the agent's model
  update: `// client.set_model(model);`
- `benchmark-core/src/lib.rs:analyze_results` is a stub that panics with
  `anyhow::bail!("Analyzer not yet implemented")`

**Suggestion:** Remove the `analyze_results` stub if not needed, or
implement it. Add a tracking issue link with `// TODO(#NNN):`.

---

## 4. Testing Gaps

### 4.1 No integration tests
All tests are unit tests. There are no integration tests that:
- Run the full Docker workflow (mock Docker would work)
- Test the web server endpoints end-to-end
- Verify result persistence round-trips
- Test the queue processor scheduling logic

**Suggestion:** Add at minimum:
- `tests/integration/` with HTTP tests against a running server (using
  `axum_test` or similar)
- Queue scheduling unit tests with a mock `ExerciseRunner`
- Result serialization round-trip tests (write → read → verify)

### 4.2 No tests for `ClaudeAgent` or `PiAgent` exercise paths
`agent/mod.rs` tests only verify agent names and that `set_output_consumer`
doesn't panic. There are no tests for `create_exercise_prompt`, `copy_exercise_files`,
`install_pi_extensions`, or any of the Docker command construction logic.

**Suggestion:** Factor Docker command construction into pure functions and
test them. Test prompt generation with a temp directory structure.

### 4.3 `ReasoningRegistry` has no test for thread safety
`ReasoningRegistry` uses a static `Mutex<Vec>`. Multiple threads calling
`register()` and `lookup()` concurrently should be tested to ensure no deadlocks.

**Suggestion:** Add a `parallel_register_and_lookup` test using
`std::thread::spawn`.

### 4.4 No tests for `contains_test_failures` edge cases
`ReferenceAgent::contains_test_failures` checks for "BUILD FAILED", "FAILURE",
etc. but has no test for:
- False positives (e.g., class named `BuildFailureException`)
- Unicode/non-ASCII output
- Empty output with zero-length body

---

## 5. Performance

### 5.1 `run_all_exercises` doesn't enforce parallelism cap
Despite `config.parallelism` existing, `run_all_exercises` spawns all exercises
as concurrent `tokio::spawn` tasks. If there are 100 exercises and parallelism=4,
it spawns 100 tasks that all contend on the single tokio runtime if they share
a Docker client bottleneck.

**Suggestion:** Use `StreamExt::buffer_unordered(parallelism)` with a
`futures::stream::iter(tasks)` to limit concurrency.

### 5.2 Result cache is rebuilt on every request
`ResultService::refresh_cache` walks the entire results directory on every
call (called at startup and on `list_individual_results`). With thousands of
result files, this is expensive.

**Suggestion:** Add filesystem watching (`notify` crate) to invalidate
individual entries, or add a configurable TTL so cache rebuilds are throttled.

### 5.3 `QueueProcessor` polling loop is busy-when-empty
The queue worker loop uses a fixed 50ms poll interval via `tokio::time::sleep`.
When the queue is empty, this burns CPU with unnecessary wakeups.

**Suggestion:** Replace the polling loop with a `tokio::sync::Notify` or
`tokio::sync::mpsc` channel. Enqueue sends a notification; the worker
`await`s on it without polling.

### 5.4 `container_id` is re-computed from scratch for every Docker command
`DockerClient::run_command_with_limits_and_volume_with_callback` generates a new
UUID-based container ID every time. This is fine for uniqueness, but each UUID
generation calls `/dev/urandom`.

**Suggestion:** Use an incrementing counter prefix (e.g.,
`bench-{agent}-{exercise}-{counter}`) for debuggability and reduced entropy
consumption.

---

## 6. Configuration & Deployment

### 6.1 Hardcoded paths in `config.rs`
`default_benchmark_path()` returns `PathBuf::from("../polyglot-benchmark")` and
`default_results_dir()` returns `PathBuf::from("../benchmark-results")`. These
are resolved relative to the current working directory, not the crate.

**Suggestion:** Use `std::env::var("BENCHMARK_PATH")` as an override, or use
`config.yaml` with a mandatory `benchmark_path` field (no default).

### 6.2 `config.yaml` not included in repo
The file exists locally but may not be in version control. Without it,
developers can't run the project without reverse-engineering the config schema.

**Suggestion:** Add `config.example.yaml` with all fields documented, and
add `config.yaml` to `.gitignore`.

### 6.3 No `Dockerfile` documentation
`docker/Dockerfile.runner` and `docker/Dockerfile.runner.debian` exist but
there's no README explaining how to build them or what dependencies they need.
The `gradle-8.7-bin.zip` is a 140MB binary in the repo.

**Suggestion:** Add a `docker/README.md` with build instructions. Move the
Gradle zip to a `.gitignore`-protected download script (e.g., `docker/fetch-deps.sh`).

### 6.4 Missing `Dockerfile.runner` inspection needed
The Runner Dockerfile should be reviewed to ensure:
- It installs `pi`, `claude`, and language toolchains correctly
- The base image is pinned to a SHA256 digest for reproducibility
- `COPY` commands are ordered for layer caching efficiency

---

## 7. Observability

### 7.1 No structured metrics export
The system collects rich data (durations, tokens, success rates) but outputs
only JSON result files and `tracing` logs. There's no Prometheus metrics
endpoint, no latency histograms, and no error counting.

**Suggestion:** Add a `/metrics` endpoint to the web server that exports:
- `benchmark_exercises_total{agent, language, status}`
- `benchmark_exercise_duration_seconds{agent, language}` (histogram)
- `benchmark_queue_depth`
- `benchmark_active_workers`

### 7.2 `tracing` spans not used for structured context
All tracing calls use `info!()`, `debug!()`, `error!()` without spans. Adding
spans like `exercise = "two-fer"`, `agent = "claude"`, `language = "java"` would
make logs filterable and enable duration timing.

**Suggestion:** Add `#[tracing::instrument]` to key functions
(`run_exercise`, `run_tests_in_docker`, `process_queue_item`).

### 7.3 Error messages leak into `error!` but may not be actionable
Several `error!` calls log full Docker output (potentially thousands of lines).
Consider logging only first 500 chars with `...truncated` (some already do this
but not consistently).

---

## 8. Dependency Hygiene

### 8.1 Unused workspace dependencies
The root `Cargo.toml` workspace lists `benchmark-reporter` and
`benchmark-token-report` as dependencies of the root crate. If these are
separate binaries, they should be workspace *members* only, not dependencies.

### 8.2 `walkdir` used for both file discovery and simple directory listing
`walkdir` is a recursive directory walker, used to find exercise files. For
simple cases like `get_available_languages` (listing immediate children of a
directory), `std::fs::read_dir` is simpler and doesn't require an extra
dependency.

**Suggestion:** Use `std::fs::read_dir` for non-recursive directory listings.

### 8.3 `once_cell` is superseded by `std::sync::LazyLock`
Rust 1.80 stabilized `std::sync::LazyLock`. `once_cell::sync::Lazy` can be
replaced to eliminate the dependency.

---

## 9. Code Hygiene

### 9.1 Inconsistent naming conventions
- Some structs use `_ms` suffix for millisecond fields (`duration_ms`), others don't
- `exit_code` vs `exitCode` in serde tags
- `exercise_name` vs `exerciseName`
- `run_agent_name` (snake_case) alongside `pub container_id` (snake_case field
  name is normal, but the serde rename to `containerId` is inconsistent)

**Suggestion:** Pick one serde convention (camelCase, since JSON consumers are
JS/TS-based) and remove all serde aliases. The `#[serde(alias = "...")]`
proliferation suggests the schema changed over time.

### 9.2 Trailing whitespace / blank lines
`benchmark-web/src/lib.rs` has a blank line at EOF with trailing whitespace.
`benchmark-cli/src/lib.rs` needs review.

### 9.3 `#[allow(unused)]` and `#[cfg(test)]` markers
Scan for any `#[allow(dead_code)]` or `#[allow(unused_variables)]` that could
indicate dead code. The `_` prefix convention (`_unused`) appears in some places.

---

## Prioritized Action Items

| Priority | Area | Item | Effort | Status |
|----------|------|------|--------|--------|
| **P0** | Correctness | Fix `start_time` serialization in ReferenceAgent (2.2) | 5 min | ✅ Done |
| **P0** | Correctness | Fix manual JSON string building in `create_models_json` (3.4) | 30 min | ✅ Done |
| **P0** | Correctness | Wire parallelism cap into `run_all_exercises` (5.1) | 1 hr | ✅ Done |
| **P1** | Architecture | Unify `AgentResult`/`ExerciseResult` and `DockerConfig` types (1.1, 1.2) | 3 hr | ✅ Done |
| **P1** | Architecture | Add `AgentKind` enum, eliminate string dispatch (1.6) | 1 hr | ✅ Done |
| **P1** | Readability | Extract shared exercise file copy logic (3.1) | 2 hr | ✅ Done |
| **P1** | Readability | Decompose giant functions (3.2) | 3 hr | ✅ Done (extracted copy logic; PiAgent clipped ~40 lines, web config extracted) |
| **P1** | Testing | Add integration tests for queue processor (4.1) | 4 hr | ⏳ Deferred |
| **P2** | Architecture | Extract God function in `run_web_server` (1.4) | 2 hr | ✅ Done |
| **P2** | Performance | Replace busy-polling queue with `Notify` (5.3) | 1 hr | ✅ Done |
| **P2** | Observability | Add `#[tracing::instrument]` spans (7.2) | 1 hr | ✅ Done |
| **P2** | Observability | Add `/metrics` endpoint (7.1) | 3 hr | ✅ Done |
| **P3** | Config | Add `config.example.yaml` and Docker README (6.2, 6.3) | 1 hr | ✅ Done |
| **P3** | Hygiene | Standardize serde naming, remove aliases (9.1) | 2 hr | ⏳ Deferred |
| **P3** | Deps | Replace `once_cell` with `std::sync::LazyLock` (8.3) | 15 min | ✅ Done |
| **P4** | Cleanup | Remove `analyze_results` stub or implement (3.6) | 15 min | ✅ Done |
| **P4** | Cleanup | Add file watching for result cache (5.2) | 3 hr | ✅ Done |
| **P4** | CI | Add `.gitignore` entries for `.DS_Store`, `target/` (2.6) | 5 min | ✅ Done |

---

## Summary

The codebase is a solid Rust port of a Java benchmark runner. The two biggest
structural issues are **(a) duplicated types between runtime and reporting
layers** and **(b) no enforced parallelism cap in the exercise runner**. Beyond
these, there are many opportunities for simplification: extracting shared
file-copy logic, decomposing giant functions, replacing manual JSON building
with `serde_json`, and adding structured observability.

The test coverage is adequate for unit-level types and serialization but thin
on integration and agent-specific logic. The manual `escape_json` is the single
most dangerous piece of code — it will produce invalid JSON for any model name
with special characters.
