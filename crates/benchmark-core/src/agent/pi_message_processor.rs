//! PiMessageProcessor - parses Pi agent JSON event stream output
//! Logs key events with tracing and forwards formatted output to consumer for web UI streaming.

use serde_json::Value;
use tracing::{info, debug, warn};

/// Callback type for sending processed output to the web UI SSE stream.
pub type OutputConsumer = Box<dyn Fn(&str) + Send + Sync>;

/// Processes Pi agent's JSON event stream output.
/// Parses structured events and logs key information with tracing.
pub struct PiMessageProcessor {
    consumer: Option<OutputConsumer>,
}

impl PiMessageProcessor {
    /// Create a new processor with an optional output consumer.
    pub fn new(consumer: Option<OutputConsumer>) -> Self {
        Self { consumer }
    }

    /// Process a single line of output from Pi agent.
    pub fn process(&self, line: &str) {
        if let Ok(json) = serde_json::from_str::<Value>(line) {
            if let Some(event_type) = json.get("type").and_then(|v| v.as_str()) {
                match event_type {
                    "session" => self.handle_session(&json),
                    "agent_start" => {
                        info!("[Pi] Agent starting...");
                        self.send_output("\n[Agent starting...]\n");
                    }
                    "agent_end" => {
                        info!("[Pi] Agent finished");
                        self.send_output("\n[Agent finished]\n");
                    }
                    "turn_start" => {
                        info!("[Pi] Turn start");
                        self.send_output("\n--- Turn Start ---\n");
                    }
                    "turn_end" => self.send_output("--- Turn End ---\n"),
                    "message_start" => self.handle_message_start(&json),
                    "message_update" => self.handle_message_update(&json),
                    "message_end" => self.send_output("\n"),
                    "tool_execution_start" => self.handle_tool_execution_start(&json),
                    "tool_execution_update" => self.handle_tool_execution_update(&json),
                    "tool_execution_end" => self.handle_tool_execution_end(&json),
                    "auto_compaction_start" => self.handle_auto_compaction_start(&json),
                    "auto_compaction_end" => self.handle_auto_compaction_end(&json),
                    "auto_retry_start" => self.handle_auto_retry_start(&json),
                    "auto_retry_end" => self.handle_auto_retry_end(&json),
                    "model_change" => self.handle_model_change(&json),
                    "thinking_level_change" => self.handle_thinking_level_change(&json),
                    "compaction" => self.handle_compaction(&json),
                    "branch_summary" => self.handle_branch_summary(&json),
                    "custom_message" => self.handle_custom_message(&json),
                    "bashExecution" => self.handle_bash_execution(&json),
                    _ => {
                        debug!("[Pi unknown event] type={}", event_type);
                        self.send_output(line);
                    }
                }
            } else {
                self.send_output(line);
            }
        } else {
            // Not JSON - might be warnings or other output
            info!("[Pi output] {}", line);
            self.send_output(line);
        }
    }

    fn handle_session(&self, json: &Value) {
        let id = json.get("id").and_then(|v| v.as_str()).unwrap_or("unknown");
        let cwd = json.get("cwd").and_then(|v| v.as_str()).unwrap_or(".");
        info!("[Pi Session] ID: {}, CWD: {}", id, cwd);
        self.send_output(&format!("[Session] ID: {}, CWD: {}\n", id, cwd));
    }

    fn handle_message_start(&self, json: &Value) {
        if let Some(message) = json.get("message").and_then(|v| v.as_object()) {
            if let Some(role) = message.get("role").and_then(|v| v.as_str()) {
                match role {
                    "user" => {
                        if let Some(content) = message.get("content") {
                            if content.is_string() {
                                let text = content.as_str().unwrap_or("");
                                info!("[Pi User] {}", &text[..text.len().min(200)]);
                                self.send_output(&format!("\n[User]: {}\n", text));
                            } else if content.is_array() {
                                if let Some(items) = content.as_array() {
                                    for item in items {
                                        if let Some(item_type) = item.get("type").and_then(|v| v.as_str()) {
                                            match item_type {
                                                "text" => {
                                                    if let Some(text) = item.get("text").and_then(|v| v.as_str()) {
                                                        self.send_output(&format!("[User]: {}\n", text));
                                                    }
                                                }
                                                _ => {}
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    "assistant" => {
                        info!("[Pi Assistant]");
                        self.send_output("\n[Assistant]:\n");
                        if let Some(content) = message.get("content") {
                            if content.is_array() {
                                if let Some(items) = content.as_array() {
                                    for item in items {
                                        if let Some(item_type) = item.get("type").and_then(|v| v.as_str()) {
                                            match item_type {
                                                "text" => {
                                                    if let Some(text) = item.get("text").and_then(|v| v.as_str()) {
                                                        self.send_output(text);
                                                    }
                                                }
                                                "thinking" => {
                                                    if let Some(thinking) = item.get("thinking").and_then(|v| v.as_str()) {
                                                        self.send_output(&format!("\n[Thinking]:\n{}\n", thinking));
                                                    }
                                                }
                                                "toolCall" => self.handle_tool_call(item),
                                                _ => {}
                                            }
                                        }
                                    }
                                }
                            } else if content.is_object() {
                                if let Some(item_type) = content.get("type").and_then(|v| v.as_str()) {
                                    match item_type {
                                        "text" => {
                                            if let Some(text) = content.get("text").and_then(|v| v.as_str()) {
                                                self.send_output(text);
                                            }
                                        }
                                        "thinking" => {
                                            if let Some(thinking) = content.get("thinking").and_then(|v| v.as_str()) {
                                                self.send_output(&format!("\n[Thinking]:\n{}\n", thinking));
                                            }
                                        }
                                        "toolCall" => self.handle_tool_call(content),
                                        _ => {}
                                    }
                                }
                            }
                        }
                    }
                    "toolResult" => {
                        if let Some(content) = message.get("content") {
                            if content.is_string() {
                                let text = content.as_str().unwrap_or("");
                                info!("[Pi Tool Result] {}", &text[..text.len().min(500)]);
                                self.send_output(&format!("[Tool Result]: {}\n", text));
                            } else if content.is_array() {
                                if let Some(items) = content.as_array() {
                                    for item in items {
                                        if let Some(item_type) = item.get("type").and_then(|v| v.as_str()) {
                                            match item_type {
                                                "text" => {
                                                    if let Some(text) = item.get("text").and_then(|v| v.as_str()) {
                                                        self.send_output(&format!("[Result]: {}\n", text));
                                                    }
                                                }
                                                _ => {}
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    fn handle_message_update(&self, json: &Value) {
        if let Some(assistant_event) = json.get("assistantMessageEvent").and_then(|v| v.as_object()) {
            if let Some(event_type) = assistant_event.get("type").and_then(|v| v.as_str()) {
                match event_type {
                    "text_delta" => {
                        if let Some(delta) = assistant_event.get("delta").and_then(|v| v.as_str()) {
                            self.send_output(delta);
                        }
                    }
                    "thinking_delta" => {
                        if let Some(delta) = assistant_event.get("delta").and_then(|v| v.as_str()) {
                            self.send_output(delta);
                        }
                    }
                    "tool_call_delta" => {
                        // Tool call in progress - skip for cleaner output
                    }
                    _ => {}
                }
            }
        }
    }

    fn handle_tool_execution_start(&self, json: &Value) {
        let tool_name = json.get("toolName").and_then(|v| v.as_str()).unwrap_or("unknown");
        let tool_call_id = json.get("toolCallId").and_then(|v| v.as_str()).unwrap_or("");
        info!("[Pi Tool: {}] (ID: {})", tool_name, tool_call_id);
        self.send_output(&format!("\n[Tool: {}] (ID: {})\n", tool_name, tool_call_id));

        if let Some(args) = json.get("args") {
            self.print_tool_args(tool_name, args);
        }
    }

    fn handle_tool_execution_update(&self, json: &Value) {
        let tool_name = json.get("toolName").and_then(|v| v.as_str()).unwrap_or("unknown");
        if json.get("partialResult").is_some() {
            info!("[Pi Tool {}] progress...", tool_name);
            self.send_output(&format!("[Tool {} progress]...\n", tool_name));
        }
    }

    fn handle_tool_execution_end(&self, json: &Value) {
        let tool_name = json.get("toolName").and_then(|v| v.as_str()).unwrap_or("unknown");
        let is_error = json.get("isError").and_then(|v| v.as_bool()).unwrap_or(false);
        if is_error {
            warn!("[Pi Tool {}] failed", tool_name);
            self.send_output(&format!("[Tool {} failed]\n", tool_name));
        } else {
            info!("[Pi Tool {}] completed", tool_name);
            self.send_output(&format!("[Tool {} completed]\n", tool_name));
        }
        if let Some(result) = json.get("result") {
            self.print_tool_result(tool_name, result);
        }
    }

    fn handle_tool_call(&self, item: &Value) {
        if let Some(name) = item.get("name").and_then(|v| v.as_str()) {
            info!("[Pi Tool Call: {}]", name);
            self.send_output(&format!("[Tool Call: {}]\n", name));
            if let Some(arguments) = item.get("arguments") {
                self.print_tool_args(name, arguments);
            }
        }
    }

    fn print_tool_args(&self, tool_name: &str, args: &Value) {
        match tool_name {
            "Read" => {
                if let Some(file_path) = args.get("file_path").and_then(|v| v.as_str()) {
                    info!("  Reading: {}", file_path);
                    self.send_output(&format!("  Reading: {}\n", file_path));
                }
            }
            "Write" => {
                if let Some(file_path) = args.get("file_path").and_then(|v| v.as_str()) {
                    info!("  Writing: {}", file_path);
                    self.send_output(&format!("  Writing: {}\n", file_path));
                }
            }
            "Edit" => {
                if let Some(file_path) = args.get("file_path").and_then(|v| v.as_str()) {
                    info!("  Editing: {}", file_path);
                    self.send_output(&format!("  Editing: {}\n", file_path));
                }
            }
            "Bash" => {
                if let Some(command) = args.get("command").and_then(|v| v.as_str()) {
                    info!("  Command: {}", command);
                    self.send_output(&format!("  Command: {}\n", command));
                }
            }
            "Glob" => {
                if let Some(pattern) = args.get("pattern").and_then(|v| v.as_str()) {
                    info!("  Pattern: {}", pattern);
                    self.send_output(&format!("  Pattern: {}\n", pattern));
                }
            }
            "Grep" => {
                if let Some(pattern) = args.get("pattern").and_then(|v| v.as_str()) {
                    if let Some(file_path) = args.get("file_path").and_then(|v| v.as_str()) {
                        info!("  Pattern: {} in {}", pattern, file_path);
                        self.send_output(&format!("  Pattern: {} in {}\n", pattern, file_path));
                    }
                }
            }
            _ => {
                debug!("  Args: {}", args);
                self.send_output(&format!("  Args: {}\n", args));
            }
        }
    }

    fn print_tool_result(&self, _tool_name: &str, result: &Value) {
        if result.is_string() {
            let content = result.as_str().unwrap_or("");
            let output_text = if content.len() > 1000 {
                format!("{}...[truncated]", &content[..1000])
            } else {
                content.to_string()
            };
            info!("  Output: {}", output_text);
            self.send_output(&format!("  Output: {}\n", output_text));
        }
    }

    fn handle_auto_compaction_start(&self, json: &Value) {
        let reason = json.get("reason").and_then(|v| v.as_str()).unwrap_or("unknown");
        info!("[Pi Auto-compaction started: {}]", reason);
        self.send_output(&format!("\n[Auto-compaction started: {}]\n", reason));
    }

    fn handle_auto_compaction_end(&self, json: &Value) {
        let aborted = json.get("aborted").and_then(|v| v.as_bool()).unwrap_or(false);
        let will_retry = json.get("willRetry").and_then(|v| v.as_bool()).unwrap_or(false);
        if aborted {
            info!("[Pi Auto-compaction aborted]");
            self.send_output("[Auto-compaction aborted]\n");
        } else if will_retry {
            info!("[Pi Auto-compaction failed, will retry]");
            self.send_output("[Auto-compaction failed, will retry]\n");
        } else {
            info!("[Pi Auto-compaction completed]");
            self.send_output("[Auto-compaction completed]\n");
        }
    }

    fn handle_auto_retry_start(&self, json: &Value) {
        let attempt = json.get("attempt").and_then(|v| v.as_i64()).unwrap_or(1);
        let max_attempts = json.get("maxAttempts").and_then(|v| v.as_i64()).unwrap_or(3);
        let error_message = json.get("errorMessage").and_then(|v| v.as_str()).unwrap_or("");
        info!("[Pi Auto-retry {}/{}: {}]", attempt, max_attempts, error_message);
        self.send_output(&format!("\n[Auto-retry {}/{}: {}]\n", attempt, max_attempts, error_message));
    }

    fn handle_auto_retry_end(&self, json: &Value) {
        let success = json.get("success").and_then(|v| v.as_bool()).unwrap_or(false);
        let attempt = json.get("attempt").and_then(|v| v.as_i64()).unwrap_or(1);
        if success {
            info!("[Pi Auto-retry {} succeeded]", attempt);
            self.send_output(&format!("[Auto-retry {} succeeded]\n", attempt));
        } else {
            let final_error = json.get("finalError").and_then(|v| v.as_str()).unwrap_or("unknown error");
            warn!("[Pi Auto-retry {} failed: {}]", attempt, final_error);
            self.send_output(&format!("[Auto-retry {} failed: {}]\n", attempt, final_error));
        }
    }

    fn handle_model_change(&self, json: &Value) {
        let provider = json.get("provider").and_then(|v| v.as_str()).unwrap_or("unknown");
        let model_id = json.get("modelId").and_then(|v| v.as_str()).unwrap_or("unknown");
        info!("[Pi Model changed to: {}/{}]", provider, model_id);
        self.send_output(&format!("\n[Model changed to: {}/{}]\n", provider, model_id));
    }

    fn handle_thinking_level_change(&self, json: &Value) {
        let level = json.get("thinkingLevel").and_then(|v| v.as_str()).unwrap_or("unknown");
        info!("[Pi Thinking level: {}]", level);
        self.send_output(&format!("\n[Thinking level: {}]\n", level));
    }

    fn handle_compaction(&self, json: &Value) {
        let summary = json.get("summary").and_then(|v| v.as_str()).unwrap_or("");
        let tokens_before = json.get("tokensBefore").and_then(|v| v.as_i64()).unwrap_or(0);
        let preview = &summary[..summary.len().min(100)];
        info!("[Pi Compacted {} tokens: {}]", tokens_before, preview);
        self.send_output(&format!("\n[Compacted {} tokens: {}]\n", tokens_before, preview));
    }

    fn handle_branch_summary(&self, json: &Value) {
        let summary = json.get("summary").and_then(|v| v.as_str()).unwrap_or("");
        let preview = &summary[..summary.len().min(100)];
        info!("[Pi Branched: {}]", preview);
        self.send_output(&format!("\n[Branched: {}]\n", preview));
    }

    fn handle_custom_message(&self, json: &Value) {
        let custom_type = json.get("customType").and_then(|v| v.as_str()).unwrap_or("extension");
        let content = json.get("content").and_then(|v| v.as_str()).unwrap_or("");
        let display = json.get("display").and_then(|v| v.as_bool()).unwrap_or(false);
        if display {
            info!("[Pi {}] {}", custom_type, content);
            self.send_output(&format!("\n[{}] {}\n", custom_type, content));
        }
    }

    fn handle_bash_execution(&self, json: &Value) {
        let message = json.get("message").or(json.get("data")).unwrap_or(json);
        let command = message.get("command").and_then(|v| v.as_str()).unwrap_or("");
        let output = message.get("output").and_then(|v| v.as_str()).unwrap_or("");
        let exit_code = message.get("exitCode").and_then(|v| v.as_i64());
        let cancelled = message.get("cancelled").and_then(|v| v.as_bool()).unwrap_or(false);
        let truncated = message.get("truncated").and_then(|v| v.as_bool()).unwrap_or(false);

        info!("[Pi Bash: {}]", command);
        self.send_output(&format!("\n[Bash: {}]\n", command));

        if !output.is_empty() {
            let display = if output.len() > 500 {
                format!("{}...[truncated]", &output[..500])
            } else {
                output.to_string()
            };
            self.send_output(&format!("{}\n", display));
        }

        if let Some(code) = exit_code {
            self.send_output(&format!("[Exit code: {}]\n", code));
        }
        if cancelled {
            self.send_output("[Command cancelled]\n");
        }
        if truncated {
            self.send_output("[Output truncated]\n");
        }
    }

    fn send_output(&self, text: &str) {
        if let Some(ref consumer) = self.consumer {
            consumer(text);
        }
    }
}
