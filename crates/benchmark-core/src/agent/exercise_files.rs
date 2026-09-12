//! Shared exercise materialization used by all agents.
//!
//! Exercises live in the compiled-in [`ExerciseSource`]; this module writes the
//! files an exercise needs into a per-run temporary work directory that is then
//! bind-mounted into Docker at `/workspace`. No host exercise tree is required.

use std::fs;
use std::path::{Path, PathBuf};
use tracing::{debug, info};
use benchmark_types::exercise::Exercise;
use benchmark_types::ExerciseSource;

/// True when a relative exercise path lives under the `.meta/` tree, which is
/// never written into the container (it holds metadata and reference solutions).
fn is_meta_path(relative: &str) -> bool {
    relative == ".meta" || relative.starts_with(".meta/")
}

/// Writes one exercise-relative file from `source` into `dest`.
fn write_file(
    source: &dyn ExerciseSource,
    exercise: &Exercise,
    relative: &str,
    dest: &Path,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let Some(bytes) = source.read(&exercise.language, &exercise.name, relative) else {
        debug!(
            "Skipping missing exercise file {}/{}/{}",
            exercise.language, exercise.name, relative
        );
        return Ok(());
    };
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(dest, bytes)?;
    Ok(())
}

/// Materializes an exercise into `dest_dir` from `source`.
///
/// `.meta/` is never written into the container. C++ files go into a
/// `<exercise_name>/` subdirectory (the layout its build system expects). For
/// Rust, `.meta/Cargo-example.toml` replaces the stub `Cargo.toml`.
///
/// Returns the directory holding the exercise files (the C++ subdirectory, or
/// `dest_dir` itself).
pub fn materialize_exercise(
    source: &dyn ExerciseSource,
    exercise: &Exercise,
    dest_dir: &Path,
) -> Result<PathBuf, Box<dyn std::error::Error + Send + Sync>> {
    let exercise_dest = if exercise.language == "cpp" {
        let dest = dest_dir.join(&exercise.name);
        fs::create_dir_all(&dest)?;
        info!("C++ exercise: materializing files to {}", dest.display());
        dest
    } else {
        dest_dir.to_path_buf()
    };

    let files = source.list_files(&exercise.language, &exercise.name);
    info!(
        "Materializing {}/{} ({} files) into {}",
        exercise.language,
        exercise.name,
        files.len(),
        exercise_dest.display()
    );

    for relative in files {
        if is_meta_path(&relative) {
            continue;
        }
        write_file(source, exercise, &relative, &exercise_dest.join(&relative))?;
    }

    // Rust exercises need the dependency-complete Cargo.toml instead of the stub.
    if exercise.language == "rust" {
        write_file(
            source,
            exercise,
            ".meta/Cargo-example.toml",
            &exercise_dest.join("Cargo.toml"),
        )?;
        info!("Materialized Cargo-example.toml as Cargo.toml");
    }

    Ok(exercise_dest)
}

/// Creates a temporary working directory for an exercise under `.benchmark-temp/`.
pub fn create_temp_work_dir(exercise: &Exercise) -> Result<PathBuf, std::io::Error> {
    let base_dir = std::env::current_dir()?;
    let base_temp_dir = base_dir.join(".benchmark-temp");
    fs::create_dir_all(&base_temp_dir)?;

    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let exercise_temp_dir = base_temp_dir.join(&exercise.name).join(ts.to_string());
    fs::create_dir_all(&exercise_temp_dir)?;

    tracing::info!("Created temporary work directory: {:?}", exercise_temp_dir);
    Ok(exercise_temp_dir)
}

#[cfg(test)]
mod tests {
    use super::*;
    use benchmark_exercises::EmbeddedSource;

    fn exercise(name: &str, language: &str) -> Exercise {
        Exercise {
            name: name.to_string(),
            language: language.to_string(),
            source_file: None,
            test_file: None,
            reference_dir: None,
            metadata: None,
            example_files: Vec::new(),
            solution_files: Vec::new(),
            test_files: Vec::new(),
        }
    }

    #[test]
    fn rust_materialization_uses_dependency_cargo_toml_and_skips_meta() {
        let source = EmbeddedSource::new();
        let dir = tempfile::tempdir().unwrap();

        materialize_exercise(&source, &exercise("alphametics", "rust"), dir.path()).unwrap();

        // .meta/Cargo-example.toml replaces the dependency-less stub Cargo.toml.
        let cargo = fs::read_to_string(dir.path().join("Cargo.toml")).unwrap();
        assert!(cargo.contains("[dependencies]"), "Cargo.toml was not replaced: {cargo}");
        assert!(cargo.contains("permutohedron"));

        // Real tests are materialized, and .meta is never copied into the container.
        assert!(dir.path().join("tests/alphametics.rs").exists());
        assert!(!dir.path().join(".meta").exists());
    }

    #[test]
    fn cpp_materialization_nests_under_the_exercise_name() {
        let source = EmbeddedSource::new();
        let dir = tempfile::tempdir().unwrap();

        let dest = materialize_exercise(&source, &exercise("allergies", "cpp"), dir.path()).unwrap();

        assert_eq!(dest, dir.path().join("allergies"));
        assert!(dest.join("CMakeLists.txt").exists());
        assert!(dest.join("allergies.cpp").exists());
        // Nothing spills into the temp root.
        assert!(!dir.path().join("CMakeLists.txt").exists());
    }
}
