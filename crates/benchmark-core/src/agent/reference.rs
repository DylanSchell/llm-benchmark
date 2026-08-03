use std::fs;
use std::path::Path;
use std::sync::{Arc, Mutex};
use tokio::time::Instant;
use tracing::{debug, error, info, warn};
use benchmark_types::agent::{Agent, AgentResult};
use benchmark_types::cancellation::CancellationToken;
use benchmark_types::exercise::Exercise;
use walkdir::WalkDir;
use crate::docker::DockerClient;

/// Reference agent that copies reference implementation and runs tests.
pub struct ReferenceAgent {
    docker_client: DockerClient,
    /// Optional callback for live output streaming.
    output_consumer: Arc<Mutex<Option<Box<dyn Fn(&str) + Send + Sync>>>>,
    /// Session cancellation signal — aborts in-flight Docker runs when fired.
    cancellation_token: Mutex<Option<CancellationToken>>,
}

impl ReferenceAgent {
    pub fn new(docker_client: DockerClient) -> Self {
        Self {
            docker_client,
            output_consumer: Arc::new(Mutex::new(None)),
            cancellation_token: Mutex::new(None),
        }
    }

    /// Sets an output consumer to receive live output during exercise execution.
    /// This is used by the web UI to stream output in real-time via SSE.
    pub fn set_output_consumer<F>(&self, consumer: F)
    where
        F: Fn(&str) + Send + Sync + 'static,
    {
        let mut guard = self.output_consumer.lock().unwrap();
        *guard = Some(Box::new(consumer));
    }

    /// Emits output through the consumer if set.
    fn emit_output(&self, message: &str) {
        if let Some(guard) = self.output_consumer.lock().ok() {
            if let Some(ref consumer) = *guard {
                consumer(message);
            }
        }
    }

    /// Patches test files for the exercise (language-specific modifications).
    /// Removes skip annotations so all tests run.
    pub fn patch_tests(&self, exercise: &Exercise, temp_work_dir: &Path) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        crate::agent::test_patches::run_patch_tests(exercise, temp_work_dir)
    }

    /// Creates a temporary working directory for the exercise.
    /// Runs the reference agent (copies reference implementation).
    fn run_reference_impl(
        exercise: &Exercise,
        temp_dir: &Path,
    ) -> Result<AgentResult, Box<dyn std::error::Error + Send + Sync>> {
        let start_time = Instant::now();

        if !exercise.example_paths.is_empty() {
            Self::copy_reference_impl(exercise, temp_dir);
        } else if exercise.reference_path.is_some() {
            // Fallback: walk the reference directory (for languages whose
            // metadata might not have files.example, like older Go exercises)
            let ref_dir = exercise.reference_path.as_ref().unwrap();
            Self::copy_legacy_reference(exercise, temp_dir, ref_dir);
        } else {
            warn!("No reference implementation found for: {}", exercise.name);
        }

        let duration_ms = start_time.elapsed().as_millis() as u64;

        Ok(AgentResult::builder()
            .exercise_name(exercise.name.clone())
            .language(exercise.language.clone())
            .success(true)
            .exit_code(0)
            .output(String::new())
            .duration_ms(duration_ms)
            .start_time(chrono::Utc::now().to_rfc3339())
            .end_time(chrono::Utc::now().to_rfc3339())
            .build())
    }

    /// Copies reference implementation files to the temp directory.
    fn copy_reference_impl(exercise: &Exercise, temp_dir: &Path) {
        // Use metadata paths from config.json (matching Java's LanguageHandler.copyReference).
        // The .meta/config.json defines files.example → reference implementation paths.
        //
        // For Java: all .java files in src/main/java/ are compiled together, so we copy
        //   examples by their original filename into src/main/java/.
        // For JS/Python/Rust/C++/Go: the reference file must overwrite the stub file, so we
        //   match examples to solution files by filename (or by extension as fallback).

        if exercise.example_paths.is_empty() {
            warn!("No example files in metadata for: {}", exercise.name);
            return;
        }

        let uses_directory_compilation = exercise.language == "java";

        if uses_directory_compilation {
            // Java: copy examples by filename into src/main/java/
            let target_dir = temp_dir.join("src").join("main").join("java");
            let _ = fs::create_dir_all(&target_dir);
            for example_path in &exercise.example_paths {
                if !example_path.exists() { continue; }
                let dest_path = target_dir.join(example_path.file_name().unwrap());
                let _ = fs::copy(example_path, &dest_path);
                info!("Copied reference: {:?}", example_path.file_name().unwrap());
            }
        } else {
            // JS/Python/Rust/C++/Go: match examples to solution files.
            // Solution paths from metadata are relative to exercise_dir and already
            // include directory prefixes (e.g., "src/lib.rs"), so we join with
            // temp_dir directly rather than a subdirectory.
            let target_dir = match exercise.language.as_str() {
                "cpp" => temp_dir.join(&exercise.name),
                _ => temp_dir.to_path_buf(),
            };
            let _ = fs::create_dir_all(&target_dir);

            let mut used_solutions: std::collections::HashSet<&std::path::PathBuf> =
                std::collections::HashSet::new();

            for example_path in &exercise.example_paths {
                if !example_path.exists() { continue; }

                let example_name = example_path.file_name().unwrap();
                let example_ext = example_path.extension().and_then(|e| e.to_str()).unwrap_or("");

                // Find matching solution: prefer exact filename, fall back to extension
                let matching = exercise.solution_paths.iter()
                    .filter(|s| !used_solutions.contains(*s))
                    .find(|s| {
                        s.file_name().and_then(|n| n.to_str()) == example_name.to_str()
                    })
                    .or_else(|| {
                        exercise.solution_paths.iter()
                            .filter(|s| !used_solutions.contains(*s))
                            .find(|s| s.extension().and_then(|e| e.to_str()) == Some(example_ext))
                    });

                let dest_path = if let Some(solution_path) = matching {
                    used_solutions.insert(solution_path);
                    if let Some(ref exercise_dir) = exercise.exercise_dir {
                        if let Ok(relative) = solution_path.strip_prefix(exercise_dir) {
                            target_dir.join(relative)
                        } else {
                            target_dir.join(solution_path.file_name().unwrap())
                        }
                    } else {
                        target_dir.join(solution_path.file_name().unwrap())
                    }
                } else {
                    target_dir.join(example_name)
                };

                if let Some(parent) = dest_path.parent() {
                    let _ = fs::create_dir_all(parent);
                }
                let _ = fs::copy(example_path, &dest_path);
                info!("Copied reference: {:?} -> {:?}", example_name, dest_path.file_name().unwrap());
            }
        }

        // Also copy Cargo-example.toml for Rust
        if exercise.language == "rust" {
            if let Some(ref exercise_dir) = exercise.exercise_dir {
                let cargo_example = exercise_dir.join(".meta").join("Cargo-example.toml");
                if cargo_example.exists() {
                    let dest = temp_dir.join("Cargo.toml");
                    let _ = fs::copy(&cargo_example, &dest);
                    info!("Copied Cargo-example.toml to Cargo.toml");
                }
            }
        }
    }

    /// Finds a non-test stub file in a directory.
    /// Returns the path to the first matching file, or None.
    /// Legacy fallback: copies reference files by walking a directory.
    /// Used when metadata (config.json) is unavailable.
    fn copy_legacy_reference(exercise: &Exercise, temp_dir: &Path, ref_dir: &Path) {
        if !ref_dir.exists() {
            warn!("Reference directory not found for: {}", exercise.name);
            return;
        }

        match exercise.language.as_str() {
            "java" => {
                let main_src_dir = temp_dir.join("src/main/java");
                let _ = fs::create_dir_all(&main_src_dir);
                info!("Copying reference implementation from {:?} to {:?}", ref_dir, main_src_dir);
                for entry in WalkDir::new(ref_dir) {
                    if let Ok(entry) = entry {
                        let ref_file = entry.path();
                        if ref_file.extension().map(|e| e == "java").unwrap_or(false) {
                            let file_name = ref_file.file_name().unwrap().to_string_lossy();
                            let dest_file = main_src_dir.join(&*file_name);
                            let _ = fs::copy(ref_file, &dest_file);
                            info!("Copied reference file: {}", file_name);
                        }
                    }
                }
            }
            "go" => {
                for entry in WalkDir::new(ref_dir) {
                    if let Ok(entry) = entry {
                        let ref_file = entry.path();
                        if ref_file.extension().map(|e| e == "go").unwrap_or(false) {
                            let file_name = ref_file.file_name().unwrap().to_string_lossy();
                            let dest_file = temp_dir.join(&*file_name);
                            let _ = fs::copy(ref_file, &dest_file);
                            info!("Copied reference file: {}", file_name);
                        }
                    }
                }
            }
            _ => {
                warn!("Legacy reference copy not implemented for: {}", exercise.language);
            }
        }
    }

    /// Prepares the workspace for a language by running setup commands.
    /// This should be called once before running tests to avoid repeating setup.
    pub async fn prepare_workspace(
        &self,
        exercise: &Exercise,
        temp_work_dir: &Path,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let prepare_command = Self::get_prepare_command(exercise, temp_work_dir);
        if prepare_command.is_empty() {
            debug!("No workspace preparation needed for {}", exercise.language);
            return Ok(());
        }

        info!(
            "Preparing workspace for {} exercise: {:?}",
            exercise.language, prepare_command
        );
        self.emit_output(&format!(
            "Preparing workspace for {}...\n",
            exercise.language
        ));

        let cancellation = self.cancellation_token.lock().unwrap().clone();
        let result = self
            .docker_client
            .run_command_with_limits_and_volume(
                None,
                Some("/workspace"),
                &prepare_command.iter().map(|s| s.as_str()).collect::<Vec<&str>>(),
                None,
                None,
                Some(&temp_work_dir.to_string_lossy()),
                cancellation,
            )
            .await?;

        if !result.completed || result.exit_code != 0 {
            let err_msg = format!(
                "Workspace preparation failed for {}: {}",
                exercise.language, crate::safe_truncate(&result.output, 500)
            );
            error!("{}", err_msg);
            self.emit_output(&format!("{}\n", err_msg));
            return Err(err_msg.into());
        }

        info!("Workspace prepared successfully for {}", exercise.language);
        self.emit_output(&format!("Workspace prepared for {}\n", exercise.language));
        Ok(())
    }

    /// Gets the workspace preparation command for a language.
    /// Checks the temp_work_dir (not the original source path) since the
    /// preparation command runs inside the Docker container against the
    /// temp directory's contents.
    fn get_prepare_command(exercise: &Exercise, temp_work_dir: &Path) -> Vec<String> {
        match exercise.language.as_str() {
            "javascript" | "typescript" => {
                if temp_work_dir.join("package.json").exists() {
                    vec!["npm".to_string(), "install".to_string()]
                } else {
                    vec![]
                }
            }
            "python" => {
                // Match Java PythonHandler: always create venv if needed
                // Python Exercism exercises don't have pyproject.toml or setup.py
                vec![
                    "sh".to_string(),
                    "-c".to_string(),
                    "if [ ! -d \".venv\" ]; then uv venv; fi".to_string(),
                ]
            }
            "java" => {
                // Gradle wrapper is already set up, but we may need to download dependencies.
                // Use --no-daemon to prevent daemon processes from persisting after the build.
                if temp_work_dir.join("build.gradle").exists() {
                    vec![
                        "./gradlew".to_string(),
                        "dependencies".to_string(),
                        "--no-daemon".to_string(),
                        "--quiet".to_string(),
                    ]
                } else if temp_work_dir.join("pom.xml").exists() {
                    vec!["mvn".to_string(), "dependency:resolve".to_string(), "-q".to_string()]
                } else {
                    vec![]
                }
            }
            "rust" => {
                // Cargo will download dependencies on build/test
                vec![]
            }
            "go" => {
                if temp_work_dir.join("go.mod").exists() {
                    vec!["go".to_string(), "mod".to_string(), "download".to_string()]
                } else {
                    vec![]
                }
            }
            "cpp" => {
                // C++ doesn't typically need workspace preparation
                vec![]
            }
            _ => vec![],
        }
    }

    /// Copies fresh test files from the source directory to the temp directory.
    /// Uses metadata test paths from config.json (matching Java's LanguageHandler.copyTests).
    pub fn copy_fresh_tests(
        &self,
        exercise: &Exercise,
        _source_dir: &Path,
        dest_dir: &Path,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Use metadata test paths when available (config.json → files.test)
        if !exercise.test_paths.is_empty() {
            info!("Copying {} fresh test files from metadata", exercise.test_paths.len());
            for test_path in &exercise.test_paths {
                if !test_path.exists() {
                    warn!("Test file not found: {:?}", test_path);
                    continue;
                }
                // Determine destination by stripping exercise_dir prefix
                let dest_path = if let Some(ref exercise_dir) = exercise.exercise_dir {
                    if let Ok(relative) = test_path.strip_prefix(exercise_dir) {
                        let dest = dest_dir.join(relative);
                        if let Some(parent) = dest.parent() {
                            let _ = fs::create_dir_all(parent);
                        }
                        dest
                    } else {
                        dest_dir.join(test_path.file_name().unwrap())
                    }
                } else {
                    dest_dir.join(test_path.file_name().unwrap())
                };
                let _ = fs::copy(test_path, &dest_path);
                info!("Copied fresh test: {:?}", test_path.file_name().unwrap());
            }
            return Ok(());
        }

        // Fallback for exercises without metadata
        match exercise.language.as_str() {
            "java" => {
                let test_src = exercise.exercise_dir.as_ref()
                    .map(|d| d.join("src").join("test").join("java"));
                if let Some(ref src) = test_src {
                    if src.exists() {
                        let test_dest = dest_dir.join("src").join("test").join("java");
                        Self::copy_directory_recursive(src, &test_dest)?;
                    }
                }
            }
            _ => {
                info!("copy_fresh_tests: using metadata-based paths for {}", exercise.language);
            }
        }

        Ok(())
    }

    /// Helper: recursively copy a directory.
    fn copy_directory_recursive(src: &Path, dest: &Path) -> Result<(), std::io::Error> {
        if !dest.exists() {
            fs::create_dir_all(dest)?;
        }
        for entry in fs::read_dir(src)? {
            let entry = entry?;
            let path = entry.path();
            let dest_path = dest.join(entry.file_name());
            if path.is_dir() {
                Self::copy_directory_recursive(&path, &dest_path)?;
            } else {
                fs::copy(&path, &dest_path)?;
            }
        }
        Ok(())
    }

    /// Runs tests inside a Docker container.
    pub async fn run_tests_in_docker(
        &self,
        exercise: &Exercise,
        temp_work_dir: &Path,
    ) -> Result<AgentResult, Box<dyn std::error::Error + Send + Sync>> {
        let start_time = Instant::now();

        // For C++, files are in a subdirectory named after the exercise
        let exercise_dir = if exercise.language == "cpp" {
            temp_work_dir.join(&exercise.name)
        } else {
            temp_work_dir.to_path_buf()
        };

        let command = Self::get_test_command(exercise, &exercise_dir);

        // Container work dir: C++ uses subdirectory, others use /workspace
        let container_work_dir = if exercise.language == "cpp" {
            format!("/workspace/{}", exercise.name)
        } else {
            "/workspace".to_string()
        };

        info!(
            "Running tests in Docker container at {} (mounted from: {:?})",
            container_work_dir, temp_work_dir
        );
        debug!("Command: {}", command.join(" "));

        let cancellation = self.cancellation_token.lock().unwrap().clone();
        let result = self
            .docker_client
            .run_command_with_limits_and_volume(
                None,
                Some(&container_work_dir),
                &command,
                None,
                None,
                Some(&temp_work_dir.to_string_lossy()),
                cancellation,
            )
            .await?;

        let duration_ms = start_time.elapsed().as_millis() as u64;
        let end_dt = chrono::Utc::now();

        // Extract all fields from result before using it
        let output = result.output.clone();
        let exit_code = result.exit_code;
        let container_id = result.container_id;
        let completed = result.completed;
        let success = completed && exit_code == 0 && !Self::contains_test_failures(&output, &exercise.language);

        if success {
            info!(
                "Tests passed for exercise: {}. Duration: {}ms",
                exercise.name, duration_ms
            );
        } else {
            error!(
                "Tests failed for exercise: {}. Exit code: {}, Output: {}",
                exercise.name, exit_code, output
            );
        }

        let output_for_error = if success {
            None
        } else {
            Some(output.clone())
        };

        Ok(AgentResult::builder()
            .exercise_name(exercise.name.clone())
            .language(exercise.language.clone())
            .success(success)
            .exit_code(exit_code)
            .output(output)
            .duration_ms(duration_ms)
            .start_time(chrono::Utc::now().to_rfc3339())
            .end_time(end_dt.to_rfc3339())
            .error_message(output_for_error)
            .container_id(container_id)
            .build())
    }

    /// Gets test command based on build system.
    fn get_test_command<'a>(exercise: &'a Exercise, polyglot_path: &Path) -> Vec<&'a str> {
        if polyglot_path.join("pom.xml").exists() {
            vec!["mvn", "test", "-q"]
        } else if polyglot_path.join("build.gradle").exists() {
            vec!["./gradlew", "test", "--no-daemon", "-q"]
        } else if polyglot_path.join("go.mod").exists() {
            vec!["go", "test"]
        } else if polyglot_path.join("package.json").exists() {
            vec!["npm", "run", "test"]
        } else if polyglot_path.join("CMakeLists.txt").exists() {
            vec![
                "sh",
                "-c",
                "mkdir -p build && cd build && cmake -DEXERCISM_RUN_ALL_TESTS=1 -G \"Unix Makefiles\" .. && make",
            ]
        } else if polyglot_path.join("Cargo.toml").exists() {
            vec!["cargo", "test"]
        } else if exercise.language == "python" {
            // Match Java PythonHandler: always use this command regardless of build files
            // Python Exercism exercises don't have pyproject.toml or setup.py
            vec!["sh", "-c", ". .venv/bin/activate && uv pip install -q pytest && pytest"]
        } else if polyglot_path.join("Gemfile").exists() {
            vec!["bundle", "exec", "rake", "test"]
        } else if Self::has_extension(polyglot_path, "csproj")
            || Self::has_extension(polyglot_path, "sln")
        {
            vec!["dotnet", "test"]
        } else {
            error!(
                "Unable to determine test command for exercise {}",
                exercise.name
            );
            vec!["false"]
        }
    }

    fn has_extension(path: &Path, ext: &str) -> bool {
        if !path.is_dir() {
            return false;
        }
        if let Ok(entries) = std::fs::read_dir(path) {
            for entry in entries.flatten() {
                if let Some(file_ext) = entry.path().extension() {
                    if file_ext == ext {
                        return true;
                    }
                }
            }
        }
        false
    }

    /// Check if test output contains failure patterns.
    /// Checks if the test output contains failure indicators.
    /// This catches cases where the test command returns exit code 0 but tests actually failed.
    /// Note: For Rust, the exit code from cargo test is reliable, so this check is skipped.
    fn contains_test_failures(output: &str, language: &str) -> bool {
        if output.is_empty() {
            return false;
        }

        // For Rust, the exit code from cargo test is reliable
        if language == "rust" {
            return false;
        }

        let lower_output = output.to_lowercase();
        // Common failure patterns from Java test frameworks (Gradle/Maven)
        let patterns = [
            "BUILD FAILED",
            "BUILD FAILURE",
            "Tests FAILED",
            "Test FAILED",
            "FAILED",
            "FAILURE",
        ];

        patterns.iter().any(|p| lower_output.contains(p))
    }

    /// Cleanup temporary directory.
    pub fn get_name(&self) -> &str {
        "reference"
    }
}

#[async_trait::async_trait]
impl Agent for ReferenceAgent {
    fn set_cancellation_token(&self, token: Option<CancellationToken>) {
        *self.cancellation_token.lock().unwrap() = token;
    }

    async fn run_exercise(
        &self,
        exercise: &Exercise,
        host_exercise_dir: &Path,
        model: &str,
        thinking_level: Option<&str>,
        results_dir: &Path,
    ) -> Result<AgentResult, Box<dyn std::error::Error + Send + Sync>> {
        self.run_exercise_with_timeout(exercise, host_exercise_dir, model, thinking_level, results_dir, None).await
    }

    #[tracing::instrument(skip(self), fields(exercise = %exercise.name, language = %exercise.language))]
    async fn run_exercise_with_timeout(
        &self,
        exercise: &Exercise,
        host_exercise_dir: &Path,
        _model: &str,
        _thinking_level: Option<&str>,
        _results_dir: &Path,
        _timeout_override_secs: Option<u64>,
    ) -> Result<AgentResult, Box<dyn std::error::Error + Send + Sync>> {
        let start_time = Instant::now();
        let start_dt = chrono::Utc::now();
        info!("Running reference agent for exercise: {}", exercise.name);

        let temp_work_dir = super::exercise_files::create_temp_work_dir(exercise)?;
        info!("Created temporary work directory: {:?}", temp_work_dir);

        super::exercise_files::copy_exercise_files(exercise, host_exercise_dir, &temp_work_dir)?;

        // Prepare workspace (npm install, uv pip install, etc.)
        if let Err(e) = self.prepare_workspace(exercise, &temp_work_dir).await {
            warn!("Workspace preparation failed: {}", e);
            // Continue anyway - tests might still work
        }

        let agent_result = Self::run_reference_impl(exercise, &temp_work_dir)?;

        // Capture end time after the "agent" phase (copy reference solution).
        // The remaining steps (copy tests, patch, run tests, cleanup) are
        // verification and should not count toward agent execution time.
        let end_dt = chrono::Utc::now();
        let duration_ms = start_time.elapsed().as_millis() as u64;

        // Copy fresh tests (original test files) then patch them to enable all tests.
        // Order matters: copy first, then patch, so @Disabled / #[ignore] / xtest
        // annotations are removed from the freshly-copied test files.
        let _ = self.copy_fresh_tests(exercise, host_exercise_dir, &temp_work_dir);
        let _ = self.patch_tests(exercise, &temp_work_dir);

        let test_result = self.run_tests_in_docker(exercise, &temp_work_dir).await?;

        // Cleanup
        let _ = fs::remove_dir_all(&temp_work_dir);

        Ok(AgentResult::builder()
            .exercise_name(exercise.name.clone())
            .language(exercise.language.clone())
            .success(test_result.success)
            .exit_code(test_result.exit_code)
            .output(format!("{}\n{}", agent_result.output, test_result.output))
            .duration_ms(duration_ms)
            .start_time(start_dt.to_rfc3339())
            .end_time(end_dt.to_rfc3339())
            .error_message(if !agent_result.success {
                agent_result.error_message
            } else {
                test_result.error_message
            })

            .container_id(test_result.container_id)
            .build())
    }

    fn get_name(&self) -> &str {
        "reference"
    }
}
