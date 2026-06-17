---
name: codebase-quality-improvements
description: Systematic approach to improving Rust codebase quality baselines including formatting config, typed API responses, unwrap safety, and test cleanup.
created: 2025-01-01
---

# Codebase Quality Improvements

Systematic improvements for establishing a quality baseline in Rust projects.

## When to Use

After initial project setup, before major feature work, or when reviewing a codebase with no existing quality configuration.

## Steps

### 1. Add Formatting Configuration

Create `rustfmt.toml` at workspace root:

```toml
max_width = 100
hard_tabs = false
tab_spaces = 4
newline_style = "Unix"
use_small_heuristics = "Default"
imports_granularity = "Crate"
reorder_imports = true
```

### 2. Add Clippy Configuration

Create `.clippy.toml` at workspace root:

```toml
allow-unwrap-in-tests = true
disallowed-methods = []
doc-valid-idents = ["Claude", "Pi"]
```

Adjust `doc-valid-idents` to match agent/product names used in the project.

### 3. Fix Production `.unwrap()` Calls

Search for `.unwrap()` in production code (not tests):

```bash
grep -rn '\.unwrap()' --include='*.rs' crates/ src/ | grep -v '/tests/'
```

For semaphore/rate limiter acquire calls:
- Change signature to return `Result<Permit, AcquireError>` instead of bare `Permit`
- Replace `.acquire().await.unwrap()` with match that logs and exits gracefully
- Reuse existing error logging patterns (`error!()`)

For JSON parsing:
- Replace `as_array().unwrap()` with `as_array().map(|a| a.to_vec()).unwrap_or_else(|| vec![single.clone()])`
- This handles the edge case where `is_array()` check passes but `as_array()` returns None

### 4. Replace HashMap API Responses with Typed Structs

Search for `HashMap<String, String>` in API response types:

```bash
grep -rn 'HashMap.*String.*String' --include='*.rs' crates/ src/
```

For each occurrence:
1. Define a new `#[derive(Debug, Serialize)]` struct with explicit fields
2. Replace `Vec<HashMap<String, String>>` with `Vec<NewStruct>`
3. Replace manual key construction with `From<&Source> impl` or inline mapping
4. Update any callers that access fields by string key

### 5. Clean Up Test Files

Remove `#![allow(dead_code, unused_variables)]` from test files:

```bash
grep -rn '#!\[allow(dead_code' --include='*.rs' tests/
```

Run `cargo build --tests` to surface warnings. Fix by:
- Removing truly unused test helpers
- Prefixing unused parameters with `_`
- Moving shared fixtures to a `test_utils` module if reused

### 6. Verify

```bash
cargo check --workspace 2>&1 | tail -40
cargo test --workspace 2>&1 | tail -60
cargo clippy --workspace --all-targets 2>&1 | tail -40
```

## Notes

- Typed API changes are breaking for consumers — document this if the API is public
- Semaphore closure panics are rare but real under resource pressure (OOM kills, signal handlers)
- Test file cleanup may reveal legitimate dead code that should be removed or refactored
