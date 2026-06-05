use super::watchdog::CommandWatchdog;
use anyhow::{anyhow, Context};
use std::collections::HashMap;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::time::{timeout, Duration};
use tracing::{debug, error, info, warn};

/// Runtime configuration for Docker execution.
/// Constructed from `benchmark_types::config::DockerConfig` via `From`.
#[derive(Debug, Clone)]
pub struct DockerConfig {
    pub image: String,
    pub memory: String,
    pub timeout: u64,
    pub work_dir: String,
    pub environment: HashMap<String, String>,
    pub per_command_timeout: u32,
}

impl From<&benchmark_types::config::DockerConfig> for DockerConfig {
    fn from(cfg: &benchmark_types::config::DockerConfig) -> Self {
        Self {
            image: cfg.image.clone(),
            memory: cfg.memory.clone(),
            timeout: cfg.timeout as u64,
            work_dir: cfg.work_dir.clone(),
            environment: cfg.environment_map(),
            per_command_timeout: cfg.per_command_timeout,
        }
    }
}

impl DockerConfig {
    /// Path to the Docker image.
    pub fn image(&self) -> &str {
        &self.image
    }

    /// Memory limit string (e.g., "2g").
    pub fn memory(&self) -> &str {
        &self.memory
    }

    /// Global timeout in seconds.
    pub fn timeout(&self) -> u64 {
        self.timeout
    }

    /// Working directory inside the container.
    pub fn work_dir(&self) -> &str {
        &self.work_dir
    }

    /// Environment variables passed to the container.
    pub fn environment(&self) -> &HashMap<String, String> {
        &self.environment
    }

    /// Per-command timeout in seconds.
    pub fn per_command_timeout(&self) -> u32 {
        self.per_command_timeout
    }

    /// Updates environment variables with the model name.
    /// Sets ANTHROPIC_MODEL and all ANTHROPIC_DEFAULT_*_MODEL variables.
    pub fn update_model_environment(&mut self, model_name: &str) {
        if let Some(v) = self.environment.get_mut("ANTHROPIC_MODEL") {
            *v = model_name.to_string();
        }
        if let Some(v) = self.environment.get_mut("ANTHROPIC_DEFAULT_HAIKU_MODEL") {
            *v = model_name.to_string();
        }
        if let Some(v) = self.environment.get_mut("ANTHROPIC_DEFAULT_OPUS_MODEL") {
            *v = model_name.to_string();
        }
        if let Some(v) = self.environment.get_mut("ANTHROPIC_DEFAULT_SONNET_MODEL") {
            *v = model_name.to_string();
        }
    }
}

/// Callback type for streaming output lines.
pub type OutputCallback = dyn Fn(&str) + Send + Sync + 'static;

/// Result of a Docker process execution.
#[derive(Debug, Clone)]
pub struct ProcessResult {
    pub exit_code: i32,
    pub output: String,
    pub completed: bool,
    pub container_id: String,
}

impl ProcessResult {
    pub fn is_success(&self) -> bool {
        self.completed && self.exit_code == 0
    }
}

/// Docker client that uses `tokio::process::Command` for async execution
/// with streaming output support.
#[derive(Clone)]
pub struct DockerClient {
    config: DockerConfig,
}

impl DockerClient {
    pub fn new(config: DockerConfig) -> Self {
        Self { config }
    }

    /// Updates the model environment variables in the Docker config.
    /// This allows dynamic model selection at runtime.
    pub fn set_model(&mut self, model_name: &str) {
        if !model_name.is_empty() {
            self.config.update_model_environment(model_name);
            tracing::debug!("Updated Docker environment to use model: {}", model_name);
        }
    }

    /// Returns the DockerConfig instance.
    /// Used by agents that need access to configuration (e.g., PiAgent for models.json).
    pub fn get_config(&self) -> &DockerConfig {
        &self.config
    }

    /// Check if Docker is available and running.
    pub async fn is_available(&self) -> bool {
        match Command::new("docker")
            .args(&["version", "--format", "{{.Server.Version}}"])
            .output()
            .await
        {
            Ok(output) => output.status.success(),
            Err(e) => {
                error!("Docker is not available: {}", e);
                false
            }
        }
    }

    /// Run a command in a Docker container with resource limits, volume mounts,
    /// and streaming output callback.
    ///
    /// This is the main entry point for executing commands inside Docker containers.
    /// It handles:
    /// - Building the docker run command with all flags
    /// - Setting up streaming output via tokio async I/O
    /// - Per-command timeout enforcement via CommandWatchdog
    /// - Container cleanup after execution
    pub async fn run_command_with_limits_and_volume(
        &self,
        container_image: Option<&str>,
        work_dir: Option<&str>,
        command: &[&str],
        timeout_seconds: Option<u64>,
        memory_limit: Option<&str>,
        volume_host_dir: Option<&str>,
    ) -> Result<ProcessResult, anyhow::Error> {
        self.run_command_with_limits_and_volume_with_callback(
            container_image,
            work_dir,
            command,
            None, // no prompt
            timeout_seconds,
            memory_limit,
            volume_host_dir,
            None,
            false, // no .pi volume mount for reference agent
        )
        .await
    }

    /// Run a command in a Docker container with resource limits, volume mounts,
    /// and a streaming output callback.
    ///
    /// `command` — the executable and its arguments (e.g. ["cargo", "test"])
    /// `prompt`   — optional prompt text appended as the last argument for agents.
    ///              Kept separate from command so logs never leak prompts.
    pub async fn run_command_with_limits_and_volume_with_callback(
        &self,
        container_image: Option<&str>,
        work_dir: Option<&str>,
        command: &[&str],
        prompt: Option<&str>,
        timeout_seconds: Option<u64>,
        memory_limit: Option<&str>,
        volume_host_dir: Option<&str>,
        output_callback: Option<std::sync::Arc<OutputCallback>>,
        enable_pi_volume: bool,
    ) -> Result<ProcessResult, anyhow::Error> {
        let image = container_image.unwrap_or(&self.config.image);
        let work = work_dir.unwrap_or(&self.config.work_dir);
        let timeout_secs = timeout_seconds.unwrap_or(self.config.timeout);
        let memory = memory_limit.unwrap_or(&self.config.memory);
        let host_dir = volume_host_dir
            .map(|s| s.to_string())
            .unwrap_or_else(|| {
                std::env::current_dir()
                    .map(|p| p.to_string_lossy().to_string())
                    .unwrap_or_else(|_| ".".to_string())
            });

        // Generate deterministic container name
        let container_id = format!(
            "bench-{}",
            uuid::Uuid::new_v4()
                .to_string()
                .replace('-', "")
                .chars()
                .take(12)
                .collect::<String>()
        );

        // Create .claude and .pi directories on host before mounting
        // (Java does this too — without it Docker creates them as root,
        //  so the runner user inside the container can't write)
        let claude_dir = std::path::Path::new(&host_dir).join(".claude");
        let _ = std::fs::create_dir_all(&claude_dir);
        if enable_pi_volume {
            let pi_dir = std::path::Path::new(&host_dir).join(".pi");
            let _ = std::fs::create_dir_all(&pi_dir);
        }

        // Build the command list: base command + optional prompt appended
        let mut exec_args = command.to_vec();
        if let Some(p) = prompt {
            exec_args.push(p);
        }

        // Build docker command for execution
        let full_command = build_docker_run_command(
            &container_id,
            image,
            work,
            memory,
            &self.config.environment,
            &host_dir,
            enable_pi_volume,
            &exec_args,
        );

        // Log only the command — never leak prompts
        let log_command = build_docker_run_command(
            &container_id,
            image,
            work,
            memory,
            &self.config.environment,
            &host_dir,
            enable_pi_volume,
            command,
        );

        debug!(
            "Executing with memory limit {} and volume {}:/workspace: {}",
            memory,
            host_dir,
            log_command.join(" ")
        );

        // Create the command watchdog for per-command timeout enforcement.
        let watchdog = CommandWatchdog::new(&container_id, self.config.per_command_timeout);

        // Wrap the output callback so the stream parser can intercept tool call boundaries.
        let container_id_for_watchdog = container_id.clone();
        let wrapped_callback: std::sync::Arc<dyn Fn(&str) + Send + Sync> = if let Some(cb) = output_callback {
            let watchdog = watchdog.clone();
            let cid = container_id_for_watchdog.clone();
            std::sync::Arc::new(move |line: &str| {
                cb(line);
                watchdog.forward_line(line, &cid);
            })
        } else {
            std::sync::Arc::new(|_line: &str| {})
        };

        // Execute the docker command asynchronously
        let result = execute_docker_command_v2(
            &full_command,
            &container_id,
            timeout_secs,
            &watchdog,
            &wrapped_callback,
        )
        .await
        .with_context(|| format!("Docker command failed: {}", log_command.join(" ")))?;

        // Always clean up the container
        cleanup_container(&container_id).await;

        // Shut down the watchdog so any pending timers are cancelled.
        watchdog.close().await;

        if !result.is_success() {
            let output_preview = if result.output.len() > 500 {
                format!("{}...[truncated]", &result.output[..500])
            } else {
                result.output.clone()
            };
            error!(
                "Docker command failed with exit code {}\n\
                 Command: {}\n\
                 Work dir: {}\n\
                 Output (first 500 chars): {}",
                result.exit_code,
                log_command.join(" "),
                work,
                output_preview
            );
        }

        Ok(result)
    }

    /// Clean up all Docker containers created by this runner (bench-* prefix).
    /// Called on shutdown to prevent orphaned containers.
    pub async fn cleanup_all_containers(&self) {
        info!("Cleaning up all benchmark Docker containers...");
        let mut cmd = Command::new("docker");
        cmd.args(&["ps", "-q", "--filter", "name=bench-"]);

        match timeout(Duration::from_secs(30), cmd.output()).await {
            Ok(Ok(output)) => {
                let container_ids = String::from_utf8_lossy(&output.stdout);
                for container_id in container_ids.lines() {
                    let container_id = container_id.trim();
                    if !container_id.is_empty() {
                        info!("Removing container: {}", container_id);
                        cleanup_container(container_id).await;
                    }
                }
            }
            Ok(Err(e)) => {
                warn!("Failed to list containers for cleanup: {}", e);
            }
            Err(_) => {
                warn!("Timeout listing containers for cleanup");
            }
        }
    }
}

/// Build the docker run command arguments.
fn build_docker_run_command(
    container_id: &str,
    image: &str,
    work: &str,
    memory: &str,
    environment: &HashMap<String, String>,
    host_dir: &str,
    enable_pi_volume: bool,
    command: &[&str],
) -> Vec<String> {
    let mut full_command = Vec::with_capacity(32);
    full_command.push("docker".to_string());
    full_command.push("run".to_string());
    full_command.push("--name".to_string());
    full_command.push(container_id.to_string());
    // Note: not using --rm flag because it doesn't work properly when the
    // docker CLI process is killed (e.g., on timeout). We explicitly clean up
    // after execution instead.
    full_command.push("-w".to_string());
    full_command.push(work.to_string());
    full_command.push("-m".to_string());
    full_command.push(memory.to_string());

    // Add environment variables
    for (key, value) in environment {
        full_command.push("-e".to_string());
        full_command.push(format!("{}={}", key, value));
    }

    // Volume mounts
    full_command.push("-v".to_string());
    full_command.push(format!("{}:/workspace", host_dir));
    full_command.push("-v".to_string());
    full_command.push(format!(
        "{}/.claude:/home/runner/.claude",
        host_dir
    ));
    if enable_pi_volume {
        full_command.push("-v".to_string());
        full_command.push(format!(
            "{}/.pi:/home/runner/.pi",
            host_dir
        ));
    }

    full_command.push(image.to_string());
    for arg in command {
        full_command.push(arg.to_string());
    }

    full_command
}

/// Execute a docker command using tokio::process::Command with async streaming.
/// This version collects output via a channel.
async fn execute_docker_command_v2(
    full_command: &[String],
    container_id: &str,
    timeout_secs: u64,
    _watchdog: &CommandWatchdog,
    output_callback: &std::sync::Arc<OutputCallback>,
) -> Result<ProcessResult, anyhow::Error> {
    let mut cmd = Command::new(&full_command[0]);
    cmd.args(&full_command[1..]);

    // Merge stderr into stdout (like Java's redirectErrorStream(true))
    // so all output is captured and forwarded through the callback
    cmd.stdout(std::process::Stdio::piped());
    cmd.stderr(std::process::Stdio::piped());

    let mut process = cmd
        .spawn()
        .with_context(|| format!("Failed to spawn docker process: {}", full_command[1..].join(" ")))?;

    let stdout = process
        .stdout
        .take()
        .ok_or_else(|| anyhow!("Failed to take stdout"))?;
    let stderr = process
        .stderr
        .take()
        .ok_or_else(|| anyhow!("Failed to take stderr"))?;

    // Channel for collecting output lines
    let (tx, mut rx) = tokio::sync::mpsc::channel::<String>(1024);

    let callback_clone = output_callback.clone();

    // Read stdout and forward through callback + watchdog + channel
    let tx_for_stdout = tx.clone();
    let stdout_task = tokio::spawn(async move {
        let reader = BufReader::new(stdout);
        let mut lines = reader.lines();
        while let Some(line) = lines.next_line().await.unwrap_or(None) {
            // Forward to callback (which also forwards to watchdog)
            callback_clone(&line);
            // Send to channel for collection
            let _ = tx_for_stdout.send(line.clone()).await;
        }
    });

    // Read stderr and forward through same callback + channel (merged with stdout)
    let tx_for_stderr = tx.clone();
    let callback_clone2 = output_callback.clone();
    let stderr_task = tokio::spawn(async move {
        let reader = BufReader::new(stderr);
        let mut lines = reader.lines();
        while let Some(line) = lines.next_line().await.unwrap_or(None) {
            // Forward stderr through callback and channel (merged with stdout)
            callback_clone2(&line);
            let _ = tx_for_stderr.send(line).await;
        }
    });

    // Wait for process with timeout
    let completed = {
        let result = timeout(
            Duration::from_secs(timeout_secs),
            process.wait(),
        )
        .await;

        match result {
            Ok(Ok(status)) => status.success(),
            Ok(Err(e)) => {
                error!("Process wait error: {}", e);
                false
            }
            Err(_) => {
                warn!("Process timed out after {} seconds", timeout_secs);
                false
            }
        }
    };

    // Kill the process if it didn't complete
    let exit_code = if completed {
        process
            .wait()
            .await
            .ok()
            .and_then(|s| s.code())
            .unwrap_or(137) // 128 + SIGKILL(9) as conventional sentinel for killed process
    } else {
        let _ = process.kill().await;
        process
            .wait()
            .await
            .ok()
            .and_then(|s| s.code())
            .unwrap_or(137) // 128 + SIGKILL(9) as conventional sentinel for killed process
    };

    // Wait for reader tasks to finish
    let _ = tokio::join!(stdout_task, stderr_task);
    drop(tx); // Close the channel so rx.recv() terminates

    // Collect output from channel
    let mut output = String::new();
    while let Some(line) = rx.recv().await {
        output.push_str(&line);
        output.push('\n');
    }

    Ok(ProcessResult {
        exit_code,
        output,
        completed,
        container_id: container_id.to_string(),
    })
}

/// Clean up a Docker container by name.
async fn cleanup_container(container_name: &str) {
    let mut cmd = Command::new("docker");
    cmd.args(&["rm", "-f", container_name]);

    match timeout(Duration::from_secs(30), cmd.output()).await {
        Ok(Ok(output)) => {
            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                warn!("Failed to cleanup container {}: {}", container_name, stderr);
            }
        }
        Ok(Err(e)) => {
            warn!("Failed to cleanup container {}: {}", container_name, e);
        }
        Err(_) => {
            warn!(
                "Cleanup timeout for container {}, forcing",
                container_name
            );
            let mut kill_cmd = Command::new("docker");
            kill_cmd.args(&["rm", "-f", container_name]);
            let _ = kill_cmd.output().await;
        }
    }
}

