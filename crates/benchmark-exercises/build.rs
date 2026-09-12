//! Build script for the embedded exercise bundle.
//!
//! Pipeline: fetch pinned Exercism tracks → prune per `exercises.manifest.yaml` →
//! stage into `target/exercises-bundle` → refresh `exercises.lock.yaml`. The staged
//! tree is embedded by `rust-embed` through a relative `#[folder]` path.
//!
//! Third-party exercise content is never committed to this repository; it lives in
//! `target/` and is (re)built when the manifest changes. `cargo clean` removes it.
//!
//! Offline builds reuse `target/exercises-cache`; set
//! `LLM_BENCHMARK_EXERCISES_OFFLINE=1` to forbid network access entirely.

use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::path::PathBuf;

fn main() {
    let crate_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));
    let repo_root = crate_dir
        .parent()
        .and_then(|parent| parent.parent())
        .expect("benchmark-exercises lives at crates/benchmark-exercises")
        .to_path_buf();
    let manifest_path = repo_root.join("exercises.manifest.yaml");

    println!("cargo:rerun-if-changed={}", manifest_path.display());
    println!("cargo:rerun-if-env-changed=LLM_BENCHMARK_EXERCISES_OFFLINE");

    let offline = env::var("LLM_BENCHMARK_EXERCISES_OFFLINE")
        .map(|value| value == "1" || value == "true")
        .unwrap_or(false);

    let manifest = benchmark_exercises_build::Manifest::load(&manifest_path)
        .unwrap_or_else(|e| panic!("failed to load {}: {e:#}", manifest_path.display()));

    let cache_root = repo_root.join("target").join("exercises-cache");
    let staging_root = repo_root.join("target").join("exercises-bundle");

    let mut resolved: BTreeMap<String, String> = BTreeMap::new();
    for (language, source) in &manifest.sources {
        let exercises = manifest
            .exercises
            .get(language)
            .unwrap_or_else(|| panic!("manifest has no `exercises` selection for `{language}`"))
            .effective();
        let outcome = benchmark_exercises_build::fetch_language(
            source,
            language,
            &exercises,
            &manifest.layout,
            &cache_root,
            offline,
        )
        .unwrap_or_else(|e| panic!("fetching exercise track `{language}`: {e:#}"));
        resolved.insert(language.clone(), outcome.resolved);
    }

    let report = benchmark_exercises_build::assemble_from(&cache_root, &staging_root, &manifest)
        .unwrap_or_else(|e| panic!("assembling exercise bundle: {e:#}"));

    // `rust-embed` embeds contents only, so the modes of the staged tree have to be
    // carried alongside it; the embedder reads this back via `include_str!`.
    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR is set for build scripts"));
    benchmark_exercises_build::write_mode_manifest(&staging_root, &out_dir.join("file-modes.txt"))
        .unwrap_or_else(|e| panic!("writing exercise mode manifest: {e:#}"));
    println!(
        "cargo:warning=LLM Benchmark: bundled {} exercises / {} files ({} pruned)",
        report.exercises, report.files, report.excluded_files
    );

    // Refresh the lock (derived metadata; committed for reproducibility) when it drifts.
    let lock = benchmark_exercises_build::Lock::generate(&staging_root, &manifest, &resolved)
        .unwrap_or_else(|e| panic!("generating exercise lock: {e:#}"));
    let serialized = lock
        .to_yaml()
        .unwrap_or_else(|e| panic!("serializing exercise lock: {e:#}"));
    let lock_path = repo_root.join("exercises.lock.yaml");
    if fs::read_to_string(&lock_path).ok().as_deref() != Some(serialized.as_str()) {
        fs::write(&lock_path, &serialized)
            .unwrap_or_else(|e| panic!("writing {}: {e}", lock_path.display()));
        println!(
            "cargo:warning=LLM Benchmark: updated {}",
            lock_path.display()
        );
    }
}
