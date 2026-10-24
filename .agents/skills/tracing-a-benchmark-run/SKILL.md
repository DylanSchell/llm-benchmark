---
name: tracing-a-benchmark-run
description: Follow a benchmark run from CLI flag or web request to persisted result. Use when debugging why an exercise hung, was cancelled, timed out, produced no result, or scored unexpectedly, or when changing the session/queue/executor path.
---

# Tracing a Benchmark Run

Two entry points, one path. `docs/ARCHITECTURE.md` (§Data Flow) has the diagrams; this is the
"where do I put the print statement" version.

## The path

```
llm-benchmark run          benchmark-cli/src/lib.rs → runner.rs
llm-benchmark web          src/web.rs → benchmark_web::run_web_server(WebOverrides)
                             routes/queue.rs → services/session_manager.rs (create_session)
                                              → services/queue_processor.rs  (parallelism semaphore)
                                              → services/benchmark_executor.rs (execute_inner)
        └──────────────────────────────┬───────────────────────────────────────┘
                                       ▼
             ExerciseRunner::run_exercise{,_with_timeout}   crates/benchmark-core/src/exercise_runner
                                       ▼
             Agent adapter (pi | claude | reference)        crates/benchmark-core/src/agent/
                                       ▼
             DockerClient::run_command_with_limits_and_volume…  crates/benchmark-core/src/docker/client.rs
                 ├─ StreamParser  … tool-call boundaries from the agent's JSON stream
                 └─ CommandWatchdog … kills a Bash call that outlives per_command_timeout
                                       ▼
             ResultPersister::save_result                   crates/benchmark-core/src/persistence
                                       ▼
             ResultService cache (web)                      benchmark-web/src/services/result_service
```

## Where runs die, and what to look at first

| Symptom | Look here |
| --- | --- |
| Runs forever, then container timeout | `docker/watchdog.rs` (per-command kill via `docker exec pkill`), `docker/stream_parser.rs` (does it see the tool-call boundary?) |
| Cancel does not stop it | the `CancellationToken` from `SessionManager`, `Agent::set_cancellation_token`, and the `is_cancelled()` checks in `benchmark_executor.rs` |
| Model not loaded / first exercise slow | `crates/benchmark-core/src/model_load.rs` and `model_load_timeout_secs` (floor: `MIN_MODEL_LOAD_TIMEOUT_SECS`) |
| Wrong endpoint inside the container | `crates/benchmark-core/src/endpoint.rs` — `resolve_with` must not repoint a container at a hosted API |
| Result exists but dashboard misses it | `ResultService::update_single_result` / `refresh_cache`, and the cache key format `directory/language/exercise` |
| Thinking/reasoning not applied | `benchmark-types/src/reasoning.rs`: the per-model `ReasoningRegistry` mapping and `ThinkingLevel::parse` |

## Facts worth knowing before you change this path

- Sessions are copied into the executing task; a status change made on the manager's copy is not
  visible there. That is why cancellation is checked via the token and not only `session.status`.
- `--results-dir`/`RESULTS_DIR` is an *override*: it wins over the `<agent>-<model>-<langs>-<run#>`
  subdirectory the executor would otherwise derive. `AppConfig::results_dir_override` carries that
  distinction, so do not re-derive it from the environment downstream.
- Prompts are passed to the container separately from the command vector so logs never contain the
  prompt. Keep it that way in any new `run_command_*` call.
- AoC runs share this path but have no test suite: the agent validates against
  `/api/aoc/validate` on the host, reached through `AOC_VALIDATOR_URL` injected as extra env.

## Local reproduction without an LLM

```bash
llm-benchmark run --agent reference --language java --exercise <name>   # reference solutions
cargo test -p benchmark-core --lib
```
