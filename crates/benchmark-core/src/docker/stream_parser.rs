use tracing::debug;

/// Wraps the output stream from a Docker container and parses JSON events to
/// detect Bash tool call boundaries. When a Bash tool call starts, it notifies
/// a [`CommandWatchdog`] to start a per-command timer. When the tool call
/// finishes, it notifies the watchdog to cancel the timer.
///
/// This class is designed to work with both Claude Code's stream-json output
/// format and Pi's JSON event stream format.
pub struct StreamParser {
    downstream: Option<std::sync::Arc<dyn Fn(&str) + Send + Sync + 'static>>,
    watchdog: Option<std::sync::Arc<crate::docker::watchdog::CommandWatchdog>>,
}

impl StreamParser {
    /// Creates a new StreamParser without watchdog support.
    ///
    /// # Arguments
    /// * `downstream` - The original output consumer (e.g., a logging callback).
    pub fn new(
        downstream: std::sync::Arc<dyn Fn(&str) + Send + Sync + 'static>,
    ) -> Self {
        Self {
            downstream: Some(downstream),
            watchdog: None,
        }
    }

    /// Creates a new StreamParser with watchdog integration.
    pub fn new_with_watchdog(
        downstream: std::sync::Arc<dyn Fn(&str) + Send + Sync + 'static>,
        watchdog: std::sync::Arc<crate::docker::watchdog::CommandWatchdog>,
    ) -> Self {
        Self {
            downstream: Some(downstream),
            watchdog: Some(watchdog),
        }
    }

    /// Process a single output line.
    ///
    /// This method:
    /// 1. Forwards the line to the downstream consumer.
    /// 2. Parses the line as JSON to detect tool call boundaries.
    /// 3. Notifies the watchdog when Bash tool calls start or finish.
    pub fn accept(&self, line: &str) {
        // Forward to downstream consumer first
        if let Some(ref downstream) = self.downstream {
            downstream(line);
        }

        // Only parse non-empty lines that look like JSON
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.chars().next() != Some('{') {
            return;
        }

        // Parse and detect tool call boundaries
        if let Ok(root) = serde_json::from_str::<serde_json::Value>(trimmed) {
            let wd = self.watchdog.as_ref();

            // Claude Code format
            parse_claude_format(&root, wd);

            // Pi agent format
            parse_pi_format(&root, wd);

            // Pi tool_execution_start / tool_execution_end
            parse_pi_tool_execution_events(&root, wd);
        }
        // If parsing fails, silently ignore — the downstream consumer
        // already received the raw line.
    }
}



/// Detects Claude Code tool_use (Bash) and tool_result events in the
/// assistant/user message format.
    fn parse_claude_format(root: &serde_json::Value, watchdog: Option<&std::sync::Arc<crate::docker::watchdog::CommandWatchdog>>) {
    let type_str = root.get("type").and_then(|v| v.as_str());

    // Assistant message with tool_use
    if type_str == Some("assistant") {
        if let Some(message) = root.get("message") {
            if let Some(content) = message.get("content") {
                if let Some(items) = content.as_array() {
                    for item in items {
                        if item.get("type").and_then(|v| v.as_str()) == Some("tool_use") {
                            if item.get("name").and_then(|v| v.as_str()) == Some("Bash") {
                                if let Some(command) = extract_command(item) {
                                        debug!(
                                            "Claude Bash tool call started: {}",
                                            crate::safe_truncate(&command, 100)
                                        );
                                    // Notify watchdog of tool call start (sync for FIFO ordering)
                                    if let Some(wd) = watchdog {
                                        wd.on_tool_call_started_sync(&command);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // User message with tool_result (signals the end of a tool call)
    if type_str == Some("user") {
        if let Some(message) = root.get("message") {
            if let Some(content) = message.get("content") {
                if let Some(items) = content.as_array() {
                    for item in items {
                        if item.get("type").and_then(|v| v.as_str()) == Some("tool_result") {
                            // Cancel oldest pending timer (FIFO ordering, sync for correctness)
                            if let Some(wd) = watchdog {
                                wd.cancel_oldest_timer_sync();
                            }
                        }
                    }
                }
            }
        }
    }
}

/// Detects Pi agent toolCall events in the message format.
    fn parse_pi_format(root: &serde_json::Value, watchdog: Option<&std::sync::Arc<crate::docker::watchdog::CommandWatchdog>>) {
    let type_str = root.get("type").and_then(|v| v.as_str());

    if type_str != Some("message") {
        return;
    }

    let Some(message) = root.get("message") else {
        return;
    };

    let Some(content) = message.get("content") else {
        return;
    };

    // Handle array or single content
    let items: Vec<_> = if content.is_array() {
        content.as_array().map(|a| a.to_vec()).unwrap_or_else(|| vec![content.clone()])
    } else {
        vec![content.clone()]
    };

    for item in &items {
        if !item.is_object() {
            continue;
        }

        // toolCall inside assistant message
        if item.get("type").and_then(|v| v.as_str()) == Some("toolCall") {
            if item
                .get("name")
                .and_then(|v| v.as_str())
                .map(|n| n.eq_ignore_ascii_case("bash"))
                .unwrap_or(false)
            {
                if let Some(args) = item.get("arguments") {
                    if let Some(command) = args.get("command").and_then(|v| v.as_str()) {
                        debug!(
                            "Pi bash tool call started: {}",
                            crate::safe_truncate(&command, 100)
                        );
                        if let Some(wd) = watchdog {
                            wd.on_tool_call_started_sync(command);
                        }
                    }
                }
            }
        }

        // toolResult inside assistant message
        if item.get("type").and_then(|v| v.as_str()) == Some("toolResult") {
            if let Some(wd) = watchdog {
                wd.cancel_oldest_timer_sync();
            }
        }
    }

    // Also check for toolResult role directly
    if message
        .get("role")
        .and_then(|v| v.as_str())
        == Some("toolResult")
    {
        if let Some(wd) = watchdog {
            wd.cancel_oldest_timer_sync();
        }
    }
}

/// Detects Pi's explicit tool_execution_start / tool_execution_end events.
/// These are top-level event types, not nested in messages.
fn parse_pi_tool_execution_events(root: &serde_json::Value, watchdog: Option<&std::sync::Arc<crate::docker::watchdog::CommandWatchdog>>) {
    let type_str = root.get("type").and_then(|v| v.as_str());

    if type_str == Some("tool_execution_start") {
        if let Some(tool_name) = root.get("toolName").and_then(|v| v.as_str()) {
            if tool_name.eq_ignore_ascii_case("bash") {
                if let Some(args) = root.get("args") {
                    if let Some(command) = args.get("command").and_then(|v| v.as_str()) {
                        debug!(
                            "Pi tool_execution_start (Bash): {}",
                            crate::safe_truncate(&command, 100)
                        );
                        if let Some(wd) = watchdog {
                            wd.on_tool_call_started_sync(command);
                        }
                    }
                }
            }
        }
    }

    if type_str == Some("tool_execution_end") {
        if let Some(tool_name) = root.get("toolName").and_then(|v| v.as_str()) {
            if tool_name.eq_ignore_ascii_case("bash") {
                if let Some(wd) = watchdog {
                    wd.cancel_oldest_timer_sync();
                }
            }
        }
    }
}

/// Extracts the command string from a tool_use node.
/// Handles both Claude Code's "input.command" and Pi's "arguments.command".
fn extract_command(tool_use_node: &serde_json::Value) -> Option<String> {
    let input = tool_use_node
        .get("input")
        .or_else(|| tool_use_node.get("arguments"))?;
    input.get("command").and_then(|v| v.as_str()).map(|s| s.to_string())
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn create_parser() -> StreamParser {
        let downstream = std::sync::Arc::new(|_s: &str| {});
        StreamParser::new(downstream)
    }

    #[test]
    fn test_detects_claude_bash_tool_call() {
        let json = r#"{"type":"assistant","message":{"content":[{"type":"tool_use","name":"Bash","input":{"command":"cd /workspace && ./gradlew test --no-daemon"}}]}}"#;
        let parser = create_parser();
        // Should not panic
        parser.accept(json);
    }

    #[test]
    fn test_detects_claude_bash_tool_call_with_long_command() {
        let json = r#"{"type":"assistant","message":{"content":[{"type":"tool_use","name":"Bash","input":{"command":"cd /workspace && timeout 60 ./gradlew test --no-daemon -q 2>&1 | tee /tmp/test.log"}}]}}"#;
        let parser = create_parser();
        parser.accept(json);
    }

    #[test]
    fn test_detects_claude_tool_result() {
        let json = r#"{"type":"user","message":{"content":[{"type":"tool_result","tool_use_id":"abc123","content":"BUILD SUCCESSFUL"}]}}"#;
        let parser = create_parser();
        parser.accept(json);
    }

    #[test]
    fn test_ignores_non_bash_tool_calls() {
        let json = r#"{"type":"assistant","message":{"content":[{"type":"tool_use","name":"Read","input":{"file_path":"/workspace/src/Main.java"}}]}}"#;
        let parser = create_parser();
        parser.accept(json);
    }

    #[test]
    fn test_ignores_non_json_lines() {
        let parser = create_parser();
        parser.accept("this is not json");
    }

    #[test]
    fn test_ignores_empty_lines() {
        let parser = create_parser();
        parser.accept("");
    }

    #[test]
    fn test_detects_pi_bash_tool_call() {
        let json = r#"{"type":"message","message":{"role":"assistant","content":[{"type":"toolCall","name":"bash","arguments":{"command":"cd /workspace && go test ./..."}}]}}"#;
        let parser = create_parser();
        parser.accept(json);
    }

    #[test]
    fn test_detects_pi_tool_result_role() {
        let json = r#"{"type":"message","message":{"role":"toolResult","content":[{"type":"text","text":"PASS"}]}}"#;
        let parser = create_parser();
        parser.accept(json);
    }

    #[test]
    fn test_detects_pi_tool_execution_start() {
        let json = r#"{"type":"tool_execution_start","toolName":"Bash","args":{"command":"cd /workspace && npm test"}}"#;
        let parser = create_parser();
        parser.accept(json);
    }

    #[test]
    fn test_detects_pi_tool_execution_end() {
        let json = r#"{"type":"tool_execution_end","toolName":"Bash"}"#;
        let parser = create_parser();
        parser.accept(json);
    }

    #[test]
    fn test_handles_malformed_json_gracefully() {
        let parser = create_parser();
        parser.accept("{invalid json");
    }

    #[test]
    fn test_handles_json_without_type_field() {
        let parser = create_parser();
        parser.accept(r#"{"foo":"bar"}"#);
    }

    #[test]
    fn test_passes_through_output_to_downstream() {
        let captured = std::sync::Arc::new(std::sync::Mutex::new(String::new()));
        let captured_clone = captured.clone();
        let downstream = std::sync::Arc::new(move |s: &str| {
            let mut c = captured_clone.lock().unwrap();
            *c = s.to_string();
        });
        let parser = StreamParser::new(downstream);

        parser.accept("line1");
        parser.accept("line2");

        let result = captured.lock().unwrap();
        assert_eq!(*result, "line2");
    }

    #[test]
    fn test_ignores_claude_text_content() {
        let json = r#"{"type":"assistant","message":{"content":[{"type":"text","text":"Let me check that for you"}]}}"#;
        let parser = create_parser();
        parser.accept(json);
    }

    #[test]
    fn test_ignores_claude_thinking_content() {
        let json = r#"{"type":"assistant","message":{"content":[{"type":"thinking","thinking":"I should run the tests"}]}}"#;
        let parser = create_parser();
        parser.accept(json);
    }

    #[test]
    fn test_detects_claude_glob_tool_call() {
        let json = r#"{"type":"assistant","message":{"content":[{"type":"tool_use","name":"Glob","input":{"pattern":"**/*.java"}}]}}"#;
        let parser = create_parser();
        parser.accept(json);
    }

    #[test]
    fn test_detects_claude_write_tool_call() {
        let json = r#"{"type":"assistant","message":{"content":[{"type":"tool_use","name":"Write","input":{"file_path":"/workspace/src/Main.java","content":"public class Main {}"}}]}}"#;
        let parser = create_parser();
        parser.accept(json);
    }

    #[test]
    fn test_handles_claude_content_as_object() {
        let json = r#"{"type":"assistant","message":{"content":{"type":"tool_use","name":"Bash","input":{"command":"echo hi"}}}}"#;
        let parser = create_parser();
        parser.accept(json);
    }

    #[test]
    fn test_handles_pi_content_as_object() {
        let json = r#"{"type":"message","message":{"role":"assistant","content":{"type":"toolCall","name":"bash","arguments":{"command":"echo hi"}}}}"#;
        let parser = create_parser();
        parser.accept(json);
    }

    #[test]
    fn test_extract_command_claude_format() {
        let node = serde_json::json!({
            "type": "tool_use",
            "name": "Bash",
            "input": {"command": "echo hello"}
        });
        let cmd = extract_command(&node);
        assert_eq!(cmd, Some("echo hello".to_string()));
    }

    #[test]
    fn test_extract_command_pi_format() {
        let node = serde_json::json!({
            "type": "toolCall",
            "name": "bash",
            "arguments": {"command": "echo hello"}
        });
        let cmd = extract_command(&node);
        assert_eq!(cmd, Some("echo hello".to_string()));
    }

    #[test]
    fn test_extract_command_missing() {
        let node = serde_json::json!({
            "type": "tool_use",
            "name": "Read"
        });
        let cmd = extract_command(&node);
        assert!(cmd.is_none());
    }
}
