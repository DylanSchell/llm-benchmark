use std::fs;
use std::path::Path;
use std::sync::{Arc, Mutex};
use tokio::time::Instant;
use tracing::{error, info, warn};
use benchmark_types::agent::{Agent, AgentResult};
use benchmark_types::exercise::Exercise;
use crate::docker::DockerClient;
use crate::agent::{reference::ReferenceAgent, ClaudeMessageProcessor};
use walkdir::WalkDir;

/// Claude agent that invokes Claude Code CLI to solve exercises.
pub struct ClaudeAgent {
    docker_client: DockerClient,
    message_processor: Arc<Mutex<ClaudeMessageProcessor>>,
}

impl ClaudeAgent {
    pub fn new(docker_client: DockerClient) -> Self {
        Self {
            docker_client,
            message_processor: Arc::new(Mutex::new(ClaudeMessageProcessor::new(None))),
        }
    }

    /// Set the message processor with an output consumer for web UI streaming.
    pub fn set_message_processor(&mut self, processor: ClaudeMessageProcessor) {
        self.message_processor = Arc::new(Mutex::new(processor));
    }

    /// Creates a temporary working directory for the exercise.
    /// Create exercise prompt for Claude Code.
    fn create_exercise_prompt(exercise: &Exercise, temp_dir: &Path) -> Result<String, std::io::Error> {
        let mut prompt = String::new();

        let instructions_path = temp_dir.join(".docs/instructions.md");
        if instructions_path.exists() {
            prompt = fs::read_to_string(instructions_path)?;
        } else {
            prompt.push_str("Please solve the following programming exercise.\n\n");
            prompt.push_str(&format!("Exercise: {}\n", exercise.name));
            prompt.push_str(&format!("Language: {}\n\n", exercise.language));
            prompt.push_str("Instructions:\n");
            prompt.push_str(
                "1. Implement the solution in the source files only, do not touch the test files.\n",
            );
            prompt.push_str("2. Run the tests to verify your solution\n\n");
            prompt.push_str(
                "3. When writing code, just write the tool_call, do not show me the code before you write it!\n",
            );
            prompt.push_str(
                "4. The tests are validated to be correct, never assume the test to be wrong!\n\n",
            );
            prompt.push_str(
                "5. Do not run tests in the background, run them synchronously in the foreground\n",
            );
        }

        if let Some(ref test_path) = exercise.test_path {
            if test_path.exists() {
                // Translate host path to container path using exercise_dir prefix.
                // The container mounts the exercise dir at /workspace.
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

        if exercise.language == "java" {
            prompt.push_str(
                "\nDo not stop working until you have executed the test suite (./gradlew test --no-daemon) and you have validated that the tests succeed!\n",
            );
        }

        // Append agent execution instructions (from prompt.md resource)
        let prompt_instructions = include_str!("../../../../benchmark-web/resources/prompt.md");
        prompt.push_str(prompt_instructions);

        Ok(prompt)
    }

    /// Collect Claude execution trace from HTML files.
    fn collect_claude_trace(temp_dir: &Path) -> Result<Option<String>, std::io::Error> {
        let claude_archive = temp_dir.join("claude-archive/workspace");

        if !claude_archive.exists() {
            return Ok(None);
        }

        let mut html_traces = Vec::new();

        for entry in WalkDir::new(&claude_archive) {
            match entry {
                Ok(entry) => {
                    let path = entry.path();
                    if path.extension().map(|e| e == "html").unwrap_or(false)
                        && path.to_string_lossy().contains("page")
                    {
                        if let Ok(content) = fs::read_to_string(path) {
                            html_traces.push(content);
                        }
                    }
                }
                Err(e) => warn!("Error reading trace file: {}", e),
            }
        }

        Ok(html_traces.first().cloned())
    }

}

#[async_trait::async_trait]
impl Agent for ClaudeAgent {
    #[tracing::instrument(skip(self), fields(exercise = %exercise.name, language = %exercise.language))]
    async fn run_exercise(
        &self,
        exercise: &Exercise,
        host_exercise_dir: &Path,
        _model: &str,
        _thinking_level: Option<&str>,
        _results_dir: &Path,
    ) -> Result<AgentResult, Box<dyn std::error::Error + Send + Sync>> {
        info!("Starting exercise: {} with Claude agent", exercise.name);

        let temp_work_dir = super::exercise_files::create_temp_work_dir(exercise)?;

        super::exercise_files::copy_exercise_files(exercise, host_exercise_dir, &temp_work_dir)?;

        let prompt = Self::create_exercise_prompt(exercise, &temp_work_dir)?;

        let result = self.run_claude_in_docker(exercise, &temp_work_dir, &prompt).await?;

        // Use the agent's own timing (captured before test verification),
        // not wall-clock time that includes post-agent work.
        Ok(AgentResult::builder()
            .exercise_name(exercise.name.clone())
            .language(exercise.language.clone())
            .success(result.success)
            .exit_code(result.exit_code)
            .output(result.output)
            .duration_ms(result.duration_ms)
            .start_time(result.start_time)
            .end_time(result.end_time)
            .error_message(result.error_message)

            .container_id(result.container_id)
            .build())
    }

    fn get_name(&self) -> &str {
        "claude"
    }
}

impl ClaudeAgent {
    /// Run Claude Code inside Docker.
    async fn run_claude_in_docker(
        &self,
        exercise: &Exercise,
        temp_work_dir: &Path,
        prompt: &str,
    ) -> Result<AgentResult, Box<dyn std::error::Error + Send + Sync>> {
        let _start_time = Instant::now();
        let start_dt = chrono::Utc::now();

        let command = vec![
            "claude",
            "--allow-dangerously-skip-permissions",
            "--dangerously-skip-permissions",
            "--print",
            "--tools",
            "Task,TaskOutput,Bash,Glob,Grep,Read,Edit,Write,NotebookEdit,WebFetch,TodoWrite,WebSearch,KillShell,ExitPlanMode",
            "--permission-mode", "bypassPermissions",
            "--verbose",
            "--output-format", "stream-json",
            "--include-partial-messages",
        ];

        let processor = Arc::clone(&self.message_processor);
        let result = self
            .docker_client
            .run_command_with_limits_and_volume_with_callback(
                None,
                Some("/workspace"),
                &command,
                Some(prompt),
                None,
                None,
                Some(&temp_work_dir.to_string_lossy()),
                Some(std::sync::Arc::new(move |line| {
                    let proc = processor.lock().unwrap();
                    proc.process(line);
                })),
                false, // no .pi volume mount for Claude agent
            )
            .await?;

        let end_time = Instant::now();
        let duration_ms = end_time.elapsed().as_millis() as u64;
        let end_dt = chrono::Utc::now();

        // Extract all fields before using result
        let completed = result.completed;
        let output = result.output.clone();
        let exit_code = result.exit_code;
        let container_id = result.container_id;
        let claude_success = completed && exit_code == 0;

        if !claude_success {
            error!(
                "Claude agent exercise FAILED: {}. Exit code: {}, Output: {}",
                exercise.name, exit_code, output
            );
        } else {
            info!(
                "Claude agent completed: {}. Duration: {}ms",
                exercise.name, duration_ms
            );
        }

        // Collect trace files
        let _trace = Self::collect_claude_trace(temp_work_dir)?;

        // Run tests in Docker to verify the agent's solution.
        // This mirrors the Java flow where runReferenceSolution() calls
        // runTestsInDocker() after runAgent().
        let test_agent = ReferenceAgent::new(self.docker_client.clone());
        let test_result = test_agent.run_tests_in_docker(exercise, &temp_work_dir).await;

        // Cleanup temporary work directory (prevents disk accumulation over many runs)
        let _ = fs::remove_dir_all(&temp_work_dir);

        // The overall success is determined by whether tests pass.
        let test_ok = match &test_result {
            Ok(t) => t.success,
            Err(_) => false,
        };
        let success = claude_success && test_ok;

        let error_message = if !success {
            if !claude_success {
                Some(format!("Claude agent failed with exit code: {}", exit_code))
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
            .exit_code(test_result.as_ref().map(|t| t.exit_code).unwrap_or(exit_code))
            .output(output)
            .duration_ms(duration_ms)
            .start_time(start_dt.to_rfc3339())
            .end_time(end_dt.to_rfc3339())
            .error_message(error_message)
            .container_id(test_result.as_ref().map(|t| t.container_id.clone()).unwrap_or(container_id))
            .build())
    }
}
