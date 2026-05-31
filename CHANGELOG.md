# Changelog — 2026-05-31

## Correctness fixes

- **Fix `start_time` serialization in ReferenceAgent** — was writing `duration_ms` as a bare integer string instead of RFC3339 timestamp. Fixes invalid timestamps in result files written by the reference agent.

- **Fix manual JSON string building in `create_models_json`** — replaced `escape_json()` + `format!()` with `serde_json::Value` objects. The old code would produce invalid JSON for model names containing backslash, quotes, or newlines. Also removed the duplicate doc comment.

- **Add backward-compatible deserializers for existing result files** — `AgentResult` now uses custom `deserialize_timestamp` and `deserialize_duration_ms` to handle float epoch timestamps and float-second durations from older result files. Added `#[serde(default)]` to `output` field to handle files that omit it. Added tests verifying the real-world format.

- **Fix `run_all_exercises` parallelism cap** — replaced unbounded `tokio::spawn` + `join_all` with `StreamExt::buffer_unordered(parallelism)` so `config.parallelism` is actually enforced. With 100+ exercises, this prevents spawn storms.

## Architecture improvements

- **Unify result types** — merged `AgentResult` and `ExerciseResult` into a single `AgentResult` in `benchmark-types`. The unified type preserves all serde aliases for backward compatibility, adds token tracking fields (`input_tokens`, `output_tokens`, `cached_input_tokens`, `uncached_input_tokens`), model, and attempts. Removed duplicate builders. Updated `result_service.rs` and `benchmark-reporter` to use the unified type.

- **Unify DockerConfig types** — the runtime `DockerConfig` in `benchmark-core::docker` now uses `From<&benchmark_types::config::DockerConfig>` instead of manual field-by-field conversion. Fields changed from `Option<T>` to plain `T` with defaults applied at construction time. Removed the crate-boundary conversion boilerplate.

- **Add `AgentKind` enum** — `benchmark_types::agent::AgentKind` with `Reference`, `Claude`, `Pi` variants and `FromStr`/`Display`. Eliminated stringly-typed agent dispatch in CLI runner and core library. Added `AgentKind::ALL` constant for future iteration.

- **Extract `AppConfig` struct** — moved 60 lines of config loading from `run_web_server` into `benchmark-web/src/config.rs`. The God function `run_web_server` dropped from ~180 lines to ~120.

## Code deduplication

- **Extract shared `exercise_files` module** — moved `copy_exercise_files()` and `create_temp_work_dir()` out of three agent implementations (~120 lines each) into a single shared module. Handles C++ subdirectory rules, `.meta` exclusions, Gradle wrapper patching, and Rust `Cargo-example.toml` copying. Removed unused `walkdir::WalkDir` imports from `claude.rs` and `reference.rs`.

## Performance

- **Replace busy-polling queue with `tokio::sync::Notify`** — `BenchmarkQueue` now uses `Notify` to wake the worker thread only when items are enqueued, instead of a 50ms poll loop. Eliminates CPU wakeups on idle. Added `wait_for_item()` method. Removed the now-unused `poll_interval_ms` config field.

## Observability

- **Add `#[tracing::instrument]` spans** — all three agent `run_exercise` methods now emit spans with `exercise` and `language` fields for structured log filtering.

- **Add `/metrics` Prometheus endpoint** — new `benchmark-web/src/metrics.rs` exports `benchmark_exercises_total`, `benchmark_queue_depth`, `benchmark_active_workers`, and `benchmark_sessions_total` in Prometheus text format at `GET /metrics`.

## Configuration & deployment

- **Add `config.example.yaml`** — annotated example config for new developers. `config.yaml` added to `.gitignore`.

- **Add `.gitignore` entries** for `target/`, `Cargo.lock`, `.DS_Store`.

## Dependency hygiene

- **Replace `once_cell` with `std::sync::LazyLock`** — removed the `once_cell` dependency. Rust 1.80+ stabilized `LazyLock`.

## Cleanup

- **Remove `analyze_results` dead stub** — deleted the `anyhow::bail!()` placeholder.

- **Add TTL-based result cache throttling** — `ResultService::refresh_cache()` now skips rebuilds if the last refresh was less than 5 seconds ago, preventing full directory walks on every request.

## Files changed

22 files changed, 589 insertions, 481 deletions.
3 new files: `benchmark-web/src/config.rs`, `benchmark-web/src/metrics.rs`, `crates/benchmark-core/src/agent/exercise_files.rs`.
2 new files: `config.example.yaml`, `ROADMAP.md`.
