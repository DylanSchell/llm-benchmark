use super::watchdog::CommandWatchdog;
use anyhow::{anyhow, Context};
use benchmark_types::cancellation::CancellationToken;
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
        cancellation: Option<CancellationToken>,
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
            cancellation,
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
        cancellation: Option<CancellationToken>,
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
            cancellation,
        )
        .await
        .with_context(|| format!("Docker command failed: {}", log_command.join(" ")))?;

        // Always clean up the container
        cleanup_container(&container_id).await;

        // Shut down the watchdog so any pending timers are cancelled.
        watchdog.close().await;

        if !result.is_success() {
            let output_preview = if result.output.len() > 500 {
                format!("{}...[truncated]", crate::safe_truncate(&result.output, 500))
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
///
/// Includes a container liveness monitor that polls `docker inspect` every 5s.
/// If the container dies but the docker CLI process hangs (known Docker issue),
/// the liveness monitor signals an abort so the function doesn't stall.
async fn execute_docker_command_v2(
    full_command: &[String],
    container_id: &str,
    timeout_secs: u64,
    _watchdog: &CommandWatchdog,
    output_callback: &std::sync::Arc<OutputCallback>,
    cancellation: Option<CancellationToken>,
) -> Result<ProcessResult, anyhow::Error> {
    let mut cmd = Command::new(&full_command[0]);
    cmd.args(&full_command[1..]);

    // Merge stderr into stdout (like Java's redirectErrorStream(true))
    // so all output is captured and forwarded through the callback.
    // Close stdin so the agent doesn't wait for interactive input.
    cmd.stdout(std::process::Stdio::piped());
    cmd.stderr(std::process::Stdio::piped());
    cmd.stdin(std::process::Stdio::null());

    let mut process = cmd
        .spawn()
        .with_context(|| format!("Failed to spawn docker process: {}", full_command[1..].join(" ")))?;

    let pid = process.id().unwrap_or(0);

    let stdout = process
        .stdout
        .take()
        .ok_or_else(|| anyhow!("Failed to take stdout"))?;
    let stderr = process
        .stderr
        .take()
        .ok_or_else(|| anyhow!("Failed to take stderr"))?;

    // Channel for collecting output lines. Use unbounded to prevent
    // deadlocks when agents produce large amounts of output (e.g., pi
    // writing 10MB+ of JSON to stdout). A bounded channel can fill up
    // and block the reader tasks, preventing process cleanup.
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<String>();

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
            let _ = tx_for_stdout.send(line.clone());
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
            let _ = tx_for_stderr.send(line);
        }
    });

    // Container liveness monitor: polls `docker inspect` every 5s in parallel
    // with process.wait(). If the container dies but the docker CLI process
    // is stuck (e.g. pipe deadlock), this signals an abort instead of hanging.
    let cid_for_liveness = container_id.to_string();
    let liveness_dead = std::sync::Arc::new(tokio::sync::Notify::new());
    let liveness_dead_clone = liveness_dead.clone();
    let liveness_task = tokio::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_secs(5)).await;
            match container_is_running(&cid_for_liveness).await {
                Ok(true) => continue, // alive
                Ok(false) => {
                    warn!("Liveness: container {} no longer running — signalling abort", cid_for_liveness);
                    liveness_dead_clone.notify_one();
                    return;
                }
                Err(_) => continue, // transient error, retry
            }
        }
    });

    // Wait for process exit, liveness abort, cancellation, or global timeout.
    // The wait is polled in a loop so the 500ms cancellation check doesn't
    // reset the timeout deadline (the deadline is absolute).
    let deadline = tokio::time::Instant::now() + Duration::from_secs(timeout_secs);
    let completed = {
        loop {
            tokio::select! {
                biased;
                status_result = process.wait() => {
                    match status_result {
                        Ok(status) => break status.success(),
                        Err(e) => { error!("Process wait error: {}", e); break false }
                    }
                }
                () = liveness_dead.notified() => {
                    warn!("Container {} died — aborting process wait", container_id);
                    break false;
                }
                _ = cancel_poll(&cancellation) => {
                    if cancellation.as_ref().is_some_and(|t| t.is_cancelled()) {
                        warn!(
                            "Session cancelled — killing docker run for container {}",
                            container_id
                        );
                        break false;
                    }
                }
                () = tokio::time::sleep_until(deadline) => {
                    warn!("Process timed out after {} seconds", timeout_secs);
                    break false;
                }
            }
        }
    };
    liveness_task.abort();

    // Kill the process if it didn't complete. Guard each phase with its own
    // timeout so no single failure can stall indefinitely.
    let exit_code = if completed {
        let wait_result = timeout(Duration::from_secs(10), process.wait()).await;
        match wait_result {
            Ok(Ok(status)) => status.code().unwrap_or(0),
            _ => { force_kill_container(container_id).await; 137 }
        }
    } else {
        // Phase 1: kill the docker CLI process
        let _ = timeout(Duration::from_secs(5), process.kill()).await;
        // Phase 2: force-kill the container directly (more reliable)
        force_kill_container(container_id).await;
        // Phase 3: wait for process to reap, with timeout
        let wait_result = timeout(Duration::from_secs(15), process.wait()).await;
        match wait_result {
            Ok(Ok(status)) => status.code().unwrap_or(137),
            _ => { error!("Process pid={} did not exit after kill+rm", pid); 137 }
        }
    };

    // Wait for reader tasks to finish (with timeout, in case pipes won't close)
    drop(tx);
    let _ = timeout(Duration::from_secs(30), async { let _ = tokio::join!(stdout_task, stderr_task); }).await;

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

/// Wait for the next cancellation poll tick. Returns immediately when the
/// token is (already) cancelled; otherwise sleeps 500ms so the caller can
/// re-check process liveness. The 500ms granularity keeps the Docker CLI
/// process responsive to user cancellation without busy-polling.
async fn cancel_poll(cancellation: &Option<CancellationToken>) {
    if cancellation.as_ref().is_some_and(|t| t.is_cancelled()) {
        return;
    }
    tokio::time::sleep(Duration::from_millis(500)).await;
}

/// Check if a Docker container is currently running via `docker inspect`.
async fn container_is_running(container_id: &str) -> Result<bool, anyhow::Error> {
    let output = tokio::process::Command::new("docker")
        .args(&["inspect", "--format={{.State.Running}}", container_id])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .output()
        .await?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).trim() == "true")
    } else {
        Ok(false) // container doesn't exist
    }
}

/// Force-kill a Docker container by name using `docker rm -f`.
/// More reliable than killing the CLI process when the CLI itself is stuck.
async fn force_kill_container(container_id: &str) {
    let mut kill_cmd = tokio::process::Command::new("docker");
    kill_cmd.args(&["rm", "-f", container_id]);
    kill_cmd.stdout(std::process::Stdio::null());
    kill_cmd.stderr(std::process::Stdio::null());
    match timeout(Duration::from_secs(10), kill_cmd.output()).await {
        Ok(Ok(o)) if o.status.success() => info!("Force-killed container: {}", container_id),
        Ok(Ok(o)) => warn!("docker rm -f {} exited with {}", container_id, o.status.code().unwrap_or(-1)),
        Ok(Err(e)) => warn!("Failed to force-kill container {}: {}", container_id, e),
        Err(_) => warn!("Timeout force-killing container {}", container_id),
    }
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


// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use benchmark_types::cancellation::CancellationToken;

    /// Regression: a cancelled session must abort an in-flight Docker run
    /// promptly instead of waiting out the (default 3600s) container timeout.
    /// Runs `sleep 999` as the "container process"; docker CLI calls fail
    /// harmlessly (no daemon needed) — only the process lifecycle matters.
    #[tokio::test]
    async fn pre_cancelled_token_aborts_docker_run_promptly() {
        let token = CancellationToken::new();
        token.cancel();

        let watchdog = CommandWatchdog::new("bench-cancel-test", 120);
        let callback: std::sync::Arc<OutputCallback> = std::sync::Arc::new(|_| {});

        let started = std::time::Instant::now();
        let result = tokio::time::timeout(
            Duration::from_secs(10),
            execute_docker_command_v2(
                &["sleep".to_string(), "999".to_string()],
                "bench-cancel-test",
                3600,
                &watchdog,
                &callback,
                Some(token),
            ),
        )
        .await
        .expect("cancellation must return promptly, not wait for the timeout")
        .expect("kill path must not return Err");

        assert!(!result.completed, "cancelled run must not report completed");
        assert_eq!(result.exit_code, 137, "process should be killed (SIGKILL)");
        assert!(
            started.elapsed() < Duration::from_secs(10),
            "abort must happen well before the 3600s timeout"
        );
    }

    /// Guard against regressions in the wait-loop refactor: the timeout arm
    /// must still fire when no cancellation occurs.
    #[tokio::test]
    async fn timeout_still_fires_when_not_cancelled() {
        let token = CancellationToken::new(); // never cancelled
        let watchdog = CommandWatchdog::new("bench-timeout-test", 120);
        let callback: std::sync::Arc<OutputCallback> = std::sync::Arc::new(|_| {});

        let started = std::time::Instant::now();
        let result = tokio::time::timeout(
            Duration::from_secs(10),
            execute_docker_command_v2(
                &["sleep".to_string(), "999".to_string()],
                "bench-timeout-test",
                1, // 1s container timeout
                &watchdog,
                &callback,
                Some(token),
            ),
        )
        .await
        .expect("timeout must fire")
        .expect("kill path must not return Err");

        assert!(!result.completed);
        assert_eq!(result.exit_code, 137);
        assert!(
            started.elapsed() >= Duration::from_millis(900),
            "timeout should take ~1s, not abort instantly"
        );
        assert!(started.elapsed() < Duration::from_secs(10));
    }

    /// A normal short command must still complete with a (non-cancelled)
    /// token attached — no false aborts.
    #[tokio::test]
    async fn short_command_completes_with_token_attached() {
        let token = CancellationToken::new(); // never cancelled
        let watchdog = CommandWatchdog::new("bench-echo-test", 120);
        let callback: std::sync::Arc<OutputCallback> = std::sync::Arc::new(|_| {});

        let result = tokio::time::timeout(
            Duration::from_secs(10),
            execute_docker_command_v2(
                &["echo".to_string(), "hi".to_string()],
                "bench-echo-test",
                3600,
                &watchdog,
                &callback,
                Some(token),
            ),
        )
        .await
        .expect("echo should complete")
        .expect("echo must not return Err");

        assert!(result.completed);
        assert_eq!(result.exit_code, 0);
        assert_eq!(result.output.trim(), "hi");
    }
}
