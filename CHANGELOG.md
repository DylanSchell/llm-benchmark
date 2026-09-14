# Changelog — correcting the runner image reproducibility claim

`docker/README.md` claimed "the same `docker/` inputs produce the same image digest on every build,
so a published tag can be re-created byte-for-byte". The v1.4.0 release produced a counterexample,
and the claim is corrected rather than left standing.

The release bumped only the version, so `docker/runner.lock` kept `inputHash 54f3fa3c8a27…` — the
pins, the toolchain, the packaged agent set and even the per-platform self-reports are identical to
1.3.2. Comparing the two published arm64 manifests from the registry showed **5 shared layers out of
19**: the rebuild re-executed its install steps, and apt/npm/curl do not produce reproducible bytes.

So the guarantee `--provenance=false` actually buys is narrower than the doc said: with the same layer
bytes the index is deterministic (and the *index* was the thing that used to change on every rebuild,
because the provenance attestation embedded the build timestamp). It does not make a rebuild
byte-identical, and therefore does not make a versioned tag immutable. `inputHash` remains the signal
for whether the inputs changed; digest-pinning an image across a rebuild is not safe.

No version bump: `docker_input_hash` excludes `*.md`.

# Changelog — finding a local model server without configuring it

Running against a model server on this machine required spelling out its port twice, once for
the dashboard and once for the agent inside the container — and getting either wrong was
silent. The endpoint is now discovered, and the two layers agree by construction.

- **`inference_endpoint` is optional.** When it is absent, startup probes
  `http://localhost:{8000,8080,9931}/v1` for `GET /models` — all three at once, in that
  priority order, each with a 750 ms budget — and adopts the first that answers. Setting the
  field explicitly disables probing entirely, so a configured endpoint is never second-guessed.
  `Config::default()` now reports `None` ("not configured") rather than guessing a port, which
  is what makes "unset" distinguishable from "set to the default".

- **A plain `200` is not taken as proof.** The response has to contain a `data` array, the
  same thing `fetch_models` consumes. Port 8000 in particular is full of FastAPI apps that
  answer *something*, and a squatter must neither be adopted as a model server nor hide a real
  one further down the list.

- **The container's endpoint is derived from the host's.** When the adopted endpoint is on
  this machine, `OPENAI_BASE_URL` for the container becomes the same port and path on
  `host.docker.internal`, so a server on 9931 is reached on 9931 inside the container and no
  `docker.environment` block is needed. A **remote** endpoint is deliberately never rewritten:
  the host-side endpoint may point at a hosted API purely to populate the dashboard's model
  list, and silently redirecting the agent there with its own credentials would be worse than
  defaulting to localhost. An `OPENAI_BASE_URL` the user set is never touched.

- **Finding nothing is not an error.** The endpoint stays unset and the dashboard keeps its
  built-in model list, while the container keeps the `http://host.docker.internal:8080/v1`
  default, so an agent always has an endpoint to try.

`ExerciseRunner::fetch_models` handles the unset case explicitly, and its three copies of the
built-in model list are now one function.

Verified by execution in four configurations: a stub model server on 8000 was found and its
port reached the real `docker run` command line; with no stub, the machine's own server on
8080 was found and the dashboard listed **23 models** through `GET /api/models`; an explicit
`inference_endpoint` was used unchanged (and moved the container's port to match); and a stub
answering `{"status":"ok"}` was correctly rejected. `config.example.yaml` and the README's
step-4 block were both run verbatim afterwards (190 tests, up from 179).

# Changelog — a first run against a local model server

Four defects compounded so that the documented path — point the tool at a local model
server on 8080 and run an exercise — failed on a machine that had never run it before.
They are fixed together because they share one user story: run the binary, run a
benchmark, talk to a model you are serving yourself.

- **A container that has not been created yet is no longer treated as a dead one.** The
  liveness monitor asked `docker inspect … {{.State.Running}}` every 5 s and read *any*
  failure as "the container died", including the "no such object" it gets while the image
  is still being pulled. So the first run on a machine without the runner image was aborted
  and its container force-removed about five seconds in, before the download had finished.
  `container_is_running` is now `container_state`, backed by `docker ps -a --filter
  name=^<id>$ --format {{.State}}` (existence is reported through stdout, so this no longer
  depends on the wording or locale of the CLI's errors), and a unit-testable
  `classify_container_state` maps the output to `Running` / `Stopped` / `Pending`.
  `Pending` covers both the empty result for a name Docker does not know yet and the brief
  `created` window; it is no longer an abort, and `ContainerState::is_fatal()` — pinned by
  a test — says only `Stopped` is.

- **Pulling the image has its own timeout instead of the run's.** `docker run` pulls a
  missing image as part of the command, so the download was charged against
  `docker.timeout` (default 300 s), and a slow link could time out a run that had not
  started benchmarking. The image is now acquired before the timed run by
  `ensure_image_present`, with its own `docker.pull_timeout` (default 1800 s). An image that
  is already local costs one `docker image inspect`. The pull inherits stdio, because the
  default log filter is `benchmark_core=warn` and this module's INFO lines only appear with
  `--verbose` — without inheriting, a first run would sit behind a silent multi-minute
  wait. It also avoids buffering `docker pull`'s progress output in memory.

- **The host-side default endpoint is port 8080, not 8000.** `default_inference_endpoint()`
  returned `http://localhost:8000/v1`, which is not where the local servers people actually
  run (Ollama, LM Studio, llama.cpp, vLLM) listen. The default, the tests that pin it,
  `config.example.yaml` and every README table now say 8080.

- **`OPENAI_BASE_URL` is defaulted for the container.** `pi` runs *inside* the container and
  reads `OPENAI_BASE_URL` from `docker.environment`; with no entry it had no endpoint at
  all, which is why it had to be set by hand. `DockerConfig::environment_with_defaults()`
  now fills in `http://host.docker.internal:8080/v1` for any variable the user has not set,
  and the single conversion point in `benchmark-core` uses it, so `pi` and the `docker run
  -e` arguments can no longer disagree. An explicit value always wins. `ANTHROPIC_BASE_URL`
  is deliberately *not* defaulted: doing so would silently redirect a `claude` run that
  means to reach Anthropic's real API to a local server.

Verified by removing the runner image and running an exercise end to end: the pull was
logged, took 22.4 s, and the run then proceeded — where the old code would have aborted it
at 5 s. Unit tests pin the state mapping, the fatality policy, the default endpoint, and the
container environment that reaches `docker run -e` (179 total, up from 174).

# Changelog — running the released binary from an empty directory

Two bugs made the documented "download a release and run it" path unusable. Both only
appeared outside the source tree, which is why the suite never caught them: the tests ran
from `benchmark-web/`, where `templates/` and `config.yaml` were on disk.

- **A missing `config.yaml` no longer aborts the server.** `AppConfig::load` already
  tolerated an absent file and logged "using defaults", but `BenchmarkExecutor::new`
  hard-failed on the same path and `benchmark-web/src/lib.rs` `.expect()`-ed the result,
  so the process panicked one line after reporting that it was falling back. The
  `.expect()` is now a propagated error, and config absence is handled in one place,
  `Config::load_or_default`, which returns the defaults for a missing file but still
  reports a file that exists and cannot be parsed — a silently ignored typo is worse than
  a loud failure.
- **The built-in defaults were wrong.** Fixing the above exposed it: `#[derive(Default)]`
  ignores `#[serde(default = "...")]`, and each nested section was reached through
  `#[serde(default)]` on the *parent* field, which falls back to the derived `Default`.
  `Config::default()` therefore yielded an empty docker image, a zero timeout, an empty
  results directory and zero parallelism — a config `validate()` rejects. Falling back to
  "defaults" would have swapped a panic for a confusing failure at `docker run`.
  `Default` is now implemented by hand for all six config structs so that it agrees with
  the serde attributes, and a test pins the values.
- **Templates and static assets are embedded in the binary.** They were read from disk via
  `Tera::new("templates/**/*.tera")` and `ServeDir`, neither of which ships in the
  archives, so every page rendered a "Template Rendering Error" — and because `Tera::new`
  returns `Ok` for a missing directory, the server still started without complaint.
  `benchmark-web/src/assets.rs` now embeds both trees with `rust-embed`; `TEMPLATES_DIR`
  and `STATIC_DIR` remain as development overrides. `debug-embed` is required, without
  which debug and test builds keep reading the filesystem. The README had claimed these
  assets were embedded; it is now true.
- **Verified by execution, not by reading.** The release binary was run in an empty
  directory: `/` returned 37 KB of real dashboard, `/css/style.css` returned 6869 bytes as
  `text/css`, and `llm-benchmark run --agent reference` finished with "All exercises
  passed!" — which also proves the corrected defaults resolve to a real runner image.
- **A test for each bug.** A missing config file yields defaults that pass `validate()`, an
  unparseable file is an error, `Config::default()` satisfies `validate()`, the defaults
  match their documented values, `BenchmarkExecutor::new` tolerates a missing path, the
  embedded template set is complete and compiles, and the embedded stylesheet is served as
  CSS. 174 unit tests pass (was 166).

# Changelog — repository licensing

- **The repository now has an explicit MIT `LICENSE`.** The README had claimed MIT for some time, but
  no `LICENSE` file existed and no crate declared a licence, so there was no actual grant. The file is
  added, and GitHub now detects the repository as MIT.
- **`license = "MIT"` is declared once in `[workspace.package]`** and inherited by all nine packages
  through `license.workspace = true`, so the declaration cannot drift between crates.
- **`LICENSE` now ships inside the release archives**, beside `THIRD_PARTY_NOTICES` and `README.md`,
  because MIT requires the notice to accompany distributions. This takes effect on the next tagged
  release.
- **No version bump and no image rebuild.** `docker_input_hash` covers only files under `docker/`, so
  `Cargo.toml` and `LICENSE` sit outside it and `docker-verify` still reports `54f3fa3c8a27…`.
  `Cargo.lock` is unchanged, as licence metadata is not recorded there.
- **Third-party licensing is unaffected.** The embedded Exercism exercises are MIT (see
  `THIRD_PARTY_NOTICES`), and Claude Code remains excluded from the published image because Anthropic
  licenses it "all rights reserved".

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
