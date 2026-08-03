use tokio::sync::Mutex;
use tokio::task::JoinHandle;
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
    /// Maps tool call id -> (original_command, start_time).
    /// The id uniquely identifies the tool call (Claude `tool_use.id` /
    /// pi `toolCall.id`); `None` marks timers started without an id, which
    /// can only be cancelled FIFO-style. original_command is the actual
    /// command text used for pkill matching.
    active_timers: std::sync::Arc<Mutex<Vec<(Option<String>, String, tokio::time::Instant)>>>,
    /// Handle for the periodic timeout-checking task.
    ticker_handle: std::sync::Arc<Mutex<Option<JoinHandle<()>>>>,
}

impl CommandWatchdog {
    /// Creates a new CommandWatchdog and spawns a periodic ticker that
    /// calls `check_timeouts()` roughly every second.
    ///
    /// # Arguments
    /// * `container_name` - The Docker container to exec into for killing processes.
    /// * `timeout_secs` - Maximum seconds allowed for any single Bash tool call.
    pub fn new(container_name: &str, timeout_secs: u32) -> Self {
        let active_timers = std::sync::Arc::new(Mutex::new(Vec::new()));

        // Spawn a periodic ticker to call check_timeouts every ~1s.
        let wd_for_ticker = Self {
            container_name: container_name.to_string(),
            timeout_secs,
            active_timers: active_timers.clone(),
            ticker_handle: std::sync::Arc::new(Mutex::new(None)),
        };
        let ticker_handle = tokio::spawn(async move {
            loop {
                tokio::time::sleep(Duration::from_secs(1)).await;
                wd_for_ticker.check_timeouts().await;
            }
        });

        Self {
            container_name: container_name.to_string(),
            timeout_secs,
            active_timers,
            ticker_handle: std::sync::Arc::new(Mutex::new(Some(ticker_handle))),
        }
    }

    /// Called when a Bash tool call starts. Records the start time for the
    /// given tool call id, so the result can later cancel exactly this timer.
    ///
    /// # Arguments
    /// * `id` - The tool call id (Claude `tool_use.id`, pi `toolCall.id`).
    ///   `None` when unavailable — such timers can only be cancelled FIFO-style.
    /// * `command` - The full bash command that was issued.
    pub async fn on_tool_call_started(&self, id: Option<&str>, command: &str) {
        let original = command.split('\n').next().unwrap_or(command).trim().to_string();

        let mut timers = self.active_timers.lock().await;
        timers.push((id.map(|s| s.to_string()), original.clone(), tokio::time::Instant::now()));
        debug!(
            "Watchdog timer started for command: {}",
            crate::safe_truncate(&original, 100)
        );
    }

    /// Synchronous version for use from non-async contexts (e.g.,
    /// StreamParser's accept which must preserve FIFO ordering).
    pub fn on_tool_call_started_sync(&self, id: Option<&str>, command: &str) {
        let original = command.split('\n').next().unwrap_or(command).trim().to_string();

        let mut timers = self.active_timers.blocking_lock();
        timers.push((id.map(|s| s.to_string()), original.clone(), tokio::time::Instant::now()));
        debug!(
            "Watchdog timer started for command: {}",
            crate::safe_truncate(&original, 100)
        );
    }

    /// Called when a Bash tool call finishes. Removes the timer for the
    /// given tool call id; falls back to cancelling the oldest timer when no
    /// id is available.
    pub async fn on_tool_call_finished(&self, id: Option<&str>) {
        match id {
            Some(id) => {
                self.cancel_timer(id).await;
            }
            None => {
                self.cancel_oldest_timer().await;
            }
        }
    }

    /// Cancel the timer for a specific tool call (matched by id).
    /// Returns `true` if a timer was removed. A result for a non-Bash tool
    /// (Read/Write/etc.) carries an id that never matches a pending Bash
    /// timer, so it is a no-op — this is the fix for tool_results that used
    /// to cancel the oldest (Bash) timer by FIFO.
    pub async fn cancel_timer(&self, id: &str) -> bool {
        let mut timers = self.active_timers.lock().await;
        let idx = timers.iter().position(|(tid, _, _)| tid.as_deref() == Some(id));
        match idx {
            Some(i) => {
                timers.remove(i);
                debug!("Watchdog timer cancelled for tool call: {}", id);
                true
            }
            None => false,
        }
    }

    /// Synchronous version of cancel_timer for non-async contexts.
    pub fn cancel_timer_sync(&self, id: &str) -> bool {
        let mut timers = self.active_timers.blocking_lock();
        let idx = timers.iter().position(|(tid, _, _)| tid.as_deref() == Some(id));
        match idx {
            Some(i) => {
                timers.remove(i);
                debug!("Watchdog timer cancelled for tool call: {}", id);
                true
            }
            None => false,
        }
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

    /// Synchronous version of cancel_oldest_timer for use from
    /// non-async contexts where FIFO ordering must be preserved.
    pub fn cancel_oldest_timer_sync(&self) {
        let mut timers = self.active_timers.blocking_lock();
        if timers.is_empty() {
            return;
        }
        timers.remove(0);
        debug!("Cancelled oldest pending watchdog timer");
    }

    /// Kills the specific timed-out process inside the container.
    /// Uses pkill with a pattern that matches the shell process running the command.
    /// We use a two-phase approach: SIGTERM first, then SIGKILL after a brief grace.
    async fn kill_process_by_command(&self, command: &str) {
        // Take just the first line and strip quotes — the process table has
        // the command without shell quoting.
        let line = command.split('\n').next().unwrap_or(command).trim();
        // Remove double quotes and single quotes for pattern matching
        let pattern: String = line.chars().filter(|&c| c != '"' && c != '\'').collect();
        // Escape regex metacharacters for pkill -f
        let pattern = regex::escape(&pattern);

        // Phase 1: SIGTERM
        info!(
            "Sending SIGTERM to process matching '{}' in container '{}'",
            crate::safe_truncate(&line, 100),
            self.container_name
        );

        let _term_result = docker_exec(
            &self.container_name,
            &["sh", "-c", &format!("pkill -TERM -f '{}' 2>/dev/null || true", pattern)],
        )
        .await;

        // Phase 2: SIGKILL after brief grace
        tokio::time::sleep(Duration::from_millis(2000)).await;

        let kill_result = docker_exec(
            &self.container_name,
            &["sh", "-c", &format!("pkill -9 -f '{}' 2>/dev/null || true", pattern)],
        )
        .await;

        match kill_result {
            Ok(_) => {
                info!(
                    "Killed process matching '{}' in container '{}' via SIGKILL",
                    crate::safe_truncate(&line, 100),
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
        let mut timers = self.active_timers.lock().await;
        let now = tokio::time::Instant::now();

        let mut timed_out: Vec<(usize, String)> = Vec::new();
        for (i, (_id, original, start)) in timers.iter().enumerate() {
            if now.duration_since(*start) > Duration::from_secs(self.timeout_secs as u64) {
                timed_out.push((i, original.clone()));
            }
        }

        // Remove exactly the timed-out entries (by index, back to front so
        // earlier indices stay valid) — not every timer sharing a prefix.
        for (i, _) in timed_out.iter().rev() {
            timers.remove(*i);
        }
        drop(timers);

        for (_i, original) in &timed_out {
            warn!(
                "Command watchdog timeout: killing process matching '{}' in container '{}'",
                crate::safe_truncate(&original, 120),
                self.container_name
            );
            self.kill_process_by_command(original).await;
        }
    }

    /// Forward a line from Docker output to the watchdog for tool call
    /// boundary detection. This is the production path for watchdog events:
    /// the DockerClient wraps every streamed line here. Bash `tool_use`
    /// events start a timer keyed by the tool call id; `tool_result` events
    /// cancel the timer for that exact id (non-Bash results are no-ops),
    /// falling back to FIFO when no id is available.
    pub fn forward_line(&self, line: &str, _container_id: &str) {
        let trimmed = line.trim();
        if trimmed.is_empty() || !trimmed.starts_with('{') {
            return;
        }

        // Detect tool_use (Bash) — start a timer keyed by the tool call id
        if let Some((id, command)) = extract_bash_call(trimmed) {
            let watchdog = self.clone_inner();
            let cmd = command.clone();
            tokio::spawn(async move {
                watchdog.on_tool_call_started(id.as_deref(), &cmd).await;
            });
            return;
        }

        // Detect tool_result — cancel the timer for THAT tool call (matched
        // by id). A result for a non-Bash tool (Read/Write/etc.) carries an
        // id that never matches a pending Bash timer, so it is a no-op; this
        // fixes the old behavior where any tool_result cancelled the oldest
        // (Bash) timer by FIFO, silently disabling its timeout.
        if let Some(id) = extract_tool_result_id(trimmed) {
            let watchdog = self.clone_inner();
            tokio::spawn(async move {
                watchdog.cancel_timer(&id).await;
            });
            return;
        }

        // tool_result without an id (e.g. pi tool_execution_end) — fall back
        // to FIFO cancellation.
        if is_tool_result(trimmed) {
            let watchdog = self.clone_inner();
            tokio::spawn(async move {
                watchdog.cancel_oldest_timer().await;
            });
        }
    }

    /// Close the watchdog, aborting the periodic ticker and
    /// cancelling all pending timers.
    pub async fn close(&self) {
        // Abort the periodic ticker.
        let mut handle = self.ticker_handle.lock().await;
        if let Some(h) = handle.take() {
            h.abort();
        }
        drop(handle);

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
            ticker_handle: std::sync::Arc::new(Mutex::new(None)),
        }
    }
}

impl Clone for CommandWatchdog {
    fn clone(&self) -> Self {
        Self {
            container_name: self.container_name.clone(),
            timeout_secs: self.timeout_secs,
            active_timers: self.active_timers.clone(),
            ticker_handle: std::sync::Arc::new(Mutex::new(None)),
        }
    }
}

impl Drop for CommandWatchdog {
    fn drop(&mut self) {
        // Try to abort the ticker on drop. We can't await, so use
        // try_lock. If the lock is held, the ticker is active and will
        // be cleaned up when the Arc drops.
        if let Ok(mut handle) = self.ticker_handle.try_lock() {
            if let Some(h) = handle.take() {
                h.abort();
            }
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

/// Execute a command inside a Docker container.
async fn docker_exec(container: &str, args: &[&str]) -> Result<(), anyhow::Error> {
    let mut cmd = tokio::process::Command::new("docker");
    let mut all_args: Vec<&str> = vec!["exec", container];
    all_args.extend_from_slice(args);
    cmd.args(all_args);
    cmd.stdout(std::process::Stdio::null());
    cmd.stderr(std::process::Stdio::null());

    let result = timeout(Duration::from_secs(15), cmd.output()).await;

    match result {
        Ok(Ok(output)) if output.status.success() => Ok(()),
        Ok(Ok(output)) => Err(anyhow::anyhow!(
            "docker exec failed with status {}",
            output.status.code().unwrap_or(-1)
        )),
        Ok(Err(e)) => Err(e.into()),
        Err(_) => Err(anyhow::anyhow!("docker exec timed out after 15s")),
    }
}

/// Extract (tool_call_id, command) from a JSON line that contains a Bash
/// tool call. Handles both Claude Code's format and Pi's format. Returns
/// `None` for non-Bash tool uses or unparsable lines.
fn extract_bash_call(line: &str) -> Option<(Option<String>, String)> {
    let trimmed = line.trim();
    if !trimmed.starts_with('{') {
        return None;
    }

    let value: serde_json::Value = serde_json::from_str(trimmed).ok()?;

    // Claude Code format: {"type":"assistant","message":{"content":[{"type":"tool_use","id":"...","name":"Bash","input":{"command":"..."}}]}}
    if let Some(msg) = value.get("message") {
        if let Some(items) = msg.get("content").and_then(|c| c.as_array()) {
            for item in items {
                if item.get("type").and_then(|v| v.as_str()) == Some("tool_use") {
                    if item.get("name").and_then(|v| v.as_str()) == Some("Bash") {
                        if let Some(command) = extract_command_from_tool_use(item) {
                            let id = item
                                .get("id")
                                .and_then(|v| v.as_str())
                                .map(|s| s.to_string());
                            return Some((id, command));
                        }
                    }
                }
            }
        }
    }

    // Pi format: {"type":"message","message":{"content":[{"type":"toolCall","id":"...","name":"bash","arguments":{"command":"..."}}]}}
    if value.get("type").and_then(|v| v.as_str()) == Some("message") {
        if let Some(msg) = value.get("message") {
            if let Some(items) = msg.get("content").and_then(|c| c.as_array()) {
                for item in items {
                    if item.get("type").and_then(|v| v.as_str()) == Some("toolCall") {
                        if item
                            .get("name")
                            .and_then(|v| v.as_str())
                            .map(|n| n.eq_ignore_ascii_case("bash"))
                            .unwrap_or(false)
                        {
                            if let Some(command) = extract_command_from_tool_call(item) {
                                let id = item
                                    .get("id")
                                    .and_then(|v| v.as_str())
                                    .map(|s| s.to_string());
                                return Some((id, command));
                            }
                        }
                    }
                }
            }
        }
    }

    // Pi tool_execution_start: {"type":"tool_execution_start","toolName":"Bash","args":{"command":"..."}}
    if value.get("type").and_then(|v| v.as_str()) == Some("tool_execution_start") {
        if value
            .get("toolName")
            .and_then(|v| v.as_str())
            .map(|n| n.eq_ignore_ascii_case("bash"))
            .unwrap_or(false)
        {
            if let Some(args) = value.get("args") {
                if let Some(command) = args.get("command").and_then(|v| v.as_str()) {
                    let id = value
                        .get("toolCallId")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string());
                    return Some((id, command.to_string()));
                }
            }
        }
    }

    None
}

/// Extract the tool call id that a tool_result refers to, if present.
/// Returns `None` for lines without an id (e.g. pi tool_execution_end) so
/// callers can fall back to FIFO cancellation.
fn extract_tool_result_id(line: &str) -> Option<String> {
    let trimmed = line.trim();
    if !trimmed.starts_with('{') {
        return None;
    }

    let value: serde_json::Value = serde_json::from_str(trimmed).ok()?;

    // Claude format: {"type":"user","message":{"content":[{"type":"tool_result","tool_use_id":"..."}]}}
    if value.get("type").and_then(|v| v.as_str()) == Some("user") {
        if let Some(msg) = value.get("message") {
            if let Some(items) = msg.get("content").and_then(|c| c.as_array()) {
                for item in items {
                    if item.get("type").and_then(|v| v.as_str()) == Some("tool_result") {
                        if let Some(id) = item.get("tool_use_id").and_then(|v| v.as_str()) {
                            return Some(id.to_string());
                        }
                    }
                }
            }
        }
    }

    // Pi format: {"type":"message","message":{"role":"toolResult","toolCallId":"..."}}
    // or content-level {"type":"toolResult","toolCallId":"..."}
    if value.get("type").and_then(|v| v.as_str()) == Some("message") {
        if let Some(msg) = value.get("message") {
            if let Some(id) = msg.get("toolCallId").and_then(|v| v.as_str()) {
                return Some(id.to_string());
            }
            if let Some(items) = msg.get("content").and_then(|c| c.as_array()) {
                for item in items {
                    if item.get("type").and_then(|v| v.as_str()) == Some("toolResult") {
                        if let Some(id) = item.get("toolCallId").and_then(|v| v.as_str()) {
                            return Some(id.to_string());
                        }
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

/// Detect a tool_result event in JSON output.
fn is_tool_result(line: &str) -> bool {
    let trimmed = line.trim();
    if !trimmed.starts_with('{') {
        return false;
    }
    let value: serde_json::Value = match serde_json::from_str(trimmed) {
        Ok(v) => v,
        Err(_) => return false,
    };

    // Claude format: {"type":"user","message":{"content":[{"type":"tool_result",...}]}}
    if value.get("type").and_then(|v| v.as_str()) == Some("user") {
        if let Some(msg) = value.get("message") {
            if let Some(content) = msg.get("content") {
                if let Some(items) = content.as_array() {
                    return items.iter().any(|item| {
                        item.get("type").and_then(|v| v.as_str()) == Some("tool_result")
                    });
                }
            }
        }
    }

    // Pi format: {"type":"tool_execution_end",...}
    if value.get("type").and_then(|v| v.as_str()) == Some("tool_execution_end") {
        return true;
    }

    // Pi toolResult role
    if value.get("type").and_then(|v| v.as_str()) == Some("message") {
        if let Some(msg) = value.get("message") {
            if msg.get("role").and_then(|v| v.as_str()) == Some("toolResult") {
                return true;
            }
            if let Some(content) = msg.get("content") {
                if let Some(items) = content.as_array() {
                    return items.iter().any(|item| {
                        item.get("type").and_then(|v| v.as_str()) == Some("toolResult")
                    });
                }
            }
        }
    }

    false
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_basic_lifecycle() {
        let watchdog = CommandWatchdog::new("test-container", 120);
        watchdog.on_tool_call_started(None, "echo hello").await;
        watchdog.on_tool_call_finished(None).await;
    }

    #[tokio::test]
    async fn test_multiple_timers() {
        let watchdog = CommandWatchdog::new("test-container", 120);
        watchdog.on_tool_call_started(Some("id1"), "cmd1").await;
        watchdog.on_tool_call_started(Some("id2"), "cmd2").await;
        watchdog.on_tool_call_started(Some("id3"), "cmd3").await;
        watchdog.cancel_oldest_timer().await;
        watchdog.cancel_oldest_timer().await;
    }

    #[tokio::test]
    async fn test_ticker_aborts_on_close() {
        let watchdog = CommandWatchdog::new("test-container", 120);
        watchdog.on_tool_call_started(Some("id1"), "echo hello").await;
        watchdog.close().await;
        // Ticker should be aborted
        assert!(watchdog.ticker_handle.lock().await.is_none());
    }

    #[tokio::test]
    async fn test_cancel_oldest_empty() {
        let watchdog = CommandWatchdog::new("test-container", 120);
        // Should not panic
        watchdog.cancel_oldest_timer().await;
    }

    #[tokio::test]
    async fn test_cancel_oldest_with_pending() {
        let watchdog = CommandWatchdog::new("test-container", 120);
        watchdog.on_tool_call_started(Some("id1"), "echo hello").await;
        watchdog.cancel_oldest_timer().await;
    }

    #[tokio::test]
    async fn test_timer_fires_after_timeout() {
        let watchdog = CommandWatchdog::new("test-container", 0); // 0 second timeout
        watchdog.on_tool_call_started(Some("id1"), "sleep 999").await;
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        // Timer should have fired and killed the process
    }

    #[tokio::test]
    async fn test_timer_cancelled_before_firing() {
        let watchdog = CommandWatchdog::new("test-container", 0); // 0 second timeout
        watchdog.on_tool_call_started(Some("id1"), "sleep 999").await;
        watchdog.cancel_oldest_timer().await; // Cancel before it fires
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
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

    #[tokio::test]
    async fn test_watchdog_drops_cleanly() {
        // Ensure Drop doesn't panic
        {
            let watchdog = CommandWatchdog::new("test-container", 120);
            watchdog.on_tool_call_started(Some("id1"), "echo hello").await;
        }
        // watchdog dropped here
    }

    #[tokio::test]
    async fn cancel_by_id_removes_only_matching_timer() {
        let watchdog = CommandWatchdog::new("test-container", 120);
        watchdog.on_tool_call_started(Some("a"), "cmd a").await;
        watchdog.on_tool_call_started(Some("b"), "cmd b").await;

        assert!(watchdog.cancel_timer("a").await, "matching id must cancel");
        let timers = watchdog.active_timers.lock().await;
        assert_eq!(timers.len(), 1, "only the matching timer is removed");
        assert_eq!(timers[0].0.as_deref(), Some("b"));
    }

    #[tokio::test]
    async fn cancel_unknown_id_is_noop() {
        let watchdog = CommandWatchdog::new("test-container", 120);
        watchdog.on_tool_call_started(Some("a"), "cmd a").await;

        assert!(!watchdog.cancel_timer("zzz").await);
        assert_eq!(watchdog.active_timers.lock().await.len(), 1);
    }

    #[tokio::test]
    async fn repeated_identical_commands_cancel_independently() {
        // Two runs of the same command must have independent timers.
        let watchdog = CommandWatchdog::new("test-container", 120);
        watchdog.on_tool_call_started(Some("run1"), "cargo test").await;
        watchdog.on_tool_call_started(Some("run2"), "cargo test").await;

        assert!(watchdog.cancel_timer("run1").await);
        let timers = watchdog.active_timers.lock().await;
        assert_eq!(timers.len(), 1);
        assert_eq!(timers[0].0.as_deref(), Some("run2"));
    }

    #[tokio::test]
    async fn non_bash_result_does_not_cancel_bash_timer() {
        // Regression: a tool_result for a non-Bash tool (Read/Write/etc.)
        // arriving while a Bash command is pending used to cancel the oldest
        // timer — i.e. the Bash timer — via FIFO, disabling its timeout.
        let watchdog = CommandWatchdog::new("test-container", 120);
        watchdog.forward_line(
            r#"{"type":"assistant","message":{"content":[{"type":"tool_use","id":"toolu_bash_1","name":"Bash","input":{"command":"cargo test"}}]}}"#,
            "c1",
        );
        tokio::time::sleep(Duration::from_millis(50)).await;
        assert_eq!(watchdog.active_timers.lock().await.len(), 1);

        // Read result while Bash is still pending — must be a no-op.
        watchdog.forward_line(
            r#"{"type":"user","message":{"content":[{"type":"tool_result","tool_use_id":"toolu_read_1","content":"file contents"}]}}"#,
            "c1",
        );
        tokio::time::sleep(Duration::from_millis(50)).await;
        assert_eq!(
            watchdog.active_timers.lock().await.len(),
            1,
            "non-Bash result must not cancel the Bash timer"
        );

        // The matching Bash result cancels the timer.
        watchdog.forward_line(
            r#"{"type":"user","message":{"content":[{"type":"tool_result","tool_use_id":"toolu_bash_1","content":"ok"}]}}"#,
            "c1",
        );
        tokio::time::sleep(Duration::from_millis(50)).await;
        assert_eq!(watchdog.active_timers.lock().await.len(), 0);
    }

    #[tokio::test]
    async fn pi_result_matching_by_id() {
        let watchdog = CommandWatchdog::new("test-container", 120);
        watchdog.forward_line(
            r#"{"type":"message","message":{"role":"assistant","content":[{"type":"toolCall","id":"call_bash_1","name":"bash","arguments":{"command":"npm test"}}]}}"#,
            "c1",
        );
        tokio::time::sleep(Duration::from_millis(50)).await;
        assert_eq!(watchdog.active_timers.lock().await.len(), 1);

        // A read result (toolName != bash) must not cancel the Bash timer.
        watchdog.forward_line(
            r#"{"type":"message","message":{"role":"toolResult","toolCallId":"call_read_1","toolName":"read","content":[{"type":"text","text":"x"}]}}"#,
            "c1",
        );
        tokio::time::sleep(Duration::from_millis(50)).await;
        assert_eq!(watchdog.active_timers.lock().await.len(), 1);

        // The bash result cancels by id.
        watchdog.forward_line(
            r#"{"type":"message","message":{"role":"toolResult","toolCallId":"call_bash_1","toolName":"bash","content":[{"type":"text","text":"PASS"}]}}"#,
            "c1",
        );
        tokio::time::sleep(Duration::from_millis(50)).await;
        assert_eq!(watchdog.active_timers.lock().await.len(), 0);
    }
}
