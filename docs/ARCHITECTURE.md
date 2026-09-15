# Architecture Overview

The LLM Benchmark Runner benchmarks autonomous coding agents against a curated Exercism exercise
suite (225 exercises across 6 languages). It runs an agent inside a Docker container against a
copy of an exercise, runs that exercise's tests, and records the result.

This document describes the Rust implementation. Every component, path and type named here exists
in the repository; the crate and module are given so a claim can be checked. An earlier revision of
this file described the original Java codebase (a `LanguageHandler` strategy, an exception
hierarchy, `BenchmarkController`) and was wrong about most of the internals — see `CHANGELOG.md`.

---

## System Context

```
                    ┌──────────────────────────────┐
                    │      llm-benchmark (src/)     │   one launcher binary
                    │  run │ web │ report │ token-  │
                    └───────┬──────────────┬────────┘
                            │              │
              ┌─────────────▼───┐     ┌────▼─────────────────────┐
              │ benchmark-cli   │     │ benchmark-web            │
              │ one-shot run    │     │ Axum dashboard + queue   │
              └─────────┬───────┘     └────┬─────────────────────┘
                        │                  │
                        └────────┬─────────┘
                                 ▼
                    ┌──────────────────────────┐
                    │  benchmark-core          │
                    │  ExerciseRunner          │
                    │  agents: reference, pi,  │
                    │  claude                  │
                    │  DockerClient            │
                    └────────────┬─────────────┘
                                 │  docker run
                                 ▼
                    ┌──────────────────────────┐
                    │  runner image            │
                    │  (separate artifact)     │
                    │  toolchains + agent CLIs │
                    └──────────────────────────┘
```

The runner image is built and versioned independently of the binaries; see
`docs/specs/runner-image.md`.

---

## Crates

| Crate | Role | Key items |
|---|---|---|
| `src/` (root) | Launcher binary | `Cli`/`Commands` (clap), `src/web.rs` maps web flags to env vars |
| `benchmark-cli` | One-shot CLI runner | `RunArgs`, `runner::create_agent` |
| `benchmark-web` | Axum dashboard | `AppState`, services, routes, `metrics`, `assets` |
| `crates/benchmark-types` | Shared types, no I/O | `config`, `exercise`, `agent`, `exercise_source`, `cancellation`, `reasoning`, `model`, `util` |
| `crates/benchmark-core` | Execution engine | `docker`, `exercise_runner`, `agent`, `endpoint`, `parallel`, `persistence` |
| `crates/benchmark-exercises` | Embedded exercise bundle | `EmbeddedSource` (`rust-embed`), `build.rs` |
| `crates/benchmark-exercises-build` | Build-time exercise assembler | fetch, prune, lock, verify |
| `benchmark-reporter` | Markdown report generator | `ReportArgs` |
| `benchmark-token-report` | Token statistics report | `TokenReportArgs` |

`benchmark-types` is the leaf: it holds the `Config` schema, the `Exercise` domain model, the
`Agent` trait and `AgentResult`, and has no Docker or HTTP dependency.

---

## `benchmark-web` internals

```
AppState (routes/mod.rs)
├── service:  BenchmarkService      ← the facade
│   ├── session_manager: SessionManager      in-memory session registry
│   ├── queue_processor: QueueProcessor      pending work + workers
│   ├── result_service:  ResultService       scans/caches result files
│   └── exercise_runner: ExerciseRunner      the actual execution engine
├── shutdown_flag: Arc<AtomicBool>
└── metrics: Metrics                Prometheus counters (metrics.rs)

models/     session.rs, queue.rs, queue_item.rs, status.rs
routes/     benchmark.rs, compare.rs, exercise.rs, queue.rs, result.rs, results.rs, scoring.rs
services/   session_manager.rs, benchmark_executor.rs, queue_processor.rs,
            result_service.rs, benchmark_service.rs
assets.rs   templates + static files embedded with rust-embed
```

- **`BenchmarkService`** coordinates the other three services and is what the routes talk to.
- **`BenchmarkExecutor`** turns a queued item into an agent run.
- **`ResultService`** is the read side: it walks the results directory, parses result files and
  (for token accounting) the Claude Code and pi log entries, and caches the summaries the
  dashboard renders.
- **`AppState`** is cloned into every handler; there is no global mutable state beyond the
  services' own locks.

---

## Module map

The real tree, not an aspirational one:

```
src/
  main.rs                  # clap subcommands -> the crates below
  web.rs                   # sets SERVER_PORT / RESULTS_DIR / PARALLELISM / CONFIG_PATH

benchmark-cli/src/
  lib.rs                   # RunArgs, config loading, CLI overrides
  runner.rs                # create_agent(AgentKind, DockerClient, verbose) + orchestration

benchmark-web/src/
  lib.rs                   # wiring: build the services, start Axum
  config.rs                # AppConfig from config.yaml + env
  assets.rs                # embedded templates and static files
  metrics.rs               # Prometheus text endpoint
  models/                  # session, queue, queue_item, status
  routes/                  # one module per feature area
  services/                # session_manager, benchmark_executor, queue_processor,
                           # result_service, benchmark_service

crates/benchmark-types/src/
  config/mod.rs            # Config and friends, defaults, validation
  exercise/mod.rs          # Exercise
  exercise_source.rs       # ExerciseSource trait (+ mode())
  agent/mod.rs             # Agent trait, AgentKind, AgentResult
  cancellation.rs          # CancellationToken
  reasoning.rs             # ThinkingLevel, ReasoningConfig, ReasoningRegistry
  model/mod.rs             # Claude Code stream-json types (log parsing)
  util/mod.rs              # supported_languages_set, recover_poisoned, ...

crates/benchmark-core/src/
  lib.rs                   # run_benchmark entry point
  exercise_runner/mod.rs   # ExerciseRunner
  agent/{reference,claude,pi}.rs + *_message_processor.rs
  agent/{exercise_files,test_patches}.rs
  docker/{client,stream_parser,watchdog}.rs
  endpoint.rs              # local model-server discovery
  parallel/mod.rs          # ParallelExecutor (semaphore)
  persistence/mod.rs       # ResultPersister, ResultSummary

crates/benchmark-exercises/       # rust-embed over the staged bundle
crates/benchmark-exercises-build/ # fetch + prune + lock + verify
```

Everything under `target/exercises-bundle/` is generated; third-party exercise content is never
committed.

---

## Data Flow

### CLI: one exercise

```
llm-benchmark run --language java --exercise series
  │
  ├─ RunArgs parsed (clap)                       benchmark-cli/src/lib.rs
  ├─ Config::load_or_default(--config)           crates/benchmark-types/src/config
  ├─ endpoint::resolve_endpoints(&mut config)    probes 8000/8080/9931 if unconfigured
  ├─ DockerClient::new(config.docker)
  ├─ create_agent(AgentKind, client, verbose)    benchmark-cli/src/runner.rs
  │
  └─ ExerciseRunner::run_exercises               crates/benchmark-core/src/exercise_runner
       │
       └─ for each exercise:
            1. materialize into a temp dir       agent/exercise_files.rs
               (EmbeddedSource bytes + file modes, reference solution withheld)
            2. patch tests                       agent/test_patches.rs
            3. agent.run_exercise(...)           reference | pi | claude
            4. docker run the test command       docker/client.rs
            5. AgentResult -> ResultPersister    persistence/mod.rs
```

### Web: queue to result

```
POST /api/benchmark/queue/schedule
  │
  ├─ BenchmarkService::schedule_batch_with_retry   -> one BenchmarkQueueItem per language
  ├─ QueueProcessor holds them as PENDING
  └─ workers (up to QueueConfig::parallelism)
       │
       └─ BenchmarkExecutor
            ├─ creates a BenchmarkSession (RunStatus::RUNNING)
            ├─ runs the same ExerciseRunner path as the CLI
            ├─ streams output to the session's broadcast channel -> GET /api/benchmark/{id}/stream
            └─ marks it COMPLETED or FAILED

GET /results  ->  ResultService (cache, refreshed by POST /api/results/refresh)
```

The CLI does not use sessions or the queue; it runs inline and exits.

### Exercise materialization

Exercises are compiled into the binary by `rust-embed`, so a run needs no checkout. The build
stages a pruned copy of the pinned Exercism tracks and records non-default file modes (the 47
`gradlew` scripts, which are executable) in a manifest, because `rust-embed` carries bytes and not
permissions. At run time `exercise_files.rs` writes the files into a fresh temp directory, applies
those modes, withholds the reference solution, and mounts that directory into the container.

### Endpoint resolution

`benchmark-core/src/endpoint.rs` runs once at startup. If `inference_endpoint` is configured it is
used as-is; otherwise the local ports 8000, 8080 and 9931 are probed concurrently for
`/v1/models`, and the first answer whose body has a `data` array wins. A loopback endpoint is also
handed to the container as `OPENAI_BASE_URL` on `host.docker.internal`; a remote one is not.

---

## Execution Lifecycle

### `RunStatus`

Five states, defined in `benchmark-web/src/models/status.rs`:

```
PENDING ──▶ RUNNING ──┬──▶ COMPLETED
                      ├──▶ FAILED
                      └──▶ CANCELLED
```

`is_active()` is `PENDING | RUNNING`. There is no `PAUSED`/`RESUMED`: pausing a benchmark run is
not implemented, and the queue's pause/resume endpoints described by an earlier revision of this
document do not exist.

### Queue item lifecycle

`QueueItemStatus` (`models/queue_item.rs`) uses the same five names:

```
PENDING ──▶ RUNNING ──┬──▶ COMPLETED
                      ├──▶ FAILED     ──▶ (retry -> PENDING)
                      └──▶ CANCELLED
```

`POST /api/benchmark/queue/retry/{id}` moves a failed item back to `PENDING`. There is no dead
letter queue, and `clear-terminal` simply deletes finished items.

---

## Design Patterns That Are Actually Present

### 1. Agent dispatch — `AgentKind` plus one trait

`AgentKind` (`benchmark-types/src/agent/mod.rs`) is an enum with `Reference`, `Claude`, `Pi`, a
`FromStr` and a `Display`. `Agent` is a trait with `run_exercise`,
`run_exercise_with_timeout`, `get_name` and `set_cancellation_token`. Construction is a plain
`match` on the enum, repeated at three sites (`benchmark-cli/src/runner.rs`,
`benchmark-web/src/services/benchmark_executor.rs`, `benchmark-core/src/lib.rs`) — adding an agent
means touching all three.

### 2. Facade — `BenchmarkService`

`BenchmarkService` (`services/benchmark_service.rs`) presents the routes with a small surface and
coordinates `SessionManager`, `QueueProcessor`, `ResultService` and `ExerciseRunner`. Handlers hold
`AppState` and never reach into a subservice directly.

### 3. Infrastructure boundary — `DockerClient`

All container interaction goes through `benchmark-core/src/docker/client.rs`, which builds and
spawns the `docker` CLI. Agents receive a `DockerClient` and never build a command line
themselves. The trade-off is deliberate: keeping the CLI means inheriting registry auth, the
credential helper, streaming and `host.docker.internal` rather than reimplementing them.

### 4. Semaphore-bounded concurrency — `ParallelExecutor`

`benchmark-core/src/parallel/mod.rs` bounds in-flight async tasks with a `tokio::sync::Semaphore`
rather than a thread pool, and returns results in input order.

### 5. Broadcast channels for streaming

A session owns a broadcast channel, so several subscribers can watch one run. `GET
/api/benchmark/{id}/stream` takes a receiver and turns it into server-sent events.

### 6. Embedded assets — `rust-embed`

Templates and static files are compiled into the binary (`benchmark-web/src/assets.rs`), the same
approach the exercise bundle uses. `TEMPLATES_DIR` and `STATIC_DIR` are development-only overrides.

For the patterns that exist only as names carried over from the Java codebase, see `ROADMAP.md`.

---

## The Docker Contract

`build_docker_run_command` (`docker/client.rs`) produces, per exercise:

```
docker run --name bench-<hex> \
  -w <work_dir> -m <memory> \
  -e KEY=VAL … \
  -v <temp_dir>:/workspace \
  -v <temp_dir>/.claude:/home/runner/.claude \
  [-v <pi_volume>] \
  <image> <command…>
```

Worth being explicit about, because the previous revision implied otherwise:

- **No `--rm`.** A finished container persists as `bench-<hex>` until it is removed; the client
  sweeps `bench-*` containers with `docker ps --filter name=bench-` on cleanup.
- **No `--network`, so containers get Docker's default bridge network.** That is required — the
  agent has to reach a model server, usually via `host.docker.internal`.
- **No `--pull`, so the default `missing` applies** and a missing image is fetched automatically.
  The client pre-pulls through `ensure_image_present` so the download is not charged against the
  run's timeout, with its own budget (`docker.pull_timeout`, default 1800s).
- One container per exercise, one `docker run` process per exercise.

Timeouts, from shortest to longest:

| Control | Config | Default | Enforces |
|---|---|---|---|
| Bash tool call | `docker.per_command_timeout` | 600s | `CommandWatchdog` kills the in-container command via `docker exec` |
| Whole exercise | `docker.timeout` | 300s | The `docker run` process |
| Image pull | `docker.pull_timeout` | 1800s | `docker pull`, once per machine |

`StreamParser` reads the agent's stdout, recognises Bash tool-call boundaries in both Claude
Code's `stream-json` and pi's event stream, and starts/stops the per-command watchdog. A liveness
loop polls `docker ps -a` every 5s and aborts the run only when the container is genuinely gone —
a container that has not started yet (still pulling) is not treated as a failure.

---

## Configuration Flow

```
config.yaml (optional)  ──▶  Config::load_or_default
        │                         │
        │                         └── serde defaults fill every unset field
        ▼
environment: CONFIG_PATH, SERVER_PORT, PARALLELISM, RESULTS_DIR
        │
        ▼
CLI flags   ── for `web`, the flags *are* env vars (src/web.rs)
```

A missing file is not an error; a file that exists but cannot be parsed is. Unknown keys are
silently ignored. `Config::validate()` runs on `run` and rejects `parallelism < 1`, an empty
`docker.image`, `docker.timeout < 10`, an empty `docker.memory` and an empty
`output.results_dir`. Details and the keys that have no effect: `docs/CONFIGURATION.md`.

---

## Error Handling

There is no exception hierarchy and no custom error enum — no `thiserror`, no `BenchmarkError`.
Rust's `?` carries errors along three shapes:

| Boundary | Shape |
|---|---|
| CLI and library entry points | `anyhow::Result<T>` |
| The `Agent` trait and other dynamic boundaries | `Result<T, Box<dyn std::error::Error + Send + Sync>>` |
| Web handlers | `Json` bodies; the handler never returns a 5xx for a domain error |

Two conventions follow from that:

- **Handlers report failures in the body, usually with HTTP 200** (for example an unknown session
  returns `{"error": "Session not found"}`). `result.rs` does use 404 and `queue.rs` uses 400. See
  `docs/API.md`.
- **A poisoned lock must not take the process down.** `benchmark_types::util::recover_poisoned`
  exists because `.unwrap()` on a `std::sync::Mutex` poisoned by one panicking request thread would
  panic on every later request.

Validation is fail-fast at startup and before a run; `Config::validate()` and the exercise manifest
check are examples.

---

## Testing

| Kind | Where | What it covers |
|---|---|---|
| Unit tests | `#[cfg(test)]` in each crate | Config defaults and validation, endpoint probing against real ephemeral TCP servers, container-state classification, the exercise assembler, mode restoration, template rendering |
| Integration tests | `benchmark-web/tests/` | `template_rendering.rs`, and `html_equality.rs`, which spawns a real server and compares rendered HTML |
| Network-gated tests | `benchmark-exercises-build` | Cloning a pinned track; the fidelity check against a local polyglot checkout |

Current count: **190 unit tests pass, 3 are `#[ignore]`d** (two need network, one needs a
`POLYGLOT_DIR` checkout of the aggregator this project deliberately does not use).

There is no coverage tooling and no coverage threshold; an earlier revision of this document
quoted percentages that nothing measures. CI runs `cargo test --release --workspace --lib --bins`,
a binary smoke test, `./build.sh docker-verify`, and `git diff --exit-code -- exercises.lock.yaml`
to prove the exercise lock is current. Neither `rustfmt` nor `clippy` gates the build.

---

## Deployment

Two artifacts, versioned together:

```
llm-benchmark-<version>-{linux,macos}-{x64,arm64}.tar.gz   the binaries
ghcr.io/dylanschell/llm-benchmark-runner:<version>          the runner image
```

- The binaries are four executables (`llm-benchmark`, `benchmark-cli`, `benchmark-reporter`,
  `benchmark-token-report`) plus `README.md`, `LICENSE` and `THIRD_PARTY_NOTICES`. Exercises and
  templates are inside the binaries, so a tarball install needs no supporting files.
- The image carries the toolchains and the agent CLIs and is published multi-arch
  (`linux/amd64`, `linux/arm64`) as an OCI index.
- `Cargo.toml` is the single source of truth for the version; `docker/runner.lock` pairs the image
  inputs with it, and `build.sh docker-verify` fails if the two disagree. A version bump therefore
  requires an image rebuild.
- The web server binds `0.0.0.0` and has **no authentication**; put it behind a trusted boundary.

---

## Performance Considerations

- **Concurrency** is bounded by `parallelism` (default 1). In the web server this is
  `QueueConfig::parallelism`, surfaced as `active_workers` / `parallelism_limit`; the CLI runs
  exercises through `ParallelExecutor`.
- **Memory** is capped per container by `docker.memory` (default `2g`). Result files are read from
  disk and cached by `ResultService` rather than held in one large structure, which is why a large
  results directory is workable.
- **Each run materializes a fresh temp directory**, so setup cost is paid per exercise; the
  exercise bytes themselves are in the binary.
- **The image pull is once per machine**, not per exercise.
- **The dashboard renders tables server-side.** With tens of thousands of results in one directory
  that is legitimately large HTML; `/results` and `/scoring` are the heavy pages.

---

## Security Considerations

- **Isolation is the container**, with a memory limit and two volume mounts, both inside the
  per-run temp directory (`/workspace` and `.claude`).
- **Containers are not network-isolated.** They use the default bridge so the agent can reach a
  model server. This is a requirement of the design, not an oversight.
- **The agent CLIs run with permission checks bypassed** (`--dangerously-skip-permissions` for
  Claude Code) because there is no interactive terminal to approve anything. The container is what
  contains that, so the trust decision is about the image and the exercise content, not the flags.
- **Agents are given no host credentials by default.** Anything they can reach is something the
  caller put in `docker.environment`.
- **The web server has no authentication or rate limiting**, and schedules work and reads results
  for anyone who can reach the port.
- Result files are read back and parsed, and the parsed content is model-generated, so it is
  handled through explicit serde types rather than assumed well-formed. `safe_truncate` exists
  because log output is truncated for display, and slicing a UTF-8 string at an arbitrary byte
  offset would panic instead of merely shortening the text.

---

## Future Work

See `ROADMAP.md`, which tracks the known structural issues (duplicate `AgentResult`/`ExerciseResult`
types, the crate-boundary leaks, splitting `benchmark-web/src/lib.rs`, the unused `parallel`
wrapper). This document describes what exists; that one describes what should change.

---

## Related Documentation

- [Configuration Reference](CONFIGURATION.md)
- [API Documentation](API.md)
- [Developer Guide](DEVELOPER.md)
- [Result Format](RESULT_FORMAT.md)
- [Exercise Selection](EXERCISE_SELECTION.md)
- [Embedded Exercises spec](specs/embedded-exercises.md)
- [Runner Image spec](specs/runner-image.md)
