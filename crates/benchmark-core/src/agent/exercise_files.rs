//! Shared exercise file-copy logic used by all agents.
//!
//! Extracted from ReferenceAgent, ClaudeAgent, and PiAgent to eliminate
//! duplicated copy-and-patch boilerplate.

use std::fs;
use std::path::Path;
use tracing::{debug, info};
use benchmark_types::exercise::Exercise;
use walkdir::WalkDir;

/// Copies exercise files from `source_dir` to `dest_dir`, skipping reference
/// implementations and patching Gradle wrapper properties.
///
/// For C++ exercises, files go into a `<exercise_name>` subdirectory inside
/// `dest_dir` so the build system finds them at the expected path.
pub fn copy_exercise_files(
    exercise: &Exercise,
    source_dir: &Path,
    dest_dir: &Path,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let exercise_dest = if exercise.language == "cpp" {
        let dest = dest_dir.join(&exercise.name);
        fs::create_dir_all(&dest)?;
        info!("C++ exercise: copying files to {}", dest.display());
        dest
    } else {
        dest_dir.to_path_buf()
    };

    info!(
        "Copying exercise files from {:?} to {:?}",
        source_dir, exercise_dest
    );

    let walker = WalkDir::new(source_dir).into_iter();
    for entry in walker {
        let entry = entry?;
        let source_path = entry.path();

        if source_path.is_dir() {
            let relative = source_path.strip_prefix(source_dir).unwrap_or(source_path);

            // Skip .meta directory tree
            if relative.to_string_lossy().contains(".meta") {
                continue;
            }

            let dest = exercise_dest.join(relative);
            fs::create_dir_all(&dest)?;
        } else {
            // Skip reference implementation directory
            let path_str = source_path.to_string_lossy();
            if path_str.contains(".meta/src/reference") {
                debug!("Skipping reference file: {:?}", source_path);
                continue;
            }

            let relative = source_path.strip_prefix(source_dir).unwrap_or(source_path);
            let dest = exercise_dest.join(relative);
            if let Some(parent) = dest.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::copy(source_path, &dest)?;

            // Patch Gradle wrapper to use local zip instead of network download
            if dest.ends_with("gradle-wrapper.properties") {
                let content = fs::read_to_string(&dest)?;
                let modified = content.replace(
                    "distributionUrl=https\\://services.gradle.org/distributions/gradle-8.7-bin.zip",
                    "distributionUrl=file:///opt/gradle/gradle-8.7-bin.zip",
                );
                fs::write(&dest, modified)?;
            }
        }
    }

    // For Rust exercises, copy Cargo-example.toml to Cargo.toml if it exists
    if exercise.language == "rust" {
        let cargo_example = source_dir.join(".meta").join("Cargo-example.toml");
        if cargo_example.exists() {
            let dest = dest_dir.join("Cargo.toml");
            fs::copy(&cargo_example, &dest)?;
            info!("Copied Cargo-example.toml to Cargo.toml");
        }
    }

    Ok(())
}

/// Creates a temporary working directory for an exercise under `.benchmark-temp/`.
pub fn create_temp_work_dir(exercise: &Exercise) -> Result<std::path::PathBuf, std::io::Error> {
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
