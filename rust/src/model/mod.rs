use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Exercise {
    pub name: String,
    pub language: String,
    #[serde(default)]
    pub source_path: Option<PathBuf>,
    #[serde(default)]
    pub test_path: Option<PathBuf>,
    #[serde(default)]
    pub reference_path: Option<PathBuf>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExerciseResult {
    pub exercise_name: String,
    pub language: String,
    pub success: bool,
    #[serde(rename = "exitCode")]
    pub exit_code: Option<i32>,
    pub output: String,
    pub duration_ms: u64,
    pub start_time: String,
    pub end_time: String,
    pub error_message: Option<String>,
    pub trace: Option<String>,
}

impl ExerciseResult {
    pub fn builder() -> ExerciseResultBuilder {
        ExerciseResultBuilder::new()
    }
}

#[derive(Default)]
pub struct ExerciseResultBuilder {
    exercise_name: Option<String>,
    language: Option<String>,
    success: bool,
    exit_code: Option<i32>,
    output: String,
    duration_ms: u64,
    start_time: Option<String>,
    end_time: Option<String>,
    error_message: Option<String>,
    trace: Option<String>,
}

impl ExerciseResultBuilder {
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
        self.exit_code = Some(exit_code);
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

    pub fn build(self) -> ExerciseResult {
        ExerciseResult {
            exercise_name: self.exercise_name.unwrap_or_default(),
            language: self.language.unwrap_or_default(),
            success: self.success,
            exit_code: self.exit_code,
            output: self.output,
            duration_ms: self.duration_ms,
            start_time: self.start_time.unwrap_or_default(),
            end_time: self.end_time.unwrap_or_default(),
            error_message: self.error_message,
            trace: self.trace,
        }
    }
}

// Log entry types

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum LogEntry {
    #[serde(rename = "queue-operation")]
    QueueOperation(QueueOperationEntry),
    #[serde(rename = "user")]
    User(UserEntry),
    #[serde(rename = "assistant")]
    Assistant(AssistantEntry),
    #[serde(rename = "system")]
    System(SystemEntry),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BaseLogEntry {
    pub uuid: Option<String>,
    pub parent_uuid: Option<String>,
    pub is_sidechain: Option<bool>,
    pub user_type: Option<String>,
    pub cwd: Option<String>,
    pub session_id: Option<String>,
    pub version: Option<String>,
    pub git_branch: Option<String>,
    pub timestamp: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueueOperationEntry {
    #[serde(flatten)]
    pub base: BaseLogEntry,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemEntry {
    #[serde(flatten)]
    pub base: BaseLogEntry,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserEntry {
    #[serde(flatten)]
    pub base: BaseLogEntry,
    pub source_tool_assistant_uuid: Option<String>,
    pub tool_use_result: Option<serde_json::Value>,
    pub agent_id: Option<String>,
    pub todos: Option<serde_json::Value>,
    pub slug: Option<String>,
    pub message: Option<Message>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssistantEntry {
    #[serde(flatten)]
    pub base: BaseLogEntry,
    pub error: Option<String>,
    pub slug: Option<String>,
    pub message: Option<Message>,
    pub is_api_error_message: bool,
    pub agent_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub context_management: Option<ContextManagement>,
    pub id: Option<String>,
    pub r#type: Option<String>,
    pub role: Option<String>,
    pub model: Option<String>,
    pub stop_reason: Option<String>,
    pub stop_sequence: Option<String>,
    pub usage: Option<Usage>,
    pub container: Option<Container>,
    pub content: Option<Vec<Content>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextManagement {
    pub active_form: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Usage {
    #[serde(rename = "cache_read_input_tokens")]
    pub cache_read_input_tokens: u64,
    #[serde(rename = "cache_creation_input_tokens")]
    pub cache_creation_input_tokens: u64,
    #[serde(rename = "input_tokens")]
    pub input_tokens: u64,
    #[serde(rename = "output_tokens")]
    pub output_tokens: u64,
    #[serde(rename = "cache_creation")]
    pub cache_creation: Option<CacheCreation>,
    #[serde(rename = "server_tool_use")]
    pub server_tool_use: Option<ServerToolUse>,
    #[serde(rename = "service_tier")]
    pub service_tier: Option<ServiceTier>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheCreation {
    #[serde(rename = "input_tokens")]
    pub input_tokens: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerToolUse {
    #[serde(rename = "input_tokens")]
    pub input_tokens: u64,
    #[serde(rename = "output_tokens")]
    pub output_tokens: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceTier {
    pub r#type: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Container {
    pub id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Content {
    #[serde(rename = "text")]
    Text(TextContent),
    #[serde(rename = "thinking")]
    Thinking(ThinkingContent),
    #[serde(rename = "tool_use")]
    ToolUse(ToolUseContent),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextContent {
    pub text: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThinkingContent {
    #[serde(rename = "thinking_id")]
    pub thinking_id: Option<String>,
    pub thinking: Option<String>,
    #[serde(rename = "thinking_data_version")]
    pub thinking_data_version: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolUseContent {
    pub id: Option<String>,
    pub name: Option<String>,
    pub input: Option<serde_json::Value>,
}
