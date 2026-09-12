//! Embedded exercise suite, compiled into the binary.
//!
//! The bundle is assembled at build time from pinned Exercism tracks (see
//! `exercises.manifest.yaml` and `docs/specs/embedded-exercises.md`). No third-party
//! content is committed to this repository; it is fetched into `target/`, pruned, and
//! embedded by [`rust_embed`].
//!
//! [`EmbeddedSource`] is the production [`ExerciseSource`]. It exposes only
//! relative-path space — no host paths leak into the domain.

use benchmark_types::ExerciseSource;
use rust_embed::Embed;
use std::collections::HashMap;
use std::sync::OnceLock;

/// The staged exercise bundle produced by `build.rs`.
///
/// The folder is a *relative* path because `rust-embed`'s `compression` feature
/// rejects absolute paths; it resolves relative to this crate (where `Cargo.toml`
/// lives), so this points at `<repo>/target/exercises-bundle`.
///
/// `debug-embed` is enabled so debug/test builds use the embedded data too.
#[derive(Embed)]
#[folder = "../../target/exercises-bundle"]
struct Exercises;

const PRACTICE: &str = "exercises/practice/";

/// Modes for files that are not plain `0644`, keyed by bundle-relative path.
///
/// `rust-embed` stores contents only, so `build.rs` records the modes it observed in
/// the staged tree here (see `write_mode_manifest`) and [`ExerciseSource::mode`] hands
/// them back to the materializer.
fn file_modes() -> &'static HashMap<&'static str, u32> {
    static MODES: OnceLock<HashMap<&'static str, u32>> = OnceLock::new();
    MODES.get_or_init(|| {
        include_str!(concat!(env!("OUT_DIR"), "/file-modes.txt"))
            .lines()
            .filter_map(|line| {
                let (mode, path) = line.split_once(' ')?;
                Some((path, u32::from_str_radix(mode, 8).ok()?))
            })
            .collect()
    })
}

/// Exercise source backed by the compiled-in bundle.
#[derive(Debug, Default)]
pub struct EmbeddedSource;

impl EmbeddedSource {
    pub fn new() -> Self {
        Self
    }
}

fn language_of(path: &str) -> Option<&str> {
    path.split('/').next().filter(|segment| !segment.is_empty())
}

fn exercise_of<'a>(path: &'a str, language: &str) -> Option<&'a str> {
    let prefix = format!("{language}/{PRACTICE}");
    let rest = path.strip_prefix(&prefix)?;
    rest.split('/').next().filter(|segment| !segment.is_empty())
}

fn exercise_prefix(language: &str, exercise: &str) -> String {
    format!("{language}/{PRACTICE}{exercise}/")
}

impl ExerciseSource for EmbeddedSource {
    fn languages(&self) -> Vec<String> {
        let mut languages: Vec<String> = Exercises::iter()
            .filter_map(|path| language_of(&path).map(str::to_string))
            .collect();
        languages.sort();
        languages.dedup();
        languages
    }

    fn exercise_names(&self, language: &str) -> Vec<String> {
        let mut names: Vec<String> = Exercises::iter()
            .filter_map(|path| exercise_of(&path, language).map(str::to_string))
            .collect();
        names.sort();
        names.dedup();
        names
    }

    fn list_files(&self, language: &str, exercise: &str) -> Vec<String> {
        let prefix = exercise_prefix(language, exercise);
        let mut files: Vec<String> = Exercises::iter()
            .filter_map(|path| path.strip_prefix(&prefix).map(str::to_string))
            .collect();
        files.sort();
        files
    }

    fn read(&self, language: &str, exercise: &str, relative: &str) -> Option<Vec<u8>> {
        let key = format!("{}{}", exercise_prefix(language, exercise), relative);
        Exercises::get(&key).map(|file| file.data.into_owned())
    }

    fn mode(&self, language: &str, exercise: &str, relative: &str) -> Option<u32> {
        let key = format!("{}{}", exercise_prefix(language, exercise), relative);
        file_modes().get(key.as_str()).copied()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lists_expected_languages() {
        assert_eq!(
            EmbeddedSource::new().languages(),
            vec!["cpp", "go", "java", "javascript", "python", "rust"]
        );
    }

    #[test]
    fn lists_exercises_per_language() {
        let source = EmbeddedSource::new();
        assert_eq!(source.exercise_names("rust").len(), 30);
        assert!(source.exercise_names("rust").contains(&"alphametics".to_string()));
        assert!(source.exercise_names("java").contains(&"series".to_string()));
    }

    #[test]
    fn reads_known_files() {
        let source = EmbeddedSource::new();
        let files = source.list_files("rust", "alphametics");
        assert!(files.contains(&"Cargo.toml".to_string()));
        assert!(files.contains(&".meta/Cargo-example.toml".to_string()));

        let bytes = source
            .read("rust", "alphametics", "Cargo.toml")
            .expect("Cargo.toml is embedded");
        assert!(String::from_utf8_lossy(&bytes).contains("edition = \"2021\""));
        assert!(source.read("rust", "alphametics", "does-not-exist").is_none());
    }

    #[test]
    fn reports_modes_for_executable_files() {
        // `rust-embed` carries no permissions, so this exercises the out-of-band
        // manifest written by build.rs (`write_mode_manifest`) — the thing that keeps
        // the Java track's `./gradlew` runnable once materialized.
        let source = EmbeddedSource::new();
        assert_eq!(source.mode("java", "series", "gradlew"), Some(0o755));
        assert_eq!(source.mode("java", "series", "build.gradle"), None);
        assert_eq!(source.mode("rust", "alphametics", "Cargo.toml"), None);
    }

    #[test]
    fn has_exercise_uses_the_bundle() {
        let source = EmbeddedSource::new();
        assert!(source.has_exercise("cpp", "allergies"));
        assert!(!source.has_exercise("rust", "not-a-real-exercise"));
    }

    #[test]
    fn pruned_content_is_absent() {
        let source = EmbeddedSource::new();
        for language in source.languages() {
            for exercise in source.exercise_names(&language) {
                for file in source.list_files(&language, &exercise) {
                    assert_ne!(file, ".gitignore", "{language}/{exercise}");
                    assert_ne!(file, ".meta/tests.toml", "{language}/{exercise}");
                    assert!(
                        !file.ends_with(".j2") && !file.ends_with(".tera"),
                        "{language}/{exercise}/{file}"
                    );
                    assert!(
                        !file.starts_with(".approaches/") && !file.starts_with(".articles/"),
                        "{language}/{exercise}/{file}"
                    );
                }
            }
        }
    }
}
