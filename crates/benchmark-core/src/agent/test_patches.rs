use std::fs;
use std::path::Path;
use std::sync::OnceLock;
use tracing::{debug, info};
use benchmark_types::exercise::Exercise;
use walkdir::WalkDir;

/// Cached regex for removing @Disabled annotations from Java tests.
static DISABLED_ANNOTATION_RE: OnceLock<regex::Regex> = OnceLock::new();

/// Patches test files for the exercise (language-specific modifications).
/// Removes skip annotations so all tests run.
pub fn run_patch_tests(exercise: &Exercise, temp_work_dir: &Path) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    info!("Patching tests for language: {}", exercise.language);
    match exercise.language.as_str() {
        "rust" => {
            run_remove_ignore_annotations(temp_work_dir)?;
        }
        "javascript" | "typescript" => {
            run_replace_xtest(temp_work_dir)?;
        }
        "java" => {
            run_remove_disabled_annotations(temp_work_dir)?;
        }
        _ => {
            debug!("No test patching needed for {}", exercise.language);
        }
    }
    Ok(())
}

/// Replaces xtest( with test( in JavaScript/TypeScript test files.
pub fn run_replace_xtest(dir: &Path) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    if !dir.exists() {
        return Ok(());
    }
    let mut count = 0;
    for entry in WalkDir::new(dir) {
        let entry = entry?;
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        if !["js", "ts", "mjs", "cjs"].contains(&ext) {
            continue;
        }
        let is_test_file = path.to_string_lossy().to_lowercase()
            .contains(".test.") || path.to_string_lossy().to_lowercase().contains(".spec.")
            || path.to_string_lossy().to_lowercase().contains("test")
            || path.to_string_lossy().to_lowercase().contains("spec");
        if !is_test_file {
            continue;
        }
        let content = fs::read_to_string(path)?;
        let modified = content.replace("xtest(", "test(");
        if content != modified {
            fs::write(path, modified)?;
            count += 1;
            info!("Replaced xtest( with test( in {}", path.display());
        }
    }
    if count > 0 {
        info!("Patched {} JavaScript/TypeScript test file(s) - replaced xtest( with test(", count);
    }
    Ok(())
}

/// Removes #[ignore] annotations from .rs files.
pub fn run_remove_ignore_annotations(dir: &Path) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    if !dir.exists() {
        return Ok(());
    }
    let mut count = 0;
    for entry in WalkDir::new(dir) {
        let entry = entry?;
        let path = entry.path();
        if !path.is_file() || !path.extension().map(|e| e == "rs").unwrap_or(false) {
            continue;
        }
        let content = fs::read_to_string(path)?;
        let re = regex::Regex::new(r"#\[ignore\([^)]*\)\]")
            .map_err(|e| format!("Invalid regex: {}", e))?;
        let modified = re.replace_all(&content.replace("#[ignore]", ""), "").to_string();
        if content != modified {
            fs::write(path, &modified)?;
            count += 1;
            info!("Removed #[ignore] from {}", path.display());
        }
    }
    if count > 0 {
        info!("Patched {} Rust test file(s) - removed #[ignore] annotations", count);
    }
    Ok(())
}

/// Removes @Disabled annotations from Java test files.
pub fn run_remove_disabled_annotations(dir: &Path) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    if !dir.exists() {
        return Ok(());
    }
    let mut count = 0;
    for entry in WalkDir::new(dir) {
        let entry = entry?;
        let path = entry.path();
        if !path.is_file() || !path.extension().map(|e| e == "java").unwrap_or(false) {
            continue;
        }
        let content = fs::read_to_string(path)?;
        let re = DISABLED_ANNOTATION_RE.get_or_init(|| {
            regex::Regex::new(r"@Disabled\([^)]*\)").expect("invalid Disabled regex")
        });
        let modified = re.replace_all(&content, "");
        if content != modified.as_ref() {
            fs::write(path, modified.as_ref())?;
            count += 1;
            info!("Removed @Disabled from {}", path.display());
        }
    }
    if count > 0 {
        info!("Patched {} Java test file(s) - removed @Disabled annotations", count);
    }
    Ok(())
}
