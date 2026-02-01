use crate::docker::DockerClient;
use crate::model::Exercise;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};
use tracing::{debug, error, info, warn};
use walkdir::WalkDir;

#[async_trait::async_trait]
pub trait Agent {
    async fn run_exercise(
        &self,
        exercise: &Exercise,
        host_exercise_dir: &Path,
    ) -> Result<AgentResult, Box<dyn std::error::Error + Send + Sync>>;
}

/// Agent result from running an exercise
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentResult {
    pub exercise_name: String,
    pub language: String,
    pub success: bool,
    pub exit_code: i32,
    pub output: String,
    pub duration_ms: u64,
    pub start_time: String,
    pub end_time: String,
    pub error_message: Option<String>,
    pub trace: Option<String>,
    pub container_id: String,
}

impl AgentResult {
    pub fn builder() -> AgentResultBuilder {
        AgentResultBuilder::new()
    }
}

#[derive(Default)]
pub struct AgentResultBuilder {
    exercise_name: Option<String>,
    language: Option<String>,
    success: bool,
    exit_code: i32,
    output: String,
    duration_ms: u64,
    start_time: Option<String>,
    end_time: Option<String>,
    error_message: Option<String>,
    trace: Option<String>,
    container_id: Option<String>,
}

impl AgentResultBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn exercise_name(mut self, exercise_name: String) -> Self {
        self.exercise_name = Some(exercise_name);
        self
    }

    pub fn language(mut self, language: String) -> Self {
        self.language = Some(language);
        self
    }

    pub fn success(mut self, success: bool) -> Self {
        self.success = success;
        self
    }

    pub fn exit_code(mut self, exit_code: i32) -> Self {
        self.exit_code = exit_code;
        self
    }

    pub fn output(mut self, output: String) -> Self {
        self.output = output;
        self
    }

    pub fn duration_ms(mut self, duration_ms: u64) -> Self {
        self.duration_ms = duration_ms;
        self
    }

    pub fn start_time(mut self, start_time: String) -> Self {
        self.start_time = Some(start_time);
        self
    }

    pub fn end_time(mut self, end_time: String) -> Self {
        self.end_time = Some(end_time);
        self
    }

    pub fn error_message(mut self, error_message: Option<String>) -> Self {
        self.error_message = error_message;
        self
    }

    pub fn trace(mut self, trace: String) -> Self {
        self.trace = Some(trace);
        self
    }

    pub fn container_id(mut self, container_id: String) -> Self {
        self.container_id = Some(container_id);
        self
    }

    pub fn build(self) -> AgentResult {
        let now = chrono::Utc::now();
        AgentResult {
            exercise_name: self.exercise_name.unwrap_or_default(),
            language: self.language.unwrap_or_default(),
            success: self.success,
            exit_code: self.exit_code,
            output: self.output,
            duration_ms: self.duration_ms,
            start_time: self.start_time.unwrap_or_else(|| now.to_rfc3339()),
            end_time: self.end_time.unwrap_or_else(|| now.to_rfc3339()),
            error_message: self.error_message,
            trace: self.trace,
            container_id: self.container_id.unwrap_or_default(),
        }
    }
}

/// Reference agent that copies reference implementation and runs tests
pub struct ReferenceAgent {
    docker_client: DockerClient,
}

impl ReferenceAgent {
    pub fn new(docker_client: DockerClient) -> Self {
        Self { docker_client }
    }
}

#[async_trait::async_trait]
impl Agent for ReferenceAgent {
    async fn run_exercise(
        &self,
        exercise: &Exercise,
        host_exercise_dir: &Path,
    ) -> Result<AgentResult, Box<dyn std::error::Error + Send + Sync>> {
        let start_time = std::time::Instant::now();
        let start_dt = chrono::Utc::now();
        info!("Running reference agent for exercise: {}", exercise.name);

        let temp_work_dir = create_temp_work_dir(exercise)?;
        info!("Created temporary work directory: {:?}", temp_work_dir);

        copy_exercise_files(exercise, host_exercise_dir, &temp_work_dir)?;

        let agent_result = run_reference_agent(exercise, &temp_work_dir)?;

        let test_result = run_tests_in_docker(&self.docker_client, exercise, &temp_work_dir)?;

        cleanup_temp_dir(&temp_work_dir);

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
            .trace(agent_result.trace.unwrap_or_default())
            .container_id(test_result.container_id)
            .build())
    }
}

/// Claude agent that invokes Claude Code CLI to solve exercises
pub struct ClaudeAgent {
    docker_client: DockerClient,
}

impl ClaudeAgent {
    pub fn new(docker_client: DockerClient) -> Self {
        Self { docker_client }
    }
}

#[async_trait::async_trait]
impl Agent for ClaudeAgent {
    async fn run_exercise(
        &self,
        exercise: &Exercise,
        host_exercise_dir: &Path,
    ) -> Result<AgentResult, Box<dyn std::error::Error + Send + Sync>> {
        let start_time = std::time::Instant::now();
        let start_dt = chrono::Utc::now();
        info!("Starting exercise: {} with Claude agent", exercise.name);

        let temp_work_dir = create_temp_work_dir(exercise)?;

        copy_exercise_files(exercise, host_exercise_dir, &temp_work_dir)?;

        let prompt = create_exercise_prompt(exercise, &temp_work_dir)?;

        let result = run_claude_in_docker(&self.docker_client, exercise, &temp_work_dir, &prompt)?;

        let end_dt = chrono::Utc::now();
        let duration_ms = start_time.elapsed().as_millis() as u64;

        Ok(AgentResult::builder()
            .exercise_name(exercise.name.clone())
            .language(exercise.language.clone())
            .success(result.success)
            .exit_code(result.exit_code)
            .output(result.output)
            .duration_ms(duration_ms)
            .start_time(start_dt.to_rfc3339())
            .end_time(end_dt.to_rfc3339())
            .error_message(result.error_message)
            .trace(result.trace.unwrap_or_default())
            .container_id(result.container_id)
            .build())
    }
}

/// Create temporary working directory for exercise
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

/// Copy exercise files to temp directory, excluding reference implementation
fn copy_exercise_files(
    _exercise: &Exercise,
    source_dir: &Path,
    dest_dir: &Path,
) -> Result<(), std::io::Error> {
    info!(
        "Copying exercise files from {:?} to {:?}",
        source_dir, dest_dir
    );

    let walker = walkdir::WalkDir::new(source_dir).into_iter();
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

/// Run the reference agent (copy reference implementation)
fn run_reference_agent(
    exercise: &Exercise,
    temp_dir: &Path,
) -> Result<AgentResult, Box<dyn std::error::Error + Send + Sync>> {
    let start_time = std::time::Instant::now();

    if let Some(ref_ref) = &exercise.reference_path {
        copy_reference_impl(exercise, temp_dir, ref_ref);
    } else {
        warn!("No reference implementation found for: {}", exercise.name);
    }

    let end_time = std::time::Instant::now();
    let duration_ms = (end_time - start_time).as_millis() as u64;

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

/// Copy reference implementation files
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
                        if ref_file.extension().map(|e| e == "java") == Some(true) {
                            let file_name = ref_file.file_name().unwrap().to_string_lossy();
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
                        if ref_file.extension().map(|e| e == "go") == Some(true) {
                            let file_name = ref_file.file_name().unwrap().to_string_lossy();
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
                        if ref_file.extension().map(|e| e == "rs") == Some(true) {
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
                        if ref_file.extension().map(|e| e == "js" || e == "ts") == Some(true) {
                            let file_name = ref_file.file_name().unwrap().to_string_lossy();
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
                        if ref_file.extension().map(|e| e == "py") == Some(true) {
                            let file_name = ref_file.file_name().unwrap().to_string_lossy();
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

/// Run tests inside Docker container
fn run_tests_in_docker(
    docker_client: &DockerClient,
    exercise: &Exercise,
    temp_work_dir: &Path,
) -> Result<AgentResult, Box<dyn std::error::Error + Send + Sync>> {
    let start_time = std::time::Instant::now();
    let command = get_test_command(exercise, temp_work_dir);

    info!(
        "Running tests in Docker container at /workspace (mounted from: {:?})",
        temp_work_dir
    );
    debug!("Command: {}", command.join(" "));

    let result = docker_client.run_command_with_limits_and_volume(
        None,
        Some("/workspace"),
        &command,
        None,
        None,
        Some(&temp_work_dir.to_string_lossy()),
    )?;

    let end_time = std::time::Instant::now();
    let duration_ms = (end_time - start_time).as_millis() as u64;
    let end_dt = chrono::Utc::now();

    // Extract all fields from result before using it
    let output = result.output.clone();
    let exit_code = result.exit_code;
    let container_id = result.container_id;
    let completed = result.completed;
    let success = completed && exit_code == 0 && !contains_test_failures(&output);

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

    let output_for_error = if success { None } else { Some(output.clone()) };

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

/// Get test command based on build system
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
    } else if polyglot_path.join("pyproject.toml").exists() || polyglot_path.join("setup.py").exists() {
        vec!["python", "-m", "pytest"]
    } else if polyglot_path.join("Gemfile").exists() {
        vec!["bundle", "exec", "rake", "test"]
    } else if has_extension(polyglot_path, "csproj") || has_extension(polyglot_path, "sln") {
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

/// Check if test output contains failure patterns
fn contains_test_failures(output: &str) -> bool {
    if output.is_empty() {
        return false;
    }

    let lower_output = output.to_lowercase();
    let patterns = [
        "FAILED",
        "FAILURE",
        "BUILD FAILED",
        "BUILD FAILURE",
        "Tests FAILED",
        "Test FAILED",
        "Error:",
        "Exception",
        "failed",
        "FAIL",
    ];

    patterns.iter().any(|p| lower_output.contains(*p))
}

/// Run Claude Code in Docker
fn run_claude_in_docker(
    docker_client: &DockerClient,
    exercise: &Exercise,
    temp_work_dir: &Path,
    prompt: &str,
) -> Result<AgentResult, Box<dyn std::error::Error + Send + Sync>> {
    let start_time = std::time::Instant::now();
    let start_dt = chrono::Utc::now();

    let command = vec![
        "claude",
        "--allow-dangerously-skip-permissions",
        "--dangerously-skip-permissions",
        "--print",
        "--tools", "Task,TaskOutput,Bash,Glob,Grep,Read,Edit,Write,NotebookEdit,WebFetch,TodoWrite,WebSearch,KillShell,ExitPlanMode",
        "--permission-mode", "bypassPermissions",
        "--verbose",
        "--output-format", "stream-json",
        "--include-partial-messages",
        prompt,
    ];

    let result = docker_client.run_command_with_limits_and_volume_with_callback(
        None,
        Some("/workspace"),
        &command,
        None,
        None,
        Some(&temp_work_dir.to_string_lossy()),
        Some(std::sync::Arc::new(|line| process_message(line))),
    )?;

    let end_time = std::time::Instant::now();
    let duration_ms = (end_time - start_time).as_millis() as u64;
    let end_dt = chrono::Utc::now();

    // Extract all fields before using result
    let completed = result.completed;
    let output = result.output.clone();
    let exit_code = result.exit_code;
    let container_id = result.container_id;
    let success = completed && exit_code == 0;

    if !success {
        error!(
            "Exercise failed: {}. Exit code: {}, Output: {}",
            exercise.name, exit_code, output
        );
    } else {
        info!(
            "Exercise completed successfully: {}. Duration: {}ms",
            exercise.name, duration_ms
        );
    }

    // Collect trace files
    let trace = collect_claude_trace(temp_work_dir)?;

    let output_clone = output.clone();

    Ok(AgentResult::builder()
        .exercise_name(exercise.name.clone())
        .language(exercise.language.clone())
        .success(success)
        .exit_code(exit_code)
        .output(output)
        .duration_ms(duration_ms)
        .start_time(start_dt.to_rfc3339())
        .end_time(end_dt.to_rfc3339())
        .error_message(if success { None } else { Some(output_clone) })
        .trace(trace.unwrap_or_default())
        .container_id(container_id)
        .build())
}

/// Create exercise prompt for Claude Code
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
        prompt.push_str("3. When writing code, just write the tool_call, do not show me the code before you write it!\n");
        prompt.push_str(
            "4. The tests are validated to be correct, never assume the test to be wrong!\n\n",
        );
        prompt.push_str(
            "5. Do not run tests in the background, run them synchronously in the foreground\n",
        );
    }

    if let Some(ref test_path) = exercise.test_path {
        if test_path.exists() {
            let needle = format!(
                "../polyglot-benchmark/{}/exercises/practice/{}",
                exercise.language, exercise.name
            );
            let fixed_path = test_path.to_string_lossy().replace(&needle, "/workspace");
            prompt.push_str(&format!("Test file location: {}\n", fixed_path));
        }
    }

    prompt.push_str("\nImplement the solution directly, do not ask me to review.\n");

    if exercise.language == "java" {
        prompt.push_str("\nDo not stop working until you have executed the test suite (./gradlew test --no-daemon) and you have validated that the tests succeed!\n");
    }

    Ok(prompt)
}

/// Collect Claude execution trace from HTML files
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
                if path.extension().map(|e| e == "html") == Some(true)
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

/// Cleanup temporary directory
fn cleanup_temp_dir(temp_dir: &Path) {
    if temp_dir.exists() {
        for entry in WalkDir::new(temp_dir) {
            match entry {
                Ok(entry) => {
                    let path = entry.path();
                    if path.is_file() {
                        let _ = fs::remove_file(path);
                    }
                }
                Err(e) => warn!("Error removing file: {}", e),
            }
        }
        let _ = fs::remove_dir(temp_dir);
    }
}

fn process_message(line: &str) {
    if let Ok(json) = serde_json::from_str::<Value>(line) {
        if let Some(msg_type) = json.get("type").and_then(|v| v.as_str()) {
            match msg_type {
                "stream_event" => process_stream_event(&json),
                "assistant" => process_assistant_message(&json),
                "user" => process_user_message(&json),
                "result" => {
                    println!("Result: {}", json);
                }
                _ => {
                    // println!("{}", line);
                }
            }
        }
    } else {
        println!("{}", line);
    }
}

fn process_stream_event(json: &Value) {
    if let Some(event) = json.get("event").and_then(|v| v.as_object()) {
        if let Some(event_type) = event.get("type").and_then(|v| v.as_str()) {
            match event_type {
                "message_start" => {
                    if let Some(message) = event.get("message").and_then(|v| v.as_object()) {
                        if let Some(content) = message.get("content") {
                            process_content(content);
                        }
                    }
                }
                "message_delta" => {
                    if let Some(delta) = event.get("delta").and_then(|v| v.as_object()) {
                        if let Some(delta_type) = delta.get("type").and_then(|v| v.as_str()) {
                            match delta_type {
                                "thinking_delta" => {
                                    if let Some(thinking) = delta.get("thinking").and_then(|v| v.as_str()) {
                                        print!("{}", thinking);
                                    }
                                }
                                "input_json_delta" => {
                                    if let Some(partial) = delta.get("partial_json").and_then(|v| v.as_str()) {
                                        print!("{}", partial);
                                    }
                                }
                                "text_delta" => {
                                    if let Some(text) = delta.get("text").and_then(|v| v.as_str()) {
                                        print!("{}", text);
                                    }
                                }
                                _ => {}
                            }
                        }
                    }
                }
                "content_block_delta" => {
                    if let Some(delta) = event.get("delta").and_then(|v| v.as_object()) {
                        if let Some(delta_type) = delta.get("type").and_then(|v| v.as_str()) {
                            match delta_type {
                                "thinking_delta" => {
                                    if let Some(thinking) = delta.get("thinking").and_then(|v| v.as_str()) {
                                        print!("{}", thinking);
                                    }
                                }
                                "input_json_delta" => {
                                    if let Some(partial) = delta.get("partial_json").and_then(|v| v.as_str()) {
                                        print!("{}", partial);
                                    }
                                }
                                "text_delta" => {
                                    if let Some(text) = delta.get("text").and_then(|v| v.as_str()) {
                                        print!("{}", text);
                                    }
                                }
                                _ => {}
                            }
                        }
                    }
                }
                "message_stop" => {
                    println!();
                }
                _ => {}
            }
        }
    }
}

fn process_assistant_message(json: &Value) {
    if let Some(message) = json.get("message").and_then(|v| v.as_object()) {
        if let Some(content) = message.get("content") {
            process_content(content);
        }
    }
}

fn process_user_message(json: &Value) {
    if let Some(message) = json.get("message").and_then(|v| v.as_object()) {
        if let Some(content) = message.get("content") {
            if content.is_array() {
                if let Some(items) = content.as_array() {
                    for item in items {
                        if let Some(item_type) = item.get("type").and_then(|v| v.as_str()) {
                            match item_type {
                                "text" => {
                                    if let Some(text) = item.get("text").and_then(|v| v.as_str()) {
                                        println!("{}", text);
                                    }
                                }
                                "tool_result" => {
                                    if let Some(tool_content) = item.get("content").and_then(|v| v.as_str()) {
                                        let with_newlines = tool_content.replace("\\n", "\n");
                                        println!("tool_result:\n{}", with_newlines);
                                    } else {
                                        println!("tool_result: {}", item);
                                    }
                                }
                                _ => {}
                            }
                        }
                    }
                }
            } else if let Some(content_type) = content.get("type").and_then(|v| v.as_str()) {
                match content_type {
                    "text" => {
                        if let Some(text) = content.get("text").and_then(|v| v.as_str()) {
                            println!("{}", text);
                        }
                    }
                    "tool_result" => {
                        println!("tool_result: {}", content.get("content").unwrap_or(&Value::Null));
                    }
                    _ => {}
                }
            }
        }
    }
}

fn process_content(content: &Value) {
    if content.is_array() {
        if let Some(items) = content.as_array() {
            for item in items {
                process_content_item(item);
            }
        }
    } else if let Some(obj) = content.as_object() {
        if let Some(item_type) = obj.get("type").and_then(|v| v.as_str()) {
            match item_type {
                "thinking" => {
                    if let Some(thinking) = obj.get("thinking").and_then(|v| v.as_str()) {
                        print!("{}", thinking);
                    }
                }
                "tool_use" => {
                    render_tool_use(obj);
                }
                "text" => {
                    if let Some(text) = obj.get("text").and_then(|v| v.as_str()) {
                        print!("{}", text);
                    }
                }
                _ => {}
            }
        }
    }
}

fn process_content_item(item: &Value) {
    if let Some(item_type) = item.get("type").and_then(|v| v.as_str()) {
        match item_type {
            "thinking" => {
                if let Some(thinking) = item.get("thinking").and_then(|v| v.as_str()) {
                    print!("{}", thinking);
                }
            }
            "tool_use" => {
                render_tool_use(item.as_object().unwrap());
            }
            "text" => {
                if let Some(text) = item.get("text").and_then(|v| v.as_str()) {
                    print!("{}", text);
                }
            }
            _ => {}
        }
    }
}

fn render_tool_use(item: &serde_json::Map<String, Value>) {
    if let Some(name) = item.get("name").and_then(|v| v.as_str()) {
        match name {
            "Edit" => {
                if let Some(input) = item.get("input").and_then(|v| v.as_object()) {
                    let file_path = input.get("file_path").and_then(|v| v.as_str()).unwrap_or("");
                    let old_string = input.get("old_string").and_then(|v| v.as_str()).unwrap_or("");
                    let new_string = input.get("new_string").and_then(|v| v.as_str()).unwrap_or("");
                    println!("Edit {}", file_path);
                    println!("Old");
                    let old_normalized = old_string.replace("\\n", "\n");
                    println!("{}", old_normalized);
                    println!("New");
                    let new_normalized = new_string.replace("\\n", "\n");
                    println!("{}", new_normalized);
                }
            }
            "Glob" => {
                if let Some(input) = item.get("input").and_then(|v| v.as_object()) {
                    if let Some(pattern) = input.get("pattern").and_then(|v| v.as_str()) {
                        println!("\ntool_use: Glob {}", pattern);
                    }
                }
            }
            "Read" => {
                if let Some(input) = item.get("input").and_then(|v| v.as_object()) {
                    if let Some(file_path) = input.get("file_path").and_then(|v| v.as_str()) {
                        println!("\ntool_use: Read {}", file_path);
                    }
                }
            }
            "Write" => {
                if let Some(input) = item.get("input").and_then(|v| v.as_object()) {
                    let file_path = input.get("file_path").and_then(|v| v.as_str()).unwrap_or("");
                    let content = input.get("content").and_then(|v| v.as_str()).unwrap_or("");
                    println!("\ntool_use: Write {}", file_path);
                    println!("Content: ");
                    let normalized = content.replace("\\n", "\n");
                    println!("{}", normalized);
                }
            }
            "Bash" => {
                if let Some(input) = item.get("input").and_then(|v| v.as_object()) {
                    let run_in_background = input.get("run_in_background").and_then(|v| v.as_bool()).unwrap_or(false);
                    let description = input.get("description").and_then(|v| v.as_str()).unwrap_or("");
                    let command = input.get("command").and_then(|v| v.as_str()).unwrap_or("");
                    println!("\ntool_use: Bash {}{}", if run_in_background { "(in background) " } else { " " }, description);
                    println!("Command: {}", command);
                }
            }
            "TaskOutput" => {
                if let Some(input) = item.get("input") {
                    println!("\ntool_use: {} {}", name, input);
                }
            }
            "TodoWrite" => {
                println!("\ntool_use: TodoWrite");
                if let Some(input) = item.get("input").and_then(|v| v.as_object()) {
                    if let Some(todos) = input.get("todos").and_then(|v| v.as_array()) {
                        for todo in todos {
                            let content = todo.get("content").and_then(|v| v.as_str()).unwrap_or("");
                            let status = todo.get("status").and_then(|v| v.as_str()).unwrap_or("");
                            match status {
                                "in_progress" => println!("[⟳] {}", content),
                                "pending" => println!("[⌛] {}", content),
                                "completed" => println!("[✅] {}", content),
                                _ => println!("[ ] {}", content),
                            }
                        }
                    }
                }
            }
            _ => {
                if let Some(input) = item.get("input") {
                    println!("\ntool_use: {} {}", name, input);
                }
            }
        }
    }
}
