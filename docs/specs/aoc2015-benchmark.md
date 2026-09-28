# Spec: Advent of Code 2015 Benchmark Category

**Status:** Draft — pending review
**Author:** llm-benchmark contributors
**Date:** 2025-09-28
**Branch:** `feature/aoc2015-benchmark`
**Related:** `docs/specs/embedded-exercises.md` (exercise sourcing model), `docs/RESULT_FORMAT.md`

---

## Objective

Add a **new, separate benchmark category** for the Advent of Code 2015 puzzle set,
incorporating the generator/validator from the `aoc2015` project
(`/Users/dylan/Developer/aoc2015`) into this benchmark suite. The goals:

1. A **new run-benchmark form** (distinct from the existing Exercism form) for scheduling AoC runs.
2. A **separate category** so AoC results never mix with existing Exercism results.
3. The **same Docker runner container** as the current exercises.
4. The agent is provided with the **puzzle description (both parts)** and a **generated input**.
5. The agent can call a **validator endpoint** to check its answer during the run.
6. The agent is given **only instructions** (not the generator/validator binary) to call the endpoint.
7. AoC runs are executed **one day at a time** for now; a future iteration may test subagent
   orchestration.

The AoC exercise is fundamentally different from an Exercism exercise: it has no
language, no test suite, no `.meta` reference implementation, and no `config.json`.
Instead it is a **puzzle with a deterministic input generator and a known answer**
(part 1 and part 2). Validation is a *compare-the-answer* check, not a *run-the-tests*
check. This drives an entirely separate execution path.

---

## Background

### The `aoc2015` project

A self-contained Rust crate (`/Users/dylan/Developer/aoc2015/aoc2015`) that, offline,
can **generate fresh puzzle inputs** and **validate answers** for every day of AoC 2015:

```rust
// aoc2015/src/days/mod.rs
pub fn generate(user: &str, year: u32, day: u32) -> String;          // input
pub fn solve(user: &str, year: u32, day: u32) -> (String, String);   // (part1, part2)
```

- **Deterministic:** the same `(user, year, day)` always produces the same input. `user` is the
  random seed (FNV-1a over `user|year|day`, then SplitMix64). This is what makes `validate`
  consistent with `generate`.
- **Self-contained:** `src/` uses only `std` (`std::collections`, `std::cmp`), plus three
  internal helpers — `rng.rs` (SplitMix64), `md5.rs` (RFC 1321, Day 4), `json.rs` (Day 12).
  No external crates. Confirmed via `cargo tree -p aoc2015` (zero dependencies).
- **25 days**, each `days/dayNN.rs` exposing `generate(&mut Rng) -> String` and
  `solve(&str) -> (String, String)`. Day 25 has **one part only** (part 2 is empty).
- **Descriptions** live in `days/dayNN/problem.md` (both parts; fetched by `scripts/fetch.sh`).
  These are **git-ignored** in the `aoc2015` repo — they are puzzle text fetched from
  adventofcode.com and are not committed.

The CLI is `aoc2015 generate <user> <year> <day>` and
`aoc2015 validate <user> <year> <day> <answer>`. The crate is currently `[[bin]]`-only
(no `[lib]` target), so the logic is not yet reusable as a library.

### The benchmark suite (`llm-benchmark`)

- Rust workspace. Exercises are embedded at build time via `rust-embed`
  (`EmbeddedSource` implementing `ExerciseSource`). See `embedded-exercises.md`.
- `ExerciseRunner` (in `benchmark-core`) discovers exercises per language and drives an
  `Agent` (reference / claude / pi). Agents run **inside** the same Docker image
  (`ghcr.io/dylanschell/llm-benchmark-runner:latest`), work in `/workspace`, and the
  container is bind-mounted from a per-run temp dir.
- `AgentResult` is the persisted result shape (`result_{agent}_{language}_{exercise}.json`),
  stored under `results/{agent}-{model}/`.
- Web UI (`benchmark-web`) has a `/run` form, `/api/exercises`, `/api/benchmark/queue/schedule`,
  and a dashboard. There is currently **no category concept** — results are keyed only by
  `agent`, `model`, `language`, `exercise`.
- The container reaches the host at `host.docker.internal` (already used for the model
  endpoint at `host.docker.internal:8080`). **`curl` and `python3` are installed** in the
  runner image, so an agent can call an HTTP validator from inside the container.

---

## Success Criteria

- [ ] AoC puzzles are a **distinct category**; AoC result files are never written into the
      same path as Exercism results and the dashboard/report can tell them apart.
- [ ] A new **run form** (`/run-aoc` or a category-tabbed `/run`) schedules AoC runs.
- [ ] AoC runs use the **same Docker runner image** and the same `/workspace` mount model.
- [ ] The agent is given, in the container, the **puzzle description (part 1 + part 2)** and a
      **generated input** for the chosen day.
- [ ] The agent can call a **validator endpoint** (reachable from inside the container) that
      reports whether its answer matches part 1 / part 2.
- [ ] AoC results use the standard `AgentResult` shape (so reporting/replay tooling works) but
      are namespaced under a category.
- [ ] Existing Exercism results and flows are **unchanged** (regression: `cargo test --workspace`).
- [ ] A reference agent can complete at least one AoC day end-to-end in Docker.

---

## Design Decisions

| Decision | Choice | Why |
|---|---|---|
| Category representation | A `category` field on `AgentResult`, `BenchmarkSession`, `BenchmarkQueueItem` (enum `Polyglot` / `Aoc2015`; serde `default = Polyglot`) | Keeps one result schema, isolates AoC from Exercism, backward compatible |
| Results namespace | `results/{agent}-{model}/{category}/result_{agent}_{language}_{exercise}.json` — or a `{category}` subdir under the existing per-agent dir | Guarantees no collision; existing results (no category) are treated as Polyglot |
| AoC logic source | Vendor the `aoc2015` crate's `src/` (days, rng, md5, json) into a new **`benchmark-aoc2015`** workspace crate as a library | aoc2015 is `[[bin]]`-only, so it needs a `[lib]`; vendoring keeps the benchmark self-contained (no external checkout at runtime), matching the embedded-exercises philosophy |
| Descriptions | Embed the 25 `problem.md` files (both parts) as `rust-embed` assets in `benchmark-aoc2015` | They are git-ignored in the `aoc2015` repo and not committed; the benchmark must ship them |
| Validator endpoint | A small HTTP server in `benchmark-core` (or `benchmark-web`), reachable from the container at `host.docker.internal:<port>` | The agent runs inside the container and must reach the validator; `curl` is present |
| Validation model | Compare the agent's answer against `aoc2015::solve(user, year, day)`; report `correct` / `incorrect`, plus whether part 1 or part 2 matched | Mirrors `aoc2015 validate` |
| Input seed | A fixed benchmark `user` (configurable via config or run form, default e.g. `benchmark`) | `(user, year, day)` is the seed; a fixed user makes a run reproducible and the validator able to regenerate the same input |
| Exercise model | New `AocExercise`/`AocDay` struct — **not** the Exercism `Exercise` | AoC has no language, test files, or `.meta`; shoehorning it into `Exercise` would be misleading |
| Execution path | A separate `AocExerciseRunner`/`AocAgent` flow, reusing the existing `DockerClient` and `Agent` plumbing | AoC needs its own materialization (description + input), prompt, and validation; the `Agent` trait's `run_exercise` is Exercism-shaped |

### Resolved decisions (confirmed by author, 2025-09-28)

| # | Decision | Resolution |
|---|---|---|
| 1 | Vendoring | **Copy** the aoc2015 code into this project as a vendored library crate (`benchmark-aoc2015`). No path dependency. |
| 2 | Validator placement | **Route on the existing `benchmark-web` server** (`POST /api/aoc/validate`). |
| 3 | Agent access | The agent **never** gets the generation/validation binary or crate. It is given only the input, the description, and **instructions for how to call the validator endpoint** (`AOC_VALIDATOR_URL`). |
| 4 | Results | AoC results are **completely separate** from other benchmarks (separate namespace/dir). The agent also sees **similar stats**: puzzles solved, tokens used, time used. |
| 5 | Execution | For now the agent is driven through puzzles **one by one** (sequential). This is a deliberate, minimal first step; a future iteration may test **orchestration** — whether an agent can drive subagents to divide and conquer across a larger task. |

---

## Architecture

### New crate: `crates/benchmark-aoc2015`

A library crate vendoring the `aoc2015` generator/validator and embedding the descriptions.

```
crates/benchmark-aoc2015/
  Cargo.toml
  build.rs                      # embeds descriptions via rust-embed (relative folder)
  src/
    lib.rs                      # pub fn generate(user, year, day) -> String
                                # pub fn solve(user, year, day) -> (String, String)
                                # pub fn description(day) -> String   (both parts)
                                # pub fn days() -> Vec<u32>
    days/mod.rs                 # vendored from aoc2015 (Puzzle table, generate, solve)
    days/day01.rs .. day25.rs
    rng.rs, md5.rs, json.rs     # vendored helpers
  descriptions/
    day01.md .. day25.md        # embedded problem text (both parts)
```

`lib.rs` exposes the same public surface as `aoc2015/src/days/mod.rs` (`generate`, `solve`) plus
`description(day)` (reads the embedded `problem.md`). No CLI needed.

**Note on descriptions:** because the `aoc2015` repo git-ignores `problem.md`, these are copied
into the benchmark. They are public puzzle text from adventofcode.com (both parts for days 1–24;
day 25 has part 1 only). The `aoc2015` `scripts/fetch.sh` already produces clean Markdown — the
benchmark consumes those files as-is.

### Category model (`benchmark-types`)

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Category {
    Polyglot,  // existing Exercism set — default
    Aoc2015,
}
```

- Add `category: Category` (default `Polyglot`) to `AgentResult`, `BenchmarkSession`,
  `BenchmarkQueueItem`. Serde `default` keeps existing result files readable.
- Results path: `results/{agent}-{model}/{category}/result_{agent}_{language}_{exercise}.json`.
  Existing files (no category dir) are read as `Polyglot` by the loader.
- The dashboard/`ResultService` and `benchmark-reporter` gain an optional `category` filter,
  defaulting to `Polyglot` so existing views are unchanged.

### AoC exercise model (`benchmark-types` or `benchmark-core`)

```rust
pub struct AocDay {
    pub day: u32,        // 1..=25
    pub year: u32,       // 2015
    pub user: String,    // seed
    pub description: String, // both parts, embedded
}
```

No `language`, no `test_file`, no `.meta`. The agent may solve in **any** language available in
the container (Rust, Python, Java, Go, C++, JS) — that is the point of the benchmark.

### Runner: `AocRunner` (`benchmark-core/src/aoc_runner.rs`)

Parallels `ExerciseRunner` but AoC-shaped. Responsibilities:

1. `run_day(agent, day, user, model, thinking_level, results_dir)`.
2. **Materialize** into a per-run temp dir (then bind-mount `/workspace`):
   - `problem.md` — the description (both parts), from `benchmark-aoc2015::description(day)`.
   - `input.txt` — from `benchmark-aoc2015::generate(user, 2015, day)`.
   - A small `AGENTS.md`/prompt file describing the validator URL and the expected answer file.
3. Build the AoC prompt (see below).
4. Run the agent in the same Docker image via the existing `DockerClient`.
5. **Validate**: read the agent's answer file, call `aoc2015::solve`, compare.
6. Persist an `AgentResult` with `category = Aoc2015`, `language = "aoc2015"`,
   `exercise_name = "dayNN"`, `success` = both parts correct (day 25: part 1 only).

### Validator endpoint

Served by the existing web server (or a standalone server). Contract:

```
POST /api/aoc/validate
{ "user": "...", "year": 2015, "day": 7, "answer": "123", "part": 1 }
→ { "correct": bool, "matched_part": 1 | 2 | null }
```

- Regenerates the input via `generate(user, year, day)`, solves via `solve(...)`, and compares
  the submitted `answer` to part 1 and part 2 (a submitted answer that matches either is
  accepted, mirroring `aoc2015 validate`).
- `matched_part` tells the agent *which* part its answer matched, so it knows whether to keep
  going on part 2. `correct` is true if the answer matched either part.
- Reachable from inside the container at `host.docker.internal:<port>/api/aoc/validate`. The URL
  is injected into the container env (`AOC_VALIDATOR_URL`) and into the prompt.

### AoC prompt

The agent is instructed to:
1. Read `problem.md` (both parts) and `input.txt`.
2. Write a program (any language) that solves the puzzle, run it against `input.txt`.
3. Compute the answer(s), then call `AOC_VALIDATOR_URL` to check correctness for part 1, then
   part 2.
4. Write the final answers to a well-known file, e.g. `/workspace/answer.txt` (one line per
   part), so the runner can read and authoritatively validate them.
5. Iterate until both parts validate.

### Reference agent for AoC

The reference agent copies the reference implementation and runs tests for Exercism. For AoC
there is no reference implementation to copy; instead the reference agent **solves via
`benchmark-aoc2015::solve`** (it is the ground truth) and writes the correct answer to the
answer file — serving as the baseline / sanity check that the harness works.

---

## Interfaces

### `benchmark-aoc2015` public API

```rust
pub fn generate(user: &str, year: u32, day: u32) -> String;
pub fn solve(user: &str, year: u32, day: u32) -> (String, String);
pub fn description(day: u32) -> String;      // both parts, embedded
pub fn days() -> Vec<u32>;                   // 1..=25
```

### `Agent` trait extension (AoC)

The existing `Agent::run_exercise(&Exercise, &dyn ExerciseSource, ...)` is Exercism-shaped and
not suitable. Add an AoC method with a default `unimplemented!` (or a separate trait), so existing
agents keep working and only the AoC path needs implementing:

```rust
async fn run_aoc(&self, day: &AocDay, model: &str, thinking_level: Option<&str>,
                 results_dir: &Path, validator_url: &str) -> Result<AgentResult, ...>;
```

Implemented by `PiAgent` / `ClaudeAgent` (reusing their Docker invocation + trace collection)
and `ReferenceAgent` (solves directly).

### `ExerciseRunner` / scheduling

Add AoC-aware scheduling alongside the existing Exercism path. The queue/session carry a
`category`; the executor branches to `AocRunner` when `category == Aoc2015`.

---

## Web UI

### New run form

A new page `run-aoc.tera` at `/run-aoc`:

- Agent (reference / claude / pi), model, thinking level.
- Day selection (1–25) or "all days".
- Optional `user` seed (default `benchmark`).
- Retry toggle.

Runs are scheduled **one day per queue item** (sequential), matching the "one by one" execution
model confirmed above.

### New endpoints

```
GET  /run-aoc                          → AoC run form (page)
POST /api/aoc/queue/schedule           → schedule AoC run(s)
GET  /api/aoc/days                     → [1,2,...,25]
POST /api/aoc/validate                 → validator (above)
```

### Dashboard / results

A category filter (default `Polyglot`) so AoC results are visible but isolated.

---

## Commands

```bash
# Build (vendored aoc2015 + embedded descriptions, no network)
cargo build

# Test
cargo test --workspace

# Generate an input for a day (reproducible)
cargo run -p benchmark-aoc2015 -- generate benchmark 2015 7

# Validate an answer
cargo run -p benchmark-aoc2015 -- validate benchmark 2015 7 123
```

---

## Testing Strategy

- **`benchmark-aoc2015` unit tests:** `generate` is deterministic for a fixed `user`; `solve`
  round-trips (a generated input solves to a known pair); `description(day)` returns both parts
  for days 1–24 and part 1 for day 25; `days()` returns 1..=25.
- **Validator tests:** an answer matching part 1 → `correct`, `matched_part=1`; matching part 2 →
  `matched_part=2`; matching neither → `correct=false`; day 25 with a correct part-1 answer →
  correct.
- **Category tests:** `AgentResult`/session/queue default to `Polyglot`; AoC results are written
  under a `{category}` subdir and never collide with Exercism files; the loader treats legacy
  (category-less) files as Polyglot.
- **Integration (Docker, net-gated):** reference agent completes day 1 end-to-end; the validator
  is reachable from inside the container; `AOC_VALIDATOR_URL` is injected.
- **Regression:** `cargo test --workspace` passes; existing Exercism flows and result paths are
  byte-for-byte unchanged.

---

## Boundaries

**Always**
- Keep AoC results out of the Exercism namespace; never write `result_*` for AoC into a
  category-less path.
- Keep the per-run temp dir + `docker -v <dir>:/workspace` model and the same runner image.
- Keep existing Exercism `Exercise`/`ExerciseRunner`/`Agent::run_exercise` untouched.
- Keep `benchmark-aoc2015` self-contained and dependency-free.

**Ask First**
- Vendoring vs. path-dependency on `aoc2015`.
- Validator placement (web route vs. standalone binary).
- Whether AoC results appear in the main dashboard or a separate view.

**Never**
- Commit `problem.md` content that is not public puzzle text, or any personal AoC input.
- Commit secrets (the `aoc2015` `.env` / session cookie).
- Break existing result-file compatibility.

---

## Risks & Mitigations

| Risk | Mitigation |
|---|---|
| Descriptions are git-ignored upstream | Embed them in `benchmark-aoc2015`; they are public text, re-fetchable |
| `aoc2015` crate is bin-only | Vendor as a lib; expose `generate`/`solve`/`description`/`days` |
| AoC answers are deterministic but large / slow (e.g. Day 22 search, Day 24 partition) | Run per-day; the runner timeout already covers long runs |
| Validator reachability from container | Bind on the host, inject `host.docker.internal:<port>`; `curl` is installed |
| Category leaks into existing results | serde `default = Polyglot`; loader treats category-less dirs as Polyglot |
| Day 25 single-part edge case | Treat part-2 empty as "skip"; success = part 1 correct |

---

## Plan

1. **Vendor crate** — `benchmark-aoc2015`: copy `aoc2015/src/{days,rng,md5,json}`, add `[lib]`,
   embed `descriptions/dayNN.md`, expose `generate`/`solve`/`description`/`days`. Unit tests.
2. **Category model** — add `Category` enum to `benchmark-types`; thread it through
   `AgentResult`, `BenchmarkSession`, `BenchmarkQueueItem`; update result path + loader.
3. **AoC runner** — `AocRunner` in `benchmark-core`: materialize `problem.md` + `input.txt`,
   build the AoC prompt, run the agent in Docker, read the answer file, validate, persist.
4. **Validator** — HTTP route `/api/aoc/validate` wired to `benchmark-aoc2015::solve`.
5. **Agent impls** — add `run_aoc` to reference/pi/claude; wire validator URL + prompt.
6. **Scheduling/executor** — AoC branch in the queue/executor; session carries `category`.
7. **Web form** — `run-aoc.tera`, `/run-aoc`, `/api/aoc/queue/schedule`, `/api/aoc/days`.
8. **Dashboard/report** — category filter (default Polyglot).
9. **Verification** — unit + Docker integration + regression.
10. **Docs** — README, API.md, RESULT_FORMAT.md, CONFIGURATION.md, EXERCISE_SELECTION.md (note the
    new category), CHANGELOG.
