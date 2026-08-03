use std::fs;
use std::path::{Path, PathBuf};
use tracing::{info, warn};
use benchmark_types::agent::AgentResult;

/// Summary of a benchmark run.
#[derive(Debug, serde::Serialize)]
pub struct ResultSummary {
    pub timestamp: String,
    pub agent: String,
    pub language: String,
    pub total_exercises: usize,
    pub successful: usize,
    pub failed: usize,
    pub success_rate: String,
    pub results: Vec<AgentResult>,
}

/// Handles persistence of benchmark results to disk.
/// Extracted from BenchmarkRunner for better separation of concerns.
pub struct ResultPersister;

impl ResultPersister {
    pub fn new() -> Self {
        Self
    }

    /// Saves a single exercise result to the results directory.
    /// Creates subdirectory as: {agent}-{model}
    ///
    /// When a result file already exists:
    ///   - Never overwrite a successful result with a failed one
    ///   - Only overwrite a successful result if the new run is faster
    ///   - Attempts counter always increments on overwrite
    pub fn save_result(
        &self,
        result: &AgentResult,
        agent_name: &str,
        model: &str,
        results_dir: &Path,
    ) -> Result<PathBuf, std::io::Error> {
        // Compute the subdirectory: {agent}-{model}
        let subdir = format!("{}-{}", agent_name, model);
        let target_dir = results_dir.join(&subdir);
        fs::create_dir_all(&target_dir)?;

        let filename = format!(
            "result_{}_{}_{}.json",
            agent_name, result.language, result.exercise_name
        );
        let result_file = target_dir.join(&filename);

        // Determine whether to save and what attempts count to use.
        let mut should_save = true;
        let mut attempts = 1u64;
        let mut existing_value_for_patch: Option<serde_json::Value> = None;

        if result_file.exists() {
            if let Ok(existing_content) = fs::read_to_string(&result_file) {
                if let Ok(existing_value) = serde_json::from_str::<serde_json::Value>(&existing_content) {
                    let existing_attempts = existing_value
                        .get("attempts")
                        .and_then(|v| v.as_u64())
                        .unwrap_or(1);
                    let existing_success = existing_value
                        .get("success")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false);
                    let existing_duration = existing_value
                        .get("duration")
                        .and_then(|v| {
                            // Handle both float seconds and integer ms
                            if let Some(f) = v.as_f64() {
                                Some((f * 1000.0) as u64)
                            } else {
                                v.as_u64()
                            }
                        })
                        .unwrap_or(u64::MAX);

                    // Never overwrite a successful result with a failed one
                    if existing_success && !result.success {
                        info!("Skipping save for {}/{}: new run failed but existing result already succeeded",
                            agent_name, filename);
                        should_save = false;
                        attempts = existing_attempts + 1;
                        // Preserve existing data to patch attempts only
                        existing_value_for_patch = Some(existing_value);
                    } else if existing_success && result.success {
                        // Existing was successful, new is also successful:
                        // only save if the new run is faster
                        let new_duration = result.duration_ms;
                        if new_duration >= existing_duration {
                            // New run was not faster — skip saving, keep the better result
                            info!("Skipping save for {}/{}: new duration {}ms >= existing {}ms",
                                agent_name, filename, new_duration, existing_duration);
                            should_save = false;
                            attempts = existing_attempts + 1;
                            // Preserve existing data to patch attempts only
                            existing_value_for_patch = Some(existing_value);
                        } else {
                            attempts = existing_attempts + 1;
                        }
                    } else {
                        // Overwriting a failure: increment attempts
                        attempts = existing_attempts + 1;
                    }
                }
            }
        } else {
            // New file: start at 1
            attempts = 1;
        }

        if !should_save {
            // When we skip the full save (e.g., retry failed against a previous
            // success, or retry was not faster), still update the attempts count
            // in the existing result file.
            if let Some(mut existing_value) = existing_value_for_patch {
                if let Some(obj) = existing_value.as_object_mut() {
                    obj.insert("attempts".to_string(), serde_json::Value::Number(serde_json::Number::from(attempts)));
                }
                let json = serde_json::to_string_pretty(&existing_value)?;
                fs::write(&result_file, json)?;
                info!("Updated attempts to {} in existing result: {:?}", attempts, result_file);
            }
            return Ok(result_file);
        }

        // Build result JSON with computed attempts and model
        let mut result_value = serde_json::to_value(result)?;
        if let Some(obj) = result_value.as_object_mut() {
            obj.insert("attempts".to_string(), serde_json::Value::Number(serde_json::Number::from(attempts)));
            obj.insert("model".to_string(), serde_json::Value::String(model.to_string()));
        }

        let json = serde_json::to_string_pretty(&result_value)?;
        fs::write(&result_file, json)?;

        info!("Result saved to: {:?} (attempts: {})", result_file, attempts);

        Ok(result_file)
    }

    /// Saves multiple exercise results to a summary file.
    pub fn save_results(
        &self,
        results: &[AgentResult],
        agent_name: &str,
        model: &str,
        language: &str,
        results_dir: &Path,
    ) -> Result<PathBuf, std::io::Error> {
        // Compute the subdirectory: {agent}-{model}
        let subdir = format!("{}-{}", agent_name, model);
        let target_dir = results_dir.join(&subdir);
        fs::create_dir_all(&target_dir)?;

        let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S").to_string();
        let filename = format!(
            "results_{}_{}_{}.json",
            agent_name, language, timestamp
        );
        let result_file = target_dir.join(&filename);

        let successful = results.iter().filter(|r| r.success).count();
        let failed = results.len() - successful;
        let success_rate = if results.is_empty() {
            "0.0%".to_string()
        } else {
            format!(
                "{:.1}%",
                (successful * 100) as f64 / results.len() as f64
            )
        };

        let summary = ResultSummary {
            timestamp: chrono::Utc::now().to_rfc3339(),
            agent: agent_name.to_string(),
            language: language.to_string(),
            total_exercises: results.len(),
            successful,
            failed,
            success_rate,
            results: results.to_vec(),
        };

        let json = serde_json::to_string_pretty(&summary)?;
        fs::write(&result_file, json)?;

        info!("Results saved to: {:?} (subdir: {})", result_file, subdir);

        // Save individual trace files — traces are saved separately by agents

        Ok(result_file)
    }

    /// Checks if a result file already exists for the given exercise.
    pub fn result_file_exists(
        &self,
        exercise_name: &str,
        agent_name: &str,
        model: &str,
        language: &str,
        results_dir: &Path,
    ) -> bool {
        let subdir = format!("{}-{}", agent_name, model);
        let result_path = results_dir.join(&subdir).join(format!(
            "result_{}_{}_{}.json",
            agent_name, language, exercise_name
        ));
        result_path.exists()
    }

    /// Checks if a result file exists and was successful.
    pub fn result_file_success(
        &self,
        exercise_name: &str,
        agent_name: &str,
        model: &str,
        language: &str,
        results_dir: &Path,
    ) -> bool {
        let subdir = format!("{}-{}", agent_name, model);
        let result_path = results_dir.join(&subdir).join(format!(
            "result_{}_{}_{}.json",
            agent_name, language, exercise_name
        ));

        if !result_path.exists() {
            return false;
        }

        match fs::read_to_string(&result_path) {
            Ok(content) => {
                if let Ok(value) = serde_json::from_str::<serde_json::Value>(&content) {
                    value
                        .get("success")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false)
                } else {
                    false
                }
            }
            Err(e) => {
                warn!("Failed to read result file {}: {}", result_path.display(), e);
                false
            }
        }
    }

    /// Prints a summary of benchmark results to stdout.
    pub fn print_summary(&self, results: &[AgentResult]) {
        let successful = results.iter().filter(|r| r.success).count();
        let failed = results.len() - successful;
        let success_rate = if results.is_empty() {
            0.0
        } else {
            (successful * 100) as f64 / results.len() as f64
        };

        println!("\n=== Benchmark Summary ===");
        println!("Exercises run: {}", results.len());
        println!("Tests passed: {} ({:.1}%)", successful, success_rate);
        println!("Tests failed: {}", failed);

        if failed > 0 {
            println!("\nFailed exercises:");
            for result in results.iter().filter(|r| !r.success) {
                println!("  - {}", result.exercise_name);
                if !result.output.is_empty() {
                    let preview = crate::safe_truncate(&result.output, 200);
                    println!("    Output preview: {}", preview);
                    if result.output.len() > 200 {
                        println!("    ... ({} more characters)", result.output.len() - 200);
                    }
                }
            }
        }
    }
}

impl Default for ResultPersister {
    fn default() -> Self {
        Self::new()
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use benchmark_types::agent::AgentResult;

    fn create_test_result(exercise_name: &str, language: &str, success: bool) -> AgentResult {
        AgentResult::builder()
            .exercise_name(exercise_name.to_string())
            .language(language.to_string())
            .success(success)
            .exit_code(if success { 0 } else { 1 })
            .output("Test output".to_string())
            .duration_ms(10_000)
            .start_time(chrono::Utc::now().to_rfc3339())
            .end_time(chrono::Utc::now().to_rfc3339())
            .build()
    }

    fn create_temp_dir() -> PathBuf {
        use std::sync::atomic::{AtomicU64, Ordering};
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let id = COUNTER.fetch_add(1, Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!("benchmark-persist-test-{}-{}", std::process::id(), id));
        let _ = std::fs::create_dir_all(&dir);
        dir
    }

    #[test]
    fn test_save_single_result() {
        let persister = ResultPersister::new();
        let temp_dir = create_temp_dir();
        let result = create_test_result("two-fer", "java", true);

        let saved_path = persister.save_result(&result, "reference", "default", &temp_dir).unwrap();

        assert!(saved_path.exists());
        assert!(saved_path.to_string_lossy().ends_with(".json"));

        // Verify file content
        let content = std::fs::read_to_string(&saved_path).unwrap();
        assert!(content.contains("two-fer"));
        assert!(content.contains("java"));
        assert!(content.contains("\"success\": true"));

        // Cleanup
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_save_single_result_with_failure() {
        let persister = ResultPersister::new();
        let temp_dir = create_temp_dir();
        let result = create_test_result("hello-world", "python", false);

        let saved_path = persister.save_result(&result, "reference", "default", &temp_dir).unwrap();
        assert!(saved_path.exists());

        let content = std::fs::read_to_string(&saved_path).unwrap();
        assert!(content.contains("\"success\": false"));

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_save_result_creates_directory() {
        let persister = ResultPersister::new();
        let temp_dir = create_temp_dir().join("subdir");
        let result = create_test_result("test", "java", true);

        let _ = persister.save_result(&result, "reference", "default", &temp_dir).unwrap();
        assert!(temp_dir.exists());

        let _ = std::fs::remove_dir_all(temp_dir.parent().unwrap());
    }

    #[test]
    fn test_save_result_different_agents() {
        let persister = ResultPersister::new();
        let temp_dir = create_temp_dir();
        let result1 = create_test_result("test", "java", true);
        let result2 = create_test_result("test", "java", true);

        let path1 = persister.save_result(&result1, "reference", "default", &temp_dir).unwrap();
        let path2 = persister.save_result(&result2, "claude", "default", &temp_dir).unwrap();

        assert!(path1.exists());
        assert!(path2.exists());

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_save_result_with_empty_output() {
        let persister = ResultPersister::new();
        let temp_dir = create_temp_dir();
        let mut result = create_test_result("empty-test", "go", true);
        result.output = String::new();

        let saved_path = persister.save_result(&result, "reference", "default", &temp_dir).unwrap();
        assert!(saved_path.exists());

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_save_result_auto_increments_attempts() {
        let persister = ResultPersister::new();
        let temp_dir = create_temp_dir();
        // First save: slow success (20s)
        let mut result = create_test_result("two-fer", "java", true);
        result.duration_ms = 20_000;

        let saved_path = persister.save_result(&result, "reference", "default", &temp_dir).unwrap();

        // Read and verify attempts = 1
        let content = std::fs::read_to_string(&saved_path).unwrap();
        let value: serde_json::Value = serde_json::from_str(&content).unwrap();
        assert_eq!(value.get("attempts").and_then(|v| v.as_u64()), Some(1));

        // Second save: faster success (10s) — should overwrite and increment attempts
        let mut result = create_test_result("two-fer", "java", true);
        result.duration_ms = 10_000;
        let saved_path = persister.save_result(&result, "reference", "default", &temp_dir).unwrap();

        // Read and verify attempts = 2 (incremented on overwrite)
        let content = std::fs::read_to_string(&saved_path).unwrap();
        let value: serde_json::Value = serde_json::from_str(&content).unwrap();
        assert_eq!(value.get("attempts").and_then(|v| v.as_u64()), Some(2));

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_save_result_increments_attempts_on_retry() {
        let persister = ResultPersister::new();
        let temp_dir = create_temp_dir();
        let result = create_test_result("test", "java", true);

        // First save with success
        let saved_path = persister.save_result(&result, "reference", "default", &temp_dir).unwrap();

        // Manually set attempts to 3
        let content = r#"{"exerciseName":"test","language":"java","success":true,"exitCode":0,"attempts":3}"#;
        std::fs::write(&saved_path, content).unwrap();

        // Save again (faster success overwriting success — increments attempts)
        let saved_path = persister.save_result(&result, "reference", "default", &temp_dir).unwrap();

        // Attempts should increment to 4
        let content = std::fs::read_to_string(&saved_path).unwrap();
        let value: serde_json::Value = serde_json::from_str(&content).unwrap();
        assert_eq!(value.get("attempts").and_then(|v| v.as_u64()), Some(4));

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_save_result_increments_from_existing_attempts() {
        let persister = ResultPersister::new();
        let temp_dir = create_temp_dir();

        // Create a file with attempts=5
        let subdir = temp_dir.join("reference-default");
        std::fs::create_dir_all(&subdir).unwrap();
        let file_path = subdir.join("result_reference_java_test.json");
        let content = r#"{"exerciseName":"test","language":"java","success":false,"exitCode":1,"attempts":5}"#;
        std::fs::write(&file_path, content).unwrap();

        // Save a new result (overwriting a failure increments attempts)
        let result = create_test_result("test", "java", false);
        let saved_path = persister.save_result(&result, "reference", "default", &temp_dir).unwrap();

        // Attempts should be 6 (5 + 1)
        let content = std::fs::read_to_string(&saved_path).unwrap();
        let value: serde_json::Value = serde_json::from_str(&content).unwrap();
        assert_eq!(value.get("attempts").and_then(|v| v.as_u64()), Some(6));

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_result_file_exists() {
        let persister = ResultPersister::new();
        let temp_dir = create_temp_dir();
        let result = create_test_result("test", "java", true);

        let _ = persister.save_result(&result, "reference", "default", &temp_dir).unwrap();

        assert!(persister.result_file_exists("test", "reference", "default", "java", &temp_dir));
        assert!(!persister.result_file_exists("nonexistent", "reference", "default", "java", &temp_dir));

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_result_file_success() {
        let persister = ResultPersister::new();
        let temp_dir = create_temp_dir();

        // Save a successful result
        let result = create_test_result("test", "java", true);
        let saved_path = persister.save_result(&result, "reference", "default", &temp_dir).unwrap();
        assert!(saved_path.exists());

        // result_file_success checks for the file in the results_dir
        assert!(persister.result_file_success("test", "reference", "default", "java", &temp_dir));

        // Attempt to save a failed result — should NOT overwrite the successful one
        let failed_result = create_test_result("test", "java", false);
        let _ = persister.save_result(&failed_result, "reference", "default", &temp_dir).unwrap();

        // The successful result should still be recorded, not overwritten by the failure
        assert!(persister.result_file_success("test", "reference", "default", "java", &temp_dir));

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_failed_never_overwrites_successful() {
        let persister = ResultPersister::new();
        let temp_dir = create_temp_dir();

        // First save a successful result
        let success_result = create_test_result("test", "rust", true);
        let saved_path = persister.save_result(&success_result, "claude", "sonnet", &temp_dir).unwrap();

        // Now try to save a failed result for the same exercise (retry mode)
        let failed_result = create_test_result("test", "rust", false);
        let _ = persister.save_result(&failed_result, "claude", "sonnet", &temp_dir).unwrap();

        // The file should still show success
        let content = std::fs::read_to_string(&saved_path).unwrap();
        let value: serde_json::Value = serde_json::from_str(&content).unwrap();
        assert_eq!(value.get("success").and_then(|v| v.as_bool()), Some(true));

        assert!(persister.result_file_success("test", "claude", "sonnet", "rust", &temp_dir));

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_print_summary() {
        let persister = ResultPersister::new();
        let results = vec![
            create_test_result("test1", "java", true),
            create_test_result("test2", "java", true),
            create_test_result("test3", "java", false),
        ];

        // Should not panic
        persister.print_summary(&results);
    }

    #[test]
    fn test_print_summary_empty() {
        let persister = ResultPersister::new();
        persister.print_summary(&[]);
    }

    #[test]
    fn test_save_result_with_trace() {
        // Traces are no longer embedded in AgentResult — they're saved as separate files.
        // This test verifies save_result still works without a trace field.
        let persister = ResultPersister::new();
        let temp_dir = create_temp_dir();
        let result = create_test_result("test", "java", true);

        let _ = persister.save_result(&result, "reference", "default", &temp_dir).unwrap();

        let target_dir = temp_dir.join("reference-default");
        let result_file = target_dir.join("result_reference_java_test.json");
        assert!(result_file.exists());

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

}

