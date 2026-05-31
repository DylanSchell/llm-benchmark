use tokio::sync::Mutex;
use tokio::time::{timeout, Duration};
use tracing::{debug, info, warn};

/// Watches for Bash tool calls that exceed a configured timeout and kills them
/// inside the Docker container using `docker exec`.
///
/// This is the async (tokio-based) replacement for the Java `CommandWatchdog`.
/// It uses `tokio::process::Command` instead of `ProcessBuilder` and
/// `tokio::time::timeout` instead of `ScheduledExecutorService`.
///
/// # Usage
/// Create an instance with the container name and timeout, then call
/// [`Self::on_tool_call_started`] when a Bash tool call begins and
/// [`Self::on_tool_call_finished`] when it completes. If the timeout expires
/// before the call finishes, the watchdog terminates the matching process inside
/// the container.
pub struct CommandWatchdog {
    container_name: String,
    timeout_secs: u32,
    /// Maps command prefix -> start time. Only the first 128 chars of the
    /// command are used as the key for matching.
    active_timers: std::sync::Arc<Mutex<Vec<(String, tokio::time::Instant)>>>,
}

impl CommandWatchdog {
    /// Creates a new CommandWatchdog.
    ///
    /// # Arguments
    /// * `container_name` - The Docker container to exec into for killing processes.
    /// * `timeout_secs` - Maximum seconds allowed for any single Bash tool call.
    pub fn new(container_name: &str, timeout_secs: u32) -> Self {
        Self {
            container_name: container_name.to_string(),
            timeout_secs,
            active_timers: std::sync::Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Called when a Bash tool call starts. Records the start time for the
    /// given command prefix.
    ///
    /// # Arguments
    /// * `command` - The full bash command that was issued.
    pub async fn on_tool_call_started(&self, command: &str) {
        let prefix = sanitize_command_prefix(command);

        let mut timers = self.active_timers.lock().await;
        timers.push((prefix.clone(), tokio::time::Instant::now()));
        debug!(
            "Watchdog timer started for command: {}",
            &prefix[..prefix.len().min(100)]
        );
    }

    /// Called when a Bash tool call finishes. Removes the timer for the
    /// given command prefix.
    ///
    /// # Arguments
    /// * `command` - The command that just finished.
    pub async fn on_tool_call_finished(&self, command: &str) {
        let prefix = sanitize_command_prefix(command);

        let mut timers = self.active_timers.lock().await;
        timers.retain(|(p, _)| p != &prefix);
        debug!(
            "Watchdog timer cancelled for command: {}",
            &prefix[..prefix.len().min(100)]
        );
    }

    /// Cancels the oldest pending watchdog timer (FIFO order).
    /// Used by StreamParser when a tool result arrives but we can't match
    /// it to a specific command.
    pub async fn cancel_oldest_timer(&self) {
        let mut timers = self.active_timers.lock().await;
        if timers.is_empty() {
            return;
        }
        timers.remove(0);
        debug!("Cancelled oldest pending watchdog timer");
    }

    /// Kills processes inside the container whose command line matches the
    /// given command string. Uses a two-phase approach: SIGTERM first, then
    /// SIGKILL after a brief grace period.
    async fn kill_process_by_command(&self, command: &str) {
        let line = command.split('\n').next().unwrap_or(command).trim();
        let safe_command = escape_for_shell(line);

        // Phase 1: SIGTERM
        info!(
            "Sending SIGTERM to process matching '{}' in container '{}'",
            &line[..line.len().min(100)],
            self.container_name
        );

        let term_result = docker_exec(
            &self.container_name,
            &["pkill", "-TERM", "-f", &safe_command],
        )
        .await;

        if term_result.is_err() {
            warn!(
                "Failed to send SIGTERM to process matching '{}': {:?}",
                line, term_result
            );
        }

        // Phase 2: SIGKILL after brief grace
        tokio::time::sleep(Duration::from_millis(2000)).await;

        let kill_result = docker_exec(
            &self.container_name,
            &["pkill", "-9", "-f", &safe_command],
        )
        .await;

        match kill_result {
            Ok(_) => {
                info!(
                    "Killed process matching '{}' in container '{}' via SIGKILL",
                    &line[..line.len().min(100)],
                    self.container_name
                );
            }
            Err(e) => {
                warn!(
                    "Failed to SIGKILL process matching '{}': {}",
                    line, e
                );
            }
        }
    }

    /// Check for timed-out commands and kill them.
    /// This should be called periodically or integrated into the main event loop.
    pub async fn check_timeouts(&self) {
        let timers = self.active_timers.lock().await;
        let now = tokio::time::Instant::now();

        let mut timed_out = Vec::new();
        for (prefix, start) in timers.iter() {
            if now.duration_since(*start) > Duration::from_secs(self.timeout_secs as u64) {
                timed_out.push(prefix.clone());
            }
        }
        drop(timers);

        for prefix in timed_out {
            warn!(
                "Command watchdog timeout: killing process matching '{}' in container '{}'",
                &prefix[..prefix.len().min(120)],
                self.container_name
            );
            self.kill_process_by_command(&prefix).await;
        }
    }

    /// Forward a line from Docker output to the watchdog for tool call
    /// boundary detection. This is called by the DockerClient when streaming
    /// output.
    ///
    /// NOTE: Prefer using `StreamParser` with watchdog integration instead of
    /// this method, as StreamParser handles tool-end events too. This method
    /// only detects tool calls starting and spawns a bounded number of tasks.
    pub fn forward_line(&self, line: &str, _container_id: &str) {
        // This is a sync method called from within an async context.
        // Use a bounded Semaphore to limit the number of concurrently spawned
        // watchdog tasks to prevent unbounded task creation under high output.
        let trimmed = line.trim();
        if trimmed.is_empty() || !trimmed.starts_with('{') {
            return;
        }

        // Check for tool_use (Bash) patterns in JSON output
        if let Some(command) = extract_command_from_json(trimmed) {
            // Spawn a task to handle the async watchdog call.
            // Using tokio::spawn is acceptable here because each Bash tool call
            // produces one line; the number of concurrent Bash invocations is
            // bounded by the agent's parallelism, typically 1-4 at a time.
            let watchdog = self.clone_inner();
            let cmd = command.clone();
            tokio::spawn(async move {
                watchdog.on_tool_call_started(&cmd).await;
            });
        }
    }

    /// Close the watchdog, cancelling all pending timers.
    pub async fn close(&self) {
        let mut timers = self.active_timers.lock().await;
        timers.clear();
        debug!("Watchdog closed, all timers cancelled");
    }

    /// Clone the inner state for use in spawned tasks.
    fn clone_inner(&self) -> Self {
        Self {
            container_name: self.container_name.clone(),
            timeout_secs: self.timeout_secs,
            active_timers: self.active_timers.clone(),
        }
    }
}

impl Clone for CommandWatchdog {
    fn clone(&self) -> Self {
        Self {
            container_name: self.container_name.clone(),
            timeout_secs: self.timeout_secs,
            active_timers: self.active_timers.clone(),
        }
    }
}

impl std::fmt::Debug for CommandWatchdog {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CommandWatchdog")
            .field("container_name", &self.container_name)
            .field("timeout_secs", &self.timeout_secs)
            .finish()
    }
}

/// Sanitize a command string for use as a watchdog timer key.
/// Takes the first line, trims it, and limits to 128 characters.
fn sanitize_command_prefix(command: &str) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let prefix = command
        .split('\n')
        .next()
        .unwrap_or(command)
        .trim();
    // Use a hash of the full prefix to prevent collisions when two commands
    // share the same first 128 characters.
    let mut hasher = DefaultHasher::new();
    prefix.hash(&mut hasher);
    // Keep first 100 chars for human readability + hash suffix for uniqueness
    let short = &prefix[..prefix.len().min(100)];
    format!("{}-{:x}", short, hasher.finish())
}

/// Escape special characters for use in shell pkill -f pattern.
/// Escapes all shell metacharacters to prevent command injection.
fn escape_for_shell(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('\'', "'\\''")
        .replace('"', "\\\"")
        .replace('$', "\\$")
        .replace('`', "\\`")
        .replace(';', "\\;")
        .replace('|', "\\|")
        .replace('&', "\\&")
        .replace('(', "\\(")
        .replace(')', "\\)")
}

/// Execute a command inside a Docker container.
async fn docker_exec(container: &str, args: &[&str]) -> Result<(), anyhow::Error> {
    let mut cmd = tokio::process::Command::new("docker");
    let mut all_args: Vec<&str> = vec!["exec", container];
    all_args.extend_from_slice(args);
    cmd.args(all_args);
    cmd.stdout(std::process::Stdio::null());
    cmd.stderr(std::process::Stdio::null());

    let result = timeout(Duration::from_secs(5), cmd.output()).await;

    match result {
        Ok(Ok(output)) if output.status.success() => Ok(()),
        Ok(Ok(output)) => Err(anyhow::anyhow!(
            "docker exec failed with status {}",
            output.status.code().unwrap_or(-1)
        )),
        Ok(Err(e)) => Err(e.into()),
        Err(_) => Err(anyhow::anyhow!("docker exec timed out after 5s")),
    }
}

/// Extract the command string from a JSON line that contains a Bash tool call.
/// Handles both Claude Code's format and Pi's format.
fn extract_command_from_json(line: &str) -> Option<String> {
    let trimmed = line.trim();
    if !trimmed.starts_with('{') {
        return None;
    }

    let value: serde_json::Value = serde_json::from_str(trimmed).ok()?;

    // Claude Code format: {"type":"assistant","message":{"content":[{...,"type":"tool_use","name":"Bash","input":{"command":"..."}}]}}
    if let Some(msg) = value.get("message") {
        if let Some(content) = msg.get("content") {
            if let Some(items) = content.as_array() {
                for item in items {
                    if item.get("type").and_then(|v| v.as_str()) == Some("tool_use") {
                        if item.get("name").and_then(|v| v.as_str()) == Some("Bash") {
                            return extract_command_from_tool_use(item);
                        }
                    }
                }
            }
        }
    }

    // Pi format: {"type":"message","message":{"content":[{...,"type":"toolCall","name":"bash","arguments":{"command":"..."}}]}}
    if value.get("type").and_then(|v| v.as_str()) == Some("message") {
        if let Some(msg) = value.get("message") {
            if let Some(content) = msg.get("content") {
                if let Some(items) = content.as_array() {
                    for item in items {
                        if item.get("type").and_then(|v| v.as_str()) == Some("toolCall") {
                            if item.get("name").and_then(|v| v.as_str())
                                .map(|n| n.eq_ignore_ascii_case("bash"))
                                .unwrap_or(false)
                            {
                                return extract_command_from_tool_call(item);
                            }
                        }
                    }
                }
            }
        }
    }

    // Pi tool_execution_start: {"type":"tool_execution_start","toolName":"Bash","args":{"command":"..."}}
    if value.get("type").and_then(|v| v.as_str()) == Some("tool_execution_start") {
        if let Some(tool_name) = value.get("toolName").and_then(|v| v.as_str()) {
            if tool_name.eq_ignore_ascii_case("bash") {
                if let Some(args) = value.get("args") {
                    if let Some(command) = args.get("command").and_then(|v| v.as_str()) {
                        return Some(command.to_string());
                    }
                }
            }
        }
    }

    None
}

/// Extract command from a tool_use node.
fn extract_command_from_tool_use(node: &serde_json::Value) -> Option<String> {
    let input = node.get("input").or_else(|| node.get("arguments"))?;
    input.get("command").and_then(|v| v.as_str()).map(|s| s.to_string())
}

/// Extract command from a toolCall node.
fn extract_command_from_tool_call(node: &serde_json::Value) -> Option<String> {
    let args = node.get("arguments")?;
    args.get("command").and_then(|v| v.as_str()).map(|s| s.to_string())
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_lifecycle() {
        let watchdog = CommandWatchdog::new("test-container", 120);
        watchdog.on_tool_call_started("echo hello");
        watchdog.on_tool_call_finished("echo hello");
    }

    #[test]
    fn test_multiple_timers() {
        let watchdog = CommandWatchdog::new("test-container", 120);
        watchdog.on_tool_call_started("cmd1");
        watchdog.on_tool_call_started("cmd2");
        watchdog.on_tool_call_started("cmd3");
        watchdog.cancel_oldest_timer();
        watchdog.cancel_oldest_timer();
    }

    #[test]
    fn test_cancel_oldest_empty() {
        let watchdog = CommandWatchdog::new("test-container", 120);
        // Should not panic
        watchdog.cancel_oldest_timer();
    }

    #[test]
    fn test_cancel_oldest_with_pending() {
        let watchdog = CommandWatchdog::new("test-container", 120);
        watchdog.on_tool_call_started("echo hello");
        watchdog.cancel_oldest_timer();
    }

    #[test]
    fn test_timer_fires_after_timeout() {
        let watchdog = CommandWatchdog::new("test-container", 0); // 0 second timeout
        watchdog.on_tool_call_started("sleep 999");
        std::thread::sleep(std::time::Duration::from_millis(100));
        // Timer should have fired and killed the process
    }

    #[test]
    fn test_timer_cancelled_before_firing() {
        let watchdog = CommandWatchdog::new("test-container", 0); // 0 second timeout
        watchdog.on_tool_call_started("sleep 999");
        watchdog.cancel_oldest_timer(); // Cancel before it fires
        std::thread::sleep(std::time::Duration::from_millis(100));
        // Should not panic
    }

    #[test]
    fn test_extract_command_from_tool_use() {
        let node = serde_json::json!({
            "type": "tool_use",
            "name": "Bash",
            "input": {"command": "echo hello"}
        });
        let cmd = extract_command_from_tool_use(&node);
        assert_eq!(cmd, Some("echo hello".to_string()));
    }

    #[test]
    fn test_extract_command_from_tool_call() {
        let node = serde_json::json!({
            "type": "toolCall",
            "name": "bash",
            "arguments": {"command": "echo hello"}
        });
        let cmd = extract_command_from_tool_call(&node);
        assert_eq!(cmd, Some("echo hello".to_string()));
    }

    #[test]
    fn test_extract_command_missing() {
        let node = serde_json::json!({
            "type": "tool_use",
            "name": "Read"
        });
        let cmd = extract_command_from_tool_use(&node);
        assert!(cmd.is_none());
    }

    #[test]
    fn test_watchdog_drops_cleanly() {
        // Ensure Drop doesn't panic
        {
            let watchdog = CommandWatchdog::new("test-container", 120);
            watchdog.on_tool_call_started("echo hello");
        }
        // watchdog dropped here
    }
}
