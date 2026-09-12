# Changelog — v1.3.1: the first release with macOS binaries

- **`v1.3.1` is the first tagged release that ships macOS binaries** — `macos/arm64` and
  `macos/x64` — alongside `linux/{x64,arm64}`. `v1.3.0` was tagged before the macOS job existed, so
  its release carries Linux assets only.
- **The image was rebuilt and republished at `1.3.1` to match.** The pin set is unchanged, so
  `inputHash` stays `54f3fa3c8a27…`, and the software inside is the same — verified by running a
  container per platform (`node v20.20.2`, `pi 0.85.1`, marker `1.3.1`, `claude` absent). The two
  images are **not** byte-identical, though: the build cache had been pruned, so every `RUN` layer
  was re-executed and its digest differs (an `apt` or `npm` install is not byte-reproducible),
  which leaves only the base-image layers shared. Republished rather than left at `1.3.0` because
  the binary and the image share one version — and `docker_verify` enforces that pairing by
  comparing the lock's version against `Cargo.toml`, so this bump could not have been committed
  without the rebuild.
- **No source change.** The source is identical, so the version string compiled in from `Cargo.toml`
  — what `llm-benchmark --version` prints — is the only build input that differs.

# Changelog — CI builds, tagged releases, and one version number

## One version number

- **The project version lives in `Cargo.toml`, and `docker/RUNNER_VERSION` is retired.** The version
  has to be in the manifest because clap prints it from `CARGO_PKG_VERSION`, which Cargo resolves at
  compile time — an external file could only ever mirror it. `build.sh` reads it through
  `runner_version()`, and `pin-agents.sh` writes it back through `set_runner_version()`, refreshing
  `Cargo.lock` with it (CI builds with `--locked`, so a stale lock is a hard failure).
- **The cost, deliberately accepted:** a re-pin now bumps the version the binary reports, so the image
  cannot be patched without cutting a binary release. The two artifacts are consumed as a pair and
  share a contract, so one number beats a compatibility table. This reverses the earlier "the image is
  versioned separately from the binary" framing, which predated the release pipeline.
- The version is **`1.3.0`**, matching the published image. The change is provably image-neutral: the
  rebuild that refreshed `runner.lock` produced the identical index digest `sha256:7f5464199442…`.

## GitHub Actions

- **`.github/workflows/build.yml`** builds and tests all four binaries (`llm-benchmark`,
  `benchmark-cli`, `benchmark-reporter`, `benchmark-token-report`) on `ubuntu-24.04`,
  `ubuntu-24.04-arm` and `macos-15`, and uploads them as tarballs with `README.md` and
  `THIRD_PARTY_NOTICES`. Free and unmetered, because standard GitHub-hosted runners are free for
  public repositories — macOS included, since it bills under that same standard SKU.
- **macOS ships both slices from one runner.** `macos-15` is Apple silicon, so the Intel slice is
  cross-compiled there (`x86_64-apple-darwin`) rather than given its own `macos-15-intel` job: the
  macOS SDK is universal, so it costs no extra runner-minutes, and Apple is retiring Intel anyway.
  That slice cannot be *executed* on an arm64 runner without Rosetta, so its architecture is
  asserted with `lipo -archs` instead of by running it. Building on macOS 15 does not raise the
  floor either: the arm64 slice declares `minos=11.0` and the Intel one `10.12`.
- **Tagging `v<version>` publishes a release** with every platform's tarball and `SHA256SUMS`. The
  workflow refuses to publish if the tag disagrees with `Cargo.toml`.
- **A `docker-inputs` job** runs `./build.sh docker-verify` on every push, so a `docker/` edit that
  forgot a version bump fails on the PR. It needs neither Docker nor the Rust toolchain.
- Also gated: the exercise lock (`git diff --exit-code exercises.lock.yaml`) proves the committed lock
  still matches the pinned upstreams.

# Changelog — runner image packaging & publishing

## Publishing

- **The image is built under its published name.** `build.sh` now tags
  `ghcr.io/dylanschell/llm-benchmark-runner:{<version>,latest}` by default, overridable with
  `--image`. `config.yaml`, `config.example.yaml` and `DockerConfig::default_image()` default to
  that same name, so the image you run and the image you publish are the same string.
- **`./build.sh docker-push`** — publishing is an explicit verb instead of a side effect of a
  build. It re-runs `docker-verify` before pushing, refuses when the image is absent locally, and
  refuses to publish an image containing Claude Code.
- **Claude Code is no longer packaged by default** (`docker/agents.env`, `INSTALL_CLAUDE=0`),
  which is what makes the image publishable at all. See `docker/README.md` for the licensing
  position and for how to build a local-only image that includes it.

# Changelog — feature/embedded-exercises

## Embedded exercise suite

- **Exercises are embedded in the binary** — the ~225-exercise suite is fetched from
  pinned Exercism tracks at build time (`exercises.manifest.yaml`), pruned, staged, and
  compiled in with `rust-embed`. No external `../polyglot-benchmark` checkout is required
  at runtime. Third-party content is never committed; `exercises.lock.yaml` records the
  resolved SHAs and per-file hashes, and `THIRD_PARTY_NOTICES` provides attribution.
- **`benchmark-exercises` crate + `build.rs`** — orchestrates fetch → prune → stage →
  lock and points `rust-embed` at the staged tree via a relative `#[folder]`. Compression
  keeps the embedded payload at ~2 MB (down from 21.5 MB raw).
- **`ExerciseSource` trait** — added to `benchmark-types`; `EmbeddedSource` implements it.
  All exercise access is relative-path based.
- **Domain refactor** — `Exercise` now carries exercise-relative paths only
  (`source_file`, `test_file`, `reference_dir`, `example_files`, `solution_files`,
  `test_files`). `ExerciseRunner` reads from `ExerciseSource`; `benchmark_path` and
  `find_exercise_host_dir` are removed.
- **`materialize_exercise`** — replaces `copy_exercise_files`; writes the exercise from
  the source into a per-run temp dir (still skipping `.meta`, applying the Rust
  `Cargo-example.toml` swap and the C++ subdirectory layout). The `Agent` trait now takes
  `&dyn ExerciseSource` instead of `host_exercise_dir`.
- **Config cleanup** — removed `Config.benchmark_path` and its filesystem validation;
  fixed `benchmark-web` falling back to `benchmark_path` as `CONFIG_PATH`.

---

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
