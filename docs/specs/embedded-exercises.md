# Spec: Embedded Exercise Bundle (v2 — build-time sourcing)

**Status:** Approved for implementation
**Author:** llm-benchmark contributors
**Date:** 2025-09-12
**Branch:** `feature/embedded-exercises`

> v2 supersedes the v1 draft. The key change: **third-party exercise content is never
> committed to this repository.** It is cloned from upstream and pruned **during the
> build**, then embedded into the binary. A committed manifest controls what is included.

---

## Objective

Make the application **self-contained** while keeping third-party sources out of git:

1. No external `../polyglot-benchmark` checkout is required at runtime.
2. All absolute-path references to exercise content are eliminated from the domain model;
   `Exercise` carries relative paths only.
3. The build process **fetches (clones) and prunes** the exercise set from upstream.
4. A commit-able **manifest** controls which languages/exercises/files are included.
5. The packaged layout matches the layout the application uses today, so no path
   assumptions change.

### Provenance (reconnaissance)

The current `../polyglot-benchmark` checkout is a **manual copy** of six Exercism tracks
(5 "copy" commits, no script) at commit `7e0611e` (2024-12-22). It is already a curated
subset. Its shape:

| Language | Exercises | Files | Notable content |
|---|---|---|---|
| cpp | 26 | 304 | `test/catch.hpp`, `.meta/example.{cpp,h}`, `.meta/tests.toml` |
| go | 39 | 355 | `.meta/example.go`, `go.mod`, `.articles/`, `.approaches/` |
| java | 47 | 601 | `gradle/wrapper/*`, `gradlew`, `build.gradle`, `.meta/src/reference/java/*` |
| javascript | 49 | 556 | `.eslintrc`, `.npmrc`, `data/*.txt`, `.meta/example.js` |
| python | 34 | 280 | `.meta/example.py`, `.meta/*.j2` |
| rust | 30 | 260 | `src/`, `tests/`, `Cargo.toml`, `.meta/example.rs`, `.meta/Cargo-example.toml`, `.meta/*.tera` |

Upstream layout is `exercises/practice/<exercise>/…` inside each track repo. Packaged
layout is `<language>/exercises/practice/<exercise>/…`.

### Pin resolution (verified)

Exercism `main` has **drifted materially** since the snapshot: Java moved Gradle
**8.7 → 9.5.0** and gained `junit-platform-launcher`; Rust moved to **edition 2024**
(`itertools 0.5` → `0.14.0`, dropped `permutohedron`); Exercism also added
`.gitignore`. Those changes would break the offline Docker assumptions (Gradle 8.7
pre-installed, edition-2021 toolchain).

Pinning each track to its head commit **before 2024-12-22** reproduces the snapshot
exactly. Verified byte-for-byte against the local tree: `java/series`
(`SeriesTest.java`, `build.gradle`, `gradle-wrapper.properties` — Gradle 8.7) and
`rust/alphametics` (`Cargo.toml` edition 2021, `.meta/Cargo-example.toml`,
`tests/alphametics.rs`). Resolved pins:

| Track | Pinned commit | Date |
|---|---|---|
| cpp | `acc32555eab077cf786f892b6ab27a22184d81c7` | 2024-11-20 |
| go | `26635e19868f0b4fbed09a986b824cd67efdbc2e` | 2024-09-05 |
| java | `1123e5b3dddeb9e57b802835285a886b4428bfcc` | 2024-11-26 |
| javascript | `dbfbeae34dd4fce7fad7afd740d075d33cc51b21` | 2024-11-13 |
| python | `7bec634f5c51cce82d233ad88f7ae81a3e98242a` | 2024-12-02 |
| rust | `6c321382019fa969401b8e923a5e12ff69065561` | 2024-12-07 |

Full-subset fidelity is validated by the diff in Shape Fidelity Strategy below.

## Success Criteria

- [x] No `.git`-tracked exercise content in this repo (manifest/lock/notices only).
- [x] `cargo build` on a clean checkout fetches + prunes + embeds with **no manual step**.
- [x] Repeated builds reuse a cache and do no network I/O.
- [x] Packaged layout matches the current layout (verified by a fidelity check).
- [x] `Config` has no `benchmark_path`; `Exercise` has no source `PathBuf` fields.
- [ ] With `../polyglot-benchmark` absent, the reference agent completes ≥1 exercise per
      language end-to-end (Docker integration run still pending; `cargo test --workspace`
      passes without an external checkout).
- [x] Manual `-v <temp>:/workspace` execution model unchanged.
- [x] `THIRD_PARTY_NOTICES` attributes Exercism per track.
- [x] Build is reproducible from the lock file; suite scope is configurable via manifest.

## Decisions

| Decision | Choice |
|---|---|
| Third-party content in repo | **Never committed** — cloned at build time |
| Upstream | Exercism track repos (`exercism/{cpp,go,java,javascript,python,rust}`), pinned to snapshot SHAs |
| Inclusion control | Committed `exercises.manifest.yaml` (sources, subset, rules) |
| Reproducibility | Committed `exercises.lock.yaml` (resolved SHA + file hashes) — *pending approval* |
| Fetch | `git` sparse + shallow + blobless clone; tarball fallback |
| Embed | `rust-embed` (`compression`, `debug-embed`, `deterministic-timestamps`); the staged tree is read via a relative `#[folder]` (compression rejects absolute paths) |
| Assembler | `benchmark-exercises-build` build-support crate (unit-testable) |
| Trait | `ExerciseSource` in `benchmark-types` |
| Refresh | Re-resolve via `exercises.manifest.yaml`; the build regenerates the lock when content changes |

## Sourcing & Build Model

```
exercises.manifest.yaml   (committed: repos, refs, exercise subset, rules)
          │
          ▼
benchmark-exercises/build.rs
   ├─ resolve refs → exercises.lock.yaml     (committed after first resolve)
   ├─ fetch   : git sparse/shallow clone → cache (target/exercises/cache/<lang>/)
   ├─ prune   : apply manifest rules → staging (target/exercises/<lockhash>/)
   ├─ verify  : file count + hashes vs lock
   └─ stage   : target/exercises-bundle
          │
          ▼
rust-embed #[folder = "../../target/exercises-bundle"]  → compiled into binary
          │
          ▼
EmbeddedSource (ExerciseSource) → ExerciseRunner → materialize per-run temp dir → docker -v
```

- **Cache:** `target/exercises/cache/<language>/` keyed by resolved SHA.
- **Staging:** `target/exercises/<lock-hash>/`; rebuilt only when the lock changes.
- **Offline:** if the lock is present and `target/exercises-cache` exists, no network
  is used. Set `LLM_BENCHMARK_EXERCISES_OFFLINE=1` to fail instead of fetching.

### Fetch strategy

Per track: `git clone --depth 1 --filter=blob:none --sparse --branch <ref> <repo>`,
then `git sparse-checkout set exercises/practice/<ex1> …`. Only selected blobs are
downloaded. Fallback: download the GitHub commit tarball and extract selected paths.
The resolved commit SHA is written to the lock.

## Manifest (committed — ours, no third-party content)

```yaml
# exercises.manifest.yaml
version: 1
# Refs are pinned to the commits that reproduce the current snapshot (see above).
# Bumping a ref is a deliberate, reviewed change; resolved SHAs go to the lock.
sources:
  cpp:        { repo: https://github.com/exercism/cpp,        ref: acc32555eab077cf786f892b6ab27a22184d81c7 }
  go:         { repo: https://github.com/exercism/go,         ref: 26635e19868f0b4fbed09a986b824cd67efdbc2e }
  java:       { repo: https://github.com/exercism/java,       ref: 1123e5b3dddeb9e57b802835285a886b4428bfcc }
  javascript: { repo: https://github.com/exercism/javascript, ref: dbfbeae34dd4fce7fad7afd740d075d33cc51b21 }
  python:     { repo: https://github.com/exercism/python,     ref: 7bec634f5c51cce82d233ad88f7ae81a3e98242a }
  rust:       { repo: https://github.com/exercism/rust,       ref: 6c321382019fa969401b8e923a5e12ff69065561 }

layout:
  source: "exercises/practice/{exercise}"
  dest:   "{language}/exercises/practice/{exercise}"

# Exercise-level selection is the PRIMARY inclusion control.
# `include` is the curated subset (the 225 exercises currently in the set). It already
# encodes the exclusion of the many upstream exercises that are too easy to be
# informative for the benchmark. `exclude` documents/adds cuts (e.g. a too-easy
# exercise that slips in) and wins over `include`. No code changes required to prune.
exercises:
  cpp:        { include: [all-your-base, allergies, ...], exclude: [] }   # 26
  go:         { include: [...], exclude: [] }                             # 39
  java:       { include: [...], exclude: [] }                             # 47
  javascript: { include: [...], exclude: [] }                             # 49
  python:     { include: [...], exclude: [] }                             # 34
  rust:       { include: [...], exclude: [] }                             # 30

# File-level rules applied inside each selected exercise, in order (last match wins).
# These drop content the app never reads. Relax a rule later if it proves valuable.
rules:
  - exclude: "**/.git/**"
  - exclude: "**/.approaches/**"
  - exclude: "**/.articles/**"
  - exclude: "**/.meta/tests.toml"
  - exclude: "**/.meta/*.j2"
  - exclude: "**/.meta/*.tera"
  - exclude: "**/.gitignore"
```

## Pruning

Inclusion is controlled entirely by the manifest — no code changes.

**Exercise level (primary).** The `include` list is the curated subset; it already
encodes the exclusion of exercises that are too easy to be informative. `exclude`
entries are the explicit lever for dropping more (e.g. a too-easy exercise) and win
over `include`.

**File level (secondary, enabled by default).** These rules remove content the app never
reads, keeping the embedded bundle small:

| Rule | Reason | Effect |
|---|---|---|
| `**/.git/**` | VCS metadata | — |
| `**/.approaches/**` | never read at runtime | removes ~40 files |
| `**/.articles/**` | never read at runtime | removes ~6 files |
| `**/.meta/tests.toml` | unused by app | removes ~200 files |
| `**/.meta/*.j2` | Exercism codegen template | removes ~28 files |
| `**/.meta/*.tera` | Exercism codegen template | removes ~15 files |
| `**/.gitignore` | Exercism adds these; polyglot strips them | removes 80 files |

These can be relaxed later if any are deemed valuable.

**Must keep:** `.meta/config.json`, `.meta/example.*`, `.meta/Cargo-example.toml`,
`.meta/src/reference/java/**`, `.docs/instructions.md`, all language build files
(`gradle/**`, `gradlew*`, `build.gradle`, `pom.xml`, `go.mod`, `Cargo.toml`,
`package.json`, `CMakeLists.txt`, `test/catch.hpp`, `.eslintrc`, `.npmrc`, `data/**`).

> Note: the file-level excludes intentionally diverge from the current disk tree (which
> still carries a handful of `.approaches`/`.articles`/`tests.toml`/`*.j2`/`*.tera`).
> The layout the application depends on is unchanged; raw fidelity diffs treat these as
> expected deletions.

## Shape Fidelity Strategy

"Same shape" = the packaged tree reproduces the current layout exactly enough that no
application path logic changes.

**Verified (2025-09-12).** Clean-cloned all six pinned tracks, assembled with the committed
manifest, and hash-compared the bundle against the local `../polyglot-benchmark` subset:

| Metric | Result |
|---|---|
| Exercises | 225/225 |
| Files matching | **2048/2048** |
| Missing | 0 |
| Extra | 0 |
| Differing content | 0 |

The only divergence found was 80 `.gitignore` files Exercism adds (49 JS, 30 Rust, 1
Python) which polyglot strips; `**/.gitignore` was added to the rules to close it. The
bundle is now byte-identical to the current set.

1. **Dev-time diff (primary):** the assembler can run in `--verify-against
   ../polyglot-benchmark` mode and `diff` path sets (+ content hashes) against the local
   checkout. Not committed; used during migration and by the author.
2. **Lock file:** `exercises.lock.yaml` records resolved SHAs and per-file hashes so
   subsequent builds are byte-reproducible and drift is detected in CI.
3. **Structural test:** assert the packaged tree matches expected per-language shape
   signatures (e.g. java always has `gradle/wrapper/gradle-wrapper.jar`,
   `src/main/java/*.java`, `.meta/config.json`).

## Interfaces

### `ExerciseSource` (crates/benchmark-types/src/exercise_source.rs)

```rust
pub trait ExerciseSource: Send + Sync {
    fn languages(&self) -> Vec<String>;
    fn exercise_names(&self, language: &str) -> Vec<String>;
    fn has_exercise(&self, language: &str, exercise: &str) -> bool;
    fn list_files(&self, language: &str, exercise: &str) -> Vec<String>; // incl. .meta
    fn read(&self, language: &str, exercise: &str, relative: &str) -> Option<Vec<u8>>;
}
```

### `Exercise` (relative model)

```rust
pub struct Exercise {
    pub name: String,
    pub language: String,
    pub source_file: Option<String>,     // was source_path
    pub test_file: Option<String>,       // was test_path
    pub reference_dir: Option<String>,   // was reference_path
    pub metadata: Option<ExerciseMetadata>,
    pub solution_files: Vec<String>,     // were solution_paths
    pub example_files: Vec<String>,      // were example_paths
    pub test_files: Vec<String>,         // were test_paths
}
```

### Materialization (crates/benchmark-core/src/agent/exercise_files.rs)

```rust
/// Write the files an exercise needs inside the container into `dest`.
/// `.meta/` is never copied into the container.
pub fn materialize_exercise(
    source: &dyn ExerciseSource,
    exercise: &Exercise,
    dest: &Path,
) -> Result<PathBuf, Box<dyn Error + Send + Sync>>;
```

Container paths derive directly from relative paths: `format!("/workspace/{rel}")`
(cpp keeps its `<exercise>/` subdirectory). No `strip_prefix` remains.

## Commands

```bash
# Build (fetches + prunes + stages + embeds automatically)
cargo build

# Refresh after editing exercises.manifest.yaml: just rebuild — the lock is
# regenerated when the staged content changes. Delete target/exercises-cache to
# force a fresh fetch.

# Offline (reuse the target/exercises-cache, no network)
LLM_BENCHMARK_EXERCISES_OFFLINE=1 cargo build

# Test
cargo test --workspace
```

## Project Structure

```
exercises.manifest.yaml                 # committed: inclusion control
exercises.lock.yaml                     # committed: resolved SHAs + file hashes
crates/benchmark-exercises-build/       # build-support: fetch + prune + lock + verify
crates/benchmark-exercises/             # rust-embed over staged dir; EmbeddedSource
  build.rs                              # orchestrates assembler, sets relative #[folder]
crates/benchmark-types/src/exercise_source.rs
crates/benchmark-types/src/exercise/mod.rs
crates/benchmark-core/src/exercise_runner/mod.rs
crates/benchmark-core/src/agent/exercise_files.rs
xtask/                                  # `cargo xtask exercises …`
THIRD_PARTY_NOTICES                     # Exercism attribution per track
```

## Testing Strategy

- **Assembler unit tests** (`benchmark-exercises-build`): given a fixture upstream tree
  and a manifest, assert selection + rule application + dest layout.
- **Fetch test** (network-gated): sparse clone one track at a tiny ref, assert only
  selected paths are materialized.
- **Renderer/embed tests**: `EmbeddedSource::languages()` returns the six languages;
  known exercise lists expected files; `read()` hit/miss.
- **Fidelity**: dev diff vs `../polyglot-benchmark`; structural shape signatures.
- **Integration/offline**: with `../polyglot-benchmark` absent, reference agent for
  java/python/rust/cpp; docker mount asserted unchanged.
- **Regression**: `cargo test --workspace`, existing `html_equality` suite.
- **Reproducibility**: two clean builds produce identical staging hashes.

## Boundaries

**Always**
- Keep `.meta` semantics (metadata + reference impls) and never copy `.meta` into containers.
- Preserve cpp subdirectory and rust `Cargo-example.toml` behavior.
- Keep the per-run temp dir + `docker -v <dir>:/workspace` model.
- Keep third-party content out of git.

**Ask First**
- Enabling file-level pruning that diverges from the current shape.
- Changing upstream repos/refs or the exercise subset.
- Changing the lock file format.

**Never**
- Commit cloned exercise content, `.git`, or build caches.
- Commit secrets.
- Remove test coverage to make the refactor pass.

## Risks & Mitigations

| Risk | Mitigation |
|---|---|
| Exercism track content differs from the 2024-12-22 snapshot | Snapshot pins verified byte-for-byte on samples; full-subset diff during migration; lock pins resolved SHAs |
| Mid-build network failure | Cache staging keyed by lock hash; clear error; `OFFLINE` mode |
| Large track clones | Sparse + shallow + blobless fetch; tarball fallback |
| `rust-embed` + build-generated folder | Stage under `target/exercises-bundle` and reference it with a relative `#[folder]` (the `compression` feature rejects absolute paths) |
| `Exercise` schema change (Serialize/Deserialize) | Audit persistence/`RESULT_FORMAT.md` before rename; version if needed |
| Attribution/licensing | `THIRD_PARTY_NOTICES` per track; importer preserves upstream notices |
| Build-time coupling to GitHub | Cache + lock + offline mode + prebuilt-tree override |

## Open Questions

1. **Provenance** — *resolved:* Exercism track repos, pinned to the snapshot SHAs above
   (verified to reproduce the current content). Aider's `polyglot-benchmark` is no longer
   used.
2. **File-level pruning** — *resolved:* enabled by default; the extra content files
   (`.approaches`, `.articles`, `.meta/tests.toml`, `.meta/*.j2`, `.meta/*.tera`) are
   explicitly removed for now and can be re-introduced if valuable.
3. **Lock file** — *resolved:* commit `exercises.lock.yaml` (resolved SHAs + per-file
   hashes; derived metadata, not content).

All open questions are resolved; the spec is approved for implementation.

---

## Plan

1. **Manifest + lock schema** — define `exercises.manifest.yaml` with the pinned snapshot
   SHAs (resolved, above); generate the exercise subset from the current tree.
2. **Assembler** — `benchmark-exercises-build`: fetch (sparse clone), prune, stage, lock,
   verify. Unit-tested against a fixture tree.
3. **Embed crate** — `benchmark-exercises` + `build.rs` wiring + `EmbeddedSource`;
   `ExerciseSource` trait in `benchmark-types`.
4. **Fidelity** — dev diff vs `../polyglot-benchmark`; structural signatures; lock hashes.
5. **Domain refactor** — `Exercise` → relative paths; metadata path resolution.
6. **Runner refactor** — `ExerciseRunner` onto `ExerciseSource`; delete `benchmark_path`
   and `find_exercise_host_dir`.
7. **Agent refactor** — `materialize_exercise`; drop `host_exercise_dir` from the `Agent`
   trait + all three agents; derive container paths from relative paths.
8. **Config/web cleanup** — remove `benchmark_path`; fix `benchmark-web/src/lib.rs:59`.
9. **Verification** — offline integration, reproducibility, full workspace tests.
10. **Docs + attribution** — README/CONFIGURATION/ARCHITECTURE/API/ROADMAP +
    `THIRD_PARTY_NOTICES`.

## Tasks

- [x] Create `feature/embedded-exercises` branch.
- [x] Define `exercises.manifest.yaml` with the pinned snapshot SHAs; generate the 225-exercise subset
      from the current tree.
- [x] Validate **full-subset** fidelity: assemble all six tracks at the pins and diff the
      whole tree against `../polyglot-benchmark` (2048/2048 files identical).
- [x] Implement `benchmark-exercises-build` assembler (fetch/prune/stage/lock/verify) + unit tests.
- [x] Add a fidelity check against `../polyglot-benchmark` (`full_subset_fidelity`, ignored/net-gated).
- [x] Create `benchmark-exercises` crate with `rust-embed` over the staged dir + `build.rs`.
- [x] Add `ExerciseSource` trait in `benchmark-types`; implement `EmbeddedSource`.
- [x] Convert `Exercise` to relative paths (audited: not persisted, so no format migration needed).
- [x] Refactor `ExerciseRunner` onto `ExerciseSource`; remove `benchmark_path`.
- [x] Replace `copy_exercise_files` with `materialize_exercise`.
- [x] Drop `host_exercise_dir` from `Agent` trait + claude/pi/reference; relative container paths.
- [x] Remove `benchmark_path` from config/validate/web; fix `lib.rs:59`.
- [ ] Offline integration verification (4 languages, no external checkout) — needs Docker.
- [x] Reproducibility check: two offline clean assembles produce an identical staged tree
      (2048 files, tree hash `e6df1f3d…`) and no `exercises.lock.yaml` drift.
- [x] Docs + `THIRD_PARTY_NOTICES`.
