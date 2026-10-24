# AGENTS.md

Work here is measured, literally: this repository benchmarks coding agents against Exercism
exercises and Advent of Code 2015 days, and the results are the product. A change that quietly
alters what an agent is asked to do, what it runs against, or how a run is scored is a product
change, not a refactor.

## Commands

```bash
./build.sh lint                                             # rustfmt + clippy -D warnings (CI gate)
cargo test --workspace --lib --bins --tests --locked        # everything CI runs (403 tests)
cargo build --release --workspace --locked                  # --workspace is load-bearing, see below
./build.sh docker-build                                     # the runner image
./build.sh docker-verify                                    # docker/ inputs vs runner.lock
```

`Cargo.toml` sets `default-members = ["."]`, so a bare `cargo build` produces only the launcher
and silently leaves `benchmark-cli`, `benchmark-reporter` and `benchmark-token-report` unbuilt.
Build and test with `--workspace`. The release binaries are smoke-tested by CI with `--help`;
add the same step if a new binary appears.

CI (`.github/workflows/build.yml`) runs, in order: `docker-verify`, `cargo fmt --all --check`,
`cargo clippy --all --workspace --all-targets -- -D warnings`, the tests, the release builds
(linux x64/arm64, macOS arm64 + an x86_64 cross slice), a lock-freshness check and packaging.
Clippy is at zero warnings. If a lint has to be silenced, silence it at the site with the reason
in a comment — do not lower the bar workspace-wide.

## Layout

- `crates/benchmark-types` — plain data: `Config`, `AgentResult`, the `Agent` trait, `ThinkingLevel`
  and the reasoning registry. No I/O beyond reading `config.yaml`.
- `crates/benchmark-core` — the execution machinery: `DockerClient`, `CommandWatchdog`,
  `StreamParser`, the agent adapters (`pi`, `claude`, `reference`), `ExerciseRunner`, persistence.
- `crates/benchmark-exercises{,-build}` — the embedded exercise bundle, assembled at build time.
- `crates/benchmark-aoc2015` — AoC reference solutions; these are benchmark fixtures, so a change
  here changes the expected answers.
- `benchmark-web` — Axum dashboard: `routes/` per feature, `services/` (session_manager,
  queue_processor, benchmark_executor, result_service, benchmark_service).
- `benchmark-cli`, `benchmark-reporter`, `benchmark-token-report`, `src/` — the four binaries.

## Invariants that are easy to break

- **Hand-written `Default` impls** for every config struct match the `#[serde(default = "...")]`
  attributes on purpose. `#[derive(Default)]` ignores those attributes and produced a config with
  an empty image, a zero timeout and an empty results directory. `Config::default()` is also the
  documented fresh-install state with no `config.yaml` at all, so it must satisfy `validate()`.
- **Version pins.** `docker/pins.env` is the single source of truth for everything the runner image
  pulls. Any change there is a runner-image change and needs a `Cargo.toml` version bump; CI's
  `docker-inputs` job enforces it via `./build.sh docker-verify`.
- **Exercise content is not committed.** `exercises.manifest.yaml` pins upstream tracks; the build
  script re-fetches, prunes and rewrites `exercises.lock.yaml`, which *is* committed and which CI
  diffs. Touch the manifest → rebuild `benchmark-exercises` → commit the regenerated lock.
- **Wire formats are shared with the Java reference.** The SCREAMING_CASE variants of `RunStatus`
  and `QueueItemStatus` are serde output that `benchmark-web/tests/html_equality.rs` compares
  against; they are why `clippy::upper_case_acronyms` is allowed at those two enums.
- **Config precedence** is flag → env (`CONFIG_PATH`, `SERVER_PORT`, `RESULTS_DIR`, `PARALLELISM`)
  → `config.yaml` → default, resolved once in `benchmark-web/src/config.rs::AppConfig`. Pass flags
  as data (`WebOverrides`); do not shuttle them through the process environment.
- **A poisoned lock must not take the process down.** Use
  `benchmark_types::util::recover_poisoned` rather than `.unwrap()` on a shared `Mutex`/`RwLock`.
- **Cancellation must be prompt.** Long-running Docker commands take a `CancellationToken`; the
  watchdog kills in-container processes so a cancelled session does not wait out the container
  timeout. Keep new agent adapters on that path.

## Errors

`anyhow` at the binaries, typed `thiserror` in the libraries (`ConfigError` today;
`library-errors` in `.agents/skills` states the target shape). `Display` names only its own
failure — never the source, which reporters print separately. Context says what was attempted.

## Where things are documented

`README.md` (install, run, endpoints) · `docs/ARCHITECTURE.md` (crates, data flow, docker contract,
error handling, testing) · `docs/DEVELOPER.md` (runner image, re-pinning, release flow) ·
`docs/API.md` (HTTP surface) · `ROADMAP.md` (known gaps, including the ones this file's siblings
in `.agents/skills/` walk through).

## Symposium

`cargo agents` installs crate-specific skills into `.agents/skills/` and `.claude/skills/`; those
copies are self-ignoring and not committed. Maintainer skills authored *for* this repo live in
`.agents/skills/` as tracked directories with real content — see `adding-an-exercise`,
`bumping-the-runner-image`, `tracing-a-benchmark-run`, `adding-an-agent-adapter`.
