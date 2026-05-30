use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use tokio::time::Instant;
use tracing::{debug, error, info, warn};
use benchmark_types::agent::{Agent, AgentResult};
use benchmark_types::exercise::Exercise;
use walkdir::WalkDir;
use crate::docker::DockerClient;

/// Reference agent that copies reference implementation and runs tests.
pub struct ReferenceAgent {
    docker_client: DockerClient,
    /// Optional callback for live output streaming.
    output_consumer: Arc<Mutex<Option<Box<dyn Fn(&str) + Send + Sync>>>>,
}

impl ReferenceAgent {
    pub fn new(docker_client: DockerClient) -> Self {
        Self {
            docker_client,
            output_consumer: Arc::new(Mutex::new(None)),
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
    fn create_temp_work_dir(exercise: &Exercise) -> Result<PathBuf, std::io::Error> {
        let base_dir = std::env::current_dir()?;
        let base_temp_dir = base_dir.join(".benchmark-temp");
        fs::create_dir_all(&base_temp_dir)?;

        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis();
        let exercise_temp_dir = base_temp_dir.join(&exercise.name).join(ts.to_string());
        fs::create_dir_all(&exercise_temp_dir)?;
        Ok(exercise_temp_dir)
    }

    /// Copies exercise files to temp directory, excluding reference implementation.
    fn copy_exercise_files(
        _exercise: &Exercise,
        source_dir: &Path,
        dest_dir: &Path,
    ) -> Result<(), std::io::Error> {
        info!(
            "Copying exercise files from {:?} to {:?}",
            source_dir, dest_dir
        );

        let walker = WalkDir::new(source_dir).into_iter();
        for entry in walker {
            let entry = entry?;
            let source_path = entry.path();

            if source_path.is_dir() {
                let relative = source_path.strip_prefix(source_dir).unwrap_or(source_path);
                let dest = dest_dir.join(relative);
                fs::create_dir_all(&dest)?;
            } else {
                // Skip reference implementation directory
                let path_str = source_path.to_string_lossy();
                if path_str.contains(".meta/src/reference") {
                    debug!("Skipping reference file: {:?}", source_path);
                    continue;
                }

                let relative = source_path.strip_prefix(source_dir).unwrap_or(source_path);
                let dest = dest_dir.join(relative);
                fs::copy(source_path, &dest)?;

                // Handle gradle-wrapper.properties modification
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
        Ok(())
    }

    /// Runs the reference agent (copies reference implementation).
    fn run_reference_impl(
        exercise: &Exercise,
        temp_dir: &Path,
    ) -> Result<AgentResult, Box<dyn std::error::Error + Send + Sync>> {
        let start_time = Instant::now();

        if let Some(ref_ref) = &exercise.reference_path {
            Self::copy_reference_impl(exercise, temp_dir, ref_ref);
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
    fn copy_reference_impl(exercise: &Exercise, temp_dir: &Path, ref_dir: &Path) {
        if !ref_dir.exists() {
            warn!("Reference directory not found for: {}", exercise.name);
            return;
        }

        match exercise.language.as_str() {
            "java" => {
                let main_src_dir = temp_dir.join("src/main/java");
                let _ = fs::create_dir_all(&main_src_dir);

                info!(
                    "Copying reference implementation from {:?} to {:?}",
                    ref_dir, main_src_dir
                );

                for entry in WalkDir::new(ref_dir) {
                    match entry {
                        Ok(entry) => {
                            let ref_file = entry.path();
                            if ref_file
                                .extension()
                                .map(|e| e == "java")
                                .unwrap_or(false)
                            {
                                let file_name = ref_file
                                    .file_name()
                                    .unwrap()
                                    .to_string_lossy();
                                let dest_file = main_src_dir.join(&*file_name);
                                let _ = fs::copy(ref_file, &dest_file);
                                info!("Copied reference file: {}", file_name);
                            }
                        }
                        Err(e) => warn!("Error walking directory: {}", e),
                    }
                }
            }
            "go" => {
                for entry in WalkDir::new(ref_dir) {
                    match entry {
                        Ok(entry) => {
                            let ref_file = entry.path();
                            if ref_file
                                .extension()
                                .map(|e| e == "go")
                                .unwrap_or(false)
                            {
                                let file_name = ref_file
                                    .file_name()
                                    .unwrap()
                                    .to_string_lossy();
                                let dest_file = temp_dir.join(&*file_name);
                                let _ = fs::copy(ref_file, &dest_file);
                                info!("Copied reference file: {}", file_name);
                            }
                        }
                        Err(e) => warn!("Error walking directory: {}", e),
                    }
                }
            }
            "rust" => {
                let src_dir = temp_dir.join("src");
                let _ = fs::create_dir_all(&src_dir);

                info!(
                    "Copying reference implementation from {:?} to {:?}",
                    ref_dir, src_dir
                );

                for entry in WalkDir::new(ref_dir) {
                    match entry {
                        Ok(entry) => {
                            let ref_file = entry.path();
                            if ref_file
                                .extension()
                                .map(|e| e == "rs")
                                .unwrap_or(false)
                            {
                                let relative = ref_file.strip_prefix(ref_dir).unwrap_or(ref_file);
                                let dest_file = src_dir.join(relative);
                                if let Some(parent) = dest_file.parent() {
                                    let _ = fs::create_dir_all(parent);
                                }
                                let _ = fs::copy(ref_file, &dest_file);
                                info!("Copied reference file: {:?}", relative);
                            }
                        }
                        Err(e) => warn!("Error walking directory: {}", e),
                    }
                }
            }
            "javascript" | "typescript" => {
                info!(
                    "Copying reference implementation from {:?} to {:?}",
                    ref_dir, temp_dir
                );

                for entry in WalkDir::new(ref_dir) {
                    match entry {
                        Ok(entry) => {
                            let ref_file = entry.path();
                            if ref_file
                                .extension()
                                .map(|e| e == "js" || e == "ts")
                                .unwrap_or(false)
                            {
                                let file_name = ref_file
                                    .file_name()
                                    .unwrap()
                                    .to_string_lossy();
                                let dest_file = temp_dir.join(&*file_name);
                                let _ = fs::copy(ref_file, &dest_file);
                                info!("Copied reference file: {}", file_name);
                            }
                        }
                        Err(e) => warn!("Error walking directory: {}", e),
                    }
                }
            }
            "python" => {
                info!(
                    "Copying reference implementation from {:?} to {:?}",
                    ref_dir, temp_dir
                );

                for entry in WalkDir::new(ref_dir) {
                    match entry {
                        Ok(entry) => {
                            let ref_file = entry.path();
                            if ref_file
                                .extension()
                                .map(|e| e == "py")
                                .unwrap_or(false)
                            {
                                let file_name = ref_file
                                    .file_name()
                                    .unwrap()
                                    .to_string_lossy();
                                let dest_file = temp_dir.join(&*file_name);
                                let _ = fs::copy(ref_file, &dest_file);
                                info!("Copied reference file: {}", file_name);
                            }
                        }
                        Err(e) => warn!("Error walking directory: {}", e),
                    }
                }
            }
            _ => {
                warn!(
                    "Reference implementation copying not implemented for language: {}",
                    exercise.language
                );
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
        let prepare_command = Self::get_prepare_command(exercise);
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

        let result = self
            .docker_client
            .run_command_with_limits_and_volume(
                None,
                Some("/workspace"),
                &prepare_command.iter().map(|s| s.as_str()).collect::<Vec<&str>>(),
                None,
                None,
                Some(&temp_work_dir.to_string_lossy()),
            )
            .await?;

        if !result.completed || result.exit_code != 0 {
            let err_msg = format!(
                "Workspace preparation failed for {}: {}",
                exercise.language, &result.output[..result.output.len().min(500)]
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
    fn get_prepare_command(exercise: &Exercise) -> Vec<String> {
        let exercises_path = exercise
            .source_path
            .as_ref()
            .map(|p| p.parent())
            .flatten()
            .unwrap_or(Path::new(""));

        match exercise.language.as_str() {
            "javascript" | "typescript" => {
                if exercises_path.join("package.json").exists() {
                    vec!["npm".to_string(), "install".to_string()]
                } else {
                    vec![]
                }
            }
            "python" => {
                // Match Java PythonHandler: create venv if needed
                if exercises_path.join("pyproject.toml").exists() {
                    vec![
                        "sh".to_string(),
                        "-c".to_string(),
                        "if [ ! -d \".venv\" ]; then uv venv; fi".to_string(),
                    ]
                } else if exercises_path.join("requirements.txt").exists() {
                    vec![]
                } else {
                    vec![]
                }
            }
            "java" => {
                // Gradle wrapper is already set up, but we may need to download dependencies
                if exercises_path.join("build.gradle").exists() {
                    vec![
                        "./gradlew".to_string(),
                        "dependencies".to_string(),
                        "--quiet".to_string(),
                    ]
                } else if exercises_path.join("pom.xml").exists() {
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
                if exercises_path.join("go.mod").exists() {
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
    pub fn copy_fresh_tests(
        &self,
        exercise: &Exercise,
        source_dir: &Path,
        dest_dir: &Path,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        info!(
            "Copying fresh test files from {:?} to {:?}",
            source_dir, dest_dir
        );

        // Copy test files based on language
        match exercise.language.as_str() {
            "java" => {
                // Copy test files from src/test/java
                let test_src = source_dir.join("src").join("test").join("java");
                if test_src.exists() {
                    Self::copy_directory_recursive(&test_src, dest_dir)?;
                }
            }
            "javascript" | "typescript" => {
                // Copy test files (usually in __tests__ or test directories)
                for test_dir in &["__tests__", "test", "tests"] {
                    let src = source_dir.join(test_dir);
                    if src.exists() {
                        Self::copy_directory_recursive(&src, dest_dir)?;
                    }
                }
                // Also copy any *.test.js / *.spec.js files
                if let Ok(entries) = fs::read_dir(source_dir) {
                    for entry in entries.flatten() {
                        let path = entry.path();
                        if path.is_file() {
                            let file_name = path.file_name().unwrap().to_string_lossy();
                            if file_name.ends_with(".test.js")
                                || file_name.ends_with(".spec.js")
                                || file_name.ends_with(".test.ts")
                                || file_name.ends_with(".spec.ts")
                            {
                                let dest = dest_dir.join(file_name.as_ref());
                                let _ = fs::copy(&path, &dest);
                            }
                        }
                    }
                }
            }
            "python" => {
                // Copy test files (usually test_*.py or *_test.py)
                if let Ok(entries) = fs::read_dir(source_dir) {
                    for entry in entries.flatten() {
                        let path = entry.path();
                        if path.is_file() {
                            let file_name = path.file_name().unwrap().to_string_lossy();
                            if file_name.starts_with("test_")
                                || file_name.ends_with("_test.py")
                            {
                                let dest = dest_dir.join(file_name.as_ref());
                                let _ = fs::copy(&path, &dest);
                            }
                        }
                    }
                }
            }
            "rust" => {
                // Rust tests are typically in the same file or in src/
                // Already copied via copy_exercise_files
            }
            "go" => {
                // Go test files end with _test.go
                if let Ok(entries) = fs::read_dir(source_dir) {
                    for entry in entries.flatten() {
                        let path = entry.path();
                        if path.is_file() {
                            let file_name = path.file_name().unwrap().to_string_lossy();
                            if file_name.ends_with("_test.go") {
                                let dest = dest_dir.join(file_name.as_ref());
                                let _ = fs::copy(&path, &dest);
                            }
                        }
                    }
                }
            }
            "cpp" => {
                // C++ tests may be in various locations
                // Already copied via copy_exercise_files
            }
            _ => {}
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
    async fn run_tests_in_docker(
        &self,
        exercise: &Exercise,
        temp_work_dir: &Path,
    ) -> Result<AgentResult, Box<dyn std::error::Error + Send + Sync>> {
        let start_time = Instant::now();
        let command = Self::get_test_command(exercise, temp_work_dir);

        info!(
            "Running tests in Docker container at /workspace (mounted from: {:?})",
            temp_work_dir
        );
        debug!("Command: {}", command.join(" "));

        let result = self
            .docker_client
            .run_command_with_limits_and_volume(
                None,
                Some("/workspace"),
                &command,
                None,
                None,
                Some(&temp_work_dir.to_string_lossy()),
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
            .start_time(duration_ms.to_string())
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
                "mkdir", "-p", "build", "&&", "cd", "build", "&&", "cmake",
                "-DEXERCISM_RUN_ALL_TESTS=1", "-G", "\"Unix Makefiles\"", "..", "&&", "make",
            ]
        } else if polyglot_path.join("Cargo.toml").exists() {
            vec!["cargo", "test"]
        } else if polyglot_path.join("pyproject.toml").exists()
            || polyglot_path.join("setup.py").exists()
        {
            // Match Java PythonHandler: activate venv and install pytest
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
    fn cleanup_temp_dir(temp_dir: &Path) {
        if temp_dir.exists() {
            let _ = fs::remove_dir_all(temp_dir);
        }
    }

    /// Returns the agent's name.
    pub fn get_name(&self) -> &str {
        "reference"
    }
}

#[async_trait::async_trait]
impl Agent for ReferenceAgent {
    async fn run_exercise(
        &self,
        exercise: &Exercise,
        host_exercise_dir: &Path,
        _model: &str,
        _thinking_level: Option<&str>,
        _results_dir: &Path,
    ) -> Result<AgentResult, Box<dyn std::error::Error + Send + Sync>> {
        let start_time = Instant::now();
        let start_dt = chrono::Utc::now();
        info!("Running reference agent for exercise: {}", exercise.name);

        let temp_work_dir = Self::create_temp_work_dir(exercise)?;
        info!("Created temporary work directory: {:?}", temp_work_dir);

        Self::copy_exercise_files(exercise, host_exercise_dir, &temp_work_dir)?;

        // Prepare workspace (npm install, uv pip install, etc.)
        if let Err(e) = self.prepare_workspace(exercise, &temp_work_dir).await {
            warn!("Workspace preparation failed: {}", e);
            // Continue anyway - tests might still work
        }

        let agent_result = Self::run_reference_impl(exercise, &temp_work_dir)?;

        // Patch tests before running
        let _ = self.patch_tests(exercise, &temp_work_dir);

        // Copy fresh tests (original test files)
        let _ = self.copy_fresh_tests(exercise, host_exercise_dir, &temp_work_dir);

        let test_result = self.run_tests_in_docker(exercise, &temp_work_dir).await?;

        Self::cleanup_temp_dir(&temp_work_dir);

        let end_dt = chrono::Utc::now();
        let duration_ms = start_time.elapsed().as_millis() as u64;

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
