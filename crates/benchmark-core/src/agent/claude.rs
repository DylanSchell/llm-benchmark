use std::fs;
use std::path::Path;
use std::sync::{Arc, Mutex};
use tokio::time::Instant;
use tracing::{error, info};
use benchmark_types::agent::{Agent, AgentResult};
use benchmark_types::cancellation::CancellationToken;
use benchmark_types::exercise::Exercise;
use benchmark_types::ExerciseSource;
use crate::docker::DockerClient;
use crate::agent::{reference::ReferenceAgent, ClaudeMessageProcessor};
use benchmark_types::util::recover_poisoned;


/// Claude agent that invokes Claude Code CLI to solve exercises.
pub struct ClaudeAgent {
    docker_client: DockerClient,
    message_processor: Arc<Mutex<ClaudeMessageProcessor>>,
    /// Session cancellation signal — aborts in-flight Docker runs when fired.
    cancellation_token: Mutex<Option<CancellationToken>>,
}

impl ClaudeAgent {
    pub fn new(docker_client: DockerClient) -> Self {
        Self {
            docker_client,
            message_processor: Arc::new(Mutex::new(ClaudeMessageProcessor::new(None))),
            cancellation_token: Mutex::new(None),
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

        // The materialized exercise is mounted at /workspace in the container.
        if let Some(ref test_file) = exercise.test_file {
            prompt.push_str(&format!("Test file location: /workspace/{}\n", test_file));
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
}

#[async_trait::async_trait]
impl Agent for ClaudeAgent {
    fn set_cancellation_token(&self, token: Option<CancellationToken>) {
        *recover_poisoned(self.cancellation_token.lock()) = token;
    }

    async fn run_exercise(
        &self,
        exercise: &Exercise,
        source: &dyn ExerciseSource,
        model: &str,
        thinking_level: Option<&str>,
        results_dir: &Path,
    ) -> Result<AgentResult, Box<dyn std::error::Error + Send + Sync>> {
        self.run_exercise_with_timeout(exercise, source, model, thinking_level, results_dir, None).await
    }

    #[tracing::instrument(skip(self, source), fields(exercise = %exercise.name, language = %exercise.language))]
    async fn run_exercise_with_timeout(
        &self,
        exercise: &Exercise,
        source: &dyn ExerciseSource,
        _model: &str,
        _thinking_level: Option<&str>,
        _results_dir: &Path,
        timeout_override_secs: Option<u64>,
    ) -> Result<AgentResult, Box<dyn std::error::Error + Send + Sync>> {
        info!("Starting exercise: {} with Claude agent", exercise.name);
        if let Some(t) = timeout_override_secs {
            info!("  Timeout override: {}s", t);
        }

        let temp_work_dir = super::exercise_files::create_temp_work_dir(exercise)?;

        super::exercise_files::materialize_exercise(source, exercise, &temp_work_dir)?;

        let prompt = Self::create_exercise_prompt(exercise, &temp_work_dir)?;

        let result = self.run_claude_in_docker(exercise, &temp_work_dir, &prompt, timeout_override_secs).await?;

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

    async fn run_aoc(
        &self,
        day: &benchmark_types::aoc::AocDay,
        work_dir: &Path,
        validator_url: &str,
        _model: &str,
        _thinking_level: Option<&str>,
        _results_dir: &Path,
    ) -> Result<AgentResult, Box<dyn std::error::Error + Send + Sync>> {
        info!("Running Claude agent for AoC day {}", day.day);
        let start_tokio = tokio::time::Instant::now();

        let prompt = crate::aoc_runner::AocRunner::new(day.user.clone(), _results_dir)
            .build_prompt(day, validator_url);

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
        let cancellation = recover_poisoned(self.cancellation_token.lock()).clone();

        let mut extra_env = std::collections::HashMap::new();
        extra_env.insert("AOC_VALIDATOR_URL".to_string(), validator_url.to_string());

        let result = self
            .docker_client
            .run_command_with_limits_and_volume_with_callback_and_env(
                None,
                Some("/workspace"),
                &command,
                Some(&prompt),
                None,
                None,
                Some(&work_dir.to_string_lossy()),
                Some(std::sync::Arc::new(move |line| {
                    let proc = recover_poisoned(processor.lock());
                    proc.process(line);
                })),
                false, // no .pi volume mount for Claude agent
                cancellation,
                Some(&extra_env),
            )
            .await?;

        let end_dt = chrono::Utc::now();
        let duration_ms = start_tokio.elapsed().as_millis() as u64;
        let claude_success = result.completed && result.exit_code == 0;

        if !claude_success {
            error!(
                "Claude agent AoC day {} FAILED: exit={}, completed={}",
                day.day, result.exit_code, result.completed
            );
        } else {
            info!("Claude agent AoC day {} completed in {}ms", day.day, duration_ms);
        }

        let error_message = if claude_success {
            None
        } else {
            Some(format!("Claude agent failed with exit code: {}", result.exit_code))
        };

        Ok(AgentResult::builder()
            .category(benchmark_types::Category::Aoc2015)
            .exercise_name(day.exercise_name())
            .language(day.language().to_string())
            .success(claude_success)
            .exit_code(result.exit_code)
            .output(String::new())
            .duration_ms(duration_ms)
            .start_time(chrono::Utc::now().to_rfc3339())
            .end_time(end_dt.to_rfc3339())
            .error_message(error_message)
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
        timeout_override_secs: Option<u64>,
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
        let cancellation = recover_poisoned(self.cancellation_token.lock()).clone();
        let result = self
            .docker_client
            .run_command_with_limits_and_volume_with_callback(
                None,
                Some("/workspace"),
                &command,
                Some(prompt),
                timeout_override_secs,
                None,
                Some(&temp_work_dir.to_string_lossy()),
                Some(std::sync::Arc::new(move |line| {
                    let proc = recover_poisoned(processor.lock());
                    proc.process(line);
                })),
                false, // no .pi volume mount for Claude agent
                cancellation,
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
