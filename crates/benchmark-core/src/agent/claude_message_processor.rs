//! ClaudeMessageProcessor - parses Claude Code stream-json output
//! Logs key events with tracing and forwards formatted output to consumer for web UI streaming.

use serde_json::Value;
use tracing::{info, debug};

/// Callback type for sending processed output to the web UI SSE stream.
pub type OutputConsumer = Box<dyn Fn(&str) + Send + Sync>;

/// Processes Claude Code's stream-json output.
/// Parses JSON events, logs key information with tracing, and forwards
/// formatted output to the consumer for real-time web UI display.
pub struct ClaudeMessageProcessor {
    consumer: Option<OutputConsumer>,
}

impl ClaudeMessageProcessor {
    /// Create a new processor with an optional output consumer.
    pub fn new(consumer: Option<OutputConsumer>) -> Self {
        Self { consumer }
    }

    /// Process a single line of output from Claude Code.
    pub fn process(&self, line: &str) {
        // Try to parse as JSON event
        if let Ok(json) = serde_json::from_str::<Value>(line) {
            if let Some(msg_type) = json.get("type").and_then(|v| v.as_str()) {
                match msg_type {
                    "stream_event" => self.handle_stream_event(&json),
                    "assistant" => self.handle_assistant(&json),
                    "user" => self.handle_user_message(&json),
                    "system" => self.handle_system(&json),
                    "result" => self.handle_result(&json),
                    _ => self.fallback(line),
                }
            } else {
                self.fallback(line);
            }
        } else {
            // Not JSON - likely a warning or raw output from Claude
            self.fallback(line);
        }
    }

    fn handle_stream_event(&self, json: &Value) {
        if let Some(event) = json.get("event").and_then(|v| v.as_object()) {
            if let Some(event_type) = event.get("type").and_then(|v| v.as_str()) {
                match event_type {
                    "message_start" => self.handle_message_start(event),
                    "message_delta" => {
                        // message_delta with stop_reason - not much to log
                    }
                    "content_block_delta" => self.handle_content_block_delta(event),
                    "content_block_start" => {} // ignore
                    "content_block_stop" => {}  // ignore
                    "message_stop" => self.send_output("\n"),
                    _ => self.send_output(&json.to_string()),
                }
            } else {
                self.send_output(&json.to_string());
            }
        } else {
            self.send_output(&json.to_string());
        }
    }

    fn handle_message_start(&self, event: &serde_json::Map<String, Value>) {
        if let Some(message) = event.get("message").and_then(|v| v.as_object()) {
            if let Some(content) = message.get("content") {
                if content.is_array() {
                    if let Some(items) = content.as_array() {
                        for item in items {
                            if let Some(item_type) = item.get("type").and_then(|v| v.as_str()) {
                                match item_type {
                                    "thinking" => {
                                        if let Some(thinking) = item.get("thinking").and_then(|v| v.as_str()) {
                                            self.send_output(thinking);
                                        }
                                    }
                                    "text" => {
                                        if let Some(text) = item.get("text").and_then(|v| v.as_str()) {
                                            self.send_output(text);
                                        }
                                    }
                                    _ => {}
                                }
                            }
                        }
                    }
                } else if let Some(content_type) = content.get("type").and_then(|v| v.as_str()) {
                    match content_type {
                        "thinking" => {
                            if let Some(thinking) = content.get("thinking").and_then(|v| v.as_str()) {
                                self.send_output(thinking);
                            }
                        }
                        "text" => {
                            if let Some(text) = content.get("text").and_then(|v| v.as_str()) {
                                self.send_output(text);
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
    }

    fn handle_content_block_delta(&self, event: &serde_json::Map<String, Value>) {
        if let Some(delta) = event.get("delta").and_then(|v| v.as_object()) {
            if let Some(delta_type) = delta.get("type").and_then(|v| v.as_str()) {
                match delta_type {
                    "thinking_delta" => {
                        if let Some(thinking) = delta.get("thinking").and_then(|v| v.as_str()) {
                            self.send_output(thinking);
                        }
                    }
                    "text_delta" => {
                        if let Some(text) = delta.get("text").and_then(|v| v.as_str()) {
                            self.send_output(text);
                        }
                    }
                    "input_json_delta" => {
                        // Partial JSON for tool calls - skip for cleaner output
                    }
                    "signature_delta" => {
                        // Skip signature deltas
                    }
                    _ => {}
                }
            }
        }
    }

    fn handle_assistant(&self, json: &Value) {
        if let Some(message) = json.get("message").and_then(|v| v.as_object()) {
            if let Some(content) = message.get("content") {
                if content.is_array() {
                    if let Some(items) = content.as_array() {
                        for item in items {
                            if let Some(item_type) = item.get("type").and_then(|v| v.as_str()) {
                                match item_type {
                                    "thinking" => {
                                        if let Some(thinking) = item.get("thinking").and_then(|v| v.as_str()) {
                                            self.send_output(thinking);
                                        }
                                    }
                                    "tool_use" => self.render_tool_use(item),
                                    "text" => {
                                        if let Some(text) = item.get("text").and_then(|v| v.as_str()) {
                                            self.send_output(text);
                                        }
                                    }
                                    _ => {}
                                }
                            }
                        }
                    }
                } else if let Some(content_type) = content.get("type").and_then(|v| v.as_str()) {
                    match content_type {
                        "thinking" => {
                            if let Some(thinking) = content.get("thinking").and_then(|v| v.as_str()) {
                                self.send_output(thinking);
                            }
                        }
                        "tool_use" => self.render_tool_use(content),
                        "text" => {
                            if let Some(text) = content.get("text").and_then(|v| v.as_str()) {
                                self.send_output(text);
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
    }

    fn handle_user_message(&self, json: &Value) {
        if let Some(message) = json.get("message").and_then(|v| v.as_object()) {
            if let Some(content) = message.get("content") {
                if content.is_array() {
                    if let Some(items) = content.as_array() {
                        for item in items {
                            if let Some(item_type) = item.get("type").and_then(|v| v.as_str()) {
                                match item_type {
                                    "text" => {
                                        if let Some(text) = item.get("text").and_then(|v| v.as_str()) {
                                            self.send_output(&format!("[User]: {}\n", text));
                                        }
                                    }
                                    "tool_result" => {
                                        if let Some(tool_content) = item.get("content").and_then(|v| v.as_str()) {
                                            let with_newlines = tool_content.replace("\\n", "\n");
                                            info!("[Tool result] {}", crate::safe_truncate(&with_newlines, 500));
                                            self.send_output(&format!("tool_result:\n{}\n", with_newlines));
                                        } else {
                                            self.send_output(&format!("tool_result: {}\n", item));
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
                                self.send_output(&format!("[User]: {}\n", text));
                            }
                        }
                        "tool_result" => {
                            if let Some(tool_content) = content.get("content").and_then(|v| v.as_str()) {
                                let with_newlines = tool_content.replace("\\n", "\n");
                                info!("[Tool result] {}", crate::safe_truncate(&with_newlines, 500));
                                self.send_output(&format!("tool_result:\n{}\n", with_newlines));
                            } else {
                                self.send_output(&format!("tool_result: {}\n", content));
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
    }

    fn handle_system(&self, _json: &Value) {
        // System messages - log at debug level
        debug!("Received system message from Claude Code");
    }

    fn handle_result(&self, json: &Value) {
        // Result event contains session summary: cost, tokens, turns, etc.
        let cost = json.get("total_cost_usd").and_then(|v| v.as_f64()).unwrap_or(0.0);
        let turns = json.get("num_turns").and_then(|v| v.as_i64()).unwrap_or(0);
        info!("[Claude Code result] Total cost: ${:.4}, Total turns: {}", cost, turns);
        self.send_output(&format!("[Result] cost=${:.4}, turns={}\n", cost, turns));
    }

    fn render_tool_use(&self, item: &Value) {
        if let Some(name) = item.get("name").and_then(|v| v.as_str()) {
            match name {
                "Edit" => {
                    if let Some(input) = item.get("input").and_then(|v| v.as_object()) {
                        let file_path = input.get("file_path").and_then(|v| v.as_str()).unwrap_or("");
                        let old_string = input.get("old_string").and_then(|v| v.as_str()).unwrap_or("");
                        let new_string = input.get("new_string").and_then(|v| v.as_str()).unwrap_or("");
                        info!("[Tool: Edit] {}", file_path);
                        let old_normalized = old_string.replace("\\n", "\n");
                        let new_normalized = new_string.replace("\\n", "\n");
                        self.send_output(&format!("Edit {}\nOld\n{}\nNew\n{}\n", file_path, old_normalized, new_normalized));
                    }
                }
                "Glob" => {
                    if let Some(input) = item.get("input").and_then(|v| v.as_object()) {
                        if let Some(pattern) = input.get("pattern").and_then(|v| v.as_str()) {
                            info!("[Tool: Glob] pattern={}", pattern);
                            self.send_output(&format!("\ntool_use: Glob {}\n", pattern));
                        }
                    }
                }
                "Grep" => {
                    if let Some(input) = item.get("input").and_then(|v| v.as_object()) {
                        let pattern = input.get("pattern").and_then(|v| v.as_str()).unwrap_or("");
                        let file_path = input.get("file_path").and_then(|v| v.as_str()).unwrap_or("");
                        info!("[Tool: Grep] pattern={} file={}", pattern, file_path);
                        self.send_output(&format!("\ntool_use: Grep {} in {}\n", pattern, file_path));
                    }
                }
                "Read" => {
                    if let Some(input) = item.get("input").and_then(|v| v.as_object()) {
                        if let Some(file_path) = input.get("file_path").and_then(|v| v.as_str()) {
                            info!("[Tool: Read] {}", file_path);
                            self.send_output(&format!("\ntool_use: Read {}\n", file_path));
                        }
                    }
                }
                "Write" => {
                    if let Some(input) = item.get("input").and_then(|v| v.as_object()) {
                        let file_path = input.get("file_path").and_then(|v| v.as_str()).unwrap_or("");
                        let content = input.get("content").and_then(|v| v.as_str()).unwrap_or("");
                        info!("[Tool: Write] {}", file_path);
                        let normalized = content.replace("\\n", "\n");
                        self.send_output(&format!("\ntool_use: Write {}\nContent:\n{}\n", file_path, normalized));
                    }
                }
                "Bash" => {
                    if let Some(input) = item.get("input").and_then(|v| v.as_object()) {
                        let run_in_background = input.get("run_in_background").and_then(|v| v.as_bool()).unwrap_or(false);
                        let description = input.get("description").and_then(|v| v.as_str()).unwrap_or("");
                        let command = input.get("command").and_then(|v| v.as_str()).unwrap_or("");
                        let bg = if run_in_background { " (in background)" } else { "" };
                        info!("[Tool: Bash{}] {} - {}", bg, description, command);
                        self.send_output(&format!("\ntool_use: Bash{} {}\nCommand: {}\n", bg, description, command));
                    }
                }
                "Task" => {
                    if let Some(input) = item.get("input").and_then(|v| v.as_object()) {
                        let prompt = input.get("prompt").and_then(|v| v.as_str()).unwrap_or("");
                        info!("[Tool: Task] {}", crate::safe_truncate(&prompt, 200));
                        self.send_output(&format!("\ntool_use: Task {}\n", crate::safe_truncate(&prompt, 200)));
                    }
                }
                "TodoWrite" => {
                    info!("[Tool: TodoWrite]");
                    self.send_output("\ntool_use: TodoWrite\n");
                    if let Some(input) = item.get("input").and_then(|v| v.as_object()) {
                        if let Some(todos) = input.get("todos").and_then(|v| v.as_array()) {
                            for todo in todos {
                                let content = todo.get("content").and_then(|v| v.as_str()).unwrap_or("");
                                let status = todo.get("status").and_then(|v| v.as_str()).unwrap_or("");
                                match status {
                                    "in_progress" => self.send_output(&format!("[⟳] {}\n", content)),
                                    "pending" => self.send_output(&format!("[⌛] {}\n", content)),
                                    "completed" => self.send_output(&format!("[✅] {}\n", content)),
                                    _ => self.send_output(&format!("[ ] {}\n", content)),
                                }
                            }
                        }
                    }
                }
                _ => {
                    debug!("[Tool: {}] args={}", name, item.get("input").map(|i| i.to_string()).unwrap_or_default());
                    self.send_output(&format!("\ntool_use: {}\n", name));
                }
            }
        }
    }

    fn fallback(&self, line: &str) {
        // Not a recognized JSON event - print as-is (warnings, errors, etc.)
        info!("[Claude output] {}", line);
        self.send_output(line);
    }

    fn send_output(&self, text: &str) {
        if let Some(ref consumer) = self.consumer {
            consumer(text);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Regression: tool results / prompts containing multi-byte UTF-8 used to
    /// panic the byte-slice truncation in the log path. With a subscriber at
    /// INFO the format args are evaluated, so this must not panic.
    fn with_info_subscriber(f: impl FnOnce()) {
        let _guard = tracing::subscriber::set_default(
            tracing_subscriber::fmt()
                .with_max_level(tracing::Level::INFO)
                .with_writer(std::io::sink)
                .finish(),
        );
        f();
    }

    #[test]
    fn non_ascii_tool_result_does_not_panic() {
        with_info_subscriber(|| {
            let processor = ClaudeMessageProcessor::new(None);
            // >500 bytes, multi-byte chars straddling the 500-byte limit
            let json = format!(
                r#"{{"type":"user","message":{{"content":[{{"type":"tool_result","tool_use_id":"t1","content":"{}"}}]}}}}"#,
                "€".repeat(167)
            );
            processor.process(&json);
        });
    }

    #[test]
    fn non_ascii_task_prompt_does_not_panic() {
        with_info_subscriber(|| {
            let processor = ClaudeMessageProcessor::new(None);
            // >200 bytes, multi-byte chars straddling the 200-byte limit
            let json = format!(
                r#"{{"type":"assistant","message":{{"content":[{{"type":"tool_use","name":"Task","input":{{"prompt":"{}"}}}}]}}}}"#,
                "€".repeat(67)
            );
            processor.process(&json);
        });
    }
}
