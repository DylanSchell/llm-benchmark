use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use tokio::time::Instant;
use tracing::{debug, error, info, warn};
use benchmark_types::agent::{Agent, AgentResult};
use benchmark_types::cancellation::CancellationToken;
use benchmark_types::exercise::Exercise;
use walkdir::WalkDir;
use crate::docker::DockerClient;
use crate::agent::{reference::ReferenceAgent, PiMessageProcessor};
use benchmark_types::util::recover_poisoned;


/// Pi agent that uses the pi coding agent to solve exercises.
/// Extends ReferenceAgent behavior with Pi-specific setup.
pub struct PiAgent {
    docker_client: DockerClient,
    message_processor: Arc<Mutex<PiMessageProcessor>>,
    /// Session cancellation signal — aborts in-flight Docker runs when fired.
    cancellation_token: Mutex<Option<CancellationToken>>,
}

impl PiAgent {
    pub fn new(docker_client: DockerClient) -> Self {
        Self {
            docker_client,
            message_processor: Arc::new(Mutex::new(PiMessageProcessor::new(None))),
            cancellation_token: Mutex::new(None),
        }
    }

    /// Returns true if the model name indicates an Anthropic-family model
    /// (Claude, Opus, Sonnet, Haiku) that should be routed through the
    /// llama-swap Anthropic Messages passthrough instead of the OpenAI
    /// compatible endpoint.
    fn is_anthropic_model(model: &str) -> bool {
        let lower = model.to_lowercase();
        ["claude", "opus", "sonnet", "haiku"]
            .iter()
            .any(|keyword| lower.contains(keyword))
    }

    /// Returns the pi provider key to use for the given model: "anthropic"
    /// for Claude/Opus/Sonnet/Haiku models, "openai" otherwise. Must stay in
    /// sync with the provider key used in `create_models_json`.
    fn provider_key_for_model(model: &str) -> &'static str {
        if Self::is_anthropic_model(model) {
            "anthropic"
        } else {
            "openai"
        }
    }

    /// Set the message processor with an output consumer for web UI streaming.
    pub fn set_message_processor(&mut self, processor: PiMessageProcessor) {
        self.message_processor = Arc::new(Mutex::new(processor));
    }

    /// Creates a models.json configuration file for pi inside the working directory.
    /// Uses the model parameter instead of Docker config env vars (matches Java behavior).
    /// Includes reasoning configuration that maps pi thinking levels to backend-specific parameters.
    fn create_models_json(
        &self,
        temp_work_dir: &Path,
        model: &str,
        thinking_level: Option<&str>,
    ) -> std::io::Result<()> {
        let pi_agent_dir = temp_work_dir.join(".pi").join("agent");
        fs::create_dir_all(&pi_agent_dir)?;

        // Read environment configuration from Docker config
        let env_vars = self.docker_client.get_config().environment().clone();

        // Claude/Opus/Sonnet/Haiku models are routed through the llama-swap
        // Anthropic Messages passthrough; everything else keeps using the
        // OpenAI-compatible endpoint.
        let use_anthropic = Self::is_anthropic_model(model);

        let (base_url, api_key, api_type) = if use_anthropic {
            let base_url = env_vars
                .get("ANTHROPIC_BASE_URL")
                .map(|s| s.as_str())
                .unwrap_or("http://host.docker.internal:8000");
            let api_key = env_vars
                .get("ANTHROPIC_AUTH_TOKEN")
                .map(|s| s.as_str())
                .or_else(|| env_vars.get("ANTHROPIC_API_KEY").map(|s| s.as_str()))
                .unwrap_or("placeholder-key");
            (base_url, api_key, "anthropic-messages")
        } else {
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
            (base_url, api_key, "openai-completions")
        };

        // Build reasoning configuration: check registry first, fall back to mechanism detection
        let reasoning_config = if let Some(level_str) = thinking_level {
            if benchmark_types::reasoning::ThinkingLevel::from_str(level_str).is_some() {
                let config = benchmark_types::reasoning::ReasoningRegistry::get_for_model(model);
                Some(config)
            } else {
                None
            }
        } else {
            None
        };

        // Build the model JSON object using serde_json for proper escaping
        let mut model_map = serde_json::Map::new();
        model_map.insert("id".to_string(), serde_json::Value::String(model.to_string()));
        // All benchmark models have at least 256k context; set it explicitly
        // to prevent premature auto-compaction (pi defaults to 128k).
        model_map.insert("contextWindow".to_string(), serde_json::Value::Number(serde_json::Number::from(262_144)));

        if let Some(ref rc) = reasoning_config {
            // pi's hard gate for thinking support: without `reasoning: true`,
            // getSupportedThinkingLevels() returns only ["off"] and any level
            // (including the CLI `--thinking` flag) is clamped away, so no
            // reasoning parameter ever reaches the API.
            model_map.insert("reasoning".to_string(), serde_json::Value::Bool(true));

            // thinkingFormat is read from model.compat.thinkingFormat, not from
            // the model top level — a top-level key is silently ignored.
            let mut compat_map = serde_json::Map::new();
            if let Some(ref tf) = rc.thinking_format {
                compat_map.insert(
                    "thinkingFormat".to_string(),
                    serde_json::Value::String(tf.to_string()),
                );
            }
            if let Some(ref cc) = rc.compat {
                if let Some(sre) = cc.supports_reasoning_effort {
                    compat_map.insert(
                        "supportsReasoningEffort".to_string(),
                        serde_json::Value::Bool(sre),
                    );
                }
                if let Some(sdr) = cc.supports_developer_role {
                    compat_map.insert(
                        "supportsDeveloperRole".to_string(),
                        serde_json::Value::Bool(sdr),
                    );
                }
            }
            if !compat_map.is_empty() {
                model_map.insert(
                    "compat".to_string(),
                    serde_json::Value::Object(compat_map),
                );
            }

            if let Some(ref tlm) = rc.thinking_level_map {
                let mut level_map = serde_json::Map::new();
                let entries: &[(&str, &Option<serde_json::Value>)] = &[
                    ("off", &tlm.off),
                    ("minimal", &tlm.minimal),
                    ("low", &tlm.low),
                    ("medium", &tlm.medium),
                    ("high", &tlm.high),
                    ("xhigh", &tlm.xhigh),
                ];
                for (k, v) in entries {
                    if let Some(val) = v {
                        level_map.insert(k.to_string(), val.clone());
                    }
                }
                if !level_map.is_empty() {
                    model_map.insert(
                        "thinkingLevelMap".to_string(),
                        serde_json::Value::Object(level_map),
                    );
                }
            }
        }
        let model_obj = serde_json::Value::Object(model_map);

        // Build providers with serde_json for bulletproof escaping
        let mut provider = serde_json::Map::new();
        provider.insert("baseUrl".to_string(), serde_json::Value::String(base_url.to_string()));
        provider.insert("apiKey".to_string(), serde_json::Value::String(api_key.to_string()));
        provider.insert("api".to_string(), serde_json::Value::String(api_type.to_string()));
        provider.insert("models".to_string(), serde_json::json!([model_obj]));

        let mut providers = serde_json::Map::new();
        providers.insert(Self::provider_key_for_model(model).to_string(), serde_json::Value::Object(provider));

        let mut root = serde_json::Map::new();
        root.insert("providers".to_string(), serde_json::Value::Object(providers));

        let models_json = serde_json::to_string_pretty(&serde_json::Value::Object(root))
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;

        let models_file = pi_agent_dir.join("models.json");
        fs::write(&models_file, &models_json)?;
        debug!("Created models.json at: {:?} with {} provider (model={}, thinking_level={:?}, mechanism={:?})",
            models_file, Self::provider_key_for_model(model), model, thinking_level,
            reasoning_config.as_ref().map(|rc| &rc.mechanism));
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
                let cancellation = recover_poisoned(self.cancellation_token.lock()).clone();
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
                        cancellation,
                    )
                    .await;

                if let Ok(ref result) = export_result {
                    info!(
                        "Export completed. Success: {:?}, Exit code: {:?}",
                        result.completed, result.exit_code
                    );
                    if !result.output.is_empty() {
                        let preview = crate::safe_truncate(&result.output, 1000);
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
    /// Includes --thinking flag when thinking_level is specified.
    /// Extensions are loaded via --extension flags pointing to globally installed npm packages.
    fn build_pi_command(&self, prompt: &str, model: &str, thinking_level: Option<&str>) -> Vec<String> {
        // Pi extensions installed globally via npm at build time.
        // Package "pi" fields from package.json:
        //   pi-caveman: extensions: ["./extensions/caveman.ts"]
        //   @mrclrchtr/supi-bash-timeout: extensions: ["./src/extension.ts"]
        // Debian NodeSource installs to /usr/lib/node_modules (not /usr/local).
        // This must match the npm global root inside the Debian runner image.
        const NPM_GLOBAL: &str = "/usr/lib/node_modules";

        let mut command = vec![
            "pi".to_string(),
            "--mode".to_string(),
            "json".to_string(),
            "--tools".to_string(),
            "read,bash,edit,write,grep,find,ls".to_string(),
            "--provider".to_string(),
            Self::provider_key_for_model(model).to_string(),
            "--model".to_string(),
            model.to_string(),
            // bash-timeout extension — injects default timeouts on bash tool calls
            "--extension".to_string(),
            format!("{NPM_GLOBAL}/@mrclrchtr/supi-bash-timeout/src/extension.ts"),
            // caveman extension — token compression mode
            "--extension".to_string(),
            format!("{NPM_GLOBAL}/pi-caveman/extensions/caveman.ts"),
        ];

        // Add thinking level if specified
        if let Some(level) = thinking_level {
            command.push("--thinking".to_string());
            command.push(level.to_string());
        }

        command.push(prompt.to_string());
        command
    }
}

#[async_trait::async_trait]
impl Agent for PiAgent {
    fn set_cancellation_token(&self, token: Option<CancellationToken>) {
        *recover_poisoned(self.cancellation_token.lock()) = token;
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
        model: &str,
        thinking_level: Option<&str>,
        results_dir: &Path,
        timeout_override_secs: Option<u64>,
    ) -> Result<AgentResult, Box<dyn std::error::Error + Send + Sync>> {
        let start_time = Instant::now();
        let start_dt = chrono::Utc::now();
        info!(
            "Running Pi agent for exercise: {}",
            exercise.name
        );
        if let Some(t) = timeout_override_secs {
            info!("  Timeout override: {}s", t);
        }

        // Create temporary working directory and copy exercise files (shared logic)
        let temp_work_dir = super::exercise_files::create_temp_work_dir(exercise)?;
        super::exercise_files::copy_exercise_files(exercise, host_exercise_dir, &temp_work_dir)?;

        // Patch tests (remove @Disabled, #[ignore], xtest) — matches Java behavior
        crate::agent::test_patches::run_patch_tests(exercise, &temp_work_dir)?;

        // Create exercise prompt
        let prompt = Self::create_exercise_prompt(exercise, &temp_work_dir)?;

        // Use model from run_exercise parameter (matches Java behavior)
        let model = model.to_string();

        // Create models.json with the correct model and thinking level from queue
        self.create_models_json(&temp_work_dir, &model, thinking_level)?;

        // Build and run pi command
        let command = self.build_pi_command(&prompt, &model, thinking_level);
        // Prompt is the last element — pass separately so it's never in logs
        let prompt_arg = command.last().map(|s| s.as_str());
        let command_refs: Vec<&str> = command[..command.len().saturating_sub(1)].iter().map(|s| s.as_str()).collect();

        // Log environment and configuration for debugging
        let env_vars = self.docker_client.get_config().environment().clone();
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
        let cancellation = recover_poisoned(self.cancellation_token.lock()).clone();
        let result = self
            .docker_client
            .run_command_with_limits_and_volume_with_callback(
                None,
                Some(&container_work_dir),
                &command_refs,
                prompt_arg,
                timeout_override_secs,
                None,
                Some(&temp_work_dir.to_string_lossy()),
                Some(std::sync::Arc::new(move |line| {
                    let proc = recover_poisoned(processor.lock());
                    proc.process(line);
                })),
                true,  // enable .pi volume mount for session data (matches Java)
                cancellation,
            )
            .await?;

        // Capture end time and duration right after the agent finishes.
        // Test verification and trace collection happen afterwards and should
        // not count toward the agent's execution time.
        let end_dt = chrono::Utc::now();
        let duration_ms = start_time.elapsed().as_millis() as u64;
        let pi_success = result.completed && result.exit_code == 0;

        if !pi_success {
            let output_preview = if result.output.len() > 1000 {
                format!("{}...[truncated]", crate::safe_truncate(&result.output, 1000))
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
                "Pi agent completed: {}. Duration: {}ms",
                exercise.name, duration_ms
            );
        }

        // Collect trace from pi session files — use the agent-model subdirectory
        let model_str = model.to_string();
        let _trace = self
            .collect_pi_trace(&temp_work_dir, &results_dir, exercise, &model_str)
            .await
            .unwrap_or_default();

        // Run tests in Docker to verify the agent's solution.
        // This mirrors the Java flow where runReferenceSolution() calls
        // runTestsInDocker() after runAgent(). Without this, a pi that
        // crashes or can't connect still exits 0 and is incorrectly
        // marked as success.
        let test_agent = ReferenceAgent::new(self.docker_client.clone());
        let test_result = test_agent.run_tests_in_docker(exercise, &temp_work_dir).await;

        // Cleanup
        let _ = fs::remove_dir_all(&temp_work_dir);

        // The overall success is determined by whether tests pass.
        // If pi failed, we still report test failure (pi didn't produce a solution).
        let test_ok = match &test_result {
            Ok(t) => t.success,
            Err(_) => false,
        };
        let success = pi_success && test_ok;

        let error_message = if !success {
            if !pi_success {
                Some(format!("Pi agent failed with exit code: {}", result.exit_code))
            } else {
                test_result.as_ref().ok().and_then(|t| t.error_message.clone())
            }
        } else {
            None
        };

        Ok(AgentResult::builder()
            .exercise_name(exercise.name.clone())
            .language(exercise.language.clone())
            .success(success)
            .exit_code(test_result.as_ref().map(|t| t.exit_code).unwrap_or(result.exit_code))
            .output(String::new()) // Don't store raw output - trace is saved separately
            .duration_ms(duration_ms)
            .start_time(start_dt.to_rfc3339())
            .end_time(end_dt.to_rfc3339())
            .error_message(error_message)
            .container_id(test_result.as_ref().map(|t| t.container_id.clone()).unwrap_or(result.container_id))
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
        // are mounted at /workspace. Translate host paths to container paths
        // using the exercise_dir prefix.
        if let Some(test_path) = &exercise.test_path {
            if test_path.exists() {
                let container_path = if let Some(ref exercise_dir) = exercise.exercise_dir {
                    if let Ok(relative) = test_path.strip_prefix(exercise_dir) {
                        format!("/workspace/{}", relative.display())
                    } else {
                        test_path.to_string_lossy().to_string()
                    }
                } else {
                    test_path.to_string_lossy().to_string()
                };
                prompt.push_str(&format!("Test file location: {}\n", container_path));
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
        prompt.push_str("\ncaveman mode\n");

        Ok(prompt)
    }
}

#[cfg(test)]
mod provider_routing_tests {
    use super::PiAgent;

    #[test]
    fn detects_claude_family_models_case_insensitively() {
        assert!(PiAgent::is_anthropic_model("claude-sonnet-5"));
        assert!(PiAgent::is_anthropic_model("Claude-Opus-4-5"));
        assert!(PiAgent::is_anthropic_model("claude-3-5-haiku-20241022"));
        assert!(PiAgent::is_anthropic_model("SONNET"));
        assert!(PiAgent::is_anthropic_model("my-opus-mirror"));
        assert!(PiAgent::is_anthropic_model("haiku-mini"));
    }

    #[test]
    fn does_not_flag_non_anthropic_models() {
        assert!(!PiAgent::is_anthropic_model("gpt-4o"));
        assert!(!PiAgent::is_anthropic_model("qwen2.5-coder:7b"));
        assert!(!PiAgent::is_anthropic_model("llama3.1:8b"));
    }

    #[test]
    fn provider_key_matches_model_family() {
        assert_eq!(PiAgent::provider_key_for_model("claude-sonnet-5"), "anthropic");
        assert_eq!(PiAgent::provider_key_for_model("gpt-4o"), "openai");
    }
}

#[cfg(test)]
mod models_json_tests {
    use super::*;
    use crate::docker::{DockerClient, DockerConfig};
    use std::collections::HashMap;

    fn test_client() -> DockerClient {
        let mut env = HashMap::new();
        env.insert("ANTHROPIC_BASE_URL".to_string(), "http://host.docker.internal:8080".to_string());
        env.insert("OPENAI_BASE_URL".to_string(), "http://host.docker.internal:8080/v1".to_string());
        env.insert("OPENAI_API_KEY".to_string(), "api-key".to_string());
        DockerClient::new(DockerConfig {
            image: "llm-benchmark/runner:latest".to_string(),
            memory: "2g".to_string(),
            timeout: 3600,
            work_dir: "/workspace".to_string(),
            environment: env,
            per_command_timeout: 120,
        })
    }

    fn temp_work_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "pi-models-json-test-{}-{}",
            tag,
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn read_models_json(dir: &Path) -> serde_json::Value {
        let path = dir.join(".pi").join("agent").join("models.json");
        let content = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("failed to read {:?}: {}", path, e));
        serde_json::from_str(&content).unwrap()
    }

    fn model_obj(models_json: &serde_json::Value) -> serde_json::Value {
        models_json["providers"]["openai"]["models"][0].clone()
    }

    #[test]
    fn ds4_flash_with_xhigh_emits_reasoning_config() {
        benchmark_types::reasoning::ReasoningRegistry::register_defaults();
        let agent = PiAgent::new(test_client());
        let dir = temp_work_dir("ds4-xhigh");

        agent.create_models_json(&dir, "ds4-flash", Some("xhigh")).unwrap();
        let model = model_obj(&read_models_json(&dir));
        let _ = std::fs::remove_dir_all(&dir);

        // pi's hard gate: without `reasoning: true` it clamps any level to "off"
        assert_eq!(model["reasoning"], serde_json::json!(true));
        // thinkingFormat must live under compat — pi reads model.compat.thinkingFormat
        assert_eq!(model["compat"]["thinkingFormat"], serde_json::json!("openai"));
        assert!(model.get("thinkingFormat").is_none(), "top-level thinkingFormat is ignored by pi");
        // ds4-specific level map at model level
        assert_eq!(model["thinkingLevelMap"]["off"], serde_json::json!("none"));
        assert_eq!(model["thinkingLevelMap"]["xhigh"], serde_json::json!("max"));
    }

    #[test]
    fn ds4_flash_with_off_maps_to_none() {
        benchmark_types::reasoning::ReasoningRegistry::register_defaults();
        let agent = PiAgent::new(test_client());
        let dir = temp_work_dir("ds4-off");

        agent.create_models_json(&dir, "ds4-flash", Some("off")).unwrap();
        let model = model_obj(&read_models_json(&dir));
        let _ = std::fs::remove_dir_all(&dir);

        // reasoning must stay enabled so pi explicitly sends reasoning_effort "none"
        // (ds4-server defaults to thinking ON — absence of the param means HIGH)
        assert_eq!(model["reasoning"], serde_json::json!(true));
        assert_eq!(model["thinkingLevelMap"]["off"], serde_json::json!("none"));
    }

    #[test]
    fn no_thinking_level_preserves_plain_model() {
        let agent = PiAgent::new(test_client());
        let dir = temp_work_dir("no-level");

        agent.create_models_json(&dir, "ds4-flash", None).unwrap();
        let model = model_obj(&read_models_json(&dir));
        let _ = std::fs::remove_dir_all(&dir);

        assert!(model.get("reasoning").is_none());
        assert!(model.get("compat").is_none());
    }
}
