use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use tokio::time::Instant;
use tracing::{debug, error, info, warn};
use benchmark_types::agent::{Agent, AgentResult};
use benchmark_types::exercise::Exercise;
use walkdir::WalkDir;
use crate::docker::DockerClient;
use crate::agent::PiMessageProcessor;

/// Pi agent that uses the pi coding agent to solve exercises.
/// Extends ReferenceAgent behavior with Pi-specific setup.
pub struct PiAgent {
    docker_client: DockerClient,
    message_processor: Arc<Mutex<PiMessageProcessor>>,
}

impl PiAgent {
    pub fn new(docker_client: DockerClient) -> Self {
        Self {
            docker_client,
            message_processor: Arc::new(Mutex::new(PiMessageProcessor::new(None))),
        }
    }

    /// Set the message processor with an output consumer for web UI streaming.
    pub fn set_message_processor(&mut self, processor: PiMessageProcessor) {
        self.message_processor = Arc::new(Mutex::new(processor));
    }

    /// Creates a models.json configuration file for pi inside the working directory.
    /// Uses the model parameter instead of Docker config env vars (matches Java behavior).
    fn create_models_json(&self, temp_work_dir: &Path, model: &str) -> std::io::Result<()> {
        let pi_agent_dir = temp_work_dir.join(".pi").join("agent");
        fs::create_dir_all(&pi_agent_dir)?;

        // Read environment configuration from Docker config
        let env_vars = self.docker_client.get_config().environment().cloned().unwrap_or_default();

        // Use OpenAI endpoint (derived from ANTHROPIC_BASE_URL with /v1 suffix)
        let base_url = env_vars
            .get("OPENAI_BASE_URL")
            .map(|s| s.as_str())
            .unwrap_or("http://host.docker.internal:8000/v1");

        let api_key = env_vars
            .get("OPENAI_API_KEY")
            .map(|s| s.as_str())
            .unwrap_or_else(|| {
                env_vars
                    .get("ANTHROPIC_AUTH_TOKEN")
                    .map(|s| s.as_str())
                    .unwrap_or("placeholder-key")
            });

        let models_json = format!(
            "{{\n  \"providers\": {{\n    \"openai\": {{\n      \"baseUrl\": \"{}\",\n      \"apiKey\": \"{}\",\n      \"api\": \"openai-completions\",\n      \"models\": [\n        {{ \"id\": \"{}\" }}\n      ]\n    }}\n  }}\n}}",
            self.escape_json(base_url),
            self.escape_json(api_key),
            self.escape_json(model)
        );

        let models_file = pi_agent_dir.join("models.json");
        fs::write(&models_file, &models_json)?;
        debug!("Created models.json at: {:?} with OpenAI provider", models_file);
        Ok(())
    }

    /// Escapes special characters for JSON string values.
    fn escape_json(&self, value: &str) -> String {
        value
            .replace('\\', "\\\\")
            .replace('"', "\\\"")
            .replace('\n', "\\n")
            .replace('\r', "\\r")
            .replace('\t', "\\t")
    }

    /// Installs Pi extensions (bash-timeout) into the working directory.
    fn install_pi_extensions(&self, temp_work_dir: &Path) -> std::io::Result<()> {
        let pi_extension_dir = temp_work_dir
            .join(".pi")
            .join("agent")
            .join("extensions")
            .join("bash-timeout");
        fs::create_dir_all(&pi_extension_dir)?;

        let target_path = pi_extension_dir.join("index.ts");
        // Use the same bash-timeout.ts content as the Java version
        let content = include_str!("../../resources/bash-timeout.ts");
        fs::write(&target_path, content)?;
        Ok(())
    }

    /// Collects trace information from pi session files and exports to HTML.
    async fn collect_pi_trace(
        &self,
        temp_work_dir: &Path,
        results_dir: &Path,
        exercise: &Exercise,
        model: &str,
    ) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        let pi_sessions_dir = temp_work_dir
            .join(".pi")
            .join("agent")
            .join("sessions");

        if !pi_sessions_dir.exists() {
            warn!("No pi sessions directory found at: {:?}", pi_sessions_dir);
            return Ok(String::new());
        }

        info!("Found pi sessions directory at: {:?}", pi_sessions_dir);

        let mut html_traces: Vec<String> = Vec::new();
        let mut jsonl_files: Vec<PathBuf> = Vec::new();

        // Walk the sessions directory for session files
        for entry in WalkDir::new(&pi_sessions_dir) {
            let entry = entry?;
            let path = entry.path();

            if !path.is_file() {
                continue;
            }

            let file_name = path.file_name().unwrap().to_string_lossy().to_string();
            info!("Processing pi session file: {}", file_name);

            if file_name.ends_with(".jsonl") {
                // Copy JSONL trace files — NO agent in trace filename (per Java convention)
                let target_name = format!("trace_{}_{}.jsonl", exercise.language, exercise.name);
                let target_dir = results_dir.join(format!("pi-{}", model));
                let _ = fs::create_dir_all(&target_dir);
                let target_path = target_dir.join(&target_name);
                fs::copy(path, &target_path)?;
                info!("Copied pi JSONL trace file: {}", target_name);
                jsonl_files.push(path.to_path_buf());
            } else if file_name.ends_with(".json") {
                let target_name =
                    format!("log_pi_{}_{}_{}", exercise.language, exercise.name, file_name);
                let target_dir = results_dir.join(format!("pi-{}", model));
                let _ = fs::create_dir_all(&target_dir);
                let target_path = target_dir.join(&target_name);
                let _ = fs::copy(path, &target_path);
                info!("Copied pi JSON log file: {}", target_name);
            } else if file_name.ends_with(".html") {
                let html_content = fs::read_to_string(path)?;
                let len = html_content.len();
                html_traces.push(html_content);
                info!("Found HTML trace with {} chars", len);
            }
        }

        // Export JSONL files to HTML using pi --export command
        if !jsonl_files.is_empty() {
            info!(
                "Exporting {} JSONL trace file(s) to HTML",
                jsonl_files.len()
            );

            for jsonl_file in &jsonl_files {
                let base_name = jsonl_file
                    .file_name()
                    .unwrap()
                    .to_string_lossy()
                    .to_string()
                    .replace(".jsonl", "");

                // The .pi volume mount causes pi to crash during export because it
                // tries to create its own session directory inside the bind-mounted path.
                // Solution: copy the JSONL to /workspace root (which IS mounted) and
                // run export without the .pi volume mount.
                let jsonl_filename = jsonl_file.file_name().unwrap();
                let host_copy_path = temp_work_dir.join(jsonl_filename);
                fs::copy(jsonl_file, &host_copy_path)?;
                let container_jsonl_path = PathBuf::from("/workspace").join(jsonl_filename);
                let container_html_path = PathBuf::from("/workspace").join(format!("{}.html", base_name));

                info!(
                    "Exporting from {} to {} (container paths)",
                    container_jsonl_path.display(),
                    container_html_path.display()
                );

                // Run: pi --export <jsonl_file> <html_file>
                // No .pi volume mount — just /workspace where the JSONL was copied.
                let export_result = self
                    .docker_client
                    .run_command_with_limits_and_volume(
                        None,
                        Some("/workspace"),
                        &[
                            "pi",
                            "--export",
                            container_jsonl_path.to_str().unwrap(),
                            container_html_path.to_str().unwrap(),
                        ],
                        Some(60),
                        None,
                        Some(&temp_work_dir.to_string_lossy()),
                    )
                    .await;

                if let Ok(ref result) = export_result {
                    info!(
                        "Export completed. Success: {:?}, Exit code: {:?}",
                        result.completed, result.exit_code
                    );
                    if !result.output.is_empty() {
                        let preview = &result.output[..result.output.len().min(1000)];
                        info!("Export output: {}", preview);
                    }
                } else {
                    warn!("Export failed to execute");
                }

                // Check if HTML file exists on host side (copied to /workspace)
                let host_html_file = temp_work_dir.join(format!("{}.html", base_name));

                if host_html_file.exists() {
                    info!("Found HTML file at: {:?}", host_html_file);
                    let html_content = fs::read_to_string(&host_html_file)?;
                    html_traces.push(html_content);
                    info!("Read HTML trace with {} chars", html_traces.last().unwrap().len());

                    // Copy to results directory with standard naming
                    let html_target_name =
                        format!("trace_{}_{}.html", exercise.language, exercise.name);
                    let target_dir = results_dir.join(format!("pi-{}", model));
                    let _ = fs::copy(
                        &host_html_file,
                        target_dir.join(&html_target_name),
                    );
                    info!("Copied HTML trace to results directory: {}", html_target_name);
                } else {
                    warn!("HTML file not found at: {:?}", host_html_file);
                    // List what's actually in the temp work dir
                    if let Ok(entries) = fs::read_dir(temp_work_dir) {
                        let files: Vec<String> = entries
                            .filter_map(|e| e.ok())
                            .map(|e| e.file_name().to_string_lossy().to_string())
                            .collect();
                        warn!("Contents of temp work dir: {:?}", files);
                    }
                }
            }
        }

        if html_traces.is_empty() {
            warn!("No HTML traces found or generated");
        } else {
            info!("Found/generated {} HTML trace(s)", html_traces.len());
        }

        Ok(html_traces
            .first()
            .cloned()
            .unwrap_or_default())
    }

    /// Builds the command line arguments for invoking pi.
    fn build_pi_command(&self, prompt: &str, model: &str) -> Vec<String> {
        let mut command = vec![
            "pi".to_string(),
            "--mode".to_string(),
            "json".to_string(),
            "--tools".to_string(),
            "read,bash,edit,write,grep,find,ls".to_string(),
            "--provider".to_string(),
            "openai".to_string(),
            "--model".to_string(),
            model.to_string(),
        ];
        command.push(prompt.to_string());
        command
    }
}

#[async_trait::async_trait]
impl Agent for PiAgent {
    async fn run_exercise(
        &self,
        exercise: &Exercise,
        host_exercise_dir: &Path,
        model: &str,
        results_dir: &Path,
    ) -> Result<AgentResult, Box<dyn std::error::Error + Send + Sync>> {
        let start_time = Instant::now();
        let start_dt = chrono::Utc::now();
        info!(
            "Running Pi agent for exercise: {}",
            exercise.name
        );

        // Create temporary working directory
        let base_temp_dir = std::env::current_dir()?.join(".benchmark-temp");
        fs::create_dir_all(&base_temp_dir)?;

        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis();
        let temp_work_dir = base_temp_dir.join(&exercise.name).join(ts.to_string());
        fs::create_dir_all(&temp_work_dir)?;
        info!("Created temporary work directory: {:?}", temp_work_dir);

        // Copy exercise files
        // For C++, create a subdirectory named after the exercise
        let exercise_dest = if exercise.language == "cpp" {
            let dest = temp_work_dir.join(&exercise.name);
            fs::create_dir_all(&dest)?;
            info!("Copying C++ exercise files to {}", dest.display());
            dest
        } else {
            temp_work_dir.clone()
        };

        for entry in WalkDir::new(host_exercise_dir) {
            let entry = entry?;
            let source_path = entry.path();

            if source_path.is_dir() {
                let relative = source_path.strip_prefix(host_exercise_dir).unwrap_or(source_path);
                // Skip .meta directory
                if relative.to_string_lossy().contains(".meta") {
                    continue;
                }
                let dest = exercise_dest.join(relative);
                fs::create_dir_all(&dest)?;
            } else {
                let path_str = source_path.to_string_lossy();
                if path_str.contains(".meta/src/reference") {
                    debug!("Skipping reference file: {:?}", source_path);
                    continue;
                }

                let relative = source_path.strip_prefix(host_exercise_dir).unwrap_or(source_path);
                let dest = exercise_dest.join(relative);
                if let Some(parent) = dest.parent() {
                    let _ = fs::create_dir_all(parent);
                }
                fs::copy(source_path, &dest)?;
            }
        }

        // For Rust exercises, copy Cargo-example.toml to Cargo.toml if it exists
        if exercise.language == "rust" {
            let cargo_example = host_exercise_dir.join(".meta").join("Cargo-example.toml");
            if cargo_example.exists() {
                let dest = temp_work_dir.join("Cargo.toml");
                fs::copy(&cargo_example, &dest)?;
                info!("Copied Cargo-example.toml to Cargo.toml");
            }
        }

        // Patch tests (remove @Disabled, #[ignore], xtest) — matches Java behavior
        crate::agent::test_patches::run_patch_tests(exercise, &temp_work_dir)?;

        // Install Pi extensions
        self.install_pi_extensions(&temp_work_dir)?;

        // Create exercise prompt
        let prompt = Self::create_exercise_prompt(exercise, &temp_work_dir)?;

        // Use model from run_exercise parameter (matches Java behavior)
        let model = model.to_string();

        // Create models.json with the correct model from queue
        self.create_models_json(&temp_work_dir, &model)?;

        // Build and run pi command
        let command = self.build_pi_command(&prompt, &model);
        let command_refs: Vec<&str> = command.iter().map(|s| s.as_str()).collect();

        // Log environment and configuration for debugging
        let env_vars = self.docker_client.get_config().environment().cloned().unwrap_or_default();
        info!(
            "Running Pi agent for exercise: {} (language: {})",
            exercise.name, exercise.language
        );
        info!("Temp work dir: {:?}", temp_work_dir);
        info!("Container work dir: /workspace{}", if exercise.language == "cpp" { "/<exercise_name>" } else { "" });
        info!("Model: {}", model);
        info!("OPENAI_BASE_URL: {}", env_vars.get("OPENAI_BASE_URL").map(|s| s.as_str()).unwrap_or("NOT SET"));
        info!("OPENAI_API_KEY set: {}", env_vars.get("OPENAI_API_KEY").map(|_| "yes").unwrap_or("no"));
        info!("ANTHROPIC_MODEL: {}", env_vars.get("ANTHROPIC_MODEL").map(|s| s.as_str()).unwrap_or("NOT SET"));
        info!("ANTHROPIC_AUTH_TOKEN set: {}", env_vars.get("ANTHROPIC_AUTH_TOKEN").map(|_| "yes").unwrap_or("no"));


        let processor = Arc::clone(&self.message_processor);
        // For C++, use subdirectory as work dir
        let container_work_dir = if exercise.language == "cpp" {
            format!("/workspace/{}", exercise.name)
        } else {
            "/workspace".to_string()
        };
        let result = self
            .docker_client
            .run_command_with_limits_and_volume_with_callback(
                None,
                Some(&container_work_dir),
                &command_refs,
                None,
                None,
                Some(&temp_work_dir.to_string_lossy()),
                Some(std::sync::Arc::new(move |line| {
                    let proc = processor.lock().unwrap();
                    proc.process(line);
                })),
                true,  // enable .pi volume mount for session data (matches Java)
            )
            .await?;

        let end_dt = chrono::Utc::now();
        let duration_ms = start_time.elapsed().as_millis() as u64;
        let success = result.completed && result.exit_code == 0;

        if !success {
            let output_preview = if result.output.len() > 1000 {
                format!("{}...[truncated]", &result.output[..1000])
            } else {
                result.output.clone()
            };
            error!(
                "Pi agent exercise FAILED: {} (language: {})\n\
                 Exit code: {}\n\
                 Completed: {}\n\
                 Container ID: {}\n\
                 Output (first 1000 chars): {}",
                exercise.name,
                exercise.language,
                result.exit_code,
                result.completed,
                result.container_id,
                output_preview
            );
        } else {
            info!(
                "Exercise completed successfully: {}. Duration: {}ms",
                exercise.name, duration_ms
            );
        }

        // Collect trace from pi session files — use the agent-model subdirectory
        let model_str = model.to_string();
        let _trace = self
            .collect_pi_trace(&temp_work_dir, &results_dir, exercise, &model_str)
            .await
            .unwrap_or_default();

        // Cleanup
        let _ = fs::remove_dir_all(&temp_work_dir);

        Ok(AgentResult::builder()
            .exercise_name(exercise.name.clone())
            .language(exercise.language.clone())
            .success(success)
            .exit_code(result.exit_code)
            .output(String::new()) // Don't store raw output - trace is saved separately
            .duration_ms(duration_ms)
            .start_time(start_dt.to_rfc3339())
            .end_time(end_dt.to_rfc3339())
            .error_message(if !success {
                Some(format!(
                    "Exit code: {}",
                    result.exit_code
                ))
            } else {
                None
            })

            .container_id(result.container_id)
            .build())
    }

    fn get_name(&self) -> &str {
        "pi"
    }
}

impl PiAgent {
    /// Creates a prompt for AI agents to solve the exercise.
    /// Shared by all agent implementations.
    fn create_exercise_prompt(
        exercise: &Exercise,
        temp_work_dir: &Path,
    ) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        let mut prompt = String::new();

        let instructions_path = temp_work_dir.join(".docs").join("instructions.md");
        if instructions_path.exists() {
            prompt.push_str(&fs::read_to_string(&instructions_path)?);
        } else {
            prompt.push_str("Please solve the following programming exercise.\n\n");
            prompt.push_str(&format!("Exercise: {}\n", exercise.name));
            prompt.push_str(&format!("Language: {}\n\n", exercise.language));
            prompt.push_str("IMPORTANT RULES:\n");
            prompt.push_str("1. Implement the solution in the source files only, do not touch the test files.\n");
            prompt.push_str("2. Run the tests to verify your solution\n\n");
            prompt.push_str("3. The tests are validated to be correct, never assume the test to be wrong!\n\n");
            prompt.push_str(
                "4. Do not run tests in the background, run them synchronously in the foreground.\n",
            );
            prompt.push_str(
                "5. When you have validated the test cases execute correctly, the original test sources will be copied back into the workspace to make sure you did not tamper with them!",
            );
        }

        // Add test file location.
        // The agent runs inside a Docker container where the exercise files
        // are mounted at /workspace/<language>/exercises/practice/<name>/.
        // We translate the host path to a container path by replacing the
        // polyglot-benchmark prefix with /workspace (matching Java ReferenceAgent).
        if let Some(test_path) = &exercise.test_path {
            if test_path.exists() {
                let needle = format!(
                    "../polyglot-benchmark/{}/exercises/practice/{}",
                    exercise.language, exercise.name
                );
                let fixed_test_path = test_path
                    .to_string_lossy()
                    .replace(&needle, "/workspace/");
                prompt.push_str(&format!("Test file location: {}\n", fixed_test_path));
            }
        }

        prompt.push_str("\nImplement the solution directly, do not ask me to review.\n");

        // Add language-specific instructions
        match exercise.language.as_str() {
            "java" => {
                prompt.push_str(
                    "\nDo not stop working until you have executed the test suite (./gradlew test --no-daemon) and you have validated that the tests succeed!\n",
                );
            }
            "javascript" => {
                prompt.push_str("\nRun tests with: npm install && npm run test\n");
                prompt.push_str("This exercise uses Jest as the test framework.\n");
            }
            "python" => {
                prompt.push_str("\nUse uv to create a virtual environment and run tests:\n");
                prompt.push_str("1. Create venv: uv venv (or use existing .venv)\n");
                prompt.push_str("2. Activate: . .venv/bin/activate\n");
                prompt.push_str("3. Install pytest: uv pip install pytest\n");
                prompt.push_str("4. Run tests: pytest\n");
            }
            "rust" => {
                prompt.push_str("\nRun tests with: cargo test\n");
                prompt.push_str("Use cargo test to validate all tests succeed.\n");
            }
            "cpp" => {
                prompt.push_str("\nBuild with: mkdir -p build && cd build && cmake -DEXERCISM_RUN_ALL_TESTS=1 -G \"Unix Makefiles\" .. && make\n");
                prompt.push_str("Run tests with: ./build/tests or the test executable in the build directory.\n");
            }
            "go" => {
                prompt.push_str("\nRun tests with: go test\n");
            }
            _ => {}
        }

        prompt.push_str(
            "<important>Check that no tests are skipped, enable any tests that shows as skipped in the test results! Any skipped tests will result in failure!</important>\n",
        );

        // Append agent execution instructions (from prompt.md resource)
        let prompt_instructions = include_str!("../../../../benchmark-web/resources/prompt.md");
        prompt.push_str(prompt_instructions);

        Ok(prompt)
    }
}
