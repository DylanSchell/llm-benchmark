---
name: adding-an-exercise
description: Add, remove, or re-pin an Exercism exercise in the embedded benchmark bundle. Use when changing exercises.manifest.yaml, exercises.lock.yaml, the quick-bench list, or anything under crates/benchmark-exercises.
---

# Adding an Exercise

Exercise content is **not** committed. `exercises.manifest.yaml` names six upstream Exercism
tracks pinned by commit; `crates/benchmark-exercises/build.rs` clones them at build time, prunes
files, assembles the bundle under `target/exercises-bundle`, and rewrites the committed
`exercises.lock.yaml`.

## The flow

1. Pick the commit. Bump `sources.<language>.ref` to a reviewed upstream SHA — never a branch.
   A moving ref makes every CI build a different benchmark.
2. Select the exercise. `exercises.<language>.include` is the curated subset; `exclude` removes
   more and **wins over include**. Exercise-level selection is the primary control — the layout
   (`exercises/practice/{exercise}` → `{language}/exercises/practice/{exercise}`) is fixed.
3. Regenerate: `cargo build -p benchmark-exercises`. This re-fetches, re-prunes and rewrites
   `exercises.lock.yaml`.
4. Commit the regenerated lock. CI runs `git diff --exit-code -- exercises.lock.yaml`; a manifest
   edit whose lock was not committed fails the PR, not someone's machine later.
5. If the exercise is meant to be part of a fast smoke run, add it to
   `QuickBenchConfig::get_exercises_for_language` in `crates/benchmark-types/src/config/mod.rs`.
   That list is what "quick bench" mode filters on; it is deliberately short (sub-60s exercises).

## Checks

```bash
cargo build -p benchmark-exercises     # rewrites the lock; must be committed
cargo test -p benchmark-exercises-build
cargo test --workspace --lib --bins --tests --locked
```

## Traps

- The bundle lives in `target/`, which `rust-cache` keys on
  `hashFiles('exercises.manifest.yaml', 'exercises.lock.yaml')`. A stale cache that still passes
  locally is what that key exists to prevent — do not "fix" a failure by clearing it and moving on.
- `.benchmark-temp/` holds materialised per-run copies of exercises. It is disposable and ignored;
  if it grows (it reached 1 GB in one working tree), the run did not clean up — investigate the
  cleanup path rather than deleting the directory in a commit.
- Pruning is part of fidelity: what gets pruned changes what the agent can see. Read the pruning
  rules in `crates/benchmark-exercises-build/src/lib.rs` before assuming a missing file is a bug.
- `crates/benchmark-aoc2015` is a separate fixture set. Changing those solutions changes the
  expected answers of every AoC run ever recorded.
